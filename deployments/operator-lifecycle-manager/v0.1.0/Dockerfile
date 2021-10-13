# See https://github.com/operator-framework/operator-registry/blob/master/docs/design/operator-bundle.md#Bundle-Dockerfile

FROM scratch

LABEL operators.operatorframework.io.bundle.mediatype.v1: "registry+v1"
LABEL operators.operatorframework.io.bundle.manifests.v1: "manifests/"
LABEL operators.operatorframework.io.bundle.metadata.v1: "metadata/"
LABEL operators.operatorframework.io.bundle.package.v1: "clever-operator"
LABEL operators.operatorframework.io.bundle.channels.v1: "alpha"
LABEL operators.operatorframework.io.bundle.channel.default.v1: "alpha"

ADD manifests /manifests/
ADD metadata /metadata/
