//! `settings/LayerSettings.java` (75 lines): per-layer overrides nested inside
//! [`crate::RouterSettings::layers`].

use serde::{Deserialize, Serialize};

/// Settings configuration for a single board layer (`LayerSettings.java:7-21`).
///
/// Every field follows the `RouterSettings` nullable-field contract (plan ruling 4): `None` means
/// "this source has no opinion" and is left for a later merge source or `DefaultSettings` (Task
/// 6) to fill in.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct LayerSettings {
    /// Whether the layer is routable by the autorouter. `LayerSettings.java:9-10`.
    #[serde(rename = "routable", default, skip_serializing_if = "Option::is_none")]
    pub routable: Option<bool>,

    /// Whether the preferred trace direction on this layer is horizontal.
    /// `LayerSettings.java:12-13`.
    #[serde(
        rename = "preferred_direction_horizontal",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub preferred_direction_horizontal: Option<bool>,

    /// Per-layer bend cost added to the maze expansion value each time the router changes
    /// direction on this layer. `None` means "use the board default". Valid range when
    /// non-`None`: 0.0 (no penalty) to 9.9 (strongly avoids bends). `LayerSettings.java:15-21`.
    #[serde(rename = "bend_cost", default, skip_serializing_if = "Option::is_none")]
    pub bend_cost: Option<f64>,
}

impl LayerSettings {
    /// Rust field names in `LayerSettings.getDeclaredFields()` source order — pins the order
    /// `copy_fields` (Task 2) must iterate in.
    pub const FIELD_NAMES: &'static [&'static str] =
        &["routable", "preferred_direction_horizontal", "bend_cost"];

    /// Convenience constructor mirroring `LayerSettings(Boolean, Boolean)`
    /// (`LayerSettings.java:31-34`).
    pub fn new(routable: Option<bool>, preferred_direction_horizontal: Option<bool>) -> Self {
        Self {
            routable,
            preferred_direction_horizontal,
            bend_cost: None,
        }
    }

    /// Full constructor mirroring `LayerSettings(Boolean, Boolean, Double)`
    /// (`LayerSettings.java:37-41`).
    pub fn with_bend_cost(
        routable: Option<bool>,
        preferred_direction_horizontal: Option<bool>,
        bend_cost: Option<f64>,
    ) -> Self {
        Self {
            routable,
            preferred_direction_horizontal,
            bend_cost,
        }
    }
}

// renamed: clone -> derive(Clone) (LayerSettings.java:44-53). Java's clone() falls back from
// Object.clone() to a manual field-by-field copy of three immutable wrapper fields; a derived
// Clone over three Copy fields is the same operation.
//
// not ported: equals, hashCode (LayerSettings.java:56-73). The survey found no caller relying on
// LayerSettings identity/hash semantics; the derived PartialEq is structurally the same anyway
// (field-by-field Option comparison), so a hand-written impl would add nothing.
