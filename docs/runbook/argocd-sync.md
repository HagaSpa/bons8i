# Runbook: ArgoCD の同期運用

全 App を auto-sync で運用している（2026-07-29 に基盤系へ拡大、#162 / #163）。
通常は手動 SYNC が不要だが、**ArgoCD 自身の設定を変えるときだけ例外がある**ので、そこをここに残す。

## 同期ポリシーの構成

| App | prune | selfHeal |
|---|---|---|
| status-page | true | true |
| それ以外すべて（root 含む） | true | false |

`selfHeal` を切っているのは、`kubectl edit` での挙動確認が数分で巻き戻るのを避けるため。
そのため live 側の drift は自動では戻らない（`OutOfSync` として検知はされる）。
ただし **Git 側に次の変更が入った時点で上書きされる**ので、手で入れた変更は永続しない。

## root-app.yaml を変えたら手動 apply

root App の source path は `clusters/pi/argocd/apps` で、`root-app.yaml` 自身はその外にある。
つまり **`root-app.yaml` はどの Application からも管理されていない**。merge しても誰も適用しない。

```bash
kubectl apply -f clusters/pi/argocd/root-app.yaml
```

`apps/*.yaml`（子 App の定義）は root が管理しているので、そちらは merge するだけで反映される。

## diff の計算方法を変えたら hard refresh

`syncPolicy` の変更は次の reconcile で即座に効く。
一方 `compare-options` や `ignoreDifferences` のように **diff の計算方法を変える設定はキャッシュが残り**、
通常の reconcile では再計算されない。

```bash
kubectl annotate application <app> -n argocd argocd.argoproj.io/refresh=hard --overwrite
```

## ServerSideApply は diff 戦略も変える

`ServerSideApply=true` を付けると、diff 戦略が Legacy（`last-applied-configuration` による 3-way diff）から
Structured-Merge Diff に**自動で切り替わる**。この戦略は CRD が定義した default 値を扱えないため、
API server に補完されたフィールドが差分として残り **常時 `OutOfSync` になる**。
ArgoCD 公式が "discontinued due to identified issues and challenges with CRDs that define default values" として
廃止予定にしている既知の問題で、設定ミスではない。

実際に踏んだ例（`monitoring-config`）:

- `VMRule` … 各 rule に `record: ""` が補完される
- `ExternalSecret` … `target.creationPolicy: Owner` / `target.deletionPolicy: Retain` / `remoteRef.conversionStrategy: Default` 等

対処は Server-Side Diff（SSA を dry-run して predicted live と比較する戦略）の有効化。
annotation を足したあと **hard refresh が必要**。

```yaml
metadata:
  annotations:
    argocd.argoproj.io/compare-options: ServerSideDiff=true
```

Helm chart ベースの App（`victoria-metrics` / `external-secrets` / `cilium`）は chart が default 値を
明示的に出力するため、SSA でも差分が出ていない。chart 更新後に `OutOfSync` が出たら同じ annotation を足す。

## Renovate との関係

`clusters/pi/argocd/apps/*.yaml` の chart バージョンは Renovate が管理している。
auto-sync 化により **PR の merge が即クラスタ適用**になった。特に `cilium` の chart 更新は
DaemonSet の rolling update を伴い、単ノードなので入れ替わりの間はネットワークが不通になる。

## トラブルシューティング

| 症状 | 原因 | 対処 |
|---|---|---|
| merge したのに反映されない | `root-app.yaml` の変更は ArgoCD 管理外 | `kubectl apply -f clusters/pi/argocd/root-app.yaml` |
| sync は `Succeeded` なのに `OutOfSync` が消えない | SSA により Structured-Merge Diff に切り替わり、CRD の default 値が差分扱いになっている | `compare-options: ServerSideDiff=true` を追加 → hard refresh |
| annotation を足したのに挙動が変わらない | diff 計算結果のキャッシュ | hard refresh |
| 手で `kubectl edit` した変更が消えた | Git 側の変更で auto-sync が走った | selfHeal は切ってあるが、Git 変更時の sync では上書きされる。残したい変更は Git に入れる |
| 反映が遅い | ArgoCD の polling は約 3 分間隔 | 待つ、または `refresh=normal` を打つ |
| Git にあるのにクラスタに存在しないリソースがある | auto-sync 化以前の手動 SYNC 漏れ | 全 App が `Synced` かを確認（`kubectl get applications -n argocd`） |

## 状態確認

```bash
kubectl get applications -n argocd -o custom-columns='NAME:.metadata.name,SYNC:.status.sync.status,HEALTH:.status.health.status,PRUNE:.spec.syncPolicy.automated.prune,SELFHEAL:.spec.syncPolicy.automated.selfHeal'
```

App が認識している差分の内訳:

```bash
kubectl get application <app> -n argocd -o json | yq -r '.status.resources[] | select(.status != "Synced") | [.kind, .name, .status] | join("  ")'
```
