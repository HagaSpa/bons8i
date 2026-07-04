# bons8i

Kubernetes manifests and cluster bootstrap materials for a personal home lab,
managed with Kustomize across three environments: **kind** (local), **GKE**, and
a single-node **Raspberry Pi 5** cluster built with `kubeadm`.

## Stack

A small two-tier workload used to exercise Workloads / Networking / Storage:

- **PostgreSQL** — `StatefulSet` + headless `Service` + `Secret`, with a
  `PersistentVolumeClaim` for data (`base/postgres/`).
- **adminer** — a `Deployment` + `Service` acting as a web client for the
  database (`base/web/`).
- **Gateway API** — a `Gateway` + `HTTPRoute` exposing adminer through the
  Cilium `GatewayClass` (`base/gateway/`). An equivalent `Ingress` is kept for
  reference but excluded from the Kustomize build.

## Networking

All clusters run **[Cilium](https://cilium.io/)** as the CNI with
**kube-proxy replacement** enabled (`kubeProxyReplacement: true`), so Service
routing is handled by Cilium's eBPF datapath instead of `iptables`/`kube-proxy`.
Cilium also provides the Gateway API and Ingress implementations, so no separate
L7 controller is needed.

## Layout

```
.
├── base/         # Environment-agnostic manifests (Kustomize base)
├── overlays/     # Per-environment composition / patches (kind / gke / pi)
└── clusters/     # Cluster bootstrap materials (kind config, Cilium values, Terraform)
```

- `base/` holds shared definitions. See `base/README.md`.
- `overlays/<env>/` composes the base resources per environment and is where
  environment-specific patches (StorageClass name, replicas, resource requests)
  are layered on top.
- `clusters/<env>/` holds how each cluster is *created* (not the app manifests).
  Each has its own README with the bootstrap steps:
  - [`clusters/kind/`](clusters/kind/README.md) — local kind cluster
  - [`clusters/pi/`](clusters/pi/README.md) — single-node Raspberry Pi cluster
  - [`clusters/gke/`](clusters/gke/README.md) — GKE (Terraform)

## Deploy the workload

Render and apply the workload for an environment via its overlay:

```bash
# preview
kubectl kustomize overlays/kind

# apply
kubectl apply -k overlays/kind
```

Cluster creation and CNI installation are separate from the workload apply —
see the per-cluster READMEs under `clusters/`.
