use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct LayerSettings {
        #[serde(rename = "routable", default, skip_serializing_if = "Option::is_none")]
    pub routable: Option<bool>,

            #[serde(
        rename = "preferred_direction_horizontal",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub preferred_direction_horizontal: Option<bool>,

                #[serde(rename = "bend_cost", default, skip_serializing_if = "Option::is_none")]
    pub bend_cost: Option<f64>,
}

impl LayerSettings {
            pub const FIELD_NAMES: &'static [&'static str] =
        &["routable", "preferred_direction_horizontal", "bend_cost"];

            pub fn new(routable: Option<bool>, preferred_direction_horizontal: Option<bool>) -> Self {
        Self {
            routable,
            preferred_direction_horizontal,
            bend_cost: None,
        }
    }

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

