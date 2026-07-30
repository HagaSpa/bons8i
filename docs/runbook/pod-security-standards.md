# Runbook: Pod Security Standards の運用

namespace 単位で admission レベルの下限を強制している（#129）。
各ワークロードが `securityContext` で自己申告する #120 とは補完関係で、こちらは
「namespace として下限を強制する」側。競合しない。

強制しているのは kube-apiserver 内蔵の **Pod Security Admission**（PSA）で、
追加のコンポーネントは動いていない。設定インターフェースは namespace のラベルだけ。

## 現在の水準

| namespace | enforce | ラベルの置き場所 |
|---|---|---|
| status-page | `restricted` / `v1.36` | `clusters/pi/status-page/namespace.yaml` |
| cloudflared | `restricted` / `v1.36` | `clusters/pi/cloudflared/namespace.yaml` |
| external-secrets | `restricted` / `v1.36` | `clusters/pi/external-secrets/namespace.yaml` |
| argocd | `restricted` / `v1.36` | `clusters/pi/argocd/bootstrap/namespace.yaml` |
| monitoring | なし | — |
| local-path-storage | なし | — |
| kube-system | なし | — |
| default / kube-public / kube-node-lease / cilium-secrets | なし（未着手） | — |

ラベルが無い namespace は `privileged` と同じ扱い（無制限）になる。

`warn` と `audit` は使っていない。`warn` の警告は Pod を作った API クライアントに返るが、
このクラスタの Pod はすべて controller 経由で作られるため警告が controller のログに落ちて手元に見えない。
`audit` は kube-apiserver に audit ログの設定（`--audit-policy-file` 等）が無いため記録先が存在しない。

### 除外している理由

- **monitoring** … node-exporter が `hostNetwork` / `hostPID` / `hostPort 9100` / `hostPath` ×3 を使うため
  `restricted` だけでなく `baseline` も通らない。この DaemonSet を privileged な専用 namespace へ隔離するまで
  monitoring 全体に enforce を貼れない
- **local-path-storage** … provisioner 本体が `restricted` 非適合。加えて PVC 作成時に生える helper pod が
  `hostPath` をマウントするため `baseline` も通らない
- **kube-system** … cilium-agent が `hostNetwork` + `hostPath` ×12 + `SYS_ADMIN` / `SYS_MODULE` / `NET_ADMIN` 等の
  capability 追加を要求する。control plane の static pod（etcd / apiserver / controller-manager / scheduler）も
  `hostNetwork` + `hostPath`。どちらも `baseline` 不可

PSS には Pod 単位の除外が無い。`AdmissionConfiguration` の `exemptions` も
namespace / username / runtimeClass 単位で、DaemonSet の Pod を作るのは `daemon-set-controller` なので
そこを exempt すると全 DaemonSet が素通しになる。**特権が必要なワークロードは専用 namespace へ隔離する**のが
PSS 運用の唯一の出口。

## enforce を貼る前の事前チェック

対象 namespace の既存 Pod を一括評価する。`--dry-run=server` なのでクラスタは変わらない。

```bash
kubectl label ns <ns> \
  pod-security.kubernetes.io/enforce=restricted \
  pod-security.kubernetes.io/enforce-version=v1.36 \
  --overwrite --dry-run=server
```

違反があれば Pod 名と違反項目が警告で返る。

```
Warning: existing pods in namespace "status-page" violate the new PodSecurity enforce level "restricted:v1.36"
Warning: status-page-74c56cd44c-z8xjz: seccompProfile
```

警告が 1 行も出なければ適合している。

このチェックが答えるのは「今この namespace に enforce を貼ったら何が弾かれるか」だけ。
既に同じ水準が貼られている namespace では、水準が変わらないため警告が返らない可能性がある
（既存 Pod の評価は enforce の水準が変わったときに走る）。

enforce 済み namespace の棚卸しや、まだ存在しない Pod spec の判定には次節の probe namespace 方式を使う。
enforce は既存 Pod を追い出さないので、**貼った後に違反 Pod が残り続けることはある**
（ラベルだけ貼って Pod 側を直さなかった場合、その Pod は次に作り直されるまで生き残る）。

## probe namespace で任意の Pod を試す

未デプロイのワークロードを試したいとき、複数の水準を比較したいとき、
enforce 済み namespace を棚卸ししたいときに使う。

```bash
kubectl create ns pss-probe
kubectl label ns pss-probe pod-security.kubernetes.io/enforce=restricted --overwrite

kubectl -n <調べたい ns> get pods -o json \
  | jq -c '.items[] | {apiVersion:"v1", kind:"Pod",
      metadata:{name:("probe-"+.metadata.name), namespace:"pss-probe"},
      spec:(.spec | del(.serviceAccountName, .serviceAccount, .nodeName, .tolerations))}' \
  | while read -r pod; do echo "$pod" | kubectl apply -f - --dry-run=server; done

kubectl delete ns pss-probe
```

`serviceAccountName` を落とすのは、probe namespace に同名の ServiceAccount が無く
PSA より先に ServiceAccount の admission で失敗してしまうため。

`restricted` の代わりに `baseline` を貼れば「どこまで緩めれば通るか」が分かる。

**Deployment / CronJob を dry-run しても弾かれない。** `enforce` が評価するのは Pod オブジェクトだけで、
Pod を作る側のリソースは対象外。実際の失敗は ReplicaSet / Job のイベントに現れる。

## enforce 済み namespace での作業

### 新しいワークロードを足すとき

`restricted` が要求するのは以下。1 つ欠けても Pod が作れない。

- `runAsNonRoot: true`
- `allowPrivilegeEscalation: false`
- `capabilities.drop: ["ALL"]`（再追加できるのは `NET_BIND_SERVICE` のみ）
- `seccompProfile.type: RuntimeDefault`（または `Localhost`）
- ホスト名前空間（`hostNetwork` / `hostPID` / `hostIPC`）と `hostPort` を使わない
- ボリュームは configMap / secret / emptyDir / downwardAPI / projected / persistentVolumeClaim / csi / ephemeral のみ
  （`hostPath` は不可）

このリポジトリの既存ワークロードは `securityContext` をコンテナレベルに書いている。
init コンテナを足す場合はそちらにも同じものが必要（Pod レベルに書けば両方に効く）。

### デバッグ Pod

`kubectl run` と既定の `kubectl debug`（`--profile=legacy`）は弾かれる。

```bash
kubectl debug -n <ns> <pod> -it --image=nicolaka/netshoot --profile=restricted
```

### バージョンを上げるとき

ArgoCD（`clusters/pi/argocd/bootstrap/kustomization.yaml` の `ref`）、Helm chart、k8s 本体を上げる PR では、
新しい Pod spec が `restricted` に適合するか分からない。上流が特権を要求するようになると
**merge した瞬間に Pod が作れなくなる**（既存 Pod は動き続けるので気づきにくい）。

上げる PR では probe namespace 方式で新バージョンの Pod spec を先に当てる。

### k8s を上げたとき

`enforce-version` を `v1.36` にピン留めしているので、k8s を上げても判定基準は固定されたままになる。
上げた後に各 namespace の `enforce-version` も追従させる。放置すると新しい検査項目が効かない。

値は `latest` か `v1.<minor>` のみ有効。`1.36` のようにプレフィックスを落とすと
`must be "latest" or "v1.x"` で Namespace 自体が invalid になる。

## 緊急脱出

enforce で Pod が作れなくなって復旧を優先したいときは、ラベルを直接外す。

```bash
kubectl label ns <ns> pod-security.kubernetes.io/enforce-
```

`selfHeal` が切れている App（cloudflared / external-secrets / argocd）では、
外したラベルは次に Git 側の変更が入るまで戻らない。

**`status-page` だけは `selfHeal: true` なので数分で貼り直される。**
status-page で脱出するには Application 側を止める必要がある。

```bash
kubectl patch application status-page -n argocd --type=merge \
  -p '{"spec":{"syncPolicy":{"automated":{"selfHeal":false}}}}'
```

argocd namespace で ArgoCD 自身の Pod が作れなくなった場合は、ラベルを外せば
`kubectl` だけで復旧できる。それでも駄目なら bootstrap をやり直す（`docs/runbook/argocd-sync.md`）。

## Namespace が prune の管理下にあること

PSS ラベルを載せた Namespace はすべて ArgoCD の管理対象リソースになっている。
`Prune=false` は意図的に付けていないので、**kustomization の `resources` から Namespace を消すコミットが
merge されると namespace ごと削除される**（中の Deployment と Secret も一緒に消える）。

`selfHeal: false` は Git 変更時の auto-sync を止めないため、ArgoCD 側に確認の余地は無い。
実質的なガードレールは merge 前に PR の diff を見ることだけ。

argocd namespace が消えた場合は全 Application（root 含む）も一緒に消えるため、
手動での再ブートストラップが必要になる。

## トラブルシューティング

| 症状 | 原因 | 対処 |
|---|---|---|
| Deployment は Synced なのに Pod が増えない | PSA が Pod を拒否している。Deployment 自体は admission を通る | `kubectl -n <ns> describe rs <rs>` のイベントで `Forbidden` と違反項目を確認 |
| `kubectl run` が Forbidden | enforce 済み namespace | `kubectl debug --profile=restricted` を使う |
| 事前チェックで警告が出ないのに Pod が弾かれる | 同じ水準が既に貼られていて既存 Pod の評価がスキップされた | probe namespace 方式で確認 |
| `seccompProfile` の違反だけが出る | `#120` の securityContext 展開に seccomp が含まれていなかった | `securityContext.seccompProfile.type: RuntimeDefault` を追加 |
| Namespace が invalid で sync 失敗 | `enforce-version` の値が `v1.x` 形式でない | `v1.36` のように `v` を付ける |
| sync で `conflict with "kubectl-client-side-apply"` | client-side で作られた既存 namespace を SSA で採用する経路 | ArgoCD UI の Sync で `Force`、または `kubectl apply --server-side --force-conflicts` で一度採用させる |

## 状態確認

```bash
kubectl get ns -L pod-security.kubernetes.io/enforce -L pod-security.kubernetes.io/enforce-version
```

`seccompProfile` の設定状況（#129 の時点では、enforce 対象にした 4 namespace で不足していたのはこれだけだった。
monitoring の VM operator 生成 Pod・node-exporter・local-path-provisioner はこれ以外にも不足がある）:

```bash
kubectl get pods -A -o json | jq -r '.items[]
  | (.spec.securityContext.seccompProfile.type) as $pod
  | .metadata.namespace + "/" + .metadata.name + "  "
  + ([(.spec.initContainers // [])[], .spec.containers[]
      | .name + ":" + (.securityContext.seccompProfile.type // $pod // "なし")] | join(" "))'
```

`seccompProfile` は Pod レベル（`spec.securityContext`）とコンテナレベルのどちらでも要件を満たし、
コンテナレベルが優先される。上の式はその解決順を再現している。initContainer も評価対象。
