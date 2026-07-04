# clusters/kind/

Bootstrap materials for the local **kind** cluster.

| File | Purpose |
|---|---|
| `kind-config.yaml` | kind cluster definition (1 control-plane + 2 workers) |
| `cilium-values.yaml` | Cilium Helm values |

The kind config disables the components Cilium replaces, so the cluster comes up
without a default dataplane:

```yaml
networking:
  disableDefaultCNI: true   # kindnetd off — Cilium is the CNI
  kubeProxyMode: none       # no kube-proxy — Cilium replaces it
```

Because kube-proxy is never installed, there is nothing to clean up after
Cilium is in place (contrast with the Raspberry Pi cluster, where `kubeadm`
installs kube-proxy and it must be removed).

## Create the cluster

```bash
kind create cluster --config clusters/kind/kind-config.yaml

# nodes stay NotReady until a CNI is installed
helm repo add cilium https://helm.cilium.io/
helm repo update
helm install cilium cilium/cilium --version 1.16.5 \
  --namespace kube-system \
  -f clusters/kind/cilium-values.yaml
```

`k8sServiceHost` points at the control-plane node's container hostname
(`bons8i-control-plane`), which is how Cilium reaches the API server without
kube-proxy on the Docker network.

The kind values also enable `gatewayAPI`, `hubble`, and `ingressController`.
The Gateway API CRDs must be installed before enabling `gatewayAPI`; after a
`helm upgrade` that toggles these, the Cilium operator / agent / envoy pods need
a `rollout restart` to pick up the change.

## Verify

```bash
kubectl get nodes                                  # Ready
kubectl -n kube-system get pods                    # cilium / cilium-operator Running
```
