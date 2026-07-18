# DR Runbook: シークレット復旧（ESO + AWS SSM Parameter Store）

クラスタ全損時に、git のマニフェストと AWS だけからすべての K8s Secret を復旧する手順。
2026-07-16 に kind での演習で検証済み（`scripts/dr-drill.sh` で再演習できる）。

## アーキテクチャと前提

```
AWS SSM Parameter Store（SecureString、/bons8i/* 階層、ap-northeast-1）
      ↓ 読み取り: IAM ユーザー bons8i-eso
ESO（ClusterSecretStore aws-ssm + ExternalSecret 群 = すべて git 管理）
      ↓ 生成・同期
K8s Secret（ワークロードが参照）
```

前提: AWS アカウントに IAM 管理権限でアクセスできること（アクセスキーは保管していない — 手順 2 で再発行する）。

> **注意（PAT の共用）**: SSM の GitHub PAT（`/bons8i/monitoring/alertmanager-to-github/github-token`）は、ESO 経由のクラスタ内 alertmanager-to-github に加えて**外形監視 Lambda（external probe、`infra/aws/`）も直接読んで共用している**。この PAT を失効させると両方が同時に止まる。ローテーションは SSM の値を更新すればよく、Lambda は毎回の実行時に SSM から読み直すため再デプロイ不要（クラスタ側は ExternalSecret の再同期を確認する）。

## 復旧手順

新クラスタの kubectl context で作業する。

### 1. ArgoCD をブートストラップし ESO を SYNC

```bash
kubectl apply -k clusters/pi/argocd/bootstrap
kubectl apply -f clusters/pi/argocd/root-app.yaml
kubectl -n argocd get secret argocd-initial-admin-secret -o jsonpath='{.data.password}' | base64 -d; echo
kubectl -n argocd port-forward svc/argocd-server 8080:443
```

`https://localhost:8080` に admin でログインし、root App を SYNC → 子 App `external-secrets` を SYNC。3 Deployment（controller / webhook / cert-controller）の Running を確認する。

### 2. アクセスキーを再発行

```bash
read -r AWS_AK AWS_SK < <(aws iam create-access-key --user-name bons8i-eso \
  --query 'AccessKey.[AccessKeyId,SecretAccessKey]' --output text)
echo "id 長: ${#AWS_AK}（期待 20）/ secret 長: ${#AWS_SK}（期待 40）"
```

- **長さが期待値と違えば停止**（空・破損のまま先に進むと不正な Secret が黙って作られる）
- `LimitExceeded` が出たら既存キーが 2 本ある。使われていない方を削除してから再実行（手順 5 参照）

### 3. クレデンシャル Secret を投入

```bash
kubectl -n external-secrets create secret generic aws-ssm-credentials \
  --from-literal=access-key-id="$AWS_AK" \
  --from-literal=secret-access-key="$AWS_SK"
unset AWS_AK AWS_SK
```

### 4. 残りの App を SYNC して検証

手順 1 と同じ ArgoCD UI で残りの App を SYNC する。ExternalSecret 群が SSM から Secret を再生成する。

```bash
kubectl get externalsecrets -A
```

全行が `SecretSynced / True` になれば復旧完了。

- 発行直後のキーは IAM の伝播遅延で **数分待つ**

### 5. 使われていないキーの掃除

クラスタが現に使っているキーだけを残す。

```bash
IN_USE=$(kubectl -n external-secrets get secret aws-ssm-credentials -o jsonpath='{.data.access-key-id}' | base64 -d)
for id in $(aws iam list-access-keys --user-name bons8i-eso --query 'AccessKeyMetadata[*].AccessKeyId' --output text); do
  [ "$id" != "$IN_USE" ] && aws iam delete-access-key --user-name bons8i-eso --access-key-id "$id" && echo "deleted: $id"
done
unset IN_USE
```

## キーローテーション

漏洩時・定期ローテーションの手順。復旧手順 2〜3 の後に Secret を作り直し、即時反映を確認して旧キーを消す。

```bash
kubectl -n external-secrets delete secret aws-ssm-credentials
# → 復旧手順 3 で再作成
kubectl -n monitoring annotate externalsecret vmks-grafana-admin force-sync=$(date +%s) --overwrite
kubectl -n monitoring get externalsecret vmks-grafana-admin   # SecretSynced / LAST SYNC 更新を確認
# → 復旧手順 5 で旧キーを削除
```

漏洩対応の場合は先に `aws iam update-access-key --status Inactive` で旧キーを無効化してから入れ替える。

## トラブルシューティング

| 症状 | 原因 | 対処 |
|---|---|---|
| `UnrecognizedClientException` | キー ID 自体が AWS に不明。①発行直後の伝播遅延 ②値の混入・破損 | ①数分待つ ②Secret 内の key-id 長が 20 / `AKIA` 始まりかを確認して作り直す |
| `AccessDenied` | IAM ポリシー不足 | `bons8i-eso` のインラインポリシー（`/bons8i/*` への GetParameter / GetParametersByPath）を確認 |
| ExternalSecret が `SecretSyncedError` のまま | 上記 2 つ、または SSM パラメータ名の不一致 | `kubectl describe externalsecret` のイベントでエラー本文を確認 |
| namespace 削除が Terminating で固まる | ExternalSecret の finalizer を外す controller が先に消えている | **削除は「CR → controller」の順**が原則。詰まったら `kubectl patch externalsecret <name> --type merge -p '{"metadata":{"finalizers":null}}'` |
| `create-access-key` が `LimitExceeded` | IAM ユーザーの同時キー上限（2 本） | 復旧手順 5 で未使用キーを削除 |

## 演習

```bash
scripts/dr-drill.sh
```

使い捨ての kind クラスタに対して復旧手順を全自動で再演し、本番クラスタと全 Secret のハッシュ一致を検証する。
演習用アクセスキーと kind クラスタは終了時（失敗時も）に自動で片付く。本番クラスタへの書き込みは行わない。
