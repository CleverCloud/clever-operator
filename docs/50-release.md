# Release the clever-operator

> This document will go through steps to release a new version of the operator

## Update operator-lifecycle-manager manifests

> This part explain how to create manifests for the clever-operator on [OperatorHub](https://operatorhub.io/operator/clever-operator).

### Create operator-lifecycle-manager manifests for the new release

Firstly, you will need to duplicate manifests located in `deployments/operator-lifecycle-manager/<latest-release>` to the new release. 
Once, this is done, you will got something like below.

```
deployments/operator-lifecycle-manager/<new-release>
├── bundle.Dockerfile
├── manifests
│  ├── clever-operator-mongodb.crd.yaml
│  ├── clever-operator-mysql.crd.yaml
│  ├── clever-operator-postgresql.crd.yaml
│  ├── clever-operator-pulsar.crd.yaml
│  ├── clever-operator-redis.crd.yaml
│  └── clever-operator.clusterserviceversion.yaml
├── metadata
│  └── annotations.yaml
└── tests
   └── scorecard
      └── config.yaml
```

You have to edit the file `clever-operator.clusterserviceversion.yaml` to update the `<latest-release>` in to the new version `<new-release>` 
and as well update docker image to the latest commit of the `main` branch.

### Update the continuous integration to build the new release

You will have to edit the `.github/ci.yaml` file and replace as well the `<latest-release>` by the `<new-release>` in task 
`docker-build-and-push-openshift-manifest`.

### Update the Makefile

You will have to update the `Makefile` to bump the variable `OLM_VERSION` to the `<new-release>`.

## Update Kubernetes manifests

> This part explain how to update manifests for the deployment of the operator in Kubernetes

You have to update the Kubernetes' Deployment with the latest docker image of the branch `main`. That's all!

## Update version of clever-operator

You will have to update the version of the project in the following file `Cargo.toml` which correspond to the Rust manifest.

## Create a release commit

Once, all steps above have been achieved, you can create a release commit with the following command on the `main` branch:

```shell
$ git add . && git commit -s -m 'Release v<new-release>' && git push
```

Then, create a git tag using the command below:

```shell
$ git tag -a 'v<new-release>' && git push --tags
```

Now, you are able once, the continuous integration is ok, to create GitHub release using the tag above.

## Publish new release on OperatorHub

You are now able to publish a new release on the OperatorHub, to do that create a pull request 
on the [k8s-operatorhub/community-operators](https://github.com/k8s-operatorhub/community-operators/) 
with the freshly created manifests in `deployments/operator-lifecycle-manager/<new-release>`.
