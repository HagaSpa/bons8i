# bons8i

Kubernetes manifests for a personal home cluster, managing kind / GKE / Raspberry Pi 5 environments with Kustomize.

## Layout

```
.
├── base/         # Environment-agnostic manifests (Kustomize base)
├── overlays/     # Per-environment patches (kind / gke / pi)
└── clusters/     # Cluster bootstrap materials (kind config / Cilium values / Terraform etc.)
```
