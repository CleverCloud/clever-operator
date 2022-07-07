# ------------------------------------------------------------------------------
# Define variables
PWD					?= $(shell pwd)
DIST				?= $(PWD)/target/release
BIN_DIR				?= $(HOME)/.local/bin

NAME				?= clever-operator
VERSION				?= $(shell git describe --candidates 1 --tags HEAD 2>/dev/null || echo HEAD)

DOCKER				?= $(shell which docker)
DOCKER_OPTS			?= --log-level debug
DOCKER_IMG			?= clevercloud/$(NAME):$(VERSION)

KUBE				?= $(shell which kubectl)
KUBE_VERSION		?= v1.21.0

OLM_SDK		    	?= $(shell which operator-sdk)
OLM_SDK_VERSION		?= 1.22.0
OLM_VERSION			?= 0.5.2

OCP_VALIDATOR		?= $(shell which ocp-olm-catalog-validator)
OCP_VERSION			?= 0.0.1

K8S_VALIDATOR		?= $(shell which k8s-community-bundle-validator)
K8S_VERSION			?= 0.0.1

KUBE_SCORE_VERSION  ?= 1.14.0
KUBE_SCORE			?= $(shell which kube-score)

DEPLOY_KUBE			?= deployments/kubernetes/$(KUBE_VERSION)
DEPLOY_OLM			?= deployments/operator-lifecycle-manager/$(OLM_VERSION)

FIND 				?= $(shell which find)

CARGO				?= $(shell which cargo)
CARGO_OPTS			?= --verbose

CURL				?= $(shell which curl)
MKDIR				?= $(shell which mkdir)
CHMOD				?= $(shell which chmod)

# ------------------------------------------------------------------------------
# Build operator
.PHONY: build
build: $(DIST)/$(NAME) $(shell $(FIND) -type f -name '*.rs')

$(DIST)/$(NAME): $(shell $(FIND) -type f -name '*.rs')
	$(CARGO) build $(CARGO_OPTS) --release

# ------------------------------------------------------------------------------
# Install command line tools

.PHONY: install-cli-tools
install-cli-tools:
	$(MKDIR) -p $(BIN_DIR)
	$(CURL) -L https://github.com/operator-framework/operator-sdk/releases/download/v$(OLM_SDK_VERSION)/operator-sdk_linux_amd64 > $(BIN_DIR)/operator-sdk && $(CHMOD) +x $(BIN_DIR)/operator-sdk
	$(CURL) -L https://github.com/redhat-openshift-ecosystem/ocp-olm-catalog-validator/releases/download/v$(OCP_VERSION)/linux-amd64-ocp-olm-catalog-validator > $(BIN_DIR)/ocp-olm-catalog-validator && $(CHMOD) +x $(BIN_DIR)/ocp-olm-catalog-validator
	$(CURL) -L https://github.com/k8s-operatorhub/bundle-validator/releases/download/v$(K8S_VERSION)/linux-amd64-k8s-community-bundle-validator > $(BIN_DIR)/k8s-community-bundle-validator && $(CHMOD) +x $(BIN_DIR)/k8s-community-bundle-validator
	$(CURL) -L https://github.com/zegl/kube-score/releases/download/v$(KUBE_SCORE_VERSION)/kube-score_$(KUBE_SCORE_VERSION)_linux_amd64 >  $(BIN_DIR)/kube-score && $(CHMOD) +x $(BIN_DIR)/kube-score

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
crd: build $(DEPLOY_OLM)/manifests/clever-operator-pulsar.crd.yaml $(DEPLOY_KUBE)/10-custom-resource-definition.yaml $(DEPLOY_OLM)/manifests/clever-operator-mongodb.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-mysql.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-postgresql.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-redis.crd.yaml

$(DEPLOY_KUBE)/10-custom-resource-definition.yaml:
	$(DIST)/$(NAME) custom-resource-definition view > $(DEPLOY_KUBE)/10-custom-resource-definition.yaml

$(DEPLOY_OLM)/manifests/clever-operator-postgresql.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view postgresql > $(DEPLOY_OLM)/manifests/clever-operator-postgresql.crd.yaml

$(DEPLOY_OLM)/manifests/clever-operator-redis.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view redis > $(DEPLOY_OLM)/manifests/clever-operator-redis.crd.yaml

$(DEPLOY_OLM)/manifests/clever-operator-mysql.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view mysql > $(DEPLOY_OLM)/manifests/clever-operator-mysql.crd.yaml

$(DEPLOY_OLM)/manifests/clever-operator-mongodb.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view mongodb > $(DEPLOY_OLM)/manifests/clever-operator-mongodb.crd.yaml

$(DEPLOY_OLM)/manifests/clever-operator-pulsar.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view pulsar > $(DEPLOY_OLM)/manifests/clever-operator-pulsar.crd.yaml

$(DEPLOY_OLM)/manifests/clever-operator-elasticsearch.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view elasticsearch > $(DEPLOY_OLM)/manifests/clever-operator-elasticsearch.crd.yaml

$(DEPLOY_OLM)/manifests/clever-operator-config-provider.crd.yaml:
	$(DIST)/$(NAME) custom-resource-definition view config-provider > $(DEPLOY_OLM)/manifests/clever-operator-config-provider.crd.yaml

.PHONY: validate
validate: $(shell $(FIND) -type f -name '*.yaml')
	$(KUBE_SCORE) score $(shell $(FIND) $(DEPLOY_KUBE) -type f -name '*.yaml')
	$(OLM_SDK) bundle validate $(DEPLOY_OLM)
	$(OCP_VALIDATOR) $(DEPLOY_OLM) --optional-values="file=$(DEPLOY_OLM)/metadata/annotations.yaml" --output json-alpha1
	$(K8S_VALIDATOR) $(DEPLOY_OLM) --output json-alpha1

.PHONY: deploy-kubernetes-crd
deploy-kubernetes-crd: crd validate $(DEPLOY_KUBE)/10-custom-resource-definition.yaml
	$(KUBE) apply -f $(DEPLOY_KUBE)/10-custom-resource-definition.yaml

.PHONY: deploy-kubernetes
deploy-kubernetes: crd validate deploy-kubernete-crd
	$(KUBE) apply -f $(DEPLOY_KUBE)/manifests/clever-operator.clusterserviceversion.yaml

.PHONY: deploy-olm-crd
deploy-olm-crd: crd $(DEPLOY_OLM)/manifests/clever-operator-elasticsearch.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-config-provider.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-postgresql.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-redis.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-mysql.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-mongodb.crd.yaml $(DEPLOY_OLM)/manifests/clever-operator-pulsar.crd.yaml validate
	$(KUBE) apply -f $(DEPLOY_OLM)/manifests/clever-operator-postgresql.crd.yaml
	$(KUBE) apply -f $(DEPLOY_OLM)/manifests/clever-operator-redis.crd.yaml
	$(KUBE) apply -f $(DEPLOY_OLM)/manifests/clever-operator-mysql.crd.yaml
	$(KUBE) apply -f $(DEPLOY_OLM)/manifests/clever-operator-mongodb.crd.yaml
	$(KUBE) apply -f $(DEPLOY_OLM)/manifests/clever-operator-pulsar.crd.yaml
	$(KUBE) apply -f $(DEPLOY_OLM)/manifests/clever-operator-config-provider.crd.yaml
	$(KUBE) apply -f $(DEPLOY_OLM)/manifests/clever-operator-elasticsearch.crd.yaml

.PHONY: deploy-olm
deploy-olm: crd validate deploy-olm-crd
	$(KUBE) apply -f $(DEPLOY_OLM)

# ------------------------------------------------------------------------------
# Clean up
.PHONY: clean
clean: clean-kubernetes clean-olm
	$(CARGO) clean $(CARGO_OPTS)
	rm $(DEPLOY_KUBE)/10-custom-resource-definition.yaml

.PHONY: clean-kubernetes
clean-kubernetes:
	$(KUBE) delete -f $(DEPLOY_KUBE)

.PHONY: clean-olm
clean-olm:
	$(KUBE) delete -f $(DEPLOY_OLM)/manifests
