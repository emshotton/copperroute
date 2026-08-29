//! Port of `autoroute.maze.AutorouteControl` (AutorouteControl.java:18-311) — "structure for
//! controlling the autoroute algorithm".
//!
//! # Plan-6 ruling 8: the settings are copied, not borrowed
//!
//! Java's `public final RouterSettings settings` (`:20`) has exactly **two** readers anywhere in
//! `autoroute/{maze,expansion,drill,path}`: `MazeSearchEngine.java:96-97` and `:111-112`, which
//! read `settings.fanout.maxEscapeLengthMm` / `minEscapeLengthMm`. So the port copies those two
//! numbers ([`fanout_max_escape_length`](AutorouteControl::fanout_max_escape_length) and its
//! `min` twin) and holds no reference. A `&RouterSettings` inside a struct the engine mutates
//! would put a lifetime on `AutorouteControl` and on everything that holds one, for two `f64`s.
//!
//! not ported: `AutorouteControl.settings` — the field itself; see above for its two readers.

use fr_board::ids::{ItemId, ViaRuleId};
use fr_board::rules::PadstackLookup;
use fr_board::{Board, Item};
use fr_geometry::Point;
use fr_settings::{ExpansionCostFactor, RouterSettings};

/// Port of `AutorouteControl.ViaMask` (AutorouteControl.java:299-310): one entry of the
/// "array of possible via ranges used by the autorouter".
///
/// The layer numbers are `i32`, not `usize`, because they come straight from
/// `Padstack.fromLayer()` / `toLayer()` (`:248-249`), which answer `shapes.length` and `-1`
/// respectively for a padstack whose every shape is `null` — an empty range, not a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViaMask {
    /// `final int fromLayer` (`:301`).
    pub from_layer: i32,
    /// `final int toLayer` (`:302`).
    pub to_layer: i32,
    /// `final boolean attachSmdAllowed` (`:303`) — the *padstack's* flag, unrelaxed. Read as a
    /// routing gate by `MazeExpansionEngine.java:339`.
    pub attach_smd_allowed: bool,
}

/// Port of `maze.AutorouteControl` (AutorouteControl.java:18-311).
///
/// Every field is `pub`, because Java's are: the maze writes `ripupAllowed`, `ripupCosts`,
/// `ripupPassNo`, `isFanout` and the four fanout-diagnostic fields directly.
#[derive(Debug, Clone, PartialEq)]
pub struct AutorouteControl {
    /// `final ExpansionCostFactor[] traceCosts` (`:23`): "the horizontal and vertical trace costs
    /// on each layer". Stored as handed in (`:179`).
    ///
    /// renamed: `AutorouteControl.ExpansionCostFactor` — the record (`:287`) is **re-exported**
    /// from `fr-settings`, never redeclared here (plan-6 ruling 8, the plan-4 obligation).
    pub trace_costs: Vec<ExpansionCostFactor>,
    /// `final double[] bendCosts` (`:25`), `settings.getBendCost(i)` per layer (`:145-147`).
    pub bend_costs: Vec<f64>,
    /// `final boolean withNeckdown` (`:26`), `settings.getAutomaticNeckdown()` (`:168`).
    pub with_neckdown: bool,
    /// `final boolean[] layerActive` (`:29`): "defines for each layer, if it may be used for
    /// routing" — `:150-161` and then `:226-228`.
    pub layer_active: Vec<bool>,
    /// `final int layerCount` (`:31`).
    pub layer_count: usize,
    /// `final int[] traceHalfWidth` (`:34`), `:217-222`.
    pub trace_half_width: Vec<i32>,
    /// `final int[] compensatedTraceHalfWidth` (`:40`), `:223-225`.
    pub compensated_trace_half_width: Vec<i32>,
    /// `final double[] viaRadii` (`:42`), `:250-260` then `:271-274`.
    pub via_radii: Vec<f64>,
    /// `final ViaCost[] addViaCosts` (`:45`): "the additional costs to min_normal via_cost for
    /// inserting a via between 2 layers". `addViaCosts[i].toLayer[j]` becomes `[i][j]`.
    ///
    /// renamed: `AutorouteControl.ViaCost` — a `static final class` (`:290-297`) whose whole
    /// body is one `int[]`, and which `:174-178` fills with zeroes and nothing ever writes
    /// again. A nested `Vec<i32>` says the same thing without a type.
    pub add_via_costs: Vec<Vec<i32>>,
    /// `int traceClearanceClassIndex` (`:48`), `:210` or `:213`.
    pub trace_clearance_class_index: usize,
    /// `boolean viasAllowed` (`:51`), `settings.getViasAllowed()` (`:141`).
    pub vias_allowed: bool,
    /// `boolean attachSmdAllowed` (`:54`): "true, if vias may drill to the pad of SMD pins".
    /// `:241-246` plus the pure-SMD relaxation of `:263-269`.
    pub attach_smd_allowed: bool,
    /// `double minNormalViaCost` (`:57`), `:282`.
    pub min_normal_via_cost: f64,
    /// `boolean ripupAllowed` (`:59`), default `false` (`:184`).
    pub ripup_allowed: bool,
    /// `int ripupCosts` (`:60`), default `1000` (`:185`). Also the seed of the maze's
    /// `java.util.Random` (`MazeSearchEngine.java:79-80`).
    pub ripup_costs: i32,
    /// `int ripupPassNo` (`:61`), default `1` (`:186`).
    pub ripup_pass_no: i32,
    /// `boolean isFanout` (`:64`): "if true, the autoroute algorithm completes after the first
    /// drill". Set by Plan 7's fanout pass.
    pub is_fanout: bool,
    /// `String fanoutStartPinName` (`:67`), default `null` (`:164`).
    pub fanout_start_pin_name: Option<String>,
    /// `Point fanoutStartPinCenter` (`:70`), default `null` (`:165`).
    pub fanout_start_pin_center: Option<Point>,
    /// `int fanoutStartPinLayer` (`:73`), default `-1` (`:73`, `:166`).
    pub fanout_start_pin_layer: i32,
    /// `boolean removeUnconnectedVias` (`:76`), default `true` (`:167`); the pass runner
    /// overwrites it with `!isFanoutEnabled()` (`BatchAutorouter.java:115`).
    pub remove_unconnected_vias: bool,
    /// `ViaRule viaRule` (`:79`): "the possible (partial) vias, which can be used by the
    /// autorouter". `:211` or `:214`. `None` is Java's `null`, which
    /// [`rebuild_via_info`](Self::rebuild_via_info) then dereferences exactly as Java does.
    pub via_rule: Option<ViaRuleId>,
    /// `int netNumber` (`:82`), `:205`.
    pub net_number: i32,
    /// `int viaClearanceClass` (`:85`), `:236-239`.
    pub via_clearance_class: usize,
    /// `ViaMask[] viaInfos` (`:88`), `:240`/`:260`.
    pub via_infos: Vec<ViaMask>,
    /// `int viaLowerBound` (`:91`), `0` (`:181`).
    pub via_lower_bound: usize,
    /// `int viaUpperBound` (`:94`), `layerCount` (`:182`).
    pub via_upper_bound: usize,
    /// `double maxViaRadius` (`:96`), `:273`.
    pub max_via_radius: f64,
    /// `int tidyRegionWidth` (`:99`), `Integer.MAX_VALUE` (`:169`).
    pub tidy_region_width: i32,
    /// `int pullTightAccuracy` (`:102`), `500` (`:170`).
    pub pull_tight_accuracy: i32,
    /// `int maxShoveTraceRecursionDepth` (`:105`), `20` (`:171`).
    pub max_shove_trace_recursion_depth: i32,
    /// `int maxShoveViaRecursionDepth` (`:108`), `5` (`:172`).
    pub max_shove_via_recursion_depth: i32,
    /// `int maxSpringOverRecursionDepth` (`:111`), `5` (`:173`).
    pub max_spring_over_recursion_depth: i32,
    /// `double minCheapViaCost` (`:114`), `0.8 * minNormalViaCost` (`:283`).
    pub min_cheap_via_cost: f64,

    /// Plan-6 ruling 8, first of the two copied settings: `MazeSearchEngine.java:96-99`'s
    /// ```text
    /// ctrl.settings.fanout != null && ctrl.settings.fanout.maxEscapeLengthMm != null
    ///     ? ctrl.settings.fanout.maxEscapeLengthMm * 1000.0
    ///     : 3000.0
    /// ```
    ///
    /// **Already scaled** — this is Java's `maxLen`, not a millimetre count. The `× 1000.0` and
    /// the `3000.0` fall-back are two different units in Java's own source; the port reproduces
    /// the arithmetic rather than tidying it, and the name drops the `_mm` the task brief used
    /// so that the trap is visible at the use site.
    pub fanout_max_escape_length: f64,
    /// Plan-6 ruling 8, second: `MazeSearchEngine.java:111-114`, `minEscapeLengthMm * 1000.0`
    /// or `500.0`. Same unit trap as [`fanout_max_escape_length`](Self::fanout_max_escape_length).
    pub fanout_min_escape_length: f64,
}

impl AutorouteControl {
    /// Port of `AutorouteControl(RoutingBoard, int, RouterSettings, int, ExpansionCostFactor[])`
    /// (`:123-132`) — the constructor `AutorouteConnectionRouter.java:43` and
    /// `BatchAutorouterThread.java:470` use, i.e. the one Plan 6 needs.
    ///
    /// # Panics
    ///
    /// For a `net_no` greater than `0` that the board does not have. `initNet`'s null-net arm
    /// (`:212-216`) runs, and then `:219`'s `board.rules.getTraceHalfWidth(netNumber, i)`
    /// dereferences the same `null` (`BoardRules.java:75-77`) and throws a
    /// `NullPointerException`. Pinned by `P6T8Probe ctrl` (`ctrl net=1094 threw
    /// java.lang.NullPointerException`) and by `tests/control.rs`'s
    /// `a_positive_net_the_board_does_not_have_throws_like_java`.
    pub fn new(
        board: &Board,
        net_no: i32,
        settings: &RouterSettings,
        via_costs: i32,
        trace_costs: &[ExpansionCostFactor],
    ) -> AutorouteControl {
        let mut control = AutorouteControl::private(board, settings, trace_costs);
        control.init_net(net_no, board, via_costs);
        control
    }

    /// Port of `AutorouteControl(RoutingBoard, int, RouterSettings)` (`:117-120`): the same
    /// thing with `settings.getTraceCosts()` and `settings.getViaCosts()`.
    ///
    /// renamed: the two-constructor overload set becomes `new` (`:123`) and this
    /// (`:117`); Rust has no overloading. `RoutingBoard.java:1023` is the caller.
    pub fn from_settings(
        board: &Board,
        net_no: i32,
        settings: &RouterSettings,
    ) -> AutorouteControl {
        let trace_costs = settings.get_trace_costs();
        AutorouteControl::new(
            board,
            net_no,
            settings,
            settings.get_via_costs(),
            &trace_costs,
        )
    }

    /// Port of the private `AutorouteControl(RoutingBoard, RouterSettings, ExpansionCostFactor[])`
    /// (`:134-187`).
    fn private(
        board: &Board,
        settings: &RouterSettings,
        trace_costs: &[ExpansionCostFactor],
    ) -> AutorouteControl {
        let layer_count = board.get_layer_count(); // :137
        let mut bend_costs = Vec::with_capacity(layer_count);
        for i in 0..layer_count {
            bend_costs.push(settings.get_bend_cost(i)); // :145-147
        }

        let mut layer_active = Vec::with_capacity(layer_count);
        for i in 0..layer_count {
            let active_setting = settings.get_layer_active(i); // :151
            // :152-161. Java logs `FRLogger.warn("Layer '…' is a dedicated power plane and cannot
            // be routed. Forcing active state to false.")`; the log is dropped (plan-6 global
            // constraints), the write is not.
            let layer = &board.layer_structure().layers[i];
            layer_active.push(if !layer.is_signal && active_setting {
                false
            } else {
                active_setting
            });
        }

        AutorouteControl {
            trace_costs: trace_costs.to_vec(),                      // :179
            bend_costs,                                             // :144-147
            with_neckdown: settings.get_automatic_neckdown(),       // :168
            layer_active,                                           // :140, :149-162
            layer_count,                                            // :137
            trace_half_width: vec![0; layer_count],                 // :138
            compensated_trace_half_width: vec![0; layer_count],     // :139
            via_radii: vec![0.0; layer_count],                      // :142
            add_via_costs: vec![vec![0; layer_count]; layer_count], // :143, :150, :174-178
            trace_clearance_class_index: 0,                         // set by initNet (:210/:213)
            vias_allowed: settings.get_vias_allowed(),              // :141
            attach_smd_allowed: false,                              // :180
            min_normal_via_cost: 0.0,                               // set by rebuildViaInfo (:282)
            ripup_allowed: false,                                   // :184
            ripup_costs: 1000,                                      // :185
            ripup_pass_no: 1,                                       // :186
            is_fanout: false,                                       // :163
            fanout_start_pin_name: None,                            // :164
            fanout_start_pin_center: None,                          // :165
            fanout_start_pin_layer: -1,                             // :166
            remove_unconnected_vias: true,                          // :167
            via_rule: None,                                         // set by initNet (:211/:214)
            net_number: 0,                                          // set by initNet (:205)
            via_clearance_class: 0,                                 // set by rebuildViaInfo (:236)
            via_infos: Vec::new(),                                  // set by rebuildViaInfo (:240)
            via_lower_bound: 0,                                     // :181
            via_upper_bound: layer_count,                           // :182
            max_via_radius: 0.0,                                    // set by rebuildViaInfo (:273)
            tidy_region_width: i32::MAX,                            // :169
            pull_tight_accuracy: 500,                               // :170
            max_shove_trace_recursion_depth: 20,                    // :171
            max_shove_via_recursion_depth: 5,                       // :172
            max_spring_over_recursion_depth: 5,                     // :173
            min_cheap_via_cost: 0.0,                                // set by rebuildViaInfo (:283)
            // plan-6 ruling 8: `MazeSearchEngine.java:96-99` and `:111-114`, copied rather than
            // borrowed. Both are already multiplied by 1000.0; see the field docs.
            fanout_max_escape_length: settings
                .fanout
                .as_ref()
                .and_then(|f| f.max_escape_length_mm)
                .map_or(3000.0, |mm| mm * 1000.0),
            fanout_min_escape_length: settings
                .fanout
                .as_ref()
                .and_then(|f| f.min_escape_length_mm)
                .map_or(500.0, |mm| mm * 1000.0),
        }
    }

    /// Port of the private `isPureSmdNet(RoutingBoard, int)` (`:189-202`): every connectable item
    /// of the net is a `Pin` that lives on exactly one layer. **An empty net is not pure SMD**
    /// (`:191-193`), which is what keeps net 0 and every unused net number out of the two
    /// relaxations of quirk #172.
    ///
    /// `pub` where Java's is `private static`, because `tests/control.rs` asserts it directly
    /// against the probe's `pureSmd=` column; `audit-port.sh` does not see private methods, so
    /// this widens nothing the audit was relying on.
    pub fn is_pure_smd_net(board: &Board, net_number: i32) -> bool {
        let net_items = board.get_connectable_items(net_number); // :190
        if net_items.is_empty() {
            return false; // :191-193
        }
        let ctx = board.ctx();
        net_items.into_iter().all(|id: ItemId| {
            // :195-199
            match board.get_item(id) {
                Some(item @ Item::Pin(_)) => item.first_layer(&ctx) == item.last_layer(&ctx),
                _ => false,
            }
        })
    }

    /// Port of the private `initNet(int, RoutingBoard, int)` (`:204-231`).
    fn init_net(&mut self, net_number: i32, board: &Board, via_costs: i32) {
        self.net_number = net_number; // :205
        let current_net_class = match board.rules.nets.get(net_number) {
            // :208-211
            Some(net) => {
                let class = net.get_net_class();
                let net_class = board.rules.net_classes.get(class);
                self.trace_clearance_class_index = net_class.get_trace_clearance_class();
                self.via_rule = net_class.get_via_rule();
                Some(class)
            }
            // :212-216
            None => {
                self.trace_clearance_class_index = 1;
                assert!(
                    !board.rules.via_rules.is_empty(),
                    "AutorouteControl.initNet: board.rules.viaRules.firstElement() — Java's \
                     Vector.firstElement() throws NoSuchElementException on an empty vector \
                     (AutorouteControl.java:214)"
                );
                self.via_rule = Some(ViaRuleId(0));
                None
            }
        };
        for i in 0..self.layer_count {
            // :218-222
            self.trace_half_width[i] = if net_number > 0 {
                board.rules.get_trace_half_width(net_number, i)
            } else {
                board.rules.get_trace_half_width(1, i)
            };
            // :223-225
            self.compensated_trace_half_width[i] = self.trace_half_width[i]
                + board
                    .rules
                    .clearance_matrix
                    .clearance_compensation_value(self.trace_clearance_class_index, i);
            // :226-228 — the net class's own layer gate, applied *after* the settings force-off.
            if let Some(class) = current_net_class
                && !board
                    .rules
                    .net_classes
                    .get(class)
                    .is_active_routing_layer(i)
            {
                self.layer_active[i] = false;
            }
        }
        self.rebuild_via_info(board, via_costs, net_number); // :230
    }

    /// Port of `rebuildViaInfo(RoutingBoard, int, int)` (`:234-284`): "rebuilds via info masks
    /// and costs for the specified board, via costs, and net."
    ///
    /// Not idempotent on [`via_radii`](Self::via_radii) or
    /// [`max_via_radius`](Self::max_via_radius) — `:258` and `:272-273` `max` into the existing
    /// values rather than resetting them, so a second run over a *smaller* padstack keeps the
    /// larger radius. That is Java's behaviour and the ripup passes rely on nothing else.
    ///
    /// # Panics
    ///
    /// When [`via_rule`](Self::via_rule) is `None`: `:235` calls `viaRule.viaCount()` with no
    /// null guard, which is a `NullPointerException` for a net class whose `viaRule` is unset
    /// (`NetClass.java:28`, `getViaRule` `:113-115`).
    ///
    /// # obligation: `AutorouteControl.rebuildViaInfo` — the ruling-H re-pointing divergence
    ///
    /// `:236`, `:243-244`, `:247` and `:260` reach the `ViaInfo` **through `viaRule.getVia(i)`**.
    /// Java's `ViaRule` holds object *references* (`ViaRule.java:21`), so after
    /// `RulesReader.applyViaInfo` (`RulesReader.java:340-350`) has replaced a via info, the rule
    /// keeps the **detached original**; this port's `ViaRule` holds `ViaInfoId` indices and
    /// necessarily reaches the **replacement**. Verified at HEAD by `P6T8Probe viadiv` on
    /// `Issue593-BBD_Mars-64.dsn` plus a one-line `.rules` re-declaring its only `(via …)` with
    /// `attach`:
    ///
    /// ```text
    /// after viainfo 0 Via[0-1]_800:400_um attach=true  cl=1
    /// after rulevia default[0] Via[0-1]_800:400_um attach=false cl=1 inList=false
    /// after rulevia default[0] Via[0-1]_800:400_um attach=false cl=1 inList=false
    /// ```
    ///
    /// so `attachSmdAllowed` here (and every `ViaMask.attachSmdAllowed`, which
    /// `MazeExpansionEngine.java:339` reads as a routing gate) differs. **Task 17** closes the
    /// register row by running `p6t1` with and without that `.rules` file against the jar; the
    /// Java half is already pinned in `docs/java-quirks.md` and `task-8-report.md`.
    pub fn rebuild_via_info(&mut self, board: &Board, via_costs: i32, net_number: i32) {
        let rule_id = self
            .via_rule
            .expect("AutorouteControl.rebuildViaInfo: viaRule is null — Java NPEs at :235");
        let via_rule = &board.rules.via_rules[rule_id.0];

        // :235-239
        self.via_clearance_class = if via_rule.via_count() > 0 {
            board
                .rules
                .via_infos
                .get(via_rule.get_via(0))
                .get_clearance_class_index()
        } else {
            1
        };
        self.via_infos = Vec::with_capacity(via_rule.via_count()); // :240
        self.attach_smd_allowed = false; // :241
        for i in 0..via_rule.via_count() {
            // :243
            let current_via = board.rules.via_infos.get(via_rule.get_via(i));
            if current_via.attach_smd_allowed() {
                self.attach_smd_allowed = true; // :244-246
            }
            let padstack = current_via.get_padstack(); // :247
            let from_layer = board.library.padstacks.padstack_from_layer(padstack); // :248
            let to_layer = board.library.padstacks.padstack_to_layer(padstack); // :249
            for j in from_layer..=to_layer {
                // :250-259. `getShape(j) != null` is the `Option`; a null shape contributes 0.
                let current_radius = board
                    .library
                    .padstacks
                    .padstack_shape_max_width(padstack, j)
                    .map_or(0.0, |width| 0.5 * width);
                let slot = &mut self.via_radii[j as usize];
                *slot = slot.max(current_radius);
            }
            // :260
            self.via_infos.push(ViaMask {
                from_layer,
                to_layer,
                attach_smd_allowed: current_via.attach_smd_allowed(),
            });
        }

        let pure_smd_net = AutorouteControl::is_pure_smd_net(board, net_number); // :263
        if !self.attach_smd_allowed && self.layer_count > 1 && pure_smd_net {
            // :264-269. HEAD-only, and it changes routing: "pure SMD nets must still be able to
            // escape their component layer, even if the DSN marks every padstack as attach-off.
            // This only relaxes the routing gate for same-net fanout; cross-net DRC remains
            // governed by the padstack's attach flag." Quirk #172, plan-6 ruling 14.
            self.attach_smd_allowed = true;
        }

        for j in 0..self.layer_count {
            // :271-274
            self.via_radii[j] = self.via_radii[j].max(f64::from(self.trace_half_width[j]));
            self.max_via_radius = self.max_via_radius.max(self.via_radii[j]);
        }
        let mut via_cost_factor = self.max_via_radius; // :275
        via_cost_factor = via_cost_factor.max(1.0); // :276
        if pure_smd_net {
            // :277-281. The second half of quirk #172: "pure SMD boards need a much cheaper via
            // escape to avoid exhausting the local pad channel before the search commits to a
            // layer change."
            via_cost_factor *= 0.1;
        }
        self.min_normal_via_cost = f64::from(via_costs) * via_cost_factor; // :282
        self.min_cheap_via_cost = 0.8 * self.min_normal_via_cost; // :283
    }
}
