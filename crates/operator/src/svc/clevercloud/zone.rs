//! # Zone module
//!
//! This module validates the region a custom resource asks for against the
//! zones the plan of its add-on can be deployed in.
//!
//! The region is a free-form string in the custom resource definition: nothing
//! constrains it before it reaches the api, which reports an unknown zone
//! through an opaque creation error, several reconciliations later. The zones
//! are already carried by the [`Plan`] the reconciliation resolves, so the
//! check costs no request.
//!
//! The sdk makes `zones` a required field of a plan, so the list is always the
//! one the api answered: it may be empty, never absent.

use std::fmt::{self, Display, Formatter};

use clevercloud_sdk::v2::addon::Plan;

// -----------------------------------------------------------------------------
// Rejection structure

/// A region none of the zones of the plan matches.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Rejection {
    /// The region the custom resource asks for.
    pub region: String,
    /// The zones the plan can be deployed in, in the order the api returned
    /// them. Never empty: an empty list is not information the region can be
    /// rejected on.
    pub zones: Vec<String>,
}

impl Rejection {
    /// Renders the accepted zones as a comma-separated list of quoted names,
    /// the way the sdk renders the plans a provider offers.
    pub fn options(&self) -> String {
        self.zones
            .iter()
            .map(|zone| format!("'{zone}'"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Display for Rejection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to find zone '{}' amongst available options: {}",
            self.region,
            self.options()
        )
    }
}

// -----------------------------------------------------------------------------
// Helpers

/// Returns the zones the resolved plan can be deployed in, or an empty list
/// when the resolution yielded no plan at all.
///
/// The sdk answers `None` for a provider that exposes no plan: it says nothing
/// about the zones either, and the empty list returned here makes [`validate`]
/// accept every region, the same way an empty `zones` does.
pub fn zones(plan: Option<&Plan>) -> &[String] {
    plan.map_or(&[], |plan| &plan.zones)
}

/// Returns whether `region` is one of the `zones` the plan can be deployed in.
///
/// The comparison is case-insensitive, as the sdk already compares the slug,
/// name and identifier of a plan.
///
/// # Errors
///
/// * [`Rejection`]: the zones are known and the region is not one of them.
///
/// An empty list of zones means the api does not tell which zones exist, not
/// that no zone is allowed: a self-hosted installation may validly answer one,
/// and the region is then accepted. This mirrors the way the sdk treats an
/// addon provider without plans.
pub fn validate(region: &str, zones: &[String]) -> Result<(), Rejection> {
    if zones.is_empty() {
        return Ok(());
    }

    if zones.iter().any(|zone| zone.eq_ignore_ascii_case(region)) {
        return Ok(());
    }

    Err(Rejection {
        region: region.to_string(),
        zones: zones.to_vec(),
    })
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use clevercloud_sdk::v2::addon::Plan;

    use super::{Rejection, validate, zones};

    /// Builds a plan as the api describes it, `zones` being the only field the
    /// validation reads.
    fn plan(zones: &[&str]) -> Plan {
        serde_json::from_value(serde_json::json!({
            "id": "plan_2e0e9596-dd73-4b73-84d9-e165372c5324",
            "name": "S Small Space",
            "slug": "s_sml",
            "price": 0.0,
            "price_id": "price_id_s_sml",
            "features": [],
            "zones": zones
        }))
        .expect("to deserialize a plan")
    }

    #[test]
    fn a_region_the_plan_is_deployed_in_is_accepted() {
        assert_eq!(
            validate("par", &plan(&["par", "grahds", "scw"]).zones),
            Ok(())
        );
    }

    /// The api answers lowercase zones while a custom resource is written by
    /// hand: the comparison ignores the case, as the plan resolution of the sdk
    /// already does on slugs, names and identifiers.
    #[test]
    fn the_case_of_the_region_does_not_matter() {
        assert_eq!(validate("PAR", &plan(&["par", "grahds"]).zones), Ok(()));
        assert_eq!(validate("GrahDS", &plan(&["par", "grahds"]).zones), Ok(()));
        assert_eq!(validate("par", &plan(&["PAR"]).zones), Ok(()));
    }

    /// The invariant of the check: a plan the api returns without any zone does
    /// not tell which zones exist, and must never reject a region. A
    /// self-hosted installation exposing its own zones may validly answer this,
    /// and rejecting here would make the operator unusable on it.
    #[test]
    fn an_empty_list_of_zones_accepts_every_region() {
        assert_eq!(validate("par", &plan(&[]).zones), Ok(()));
        assert_eq!(
            validate("a-zone-of-a-private-installation", &plan(&[]).zones),
            Ok(())
        );
        assert_eq!(validate("", &plan(&[]).zones), Ok(()));
    }

    /// The other half of the invariant: the plan resolution yielding no plan at
    /// all tells nothing about the zones either, and must accept every region
    /// rather than reject it.
    #[test]
    fn a_resolution_without_a_plan_accepts_every_region() {
        assert!(zones(None).is_empty());
        assert_eq!(validate("par", zones(None)), Ok(()));
        assert_eq!(
            validate("a-zone-of-a-private-installation", zones(None)),
            Ok(())
        );
    }

    /// The zones of a resolved plan are the ones the api answered with it.
    #[test]
    fn the_zones_of_a_resolved_plan_are_the_ones_of_the_api() {
        let plan = plan(&["par", "grahds"]);

        assert_eq!(
            zones(Some(&plan)),
            ["par".to_string(), "grahds".to_string()]
        );
        assert_eq!(validate("grahds", zones(Some(&plan))), Ok(()));
        assert!(validate("mars", zones(Some(&plan))).is_err());
    }

    /// A region the plan cannot be deployed in is rejected, and the message
    /// names the zones the user may pick from.
    #[test]
    fn a_region_the_plan_is_not_deployed_in_is_rejected_with_the_accepted_zones() {
        let rejection = validate("mars", &plan(&["par", "grahds", "scw"]).zones)
            .expect_err("the region to be rejected");

        assert_eq!(
            rejection,
            Rejection {
                region: "mars".to_string(),
                zones: vec!["par".to_string(), "grahds".to_string(), "scw".to_string()],
            }
        );

        assert_eq!(
            rejection.to_string(),
            "failed to find zone 'mars' amongst available options: 'par', 'grahds', 'scw'"
        );
        assert_eq!(rejection.options(), "'par', 'grahds', 'scw'");
    }

    /// A plan bound to a single zone is a list like any other: it accepts that
    /// zone and rejects everything else, listing the only option available.
    #[test]
    fn a_plan_with_a_single_zone_still_accepts_and_rejects() {
        assert_eq!(validate("par", &plan(&["par"]).zones), Ok(()));

        let rejection =
            validate("grahds", &plan(&["par"]).zones).expect_err("the region to be rejected");

        assert_eq!(
            rejection.to_string(),
            "failed to find zone 'grahds' amongst available options: 'par'"
        );
    }

    /// A region that is empty, or made of spaces, matches no zone: it is
    /// rejected like any other unknown region rather than silently accepted.
    /// The region is not trimmed, as the api would not trim it either.
    #[test]
    fn a_blank_region_is_rejected_when_the_zones_are_known() {
        let rejection =
            validate("", &plan(&["par", "grahds"]).zones).expect_err("the region to be rejected");

        assert_eq!(
            rejection.to_string(),
            "failed to find zone '' amongst available options: 'par', 'grahds'"
        );

        let rejection =
            validate("   ", &plan(&["par"]).zones).expect_err("the region to be rejected");

        assert_eq!(rejection.region, "   ");

        let rejection =
            validate(" par ", &plan(&["par"]).zones).expect_err("the region to be rejected");

        assert_eq!(rejection.region, " par ");
    }
}
