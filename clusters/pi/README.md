# clusters/pi/

Bootstrap materials and GitOps manifests for the single-node **Raspberry Pi 5**
cluster (`kubeadm`, Ubuntu Server 24.04 arm64).

## Architecture

![Pi cluster architecture](bons8i-RaspberryPi5.architecture.drawio.svg)

Editable source: [`bons8i-RaspberryPi5.architecture.drawio`](bons8i-RaspberryPi5.architecture.drawio)
(open with [draw.io](https://app.diagrams.net/) / File > Open From > Device).

Every Kubernetes resource on the cluster is declared in this repository and
reconciled by ArgoCD (pull-based GitOps) — there is no remaining imperative
Kubernetes workload. The one inherent exception is the cluster substrate
itself (`kubeadm`, `containerd`): it sits below the Kubernetes API, so it is
a one-time imperative bootstrap step by nature, not a GitOps target.

## Components

| Component | Purpose | Delivery method | Status |
|---|---|---|---|
| `kubeadm` + `containerd` | Cluster bootstrap: control plane, kubelet, CRI | Manual, one-time (predates Kubernetes; not a GitOps target) | Not GitOps-managed (cluster substrate) |
| **Cilium** 1.19.5 | CNI, kube-proxy replacement (eBPF) | ArgoCD Helm source Application (values inline) | Adopted from an imperative `helm` release |
| **local-path-provisioner** v0.0.36 | Default `StorageClass`, dynamic PV provisioning from node-local disk | ArgoCD git source Application, Kustomize **remote base** pointing at the upstream repo | Adopted from an imperative `kubectl apply` |
| **ArgoCD** | GitOps controller | Self-managed: git source Application, Kustomize remote base pointing at the official install manifests | Bootstrap |
| **sealed-secrets** | Encrypts secrets so they can be committed to git | ArgoCD Helm source Application | Native GitOps |
| **VictoriaMetrics k8s stack** (vmsingle, vmagent, vmalert, alertmanager, kube-state-metrics, node-exporter, Grafana) | Metrics collection, alert evaluation, dashboards | ArgoCD Helm source Application (values inline) | Native GitOps |
| **alertmanager-to-github** ([pfnet-research](https://github.com/pfnet-research/alertmanager-to-github)) | Turns firing Alertmanager alerts into GitHub Issues with an open/close/reopen lifecycle | ArgoCD git source Application (custom Deployment/Service manifests) | Native GitOps |
| **Tailscale** | Remote ssh / kubectl access for day-to-day operations | Installed on the host, outside the cluster | N/A |

## Operations

- **Alert-to-issue pipeline**: `VMRule → vmalert → Alertmanager → alertmanager-to-github → GitHub Issue`.
  An open issue means an alert is *currently firing*; it auto-closes when the
  alert resolves and reopens on recurrence. The steady-state goal is **zero
  open issues** — if one is open, it's a real problem.
- **Real-time notification**: the GitHub repository is subscribed in a
  personal Slack workspace via GitHub's official Slack app
  (`/github subscribe <owner>/<repo> issues`). Zero custom code — no
  webhook receiver runs on the cluster.
- **Remote access**: Tailscale is used for day-to-day `ssh`/`kubectl`, so no
  inbound port is exposed to the internet.
- **Change control**: the `main` branch is protected on GitHub (no direct
  pushes), and a local pre-commit hook additionally blocks committing
  directly to `main`/`master` on this machine. All changes land through a
  PR.
- **Manifest convention**: when upstream ships a Helm chart, it's adopted as
  an ArgoCD Helm source Application with values inlined. When upstream ships
  plain YAML and publishes a `kustomization.yaml`, it's adopted via a
  Kustomize **remote base** (see `local-path/` and `argocd/bootstrap/`) so
  only the local customization (as a patch) lives in this repo — not a full
  copy of upstream. Plain YAML without a `kustomization.yaml`, or fully
  custom manifests (e.g. `alertmanager-to-github`), are kept as regular
  Kustomize resources in this repo.
