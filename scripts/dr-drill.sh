#!/usr/bin/env bash
set -euo pipefail
export AWS_PAGER=""

IAM_USER="${IAM_USER:-bons8i-eso}"
PROD_CONTEXT="${PROD_CONTEXT:-pi}"
DRILL_CLUSTER="${DRILL_CLUSTER:-bons8i-dr-drill}"
DRILL_CONTEXT="kind-${DRILL_CLUSTER}"

log() { printf '\n==> %s\n' "$*"; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

cd "$(git rev-parse --show-toplevel)"

for cmd in aws kind kubectl helm docker jq git; do
  command -v "$cmd" >/dev/null || die "$cmd が見つかりません"
done
docker info >/dev/null 2>&1 || die "docker daemon が起動していません"
aws sts get-caller-identity >/dev/null || die "aws CLI の認証が通りません（AWS_PROFILE を確認）"
kubectl --context "$PROD_CONTEXT" get ns >/dev/null || die "本番 context ($PROD_CONTEXT) に到達できません"
if kind get clusters 2>/dev/null | grep -qx "$DRILL_CLUSTER"; then
  die "kind クラスタ $DRILL_CLUSTER が既に存在します。前回の残骸なら 'kind delete cluster --name $DRILL_CLUSTER' で削除してください"
fi

KEY_COUNT=$(aws iam list-access-keys --user-name "$IAM_USER" --query 'length(AccessKeyMetadata)' --output text)
if [ "$KEY_COUNT" -ge 2 ]; then
  die "IAM ユーザー $IAM_USER のアクセスキーが上限（2 本）に達しています。docs/runbook/dr-secrets.md の手順 5 で未使用キーを削除してから再実行してください"
fi

ESO_VERSION=$(awk '/targetRevision:/ {print $2; exit}' clusters/pi/argocd/apps/external-secrets.yaml)
[ -n "$ESO_VERSION" ] || die "ESO の chart バージョンを clusters/pi/argocd/apps/external-secrets.yaml から特定できません"

ES_FILES=$(find clusters/pi -name 'externalsecret-*.yaml' | sort)
[ -n "$ES_FILES" ] || die "ExternalSecret のマニフェストが見つかりません"

DRILL_KEY_ID=""
cleanup() {
  set +e
  log "後片付け"
  if [ -n "$DRILL_KEY_ID" ]; then
    aws iam delete-access-key --user-name "$IAM_USER" --access-key-id "$DRILL_KEY_ID" \
      && echo "演習用アクセスキーを削除しました"
  fi
  kind delete cluster --name "$DRILL_CLUSTER" >/dev/null 2>&1 \
    && echo "演習クラスタ $DRILL_CLUSTER を削除しました"
}
trap cleanup EXIT

log "1/6 演習クラスタを作成 ($DRILL_CLUSTER)"
kind create cluster --name "$DRILL_CLUSTER" --wait 120s

log "2/6 ESO chart $ESO_VERSION をインストール"
helm repo add external-secrets https://charts.external-secrets.io --force-update >/dev/null
helm install external-secrets external-secrets/external-secrets --version "$ESO_VERSION" \
  -n external-secrets --create-namespace --kube-context "$DRILL_CONTEXT" --wait --timeout 5m >/dev/null

log "3/6 演習用アクセスキーを発行して投入"
read -r DRILL_KEY_ID DRILL_KEY_SECRET < <(aws iam create-access-key --user-name "$IAM_USER" \
  --query 'AccessKey.[AccessKeyId,SecretAccessKey]' --output text)
if [ "${#DRILL_KEY_ID}" -ne 20 ] || [ "${#DRILL_KEY_SECRET}" -ne 40 ]; then
  die "create-access-key の出力が不正です（id 長=${#DRILL_KEY_ID} / secret 長=${#DRILL_KEY_SECRET}）"
fi
kubectl --context "$DRILL_CONTEXT" -n external-secrets create secret generic aws-ssm-credentials \
  --from-literal=access-key-id="$DRILL_KEY_ID" \
  --from-literal=secret-access-key="$DRILL_KEY_SECRET" \
  >/dev/null
unset DRILL_KEY_SECRET
echo "投入完了（キーは演習終了時に自動削除されます）"

log "4/6 git の宣言を apply"
for ns in $(awk '/^  namespace:/ {print $2}' $ES_FILES | sort -u); do
  kubectl --context "$DRILL_CONTEXT" create namespace "$ns" --dry-run=client -o yaml \
    | kubectl --context "$DRILL_CONTEXT" apply -f - >/dev/null
done
kubectl --context "$DRILL_CONTEXT" apply -k clusters/pi/external-secrets/
for f in $ES_FILES; do
  kubectl --context "$DRILL_CONTEXT" apply -f "$f"
done

log "5/6 同期を待機（発行直後のキーは IAM 伝播で数十秒かかることがある）"
kubectl --context "$DRILL_CONTEXT" wait --for=condition=Ready externalsecret --all -A --timeout=300s
kubectl --context "$DRILL_CONTEXT" get externalsecrets -A

log "6/6 本番 ($PROD_CONTEXT) とのハッシュ照合"
secret_hash() {
  kubectl --context "$1" -n "$2" get secret "$3" -o json \
    | jq -Sc '.data' | shasum -a 256 | awk '{print $1}'
}
FAIL=0
for f in $ES_FILES; do
  ns=$(awk '/^  namespace:/ {print $2; exit}' "$f")
  name=$(awk '/^  target:$/ {getline; print $2; exit}' "$f")
  prod_hash=$(secret_hash "$PROD_CONTEXT" "$ns" "$name")
  drill_hash=$(secret_hash "$DRILL_CONTEXT" "$ns" "$name")
  if [ "$prod_hash" = "$drill_hash" ]; then
    echo "PASS: $ns/$name"
  else
    echo "FAIL: $ns/$name"
    FAIL=1
  fi
done

if [ "$FAIL" -ne 0 ]; then
  die "ハッシュ不一致があります。本番とドリルで Secret の内容が異なります"
fi
log "DR 演習成功: すべての Secret が git + AWS のみから再現されました"
