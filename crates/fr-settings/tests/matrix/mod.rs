//! workspace can `#[path = "…/tests/matrix/mod.rs"] mod matrix;` and enumerate the identical
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use fr_board::prelude::*;
use fr_geometry::{IntBox, PolylineShapeRef, TileShape};
use fr_settings::prelude::*;

pub struct DsnCase {
    pub id: &'static str,
    pub fixture: Option<&'static str>,
    pub layer_count: usize,
}

pub struct RulesCase {
    pub id: &'static str,
    pub cli_rules: Option<&'static str>,
    pub scheduler_rules: Option<&'static str>,
}

pub struct EnvCase {
    pub id: &'static str,
    pub vars: &'static [(&'static str, &'static str)],
}

pub struct CliCase {
    pub id: &'static str,
    pub argv: &'static [&'static str],
}

pub const BOARD_WIDTH: i32 = 2_000_000;
pub const BOARD_HEIGHT: i32 = 1_000_000;

pub const DSN_CASES: [DsnCase; 4] = [
    DsnCase {
        id: "dsn-none",
        fixture: None,
        layer_count: 2,
    },
    DsnCase {
        id: "dsn2-bare",
        fixture: Some("Issue413-test.dsn"),
        layer_count: 2,
    },
    DsnCase {
        id: "dsn2-autoroute",
        fixture: Some("Issue187-processor.Z80.dsn"),
        layer_count: 2,
    },
    DsnCase {
        id: "dsn4-bare",
        fixture: Some("Issue066-Project_GP8B.dsn"),
        layer_count: 4,
    },
];

pub const PRIMARY_RULES: &str = "Plan4Matrix-primary.rules";
pub const ADJACENT_RULES: &str = "Plan4Matrix-adjacent.rules";

pub const RULES_CASES: [RulesCase; 4] = [
    RulesCase {
        id: "rules-none",
        cli_rules: None,
        scheduler_rules: None,
    },
    RulesCase {
        id: "rules-cli",
        cli_rules: Some(PRIMARY_RULES),
        scheduler_rules: Some(PRIMARY_RULES),
    },
    RulesCase {
        id: "rules-scheduler",
        cli_rules: None,
        scheduler_rules: Some(ADJACENT_RULES),
    },
    RulesCase {
        id: "rules-split",
        cli_rules: Some(PRIMARY_RULES),
        scheduler_rules: Some(ADJACENT_RULES),
    },
];

pub const ENV_CASES: [EnvCase; 2] = [
    EnvCase {
        id: "env-none",
        vars: &[],
    },
    EnvCase {
        id: "env-set",
        vars: &[
            ("FREEROUTING__ROUTER__MAX_PASSES", "77"),
            (
                "FREEROUTING__ROUTER__LAYERS__PREFERRED_DIRECTION_HORIZONTAL",
                "true,true",
            ),
        ],
    },
];

pub const CLI_CASES: [CliCase; 2] = [
    CliCase {
        id: "cli-none",
        argv: &[],
    },
    CliCase {
        id: "cli-set",
        argv: &["--router.max_passes=88", "--router.optimizer.enabled=false"],
    },
];

pub struct Case {
    pub id: String,
    pub dsn: &'static DsnCase,
    pub rules: &'static RulesCase,
    pub env: &'static EnvCase,
    pub cli: &'static CliCase,
}

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(64);
    for dsn in &DSN_CASES {
        for rules in &RULES_CASES {
            for env in &ENV_CASES {
                for cli in &CLI_CASES {
                    cases.push(Case {
                        id: format!("{}/{}/{}/{}", dsn.id, rules.id, env.id, cli.id),
                        dsn,
                        rules,
                        env,
                        cli,
                    });
                }
            }
        }
    }
    cases
}

pub fn data_path(name: &str) -> PathBuf {
    parity::workspace_root()
        .join("crates")
        .join("fr-settings")
        .join("tests")
        .join("data")
        .join(name)
}

pub fn dsn_source(case: &DsnCase) -> Option<DsnFileSettings> {
    static CACHE: OnceLock<Vec<Option<DsnFileSettings>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        DSN_CASES
            .iter()
            .map(|dsn| {
                let name = dsn.fixture?;
                let bytes = std::fs::read(parity::fixture(name))
                    .unwrap_or_else(|e| panic!("cannot read fixture {name}: {e}"));
                Some(DsnFileSettings::new(&bytes[..], name))
            })
            .collect()
    });
    let index = DSN_CASES
        .iter()
        .position(|candidate| candidate.id == case.id)
        .expect("case comes from DSN_CASES");
    cache[index].clone()
}

pub fn rules_bytes(name: Option<&str>) -> Option<Vec<u8>> {
    let name = name?;
    static CACHE: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        [PRIMARY_RULES, ADJACENT_RULES]
            .into_iter()
            .map(|name| {
                let path = data_path(name);
                let bytes = std::fs::read(&path)
                    .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
                (name.to_string(), bytes)
            })
            .collect()
    });
    Some(cache[name].clone())
}

pub fn env_source(case: &EnvCase) -> EnvironmentVariablesSource {
    let vars: BTreeMap<String, String> = case
        .vars
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    EnvironmentVariablesSource::new(&vars)
}

pub fn cli_source(case: &CliCase) -> CliSettings {
    let argv: Vec<String> = case.argv.iter().map(|arg| (*arg).to_string()).collect();
    CliSettings::new(&argv)
}

pub fn board(case: &DsnCase) -> Board {
    let names: [&str; 4] = ["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"];
    let layer_names: Vec<&str> = match case.layer_count {
        2 => vec![names[0], names[3]],
        4 => names.to_vec(),
        other => panic!("no layer naming for {other} layers"),
    };
    let layers = LayerStructure::new(
        layer_names
            .into_iter()
            .map(|name| Layer::new(name.to_string(), true))
            .collect(),
    );
    let clearance_matrix = ClearanceMatrix::get_default_instance(&layers, 10);
    let mut rules = BoardRules::new(layers.clone(), clearance_matrix);
    rules.create_default_net_class();
    let box_ = IntBox::from_coords(0, 0, BOARD_WIDTH, BOARD_HEIGHT);
    Board::new(
        vec![PolylineShapeRef::Tile(TileShape::Box(box_))],
        0,
        box_,
        rules,
        BoardLibrary::new(Padstacks::new(layers), Packages::new()),
        Components::new(),
        Communication::default(),
    )
}
