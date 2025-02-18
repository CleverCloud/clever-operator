# Clever operator

[![Continuous integration](https://github.com/CleverCloud/clever-operator/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/CleverCloud/clever-operator/actions/workflows/ci.yml)

> A kubernetes operator that exposes clever cloud's resources through custom resource definition

## How it works

This project is based on the [operator pattern](https://kubernetes.io/docs/concepts/extend-kubernetes/operator/) to
provide [Custom Resources](https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/). Those
resources will match Clever-Cloud's add-ons.

## Status

The operator is under development, you can use it, but it may have bugs or unimplemented features. You could see missing
features by checking issues with the label `enhancement`.

## Install

There are multiple ways to install the operator, you can built it from sources and deploy it through your own registry
or use an already built operator hosted on dockerhub or operatorhub.

### From source

You will need some tools on your computer to build and deploy the operator, at least you will `git`, `rust` toolchain and
`docker`. To deploy the operator, you will also need the `kubectl` command and a `kubernetes` cluster.

So, firstly, you will need to retrieve the source. You can clone them directly from
[GitHub](https://github.com/CleverCloud/clever-operator) using the following command.

```
$ git clone https://github.com/CleverCloud/clever-operator.git
```
or
```
$ gh repo clone CleverCloud/clever-operator
```

Then, you will need to go into the new created folder where are located the source code.

```
$ cd clever-operator
```

At this step, you can choose to build the binary and use it directly or build the docker image and push it to your
registry and then deploy it into your kubernetes cluster.

#### Build the binary

To build the binary, you can use the following command:

```
$ make build
```

The operator binary will be located under the folder `target/release/clever-operator`. Then, you can run it.

```
$ target/release/clever-operator
```

#### Build the docker image and deploy it

To build the docker image, you can use the following command:

```
$ DOCKER_IMG=<your-registry>/<your-namespace>/clever-operator:latest make docker-build
```

Then, push it to your registry.

```
$ DOCKER_IMG=<your-registry>/<your-namespace>/clever-operator:latest make docker-push
```

Then, update the kubernetes deployment script located in `deployments/kubernetes/v1.24.0/20-deployment.yaml` to deploy
your docker image in your kubernetes cluster. Finally, apply the deployment script.

```
$ make deploy-kubernetes
```
or
```
$ kubectl apply -f deployments/kubernetes/v1.30.0
```

#### From the helm chart

You can also use the available Helm chart. Configure the values.yaml file in `deployments/kubernetes/helm` with your own values, then run:

```console
$ helm install clever-operator -n clever-operator --create-namespace -f values.yaml .
```

### From dockerhub

The docker image will be provided by the dockerhub account of Clever-Cloud and you only need to apply the deployment
script.

```
$ make deploy-kubernetes
```
or
```
$ kubectl apply -f https://raw.githubusercontent.com/CleverCloud/clever-operator/main/deployments/kubernetes/v1.30.0/10-custom-resource-definition.yaml
$ kubectl apply -f https://raw.githubusercontent.com/CleverCloud/clever-operator/main/deployments/kubernetes/v1.30.0/20-deployment.yaml
```

## Configuration

### Global

To work properly, the operator needs to be configured with at least credentials to connect the Clever Cloud's API. 
Those configurations could be provided through a `ConfigMap`, a `Secret` or by the environment.

An example of deployment using a `Secret` is located at [deployments/kubernetes/v1.30.0/20-deployment.yaml](./deployments/kubernetes/v1.30.0/20-deployment.yaml).
An example of deployment using a `ConfigMap` is located at [deployments/helm/](./deployments/kubernetes/helm/templates/configmap.yaml).

Environment variables are:

| Name                                  | Kind            | Default                        | Required | Description                                                       |
| ------------------------------------- | --------------- | ------------------------------ |----------|-------------------------------------------------------------------|
| `CLEVER_OPERATOR_OPERATOR_LISTEN`     | `SocketAddress` | `0.0.0.0:7080`                 | yes      |                                                                   |
| `CLEVER_OPERATOR_API_ENDPOINT`        | `Url`           | `https://api.clever-cloud.com` | yes      |                                                                   |
| `CLEVER_OPERATOR_API_SECRET`          | `String`        | none                           | false    |                                                                   |
| `CLEVER_OPERATOR_API_TOKEN`           | `String`        | none                           | yes      | if used alone, we assume that we are using oauthless auth backend |
| `CLEVER_OPERATOR_API_CONSUMER_KEY`    | `String`        | none                           | false    |                                                                   |
| `CLEVER_OPERATOR_API_CONSUMER_SECRET` | `String`        | none                           | false    |                                                                   |

By default, if the `--config` flag is not provided to the binary, the operator will look at the following paths to
retrieve its configuration:

- `/usr/share/clever-operator/config.{toml,yaml,json}`
- `/etc/clever-operator/config.{toml,yaml,json}`
- `$HOME/.config/clever-operator/config.{toml,yaml,json}`
- `$HOME/.local/share/clever-operator/config.{toml,yaml,json}`
- `config.{toml,yaml,json}`

## License

See the [license](LICENSE).

## Getting in touch

- [@FlorentinDUBOIS](https://twitter.com/FlorentinDUBOIS)
