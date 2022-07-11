# Set up operator-sdk development environment

> This documentation has to goal to help you to set up an operator-sdk compliant environment

## Pre-requisites

Before going further, you will need to follow instructions about
[set up a development environment](20-set-up-development-environment.md).

- The [curl](https://curl.se/) command line

## Install command line tools

To ensure that manifests and the operator is compliant with OpenShift eco-system, you will need some
command line tools which are the `operator-sdk`, `ocp-olm-catalog-validator`
and `k8s-community-bundle-validator`

You can install those command line tools using the following command.

```
$ make install-cli-tools
```

It will downland and copy the command line tools in the `$HOME/.local/bin` directory. Then, you will
have to export this folder in your `$PATH` environment variable if not already done using this
command.

```
$ export PATH="$HOME/.local/bin:$PATH"
```

## Check manifests OpenShift compliance

To test compliancy of manifests, you could use the following command.

```
$ make validate
```

## Install the operator-sdk components in kubernetes

You could install the operator-sdk components into kubernetes using the following command, it will
unlock you the deployment of the bundle (a docker image containing the operator lifecycle manager
manifests and some metadata).

```
$ operator-sdk olm install --timeout 10m --verbose
```

To check the status, you could run:

```
$ operator-sdk olm status
```

## Deploy a bundle and test deployments

To deploy a bundle (a docker image containing the operator lifecycle manager manifests and some
metadata) on kubernetes. You will first need to build a docker image of the current revision. By
default the ci will do that for you and then run the command below.

```
$ operator-sdk run bundle --verbose --timeout 10m docker.io/clevercloud/clever-operator-manifest:<githash>
```

During the deployment, you should have some terminal instances running with the following commands
to help you finding a possible error, if one or many occurs.

- `kubectl get events -A -w`
- `watch -n1 'kubectl get pods -A'`
- `journalctl -f -o short-monotonic`

Those will help you to see what happening on your system during the deployment.
