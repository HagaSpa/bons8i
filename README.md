# bons8i

GitOps repository for a single-node **Raspberry Pi 5** Kubernetes cluster built
with `kubeadm`. It holds the cluster's platform manifests, the Argo CD
`Application` definitions that deploy them, and the source code of the public
status page served at [bons8i.hagaspa.com](https://bons8i.hagaspa.com).

## Layout

```
.
├── clusters/pi/   # Cluster manifests + Argo CD Applications (App of Apps)
├── infra/         # Terraform for the AWS-side pieces (external probe Lambda, IAM)
├── web/           # Application source (status page: Rust BFF + React)
├── scripts/       # Operational tooling (DR drill, etcd backup, diagram rendering)
└── docs/          # Postmortems & runbooks
```

See [`clusters/pi/README.md`](clusters/pi/README.md) for the component list and
architecture diagrams (internet-facing traffic / cluster internals /
backup & alerting).

## Platform

- **GitOps** — [Argo CD](https://argo-cd.readthedocs.io/) (self-managed,
  App of Apps). Every workload is declared as an `Application` under
  `clusters/pi/argocd/apps/` and synced from this repository.
- **CNI** — [Cilium](https://cilium.io/) with kube-proxy replacement enabled:
  Service routing is handled by the eBPF datapath instead of
  `iptables`/`kube-proxy`. There is no Ingress/Gateway controller — public
  traffic enters through the Cloudflare Tunnel connector and goes straight to
  in-cluster Services.
- **Monitoring** — VictoriaMetrics k8s stack (vmsingle / vmagent / vmalert /
  Alertmanager / Grafana), 13-month retention. Alerts are routed to GitHub
  Issues via alertmanager-to-github, so an open issue means an ongoing
  incident. An AWS Lambda probe (EventBridge, every 10 min; Terraform under
  `infra/aws/`) additionally checks the status page from outside the cluster
  and files outage issues.
- **Backup** — three layers for metrics: the live TSDB on NVMe, a daily
  `vmbackup` mirror to S3, and a monthly append-only `vm-archive` export that
  outlives retention — plus manual etcd snapshots via
  [`scripts/etcd-backup.sh`](scripts/etcd-backup.sh). The backup jobs are
  themselves monitored by a VMRule.
- **Secrets** — External Secrets Operator + AWS SSM Parameter Store. No secret
  material (not even ciphertext) lives in git: `ExternalSecret` resources only
  reference parameters, and a read-only scoped IAM key is the single
  out-of-band credential. Disaster recovery is rehearsed with
  [`scripts/dr-drill.sh`](scripts/dr-drill.sh) against a throwaway kind
  cluster; see the [DR runbook](docs/runbook/dr-secrets.md) (Japanese).
- **Edge** — Cloudflare Tunnel (outbound-only connector, no open inbound
  ports) + Cloudflare Access for authentication in front of private apps
  (Grafana); the status page is public. Day-to-day `ssh`/`kubectl` go over
  Tailscale.
- **Storage** — local-path-provisioner, consumed via the upstream kustomization
  as a remote base with local patches.

## Continuous delivery (status page)

Merging a change under `web/status-page/` deploys it without manual steps:

1. GitHub Actions builds a `linux/arm64` image on a native arm64 runner and
   pushes it to `ghcr.io` tagged with the git SHA.
2. The same workflow updates the image tag in
   `clusters/pi/status-page/kustomization.yaml` via `kustomize edit set image`
   and opens a `deploy`-labeled PR that auto-merges.
3. Argo CD auto-syncs the application (`prune` + `selfHeal`); rollback is a
   `git revert` of the deploy commit.

Platform applications are synced manually after reviewing the diff.
