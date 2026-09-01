# Clever kubernetes operator

[![Continuous integration](https://github.com/CleverCloud/clever-kubernetes-operator/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/CleverCloud/clever-kubernetes-operator/actions/workflows/ci.yml)

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
[GitHub](https://github.com/CleverCloud/clever-kubernetes-operator) using the following command.

```
$ git clone https://github.com/CleverCloud/clever-kubernetes-operator.git
```
or
```
$ gh repo clone CleverCloud/clever-kubernetes-operator
```

Then, you will need to go into the new created folder where are located the source code.

```
$ cd clever-kubernetes-operator
```

At this step, you can choose to build the binary and use it directly or build the docker image and push it to your
registry and then deploy it into your kubernetes cluster.

#### Build the binary

To build the binary, you can use the following command:

```
$ make build
```

The operator binary will be located under the folder `target/release/clever-kubernetes-operator`. Then, you can run it.

```
$ target/release/clever-kubernetes-operator
```

#### Build the docker image and deploy it

To build the docker image, you can use the following command:

```
$ DOCKER_IMG=<your-registry>/<your-namespace>/clever-kubernetes-operator:latest make docker-build
```

Then, push it to your registry.

```
$ DOCKER_IMG=<your-registry>/<your-namespace>/clever-kubernetes-operator:latest make docker-push
```

Then, update the kubernetes deployment script located in `deployments/kubernetes/v1.36.0/20-deployment.yaml` to deploy
your docker image in your kubernetes cluster. Finally, apply the deployment script.

```
$ make deploy-kubernetes
```
or
```
$ kubectl apply -f deployments/kubernetes/v1.36.0
```

#### From the helm chart

You can also use the available Helm chart. Configure the values.yaml file in `deployments/kubernetes/helm` with your own values, then run:

```console
$ helm install clever-kubernetes-operator -n clever-kubernetes-operator --create-namespace -f values.yaml .
```

### From dockerhub

The docker image will be provided by the dockerhub account of Clever-Cloud and you only need to apply the deployment
script.

```
$ make deploy-kubernetes
```
or
```
$ kubectl apply -f https://raw.githubusercontent.com/CleverCloud/clever-kubernetes-operator/main/deployments/kubernetes/v1.36.0/10-custom-resource-definition.yaml
$ kubectl apply -f https://raw.githubusercontent.com/CleverCloud/clever-kubernetes-operator/main/deployments/kubernetes/v1.36.0/20-deployment.yaml
```

## Credentials

To connect to the Clever Cloud API, the operator supports three authentication methods:

### 1. Full OAuth1 Authentication (Four Parameters)

This option uses the complete OAuth1 authentication flow with all four required credentials.

```toml
[api]
token = "your-oauth-token"
secret = "your-oauth-secret"
consumer-key = "your-consumer-key"
consumer-secret = "your-consumer-secret"
```

**When to use:**
- When you have your own OAuth consumer application registered with Clever Cloud
- When you need custom permissions or scopes
- For production environments where you want full control over authentication

### 2. Simplified OAuth1 Authentication (Token and Secret Only)

This option uses OAuth1 authentication but automatically applies the public Clever Tools consumer credentials when you only provide token and secret.

```toml
[api]
token = "your-oauth-token"
secret = "your-oauth-secret"
```

**When to use:**
- For most standard use cases
- When you have personal tokens but don't want to manage consumer credentials
- For development or testing environments

When only token and secret are provided, the SDK automatically uses the same public consumer credentials as the Clever Cloud CLI tools.

### 3. Bearer Token Authentication

This is the simplest authentication method, using a single bearer token with the "oauthless auth backend."

```toml
[api]
token = "your-bearer-token"
```

**When to use:**
- When working with the oauthless auth backend
- For simple integrations or scripts
- When you have a personal access token that doesn't require OAuth flow

For most users, Option 2 (token and secret only) provides the best balance of security and convenience.

## Configuration

### Global

To work properly, the operator needs to be configured with at least credentials to connect the Clever Cloud's API.
Those configurations could be provided through a `ConfigMap`, a `Secret` or by the environment.

An example of deployment using a `Secret` is located at [deployments/kubernetes/v1.36.0/20-deployment.yaml](./deployments/kubernetes/v1.36.0/20-deployment.yaml).
An example of deployment using a `ConfigMap` is located at [deployments/helm/](./deployments/kubernetes/helm/templates/configmap.yaml).

Environment variables are:

| Name                                  | Kind            | Default                        | Required | Description                                                       |
| ------------------------------------- | --------------- | ------------------------------ |----------|-------------------------------------------------------------------|
| `CLEVER_OPERATOR_OPERATOR_LISTEN`     | `SocketAddress` | `0.0.0.0:8000`                 | yes      |                                                                   |
| `CLEVER_OPERATOR_API_SECRET`          | `String`        | none                           | false    |                                                                   |
| `CLEVER_OPERATOR_API_TOKEN`           | `String`        | none                           | yes      | if used alone, we assume that we are using oauthless auth backend |
| `CLEVER_OPERATOR_API_CONSUMER_KEY`    | `String`        | none                           | false    |                                                                   |
| `CLEVER_OPERATOR_API_CONSUMER_SECRET` | `String`        | none                           | false    |                                                                   |
| `CLEVER_OPERATOR_API_ENDPOINT`        | `String`        | none                           | false    | base url of the Clever Cloud API, see below                       |

### API endpoint

By default, the operator talks to the public Clever Cloud API, `https://api.clever-cloud.com` — or
`https://api-bridge.clever-cloud.com` when a bare bearer token is used, as this is the endpoint the
oauthless auth backend lives on.

To target another installation, set the `CLEVER_OPERATOR_API_ENDPOINT` environment variable or the
`endpoint` key of the `api` section:

```toml
[api]
endpoint = "https://api.clever-cloud.example.com"
token = "your-oauth-token"
secret = "your-oauth-secret"
```

The value is normalised: a trailing slash is trimmed, and anything the operator could not append a
path to — another scheme than `http`/`https`, a query string, a fragment, embedded credentials — is
rejected when the configuration is loaded. Running with `--config <path> --check` reports such a
value; when the configuration is discovered from the default search paths below, a rejected value
makes the operator fall back to the clever-tools configuration instead of reporting the problem.

As for the credentials, the environment variable only provides a default: a value set in the
configuration file takes precedence over it. An empty variable counts as not set.

A per-namespace `clever-kubernetes-operator` `Secret` overrides the credentials of a namespace, not
the installation they belong to: it inherits this endpoint unless it declares an `endpoint` of its
own.

By default, if the `--config` flag is not provided to the binary, the operator will look at the following paths to
retrieve its configuration:

- `/usr/share/clever-kubernetes-operator/config.{toml,yaml,json}`
- `/etc/clever-kubernetes-operator/config.{toml,yaml,json}`
- `$HOME/.config/clever-kubernetes-operator/config.{toml,yaml,json}`
- `$HOME/.local/share/clever-kubernetes-operator/config.{toml,yaml,json}`
- `config.{toml,yaml,json}`

## License

See the [license](LICENSE).

## Getting in touch

- [@FlorentinDUBOIS](https://twitter.com/FlorentinDUBOIS)
