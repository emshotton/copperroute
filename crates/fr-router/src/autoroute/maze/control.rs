//! Port of `autoroute.maze.AutorouteControl` (AutorouteControl.java:18-311) — "structure for
//! controlling the autoroute algorithm".
//!
//! # Plan-6 ruling 8: the settings are copied, not borrowed
//!
//! **Java wins over the ruling's count.** The ruling says two readers; Task 13 found a third,
//! `MazeRipupResolver.java:99`'s `ctrl.settings.getStartRipupCosts()`. It is copied the same way,
//! into [`AutorouteControl::start_ripup_costs`].
//!
//! Java's `public final RouterSettings settings` (`:20`) has exactly **three** readers anywhere
//! in `autoroute/{maze,expansion,drill,path}`: `MazeSearchEngine.java:96-97` and `:111-112`,
//! which read `settings.fanout.maxEscapeLengthMm` / `minEscapeLengthMm`, and
//! `MazeRipupResolver.java:99`. So the port copies those three numbers
//! ([`fanout_max_escape_length`](AutorouteControl::fanout_max_escape_length), its `min` twin and
//! [`start_ripup_costs`](AutorouteControl::start_ripup_costs)) and holds no reference. A
//! `&RouterSettings` inside a struct the engine mutates would put a lifetime on
//! `AutorouteControl` and on everything that holds one, for three numbers.
//!
//! not ported: `AutorouteControl.settings` — the field itself; see above for its three readers.
//!
//! The four `Math.max(double, double)` sites of `rebuildViaInfo` (`:258`, `:272`, `:273`, `:276`)
//! are [`java_max`], not `f64::max`: Java propagates a NaN where Rust absorbs one, and the two
//! disagree on signed zero. See `destination_distance.rs`' module docs for why the distinction is
//! load-bearing in this package.

use fr_board::ids::{ItemId, NetClassId};
use fr_board::rules::{PadstackLookup, ViaRule};
use fr_board::{Board, Item};
use fr_geometry::{Point, java_max};
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
    ///
    /// # An owned rule, not an index into `board.rules.via_rules`
    ///
    /// Java's field is an object reference, and `RoutingBoard.fanout:1025-1044` **assigns a rule
    /// that is in no list at all** — a `new ViaRule(name + "_fallback")` built at run time from
    /// the net class's vias plus `rules.viaRules.firstElement()`'s. A `ViaRuleId` cannot name
    /// that rule, and pushing the synthetic rule onto `board.rules.via_rules` to get an index
    /// would put it in the DSN writer's output. So the control block owns its rule, exactly as
    /// [`fr_board::NetClass`] does since Plan 7 Task 11 (controller ruling AN).
    pub via_rule: Option<ViaRule>,
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

    /// Plan-6 ruling 8, the **third** copied setting — the one the ruling's survey missed:
    /// `MazeRipupResolver.java:99` reads `ctrl.settings.getStartRipupCosts()` to decide whether
    /// this pass is still early enough to protect fanout vias
    /// (`ctrl.ripupCosts <= startRipupCosts * 2`). It is a plain `int` off `RouterSettings`, with
    /// no board or layer dependence, so it is copied here exactly as the two fanout escape
    /// lengths are rather than reached through a borrowed settings object.
    ///
    /// `RouterSettings.getStartRipupCosts` (RouterSettings.java:537-548) answers
    /// `scoring != null ? scoring.startRipupCosts : 1`.
    pub start_ripup_costs: i32,
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
    // The seam marker that stood here is **closed** (Plan 7 Task 11): `RoutingBoard.fanout`
    // (RoutingBoard.java:978, the `new AutorouteControl(this, pinNetNo, routerSettings)` at
    // `:1023`) is the only Java caller of this overload, and Plan 7 Task 11 landed it as
    // `fr_router::board_ext::RoutingBoardExt::fanout`, which calls this and nothing else does.
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
            // plan-6 ruling 8's third reader, `MazeRipupResolver.java:99`.
            start_ripup_costs: settings.get_start_ripup_costs(),
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
                // `:211` copies the reference; the port copies the rule the class owns.
                self.via_rule = net_class.get_via_rule().cloned();
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
                self.via_rule = Some(board.rules.via_rules[0].clone());
                None
            }
        };
        for i in 0..self.layer_count {
            // :218-222.
            //
            // Java bug: `AutorouteControl.initNet`'s null-net arm (`:212-216`) completes only for
            // `netNumber <= 0`. `:218-222` calls `board.rules.getTraceHalfWidth(netNumber, i)` for
            // every `netNumber > 0`, and that is `nets.get(netNumber).getNetClass()` with no null
            // guard (BoardRules.java:74-77) — so a **positive unknown** net takes the arm and then
            // throws two lines later. `RoutingBoard.java:1023` builds a control from a pin's net
            // number, which makes a stale net number the reachable path; the probe's
            // `ctrl net=1094 threw java.lang.NullPointerException` is that. See
            // `docs/java-quirks.md` #173.
            //
            // fixed: T6 (#173) — the null-net arm gets its **own** half-width fallback, which is
            // the register's second option and the better one. Moving the `netNumber > 0` test
            // above the null lookup would keep the arm reading **net 1's** widths, and net 1 is
            // arbitrary: it is whichever net happens to have been declared first, it need not
            // exist, and nothing ties its class to a net the board does not have. The default net
            // class is the honest answer — it is where a net with no class of its own belongs, and
            // it is already the arm's choice for the clearance class (`:213`'s literal 1 is
            // `BoardRules.defaultClearanceClass`).
            self.trace_half_width[i] = match current_net_class {
                // A net the board has: unchanged, `:219`'s own lookup.
                Some(_) => board.rules.get_trace_half_width(net_number, i),
                // The null-net arm, for `netNumber <= 0` **and** for a positive unknown net,
                // which is the case Java never reached. `NetClassId(0)` is the slot
                // `BoardRules.createDefaultNetClass` (BoardRules.java:202-209) fills and
                // `getDefaultNetClass` (`:137-143`) hands back; it is read here rather than
                // through `get_default_net_class`, which takes `&mut self` to create the class
                // lazily and cannot be called on the `&Board` this method has. A board with no
                // net class at all cannot reach this line — `:214`'s `viaRules.firstElement()`
                // asserts above it, and a board with a via rule has rules — but the `None` arm
                // keeps the read total rather than resting on that.
                None if board.rules.net_classes.count() == 0 => 0,
                None => board
                    .rules
                    .net_classes
                    .get(NetClassId(0))
                    .get_trace_half_width(i),
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
    /// # Ruling H, closed — `ViaRule` owns its `ViaInfo`s (Plan 7 Task 0)
    ///
    /// `:236`, `:243-244`, `:247` and `:260` reach the `ViaInfo` **through `viaRule.getVia(i)`**.
    /// Java's `ViaRule` holds object *references* (`ViaRule.java:21`), so after
    /// `RulesReader.applyViaInfo` (`RulesReader.java:340-350`) has replaced a via info, the rule
    /// keeps the **detached original**. This port's `ViaRule` held `ViaInfoId` indices until Plan 7
    /// Task 0 and necessarily reached the **replacement**; it now holds owned copies and reaches
    /// the original, so this method's `via_rule.get_via(i)` — a `&ViaInfo` — needs no lookup
    /// through `board.rules.via_infos` and no logic change.
    ///
    /// The divergence was verified at HEAD by `P6T8Probe viadiv` on `Issue593-BBD_Mars-64.dsn`
    /// plus a one-line `.rules` re-declaring its only `(via …)` with `attach`:
    ///
    /// ```text
    /// after viainfo 0 Via[0-1]_800:400_um attach=true  cl=1
    /// after rulevia default[0] Via[0-1]_800:400_um attach=false cl=1 inList=false
    /// after rulevia default[0] Via[0-1]_800:400_um attach=false cl=1 inList=false
    /// ```
    ///
    /// so `attachSmdAllowed` here (and every `ViaMask.attachSmdAllowed`, which
    /// `MazeExpansionEngine.java:339` reads as a routing gate) differed. Plan 6 Task 17 ran the
    /// deciding comparison — `scripts/differential/run.sh p6t1
    /// ../freerouting/fixtures/Issue593-BBD_Mars-64.dsn 50 1 <rules>`, the port against the HEAD
    /// jar on the same board and the same `crates/fr-router/tests/data/ruling-h-redeclare.rules`:
    ///
    /// * **without** the `.rules` file the two agreed on all 50 connections, byte for byte;
    /// * **with** it they first differed at connection k = 6 (one extra item id, same geometry)
    ///   and then genuinely diverged from k = 8 on — the jar laying four traces to the port's two,
    ///   cumulative trace lengths `1401450.8259119983` and `1395031.4105961146`. The port routed
    ///   *shorter*, which is what `attachSmdAllowed = true` buys: it lets a via attach to an SMD
    ///   pad the jar's detached `ViaInfo` forbids.
    ///
    /// so per plan-6 ruling 9's other branch the register row closed **against** the re-pointing.
    /// **Plan 7 Task 0 landed the fix** (controller ruling AL) and the same command now MATCHes on
    /// all 50 connections, k = 6 and k = 8 included — transcript committed as
    /// `crates/fr-router/tests/data/p7t0-ruling-h-match.txt`, which is this marker's regression
    /// test. No acceptance fixture uses a `.rules` file, so `tests/reference/router-fixtures.txt`
    /// is unaffected.
    ///
    /// The *other* half of that register row — `Network.addViaRule` replacing a `ViaRule` while
    /// `NetClass.viaRule` keeps the detached original — **closed in Plan 7 Task 11** (controller
    /// ruling AN): [`fr_board::NetClass`] owns its rule, and so does
    /// [`via_rule`](Self::via_rule) here, so `:211`'s read reaches the detached original exactly
    /// as Java's does. Measured on `Issue143-rpi_splitter.dsn` plus
    /// `crates/fr-router/tests/data/ruling-h-viarule.rules` — DIFF on all eight connections
    /// before, MATCH after (`crates/fr-router/tests/data/p7t11-ruling-h-viarule.txt`).
    /// **Nothing in ruling H is open.**
    pub fn rebuild_via_info(&mut self, board: &Board, via_costs: i32, net_number: i32) {
        let via_rule = self
            .via_rule
            .clone()
            .expect("AutorouteControl.rebuildViaInfo: viaRule is null — Java NPEs at :235");
        let via_rule = &via_rule;

        // :235-239
        self.via_clearance_class = if via_rule.via_count() > 0 {
            via_rule.get_via(0).get_clearance_class_index()
        } else {
            1
        };
        self.via_infos = Vec::with_capacity(via_rule.via_count()); // :240
        self.attach_smd_allowed = false; // :241
        for i in 0..via_rule.via_count() {
            // :243
            let current_via = via_rule.get_via(i);
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
                *slot = java_max(*slot, current_radius);
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
            self.via_radii[j] = java_max(self.via_radii[j], f64::from(self.trace_half_width[j]));
            self.max_via_radius = java_max(self.max_via_radius, self.via_radii[j]);
        }
        let mut via_cost_factor = self.max_via_radius; // :275
        via_cost_factor = java_max(via_cost_factor, 1.0); // :276
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
