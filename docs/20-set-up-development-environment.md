# Set up development environment

> This document provide information about set up a development environment.

## Set up development environment using minikube

### Pre-requisites

- A [rust development environment](https://rustup.rs/)
- A [minikube installation](https://minikube.sigs.k8s.io/docs/start/)

### Set up minikube

Below, the command to start minikube with a self-signed certificate valid for
a domain which allow the operator to connect to it without dns resolve error.

```
$ minikube start --cpus=4 --disk-size=60g --memory=16g --apiserver-names=minikube.localdomain --delete-on-failure=true
```

Then, you need to change the minikube address in the kubeconfig file and set the
 link of the custom domain to the minikube's ip address on the `/etc/hosts` file

```
$ cat $HOME/.kube/config
apiVersion: v1
clusters:
- cluster:
    certificate-authority: <home>/.minikube/ca.crt
    extensions:
    - extension:
        last-update: Thu, 09 Sep 2021 11:16:24 CEST
        provider: minikube.sigs.k8s.io
        version: v1.22.0
      name: cluster_info
    server: https://192.168.39.127:8443
  name: minikube
contexts:
- context:
    cluster: minikube
    extensions:
    - extension:
        last-update: Thu, 09 Sep 2021 11:16:24 CEST
        provider: minikube.sigs.k8s.io
        version: v1.22.0
      name: context_info
    namespace: default
    user: minikube
  name: minikube
current-context: minikube
kind: Config
preferences: {}
users:
- name: minikube
  user:
    client-certificate: <home>/.minikube/profiles/minikube/client.crt
    client-key: <home>/.minikube/profiles/minikube/client.key
```

Put the ip address in the `/etc/hosts` file:

```
$ echo 192.168.39.127 minikube.localdomain minikube >> /etc/hosts
```

Edit the kubeconfig file

```
$ $EDITOR $HOME/.kube/config
apiVersion: v1
clusters:
- cluster:
    certificate-authority: <home>/.minikube/ca.crt
    extensions:
    - extension:
        last-update: Thu, 09 Sep 2021 11:16:24 CEST
        provider: minikube.sigs.k8s.io
        version: v1.22.0
      name: cluster_info
    server: https://minikube.localdomain:8443
  name: minikube
contexts:
- context:
    cluster: minikube
    extensions:
    - extension:
        last-update: Thu, 09 Sep 2021 11:16:24 CEST
        provider: minikube.sigs.k8s.io
        version: v1.22.0
      name: context_info
    namespace: default
    user: minikube
  name: minikube
current-context: minikube
kind: Config
preferences: {}
users:
- name: minikube
  user:
    client-certificate: <home>/.minikube/profiles/minikube/client.crt
    client-key: <home>/.minikube/profiles/minikube/client.key

```

The operator is now able to interact with minkube

### Generate and register kubernetes custom resource

First, we will generate custom resource definition for resources managed by the
operator.

```
$ make crd
```

Then, apply the custom resource definition to minikube

```
$ make deploy-kubernetes-crd
```

### Start the operator to start tests and developments

Go to the operator folder and start it:

```
$ cargo run -- -vvvvvvv
```

The operator once compiled should listen to custom resource events.
