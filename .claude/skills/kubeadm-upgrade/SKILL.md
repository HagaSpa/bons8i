---
name: kubeadm-upgrade
description: Pi クラスタ（cp1）の kubeadm パッチアップグレードを実施する。「kubeadm を上げる」「k8s のパッチ更新」「kubeadm upgrade」「Issue の kubeadm 定期メンテ」を求められたときに使う。ターゲット決定から etcd スナップショット、apply、kubelet 更新、検証、Issue への記録までを決定的な手順で通す。マイナーバージョン跨ぎ（1.36 → 1.37）には使わない。
---

# kubeadm upgrade（Pi クラスタ）

3〜4 ヶ月周期の定期メンテ。**パッチアップグレード専用**（例: v1.36.2 → v1.36.4）。

## 前提

この手順は次の構成に固定されている。変わっていたら手順を先に見直す。

- **単一ノード** `cp1`（control-plane 兼 worker）。worker ノードは無い
- **Cilium の kubeProxyReplacement**。kube-proxy は DaemonSet も ConfigMap も存在しない
- `kube-system/kubeadm-config` の `ClusterConfiguration` に `proxy.disabled: true` が入っている
- ssh は `hagaspa@cp1`（ユーザー名を省くと publickey で弾かれる）。ssh 越しの sudo が対話を求める場合は各コマンドを分けて実行する
- `kubeadm` / `kubelet` / `kubectl` は 3 つとも apt hold されている
- kubectl / ssh はホスト名解決に Tailscale MagicDNS を使うため、サンドボックス下では実行できない

## 変数

以降のコマンドはこの 4 つを置き換えて使う。

```
TARGET=v1.36.5
PKG=1.36.5-1.1
NODE=cp1
SSH=hagaspa@cp1
```

## Step 1: ターゲットを決める

```
ssh $SSH 'sudo kubeadm upgrade plan'
ssh $SSH 'apt-cache madison kubeadm kubelet kubectl | grep <パッチ番号>'
```

- `Target version` を採る。plan 実行時点で新しいパッチが出ていたら**最新パッチまで上げる**（周期が長いので 1 パッチ遅れを残さない）
- **3 パッケージすべてに同じリビジョンがあること**を確認する。kubelet / kubectl 側が無いと Step 7 で詰まる
- `CoreDNS` / `etcd` が CURRENT = TARGET であること。差があるならこの手順の範囲外（addon 入れ替えの調査が必要）
- [CHANGELOG-1.36.md の Urgent Upgrade Notes](https://github.com/kubernetes/kubernetes/blob/master/CHANGELOG/CHANGELOG-1.36.md#urgent-upgrade-notes) を該当パッチの節まで読む。メトリクス名の変更などが挙がっていたら参照箇所を潰す

```
kubectl get vmrule -A -o yaml | grep -c '<メトリクス名>'
kubectl -n monitoring get cm -o yaml | grep -c '<メトリクス名>'
```

**plan の表に出る `kube-proxy` 行と `configmaps "kube-proxy" not found` の警告は判定材料にならない。** 表は固定のコンポーネント一覧から作られ、警告は component config のロード時に出るもので、どちらも addon フェーズを実行するかどうかを反映しない。判定できるのは Step 3 の dry-run だけ。

## Step 2: kubeadm だけ上げる

クラスタは変更されない。

```
ssh $SSH "sudo apt-mark unhold kubeadm && sudo apt-get update && sudo apt-get install -y kubeadm=$PKG && sudo apt-mark hold kubeadm && kubeadm version && sudo apt-mark showhold"
```

合格条件: `kubeadm version` が TARGET、`showhold` に 3 パッケージが戻っている。

## Step 3: 事前確認（read-only）

### static pod マニフェストの差分

```
ssh $SSH "sudo kubeadm upgrade diff $TARGET --context-lines 2 | tee /tmp/upgrade-diff.log"
ssh $SSH "grep -E '^[+-]' /tmp/upgrade-diff.log | grep -vE '^(\+\+\+|---)'"
```

合格条件: 変更行が apiserver / controller-manager / scheduler の `image:` 6 行だけ。引数・volume・probe の差分が混ざったら、その内容を理解するまで apply しない。

### addon フェーズ

```
ssh $SSH "sudo kubeadm upgrade apply $TARGET --dry-run > /tmp/upgrade-dryrun.log 2>&1; grep -niE 'kube-proxy|node-proxier' /tmp/upgrade-dryrun.log"
```

合格条件: `[upgrade/addon] Skipping the addon/kube-proxy phase. The addon is disabled.` が出ていること。他のヒットは component config のロード試行とノードに残る古いイメージの一覧なので無関係。ServiceAccount / ClusterRoleBinding / ConfigMap / DaemonSet の**生成**が出たら `proxy.disabled: true` が効いていないので、それを直すまで進めない。

### kubelet 設定の差分

dry-run のディレクトリは既定では消えるので、固定して diff を取る。

```
ssh $SSH "sudo rm -rf /tmp/kubeadm-dryrun && sudo mkdir -p /tmp/kubeadm-dryrun && sudo env KUBEADM_UPGRADE_DRYRUN_DIR=/tmp/kubeadm-dryrun kubeadm upgrade apply $TARGET --dry-run > /dev/null 2>&1; sudo diff -u /var/lib/kubelet/config.yaml /tmp/kubeadm-dryrun/config.yaml; sudo diff -u /var/lib/kubelet/instance-config.yaml /tmp/kubeadm-dryrun/instance-config.yaml"
```

合格条件: 両方とも差分なし。`cgroupDriver` / `resolvConf` / `clusterDNS` / `containerRuntimeEndpoint` が変わるなら理由を潰してから進む。

dry-run ログの `--container-runtime-endpoint value from "kubeadm-flags.env", which is missing` は、ファイルではなく**フラグが未設定**（`KUBELET_KUBEADM_ARGS=""`）という意味。代わりに使われるデフォルト `unix:///var/run/containerd/containerd.sock` が現行値と一致すれば問題ない（`/var/run` は `/run` への symlink）。

終わったら片付ける。

```
ssh $SSH 'sudo rm -rf /tmp/kubeadm-dryrun'
```

## Step 4: アラートを確認する（silence は張らない）

apply 中は apiserver が 2 分程度落ちる。**silence を張らない**のが既定。`TargetDown`（10m）→ `KubeAPIDown`（15m）を「apply が詰まった」ことの検知として使うため。

発火域に入っていないことを毎回確認する。

```
kubectl get vmrule -A -o json | jq -r '.items[].spec.groups[].rules[] | select((.alert // "") != "") | select(.expr | test("apiserver|kube_api")) | [.alert, (.for // "-")] | @tsv' | sort -u
```

`for` の最短は `KubeAPIErrorBudgetBurn` の **2m** だが、これは `burnrate5m` と `burnrate1h` の両方が閾値超えという条件で、apiserver が落ちている間はリクエストが 0 になりエラー比率が上がらないため発火しない。実質の下限は `KubeAggregatedAPIDown` / `KubeAPITerminatedRequests` の 5m で、実測の API 断（1 分 45 秒）ではどれも発火しなかった。

判定: エラー比率ではなく**到達性そのもの**を見るアラートで `for` が 5m 未満のものが増えていたら、その分だけ silence を検討する。

## Step 5: etcd スナップショット

```
ETCD_BACKUP_BUCKET=bons8i-backup ETCD_BACKUP_SSH_HOST=$SSH ./scripts/etcd-backup.sh
```

合格条件: `5/5` まで通って `s3://bons8i-backup/etcd/<timestamp>.db` が出る。2/5 の `etcdutl snapshot status` で hash / revision / TOTAL KEYS が出ていること。

## Step 6: apply

事前状態を保存する。Step 7 の判定に使う。

```
kubectl get pods -A -o custom-columns='NS:.metadata.namespace,NAME:.metadata.name,RESTARTS:.status.containerStatuses[*].restartCount,START:.status.containerStatuses[*].state.running.startedAt' --sort-by=.metadata.name > /tmp/pods-pre.txt
```

別ターミナルで API 断を観測する。**`sleep` を必ず入れる**（入れないと数十秒で回り切って apply の窓を外す）。

```
while :; do printf '%s ' "$(date -u +%H:%M:%S)"; kubectl get --raw=/readyz 2>&1 | tail -1; sleep 2; done | tee /tmp/readyz.log
```

```
ssh $SSH "sudo kubeadm upgrade apply $TARGET --yes 2>&1 | tee /tmp/upgrade-apply.log"
```

合格条件: 末尾が `[upgrade] SUCCESS!`、途中に `Skipping the addon/kube-proxy phase`。`/readyz` の断が 5 分を超えたら発火域なので、その時点で切り分けに移る。

**apply は etcd も再起動する。** 証明書更新（etcd-server / etcd-peer / etcd-healthcheck-client）に伴って static pod が入れ替わるため、再起動するのは etcd → apiserver → controller-manager → scheduler の 4 つ。dry-run に etcd マニフェストの書き込みが出ないことを「触らない」と読まない。全 10 証明書の有効期限も 1 年更新される。

apply が作ったロールバック材料の場所を控える。

```
ssh $SSH 'sudo ls -dlt /etc/kubernetes/tmp/kubeadm-backup-manifests-*'
```

## Step 7: kubelet / kubectl（drain しない）

**drain はしない。** 単一ノードで退避先が無く、drain は Argo CD / 監視 / status page を落とすことそのものになる。`emptyDir` を持つ Pod が 10 個あるため `--delete-emptydir-data` も要求される。kubelet の restart では実行中コンテナは落ちない。

```
ssh $SSH "sudo apt-mark unhold kubelet kubectl && sudo apt-get update -qq && sudo apt-get install -y kubelet=$PKG kubectl=$PKG && sudo apt-mark hold kubelet kubectl && sudo systemctl daemon-reload && sudo systemctl restart kubelet && sudo apt-mark showhold && kubelet --version"
```

落ちていないことを毎回裏付ける。

```
kubectl get pods -A -o custom-columns='NS:.metadata.namespace,NAME:.metadata.name,RESTARTS:.status.containerStatuses[*].restartCount,START:.status.containerStatuses[*].state.running.startedAt' --sort-by=.metadata.name > /tmp/pods-post.txt
diff -u /tmp/pods-pre.txt /tmp/pods-post.txt
```

合格条件: control plane の 4 つ（apply で入れ替わった分）以外に差分が無いこと。他の Pod の `restartCount` / `startedAt` が動いていたら原因を追う。

## Step 8: 検証

```
kubectl get nodes -o wide
kubectl -n kube-system get pods -o custom-columns='NAME:.metadata.name,IMAGE:.spec.containers[0].image,RESTARTS:.status.containerStatuses[0].restartCount'
kubectl get pods -A | grep -vE 'Running|Completed|NAME'
kubectl -n kube-system get ds
kubectl -n kube-system get cm | grep -i proxy
kubectl get applications -n argocd -o custom-columns='NAME:.metadata.name,SYNC:.status.sync.status,HEALTH:.status.health.status'
ssh $SSH 'systemctl is-active kubelet'
kubectl -n kube-system get cm kubeadm-config -o jsonpath='{.data.ClusterConfiguration}' | grep -E 'kubernetesVersion|disabled'
curl -s -o /dev/null -w 'status-page HTTP %{http_code}\n' https://bons8i.hagaspa.com/
```

合格条件: ノードが TARGET / Ready、control plane 3 コンポーネントが TARGET のイメージ、異常 Pod 0、DaemonSet は cilium / cilium-envoy のみ、`kube-proxy` の ConfigMap なし、Argo CD が全 Synced / Healthy、kubelet active、`kubernetesVersion` が TARGET で `disabled: true` が残っている、外形 200。

## Step 9: API 断とアラートを実測する

```
kubectl -n monitoring port-forward svc/vmsingle-vmks-victoria-metrics-k8s-stack 18428:8428 &
START=$(( $(date -u +%s) - 3600 )); END=$(date -u +%s)
curl -s http://127.0.0.1:18428/api/v1/query_range --data-urlencode 'query=up{job="apiserver"}' --data-urlencode "start=$START" --data-urlencode "end=$END" --data-urlencode 'step=15s' | jq -r '.data.result[0].values[] | "\(.[0]) \(.[1])"' | awk '{ cmd="date -u -r " $1 " +%H:%M:%S"; cmd | getline t; close(cmd); print t, $2 }' | awk '$2!="1" || prev!="1" {print} {prev=$2}'
curl -s http://127.0.0.1:18428/api/v1/query --data-urlencode 'query=ALERTS{alertstate="firing"}' | jq -r '.data.result[] | "\(.metric.alertname) \(.metric.severity // "-")"' | sort | uniq -c
```

`up` が 1 に戻った時刻から断の長さを出す。欠測（サンプルが無い区間）も断として数える。firing が `Watchdog` と `InfoInhibitor` だけなら発火なし（両方とも設計上常時）。

```
gh issue list --repo HagaSpa/bons8i --state all --label alert --limit 10 --json number,title,createdAt --jq '.[] | [.number,.createdAt,.title] | @tsv'
```

当日作成の alert Issue が無いことを確認する。あれば内容を見て、この作業由来かを判定する。

終わったら port-forward を止める。

```
pkill -f 'port-forward svc/vmsingle-vmks'
```

## Step 10: 記録

kubeadm 更新の Issue にコメントで残す。含める項目:

- ターゲットの決定理由（plan 実行時点の最新パッチ）と Urgent Upgrade Notes の判定
- Step 3 の 3 つの合格条件の結果
- 実行したコマンド
- etcd スナップショットの S3 キー / revision
- API 断の実測値とアラートの発火有無
- Step 8 の検証結果
- ロールバック材料のパス
- 想定と違った点

定期メンテなので、完了したら Issue は close する（次回は新しい Issue を立てる）。

## ロールバック

apply が失敗した場合、退避されたマニフェストを戻す。パッチアップグレードでは変わるのは `image:` 行だけなので、これで元のバージョンに戻る。

```
ssh $SSH 'sudo ls -dlt /etc/kubernetes/tmp/kubeadm-backup-manifests-*'
ssh $SSH 'sudo cp /etc/kubernetes/tmp/kubeadm-backup-manifests-<timestamp>/*.yaml /etc/kubernetes/manifests/'
```

kubelet が static pod を作り直す。kubelet パッケージまで上げていた場合は apt で戻す。

```
ssh $SSH "sudo apt-mark unhold kubelet kubectl && sudo apt-get install -y --allow-downgrades kubelet=<旧リビジョン> kubectl=<旧リビジョン> && sudo apt-mark hold kubelet kubectl && sudo systemctl daemon-reload && sudo systemctl restart kubelet"
```

etcd のデータ復旧が必要なのは etcd の再起動が失敗した場合のみ。Step 5 のスナップショットから戻す。

## この手順を使わない場合

- **マイナーバージョン跨ぎ**（1.36 → 1.37）: version skew、addon（CoreDNS / etcd）の入れ替え、削除された API の棚卸しが増える。この手順は使わず、[公式手順](https://kubernetes.io/docs/tasks/administer-cluster/kubeadm/kubeadm-upgrade/)から組み直す
- **worker ノードが増えた場合**: drain しない判断が成立しなくなる（退避先ができる）。Step 7 を公式手順の drain / uncordon に戻す
- **CoreDNS / etcd に差分がある場合**: Step 1 で止めて addon の影響を先に調べる
