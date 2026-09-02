//! `p8t5` — the legacy command line, Rust half.
//!
//! The twin of `scripts/differential/java/P8T5.java`. Runs the **same** code the binary runs —
//! `freerouting::legacy::resolve_slots` (the slots and the `FRLogger` lines) and
//! `fr_settings::prelude::apply_command_line_arguments` (the dead `routerSettings` bridge) — over
//! the argv table in `scripts/differential/matrix/p8t5-argv.tsv`, and prints the same transcript
//! Java prints.
//!
//! # The two halves of one Java method
//!
//! `GlobalSettings.applyCommandLineArguments` (`:521-838`) writes two disjoint sets of fields, and
//! the port splits them across two crates: `fr-settings` owns everything that lands on the
//! `@Deprecated routerSettings` bridge (plan-4 ruling 8), and `crates/freerouting` owns the
//! filename slots and the mode flags. Both are printed here, so the row is a comparison of the
//! whole method rather than of either half.
//!
//! # Two things this driver deliberately does not compare
//!
//! * **The rewritten native command line.** Java never builds one.
//!   `crates/freerouting/tests/legacy_cli.rs` pins it against these same slots instead.
//! * **The port-only diagnostics.** `-di` and `--compare-boards=` are accepted silently by Java
//!   and refused with a message by the port, because neither has anywhere to go in a headless
//!   program with no `BoardComparator`. Those two carry a `java_site` that
//!   `freerouting::logging::MESSAGE_MAP` does not hold, which is exactly the filter applied below:
//!   the transcript compares the **Java** message set.

use std::path::PathBuf;

use fr_dsn::format::double::java_float_to_string;
use fr_settings::prelude::{JavaEnum, LegacyBridge, apply_command_line_arguments};
use freerouting::legacy::{Level, LegacySlots, resolve_slots};
use freerouting::logging::{MESSAGE_MAP, console_level_string};

fn main() {
    let table = std::env::args()
        .nth(1)
        .map_or_else(
            || PathBuf::from("scripts/differential/matrix/p8t5-argv.tsv"),
            PathBuf::from,
        );
    let text = std::fs::read_to_string(&table)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", table.display()));

    let mut out = String::new();
    out.push_str("# p8t5 legacy command line — Java (applyCommandLineArguments)\n");

    let mut rows = 0usize;
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let label = fields.next().expect("split yields at least one field");
        let argv: Vec<String> = fields.map(str::to_string).collect();
        emit(&mut out, label, &argv);
        rows += 1;
    }
    out.push_str(&format!("# rows {rows}\n"));

    print!("{out}");
}

fn emit(out: &mut String, label: &str, argv: &[String]) {
    let (slots, diagnostics) = resolve_slots(argv);
    let bridge = apply_command_line_arguments(argv);

    out.push_str(&format!("[row] {label}\n"));
    out.push_str(&format!("argv {}\n", argv.len()));
    for (i, arg) in argv.iter().enumerate() {
        out.push_str(&format!("argv[{i}] = <{arg}>\n"));
    }

    let LegacySlots {
        initial_input_file,
        initial_output_file,
        initial_rules_file,
        design_session_filename,
        drc_report_file,
        show_help_option,
        ..
    } = &slots;
    out.push_str(&format!("input = {}\n", quote(initial_input_file.as_deref())));
    out.push_str(&format!(
        "output = {}\n",
        quote(initial_output_file.as_deref())
    ));
    out.push_str(&format!("rules = {}\n", quote(initial_rules_file.as_deref())));
    out.push_str(&format!(
        "session = {}\n",
        quote(design_session_filename.as_deref())
    ));
    out.push_str(&format!("drc = {}\n", quote(drc_report_file.as_deref())));
    out.push_str(&format!("help = {show_help_option}\n"));
    out.push_str(&format!(
        "console_level = {}\n",
        quote(Some(console_level_string(argv).as_str()))
    ));

    let LegacyBridge {
        max_passes,
        optimizer_max_threads,
        optimization_improvement_threshold,
        board_update_strategy,
        item_selection_strategy,
        hybrid_ratio,
        ignore_net_classes,
        router_enabled,
        drc_enabled,
    } = &bridge;
    // `new RouterSettings()` leaves every one of these null, so an unwritten bridge field and
    // Java's pristine object print the same four letters.
    out.push_str(&format!("router.enabled = {}\n", option(router_enabled.as_ref())));
    out.push_str(&format!("router.max_passes = {}\n", option(max_passes.as_ref())));
    out.push_str(&format!(
        "router.ignore_net_classes = {}\n",
        ignore_net_classes
            .as_ref()
            .map_or_else(|| "null".to_string(), |v| format!("<{}>", v.join("|")))
    ));
    out.push_str(&format!(
        "optimizer.max_threads = {}\n",
        option(optimizer_max_threads.as_ref())
    ));
    out.push_str(&format!(
        "optimizer.optimization_improvement_threshold = {}\n",
        optimization_improvement_threshold
            .map_or_else(|| "null".to_string(), java_float_to_string)
    ));
    out.push_str(&format!(
        "optimizer.board_update_strategy = {}\n",
        board_update_strategy.map_or("null", JavaEnum::java_name)
    ));
    out.push_str(&format!(
        "optimizer.item_selection_strategy = {}\n",
        item_selection_strategy.map_or("null", JavaEnum::java_name)
    ));
    out.push_str(&format!(
        "optimizer.hybrid_ratio = {}\n",
        quote(hybrid_ratio.as_deref())
    ));
    // `DesignRulesCheckerSettings.enabled` is a primitive `boolean`, so Java prints `false` where
    // the bridge holds nothing.
    out.push_str(&format!(
        "drc.enabled = {}\n",
        drc_enabled.unwrap_or(false)
    ));

    // Only the diagnostics whose Java site really is a `FRLogger` call — see the module docs.
    let messages: Vec<String> = diagnostics
        .iter()
        .filter(|d| MESSAGE_MAP.iter().any(|(site, _)| *site == d.java_site))
        .map(|d| {
            let kind = match d.level {
                // `LogEntryType.Warning` / `LogEntryType.Error`, upper-cased as Java prints them.
                Level::Warn => "WARNING",
                Level::Error => "ERROR",
            };
            format!("{kind} {}", d.message)
        })
        .collect();
    out.push_str(&format!("log {}\n", messages.len()));
    for (i, message) in messages.iter().enumerate() {
        out.push_str(&format!("log[{i}] = <{message}>\n"));
    }
    out.push('\n');
}

/// `null` as the four letters, anything else inside angle brackets so spaces show.
fn quote(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_string(), |v| format!("<{v}>"))
}

/// Java prints a boxed `Integer`/`Boolean` through `String.valueOf`, i.e. `null` or the value.
fn option<T: std::fmt::Display>(value: Option<&T>) -> String {
    value.map_or_else(|| "null".to_string(), std::string::ToString::to_string)
}
