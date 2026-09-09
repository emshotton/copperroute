use copper_dsn::error::BoardReadResult;
use copper_dsn::kicad::pcb::{default_net_class, read_pcb};
use copper_dsn::kicad::read_board_json;

const BOARDS: [&str; 11] = [
    "LPC2148_Stick_LPC2148_stick",
    "avr-fuser-32_adapter",
    "NRC2016_banked_ram",
    "MixSID_mixsid",
    "kitspace_dropbot-front-panel",
    "NRC2016_usb_sio",
    "Own-Mailbox-Hardware_mailbox",
    "Own-Mailbox-Hardware_eth",
    "Librecalc-Hardware__autosave-calculator",
    "FRM16_Relay_Module_I2C_Controller_relay_controller",
    "8bit-cpu_programming_interface",
];

#[test]
fn it_imports_the_solder_mask_validation_boards() {
    let Ok(root) = std::env::var("COPPERROUTE_PCBENCH") else {
        eprintln!(
            "skipping: set COPPERROUTE_PCBENCH to the PCBench corpus root to run this test, \
             e.g. COPPERROUTE_PCBENCH=<corpus root> cargo test -p copper-dsn \
             --test kicad_pcb_corpus -- --nocapture"
        );
        return;
    };
    let defaults = default_net_class();
    let mut failures = Vec::new();
    for stem in BOARDS {
        let path = std::path::Path::new(&root)
            .join(stem)
            .join("stripped.kicad_pcb");
        if !path.exists() {
            failures.push(format!("{stem}: no stripped.kicad_pcb at {}", path.display()));
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("the board reads");
        match read_pcb(&text, stem, &defaults) {
            Ok(imported) => match read_board_json(imported.board, None) {
                BoardReadResult::Success { board: Some(_), .. } => {}
                other => failures.push(format!("{stem}: the reader rejected it: {other:?}")),
            },
            Err(error) => failures.push(format!("{stem}: {}", error.message)),
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
