# clusters/gke/

Bootstrap materials for a **GKE** cluster, provisioned with Terraform.

> Not yet implemented — this directory is a placeholder.

Planned scope:

- Terraform definitions for a small GKE Standard cluster, created on demand and
  torn down with `terraform destroy` (not kept running).
- Used to compare the managed Gateway API implementation (`gke-l7` GatewayClass)
  against Cilium's, and to practice migrating an existing NGINX Ingress
  Controller setup to the Gateway API.
