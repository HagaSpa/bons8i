# base/

Environment-agnostic manifests (Kustomize base). Shared definitions used by all environments.

Environment-specific differences (StorageClass name, replicas, resource requests, etc.) are layered on top via Kustomize patches in `overlays/<env>/`.
