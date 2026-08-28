//! Board items: the [`Item`] enum, its nine variants, and the [`Connectable`] dispatch.
//!
//! Java: `board/model/items/{Item,Connectable,BoardItemType}.java` plus the nine concrete
//! subclasses (`board/trace/PolylineTrace.java`, `board/model/items/{Via,Pin,ObstacleArea,
//! ConductionArea,ViaObstacleArea,ComponentObstacleArea,ComponentOutline}.java`,
//! `board/model/structure/BoardOutline.java`).
//!
//! # Shape of the port
//!
//! Java's item model is a five-level class hierarchy:
//!
//! ```text
//! Item (abstract)
//! ├── Trace (abstract, Connectable) ── PolylineTrace
//! ├── DrillItem (abstract, Connectable) ── Via, Pin
//! ├── ObstacleArea ── ConductionArea (Connectable), ViaObstacleArea, ComponentObstacleArea
//! ├── ComponentOutline
//! └── BoardOutline
//! ```
//!
//! The port flattens it into one `enum Item` with the nine **concrete** variants, because the
//! board stores items in a single `BTreeMap<ItemId, Item>` (plan-rulings.md #1) and Rust has no
//! inheritance. Three consequences run through this file:
//!
//! * Java's shared state (`Item`'s fields) becomes [`ItemHeader`], embedded in every variant
//!   struct as `hdr` and reached through [`Item::header`] / [`Item::header_mut`]. Java's `super`
//!   calls become calls on the header.
//! * Java's `instanceof` tests against the **abstract** classes become the family predicates
//!   [`Item::is_trace`], [`Item::is_drill_item`] and [`Item::is_obstacle_area`]. Getting these
//!   wrong is the main hazard of the flattening: `other instanceof ObstacleArea` in
//!   `BoardOutline.isObstacle` (BoardOutline.java:85) is true for **all four** area variants,
//!   not just [`ObstacleArea`].
//! * Java's reference identity (`other == this`, e.g. Trace.java:93) becomes an [`ItemId`]
//!   comparison. The board keys items by id, so two live items never share one.
//!
//! # Which `Item` methods live where
//!
//! | Java `Item` method | Here |
//! |---|---|
//! | field accessors, net ops, fixed state, tree bookkeeping | [`ItemHeader`] (`header.rs`) |
//! | per-subclass dispatch with a body that needs no board | [`Item`], this file |
//! | per-subclass dispatch whose body needs geometry | dispatches to a variant-struct method; the header-only structs carry `// added in Task N:` stubs that Tasks 6-8 replace |
//! | anything that queries the search tree, the item list or the components | `Board`, Task 11 — marked `// added in Task 11:` here |
//! | `printInfo`, `getHoverInfo`, `isSelectedByFilter`, `write` | not ported (GUI / serialization) |
//!
//! # The board-dependent methods, deferred to Task 11
//!
//! Each of these reads `Item.board` in Java, so it becomes a `Board` method taking an
//! [`ItemId`]: `getTileShape` goes through `board.searchTreeManager.getDefaultTree()`, the
//! contact family through `board.overlappingObjects`, the net family through
//! `board.rules.nets`, and `validate` through `board.searchTreeManager.validateEntries`. Each
//! marker below names the Java method, its lines, and the `Board` method that will replace it.
//!
//! added in Task 11: `clearanceViolations` (Item.java:363-469, plus `Via`'s override at Via.java:88-112 and the private `calculateClearanceBetweenTwoShapes` at Item.java:471-493) and `clearanceViolationCount` (Item.java:357-361) -> `Board::clearance_violations` / `Board::clearance_violation_count`.
//! added in Task 11: `componentName` (Item.java:346-355) -> `Board::item_component_name`; it reads `board.components.get(componentId).name`.
//! added in Task 11: `getAllContacts` (both overloads, Item.java:495-548), `isConnected` (Item.java:550-557) and `isConnectedOnLayer` (Item.java:559-566) -> `Board::all_contacts`, `Board::all_contacts_on_layer`, `Board::is_connected`, `Board::is_connected_on_layer`.
//! added in Task 11: `getNormalContacts` (Item.java:568-571 plus the `Trace`, `DrillItem` and `ConductionArea` overrides) and `normalContactPoint` (Item.java:573-589 plus its overrides) -> `Board::normal_contacts`, `Board::normal_contact_point`.
//! added in Task 11: `getConnectedSet` (Item.java:591-614, with the private `getConnectedSetRecu` at Item.java:616-635), `getUnconnectedSet` (Item.java:671-690) and `getConnectionItems` (both overloads, Item.java:692-781) -> `Board::connected_set`, `Board::unconnected_set`, `Board::connection_items`.
//! added in Task 11: `isTail` (Item.java:783-786 plus `Trace`/`Via`'s overrides), `isOverlap` (Item.java:637-640 plus `Trace`'s override), the package-private `isCycleRecu` (Item.java:642-669) and `isFanoutVia` (Item.java:1202-1239) -> `Board::is_tail`, `Board::is_overlap`, `Board::is_cycle_recu`, `Board::is_fanout_via`.
//! added in Task 11: `getRatsnestCorners` (Item.java:788-794 plus the `Trace`, `DrillItem` and `ConductionArea` overrides — the last is ConductionArea.java:367-377, `getArea().cornerApproxArr()` rounded) -> `Board::ratsnest_corners`.
//! added in Task 11: `hasIgnoredNets` (Item.java:1241-1255), `getAllNets` (Item.java:1271-1281) and `getAllNetNames` (Item.java:1283-1288) -> `Board::has_ignored_nets`, `Board::all_nets`, `Board::all_net_names`; all three resolve net numbers through `board.rules.nets`.
//! added in Task 11: `moveBy` (Item.java:300-311 plus `DrillItem`'s override at DrillItem.java:95-145) -> `Board::move_item_by`; it saves for undo and re-inserts the item into the search trees.
//! added in Task 11: `validate` (Item.java:796-807 plus `Trace`'s override at Trace.java:447-461) -> `Board::validate_item`; both halves need the board.
//!
//! # Not ported
//!
//! not ported: `Item.getHoverInfo` (Item.java:1067-1070), `getConnectableItemHoverInfo` (Item.java:1072-1075), `getNetHoverInfo` (Item.java:1077-1090), the six `printXInfo` helpers (Item.java:1092-1172) and the abstract `Item.printInfo` (Item.java:1058) with all nine of its subclass overrides — GUI hover text and `ItemInfoPrinter` output, localized through `TextManager`; this crate is headless.
//!
//! not ported: `Item.isSelectedByFilter` (Item.java:982-983) and the protected `isSelectedByFixedFilter` (Item.java:985-994), plus all nine subclass overrides — they take an `ItemSelectionFilter`, which is interactive-GUI selection state.
//!
//! not ported: `Item.write(ObjectOutputStream)` (Item.java:277-278) and its overrides — Java
//! serialization, which `global-constraints.md` excludes.
//!
//! not ported: the whole of `PrintableShape` (`board/model/items/PrintableShape.java`) and its three nested classes `PrintableShape.Circle` (:26-51), `PrintableShape.Rectangle` (:54-77) and `PrintableShape.Polygon` (:79-98) — each is a `Locale` plus a `toString()` that renders a shape into localized user-coordinate text through `TextManager`. Its only consumers are `board/state/CoordinateTransform.java` and the two `gui/windows/board` object-info windows; nothing in the model or the router reads one.

pub mod area;
pub mod drill;
pub mod header;
pub mod trace;

use std::cmp::Ordering;

use fr_geometry::{FloatPoint, IntBox, IntPoint, PolylineError, TileShape, Vector};

use crate::datastructures::LeafId;
use crate::ids::{ItemId, TreeId};
use crate::rules::{BoardRules, Nets};
use crate::structure::FixedState;

pub use area::{
    ComponentObstacleArea, ComponentOutline, ConductionArea, ObstacleArea, ObstacleAreaData,
    ViaObstacleArea,
};
pub use drill::{DrillItemData, ItemCtx, Pin, TraceExitRestriction, Via};
pub use header::{AutorouteInfo, ItemHeader, TreeEntries};
pub use trace::PolylineTrace;

pub use crate::structure::board_outline::BoardOutline;

/// Port of `BoardItemType` (`board/model/items/BoardItemType.java`): the neutral semantic
/// category of a board item, independent of any rendering API.
// renamed: BoardItemType -> ItemKind, and Item.getBoardItemType -> Item::kind (Task 5 brief).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Trace,
    Pin,
    Via,
    ObstacleArea,
    ViaObstacleArea,
    ConductionArea,
    ComponentObstacleArea,
    BoardOutline,
    ComponentOutline,
    /// BoardItemType.java:19. Unreachable from [`Item::kind`]: `Item.getBoardItemType`
    /// (Item.java:117-146) ends in `return BoardItemType.OTHER` only for an `Item` subclass
    /// that is none of the nine, and the enum here *is* the closed set of subclasses. Kept
    /// because the Java enum is public and readers of a parsed value may still see it.
    Other,
}

// ---------------------------------------------------------------------------------------------
// The nine variant structs.
//
// Task 5 creates all nine at once, each holding only the state its `Item`-level dispatch needs
// right now: the shared `hdr`, plus the two flags that `isObstacle` reads
// (`ConductionArea.isObstacle`, `Via.attachAllowed`). Tasks 6-8 add the geometry fields and
// replace the `// added in Task N:` method stubs below; the enum's dispatch surface does not
// change when they do.
// ---------------------------------------------------------------------------------------------

/// Java's `HALF_WIDTH` for board outlines (BoardOutline.java:27).
pub const BOARD_OUTLINE_HALF_WIDTH: i32 = 100;

/// Port of `Item` (`board/model/items/Item.java`): anything that can sit on a board.
///
/// Variant order is the order `Item.getBoardItemType` tests in (Item.java:117-146) with the
/// abstract-class fallbacks folded in; it carries no semantics of its own, because [`Item`] has
/// no derived `Ord` (see [`Item::compare_to`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Trace(PolylineTrace),
    Via(Via),
    Pin(Pin),
    ObstacleArea(ObstacleArea),
    ConductionArea(ConductionArea),
    ViaObstacleArea(ViaObstacleArea),
    ComponentObstacleArea(ComponentObstacleArea),
    ComponentOutline(ComponentOutline),
    BoardOutline(BoardOutline),
}

impl Item {
    // -- the base-class state (Item.java:41-67) ------------------------------------------------

    /// The [`ItemHeader`] embedded in this variant. This is what Java gets for free from
    /// inheritance.
    pub fn header(&self) -> &ItemHeader {
        match self {
            Item::Trace(i) => &i.hdr,
            Item::Via(i) => &i.hdr,
            Item::Pin(i) => &i.hdr,
            Item::ObstacleArea(i) => &i.hdr,
            Item::ConductionArea(i) => &i.hdr,
            Item::ViaObstacleArea(i) => &i.hdr,
            Item::ComponentObstacleArea(i) => &i.hdr,
            Item::ComponentOutline(i) => &i.hdr,
            Item::BoardOutline(i) => &i.hdr,
        }
    }

    /// Mutable counterpart of [`Self::header`].
    pub fn header_mut(&mut self) -> &mut ItemHeader {
        match self {
            Item::Trace(i) => &mut i.hdr,
            Item::Via(i) => &mut i.hdr,
            Item::Pin(i) => &mut i.hdr,
            Item::ObstacleArea(i) => &mut i.hdr,
            Item::ConductionArea(i) => &mut i.hdr,
            Item::ViaObstacleArea(i) => &mut i.hdr,
            Item::ComponentObstacleArea(i) => &mut i.hdr,
            Item::ComponentOutline(i) => &mut i.hdr,
            Item::BoardOutline(i) => &mut i.hdr,
        }
    }

    /// Port of `Item.getId` (Item.java:106-109).
    // renamed: Item.getId -> Item::id.
    pub fn id(&self) -> ItemId {
        self.header().id()
    }

    /// Port of `Item.compareTo` (Item.java:93-103).
    ///
    /// Exposed as an inherent method rather than an `Ord` impl for the reason `Net`, `Package`
    /// and `PartPin` do the same: the port's `PartialEq` on [`Item`] is structural (every
    /// field), while Java's comparator looks only at the id, so an `Ord` impl would break the
    /// `a == b <=> a.cmp(b) == Equal` contract for two different items that happen to share an
    /// id. The ordering that the search tree actually needs lives on
    /// [`crate::ids::TreeObject`].
    //
    // Java bug: the subtraction is the wrong way round. `result = item.id - id` (Item.java:98)
    // with `item` the *argument* means `this.compareTo(other)` is positive when `other.id` is
    // the larger — so a `TreeSet<Item>` iterates by **descending** id. Java's own
    // `ShapeTree.Leaf.compareTo` (ShapeTree.java:216-223) delegates to it, so every search-tree
    // result set is in descending item-id order too. Reproduced here and in `TreeObject`'s
    // `Ord`; see docs/java-quirks.md.
    pub fn compare_to(&self, other: &Item) -> Ordering {
        other.id().cmp(&self.id())
    }

    /// Port of `Item.getBoardItemType` (Item.java:111-146).
    // renamed: Item.getBoardItemType -> Item::kind.
    pub fn kind(&self) -> ItemKind {
        match self {
            Item::Trace(_) => ItemKind::Trace,
            Item::Via(_) => ItemKind::Via,
            Item::Pin(_) => ItemKind::Pin,
            Item::ObstacleArea(_) => ItemKind::ObstacleArea,
            Item::ConductionArea(_) => ItemKind::ConductionArea,
            Item::ViaObstacleArea(_) => ItemKind::ViaObstacleArea,
            Item::ComponentObstacleArea(_) => ItemKind::ComponentObstacleArea,
            Item::ComponentOutline(_) => ItemKind::ComponentOutline,
            Item::BoardOutline(_) => ItemKind::BoardOutline,
        }
    }

    // -- Java's `instanceof` tests against the abstract classes --------------------------------

    /// `this instanceof Trace` (Trace.java:28 — abstract, extended only by `PolylineTrace`).
    ///
    /// Not a Java method: Java writes the `instanceof` inline, at Item.java:124,383,715 and in
    /// four `isObstacle` overrides.
    pub fn is_trace(&self) -> bool {
        matches!(self, Item::Trace(_))
    }

    /// `this instanceof DrillItem` (DrillItem.java:25 — abstract, extended by `Via` and `Pin`).
    ///
    /// Not a Java method; see [`Self::is_trace`].
    pub fn is_drill_item(&self) -> bool {
        matches!(self, Item::Via(_) | Item::Pin(_))
    }

    /// `this instanceof ObstacleArea` (ObstacleArea.java:25) — true for **all four** area
    /// variants, because `ConductionArea`, `ViaObstacleArea` and `ComponentObstacleArea` all
    /// extend it. `Pin.isObstacle` (Pin.java:354) and `BoardOutline.isObstacle`
    /// (BoardOutline.java:85) both depend on that.
    ///
    /// Not a Java method; see [`Self::is_trace`].
    pub fn is_obstacle_area(&self) -> bool {
        matches!(
            self,
            Item::ObstacleArea(_)
                | Item::ConductionArea(_)
                | Item::ViaObstacleArea(_)
                | Item::ComponentObstacleArea(_)
        )
    }

    // -- net membership (Item.java:148-189, 873-915, 1174-1200) --------------------------------

    /// The net numbers this item belongs to — Java's public `int[] netNumbers`
    /// (Item.java:52-53).
    pub fn net_nos(&self) -> &[i32] {
        &self.header().net_nos
    }

    /// Port of `Item.netCount` (Item.java:873-876).
    pub fn net_count(&self) -> usize {
        self.header().net_count()
    }

    /// Port of `Item.getNetNumber` (Item.java:878-881).
    pub fn get_net_number(&self, no: usize) -> i32 {
        self.header().get_net_number(no)
    }

    /// Port of `Item.containsNet` (Item.java:148-159).
    pub fn contains_net(&self, net_number: i32) -> bool {
        self.header().contains_net(net_number)
    }

    /// Port of `Item.sharesNet(Item)` (Item.java:174-177).
    pub fn shares_net(&self, other: &Item) -> bool {
        self.shares_net_no(other.net_nos())
    }

    /// Port of `Item.sharesNetNo(int[])` (Item.java:179-189).
    pub fn shares_net_no(&self, net_nos: &[i32]) -> bool {
        self.header().shares_net_no(net_nos)
    }

    /// Port of `Item.netsEqual(Item)` (Item.java:1184-1187).
    pub fn nets_equal(&self, other: &Item) -> bool {
        self.header().nets_equal(other.net_nos())
    }

    /// Port of `Item.netsEqual(int[])` (Item.java:1189-1200).
    pub fn nets_equal_to(&self, net_nos: &[i32]) -> bool {
        self.header().nets_equal(net_nos)
    }

    /// Port of `Item.netsNormal` (Item.java:1174-1182).
    pub fn nets_normal(&self) -> bool {
        self.header().nets_normal()
    }

    /// Port of `Item.assignNetNo` (Item.java:957-980); see [`ItemHeader::assign_net_no`] for the
    /// two reproduced Java bugs.
    pub fn assign_net_no(&mut self, net_number: i32, nets: &Nets) {
        self.header_mut().assign_net_no(net_number, nets);
    }

    /// Port of `Item.removeFromNet` (Item.java:888-915).
    pub fn remove_from_net(&mut self, net_number: i32) -> bool {
        self.header_mut().remove_from_net(net_number)
    }

    /// Port of `Item.isConnectable` (Item.java:868-871).
    pub fn is_connectable(&self) -> bool {
        self.as_connectable().is_some() && self.net_count() > 0
    }

    /// `Item`'s half of the `Connectable` dispatch: `Some` exactly for the four classes that
    /// implement the interface — `Trace` (Trace.java:28), `DrillItem` (DrillItem.java:25, so
    /// both `Via` and `Pin`) and `ConductionArea` (ConductionArea.java:25).
    ///
    /// Not a Java method: Java writes `this instanceof Connectable` and casts.
    pub fn as_connectable(&self) -> Option<ConnectableRef<'_>> {
        match self {
            Item::Trace(i) => Some(ConnectableRef::Trace(i)),
            Item::Via(i) => Some(ConnectableRef::Via(i)),
            Item::Pin(i) => Some(ConnectableRef::Pin(i)),
            Item::ConductionArea(i) => Some(ConnectableRef::ConductionArea(i)),
            _ => None,
        }
    }

    // -- fixed state (Item.java:816-861) --------------------------------------------------------

    /// Port of `Item.getFixedState` (Item.java:841-844).
    pub fn get_fixed_state(&self) -> FixedState {
        self.header().get_fixed_state()
    }

    /// Port of `Item.setFixedState` (Item.java:846-849).
    pub fn set_fixed_state(&mut self, fixed_state: FixedState) {
        self.header_mut().set_fixed_state(fixed_state);
    }

    /// Port of `Item.unfix` (Item.java:856-861).
    pub fn unfix(&mut self) {
        self.header_mut().unfix();
    }

    /// Port of `Item.isUserFixed` (Item.java:816-819). No subclass overrides it.
    pub fn is_user_fixed(&self) -> bool {
        self.header().is_user_fixed()
    }

    /// Port of `Item.isShoveFixed` (Item.java:834-839) **and** its one override,
    /// `Trace.isShoveFixed` (Trace.java:236-254), which additionally reports true when any of
    /// the trace's nets belongs to a shove-fixed net class.
    ///
    /// `rules` replaces Java's `this.board.rules` (Trace.java:243). Java's `nets.get(n)` returns
    /// `null` for a number past the end of the net list and then throws a
    /// `NullPointerException` at `.getNetClass()`; the `expect` reproduces that crash.
    pub fn is_shove_fixed(&self, rules: &BoardRules) -> bool {
        if self.header().is_shove_fixed() {
            return true;
        }
        if !self.is_trace() {
            return false;
        }
        // Trace.java:244-250.
        self.net_nos()
            .iter()
            .filter(|n| Nets::is_normal_net_number(**n))
            .any(|n| {
                let net = rules.nets.get(*n).expect(
                    "Trace.isShoveFixed: nets.get(currentNetNumber) is null — Java throws a \
                     NullPointerException here too (Trace.java:246)",
                );
                rules.net_classes.get(net.get_net_class()).is_shove_fixed()
            })
    }

    /// Port of `Item.isDeletionForbidden` (Item.java:821-832): items of a component and
    /// user-fixed items may not be deleted, and neither may a power plane — a conduction area on
    /// a non-signal layer.
    ///
    /// `rules` replaces Java's `this.board.layerStructure` (Item.java:829); the board's stack and
    /// [`BoardRules::layer_structure`] are the same stack. Java indexes `layers[area.getLayer()]`
    /// directly and throws out of range; the port panics identically.
    pub fn is_deletion_forbidden(&self, rules: &BoardRules) -> bool {
        if self.header().get_component_id() > 0 || self.is_user_fixed() {
            return true;
        }
        match self {
            Item::ConductionArea(area) => {
                !rules.layer_structure().layers[area.get_layer()].is_signal
            }
            _ => false,
        }
    }

    // -- clearance class, component, board membership -------------------------------------------

    /// Port of `Item.clearanceClassIndex` (Item.java:917-923).
    // renamed: Item.clearanceClassIndex -> Item::clearance_class.
    pub fn clearance_class(&self) -> usize {
        self.header().clearance_class()
    }

    /// Port of `Item.setClearanceClassIndex` (Item.java:925-935).
    ///
    /// Java's sibling `changeClearanceClassIndex` (Item.java:937-950) does the same thing and
    /// then re-inserts the item into the search tree.
    // added in Task 11: `Board::change_clearance_class_index(ItemId, usize)` — the search-tree
    // half of `Item.changeClearanceClassIndex` (Item.java:944-949: `clearDerivedData`, then
    // `searchTreeManager.remove`/`insert` when clearance compensation is on).
    pub fn set_clearance_class(&mut self, index: usize, rules: &BoardRules) {
        self.header_mut().set_clearance_class(index, rules);
    }

    /// Port of `Item.getComponentId` (Item.java:883-886).
    // renamed: Item.getComponentId -> Item::component_id (the header keeps the Java name).
    pub fn component_id(&self) -> i32 {
        self.header().get_component_id()
    }

    /// Port of `Item.assignComponentId` (Item.java:952-955).
    pub fn assign_component_id(&mut self, id: i32) {
        self.header_mut().assign_component_id(id);
    }

    /// Port of `Item.isOnTheBoard` (Item.java:243-246).
    pub fn is_on_the_board(&self) -> bool {
        self.header().is_on_the_board()
    }

    /// Port of `Item.setOnTheBoard` (Item.java:248-250).
    pub fn set_on_the_board(&mut self, value: bool) {
        self.header_mut().set_on_the_board(value);
    }

    // -- obstacle relations (Item.java:161-172 + eight overrides) --------------------------------

    /// Port of the abstract `Item.isObstacle(Item)` (Item.java:166-167) and all eight of its
    /// overrides: whether this item may not overlap `other`.
    ///
    /// The Java bodies, in this file's variant order:
    ///
    /// * `Trace` (Trace.java:91-102)
    /// * `Via` (Via.java:151-166)
    /// * `Pin` (Pin.java:352-366)
    /// * `ObstacleArea` (ObstacleArea.java:174-180)
    /// * `ConductionArea` (ConductionArea.java:379-386) — delegates to `super`, i.e. the
    ///   `ObstacleArea` body, when the area is an obstacle
    /// * `ViaObstacleArea` (ViaObstacleArea.java:91-97)
    /// * `ComponentObstacleArea` (ComponentObstacleArea.java:63-68)
    /// * `ComponentOutline` (ComponentOutline.java:119-122)
    /// * `BoardOutline` (BoardOutline.java:83-86)
    ///
    /// The Task 5 brief asks for a `&BoardRules` parameter "because `is_obstacle` needs
    /// `rules.ignore_conduction`". Java does not: no `isObstacle` body reads
    /// `BoardRules.ignoreConduction`, and grep finds the flag only in `BoardRules`
    /// (BoardRules.java:34,371,376), `BasicBoard.unfillConductionAreas`
    /// (BasicBoard.java:1427), `RoutingBoard.changeConductionIsObstacle`
    /// (RoutingBoard.java:1254,1273) and the GUI. The coupling is indirect:
    /// `changeConductionIsObstacle(value)` walks the item list and pushes `value` into every
    /// signal-layer conduction area's own `isObstacle` field, then stores `!value` in
    /// `rules.ignoreConduction`. So the rules flag is a *cache of the last board-wide setting*,
    /// and the per-item field is what `isObstacle` actually reads. Java wins over the brief;
    /// `Board::change_conduction_is_obstacle` (Task 11) owns the flag.
    pub fn is_obstacle(&self, other: &Item, ctx: &ItemCtx<'_>) -> bool {
        match self {
            Item::Trace(_) => {
                // Trace.java:93-96.
                if other.id() == self.id()
                    || matches!(
                        other,
                        Item::ViaObstacleArea(_) | Item::ComponentObstacleArea(_)
                    )
                {
                    return false;
                }
                // Trace.java:98-100.
                if let Item::ConductionArea(area) = other
                    && !area.get_is_obstacle()
                {
                    return false;
                }
                // Trace.java:101.
                !other.shares_net(self)
            }
            Item::Via(via) => {
                // Via.java:153-155.
                if other.id() == self.id() || matches!(other, Item::ComponentObstacleArea(_)) {
                    return false;
                }
                // Via.java:156-158.
                if let Item::ConductionArea(area) = other
                    && !area.get_is_obstacle()
                {
                    return false;
                }
                // Via.java:159-161.
                if !other.shares_net(self) {
                    return true;
                }
                // Via.java:162-164.
                if other.is_trace() {
                    return false;
                }
                // Via.java:165.
                match other {
                    Item::Pin(pin) => !via.attach_allowed || !pin.drill_allowed(ctx),
                    _ => true,
                }
            }
            Item::Pin(pin) => {
                // Pin.java:354-356. `instanceof ObstacleArea` covers all four area variants.
                if other.id() == self.id() || other.is_obstacle_area() {
                    return false;
                }
                // Pin.java:357-359.
                if !other.shares_net(self) {
                    return true;
                }
                // Pin.java:360-362.
                if other.is_trace() {
                    return false;
                }
                // Pin.java:365: same-net vias must be allowed to contact SMD pins during fanout.
                !pin.drill_allowed(ctx) || !matches!(other, Item::Via(_))
            }
            Item::ObstacleArea(_) => obstacle_area_is_obstacle(self, other),
            Item::ConductionArea(area) => {
                // ConductionArea.java:381-385.
                if area.get_is_obstacle() {
                    obstacle_area_is_obstacle(self, other)
                } else {
                    false
                }
            }
            Item::ViaObstacleArea(_) => {
                // ViaObstacleArea.java:93-97.
                if other.shares_net(self) {
                    return false;
                }
                matches!(other, Item::Via(_))
            }
            Item::ComponentObstacleArea(_) => {
                // ComponentObstacleArea.java:65-67.
                other.id() != self.id()
                    && matches!(other, Item::ComponentObstacleArea(_))
                    && other.component_id() != self.component_id()
            }
            // ComponentOutline.java:120-122.
            Item::ComponentOutline(_) => false,
            // BoardOutline.java:85: `instanceof ObstacleArea` covers all four area variants.
            Item::BoardOutline(_) => {
                !(matches!(other, Item::BoardOutline(_)) || other.is_obstacle_area())
            }
        }
    }

    /// Port of `Item.isObstacle(int)` (Item.java:161-164), the `SearchTreeObject` method: this
    /// item is an obstacle to anything on `net_number` unless it is on that net too. No subclass
    /// overrides it.
    // renamed: Item.isObstacle(int) -> Item::is_obstacle_for_net, to keep it apart from the
    // `isObstacle(Item)` overload above, which Rust cannot overload on.
    pub fn is_obstacle_for_net(&self, net_number: i32) -> bool {
        !self.contains_net(net_number)
    }

    /// Port of `Item.isTraceObstacle(int)` (Item.java:169-172) and its three overrides:
    /// `ConductionArea` (ConductionArea.java:397-400), `ViaObstacleArea`
    /// (ViaObstacleArea.java:99-102) and `ComponentObstacleArea`
    /// (ComponentObstacleArea.java:70-73).
    pub fn is_trace_obstacle(&self, net_number: i32) -> bool {
        match self {
            Item::ConductionArea(area) => area.get_is_obstacle() && !self.contains_net(net_number),
            Item::ViaObstacleArea(_) | Item::ComponentObstacleArea(_) => false,
            _ => !self.contains_net(net_number),
        }
    }

    /// Port of `Item.isDrillable(int)` (Item.java:851-854) and its two overrides:
    /// `Trace` (Trace.java:221-225) and `ConductionArea` (ConductionArea.java:402-405).
    pub fn is_drillable(&self, net_number: i32) -> bool {
        match self {
            Item::Trace(_) => self.contains_net(net_number),
            Item::ConductionArea(area) => !area.get_is_obstacle() || self.contains_net(net_number),
            _ => false,
        }
    }

    /// Port of `Item.isRoutable` (Item.java:863-866) and its two overrides, `Trace.isRoutable`
    /// (Trace.java:205-209) and `Via.isRoutable` (Via.java:146-150), which are textually
    /// identical.
    pub fn is_routable(&self) -> bool {
        match self {
            Item::Trace(_) | Item::Via(_) => !self.is_user_fixed() && self.net_count() > 0,
            _ => false,
        }
    }

    // -- layers (Item.java:268-275, 313-344, 809-814) ------------------------------------------

    /// Port of the abstract `Item.firstLayer` (Item.java:271-272) and its overrides.
    pub fn first_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Trace(i) => i.first_layer(),
            Item::Via(i) => i.first_layer(ctx),
            Item::Pin(i) => i.first_layer(ctx),
            Item::ObstacleArea(i) => i.get_layer(),
            Item::ConductionArea(i) => i.get_layer(),
            Item::ViaObstacleArea(i) => i.get_layer(),
            Item::ComponentObstacleArea(i) => i.get_layer(),
            Item::ComponentOutline(i) => i.get_layer(ctx),
            // BoardOutline.java:97-100.
            Item::BoardOutline(_) => 0,
        }
    }

    /// Port of the abstract `Item.lastLayer` (Item.java:274-275) and its overrides.
    pub fn last_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Trace(i) => i.last_layer(),
            Item::Via(i) => i.last_layer(ctx),
            Item::Pin(i) => i.last_layer(ctx),
            Item::ObstacleArea(i) => i.get_layer(),
            Item::ConductionArea(i) => i.get_layer(),
            Item::ViaObstacleArea(i) => i.get_layer(),
            Item::ComponentObstacleArea(i) => i.get_layer(),
            Item::ComponentOutline(i) => i.get_layer(ctx),
            Item::BoardOutline(i) => i.last_layer(ctx),
        }
    }

    /// Port of the abstract `Item.isOnLayer` (Item.java:268-269) and its overrides.
    pub fn is_on_layer(&self, layer: usize, ctx: &ItemCtx<'_>) -> bool {
        match self {
            // PolylineTrace.java:81-84, ObstacleArea.java:150-153, ComponentOutline.java:114-117
            // are all `getLayer() == layer`; DrillItem.java:156-159 is a range test.
            Item::Via(i) => i.is_on_layer(layer, ctx),
            Item::Pin(i) => i.is_on_layer(layer, ctx),
            // BoardOutline.java:107-110 is unconditionally true.
            Item::BoardOutline(_) => true,
            _ => self.first_layer(ctx) == layer,
        }
    }

    /// Port of the abstract `Item.shapeLayer(int)` (Item.java:809-814) and its overrides:
    /// `Trace` (Trace.java:332-335), `DrillItem` (DrillItem.java:147-154), `ObstacleArea`
    /// (ObstacleArea.java:265-268), `ComponentOutline` (ComponentOutline.java:124-127) and
    /// `BoardOutline` (BoardOutline.java:68-81).
    pub fn shape_layer(&self, index: usize, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Via(i) => i.shape_layer(index, ctx),
            Item::Pin(i) => i.shape_layer(index, ctx),
            Item::BoardOutline(i) => i.shape_layer(index, ctx),
            // Every other override ignores `index` and answers the item's single layer.
            _ => self.first_layer(ctx),
        }
    }

    /// Port of `Item.sharesLayer` (Item.java:313-318).
    pub fn shares_layer(&self, other: &Item, ctx: &ItemCtx<'_>) -> bool {
        self.first_layer(ctx).max(other.first_layer(ctx))
            <= self.last_layer(ctx).min(other.last_layer(ctx))
    }

    /// Port of `Item.firstCommonLayer` (Item.java:320-331). Java's `-1` is `None`.
    pub fn first_common_layer(&self, other: &Item, ctx: &ItemCtx<'_>) -> Option<usize> {
        let max_first = self.first_layer(ctx).max(other.first_layer(ctx));
        let min_last = self.last_layer(ctx).min(other.last_layer(ctx));
        (max_first <= min_last).then_some(max_first)
    }

    /// Port of `Item.lastCommonLayer` (Item.java:333-344). Java's `-1` is `None`.
    pub fn last_common_layer(&self, other: &Item, ctx: &ItemCtx<'_>) -> Option<usize> {
        let max_first = self.first_layer(ctx).max(other.first_layer(ctx));
        let min_last = self.last_layer(ctx).min(other.last_layer(ctx));
        (max_first <= min_last).then_some(min_last)
    }

    // -- geometry (Item.java:191-241, 277-298) --------------------------------------------------

    /// Port of the abstract `Item.boundingBox` (Item.java:297-298) and its overrides.
    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        match self {
            Item::Trace(i) => i.bounding_box(),
            Item::Via(i) => i.bounding_box(ctx),
            Item::Pin(i) => i.bounding_box(ctx),
            Item::ObstacleArea(i) => i.bounding_box(ctx),
            Item::ConductionArea(i) => i.bounding_box(ctx),
            Item::ViaObstacleArea(i) => i.bounding_box(ctx),
            Item::ComponentObstacleArea(i) => i.bounding_box(ctx),
            Item::ComponentOutline(i) => i.bounding_box(ctx),
            Item::BoardOutline(i) => i.bounding_box(),
        }
    }

    /// Port of the abstract `Item.tileShapeCount` (Item.java:191-192) and its overrides.
    pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        match self {
            Item::Trace(i) => i.tile_shape_count(),
            Item::Via(i) => i.tile_shape_count(ctx),
            Item::Pin(i) => i.tile_shape_count(ctx),
            Item::ObstacleArea(i) => i.tile_shape_count(ctx),
            Item::ConductionArea(i) => i.tile_shape_count(ctx),
            Item::ViaObstacleArea(i) => i.tile_shape_count(ctx),
            Item::ComponentObstacleArea(i) => i.tile_shape_count(ctx),
            Item::ComponentOutline(i) => i.tile_shape_count(),
            Item::BoardOutline(i) => i.tile_shape_count(ctx),
        }
    }

    /// Port of `Item.getTileShape(int)` (Item.java:194-201) and its one override,
    /// `ObstacleArea.getTileShape` (ObstacleArea.java:197-205), which the three area subclasses
    /// inherit.
    ///
    /// `default_tree` replaces Java's `this.board.searchTreeManager.getDefaultTree()`
    /// (Item.java:200); the `ObstacleArea` override ignores it and splits its own area instead.
    /// Java's `board == null` warning path (Item.java:196-199) returns `null`, which is `None`.
    pub fn get_tile_shape(
        &self,
        default_tree: TreeId,
        index: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        match self {
            Item::ObstacleArea(i) => i.get_tile_shape(index, ctx),
            Item::ConductionArea(i) => i.get_tile_shape(index, ctx),
            Item::ViaObstacleArea(i) => i.get_tile_shape(index, ctx),
            Item::ComponentObstacleArea(i) => i.get_tile_shape(index, ctx),
            _ => self.get_tree_shape(default_tree, index).cloned(),
        }
    }

    /// Port of `Item.treeShapeCount(ShapeTree)` (Item.java:203-210), reading the cache only.
    ///
    /// Java's `board == null` early return of 0 (Item.java:205-207) is the same answer this
    /// gives for an item with nothing cached for `tree`.
    // added in Task 10: the lazy fill (Item.java:208 -> Item.java:228-238 ->
    // `calculateTreeShapes(searchTree)`), which needs the `ShapeSearchTree` itself.
    pub fn tree_shape_count(&self, tree: TreeId) -> usize {
        self.header()
            .get_precalculated_tree_shapes(tree)
            .map_or(0, <[TileShape]>::len)
    }

    /// Port of `Item.getTreeShape(ShapeTree, int)` (Item.java:212-226), reading the cache only.
    ///
    /// Java's two `null` returns (no board, index out of range after a recompute) are `None`.
    // added in Task 10: the `clearDerivedData()` + recompute retry at Item.java:218-224.
    pub fn get_tree_shape(&self, tree: TreeId, index: usize) -> Option<&TileShape> {
        self.header()
            .get_precalculated_tree_shapes(tree)
            .and_then(|shapes| shapes.get(index))
    }

    /// Port of `Item.setSearchTreeEntries` (Item.java:996-1006).
    // renamed: Item.setSearchTreeEntries -> Item::set_tree_entries (Task 5 brief naming); the
    // header keeps the same name.
    pub fn set_tree_entries(&mut self, tree: TreeId, leaves: Vec<Option<LeafId>>) {
        self.header_mut().set_tree_entries(tree, leaves);
    }

    /// Port of `Item.getSearchTreeEntries` (Item.java:1008-1017).
    pub fn get_search_tree_entries(&self, tree: TreeId) -> Option<&[Option<LeafId>]> {
        self.header().get_tree_entries(tree)
    }

    /// Port of `Item.setPrecalculatedTreeShapes` (Item.java:1019-1031).
    pub fn set_precalculated_tree_shapes(&mut self, tree: TreeId, shapes: Vec<TileShape>) {
        self.header_mut()
            .set_precalculated_tree_shapes(tree, shapes);
    }

    /// Port of `Item.clearSearchTreeEntries` (Item.java:1033-1036), which drops the leaves and
    /// the cached shapes of **every** tree at once.
    ///
    /// Java has no per-tree clear; the Task 5 brief's `clear_tree_entries(TreeId)` would be a
    /// new method with no Java counterpart, so it is not added.
    // renamed: Item.clearSearchTreeEntries -> Item::clear_tree_entries (brief naming).
    pub fn clear_tree_entries(&mut self) {
        self.header_mut().clear_search_tree_entries();
    }

    /// Port of `Item.getAutorouteInfo` (Item.java:1038-1044).
    pub fn get_autoroute_info(&mut self) -> &mut AutorouteInfo {
        self.header_mut().get_autoroute_info()
    }

    /// Port of `Item.getAutorouteInfoPur` (Item.java:1046-1049).
    pub fn get_autoroute_info_pur(&self) -> Option<&AutorouteInfo> {
        self.header().get_autoroute_info_pur()
    }

    /// Port of `Item.clearAutorouteInfo` (Item.java:1051-1054) and its one override,
    /// `Via.clearAutorouteInfo` (Via.java:226-230), which also drops the via's cached
    /// `autorouteDrillInfo`.
    pub fn clear_autoroute_info(&mut self) {
        match self {
            // Via.java:226-230 also drops `autorouteDrillInfo`.
            Item::Via(i) => i.clear_autoroute_info(),
            _ => self.header_mut().clear_autoroute_info(),
        }
    }

    /// Port of `Item.clearDerivedData` (Item.java:1056-1065) and the six overrides that each
    /// drop a subclass cache and then call `super`: `Via` (Via.java:219-224), `Pin`
    /// (Pin.java:385-390), `DrillItem` (DrillItem.java:390-395), `ObstacleArea`
    /// (ObstacleArea.java:328-332), `ConductionArea` (ConductionArea.java:76-82) and
    /// `ComponentOutline` (ComponentOutline.java:218-221).
    pub fn clear_derived_data(&mut self) {
        match self {
            // Via.java:219-224 and Pin.java:385-389 each drop a shape cache and the two
            // `DrillItem` layer memos before calling `super`.
            Item::Via(i) => i.clear_derived_data(),
            Item::Pin(i) => i.clear_derived_data(),
            // ObstacleArea.java:328-332 (which ConductionArea.java:76-81 extends with the
            // not-ported fill cache) drops `precalculatedAbsoluteArea` and then calls `super`.
            Item::ObstacleArea(i) => i.clear_derived_data(),
            Item::ConductionArea(i) => i.clear_derived_data(),
            Item::ViaObstacleArea(i) => i.clear_derived_data(),
            Item::ComponentObstacleArea(i) => i.clear_derived_data(),
            // Java quirk: ComponentOutline.java:218-221 does **not** call `super`, so the
            // header's caches survive. See docs/java-quirks.md.
            Item::ComponentOutline(i) => i.clear_derived_data(),
            _ => self.header_mut().clear_derived_data(),
        }
    }

    // -- transforms (Item.java:277-295) ---------------------------------------------------------

    /// Port of the abstract `Item.translateBy` (Item.java:280-281) and its overrides.
    ///
    /// `Result` because one override can fail: `PolylineTrace.translateBy`
    /// (PolylineTrace.java:143-147) re-runs the normalising `Polyline(Line[])` constructor,
    /// whose one crash Plan 1 ruling 12 turned into [`PolylineError`]. The other eight
    /// overrides always succeed.
    pub fn translate_by(&mut self, vector: &Vector) -> Result<(), PolylineError> {
        match self {
            Item::Trace(i) => return i.translate_by(vector),
            Item::Via(i) => i.translate_by(vector),
            Item::Pin(i) => i.translate_by(vector),
            Item::ObstacleArea(i) => i.translate_by(vector),
            Item::ConductionArea(i) => i.translate_by(vector),
            Item::ViaObstacleArea(i) => i.translate_by(vector),
            Item::ComponentObstacleArea(i) => i.translate_by(vector),
            Item::ComponentOutline(i) => i.translate_by(vector),
            Item::BoardOutline(i) => i.translate_by(vector),
        }
        Ok(())
    }

    /// Port of the abstract `Item.turn90Degree` (Item.java:283-286) and its overrides.
    ///
    /// `Result` for the reason [`Item::translate_by`] is.
    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) -> Result<(), PolylineError> {
        match self {
            Item::Trace(i) => return i.turn_90_degree(factor, pole),
            Item::Via(i) => i.turn_90_degree(factor, pole),
            Item::Pin(i) => i.turn_90_degree(factor, pole),
            Item::ObstacleArea(i) => i.turn_90_degree(factor, pole),
            Item::ConductionArea(i) => i.turn_90_degree(factor, pole),
            Item::ViaObstacleArea(i) => i.turn_90_degree(factor, pole),
            Item::ComponentObstacleArea(i) => i.turn_90_degree(factor, pole),
            Item::ComponentOutline(i) => i.turn_90_degree(factor, pole),
            Item::BoardOutline(i) => i.turn_90_degree(factor, pole),
        }
        Ok(())
    }

    /// Port of the abstract `Item.rotateApprox` (Item.java:288-289) and its overrides.
    ///
    /// `ctx` is Java's `this.board.components.getFlipStyleRotateFirst()`, which
    /// `ObstacleArea.rotateApprox` (ObstacleArea.java:230) and `ComponentOutline.rotateApprox`
    /// (ComponentOutline.java:161) both branch on.
    pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint, ctx: &ItemCtx<'_>) {
        match self {
            Item::Trace(i) => i.rotate_approx(angle_in_degree, pole),
            Item::Via(i) => i.rotate_approx(angle_in_degree, pole),
            Item::Pin(i) => i.rotate_approx(angle_in_degree, pole),
            Item::ObstacleArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ConductionArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ViaObstacleArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ComponentObstacleArea(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::ComponentOutline(i) => i.rotate_approx(angle_in_degree, pole, ctx),
            Item::BoardOutline(i) => i.rotate_approx(angle_in_degree, pole),
        }
    }

    /// Port of the abstract `Item.changePlacementSide` (Item.java:291-295) and its overrides.
    ///
    /// `Result` for the reason [`Item::translate_by`] is —
    /// `PolylineTrace.changePlacementSide` (PolylineTrace.java:160-168) mirrors through the same
    /// constructor.
    pub fn change_placement_side(
        &mut self,
        pole: &IntPoint,
        ctx: &ItemCtx<'_>,
    ) -> Result<(), PolylineError> {
        match self {
            Item::Trace(i) => return i.change_placement_side(pole, ctx),
            Item::Via(i) => i.change_placement_side(pole, ctx),
            Item::Pin(i) => i.change_placement_side(pole),
            Item::ObstacleArea(i) => i.change_placement_side(pole, ctx),
            Item::ConductionArea(i) => i.change_placement_side(pole, ctx),
            Item::ViaObstacleArea(i) => i.change_placement_side(pole, ctx),
            Item::ComponentObstacleArea(i) => i.change_placement_side(pole, ctx),
            Item::ComponentOutline(i) => i.change_placement_side(pole),
            Item::BoardOutline(i) => i.change_placement_side(pole),
        }
        Ok(())
    }

    // -- copying (Item.java:252-266) -------------------------------------------------------------

    /// Port of the abstract `Item.copy(int)` (Item.java:252-256) and its nine overrides: a copy
    /// of this item carrying `new_id`.
    ///
    /// Returns `Option` because one override can fail: `ConductionArea.copy`
    /// (ConductionArea.java:309-313) warns and returns **`null`** for an area on more than one
    /// net. Every other override always produces an item.
    ///
    /// Java's `id <= 0 means generate one` clause (Item.java:253-255) is resolved by the caller,
    /// as it is for [`ItemHeader::new`].
    ///
    /// Note what each Java constructor drops or hard-codes, which the header copies below
    /// reproduce: `ComponentOutline`'s constructor passes `new int[0], 0` for the net numbers and
    /// clearance class (ComponentOutline.java:46), `ComponentObstacleArea`'s passes `new int[0]`
    /// for the net numbers (ComponentObstacleArea.java:38), and `BoardOutline`'s passes
    /// `new int[0], …, 0, FixedState.SYSTEM_FIXED` (BoardOutline.java:47). None of the copies
    /// carries `onTheBoard`, the search-tree entries or the autoroute scratch — Java allocates a
    /// fresh object, and `Item.clone` (Item.java:258-266) exists precisely to put `onTheBoard`
    /// back afterwards.
    pub fn copy(&self, new_id: ItemId) -> Option<Item> {
        match self {
            Item::Trace(i) => Some(Item::Trace(i.copy(new_id))),
            Item::Via(i) => Some(Item::Via(i.copy(new_id))),
            Item::Pin(i) => Some(Item::Pin(i.copy(new_id))),
            Item::ObstacleArea(i) => Some(Item::ObstacleArea(i.copy(new_id))),
            Item::ConductionArea(i) => i.copy(new_id).map(Item::ConductionArea),
            Item::ViaObstacleArea(i) => Some(Item::ViaObstacleArea(i.copy(new_id))),
            Item::ComponentObstacleArea(i) => Some(Item::ComponentObstacleArea(i.copy(new_id))),
            Item::ComponentOutline(i) => Some(Item::ComponentOutline(i.copy(new_id))),
            Item::BoardOutline(i) => Some(Item::BoardOutline(i.copy(new_id))),
        }
    }

    /// Port of `Item.clone` (Item.java:258-266): `copy(getId())` with `onTheBoard` carried over.
    // renamed: Item.clone -> Item::java_clone, because `#[derive(Clone)]` already gives this
    // type a `clone` and the two are different operations — the derived one copies the search
    // tree entries and the autoroute scratch, Java's does not.
    pub fn java_clone(&self) -> Option<Item> {
        let mut dup = self.copy(self.id())?;
        dup.set_on_the_board(self.is_on_the_board());
        Some(dup)
    }
}

/// `ObstacleArea.isObstacle(Item)` (ObstacleArea.java:174-180), factored out because
/// `ConductionArea.isObstacle` reaches it through `super` (ConductionArea.java:382).
fn obstacle_area_is_obstacle(this: &Item, other: &Item) -> bool {
    if other.shares_net(this) {
        return false;
    }
    other.is_trace() || matches!(other, Item::Via(_))
}

/// The search-tree key of a board item (plan-rulings.md #2). Not a Java conversion: Java puts
/// the `Item` itself into the tree, because `Item implements SearchTreeObject`
/// (Item.java:38-39); this port stores the id, and [`crate::ids::TreeObject`]'s `Ord`
/// reproduces `Item.compareTo`.
impl From<&Item> for crate::ids::TreeObject {
    fn from(item: &Item) -> crate::ids::TreeObject {
        crate::ids::TreeObject::Item(item.id())
    }
}

impl std::fmt::Display for Item {
    // renamed: Item.toString -> Display::fmt (Item.java:1257-1269): the lower-cased simple class
    // name, plus " of component #N" when the item belongs to a component.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let simple_name = match self {
            Item::Trace(_) => "polylinetrace",
            Item::Via(_) => "via",
            Item::Pin(_) => "pin",
            Item::ObstacleArea(_) => "obstaclearea",
            Item::ConductionArea(_) => "conductionarea",
            Item::ViaObstacleArea(_) => "viaobstaclearea",
            Item::ComponentObstacleArea(_) => "componentobstaclearea",
            Item::ComponentOutline(_) => "componentoutline",
            Item::BoardOutline(_) => "boardoutline",
        };
        f.write_str(simple_name)?;
        // Pin.toString (Pin.java:675-692) inserts the pin index before the component clause.
        if let Item::Pin(pin) = self
            && pin.get_pin_index() > 0
        {
            write!(f, " #{}", pin.get_pin_index())?;
        }
        if self.component_id() > 0 {
            write!(f, " of component #{}", self.component_id())?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Connectable
// ---------------------------------------------------------------------------------------------

/// Port of the `Connectable` interface (`board/model/items/Connectable.java`): what an item that
/// can be electrically connected must provide.
///
/// Implemented by exactly the four classes Java implements it on: `Trace` (so [`PolylineTrace`]),
/// `DrillItem` (so [`Via`] and [`Pin`]) and [`ConductionArea`].
///
/// The interface's other four methods — `getAllContacts()`, `getAllContacts(int)`,
/// `getNormalContacts()` and `getConnectedSet(int)` (Connectable.java:17-37) — all walk the
/// board's search tree, so they are not on this trait.
// added in Task 11: `Board::all_contacts`, `Board::all_contacts_on_layer`,
// `Board::normal_contacts` and `Board::connected_set`, which are those four.
pub trait Connectable {
    /// The item's shared state, so the two net methods below need no per-implementor body.
    fn header(&self) -> &ItemHeader;

    /// `Connectable.containsNet` (Connectable.java:11) = `Item.containsNet`
    /// (Item.java:148-159).
    fn contains_net(&self, net_number: i32) -> bool {
        self.header().contains_net(net_number)
    }

    /// `Connectable.sharesNetNo` (Connectable.java:14) = `Item.sharesNetNo`
    /// (Item.java:179-189).
    fn shares_net_no(&self, net_nos: &[i32]) -> bool {
        self.header().shares_net_no(net_nos)
    }

    /// `Connectable.getTraceConnectionShape(ShapeSearchTree, int)` (Connectable.java:43): the
    /// subshape of tree shape `index` that a trace may connect to.
    fn get_trace_connection_shape(
        &self,
        tree: TreeId,
        index: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<TileShape>;
}

/// A borrowed view of an [`Item`] that implements [`Connectable`] — the result of
/// [`Item::as_connectable`].
///
/// Not a Java type: Java casts to the `Connectable` interface. An enum rather than
/// `&dyn Connectable` because the callers that matter (Task 11's contact walks) need to know
/// *which* connectable they have.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectableRef<'a> {
    Trace(&'a PolylineTrace),
    Via(&'a Via),
    Pin(&'a Pin),
    ConductionArea(&'a ConductionArea),
}

impl ConnectableRef<'_> {
    /// The `Connectable` behind this reference, for callers that only need the interface.
    pub fn as_dyn(&self) -> &dyn Connectable {
        match self {
            ConnectableRef::Trace(i) => *i,
            ConnectableRef::Via(i) => *i,
            ConnectableRef::Pin(i) => *i,
            ConnectableRef::ConductionArea(i) => *i,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Per-variant methods.
//
// Everything below is either a body that is already complete (it needs nothing but the header)
// or a `// added in Task N:` stub whose Java body needs a field that task adds. The stubs panic
// with the Java line they will be ported from rather than returning a plausible-looking value,
// so a caller that reaches one before its task lands fails loudly instead of routing on a
// fabricated layer or bounding box.
// ---------------------------------------------------------------------------------------------

/// The `hdr`-only half of a variant's `copy`: a fresh header with `new_id`, carrying exactly the
/// fields the Java constructor forwards.
pub(crate) fn copied_header(hdr: &ItemHeader, new_id: ItemId) -> ItemHeader {
    ItemHeader::new(
        new_id,
        hdr.net_nos.clone(),
        hdr.clearance_class(),
        hdr.get_component_id(),
        hdr.get_fixed_state(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_geometry::{Point, Shape, TileShape};

    use crate::ids::{PadstackId, TreeObject};
    use crate::library::{BoardLibrary, PackagePin, Packages, Padstacks};
    use crate::rules::ClearanceMatrix;
    use crate::structure::{Components, Layer, LayerStructure};

    /// The board state the drill-item bodies read through [`ItemCtx`] — Java reaches it through
    /// `Item.board`. Two padstacks (an SMD pad on layer 0 only and a through-hole pad on both
    /// layers), one package per padstack, and one component per package.
    struct Fixture {
        library: BoardLibrary,
        components: Components,
        rules: BoardRules,
        bounding_box: IntBox,
    }

    /// The SMD component's id, whose single pin is on a one-layer padstack (`drillAllowed`).
    const SMD_COMPONENT: i32 = 1;
    /// The through-hole component's id, whose single pin spans both layers.
    const THT_COMPONENT: i32 = 2;
    const VIA_PADSTACK: PadstackId = PadstackId(1);

    impl Fixture {
        fn new() -> Fixture {
            let mut padstacks = Padstacks::new(layer_structure());
            let smd = padstacks.add(
                "SMD",
                vec![
                    Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                        -10, -10, 10, 10,
                    )))),
                    None,
                ],
                true,
                false,
            );
            let tht = padstacks.add(
                "THT",
                vec![
                    Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                        -10, -10, 10, 10,
                    )))),
                    Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                        -10, -10, 10, 10,
                    )))),
                ],
                true,
                false,
            );
            let mut packages = Packages::new();
            let smd_package =
                packages.add_pins(vec![PackagePin::new("1", smd, Vector::new(0, 0), 0.0)]);
            let tht_package =
                packages.add_pins(vec![PackagePin::new("1", tht, Vector::new(0, 0), 0.0)]);
            let mut components = Components::new();
            components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, smd_package);
            components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, tht_package);
            Fixture {
                library: BoardLibrary::new(padstacks, packages),
                components,
                rules: rules(),
                bounding_box: IntBox::from_coords(-10_000, -10_000, 10_000, 10_000),
            }
        }

        fn ctx(&self) -> ItemCtx<'_> {
            ItemCtx {
                library: &self.library,
                components: &self.components,
                rules: &self.rules,
                bounding_box: &self.bounding_box,
            }
        }
    }

    fn rules() -> BoardRules {
        let ls = layer_structure();
        let cm = ClearanceMatrix::get_default_instance(&ls, 100);
        BoardRules::new(ls, cm)
    }

    fn hdr(id: u32, net_nos: Vec<i32>) -> ItemHeader {
        ItemHeader::new(ItemId(id), net_nos, 1, 0, FixedState::Unfixed)
    }

    fn hdr_of_component(id: u32, component_id: i32) -> ItemHeader {
        ItemHeader::new(ItemId(id), Vec::new(), 1, component_id, FixedState::Unfixed)
    }

    fn trace(id: u32, net_nos: Vec<i32>) -> Item {
        // A degenerate one-segment polyline: these tests only exercise the `Item` dispatch,
        // never the geometry; `tests/polyline_trace.rs` is where the geometry is pinned.
        Item::Trace(PolylineTrace::new(
            hdr(id, net_nos),
            fr_geometry::Polyline::from_two_points(&Point::new(0, 0), &Point::new(100, 0)),
            0,
            50,
            None,
        ))
    }

    fn via(id: u32, net_nos: Vec<i32>, attach_allowed: bool) -> Item {
        Item::Via(Via::new(
            hdr(id, net_nos),
            VIA_PADSTACK,
            Point::new(0, 0),
            attach_allowed,
        ))
    }

    /// An SMD pin — one layer, so `Pin.drillAllowed` (Pin.java:344-350) is true.
    fn pin(id: u32, net_nos: Vec<i32>) -> Item {
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(id), net_nos, 1, SMD_COMPONENT, FixedState::Unfixed),
            0,
        ))
    }

    /// A through-hole pin — two layers, so `Pin.drillAllowed` is false.
    fn tht_pin(id: u32, net_nos: Vec<i32>) -> Item {
        Item::Pin(Pin::new(
            ItemHeader::new(ItemId(id), net_nos, 1, THT_COMPONENT, FixedState::Unfixed),
            0,
        ))
    }

    /// A unit square on layer 0 at the origin — enough geometry for the dispatch tests here;
    /// `crates/fr-board/tests/areas_and_outlines.rs` is where the geometry itself is pinned.
    fn area_data() -> ObstacleAreaData {
        ObstacleAreaData::new(
            fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                0, 0, 10, 10,
            )))),
            0,
            Vector::new(0, 0),
            0.0,
            false,
            None,
        )
    }

    fn obstacle_area(id: u32, net_nos: Vec<i32>) -> Item {
        Item::ObstacleArea(ObstacleArea::new(hdr(id, net_nos), area_data()))
    }

    fn conduction_area(id: u32, net_nos: Vec<i32>, is_obstacle: bool) -> Item {
        Item::ConductionArea(ConductionArea::new(
            hdr(id, net_nos),
            area_data(),
            is_obstacle,
        ))
    }

    fn via_keepout(id: u32, net_nos: Vec<i32>) -> Item {
        Item::ViaObstacleArea(ViaObstacleArea::new(hdr(id, net_nos), area_data()))
    }

    fn component_keepout(id: u32, component_id: i32) -> Item {
        Item::ComponentObstacleArea(ComponentObstacleArea::new(
            hdr_of_component(id, component_id),
            area_data(),
        ))
    }

    fn component_outline(id: u32) -> Item {
        Item::ComponentOutline(ComponentOutline::new(
            hdr(id, Vec::new()),
            fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                0, 0, 10, 10,
            )))),
            true,
            Vector::new(0, 0),
            0.0,
            true,
            false,
            true,
        ))
    }

    fn board_outline(id: u32) -> Item {
        Item::BoardOutline(BoardOutline::new(
            ItemHeader::new(ItemId(id), Vec::new(), 1, 0, FixedState::SystemFixed),
            Vec::new(),
        ))
    }

    /// Every variant, for tests that must cover the whole enum.
    fn one_of_each() -> Vec<Item> {
        vec![
            trace(1, vec![1]),
            via(2, vec![1], true),
            pin(3, vec![1]),
            obstacle_area(4, vec![]),
            conduction_area(5, vec![1], true),
            via_keepout(6, vec![]),
            component_keepout(7, 1),
            component_outline(8),
            board_outline(9),
        ]
    }

    // ---- kind / family predicates ------------------------------------------------------------

    #[test]
    fn kind_matches_get_board_item_type() {
        // Item.java:117-146, in Java's test order.
        let expected = [
            ItemKind::Trace,
            ItemKind::Via,
            ItemKind::Pin,
            ItemKind::ObstacleArea,
            ItemKind::ConductionArea,
            ItemKind::ViaObstacleArea,
            ItemKind::ComponentObstacleArea,
            ItemKind::ComponentOutline,
            ItemKind::BoardOutline,
        ];
        for (item, kind) in one_of_each().iter().zip(expected) {
            assert_eq!(item.kind(), kind, "{item}");
        }
    }

    #[test]
    fn is_obstacle_area_covers_the_whole_java_subclass_family() {
        // ObstacleArea.java:25 is extended by ConductionArea.java:25,
        // ViaObstacleArea.java:13 and ComponentObstacleArea.java:14.
        let expected = [false, false, false, true, true, true, true, false, false];
        for (item, is_area) in one_of_each().iter().zip(expected) {
            assert_eq!(item.is_obstacle_area(), is_area, "{item}");
        }
    }

    #[test]
    fn is_trace_and_is_drill_item_cover_their_families() {
        // Trace.java:28 (PolylineTrace only), DrillItem.java:25 (Via and Pin).
        let traces = [true, false, false, false, false, false, false, false, false];
        let drills = [false, true, true, false, false, false, false, false, false];
        for ((item, is_trace), is_drill) in one_of_each().iter().zip(traces).zip(drills) {
            assert_eq!(item.is_trace(), is_trace, "{item}");
            assert_eq!(item.is_drill_item(), is_drill, "{item}");
        }
    }

    // ---- Connectable dispatch -----------------------------------------------------------------

    #[test]
    fn as_connectable_matches_javas_implements_clauses() {
        // Trace.java:28, DrillItem.java:25, ConductionArea.java:25 implement Connectable;
        // ObstacleArea, ViaObstacleArea, ComponentObstacleArea, ComponentOutline and
        // BoardOutline do not.
        let expected = [true, true, true, false, true, false, false, false, false];
        for (item, connectable) in one_of_each().iter().zip(expected) {
            assert_eq!(item.as_connectable().is_some(), connectable, "{item}");
        }
    }

    #[test]
    fn is_connectable_also_needs_a_net() {
        // Item.java:868-871: `(this instanceof Connectable) && this.netCount() > 0`.
        assert!(trace(1, vec![3]).is_connectable());
        assert!(!trace(1, vec![]).is_connectable());
        assert!(!obstacle_area(1, vec![3]).is_connectable());
    }

    #[test]
    fn connectable_ref_forwards_the_two_net_methods() {
        // Connectable.java:11,14 are Item.containsNet / Item.sharesNetNo.
        let item = via(1, vec![4], true);
        let connectable = item.as_connectable().expect("a via is connectable");
        assert!(connectable.as_dyn().contains_net(4));
        assert!(!connectable.as_dyn().contains_net(5));
        assert!(connectable.as_dyn().shares_net_no(&[9, 4]));
    }

    // ---- isObstacle: one test per Java override -----------------------------------------------

    #[test]
    fn trace_is_obstacle_matches_trace_java() {
        let f = Fixture::new();
        // Trace.java:91-102.
        let t = trace(1, vec![5]);
        // Trace.java:93-96: itself, via keepouts and component keepouts are never obstacles.
        assert!(!t.is_obstacle(&trace(1, vec![9]), &f.ctx()));
        assert!(!t.is_obstacle(&via_keepout(2, vec![9]), &f.ctx()));
        assert!(!t.is_obstacle(&component_keepout(3, 1), &f.ctx()));
        // Trace.java:98-100: a non-obstacle conduction area is not an obstacle either.
        assert!(!t.is_obstacle(&conduction_area(4, vec![9], false), &f.ctx()));
        assert!(t.is_obstacle(&conduction_area(5, vec![9], true), &f.ctx()));
        // Trace.java:101: everything else, unless the nets are shared.
        assert!(t.is_obstacle(&trace(6, vec![9]), &f.ctx()));
        assert!(!t.is_obstacle(&trace(7, vec![5]), &f.ctx()));
        assert!(t.is_obstacle(&obstacle_area(8, vec![9]), &f.ctx()));
    }

    #[test]
    fn via_is_obstacle_matches_via_java() {
        let f = Fixture::new();
        // Via.java:151-166.
        let v = via(1, vec![5], true);
        // Via.java:153-155. Note a *via* keepout is not excused here, unlike for a trace.
        assert!(!v.is_obstacle(&via(1, vec![9], true), &f.ctx()));
        assert!(!v.is_obstacle(&component_keepout(2, 1), &f.ctx()));
        assert!(v.is_obstacle(&via_keepout(3, vec![9]), &f.ctx()));
        // Via.java:156-158.
        assert!(!v.is_obstacle(&conduction_area(4, vec![9], false), &f.ctx()));
        // Via.java:159-161.
        assert!(v.is_obstacle(&trace(5, vec![9]), &f.ctx()));
        // Via.java:162-164: a same-net trace is not an obstacle.
        assert!(!v.is_obstacle(&trace(6, vec![5]), &f.ctx()));
        // Via.java:165: a same-net non-pin item is.
        assert!(v.is_obstacle(&via(7, vec![5], true), &f.ctx()));
    }

    #[test]
    fn via_is_obstacle_to_a_same_net_pin_only_when_attach_is_not_allowed() {
        // Via.java:165: `!attachAllowed || !(other instanceof Pin) || !((Pin) other).drillAllowed()`.
        let f = Fixture::new();
        assert!(via(1, vec![5], false).is_obstacle(&pin(2, vec![5]), &f.ctx()));
        assert!(!via(1, vec![5], true).is_obstacle(&pin(2, vec![5]), &f.ctx()));
        // A through-hole pin is not drillable, so an attachable via is an obstacle to it.
        assert!(via(1, vec![5], true).is_obstacle(&tht_pin(2, vec![5]), &f.ctx()));
    }

    #[test]
    fn pin_is_obstacle_matches_pin_java() {
        let f = Fixture::new();
        // Pin.java:352-366.
        let p = pin(1, vec![5]);
        // Pin.java:354-356: itself and *every* ObstacleArea subclass.
        assert!(!p.is_obstacle(&pin(1, vec![9]), &f.ctx()));
        for area in [
            obstacle_area(2, vec![9]),
            conduction_area(3, vec![9], true),
            via_keepout(4, vec![9]),
            component_keepout(5, 1),
        ] {
            assert!(!p.is_obstacle(&area, &f.ctx()), "{area}");
        }
        // Pin.java:357-359.
        assert!(p.is_obstacle(&via(6, vec![9], true), &f.ctx()));
        // Pin.java:360-362.
        assert!(!p.is_obstacle(&trace(7, vec![5]), &f.ctx()));
    }

    #[test]
    fn pin_is_obstacle_to_a_same_net_via_only_when_it_is_not_an_smd_pad() {
        // Pin.java:364: `!this.drillAllowed() || !(other instanceof Via)`.
        let f = Fixture::new();
        assert!(!pin(1, vec![5]).is_obstacle(&via(2, vec![5], true), &f.ctx()));
        assert!(tht_pin(1, vec![5]).is_obstacle(&via(2, vec![5], true), &f.ctx()));
    }

    #[test]
    fn obstacle_area_is_obstacle_only_to_foreign_net_traces_and_vias() {
        let f = Fixture::new();
        // ObstacleArea.java:174-180.
        let a = obstacle_area(1, vec![5]);
        assert!(a.is_obstacle(&trace(2, vec![9]), &f.ctx()));
        assert!(a.is_obstacle(&via(3, vec![9], true), &f.ctx()));
        assert!(!a.is_obstacle(&trace(4, vec![5]), &f.ctx()));
        assert!(!a.is_obstacle(&pin(5, vec![9]), &f.ctx()));
        assert!(!a.is_obstacle(&obstacle_area(6, vec![9]), &f.ctx()));
    }

    #[test]
    fn conduction_area_is_obstacle_delegates_to_super_only_when_the_flag_is_set() {
        let f = Fixture::new();
        // ConductionArea.java:379-386.
        let on = conduction_area(1, vec![5], true);
        assert!(on.is_obstacle(&trace(2, vec![9]), &f.ctx()));
        assert!(!on.is_obstacle(&trace(3, vec![5]), &f.ctx()));
        let off = conduction_area(4, vec![5], false);
        assert!(!off.is_obstacle(&trace(5, vec![9]), &f.ctx()));
        assert!(!off.is_obstacle(&via(6, vec![9], true), &f.ctx()));
    }

    #[test]
    fn via_keepout_is_obstacle_only_to_foreign_net_vias() {
        let f = Fixture::new();
        // ViaObstacleArea.java:91-97.
        let k = via_keepout(1, vec![5]);
        assert!(k.is_obstacle(&via(2, vec![9], true), &f.ctx()));
        assert!(!k.is_obstacle(&via(3, vec![5], true), &f.ctx()));
        assert!(!k.is_obstacle(&trace(4, vec![9]), &f.ctx()));
    }

    #[test]
    fn component_keepout_is_obstacle_only_to_other_components_keepouts() {
        let f = Fixture::new();
        // ComponentObstacleArea.java:63-68.
        let k = component_keepout(1, 7);
        assert!(k.is_obstacle(&component_keepout(2, 8), &f.ctx()));
        assert!(!k.is_obstacle(&component_keepout(3, 7), &f.ctx()));
        assert!(!k.is_obstacle(&component_keepout(1, 8), &f.ctx())); // `other == this`
        assert!(!k.is_obstacle(&trace(4, vec![9]), &f.ctx()));
    }

    #[test]
    fn component_outline_is_never_an_obstacle() {
        let f = Fixture::new();
        // ComponentOutline.java:119-122.
        let o = component_outline(1);
        for other in one_of_each() {
            assert!(!o.is_obstacle(&other, &f.ctx()), "{other}");
        }
    }

    #[test]
    fn board_outline_is_an_obstacle_to_everything_but_outlines_and_areas() {
        let f = Fixture::new();
        // BoardOutline.java:83-86: `!(other instanceof BoardOutline || other instanceof
        // ObstacleArea)` — and `instanceof ObstacleArea` covers all four area variants.
        let b = board_outline(1);
        let expected = [true, true, true, false, false, false, false, true, false];
        for (other, is_obstacle) in one_of_each().iter().zip(expected) {
            assert_eq!(b.is_obstacle(other, &f.ctx()), is_obstacle, "{other}");
        }
    }

    // ---- the net-number obstacle predicates ---------------------------------------------------

    #[test]
    fn is_obstacle_for_net_is_plain_non_membership() {
        // Item.java:161-164; no subclass overrides it.
        for item in one_of_each() {
            assert_eq!(item.is_obstacle_for_net(1), !item.contains_net(1), "{item}");
        }
    }

    #[test]
    fn is_trace_obstacle_matches_the_base_and_its_three_overrides() {
        // Item.java:169-172; ConductionArea.java:397-400; ViaObstacleArea.java:99-102;
        // ComponentObstacleArea.java:70-73.
        assert!(trace(1, vec![5]).is_trace_obstacle(9));
        assert!(!trace(1, vec![5]).is_trace_obstacle(5));
        assert!(conduction_area(2, vec![5], true).is_trace_obstacle(9));
        assert!(!conduction_area(2, vec![5], true).is_trace_obstacle(5));
        assert!(!conduction_area(2, vec![5], false).is_trace_obstacle(9));
        assert!(!via_keepout(3, vec![]).is_trace_obstacle(9));
        assert!(!component_keepout(4, 1).is_trace_obstacle(9));
        assert!(obstacle_area(5, vec![]).is_trace_obstacle(9));
    }

    #[test]
    fn is_drillable_matches_the_base_and_its_two_overrides() {
        // Item.java:851-854; Trace.java:221-225; ConductionArea.java:402-405.
        assert!(trace(1, vec![5]).is_drillable(5));
        assert!(!trace(1, vec![5]).is_drillable(9));
        assert!(!conduction_area(2, vec![5], true).is_drillable(9));
        assert!(conduction_area(2, vec![5], true).is_drillable(5));
        assert!(conduction_area(2, vec![5], false).is_drillable(9));
        assert!(!via(3, vec![5], true).is_drillable(5));
        assert!(!obstacle_area(4, vec![]).is_drillable(5));
    }

    #[test]
    fn is_routable_is_true_only_for_unfixed_traces_and_vias_with_a_net() {
        // Item.java:863-866; Trace.java:205-209; Via.java:146-150.
        assert!(trace(1, vec![5]).is_routable());
        assert!(via(2, vec![5], true).is_routable());
        assert!(!trace(3, vec![]).is_routable());
        assert!(!pin(4, vec![5]).is_routable());
        let mut fixed = trace(5, vec![5]);
        fixed.set_fixed_state(FixedState::UserFixed);
        assert!(!fixed.is_routable());
        // Shove-fixed is below USER_FIXED, so it is still routable (Item.java:817-818).
        let mut shoved = trace(6, vec![5]);
        shoved.set_fixed_state(FixedState::ShoveFixed);
        assert!(shoved.is_routable());
    }

    // ---- isShoveFixed / isDeletionForbidden (the two that take BoardRules) ---------------------

    fn layer_structure() -> LayerStructure {
        LayerStructure::new(vec![Layer::new("F.Cu", true), Layer::new("B.Cu", true)])
    }

    /// Rules with net 1 on an ordinary class and net 2 on a shove-fixed one
    /// (`NetClass.isShoveFixed`, which Trace.isShoveFixed reads at Trace.java:246-248).
    fn rules_with_a_shove_fixed_net() -> BoardRules {
        let layers = layer_structure();
        let matrix = ClearanceMatrix::new(2, &layers, &["default", "c1"]);
        let mut rules = BoardRules::new(layers.clone(), matrix);
        let plain = rules.net_classes.append("plain", &layers, false);
        let shove_fixed = rules.net_classes.append("shove_fixed", &layers, false);
        rules.net_classes.get_mut(shove_fixed).set_shove_fixed(true);
        rules.nets.add("n1", 1, false, plain);
        rules.nets.add("n2", 1, false, shove_fixed);
        rules
    }

    #[test]
    fn is_shove_fixed_applies_traces_net_class_override() {
        // Trace.isShoveFixed (Trace.java:236-254): `super.isShoveFixed()` first, then true if
        // any normal net of the trace belongs to a shove-fixed net class.
        let rules = rules_with_a_shove_fixed_net();
        assert!(!trace(1, vec![1]).is_shove_fixed(&rules));
        assert!(trace(2, vec![2]).is_shove_fixed(&rules));
        assert!(trace(3, vec![1, 2]).is_shove_fixed(&rules));
    }

    #[test]
    fn is_shove_fixed_override_is_traces_only() {
        // Only Trace overrides it; a via on the same shove-fixed net answers with the base body
        // (Item.java:834-839).
        let rules = rules_with_a_shove_fixed_net();
        assert!(!via(1, vec![2], true).is_shove_fixed(&rules));
        assert!(!conduction_area(2, vec![2], true).is_shove_fixed(&rules));
    }

    #[test]
    fn is_shove_fixed_base_body_still_wins_for_a_fixed_trace() {
        // Trace.java:238-240 returns before the net-class scan.
        let rules = rules_with_a_shove_fixed_net();
        let mut t = trace(1, vec![1]);
        t.set_fixed_state(FixedState::ShoveFixed);
        assert!(t.is_shove_fixed(&rules));
    }

    #[test]
    fn is_shove_fixed_skips_non_normal_net_numbers() {
        // Trace.java:245: the scan only looks at `Nets.isNormalNetNumber` entries, which is what
        // keeps `nets.get(...)` from returning null there.
        let rules = rules_with_a_shove_fixed_net();
        assert!(!trace(1, vec![0]).is_shove_fixed(&rules));
        assert!(!trace(2, vec![]).is_shove_fixed(&rules));
    }

    #[test]
    #[should_panic(expected = "NullPointerException")]
    fn is_shove_fixed_panics_on_a_net_number_past_the_net_list_like_java() {
        // Trace.java:246: `nets.get(currentNetNumber).getNetClass()` with a normal-but-unknown
        // net number dereferences null.
        trace(1, vec![9]).is_shove_fixed(&rules_with_a_shove_fixed_net());
    }

    #[test]
    fn is_deletion_forbidden_for_component_items_and_user_fixed_items() {
        // Item.java:821-832, the two branches that need no layer lookup.
        let rules = rules_with_a_shove_fixed_net();
        assert!(!trace(1, vec![1]).is_deletion_forbidden(&rules));

        let mut of_component = trace(2, vec![1]);
        of_component.assign_component_id(4);
        assert!(of_component.is_deletion_forbidden(&rules));

        let mut user_fixed = trace(3, vec![1]);
        user_fixed.set_fixed_state(FixedState::UserFixed);
        assert!(user_fixed.is_deletion_forbidden(&rules));

        // SYSTEM_FIXED is above USER_FIXED in the ordinal order (Item.java:817-818).
        let mut system_fixed = trace(4, vec![1]);
        system_fixed.set_fixed_state(FixedState::SystemFixed);
        assert!(system_fixed.is_deletion_forbidden(&rules));

        // SHOVE_FIXED is below it, so it does not forbid deletion on its own.
        let mut shove_fixed = trace(5, vec![1]);
        shove_fixed.set_fixed_state(FixedState::ShoveFixed);
        assert!(!shove_fixed.is_deletion_forbidden(&rules));
    }

    #[test]
    fn is_deletion_forbidden_takes_the_short_circuit_before_the_conduction_area_branch() {
        // Item.java:824-826 returns before reaching `area.getLayer()` (Item.java:828-830), which
        // is why a component-owned conduction area answers without Task 7's layer field.
        let rules = rules_with_a_shove_fixed_net();
        let mut area = conduction_area(1, vec![1], true);
        area.assign_component_id(2);
        assert!(area.is_deletion_forbidden(&rules));
    }

    #[test]
    fn is_deletion_forbidden_on_a_free_conduction_area_reads_its_layer() {
        // Item.java:828-830: `!board.layerStructure.layers[area.getLayer()].isSignal` — the
        // power-plane branch. `area_data()` puts the area on layer 0, which
        // `layer_structure()` makes a signal layer, so deletion is allowed; a non-signal layer
        // forbids it.
        let rules = rules_with_a_shove_fixed_net();
        assert!(!conduction_area(1, vec![1], true).is_deletion_forbidden(&rules));

        let power_plane = BoardRules::new(
            LayerStructure::new(vec![Layer::new("plane", false), Layer::new("back", true)]),
            ClearanceMatrix::get_default_instance(
                &LayerStructure::new(vec![Layer::new("plane", false), Layer::new("back", true)]),
                100,
            ),
        );
        assert!(conduction_area(1, vec![1], true).is_deletion_forbidden(&power_plane));
    }

    // ---- ordering, identity, display ------------------------------------------------------------

    #[test]
    fn compare_to_orders_by_descending_id_like_java() {
        // Java bug (Item.java:98): `item.id - id` is the wrong way round.
        let low = trace(1, vec![]);
        let high = trace(2, vec![]);
        assert_eq!(low.compare_to(&high), Ordering::Greater);
        assert_eq!(high.compare_to(&low), Ordering::Less);
        assert_eq!(low.compare_to(&trace(1, vec![7])), Ordering::Equal);
    }

    #[test]
    fn tree_object_from_an_item_is_its_id() {
        // plan-rulings.md #2: the shared search tree stores items by id.
        let item = via(42, vec![], true);
        assert_eq!(TreeObject::from(&item), TreeObject::Item(ItemId(42)));
    }

    #[test]
    fn tree_objects_of_two_items_order_like_item_compare_to() {
        // ShapeTree.Leaf.compareTo (ShapeTree.java:216-223) delegates to Item.compareTo, so the
        // TreeObject ordering must agree with it.
        let low = trace(1, vec![]);
        let high = trace(2, vec![]);
        assert_eq!(
            TreeObject::from(&low).cmp(&TreeObject::from(&high)),
            low.compare_to(&high)
        );
    }

    #[test]
    fn display_matches_java_to_string() {
        // Item.java:1257-1269: lower-cased simple class name, plus the component suffix.
        assert_eq!(trace(1, vec![]).to_string(), "polylinetrace");
        assert_eq!(board_outline(2).to_string(), "boardoutline");
        assert_eq!(
            component_keepout(3, 5).to_string(),
            "componentobstaclearea of component #5"
        );
    }

    // ---- copy ------------------------------------------------------------------------------------

    #[test]
    fn copy_carries_the_header_and_takes_the_new_id() {
        // e.g. PolylineTrace.java:62-79.
        let mut original = trace(1, vec![4, 6]);
        original.set_fixed_state(FixedState::UserFixed);
        original.assign_component_id(3);
        original.set_on_the_board(true);
        let copy = original.copy(ItemId(99)).expect("a trace copy never fails");
        assert_eq!(copy.id(), ItemId(99));
        assert_eq!(copy.net_nos(), &[4, 6]);
        assert_eq!(copy.get_fixed_state(), FixedState::UserFixed);
        assert_eq!(copy.component_id(), 3);
        assert_eq!(copy.clearance_class(), 1);
        // Java allocates a fresh object, so `onTheBoard` is not carried by `copy`
        // (Item.java:258-266 exists to put it back).
        assert!(!copy.is_on_the_board());
    }

    #[test]
    fn java_clone_restores_on_the_board() {
        // Item.java:258-266.
        let mut original = trace(1, vec![4]);
        original.set_on_the_board(true);
        let dup = original.java_clone().expect("a trace clone never fails");
        assert_eq!(dup.id(), ItemId(1));
        assert!(dup.is_on_the_board());
    }

    #[test]
    fn conduction_area_copy_resets_is_filled_to_true() {
        // ConductionArea.copy (ConductionArea.java:314-329) calls the constructor
        // (ConductionArea.java:46-73), whose parameter list has no `isFilled`, so the copy picks
        // up the field initialiser `private boolean isFilled = true` (ConductionArea.java:30)
        // however the original was set.
        let mut area = ConductionArea::new(hdr(1, vec![5]), area_data(), true);
        area.set_is_filled(false);
        assert!(!area.get_is_filled());
        let copy = area.copy(ItemId(2)).expect("one net, so the copy succeeds");
        assert!(copy.get_is_filled());
        // `isObstacle` *is* a constructor parameter (ConductionArea.java:57), so it is carried.
        assert!(copy.get_is_obstacle());
    }

    #[test]
    fn component_outline_has_no_tile_shapes() {
        // ComponentOutline.tileShapeCount (ComponentOutline.java:129-132) is literally
        // `return 0;`, matching its `calculateTreeShapes` returning `new TileShape[0]`
        // (ComponentOutline.java:134-137).
        let f = Fixture::new();
        assert_eq!(component_outline(1).tile_shape_count(&f.ctx()), 0);
    }

    #[test]
    fn conduction_area_copy_fails_unless_the_area_is_on_exactly_one_net() {
        // Java bug (ConductionArea.java:310-313): `netCount() != 1` warns and returns null.
        assert!(conduction_area(1, vec![5], true).copy(ItemId(2)).is_some());
        assert!(conduction_area(1, vec![], true).copy(ItemId(2)).is_none());
        assert!(
            conduction_area(1, vec![5, 6], true)
                .copy(ItemId(2))
                .is_none()
        );
    }

    #[test]
    fn component_outline_copy_drops_the_nets_and_the_clearance_class() {
        // ComponentOutline.java:46: the constructor passes `new int[0], 0`.
        let mut original = ComponentOutline::new(
            ItemHeader::new(ItemId(1), vec![5], 7, 3, FixedState::UserFixed),
            fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                0, 0, 10, 10,
            )))),
            true,
            Vector::new(0, 0),
            0.0,
            true,
            false,
            true,
        );
        original.hdr.set_on_the_board(true);
        let copy = original.copy(ItemId(2));
        assert_eq!(copy.hdr.id(), ItemId(2));
        assert!(copy.hdr.net_nos.is_empty());
        assert_eq!(copy.hdr.clearance_class(), 0);
        assert_eq!(copy.hdr.get_component_id(), 3);
        assert_eq!(copy.hdr.get_fixed_state(), FixedState::UserFixed);
    }

    #[test]
    fn board_outline_copy_resets_to_the_constructors_hard_coded_values() {
        // BoardOutline.java:46-49: `new int[0]`, component 0, SYSTEM_FIXED; only the clearance
        // class survives.
        let original = BoardOutline::new(
            ItemHeader::new(ItemId(1), vec![5], 7, 3, FixedState::Unfixed),
            Vec::new(),
        );
        let copy = original.copy(ItemId(2));
        assert!(copy.hdr.net_nos.is_empty());
        assert_eq!(copy.hdr.clearance_class(), 7);
        assert_eq!(copy.hdr.get_component_id(), 0);
        assert_eq!(copy.hdr.get_fixed_state(), FixedState::SystemFixed);
    }

    #[test]
    fn component_keepout_copy_drops_the_nets() {
        // ComponentObstacleArea.java:38: the constructor passes `new int[0]`.
        let original = ComponentObstacleArea::new(
            ItemHeader::new(ItemId(1), vec![5], 7, 3, FixedState::Unfixed),
            area_data(),
        );
        let copy = original.copy(ItemId(2));
        assert!(copy.hdr.net_nos.is_empty());
        assert_eq!(copy.hdr.clearance_class(), 7);
    }

    #[test]
    fn via_copy_carries_attach_allowed_the_padstack_and_the_escape_via_fields() {
        // Via.java:70-86 passes `attachAllowed` to the new via and then copies the two
        // escape-via fields onto it (Via.java:83-84).
        let original = Via::new(hdr(1, vec![5]), VIA_PADSTACK, Point::new(3, 4), false);
        assert!(!original.copy(ItemId(2)).attach_allowed);
        let mut original = Via::new(hdr(1, vec![5]), VIA_PADSTACK, Point::new(3, 4), true);
        original.is_escape_via = true;
        original.escape_via_smd_layer = 1;
        let copy = original.copy(ItemId(2));
        assert!(copy.attach_allowed);
        assert_eq!(copy.get_padstack_id(), VIA_PADSTACK);
        assert_eq!(copy.get_center(), Point::new(3, 4));
        assert!(copy.is_escape_via);
        assert_eq!(copy.escape_via_smd_layer, 1);
        assert_eq!(copy.hdr.id(), ItemId(2));
    }

    // ---- header delegation and the tree-entry surface --------------------------------------------

    #[test]
    fn header_dispatch_reaches_every_variant() {
        for (index, item) in one_of_each().iter().enumerate() {
            assert_eq!(item.header().id(), ItemId(index as u32 + 1), "{item}");
            assert_eq!(item.id(), item.header().id());
        }
    }

    #[test]
    fn tree_shape_bookkeeping_goes_through_the_header() {
        // Item.java:203-226, 996-1036.
        let mut item = trace(1, vec![]);
        assert_eq!(item.tree_shape_count(TreeId(0)), 0);
        assert_eq!(item.get_tree_shape(TreeId(0), 0), None);
        item.set_precalculated_tree_shapes(TreeId(0), vec![]);
        assert_eq!(item.tree_shape_count(TreeId(0)), 0);
        item.set_tree_entries(TreeId(0), vec![None, None]);
        assert_eq!(
            item.get_search_tree_entries(TreeId(0)).map(<[_]>::len),
            Some(2)
        );
        item.clear_tree_entries();
        assert_eq!(item.get_search_tree_entries(TreeId(0)), None);
    }

    #[test]
    fn autoroute_scratch_goes_through_the_header() {
        // Item.java:1038-1054.
        let mut item = via(1, vec![], true);
        assert_eq!(item.get_autoroute_info_pur(), None);
        item.get_autoroute_info();
        assert!(item.get_autoroute_info_pur().is_some());
        item.clear_autoroute_info();
        assert_eq!(item.get_autoroute_info_pur(), None);
    }

    #[test]
    fn conduction_area_flags_round_trip() {
        // ConductionArea.java:32-40, 387-395.
        let mut area = ConductionArea::new(hdr(1, vec![]), area_data(), false);
        assert!(!area.get_is_obstacle());
        assert!(area.get_is_filled());
        area.set_is_obstacle(true);
        assert!(area.get_is_obstacle());
        area.set_is_filled(false);
        assert!(!area.get_is_filled());
    }

    #[test]
    fn board_outline_half_width_is_javas_constant() {
        // BoardOutline.java:27.
        assert_eq!(board_outline_of().get_half_width(), 100);
    }

    fn board_outline_of() -> BoardOutline {
        BoardOutline::new(
            ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
            Vec::new(),
        )
    }
}
