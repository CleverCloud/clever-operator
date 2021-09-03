# ------------------------------------------------------------------------------
# Define variables
DIST			?= $(PWD)/target/release

NAME			?= clever-operator
VERSION			?= $(shell git describe --candidates 1 --tags HEAD 2>/dev/null || echo HEAD)

DOCKER			?= $(shell which docker)
DOCKER_OPTS		?= --log-level debug
DOCKER_IMG		?= clevercloud/$(NAME):$(VERSION)

KUBE			?= $(shell which kubectl)
KUBE_SCORE		?= $(shell which kube-score)
KUBE_VERSION	?= v1.21.0
KUBE_DEPLOY 	?= $(PWD)/deployments/kubernetes/$(KUBE_VERSION)

FIND 			?= $(shell which find)

CARGO			?= $(shell which cargo)
CARGO_OPTS		?= --verbose

# ------------------------------------------------------------------------------
# Build operator
.PHONY: build
build: $(DIST)/$(NAME)

$(DIST)/$(NAME): $(shell $(FIND) -type f -name '*.rs')
	$(CARGO) build $(CARGO_OPTS) --release

# ------------------------------------------------------------------------------
# Build docker
.PHONY: docker-build
docker-build: $(shell $(FIND) -type f -name '*.rs') Dockerfile
	$(DOCKER) $(DOCKER_OPTS) build -t $(DOCKER_IMG) $(PWD)

.PHONY: docker-push
docker-push: docker-build
	$(DOCKER) $(DOCKER_OPTS) push $(DOCKER_IMG)

# ------------------------------------------------------------------------------
# Kubernetes deployment
.PHONY: crd
crd: build $(shell $(FIND) -type f -name '*.rs') $(KUBE_DEPLOY)/10-custom-resource-definition.yaml

$(KUBE_DEPLOY)/10-custom-resource-definition.yaml:
	$(DIST)/$(NAME) custom-resource-definition view > $(KUBE_DEPLOY)/10-custom-resource-definition.yaml

validate: $(shell $(FIND) -type f -name '*.yaml')
	$(KUBE_SCORE) score $(shell $(FIND) $(KUBE_DEPLOY) -type f -name '*.yaml')

.PHONY: deploy-kubernetes-crd
deploy-kubernetes-crd: crd validate $(KUBE_DEPLOY)/10-custom-resource-definition.yaml
	$(KUBE) apply -f $(KUBE_DEPLOY)/10-custom-resource-definition.yaml

.PHONY: deploy-kubernetes
deploy-kubernetes: crd validate deploy-kubernete-crd
	$(KUBE) apply -f $(KUBE_DEPLOY)

# ------------------------------------------------------------------------------
# Clean up
.PHONY: clean
clean:
	$(CARGO) clean $(CARGO_OPTS)
	rm $(KUBE_DEPLOY)/10-custom-resource-definition.yaml

.PHONY: clean-kubernetes
clean-kubernetes:
	$(KUBE) delete -f $(KUBE_DEPLOY)
