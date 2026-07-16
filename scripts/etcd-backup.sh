#!/usr/bin/env bash
set -euo pipefail
export AWS_PAGER=""

CONTEXT="${ETCD_BACKUP_CONTEXT:-pi}"
BUCKET="${ETCD_BACKUP_BUCKET:?環境変数 ETCD_BACKUP_BUCKET にバケット名を設定してください}"
PREFIX="${ETCD_BACKUP_PREFIX:-etcd}"

WORK=/var/lib/etcd/snapshot-work.db
STAGE=/tmp/etcd-snapshot-stage.db
STAMP=$(date -u +%Y%m%dT%H%M%SZ)

log() { printf '\n==> %s\n' "$*"; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

for cmd in kubectl ssh scp aws; do
  command -v "$cmd" >/dev/null || die "$cmd が見つかりません"
done

POD=$(kubectl --context "$CONTEXT" -n kube-system get pods -l component=etcd -o jsonpath='{.items[0].metadata.name}')
NODE=$(kubectl --context "$CONTEXT" -n kube-system get pod "$POD" -o jsonpath='{.spec.nodeName}')
[ -n "$POD" ] && [ -n "$NODE" ] || die "etcd Pod / ノードを特定できません"
SSH_HOST="${ETCD_BACKUP_SSH_HOST:-$NODE}"

TMP=$(mktemp -d)
cleanup() {
  set +e
  rm -rf "$TMP"
  ssh -t "$SSH_HOST" "sudo rm -f $WORK $STAGE" >/dev/null 2>&1
}
trap cleanup EXIT

log "1/5 snapshot 取得（${POD} → ${WORK}）"
kubectl --context "$CONTEXT" -n kube-system exec "$POD" -- etcdctl \
  --endpoints=https://127.0.0.1:2379 \
  --cacert=/etc/kubernetes/pki/etcd/ca.crt \
  --cert=/etc/kubernetes/pki/etcd/server.crt \
  --key=/etc/kubernetes/pki/etcd/server.key \
  snapshot save "$WORK"

log "2/5 整合性検証（etcdutl snapshot status）"
kubectl --context "$CONTEXT" -n kube-system exec "$POD" -- etcdutl snapshot status "$WORK" --write-out table

log "3/5 ノード ${SSH_HOST} から取り出し（sudo パスワードを求められたら入力）"
ssh -t "$SSH_HOST" "sudo install -m 644 $WORK $STAGE"
scp -q "$SSH_HOST:$STAGE" "$TMP/snapshot.db"
REMOTE_SIZE=$(ssh "$SSH_HOST" "stat -c %s $STAGE")
LOCAL_SIZE=$(wc -c < "$TMP/snapshot.db" | tr -d ' ')
if [ "$REMOTE_SIZE" != "$LOCAL_SIZE" ]; then
  die "転送サイズ不一致（node=${REMOTE_SIZE} local=${LOCAL_SIZE}）"
fi
log "サイズ一致: ${LOCAL_SIZE} bytes"

log "4/5 S3 へアップロード"
aws s3 cp "$TMP/snapshot.db" "s3://$BUCKET/$PREFIX/$STAMP.db"

log "5/5 アップロード確認"
aws s3 ls "s3://$BUCKET/$PREFIX/$STAMP.db"

log "完了: s3://$BUCKET/$PREFIX/$STAMP.db"
