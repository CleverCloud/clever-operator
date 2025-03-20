# Addon integration

> This document will go through the main steps to integrate a new addon in the
Clever Cloud operator.

## clever-cloud operator

* Create new custom resource definition module in `src/svc/crd/<ADDON>`
* Create new `CustomResource::<ADDON>` variant at `src/cmd.rs`
* Create new `Error::Watch<ADDON>` variant at `src/cmd/mod.rs`
* Insert new section in `docs/40-custom-resources.md`
* Create new exemplar configuration in `examples/kubernetes/<INCREMENT>-<ADDON>-addon.yml`
* Define resources in `deployments/kubernetes/<KUBE_VERSION>/20-deployment.yml`
* Generate custom resource definition with:

```sh
cargo run -- crd view > deployments/kubernetes/<KUBE_VERSION>/10-custom-resource-definition.yml
```

## clevercloud-sdk-rust

* Create new `AddonProviderId` variant at `src/v4/addon_provider/mod.rs#AddonProviderId::ADDON>`
  * Extend `FromStr` and `Display` implementations
  * Extend `Error::Parse` variants list
