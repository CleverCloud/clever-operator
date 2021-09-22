# ------------------------------------------------------------------------------
# Define variables
DIST				?= $(PWD)/target/release

NAME				?= clever-operator
VERSION				?= $(shell git describe --candidates 1 --tags HEAD 2>/dev/null || echo HEAD)

DOCKER				?= $(shell which docker)
DOCKER_OPTS			?= --log-level debug
DOCKER_IMG			?= clevercloud/$(NAME):$(VERSION)

KUBE				?= $(shell which kubectl)
KUBE_SCORE			?= $(shell which kube-score)
KUBE_VERSION		?= v1.21.0

OLM_SDK		    	?= $(shell which operator-sdk)
OLM_VERSION			?= v0.1.0

DEPLOY_KUBE			?= deployments/kubernetes/$(KUBE_VERSION)
DEPLOY_OLM			?= deployments/operator-lifecycle-manager/$(OLM_VERSION)

FIND 				?= $(shell which find)

CARGO				?= $(shell which cargo)
CARGO_OPTS			?= --verbose

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
crd: build $(shell $(FIND) -type f -name '*.rs') $(DEPLOY_KUBE)/10-custom-resource-definition.yaml $(DEPLOY_OLM)/clever-operator-postgresql.crd.yaml

$(DEPLOY_KUBE)/10-custom-resource-definition.yaml:
	$(DIST)/$(NAME) custom-resource-definition view > $(DEPLOY_KUBE)/10-custom-resource-definition.yaml

$(DEPLOY_OLM)/clever-operator-postgresql.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view postgresql > $(DEPLOY_OLM)/clever-operator-postgresql.crd.yaml

.PHONY: validate
validate: $(shell $(FIND) -type f -name '*.yaml')
	$(KUBE_SCORE) score $(shell $(FIND) $(DEPLOY_KUBE) -type f -name '*.yaml')
	$(OLM_SDK) bundle validate $(DEPLOY_OLM)

.PHONY: deploy-kubernetes-crd
deploy-kubernetes-crd: crd validate $(DEPLOY_KUBE)/10-custom-resource-definition.yaml
	$(KUBE) apply -f $(DEPLOY_KUBE)/10-custom-resource-definition.yaml

.PHONY: deploy-kubernetes
deploy-kubernetes: crd validate deploy-kubernete-crd
	$(KUBE) apply -f $(DEPLOY_KUBE)

.PHONY: deploy-olm-crd
deploy-olm-crd: crd validate $(DEPLOY_OLM)/clever-operator-postgresql.crd.yaml
	$(KUBE) apply -f $(DEPLOY_OLM)/clever-operator-postgresql.crd.yaml

.PHONY: deploy-olm
deploy-olm: crd validate deploy-olm-crd
	$(KUBE) apply -f $(DEPLOY_OLM)

# ------------------------------------------------------------------------------
# Clean up
.PHONY: clean
clean:
	$(CARGO) clean $(CARGO_OPTS)
	rm $(DEPLOY_KUBE)/10-custom-resource-definition.yaml
	rm $(DEPLOY_OLM)/clever-operator-postgresql.crd.yaml

.PHONY: clean-kubernetes
clean-kubernetes:
	$(KUBE) delete -f $(DEPLOY_KUBE)

.PHONY: clean-olm
clean-olm:
	$(KUBE) delete -f $(DEPLOY_OLM)
