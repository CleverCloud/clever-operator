# Clever operator

> A kubernetes operator that expose clever cloud's resources through custom resource definition

## How it works

This project is based on the [operator pattern](https://kubernetes.io/docs/concepts/extend-kubernetes/operator/) to
provide [Custom Resources](https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/). Those
resources will match Clever-Cloud's add-ons.

## Status

The operator is under development, you can use it, but it may have bugs or unimplemented features.

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

Then, update the kubernetes deployment script to deploy your docker image in your kubernetes cluster. Finally, apply
the deployment script.

```
$ make deploy-kubernetes
```

### From dockerhub

The docker image will be provided by the dockerhub account of Clever-Cloud and you only need to apply the deployment
script.

```
$ make deploy-kubernetes
```

## License

See the [license](LICENSE).

## Getting in touch

- [@FlorentinDUBOIS](https://twitter.com/FlorentinDUBOIS)
