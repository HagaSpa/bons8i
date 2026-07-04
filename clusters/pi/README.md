# clusters/pi/

Bootstrap materials for the single-node **Raspberry Pi 5** cluster
(`kubeadm`, Ubuntu Server 24.04 arm64).

| File | Purpose |
|---|---|
| `cilium-values.yaml` | Cilium Helm values (CNI-first minimal set) |

The cluster is created with `kubeadm init --pod-network-cidr=10.244.0.0/16`.
Unlike kind, `kubeadm` installs kube-proxy, so it must be removed after Cilium
takes over Service routing.

The values are intentionally minimal for the first install; `gatewayAPI`,
`hubble`, and `ingressController` are enabled later via `helm upgrade` (the
Gateway API CRDs must be installed first). `ipam.mode: kubernetes` makes Cilium
read the pod CIDR that `kubeadm` assigned to the node.

> Replace `<node-ip>` with the API server address used at `kubeadm init`
> (`--apiserver-advertise-address`), which also appears as `k8sServiceHost` in
> `cilium-values.yaml`.

## Install Cilium

Cilium is pinned to **1.19.5** (latest stable; the 1.16.x line is EOL).

```bash
helm repo add cilium https://helm.cilium.io/
helm repo update
helm install cilium cilium/cilium --version 1.19.5 \
  --namespace kube-system \
  -f clusters/pi/cilium-values.yaml
```

After the agent rolls out, the node moves to `Ready` and
`/etc/cni/net.d/05-cilium.conflist` appears.

## Remove kube-proxy

Delete the kube-proxy addon left by `kubeadm init`, then clear its `iptables`
rules by rebooting the node (simplest on a single node):

```bash
kubectl -n kube-system delete ds kube-proxy
kubectl -n kube-system delete cm kube-proxy
sudo reboot
```

## Verify

```bash
kubectl get nodes                                  # Ready
sudo iptables-save | grep -c KUBE-SVC              # 0 (kube-proxy Service chains gone)
kubectl -n kube-system exec ds/cilium -- cilium-dbg status | grep -i kubeproxy
#   KubeProxyReplacement: True [eth0 <node-ip> ... (Direct Routing)]

# allow workloads on the single (control-plane) node
kubectl taint nodes <node-name> node-role.kubernetes.io/control-plane:NoSchedule-

# end-to-end: a pod resolving a Service ClusterIP proves routing works without kube-proxy
kubectl run test --image=nginx --restart=Never
kubectl run dns --rm -it --image=busybox:1.36 --restart=Never -- nslookup kubernetes
kubectl delete pod test
```

## Notes

- The single node runs the control plane and workloads (the control-plane taint
  is removed). A second node can join later without downtime.
- The Cilium operator defaults to 2 replicas with anti-affinity, so one replica
  stays `Pending` on a single-node cluster. Set `operator.replicas: 1` via
  `helm upgrade` to clear it (optional).
