# Obligations: k3d dev cluster, as it ought to be

Every gap this projection did **not** write a change for, and the reason. Applying every file in the tree beside this one leaves exactly these.

Computed from specification `6f52fb34191df4f822a31f9242e103c7e61976b69bed41b772aba8a72f747689` against snapshot `9ed0e8608fd69c43c3b0405a7a5fd599fad61a6246b42e2cdd4cffd1e29c8e75` of `k3d-dev-cluster`.

## Decisions owed (16)

### `checkout-exists` — `workloads/shop/deployment/checkout-api`

* **observed:** no deployment named checkout-api in shop
* **why not a patch:** the expectation names the object and not what it would run
* **decision:** write a deployment manifest for shop/checkout-api: this specification says it should exist and says nothing about what it would run

### `shop-probes` — `workloads/shop/deployment/flaky-agent`

* **observed:** container agent declares no liveness probe
* **why not a patch:** no value is stated for probes.liveness
* **decision:** choose what makes container `agent` healthy — a path and a port, or a port to connect to — and state it as a `remedy: {probes: …}` on expectation `shop-probes`; probes.liveness unstated

### `shop-probes` — `workloads/shop/deployment/queue-redis`

* **observed:** container redis declares no liveness probe
* **why not a patch:** no value is stated for probes.liveness
* **decision:** choose what makes container `redis` healthy — a path and a port, or a port to connect to — and state it as a `remedy: {probes: …}` on expectation `shop-probes`; probes.liveness unstated

### `shop-probes` — `workloads/shop/deployment/storefront-server`

* **observed:** container storefront-server declares no liveness probe
* **why not a patch:** no value is stated for probes.liveness
* **decision:** choose what makes container `storefront-server` healthy — a path and a port, or a port to connect to — and state it as a `remedy: {probes: …}` on expectation `shop-probes`; probes.liveness unstated

### `shop-probes` — `workloads/shop/statefulset/switchboard`

* **observed:** container switchboard declares no liveness probe
* **why not a patch:** no value is stated for probes.liveness
* **decision:** choose what makes container `switchboard` healthy — a path and a port, or a port to connect to — and state it as a `remedy: {probes: …}` on expectation `shop-probes`; probes.liveness unstated

### `flaky-agent-registry` — `workloads/shop/deployment/flaky-agent`

* **observed:** container agent pulls registry.local/flaky-agent from registry.local, wanted one of [registry.example.com]
* **why not a patch:** choosing an image is an engineering decision, not a substitution
* **decision:** choose the build of `registry.local/flaky-agent` that container `agent` should run from one of [registry.example.com]; a rewritten registry prefix is a different image, not the same one somewhere else

### `shop-tags` — `workloads/shop/deployment/flaky-agent`

* **observed:** container agent pulls registry.local/flaky-agent, untagged — which is `latest`
* **why not a patch:** choosing an image is an engineering decision, not a substitution
* **decision:** choose the version of `registry.local/flaky-agent` that container `agent` should run and write it as a tag or a `sha256:` digest; nothing in the snapshot says which build `registry.local/flaky-agent` is today

### `shop-tags` — `workloads/shop/statefulset/switchboard`

* **observed:** container switchboard pulls registry.example.com/services/switchboard-8:latest, tagged `latest`
* **why not a patch:** choosing an image is an engineering decision, not a substitution
* **decision:** choose the version of `registry.example.com/services/switchboard-8:latest` that container `switchboard` should run and write it as a tag or a `sha256:` digest; nothing in the snapshot says which build `registry.example.com/services/switchboard-8:latest` is today

### `shop-digests` — `workloads/shop/deployment/flaky-agent`

* **observed:** container agent pulls registry.local/flaky-agent, not pinned by digest
* **why not a patch:** choosing an image is an engineering decision, not a substitution
* **decision:** choose the version of `registry.local/flaky-agent` that container `agent` should run and write it as a tag or a `sha256:` digest; nothing in the snapshot says which build `registry.local/flaky-agent` is today

### `shop-digests` — `workloads/shop/deployment/queue-redis`

* **observed:** container redis pulls redis:7-alpine, not pinned by digest
* **why not a patch:** choosing an image is an engineering decision, not a substitution
* **decision:** choose the version of `redis:7-alpine` that container `redis` should run and write it as a tag or a `sha256:` digest; nothing in the snapshot says which build `redis:7-alpine` is today

### `shop-digests` — `workloads/shop/deployment/storefront-server`

* **observed:** container storefront-server pulls localhost:31721/apps/storefront-server:AtqQlTV, not pinned by digest
* **why not a patch:** choosing an image is an engineering decision, not a substitution
* **decision:** choose the version of `localhost:31721/apps/storefront-server:AtqQlTV` that container `storefront-server` should run and write it as a tag or a `sha256:` digest; nothing in the snapshot says which build `localhost:31721/apps/storefront-server:AtqQlTV` is today

### `shop-digests` — `workloads/shop/statefulset/switchboard`

* **observed:** container switchboard pulls registry.example.com/services/switchboard-8:latest, not pinned by digest
* **why not a patch:** choosing an image is an engineering decision, not a substitution
* **decision:** choose the version of `registry.example.com/services/switchboard-8:latest` that container `switchboard` should run and write it as a tag or a `sha256:` digest; nothing in the snapshot says which build `registry.example.com/services/switchboard-8:latest` is today

### `shop-selectors` — `services/shop/lost-lookup`

* **observed:** selector app=retired matches no observed pod
* **why not a patch:** nobody observed what this points at
* **decision:** decide which is true: the workload that should carry app=retired was never deployed, or this selector names labels nothing carries. Deploying one and rewriting the other are different changes and the snapshot does not say which was meant

### `shop-config-refs` — `workloads/shop/deployment/flaky-agent`

* **observed:** containers[agent].env[AGENT_TOKEN] requires secret agent-credentials, which was not observed
* **why not a patch:** nobody observed what this points at
* **decision:** create the secret agent-credentials this cluster expects at containers[agent].env[AGENT_TOKEN], or change the reference. Its contents are not in the snapshot — a secret is only ever there as a digest — so nothing here can write one

### `shop-only` — `workloads/kube-system/daemonset/svclb-traefik-2290261f`

* **observed:** sits in kube-system, wanted one of [shop]
* **why not a patch:** `metadata.namespace` cannot be patched on a live object
* **decision:** recreate this workload in one of [shop]: a live object's namespace cannot be patched, so moving it out of `kube-system` is a delete and a create, which is not a patch and not this projection's to write

### `shop-only` — `workloads/kube-system/deployment/coredns`

* **observed:** sits in kube-system, wanted one of [shop]
* **why not a patch:** `metadata.namespace` cannot be patched on a live object
* **decision:** recreate this workload in one of [shop]: a live object's namespace cannot be patched, so moving it out of `kube-system` is a delete and a create, which is not a patch and not this projection's to write

## Refused (0)

None. Every gap this tree does not close is one a person can close.

