# version-rs
[![ci](https://github.com/kube-rs/version-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/kube-rs/version-rs/actions/workflows/ci.yml)
[![docker image](https://img.shields.io/docker/pulls/clux/version.svg)](
https://hub.docker.com/r/clux/version/tags/)

An example kube deployment reflector and [axum](https://github.com/tokio-rs/axum) web server in ~100 lines of rust. It exposes a simple version api for deployments on `/versions`.

## Usage
Clone the repo and either run locally or deploy into a cluster:

### Locally
Run against your current kubernetes context:

```sh
cargo run
```

### In-Cluster
Apply [deployment.yaml](./deployment.yaml), then `kubectl port-forward service/version 8000:8000`

### Api
Once running, the app will monitor the namespace of your context, and give you simplified version info on its web interface:

```sh
$ curl 0.0.0.0:8000/versions
[{"container":"clux/controller","name":"foo-controller","version":"latest"},{"container":"alpine","name":"debugger","version":"3.13"}]

$ curl 0.0.0.0:8000/versions/default/foo-controller
{"container":"clux/controller","name":"foo-controller","version":"latest"}
```

## Developing
- Locally against a cluster: `cargo run`
- In-cluster: edit and `tilt up` [*](https://tilt.dev/)

To build the image directly, run:

```sh
DOCKER_BUILDKIT=1 docker build -t clux/version .
```
