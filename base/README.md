# base/

Environment-agnostic manifests (Kustomize base). Shared definitions used by all
environments.

| Directory | Contents |
|---|---|
| `postgres/` | PostgreSQL `StatefulSet` + headless `Service` + `Secret` (data on a `PersistentVolumeClaim`) |
| `web/` | `adminer` `Deployment` + `Service` (web client, points at `postgres-headless`) |
| `gateway/` | `Gateway` + `HTTPRoute` exposing adminer via the Cilium `GatewayClass`; an `Ingress` is kept for reference but excluded from the Kustomize build |

Environment-specific differences (StorageClass name, replicas, resource
requests, etc.) are layered on top via Kustomize patches in `overlays/<env>/`.
