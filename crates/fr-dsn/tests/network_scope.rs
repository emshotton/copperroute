//! `Network.readScope` and the insertion tail at Network.java:1300-1322 — the id-parity tests.
//!
//! Java authority: `io/specctra/parser/Network.java`, `io/KiCadNetClassNames.java`.
//!
//! Every expectation here is a golden file under `tests/data/`, captured from the **pinned 2.3.0
//! jar** (plan ruling 10) by `tests/data/NProbe.java`. The exact commands are:
//!
//! ```text
//! $ export PATH=/opt/homebrew/opt/openjdk@25/bin:$PATH
//! $ javac -cp tools/freerouting-2.3.0.jar -d /tmp/nprobe crates/fr-dsn/tests/data/NProbe.java
//! $ java -Djava.awt.headless=true -cp tools/freerouting-2.3.0.jar:/tmp/nprobe \
//!       NProbe ../freerouting/fixtures/Issue026-J2_reference.dsn
//! ```
//!
//! and the same for `../freerouting/fixtures/Issue034-Green14SegLED.dsn`,
//! `../freerouting/fixtures/empty_board.dsn` and `crates/fr-dsn/tests/data/`'s
//! `via_order.dsn`, `network_via.dsn`, `class_pair.dsn` and `alias.dsn`. Each golden repeats its
//! own command in a `#`
//! header. [`dump`] below reproduces `NProbe.describe`'s line format exactly, so the comparison
//! is line-for-line over the whole file — item ids, kinds, layers, components, clearance
//! classes and net numbers included.

use fr_board::{Board, Item};
use fr_dsn::keyword::{Keyword, ScopeKeyword};
use fr_dsn::lexer::{DsnScanner, Token};
use fr_dsn::parser::scope_parameter::{DsnReadOptions, ReadScopeParameter, read_scope};

/// Reads a whole `(pcb …)` file the way Task 10's `DsnReader::read_board` will: consume the
/// leading `(` and `pcb`, then run the generic scope loop, which dispatches `parser`,
/// `resolution`, `structure`, `placement`, `library`, `part_library` and `network` in file
/// order.
fn read_pcb<T>(text: &str, f: impl FnOnce(bool, &mut ReadScopeParameter<'_>) -> T) -> T {
    let options = DsnReadOptions::default();
    let scanner = DsnScanner::new(text).expect("fits the lexer buffer");
    let mut p = ReadScopeParameter::new(scanner, &options);
    assert_eq!(p.scanner.next_token().expect("scan"), Some(Token::Open));
    assert_eq!(
        p.scanner.next_token().expect("scan"),
        Some(Token::Kw(Keyword::PcbScope))
    );
    let ok = read_scope(ScopeKeyword::Pcb, &mut p).expect("no scan error");
    f(ok, &mut p)
}

fn fixture(name: &str) -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../freerouting/fixtures/"
    );
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn test_data(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("test data {name}: {e}"))
}

/// The golden file's payload: every line that is not part of the `#` header.
fn golden(name: &str) -> Vec<String> {
    test_data(name)
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}

/// The Rust half of `NProbe.main` — same lines, same order, same spelling.
fn dump(board: &Board, warnings: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!("layers {}", board.get_layer_count()));
    let ctx = board.ctx();
    for (id, item) in board.items.iter() {
        let kind = match item {
            Item::BoardOutline(_) => "BoardOutline",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::ConductionArea(_) => "ConductionArea",
            Item::Pin(_) => "Pin",
            Item::Via(_) => "Via",
            Item::Trace(_) => "PolylineTrace",
        };
        let detail = match item {
            Item::ObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ViaObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ComponentObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ComponentOutline(o) => {
                format!(
                    " layer={} courtyard={}",
                    o.get_layer(&ctx),
                    o.is_courtyard()
                )
            }
            Item::Pin(p) => format!(" pin={}", p.name(&ctx).unwrap_or("null")),
            // Java's `ConductionArea extends ObstacleArea`, so `NProbe`'s `instanceof
            // ObstacleArea` arm catches it and prints the name too.
            Item::ConductionArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            _ => String::new(),
        };
        let header = item.header();
        let mut nets = String::new();
        for i in 0..header.net_count() {
            nets.push_str(&format!("{},", header.get_net_number(i)));
        }
        out.push(format!(
            "item {} {kind}{detail} cmp={} cl={} nets=[{nets}]",
            id.0,
            header.get_component_id(),
            header.clearance_class(),
        ));
    }
    out.push(format!("itemcount {}", board.items.len()));

    for i in 1..=board.rules.nets.max_net_number() {
        let net = board.rules.nets.get(i).expect("net in range");
        out.push(format!(
            "net {i} {} subnet={} class={} plane={}",
            net.name,
            net.subnet_number,
            board.rules.net_classes.get(net.get_net_class()).get_name(),
            net.contains_plane(),
        ));
    }

    let via_padstacks = board.library.get_via_padstacks();
    let names: Vec<&str> = via_padstacks
        .iter()
        .map(|id| {
            board
                .library
                .get_padstack(*id)
                .map_or("null", |p| p.name.as_str())
        })
        .collect();
    out.push(format!(
        "viapadstacks[{}] {}",
        via_padstacks.len(),
        names.join(" ")
    ));

    for (i, via) in board.rules.via_infos.iter().enumerate() {
        out.push(format!(
            "viainfo {i} {} padstack={} cl={} attach={}",
            via.get_name(),
            board
                .library
                .get_padstack(via.get_padstack())
                .map_or("null", |p| p.name.as_str()),
            via.get_clearance_class_index(),
            via.attach_smd_allowed(),
        ));
    }

    for (i, rule) in board.rules.via_rules.iter().enumerate() {
        let vias: Vec<&str> = rule.iter().map(fr_board::ViaInfo::get_name).collect();
        out.push(format!("viarule {i} {} [{}]", rule.name, vias.join(" ")));
    }

    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(fr_board::NetClassId(i));
        let via_rule = net_class
            .get_via_rule()
            .map_or("null", |rule| rule.name.as_str());
        let half_widths: Vec<String> = (0..net_class.layer_count())
            .map(|layer| net_class.get_trace_half_width(layer).to_string())
            .collect();
        out.push(format!(
            "netclass {i} {} traceCl={} viaRule={via_rule} hw=[{}] pullTight={} shoveFixed={} \
             minLen={:?} maxLen={:?}",
            net_class.get_name(),
            net_class.get_trace_clearance_class(),
            half_widths.join(","),
            net_class.get_pull_tight(),
            net_class.is_shove_fixed(),
            net_class.get_minimum_trace_length(),
            net_class.get_maximum_trace_length(),
        ));
    }

    let cm = &board.rules.clearance_matrix;
    let mut class_names = String::new();
    for i in 0..cm.get_class_count() {
        class_names.push_str(&format!("{i}:{} ", cm.get_name(i).unwrap_or("null")));
    }
    out.push(format!(
        "clclasses[{}] {}",
        cm.get_class_count(),
        class_names.trim_end()
    ));
    for i in 0..cm.get_class_count() {
        let mut row = String::new();
        for j in 0..cm.get_class_count() {
            row.push_str(&format!("{} ", cm.get_value(i, j, 0, false)));
        }
        out.push(format!("clrow {i} {}", row.trim_end()));
    }

    for i in 1..=board.components.count() {
        let component = board.components.get(i as i32);
        out.push(format!(
            "component {i} {} pkg={} placed={} front={} lp={}",
            component.name,
            board.library.packages.get(component.get_package()).name,
            component.is_placed(),
            component.placed_on_front(),
            component.get_logical_part().map_or("null", |no| board
                .library
                .logical_parts
                .get(no)
                .name
                .as_str()),
        ));
    }

    for i in 0..board.library.logical_parts.count() {
        out.push(format!(
            "logicalpart {i} {}",
            board.library.logical_parts.get(i).name
        ));
    }

    for warning in warnings {
        out.push(format!("warning {warning}"));
    }
    out
}

fn assert_matches_golden(dsn: &str, golden_name: &str) {
    read_pcb(dsn, |ok, p| {
        assert!(ok, "{golden_name}: the read must succeed");
        let board = p.board.as_ref().expect("board built");
        let actual = dump(board, &p.warnings);
        let expected = golden(golden_name);
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert_eq!(a, e, "{golden_name}: line {} differs", i + 1);
        }
        assert_eq!(
            actual.len(),
            expected.len(),
            "{golden_name}: line count differs"
        );
    });
}

// ------------------------------------------------------------------- whole-file id parity

/// The id-parity test the task exists for: every item id, kind, layer, component, clearance
/// class and net number of a real KiCad export, against the 2.3.0 jar.
///
/// `Issue026-J2_reference.dsn` has no `wiring` scope, so the 81 items it produces are *exactly*
/// the ones `Structure.createBoard` and `Network.readScope` insert: the outline (id 1), then
/// component `J2`'s 42 pins (ids 2-43), its four package keepouts (44-47), its 17 component
/// outlines (48-64), then component `U1`'s 17 pins (65-81). That interleaving — all of one
/// component's pins, then its keepouts, then its outlines, then the next component — is
/// `Network.insertComponent` (Network.java:1035, :1082, :1192) and is what fixes every later id.
#[test]
fn issue026_matches_javas_item_ids_nets_and_rules() {
    assert_matches_golden(
        &fixture("Issue026-J2_reference.dsn"),
        "Issue026-J2_reference-items.txt",
    );
}

/// The same, on a much bigger board (895 items, 38 components, 15 of them on the **back** side,
/// 616 component outlines): `Issue034-Green14SegLED.dsn`, the "multiple boundary paths" fixture.
#[test]
fn issue034_matches_javas_item_ids_nets_and_rules() {
    assert_matches_golden(
        &fixture("Issue034-Green14SegLED.dsn"),
        "Issue034-Green14SegLED-items.txt",
    );
}

/// The degenerate control: a file with **no** `network` scope at all. `Network.readScope` never
/// runs, so no via rule is created and the default net class keeps `viaRule = null`.
#[test]
fn empty_board_has_no_network_scope_and_no_via_rule() {
    assert_matches_golden(&fixture("empty_board.dsn"), "empty_board-items.txt");
}

// --------------------------------------------------------------- the via-padstack merge

/// Network.java:1276-1313 in one file:
///
/// * `(via VA VB.1 VMISSING VC)` in the `structure` scope and `(use_via VD)` in a net class are
///   **merged** into one list, structure first;
/// * `VB.1` is normalised to `VB` by stripping `\.\d+`;
/// * `VMISSING` has no padstack in the library, so it is **compacted out** — and `VC`, which
///   followed it, moves from index 3 to index 2. The assertion is on the *shifted* indices: the
///   golden's `viainfo 2 VC` (not `viainfo 3 VC`) is the whole point.
#[test]
fn via_padstack_names_are_merged_normalised_and_compacted() {
    assert_matches_golden(&test_data("via_order.dsn"), "via_order-items.txt");
}

/// The other half of Network.java:1282-1313: when the `structure` scope named **no** via
/// padstacks and no net class has a `(use_via …)`, `ReadScopeParameter.viaPadstackNames` is
/// still `null`, so the `if (scopeParameter.viaPadstackNames != null)` guard at :1286 skips
/// `setViaPadstacks` **entirely** — and the padstacks `Network.readViaInfo` appended along the
/// way (`board.library.addViaPadstack`, :274) survive instead of being overwritten.
///
/// `network_via.dsn` is exactly that file: a `structure` with no `(via …)`, no `(class …)` at
/// all, and two `(via <name> <padstack> default)` infos in the `network` scope. Java's answer is
/// `viapadstacks[2] VA VB`. This is the test that fails if `via_padstack_names` ever regresses
/// from `Option<Vec<String>>` to a plain `Vec`: an empty `Vec` takes the `Some` branch, calls
/// `set_via_padstacks(vec![])` and the line becomes `viapadstacks[0]`.
#[test]
fn a_network_only_via_padstack_list_survives_because_set_via_padstacks_is_skipped() {
    assert_matches_golden(&test_data("network_via.dsn"), "network_via-items.txt");

    read_pcb(&test_data("network_via.dsn"), |_, p| {
        let board = p.board.as_ref().expect("board built");
        let names: Vec<&str> = board
            .library
            .get_via_padstacks()
            .iter()
            .map(|id| {
                board
                    .library
                    .get_padstack(*id)
                    .expect("resolves")
                    .name
                    .as_str()
            })
            .collect();
        assert_eq!(names, ["VA", "VB"]);
    });
}

// ------------------------------------------------------------------ insert_class_pairs

/// `Network.addMixedClearanceRule` (Network.java:640-643) writes `(i, j)` **and** `(j, i)`, and
/// `Network.insertClassPairs` (:557) reuses the outer iterator for the inner loop, so a
/// `(classes Alpha Beta Gamma)` pairs `Alpha` with both of the others and never pairs `Beta`
/// with `Gamma`.
///
/// `Alpha` also carries the three `insertNetClass` paths no corpus fixture exercises, all
/// visible in the golden's `netclass 1 Alpha … hw=[3000,0] … minLen=10000.0 maxLen=50000.0`
/// line: a `(layer_rule F.Cu (rule (width 600)))` (Network.java:499-514 — halved *before*
/// `dsnToBoard`, so `hw[0]` is 3000), a `(use_layer F.Cu)` that deactivates `B.Cu` and zeroes
/// its half width (`createActiveTraceLayers`, :710-727 — and it runs *after* the layer rules,
/// which is why `hw[1]` is 0 and not 3000), and a `(length 5000 1000)` (`:475-481`, unrounded).
#[test]
fn class_pairs_write_both_halves_of_the_clearance_matrix() {
    assert_matches_golden(&test_data("class_pair.dsn"), "class_pair-items.txt");

    // Spelled out, so a golden regeneration cannot quietly drop either property.
    read_pcb(&test_data("class_pair.dsn"), |_, p| {
        let board = p.board.as_ref().expect("board built");
        let cm = &board.rules.clearance_matrix;
        let alpha = cm.get_no("Alpha").expect("Alpha clearance class");
        let beta = cm.get_no("Beta").expect("Beta clearance class");
        let gamma = cm.get_no("Gamma").expect("Gamma clearance class");
        // both halves
        assert_eq!(cm.get_value(alpha, beta, 0, false), 7770);
        assert_eq!(cm.get_value(beta, alpha, 0, false), 7770);
        assert_eq!(cm.get_value(alpha, gamma, 0, false), 7770);
        assert_eq!(cm.get_value(gamma, alpha, 0, false), 7770);
        // …but `(Beta, Gamma)` keeps the value `addClearanceRule` left, because the pair was
        // never written: Java's inner loop drained the outer iterator.
        assert_eq!(cm.get_value(beta, gamma, 0, false), 5000);
        assert_eq!(cm.get_value(gamma, beta, 0, false), 5000);
    });
}

// --------------------------------------------------------- the `viaPadstackNames` aliasing

/// Network.java:1278's `scopeParameter.viaPadstackNames = n.useVia` is an **assignment by
/// reference**: with no `(via …)` in the structure scope, the first net class's own `useVia`
/// list becomes the merged list, and the second class's `addAll` mutates it. `insertNetClass`
/// reads that list back (`createViaRule`, :539), so `Alpha`'s via rule ends up holding `Beta`'s
/// via too — `viarule 1 Alpha [VA-Alpha VB-Alpha]` in the golden, where `Beta`'s holds only
/// `[VB-Beta]`.
#[test]
fn a_net_classs_use_via_list_is_aliased_into_the_merged_via_padstack_names() {
    assert_matches_golden(&test_data("alias.dsn"), "alias-items.txt");

    read_pcb(&test_data("alias.dsn"), |_, p| {
        let board = p.board.as_ref().expect("board built");
        let alpha = board.rules.get_via_rule("Alpha").expect("Alpha via rule");
        let beta = board.rules.get_via_rule("Beta").expect("Beta via rule");
        assert_eq!(board.rules.via_rules[alpha.0].via_count(), 2);
        assert_eq!(board.rules.via_rules[beta.0].via_count(), 1);
    });
}
