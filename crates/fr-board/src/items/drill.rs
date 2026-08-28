//! Drill items: the state and behaviour Java's abstract `DrillItem` shares between [`Via`] and
//! [`Pin`].
//!
//! Java: `board/model/items/{DrillItem,Via,Pin}.java`.
//!
//! # What replaces Java's `board` back-pointer
//!
//! Almost every method here reads `this.board` in Java — `board.library.padstacks` for a pin's
//! padstack, `board.components` for its component's placement, `board.layerStructure` for the
//! signal-layer test in `minWidth`. `global-constraints.md` forbids the back-pointer, so those
//! methods take an [`ItemCtx`] instead: a borrow of the three board fields they need. It is a
//! parameter, not a field, so an [`Item`](super::Item) stays a plain value that the board's
//! `BTreeMap` owns outright.
//!
//! Because `Item`'s own dispatch (`first_layer`, `bounding_box`, `is_obstacle`, …) reaches these
//! bodies, the same context parameter runs through `items/mod.rs`. The variants that need
//! nothing from it — traces, areas, outlines — simply ignore it.
//!
//! # The four caches
//!
//! Java memoises four things on a drill item: `precalculatedFirstLayer`,
//! `precalculatedLastLayer` and `precalculatedMinWidth` on `DrillItem` (DrillItem.java:34-46),
//! and `precalculatedShapes` on each of `Via` (Via.java:49) and `Pin` (Pin.java:45). It fills
//! them inside *getters*, which are plain instance methods — `firstLayer()` mutates the object.
//!
//! The port keeps all four (they are not pure memoisation — see the `min_width` note below) and
//! keeps the getters `&self`, because the callers that matter hold two items at once
//! (`a.is_obstacle(&b)`, `a.shares_layer(&b)`) and could not both be `&mut`. That means
//! [`Cell`]/[`RefCell`]: the cache fields are the only interior mutability in this crate, and
//! they are invalidated at exactly Java's invalidation points and nowhere else.
//!
//! not ported: the private `DrillItem.isWithinTolerance(Point, Point, int)`
//! (DrillItem.java:307-324) — dead code. Nothing in the Java tree calls it: `getNormalContacts`
//! used to match trace endpoints to a pin centre with it and now uses exact equality instead
//! (DrillItem.java:285-288 explains why), leaving the helper behind.
//!
//! `PartialEq` on [`Via`] and [`Pin`] is written by hand to skip those cache fields; two items
//! that differ only in what has been computed so far are equal. `center` is **not** skipped —
//! it is a real Java field (`DrillItem.center`, DrillItem.java:28) that `translateBy` mutates,
//! and for a `Pin` its `null`/non-`null` state is observable through `translateBy`.

use std::cell::{Cell, RefCell};

use fr_geometry::{
    Direction, FloatPoint, IntBox, IntPoint, Line, Point, Polyline, Shape, ShapeOps, TileShape,
    Vector, java_min,
};

use crate::ids::{ItemId, PadstackId, TreeId};
use crate::items::header::ItemHeader;
use crate::items::{Connectable, copied_header};
use crate::library::{BoardLibrary, Package, Padstack};
use crate::rules::{BoardRules, Nets};
use crate::structure::{Component, Components};

/// The three board fields Java's `DrillItem`/`Via`/`Pin` reach through `Item.board`.
///
/// Not a Java type: Java writes `this.board.library`, `this.board.components` and
/// `this.board.rules`/`this.board.layerStructure` directly. `global-constraints.md` forbids the
/// back-pointer, so the same three are passed in.
///
/// The layer structure is reached as `rules.layer_structure()`: Java's `board.layerStructure`
/// and `board.rules.layerStructure` are the same stack (`BasicBoard`'s constructor hands one
/// object to both), which is why this struct has three fields and not four.
#[derive(Debug, Clone, Copy)]
pub struct ItemCtx<'a> {
    /// Java `board.library` (`BasicBoard.library`).
    pub library: &'a BoardLibrary,
    /// Java `board.components` (`BasicBoard.components`).
    pub components: &'a Components,
    /// Java `board.rules` (`BasicBoard.rules`), which also carries `board.layerStructure`.
    pub rules: &'a BoardRules,
}

impl<'a> ItemCtx<'a> {
    /// Java's `board.components.get(componentId)` with Java's `null` semantics.
    ///
    /// [`Components::get`] panics outside `1..=count`, faithfully to Java's
    /// `Vector.elementAt(id - 1)`. But `Pin.isPlacedOnFront` (Pin.java:484-487),
    /// `Pin.getPadstack` (Pin.java:124-128), `Pin.name` (Pin.java:151-155) and
    /// `Pin.getTraceExitRestrictions` (Pin.java:272-277) all branch on
    /// `component == null` — and Java only ever hands them a `componentId` of `0` (an item that
    /// belongs to no component), for which `elementAt(-1)` throws rather than returning `null`.
    /// Java's null branches are therefore reachable only through a corrupted board; this helper
    /// makes them reachable here too, by bounds-checking first and answering `None` instead of
    /// panicking.
    fn component(&self, component_id: i32) -> Option<&'a Component> {
        if component_id < 1 || component_id as usize > self.components.count() {
            return None;
        }
        Some(self.components.get(component_id))
    }
}

/// Port of the state `DrillItem` (`board/model/items/DrillItem.java`) adds to `Item`: the centre
/// point plus three memo fields.
///
/// Embedded in [`Via`] and [`Pin`] as `drill`, the way [`ItemHeader`] is embedded as `hdr`.
#[derive(Debug, Default)]
pub struct DrillItemData {
    /// Java `private Point center` (DrillItem.java:28). `None` is Java's `null`, which is what a
    /// [`Pin`] starts with (Pin.java:59 passes `null` to `super`) until
    /// [`Pin::get_center`] fills it in.
    center: RefCell<Option<Point>>,
    /// Java `private double precalculatedMinWidth = -1` (DrillItem.java:34). `None` is Java's
    /// "< 0, not yet calculated" sentinel.
    min_width: Cell<Option<f64>>,
    /// Java `private int precalculatedFirstLayer = -1` (DrillItem.java:40).
    first_layer: Cell<Option<usize>>,
    /// Java `private int precalculatedLastLayer = -1` (DrillItem.java:46).
    last_layer: Cell<Option<usize>>,
}

impl Clone for DrillItemData {
    fn clone(&self) -> DrillItemData {
        DrillItemData {
            center: RefCell::new(self.center.borrow().clone()),
            min_width: self.min_width.clone(),
            first_layer: self.first_layer.clone(),
            last_layer: self.last_layer.clone(),
        }
    }
}

/// Compares only `center` — the three memo fields are derived state (see the module doc).
impl PartialEq for DrillItemData {
    fn eq(&self, other: &DrillItemData) -> bool {
        *self.center.borrow() == *other.center.borrow()
    }
}

impl DrillItemData {
    /// A drill item centred on `center`.
    pub fn new(center: Option<Point>) -> DrillItemData {
        DrillItemData {
            center: RefCell::new(center),
            min_width: Cell::new(None),
            first_layer: Cell::new(None),
            last_layer: Cell::new(None),
        }
    }

    /// Port of `DrillItem.getCenter` (DrillItem.java:228-231): the raw field, which is `None`
    /// for a [`Pin`] whose centre has not been calculated yet.
    // renamed: `DrillItem.getCenter` -> `DrillItemData::raw_center`, because `Pin` overrides
    // `getCenter` with a lazy calculation and both are needed (Java reaches this one through
    // `super.getCenter()`, Pin.java:93).
    pub fn raw_center(&self) -> Option<Point> {
        self.center.borrow().clone()
    }

    /// Port of the protected `DrillItem.setCenter` (DrillItem.java:233-235).
    pub fn set_center(&self, center: Option<Point>) {
        *self.center.borrow_mut() = center;
    }

    /// Port of `DrillItem.clearDerivedData` (DrillItem.java:390-395): drop the two layer memos.
    ///
    // Java bug: `precalculatedMinWidth` is **not** reset here, although it is derived from the
    // same padstack the layer memos are. `Via.changePlacementSide` (Via.java:189-201) swaps in
    // a differently shaped padstack and then calls this, so a via keeps the minimum width of
    // its old padstack for the rest of its life. Reproduced; see docs/java-quirks.md.
    fn clear_derived_data(&self) {
        self.first_layer.set(None);
        self.last_layer.set(None);
    }
}

// ---------------------------------------------------------------------------------------------
// The shared `DrillItem` bodies.
//
// Java puts these on the abstract base and calls three abstract methods from them
// (`getPadstack`, `getShape`, `isPlacedOnFront`). The port makes that explicit as a private
// trait, and the public inherent methods on `Via`/`Pin` forward to the free functions below —
// so `Via::first_layer` and `Pin::first_layer` are literally one Java body, not two copies.
// ---------------------------------------------------------------------------------------------

trait DrillItemBase {
    fn drill(&self) -> &DrillItemData;

    /// Port of the abstract `DrillItem.getPadstack` (DrillItem.java:238).
    fn padstack_of<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a Padstack>;

    /// Port of the abstract `DrillItem.getShape(int)` (DrillItem.java:188).
    fn shape_of(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<Shape>;

    /// Port of `DrillItem.isPlacedOnFront` (DrillItem.java:363-366) and `Pin`'s override
    /// (Pin.java:480-489).
    fn placed_on_front(&self, ctx: &ItemCtx<'_>) -> bool;
}

/// Java's `int` layer numbers are `usize` here (the Task 5 signature). A padstack with no shape
/// on any layer makes `Padstack.fromLayer()` return the layer count and `toLayer()` return `-1`
/// (Padstack.java:137-152), which drives every expression below negative; Java then returns the
/// negative number and misbehaves downstream (`Via.getShape` allocates an array of negative
/// length and throws `NegativeArraySizeException`). Such a padstack is unreachable — the DSN
/// reader synthesises copper for a drill-only padstack precisely so that it cannot happen
/// (`Padstack.holeOnly`) — so the port fails loudly instead of picking a value Java never
/// produces.
fn layer_index(value: i32, what: &str) -> usize {
    usize::try_from(value).unwrap_or_else(|_| {
        panic!(
            "{what}: negative layer index {value} — the padstack has no shape on any layer \
             (Padstack.java:137-152); Java returns the negative number and then throws \
             downstream"
        )
    })
}

/// Port of `DrillItem.firstLayer` (DrillItem.java:161-172).
fn first_layer_of<D: DrillItemBase>(item: &D, ctx: &ItemCtx<'_>) -> usize {
    if let Some(cached) = item.drill().first_layer.get() {
        return cached;
    }
    let padstack = item
        .padstack_of(ctx)
        .expect("DrillItem.firstLayer: Java NPEs on a null padstack (DrillItem.java:164-166)");
    let value = if item.placed_on_front(ctx) || padstack.placed_absolute {
        padstack.from_layer()
    } else {
        padstack.board_layer_count() as i32 - padstack.to_layer() - 1
    };
    let value = layer_index(value, "DrillItem.firstLayer");
    item.drill().first_layer.set(Some(value));
    value
}

/// Port of `DrillItem.lastLayer` (DrillItem.java:174-186).
fn last_layer_of<D: DrillItemBase>(item: &D, ctx: &ItemCtx<'_>) -> usize {
    if let Some(cached) = item.drill().last_layer.get() {
        return cached;
    }
    let padstack = item
        .padstack_of(ctx)
        .expect("DrillItem.lastLayer: Java NPEs on a null padstack (DrillItem.java:177-179)");
    let value = if item.placed_on_front(ctx) || padstack.placed_absolute {
        padstack.to_layer()
    } else {
        padstack.board_layer_count() as i32 - padstack.from_layer() - 1
    };
    let value = layer_index(value, "DrillItem.lastLayer");
    item.drill().last_layer.set(Some(value));
    value
}

/// Port of `DrillItem.tileShapeCount` (DrillItem.java:202-208): the padstack's own layer span,
/// read straight off the padstack rather than through `firstLayer()`/`lastLayer()`. The two
/// agree — mirroring a back-placed padstack maps `[from, to]` to
/// `[n - to - 1, n - from - 1]`, which is the same width — so this is a shortcut, not a
/// different number.
fn tile_shape_count_of<D: DrillItemBase>(item: &D, ctx: &ItemCtx<'_>) -> usize {
    let padstack = item
        .padstack_of(ctx)
        .expect("DrillItem.tileShapeCount: Java NPEs on a null padstack (DrillItem.java:204)");
    layer_index(
        padstack.to_layer() - padstack.from_layer() + 1,
        "DrillItem.tileShapeCount",
    )
}

/// Port of `DrillItem.boundingBox` (DrillItem.java:190-200).
fn bounding_box_of<D: DrillItemBase>(item: &D, ctx: &ItemCtx<'_>) -> IntBox {
    let mut result = IntBox::EMPTY;
    for i in 0..tile_shape_count_of(item, ctx) {
        if let Some(shape) = item.shape_of(i, ctx) {
            result = result.union(&shape.bounding_box());
        }
    }
    result
}

/// Port of `DrillItem.smallestRadius` (DrillItem.java:215-226).
fn smallest_radius_of<D: DrillItemBase>(item: &D, center: &Point, ctx: &ItemCtx<'_>) -> f64 {
    let mut result = f64::MAX;
    let c = center.to_float();
    for i in 0..tile_shape_count_of(item, ctx) {
        if let Some(shape) = item.shape_of(i, ctx) {
            result = java_min(result, shape.border_distance(&c));
        }
    }
    result
}

/// Port of `DrillItem.minWidth` (DrillItem.java:368-388).
///
/// Java's `this.board != null` guard on the signal-layer test (DrillItem.java:375) is always
/// true for an item that is on a board, which is the only way this method is reached, so the
/// port always applies the test.
fn min_width_of<D: DrillItemBase>(item: &D, ctx: &ItemCtx<'_>) -> f64 {
    if let Some(cached) = item.drill().min_width.get() {
        return cached;
    }
    let mut min_width = f64::from(i32::MAX);
    let begin_layer = first_layer_of(item, ctx);
    let end_layer = last_layer_of(item, ctx);
    for current_layer in begin_layer..=end_layer {
        if !ctx.rules.layer_structure().layers[current_layer].is_signal {
            continue;
        }
        if let Some(shape) = shape_on_layer_of(item, current_layer, ctx) {
            let bounding_box = shape.bounding_box();
            min_width = java_min(min_width, f64::from(bounding_box.width()));
            min_width = java_min(min_width, f64::from(bounding_box.height()));
        }
    }
    item.drill().min_width.set(Some(min_width));
    min_width
}

/// Port of `DrillItem.getShapeOnLayer` (DrillItem.java:262-271). Java's out-of-range branch
/// warns and returns `null`.
fn shape_on_layer_of<D: DrillItemBase>(item: &D, layer: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
    let from_layer = first_layer_of(item, ctx);
    let to_layer = last_layer_of(item, ctx);
    if layer < from_layer || layer > to_layer {
        return None;
    }
    item.shape_of(layer - from_layer, ctx)
}

/// Port of `DrillItem.shapeLayer` (DrillItem.java:147-154): `firstLayer() + index`, with `index`
/// clamped into `0 ..= lastLayer() - firstLayer()`. Java's `Math.max(index, 0)` is free here
/// (`usize`); its `Math.min` is the `min` below.
fn shape_layer_of<D: DrillItemBase>(item: &D, index: usize, ctx: &ItemCtx<'_>) -> usize {
    let from_layer = first_layer_of(item, ctx);
    let to_layer = last_layer_of(item, ctx);
    from_layer + index.min(to_layer - from_layer)
}

/// Port of `DrillItem.getTraceConnectionShape` (DrillItem.java:358-362): the centre as a
/// degenerate tile.
fn trace_connection_shape_of(center: &Point) -> TileShape {
    TileShape::Box(TileShape::get_instance_from_point(center))
}

// ---------------------------------------------------------------------------------------------
// Via
// ---------------------------------------------------------------------------------------------

/// Port of `Via` (`board/model/items/Via.java`), a `DrillItem` whose geometry is a padstack
/// placed at a point.
///
/// not ported: `Via.getAutorouteDrillInfo` (Via.java:203-217) and the `autorouteDrillInfo` field
/// it memoises (Via.java:52) — see the `added in Plan 6:` marker on
/// [`Via::clear_autoroute_drill_info`].
#[derive(Debug, Clone)]
pub struct Via {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// The `DrillItem` base-class state (DrillItem.java:28-46).
    pub drill: DrillItemData,
    /// Java `private Padstack padstack` (Via.java:48), stored as the padstack's id rather than
    /// an object reference (the Plan 2 "no object references between model objects" rule).
    padstack: PadstackId,
    /// Java `public final boolean attachAllowed` (Via.java:32): copper sharing with SMD pins of
    /// the same net. Read by `Via.isObstacle` (Via.java:165).
    pub attach_allowed: bool,
    /// Java `public boolean isEscapeVia` (Via.java:40).
    pub is_escape_via: bool,
    /// Java `public int escapeViaSmdLayer = -1` (Via.java:46).
    pub escape_via_smd_layer: i32,
    /// Java `private transient Shape[] precalculatedShapes` (Via.java:49).
    shapes: RefCell<Option<Vec<Option<Shape>>>>,
}

/// Skips `shapes`, which is pure memoisation (see the module doc).
impl PartialEq for Via {
    fn eq(&self, other: &Via) -> bool {
        self.hdr == other.hdr
            && self.drill == other.drill
            && self.padstack == other.padstack
            && self.attach_allowed == other.attach_allowed
            && self.is_escape_via == other.is_escape_via
            && self.escape_via_smd_layer == other.escape_via_smd_layer
    }
}

impl DrillItemBase for Via {
    fn drill(&self) -> &DrillItemData {
        &self.drill
    }

    fn padstack_of<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a Padstack> {
        ctx.library.get_padstack(self.padstack)
    }

    fn shape_of(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
        self.get_shape(index, ctx)
    }

    /// `Via` inherits `DrillItem.isPlacedOnFront` (DrillItem.java:363-366), whose whole body is
    /// `return true`.
    fn placed_on_front(&self, _ctx: &ItemCtx<'_>) -> bool {
        true
    }
}

impl Via {
    /// Port of the `Via(Padstack, Point, int[], int, int, int, FixedState, boolean, BasicBoard)`
    /// constructor (Via.java:54-68).
    pub fn new(hdr: ItemHeader, padstack: PadstackId, center: Point, attach_allowed: bool) -> Via {
        Via {
            hdr,
            drill: DrillItemData::new(Some(center)),
            padstack,
            attach_allowed,
            // Via.java:40,46: the two escape-via fields default to `false` / `-1`.
            is_escape_via: false,
            escape_via_smd_layer: -1,
            shapes: RefCell::new(None),
        }
    }

    /// Port of `Via.copy` (Via.java:70-86), including the two escape-via fields it carries over
    /// after construction (Via.java:83-84).
    pub fn copy(&self, new_id: ItemId) -> Via {
        let mut copy = Via::new(
            copied_header(&self.hdr, new_id),
            self.padstack,
            self.get_center(),
            self.attach_allowed,
        );
        copy.is_escape_via = self.is_escape_via;
        copy.escape_via_smd_layer = self.escape_via_smd_layer;
        copy
    }

    /// Port of `DrillItem.getCenter` (DrillItem.java:228-231). A via is always constructed with
    /// a centre, so unlike [`Pin::get_center`] this one cannot be `null`.
    pub fn get_center(&self) -> Point {
        self.drill
            .raw_center()
            .expect("a Via is always constructed with a centre (Via.java:65)")
    }

    /// The via's padstack id — the port's stand-in for Java's `Padstack` reference.
    ///
    /// Not a Java method; [`Via::get_padstack`] is the port of `Via.getPadstack`.
    pub fn get_padstack_id(&self) -> PadstackId {
        self.padstack
    }

    /// Port of `Via.getPadstack` (Via.java:137-140). `None` where Java has `null`, which for a
    /// via means an id the library does not know.
    pub fn get_padstack<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a Padstack> {
        ctx.library.get_padstack(self.padstack)
    }

    /// Port of `Via.setPadstack` (Via.java:142-144).
    pub fn set_padstack(&mut self, padstack: PadstackId) {
        self.padstack = padstack;
    }

    /// Port of `Via.getShape(int)` (Via.java:114-135): the padstack shape of layer `index`
    /// translated to the via centre. Java's `padstack == null` branch warns and returns `null`.
    ///
    /// Note Java's index arithmetic (Via.java:123): the cached array is indexed relative to the
    /// padstack's own `fromLayer`, but the *padstack* layer it reads is `i + firstLayer()`. For
    /// a via those agree, because `DrillItem.isPlacedOnFront` is unconditionally true and
    /// `firstLayer()` is therefore always `padstack.fromLayer()`.
    //
    // totalized: Java's `return this.precalculatedShapes[index]` (Via.java:134) throws an
    // `ArrayIndexOutOfBoundsException` for an index past the padstack's layer span; this
    // returns `None`. No reachable caller observes the difference — `boundingBox`,
    // `smallestRadius` and `Pin.getCenter` all loop over `0 .. tileShapeCount()`, and
    // `getShapeOnLayer`/`getTraceExitRestrictions` range-check the layer first. See
    // docs/java-quirks.md.
    pub fn get_shape(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
        let padstack = self.get_padstack(ctx)?;
        if self.shapes.borrow().is_none() {
            let count = layer_index(
                padstack.to_layer() - padstack.from_layer() + 1,
                "Via.getShape",
            );
            let translate_vector = self.get_center().difference_by(&Point::ZERO);
            let first_layer = first_layer_of(self, ctx) as i32;
            let mut shapes = Vec::with_capacity(count);
            for i in 0..count {
                let padstack_layer = i as i32 + first_layer;
                shapes.push(
                    padstack
                        .get_shape(padstack_layer)
                        .map(|shape| shape.translate_by(&translate_vector)),
                );
            }
            *self.shapes.borrow_mut() = Some(shapes);
        }
        self.shapes
            .borrow()
            .as_ref()
            .expect("just filled")
            .get(index)
            .cloned()
            .flatten()
    }

    /// Port of `DrillItem.getShapeOnLayer` (DrillItem.java:262-271).
    pub fn get_shape_on_layer(&self, layer: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
        shape_on_layer_of(self, layer, ctx)
    }

    /// Port of `DrillItem.getTileShapeOnLayer` (DrillItem.java:251-260).
    ///
    /// `tree` replaces Java's implicit `board.searchTreeManager.getDefaultTree()`: Java's
    /// `getTileShape(int)` (Item.java:194-201) resolves it from the board.
    pub fn get_tile_shape_on_layer(
        &self,
        tree: TreeId,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<&TileShape> {
        self.get_tree_shape_on_layer(tree, layer, ctx)
    }

    /// Port of `DrillItem.getTreeShapeOnLayer` (DrillItem.java:240-249).
    pub fn get_tree_shape_on_layer(
        &self,
        tree: TreeId,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<&TileShape> {
        let from_layer = self.first_layer(ctx);
        if layer < from_layer || layer > self.last_layer(ctx) {
            return None;
        }
        self.hdr
            .get_precalculated_tree_shapes(tree)
            .and_then(|shapes| shapes.get(layer - from_layer))
    }

    /// Port of `DrillItem.firstLayer` (DrillItem.java:161-172).
    pub fn first_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        first_layer_of(self, ctx)
    }

    /// Port of `DrillItem.lastLayer` (DrillItem.java:174-186).
    pub fn last_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        last_layer_of(self, ctx)
    }

    /// Port of `DrillItem.isOnLayer` (DrillItem.java:156-159).
    pub fn is_on_layer(&self, layer: usize, ctx: &ItemCtx<'_>) -> bool {
        layer >= self.first_layer(ctx) && layer <= self.last_layer(ctx)
    }

    /// Port of `DrillItem.shapeLayer` (DrillItem.java:147-154).
    pub fn shape_layer(&self, index: usize, ctx: &ItemCtx<'_>) -> usize {
        shape_layer_of(self, index, ctx)
    }

    /// Port of `DrillItem.isPlacedOnFront` (DrillItem.java:363-366).
    pub fn is_placed_on_front(&self, _ctx: &ItemCtx<'_>) -> bool {
        true
    }

    /// Port of `DrillItem.boundingBox` (DrillItem.java:190-200).
    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        bounding_box_of(self, ctx)
    }

    /// Port of `DrillItem.tileShapeCount` (DrillItem.java:202-208).
    pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        tile_shape_count_of(self, ctx)
    }

    /// Port of `DrillItem.smallestRadius` (DrillItem.java:215-226).
    pub fn smallest_radius(&self, ctx: &ItemCtx<'_>) -> f64 {
        smallest_radius_of(self, &self.get_center(), ctx)
    }

    /// Port of `DrillItem.minWidth` (DrillItem.java:368-388).
    pub fn min_width(&self, ctx: &ItemCtx<'_>) -> f64 {
        min_width_of(self, ctx)
    }

    /// Port of `DrillItem.translateBy` (DrillItem.java:61-68).
    pub fn translate_by(&mut self, vector: &Vector) {
        let translated = self.drill.raw_center().map(|c| c.translate_by(vector));
        if translated.is_some() {
            self.drill.set_center(translated);
        }
        self.clear_derived_data();
    }

    /// Port of `DrillItem.turn90Degree` (DrillItem.java:70-76).
    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        let turned = self
            .drill
            .raw_center()
            .map(|c| c.turn_90_degree(factor, &Point::Int(*pole)));
        if turned.is_some() {
            self.drill.set_center(turned);
        }
        self.clear_derived_data();
    }

    /// Port of `DrillItem.rotateApprox` (DrillItem.java:78-85).
    pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint) {
        let rotated = self.drill.raw_center().map(|c| {
            Point::Int(
                c.to_float()
                    .rotate(angle_in_degree.to_radians(), pole)
                    .round(),
            )
        });
        if rotated.is_some() {
            self.drill.set_center(rotated);
        }
        self.clear_derived_data();
    }

    /// Port of `Via.changePlacementSide` (Via.java:189-201), which swaps in the mirrored via
    /// padstack before running `DrillItem.changePlacementSide` (DrillItem.java:87-93).
    ///
    /// Java's `this.board == null` guard (Via.java:191-193) is dropped: with the board reached
    /// through [`ItemCtx`] there is no null case. Its second guard — no mirrored padstack, so
    /// leave the via alone entirely, *without* mirroring its centre — is kept.
    pub fn change_placement_side(&mut self, pole: &IntPoint, ctx: &ItemCtx<'_>) {
        let Some(new_padstack) = ctx.library.get_mirrored_via_padstack(self.padstack) else {
            return;
        };
        self.padstack = new_padstack;
        // DrillItem.changePlacementSide (DrillItem.java:87-93).
        let mirrored = self
            .drill
            .raw_center()
            .map(|c| c.mirror_vertical(&Point::Int(*pole)));
        if mirrored.is_some() {
            self.drill.set_center(mirrored);
        }
        self.clear_derived_data();
        // Via.java:200 calls `clearDerivedData()` a second time; harmless and reproduced by the
        // single call above.
    }

    /// Port of `Via.clearDerivedData` (Via.java:219-224): the `DrillItem` body plus this via's
    /// cached shapes. The `Item`-level half (the search-tree caches) is
    /// [`ItemHeader::clear_derived_data`], which [`super::Item::clear_derived_data`] calls.
    // added in Plan 6: `autorouteDrillInfo = null` (Via.java:223) — see
    // [`Via::clear_autoroute_drill_info`].
    pub fn clear_derived_data(&mut self) {
        self.hdr.clear_derived_data();
        self.drill.clear_derived_data();
        *self.shapes.borrow_mut() = None;
        self.clear_autoroute_drill_info();
    }

    /// The `autorouteDrillInfo = null` half of `Via.clearDerivedData` (Via.java:223) and
    /// `Via.clearAutorouteInfo` (Via.java:226-230).
    // added in Plan 6: `Via.getAutorouteDrillInfo` (Via.java:203-217) and the
    // `autorouteDrillInfo` field (Via.java:52). Both are `ExpansionDrill`, an
    // `app.freerouting.autoroute.drill` type that Plan 6 introduces; until then there is no
    // cache to drop and this method is empty.
    pub fn clear_autoroute_drill_info(&self) {}

    /// Port of `Via.clearAutorouteInfo` (Via.java:226-230).
    pub fn clear_autoroute_info(&mut self) {
        self.hdr.clear_autoroute_info();
        self.clear_autoroute_drill_info();
    }
}

impl Connectable for Via {
    fn header(&self) -> &ItemHeader {
        &self.hdr
    }

    /// Port of `DrillItem.getTraceConnectionShape` (DrillItem.java:358-362), which ignores both
    /// arguments and answers the centre point.
    fn get_trace_connection_shape(
        &self,
        _tree: TreeId,
        _index: usize,
        _ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        Some(trace_connection_shape_of(&self.get_center()))
    }
}

// ---------------------------------------------------------------------------------------------
// Pin
// ---------------------------------------------------------------------------------------------

/// Port of `Pin` (`board/model/items/Pin.java`), a `DrillItem` whose padstack, location and
/// rotation all come from its component's library package.
///
/// The component is `hdr.get_component_id()`: Java's `Pin` constructor passes `componentId` as
/// the `Item` `groupId` (Pin.java:59), so `getComponentId()` is the pin's component.
#[derive(Debug, Clone)]
pub struct Pin {
    /// The `Item` base-class state (Item.java:41-67); its component id is the pin's component.
    pub hdr: ItemHeader,
    /// The `DrillItem` base-class state (DrillItem.java:28-46). A pin starts with **no** centre
    /// (Pin.java:59 passes `null`); [`Pin::get_center`] fills it in on first use.
    pub drill: DrillItemData,
    /// Java `public final int pinIndex` (Pin.java:40): the index of this pin in its component's
    /// package, starting at 0.
    pin_index: i32,
    /// Java `private Pin changedTo = this` (Pin.java:43): the pin this one was swapped with.
    ///
    /// Java's "no swap yet" value is a self-reference; this port spells that `None`, because an
    /// [`ItemId`] pointing at the pin's own id would be a second spelling of the same state that
    /// [`Pin::swap`] would have to keep normalising. [`Pin::get_changed_to`] resolves it.
    changed_to: Option<ItemId>,
    /// Java `private transient Shape[] precalculatedShapes` (Pin.java:45).
    shapes: RefCell<Option<Vec<Option<Shape>>>>,
}

/// Skips `shapes`, which is pure memoisation (see the module doc).
impl PartialEq for Pin {
    fn eq(&self, other: &Pin) -> bool {
        self.hdr == other.hdr
            && self.drill == other.drill
            && self.pin_index == other.pin_index
            && self.changed_to == other.changed_to
    }
}

impl DrillItemBase for Pin {
    fn drill(&self) -> &DrillItemData {
        &self.drill
    }

    fn padstack_of<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a Padstack> {
        self.get_padstack(ctx)
    }

    fn shape_of(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
        self.get_shape(index, ctx)
    }

    fn placed_on_front(&self, ctx: &ItemCtx<'_>) -> bool {
        self.is_placed_on_front(ctx)
    }
}

impl Pin {
    /// Port of the `Pin(int, int, int[], int, int, FixedState, BasicBoard)` constructor
    /// (Pin.java:51-62). Java passes `null` for the centre (Pin.java:59); the component id
    /// travels in `hdr`.
    pub fn new(hdr: ItemHeader, pin_index: i32) -> Pin {
        Pin {
            hdr,
            drill: DrillItemData::new(None),
            pin_index,
            changed_to: None,
            shapes: RefCell::new(None),
        }
    }

    /// Port of `Pin.copy` (Pin.java:133-147). The new pin's `changedTo` is its own
    /// self-reference again, because Java builds it with the constructor.
    pub fn copy(&self, new_id: ItemId) -> Pin {
        Pin::new(copied_header(&self.hdr, new_id), self.pin_index)
    }

    /// Port of `Pin.getPinIndex` (Pin.java:159-162).
    pub fn get_pin_index(&self) -> i32 {
        self.pin_index
    }

    /// The pin's component, or `None` where Java's `board.components.get(componentId)` would be
    /// `null` — see [`ItemCtx::component`].
    fn component<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a Component> {
        ctx.component(self.hdr.get_component_id())
    }

    /// The pin's library package: `component.getPackage()` (Pin.java:67, 124, 177, …).
    fn package<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a Package> {
        let component = self.component(ctx)?;
        Some(ctx.library.get_package(component.get_package()))
    }

    /// Port of `Pin.isPlacedOnFront` (Pin.java:480-489): the component's side, or `true` when
    /// the component is missing.
    pub fn is_placed_on_front(&self, ctx: &ItemCtx<'_>) -> bool {
        self.component(ctx).is_none_or(Component::placed_on_front)
    }

    /// Port of `Pin.getPadstack` (Pin.java:122-131). Java's missing-component branch warns and
    /// returns `null`.
    pub fn get_padstack<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a Padstack> {
        let package = self.package(ctx)?;
        let package_pin = package
            .get_pin(self.pin_index)
            .expect("Pin.getPadstack: Java NPEs on a pin index outside the package (Pin.java:129)");
        ctx.library.padstacks.get(package_pin.padstack_no)
    }

    /// Port of `Pin.name` (Pin.java:149-157): the pin's name in its component's package.
    ///
    /// Java's missing-component branch warns and returns `null`.
    // renamed: the Task 6 brief calls this `get_component_pin_name`; Java calls it `name`.
    pub fn name<'a>(&self, ctx: &ItemCtx<'a>) -> Option<&'a str> {
        let package = self.package(ctx)?;
        let package_pin = package
            .get_pin(self.pin_index)
            .expect("Pin.name: Java NPEs on a pin index outside the package (Pin.java:156)");
        Some(&package_pin.name)
    }

    /// Port of `Pin.relativeLocation` (Pin.java:64-89): the pin's location relative to its
    /// component's location, with the package pin's offset mirrored and rotated the way the
    /// component is placed.
    // renamed: the Task 6 brief calls this `get_package_pin_relative_location`; Java calls it
    // `relativeLocation`.
    pub fn relative_location(&self, ctx: &ItemCtx<'_>) -> Vector {
        let component = self
            .component(ctx)
            .expect("Pin.relativeLocation: Java NPEs on a missing component (Pin.java:66-67)");
        let package = ctx.library.get_package(component.get_package());
        let package_pin = package.get_pin(self.pin_index).expect(
            "Pin.relativeLocation: Java NPEs on a pin index outside the package (Pin.java:68-69)",
        );
        let mut rel_location = package_pin.relative_location.clone();
        let component_rotation = component.get_rotation_in_degree();
        // Pin.java:71-73.
        if !component.placed_on_front() && !ctx.components.get_flip_style_rotate_first() {
            rel_location = package_pin.relative_location.mirror_at_y_axis();
        }
        // Pin.java:74-84.
        if component_rotation % 90.0 == 0.0 {
            let factor = component_rotation as i32 / 90;
            if factor != 0 {
                rel_location = rel_location.turn_90_degree(factor);
            }
        } else {
            let location_approx = rel_location
                .to_float()
                .rotate(component_rotation.to_radians(), &FloatPoint::ZERO);
            rel_location = Point::Int(location_approx.round()).difference_by(&Point::ZERO);
        }
        // Pin.java:85-87.
        if !component.placed_on_front() && ctx.components.get_flip_style_rotate_first() {
            rel_location = rel_location.mirror_at_y_axis();
        }
        rel_location
    }

    /// Port of `Pin.getCenter` (Pin.java:91-120): the component's location translated by
    /// [`Pin::relative_location`], corrected to the centre of gravity of the first non-`null`
    /// pad shape when that point is not strictly inside it. Memoised into `DrillItem.center`.
    pub fn get_center(&self, ctx: &ItemCtx<'_>) -> Point {
        if let Some(center) = self.drill.raw_center() {
            return center;
        }
        let component = self
            .component(ctx)
            .expect("Pin.getCenter: Java NPEs on a missing component (Pin.java:97-98)");
        let location = component
            .get_location()
            .expect("Pin.getCenter: Java NPEs on an unplaced component (Pin.java:98)");
        let mut pin_center = location.translate_by(&self.relative_location(ctx));

        // Pin.java:100-116: check that the pin centre is inside the pin shape, correct it if not.
        let padstack = self
            .get_padstack(ctx)
            .expect("Pin.getCenter: Java NPEs on a null padstack (Pin.java:102-104)");
        let count = layer_index(
            padstack.to_layer() - padstack.from_layer() + 1,
            "Pin.getCenter",
        );
        let current_shape = (0..count).find_map(|i| self.get_shape(i, ctx));
        match current_shape {
            // Pin.java:112-113 only warns; the centre stays as calculated.
            None => {}
            Some(shape) if !shape.contains_inside(&pin_center) => {
                pin_center = Point::Int(shape.centre_of_gravity().round());
            }
            Some(_) => {}
        }
        self.drill.set_center(Some(pin_center.clone()));
        pin_center
    }

    /// Port of the package-private `Pin.getPadstackLayer` (Pin.java:247-258): the padstack layer
    /// that the shape with index `index` comes from.
    ///
    /// `index` is `i32` because Java's callers pass `layer - firstLayer()`, which is negative for
    /// a layer below the pin (Pin.java:267, 493, 518).
    pub fn get_padstack_layer(&self, index: i32, ctx: &ItemCtx<'_>) -> i32 {
        let padstack = self
            .get_padstack(ctx)
            .expect("Pin.getPadstackLayer: Java NPEs on a null padstack (Pin.java:249)");
        let component = self
            .component(ctx)
            .expect("Pin.getPadstackLayer: Java NPEs on a missing component (Pin.java:250-252)");
        let first_layer = first_layer_of(self, ctx) as i32;
        if component.placed_on_front() || padstack.placed_absolute {
            index + first_layer
        } else {
            padstack.board_layer_count() as i32 - index - first_layer - 1
        }
    }

    /// Port of `Pin.getShape(int)` (Pin.java:164-245): the padstack shape, rotated by the
    /// package pin's own rotation, mirrored and rotated with the component, and finally
    /// translated to the component's location.
    ///
    /// Java computes **all** the shapes on the first call ("otherwise calculation of fromLayer
    /// and toLayer may not be correct", Pin.java:168-169) and this port does the same. Java's
    /// three missing-data branches (no component, no package, no package pin) each warn and
    /// return `null` — after having already replaced the cache with an all-`null` array, so a
    /// later call on a repaired board keeps answering `null`; the port returns `None` from the
    /// same three places without installing the cache, which no reachable caller can tell apart
    /// (the three are unreachable for a pin that is on a board at all).
    //
    // Java bug: the three branches install the all-`null` cache before bailing out, so a later
    // call on a repaired board keeps answering `null` — see quirk #53 in docs/java-quirks.md.
    //
    // totalized: as for `Via::get_shape` — Java's `precalculatedShapes[index]`
    // (Pin.java:244) throws out of range, this returns `None`. See docs/java-quirks.md.
    //
    // Java calls `getPadstack()` at Pin.java:166, *outside* the cache guard, so a cache hit
    // still pays for (and can NPE on) the component -> package -> padstack walk. The lookup is
    // inside the guard here: filling the cache is the only thing that needs it, and a filled
    // cache implies the walk already succeeded — a pin's padstack is fixed by its package, and
    // every mutator that could invalidate it drops the cache too.
    pub fn get_shape(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
        if self.shapes.borrow().is_none() {
            let padstack = self
                .get_padstack(ctx)
                .expect("Pin.getShape: Java NPEs on a null padstack (Pin.java:166,170)");
            let count = layer_index(
                padstack.to_layer() - padstack.from_layer() + 1,
                "Pin.getShape",
            );
            let component = self.component(ctx)?;
            let package = ctx.library.get_package(component.get_package());
            let package_pin = package.get_pin(self.pin_index)?;

            let component_rotation = component.get_rotation_in_degree();
            // Pin.java:190-195.
            let mirror_on_y_axis =
                !component.placed_on_front() && !ctx.components.get_flip_style_rotate_first();
            let rel_location = if mirror_on_y_axis {
                package_pin.relative_location.mirror_at_y_axis()
            } else {
                package_pin.relative_location.clone()
            };
            let component_translation = component
                .get_location()
                .expect("Pin.getShape: Java NPEs on an unplaced component (Pin.java:197)")
                .difference_by(&Point::ZERO);

            let mut shapes: Vec<Option<Shape>> = Vec::with_capacity(count);
            for shape_index in 0..count {
                let padstack_layer = self.get_padstack_layer(shape_index as i32, ctx);
                // Pin.java:203-206: a layer without copper leaves a `null` in the array.
                let Some(base) = padstack.get_shape(padstack_layer) else {
                    shapes.push(None);
                    continue;
                };
                // Pin.java:207-217: the package pin's own rotation, around the origin.
                let pin_rotation = package_pin.rotation_in_degree;
                let mut current_shape = if pin_rotation % 90.0 == 0.0 {
                    let factor = pin_rotation as i32 / 90;
                    if factor == 0 {
                        base.clone()
                    } else {
                        base.turn_90_degree(factor, &IntPoint::ZERO)
                    }
                } else {
                    base.rotate_approx(pin_rotation.to_radians(), &FloatPoint::ZERO)
                };
                // Pin.java:219-221.
                if mirror_on_y_axis {
                    current_shape = current_shape.mirror_vertical(&IntPoint::ZERO);
                }
                // Pin.java:223-224: translate relative to the component first.
                let mut translated_shape = current_shape.translate_by(&rel_location);
                // Pin.java:226-236: then the component's rotation, still around the origin.
                if component_rotation % 90.0 == 0.0 {
                    let factor = component_rotation as i32 / 90;
                    if factor != 0 {
                        translated_shape = translated_shape.turn_90_degree(factor, &IntPoint::ZERO);
                    }
                } else {
                    translated_shape = translated_shape
                        .rotate_approx(component_rotation.to_radians(), &FloatPoint::ZERO);
                }
                // Pin.java:237-239.
                if !component.placed_on_front() && ctx.components.get_flip_style_rotate_first() {
                    translated_shape = translated_shape.mirror_vertical(&IntPoint::ZERO);
                }
                // Pin.java:240-241.
                shapes.push(Some(translated_shape.translate_by(&component_translation)));
            }
            *self.shapes.borrow_mut() = Some(shapes);
        }
        self.shapes
            .borrow()
            .as_ref()
            .expect("just filled")
            .get(index)
            .cloned()
            .flatten()
    }

    /// Port of `DrillItem.getShapeOnLayer` (DrillItem.java:262-271).
    pub fn get_shape_on_layer(&self, layer: usize, ctx: &ItemCtx<'_>) -> Option<Shape> {
        shape_on_layer_of(self, layer, ctx)
    }

    /// Port of `DrillItem.getTileShapeOnLayer` (DrillItem.java:251-260); see
    /// [`Via::get_tile_shape_on_layer`] for what `tree` replaces.
    pub fn get_tile_shape_on_layer(
        &self,
        tree: TreeId,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<&TileShape> {
        self.get_tree_shape_on_layer(tree, layer, ctx)
    }

    /// Port of `DrillItem.getTreeShapeOnLayer` (DrillItem.java:240-249).
    pub fn get_tree_shape_on_layer(
        &self,
        tree: TreeId,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<&TileShape> {
        let from_layer = self.first_layer(ctx);
        if layer < from_layer || layer > self.last_layer(ctx) {
            return None;
        }
        self.hdr
            .get_precalculated_tree_shapes(tree)
            .and_then(|shapes| shapes.get(layer - from_layer))
    }

    /// Port of `DrillItem.firstLayer` (DrillItem.java:161-172).
    pub fn first_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        first_layer_of(self, ctx)
    }

    /// Port of `DrillItem.lastLayer` (DrillItem.java:174-186).
    pub fn last_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        last_layer_of(self, ctx)
    }

    /// Port of `DrillItem.isOnLayer` (DrillItem.java:156-159).
    pub fn is_on_layer(&self, layer: usize, ctx: &ItemCtx<'_>) -> bool {
        layer >= self.first_layer(ctx) && layer <= self.last_layer(ctx)
    }

    /// Port of `DrillItem.shapeLayer` (DrillItem.java:147-154).
    pub fn shape_layer(&self, index: usize, ctx: &ItemCtx<'_>) -> usize {
        shape_layer_of(self, index, ctx)
    }

    /// Port of `DrillItem.boundingBox` (DrillItem.java:190-200).
    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        bounding_box_of(self, ctx)
    }

    /// Port of `DrillItem.tileShapeCount` (DrillItem.java:202-208).
    pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        tile_shape_count_of(self, ctx)
    }

    /// Port of `DrillItem.smallestRadius` (DrillItem.java:215-226).
    pub fn smallest_radius(&self, ctx: &ItemCtx<'_>) -> f64 {
        smallest_radius_of(self, &self.get_center(ctx), ctx)
    }

    /// Port of `DrillItem.minWidth` (DrillItem.java:368-388): the smallest bounding-box side of
    /// the **placed** pad over the pin's signal layers.
    pub fn min_width(&self, ctx: &ItemCtx<'_>) -> f64 {
        min_width_of(self, ctx)
    }

    /// Port of `Pin.getMinWidth(int)` (Pin.java:491-505): the smallest side of the **padstack**
    /// shape's bounding box on `layer`. Java's two `null` branches warn and return 0.
    pub fn get_min_width(&self, layer: usize, ctx: &ItemCtx<'_>) -> f64 {
        self.padstack_bounding_box(layer, ctx)
            .map_or(0.0, |b| b.min_width())
    }

    /// Port of `Pin.getMaxWidth(int)` (Pin.java:516-530).
    pub fn get_max_width(&self, layer: usize, ctx: &ItemCtx<'_>) -> f64 {
        self.padstack_bounding_box(layer, ctx)
            .map_or(0.0, |b| b.max_width())
    }

    /// The shared first half of `Pin.getMinWidth`/`getMaxWidth` (Pin.java:493-503, 518-528).
    fn padstack_bounding_box(&self, layer: usize, ctx: &ItemCtx<'_>) -> Option<IntBox> {
        let padstack_layer =
            self.get_padstack_layer(layer as i32 - first_layer_of(self, ctx) as i32, ctx);
        let padstack = self
            .get_padstack(ctx)
            .expect("Pin.getMinWidth: Java NPEs on a null padstack (Pin.java:494)");
        padstack
            .get_shape(padstack_layer)
            .map(ShapeOps::bounding_box)
    }

    /// Port of `Pin.getTraceNeckdownHalfwidth` (Pin.java:507-514): `max(0.5 * minWidth - 1, 1)`,
    /// truncated towards zero by Java's `(int)` cast.
    pub fn get_trace_neckdown_halfwidth(&self, layer: usize, ctx: &ItemCtx<'_>) -> i32 {
        let result = (0.5 * self.get_min_width(layer, ctx) - 1.0).max(1.0);
        result as i32
    }

    /// Port of `Pin.drillAllowed` (Pin.java:344-350): vias may be drilled through this pin's
    /// pads only if it is an SMD pad, i.e. its padstack lives on a single layer.
    ///
    /// Read by both `Pin.isObstacle` (Pin.java:364) and `Via.isObstacle` (Via.java:165).
    pub fn drill_allowed(&self, ctx: &ItemCtx<'_>) -> bool {
        self.first_layer(ctx) == self.last_layer(ctx)
    }

    /// Port of `Pin.getTraceExitRestrictions(int)` (Pin.java:260-331): the directions in which a
    /// trace may leave this pin's pad on `layer`, each with the distance from the pin centre to
    /// the pad border along it.
    ///
    /// Implemented, as Java's comment says, only for box-shaped pads: the direction set comes
    /// from [`Padstack::get_trace_exit_directions`] (Padstack.java:164-195), which is empty for
    /// anything that is not an `IntBox` or an `IntOctagon`.
    ///
    /// `padXyFactor` starts at 1.5 and doubles for a package with at most three pins
    /// (Pin.java:268-277) — the comment there explains that a larger factor would let the
    /// shove algorithm block the channels between the pins of an SMD component.
    ///
    /// Note that this method does **not** read `BoardRules.pinEdgeToTurnDist`; that value is
    /// consumed by [`Pin::calc_nearest_exit_restriction_direction`] and
    /// [`Pin::nearest_trace_exit_corner`], which offset the pad shape by it.
    //
    // Java bug: `getPadstackLayer` (Pin.java:267) and `getPadstack` (Pin.java:280) both
    // dereference the component *before* the `component == null` guard at Pin.java:285-287, so
    // that guard is dead code and a pin with no component NPEs instead. The order is kept, so
    // the two `.expect(...)` calls below panic exactly where Java throws. See quirk #52 in
    // docs/java-quirks.md.
    pub fn get_trace_exit_restrictions(
        &self,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> Vec<TraceExitRestriction> {
        let mut result = Vec::new();
        let first_layer = first_layer_of(self, ctx) as i32;
        let padstack_layer = self.get_padstack_layer(layer as i32 - first_layer, ctx);
        let mut pad_xy_factor = 1.5;

        // Pin.java:272-277.
        let component = self.component(ctx);
        if let Some(component) = component {
            let package = ctx.library.get_package(component.get_package());
            if package.pin_count() <= 3 {
                pad_xy_factor *= 2.0;
            }
        }

        // Pin.java:279-283.
        let padstack = self
            .get_padstack(ctx)
            .expect("Pin.getTraceExitRestrictions: Java NPEs on a null padstack (Pin.java:280)");
        let padstack_exit_directions =
            padstack.get_trace_exit_directions(padstack_layer, pad_xy_factor);
        if padstack_exit_directions.is_empty() {
            return result;
        }
        // Pin.java:285-287.
        let Some(component) = component else {
            return result;
        };
        // Pin.java:288-291: only a tile-shaped pad has border lines to measure against.
        let shape_index = layer as i32 - first_layer;
        let current_shape = usize::try_from(shape_index)
            .ok()
            .and_then(|i| self.get_shape(i, ctx));
        let Some(Shape::Tile(pad_shape)) = current_shape else {
            return result;
        };
        let component_rotation = component.get_rotation_in_degree();
        let pin_center = self.get_center(ctx);
        let center_approx = pin_center.to_float();

        for padstack_exit_direction in padstack_exit_directions {
            // Pin.java:298-305.
            let package = ctx.library.get_package(component.get_package());
            let Some(package_pin) = package.get_pin(self.pin_index) else {
                continue;
            };
            // Pin.java:306-315.
            let current_rotation_in_degree = component_rotation + package_pin.rotation_in_degree;
            let current_exit_direction = if current_rotation_in_degree % 45.0 == 0.0 {
                let fortyfive_degree_factor = current_rotation_in_degree as i32 / 45;
                padstack_exit_direction.turn_45_degree(fortyfive_degree_factor)
            } else {
                let current_angle_in_radian = current_rotation_in_degree.to_radians()
                    + padstack_exit_direction.angle_approx();
                Direction::from_angle_approx(current_angle_in_radian)
            };
            // Pin.java:316-322: Java's `< 0` "border line not found" branch warns and continues.
            let Some(intersecting_border_line_no) =
                pad_shape.intersecting_border_line_no(&pin_center, &current_exit_direction)
            else {
                continue;
            };
            // Pin.java:323-328.
            let Point::Int(int_center) = pin_center else {
                // `intersectingBorderLineNo` already returned `None` for a non-integer centre.
                continue;
            };
            let Some(current_exit_line) =
                Line::from_direction_any(int_center, &current_exit_direction)
            else {
                continue;
            };
            let border_line = pad_shape
                .border_line(intersecting_border_line_no)
                .expect("intersectingBorderLineNo returned a valid border line index");
            let nearest_border_point = current_exit_line.intersection_approx(&border_line);
            result.push(TraceExitRestriction {
                direction: current_exit_direction,
                min_length: center_approx.distance(&nearest_border_point),
            });
        }
        result
    }

    /// Port of `Pin.hasTraceExitRestrictions` (Pin.java:333-342).
    pub fn has_trace_exit_restrictions(&self, ctx: &ItemCtx<'_>) -> bool {
        (self.first_layer(ctx)..=self.last_layer(ctx))
            .any(|layer| !self.get_trace_exit_restrictions(layer, ctx).is_empty())
    }

    /// Port of `Pin.calcNearestExitRestrictionDirection` (Pin.java:565-632): the exit
    /// restriction direction whose exit corner is nearest to where `trace_polyline` enters the
    /// pad, or `None` when the pin has no exit restrictions, is not tile-shaped, the rules'
    /// `pinEdgeToTurnDist` is negative, or the polyline never enters the offset pad.
    ///
    /// `trace_polyline` is assumed to start at the pin centre.
    pub fn calc_nearest_exit_restriction_direction(
        &self,
        trace_polyline: &Polyline,
        trace_half_width: i32,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<Direction> {
        let trace_exit_restrictions = self.get_trace_exit_restrictions(layer, ctx);
        if trace_exit_restrictions.is_empty() {
            return None;
        }
        let offset_pin_shape = self.offset_pad_shape(layer, trace_half_width, ctx)?;
        let entries = offset_pin_shape.entrance_points(trace_polyline);
        let latest_entry_tuple = *entries.last()?;
        let trace_entry_location_approx = trace_polyline.lines()[latest_entry_tuple[0]]
            .intersection_approx(
                &offset_pin_shape
                    .border_line(latest_entry_tuple[1])
                    .expect("entrancePoints returns valid border line indices"),
            );

        // Pin.java:594-630: the nearest legal pin exit point to `traceEntryLocationApprox`.
        let mut min_exit_corner_distance = f64::MAX;
        let mut nearest_exit_corner: Option<FloatPoint> = None;
        let mut pin_exit_direction = None;
        const TOLERANCE: f64 = 1.0;
        let pin_center = self.get_center(ctx);
        for current_exit_restriction in &trace_exit_restrictions {
            let Some(current_exit_corner) = self.exit_corner(
                &offset_pin_shape,
                &pin_center,
                &current_exit_restriction.direction,
            ) else {
                continue;
            };
            let current_exit_corner_distance =
                current_exit_corner.distance_square(&trace_entry_location_approx);
            let mut new_nearest_corner_found = false;
            if current_exit_corner_distance + TOLERANCE < min_exit_corner_distance {
                new_nearest_corner_found = true;
            } else if current_exit_corner_distance < min_exit_corner_distance + TOLERANCE {
                // Pin.java:612-623: the distances are near equal, so compare against the
                // previous corners of the trace polyline.
                for i in 1..trace_polyline.corner_count() {
                    let Some(current_trace_corner) = trace_polyline.corner_approx(i) else {
                        break;
                    };
                    let current_trace_corner_distance =
                        current_trace_corner.distance_square(&current_exit_corner);
                    // Java dereferences `nearestExitCorner` here; on the very first iteration it
                    // is still `null` and this line throws — but it is unreachable, because the
                    // first iteration always takes the branch above (`MAX_VALUE`).
                    let old_trace_corner_distance = current_trace_corner.distance_square(
                        nearest_exit_corner
                            .as_ref()
                            .expect("Java NPEs here; unreachable on the first iteration"),
                    );
                    if current_trace_corner_distance + TOLERANCE < old_trace_corner_distance {
                        new_nearest_corner_found = true;
                        break;
                    } else if current_trace_corner_distance > old_trace_corner_distance + TOLERANCE
                    {
                        break;
                    }
                }
            }
            if new_nearest_corner_found {
                min_exit_corner_distance = current_exit_corner_distance;
                pin_exit_direction = Some(current_exit_restriction.direction.clone());
                nearest_exit_corner = Some(current_exit_corner);
            }
        }
        pin_exit_direction
    }

    /// Port of `Pin.nearestTraceExitCorner` (Pin.java:634-673).
    pub fn nearest_trace_exit_corner(
        &self,
        from_point: &FloatPoint,
        trace_half_width: i32,
        layer: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<FloatPoint> {
        let trace_exit_restrictions = self.get_trace_exit_restrictions(layer, ctx);
        if trace_exit_restrictions.is_empty() {
            return None;
        }
        let pin_center = self.get_center(ctx);
        let offset_pin_shape = self.offset_pad_shape(layer, trace_half_width, ctx)?;
        let mut min_exit_corner_distance = f64::MAX;
        let mut nearest_exit_corner = None;
        for current_exit_restriction in &trace_exit_restrictions {
            let Some(current_exit_corner) = self.exit_corner(
                &offset_pin_shape,
                &pin_center,
                &current_exit_restriction.direction,
            ) else {
                continue;
            };
            let current_exit_corner_distance = current_exit_corner.distance_square(from_point);
            if current_exit_corner_distance < min_exit_corner_distance {
                min_exit_corner_distance = current_exit_corner_distance;
                nearest_exit_corner = Some(current_exit_corner);
            }
        }
        nearest_exit_corner
    }

    /// The shared preamble of `calcNearestExitRestrictionDirection` (Pin.java:576-585) and
    /// `nearestTraceExitCorner` (Pin.java:644-654): the pad shape on `layer`, offset by
    /// `pinEdgeToTurnDist + traceHalfWidth`.
    fn offset_pad_shape(
        &self,
        layer: usize,
        trace_half_width: i32,
        ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        let shape_index = usize::try_from(layer as i32 - first_layer_of(self, ctx) as i32).ok()?;
        let Some(Shape::Tile(pin_shape)) = self.get_shape(shape_index, ctx) else {
            return None;
        };
        let edge_to_turn_dist = ctx.rules.get_pin_edge_to_turn_dist();
        if edge_to_turn_dist < 0.0 {
            return None;
        }
        Some(pin_shape.offset(edge_to_turn_dist + f64::from(trace_half_width)))
    }

    /// The shared inner step of the two methods above (Pin.java:600-606, 659-665): where the ray
    /// from the pin centre into `direction` leaves the offset pad.
    fn exit_corner(
        &self,
        offset_pin_shape: &TileShape,
        pin_center: &Point,
        direction: &Direction,
    ) -> Option<FloatPoint> {
        // Java passes `intersectingBorderLineNo`'s result straight into `borderLine` without
        // checking it, so a `-1` throws an ArrayIndexOutOfBoundsException; `None` here.
        let border_line_no = offset_pin_shape.intersecting_border_line_no(pin_center, direction)?;
        let Point::Int(int_center) = pin_center else {
            return None;
        };
        let pin_exit_ray = Line::from_direction_any(*int_center, direction)?;
        let border_line = offset_pin_shape.border_line(border_line_no)?;
        Some(pin_exit_ray.intersection_approx(&border_line))
    }

    /// Port of `Pin.swap(Pin)` (Pin.java:437-461): exchanges the two pins' nets and their
    /// `changedTo` aliases. Returns false — after Java's warning, which is dropped — when either
    /// pin is on more than one net.
    ///
    /// `nets` replaces Java's `board.rules.nets`, which `Item.assignNetNo` reads
    /// (Item.java:965).
    // renamed: the Task 6 brief calls this `swap_pin`; Java calls it `swap`.
    pub fn swap(&mut self, other: &mut Pin, nets: &Nets) -> bool {
        if self.hdr.net_count() > 1 || other.hdr.net_count() > 1 {
            return false;
        }
        // Pin.java:443-454: an item with no net swaps a 0, which `assignNetNo` then ignores
        // (Item.java:962-964 rejects every non-normal net number).
        let this_net_no = net_no_of_pin(self);
        let other_net_no = net_no_of_pin(other);
        self.hdr.assign_net_no(other_net_no, nets);
        other.hdr.assign_net_no(this_net_no, nets);
        // Pin.java:457-459, with Java's self-reference spelled `None`.
        let this_changed_to = self.get_changed_to();
        let other_changed_to = other.get_changed_to();
        self.changed_to = (other_changed_to != self.hdr.id()).then_some(other_changed_to);
        other.changed_to = (this_changed_to != other.hdr.id()).then_some(this_changed_to);
        true
    }

    // added in Task 11: `getSwappablePins` (Pin.java:391-427) -> `Board::swappable_pins(ItemId)`.
    // Its body walks the component's `LogicalPart` for part pins with the same `gateName` and
    // `gatePinSwapCode`, then resolves each one through `board.getPin(componentId, pinIndex)`
    // (Pin.java:418) — a lookup over the board's whole item list, which is what makes it a
    // `Board` method rather than a `Pin` one. Note also that its result is a `TreeSet<Pin>`, so
    // Task 11 must give it `Item.compareTo`'s descending-id order (quirk #44).

    /// Port of `Pin.getChangedTo` (Pin.java:463-468): the pin this one was swapped with, or this
    /// pin's own id if it was never swapped.
    pub fn get_changed_to(&self) -> ItemId {
        self.changed_to.unwrap_or_else(|| self.hdr.id())
    }

    /// Port of `Pin.turn90Degree` (Pin.java:367-371), which — unlike `DrillItem`'s — throws the
    /// centre away instead of turning it, because it is recomputed from the component.
    pub fn turn_90_degree(&mut self, _factor: i32, _pole: &IntPoint) {
        self.drill.set_center(None);
        self.clear_derived_data();
    }

    /// Port of `Pin.rotateApprox` (Pin.java:373-377).
    pub fn rotate_approx(&mut self, _angle_in_degree: f64, _pole: &FloatPoint) {
        self.drill.set_center(None);
        self.clear_derived_data();
    }

    /// Port of `Pin.changePlacementSide` (Pin.java:379-383).
    pub fn change_placement_side(&mut self, _pole: &IntPoint) {
        self.drill.set_center(None);
        self.clear_derived_data();
    }

    /// Port of `DrillItem.translateBy` (DrillItem.java:61-68), which `Pin` does **not** override:
    /// a pin whose centre has already been calculated has it translated, and a pin whose centre
    /// is still `null` keeps it `null` and recomputes it from the (also translated) component.
    pub fn translate_by(&mut self, vector: &Vector) {
        let translated = self.drill.raw_center().map(|c| c.translate_by(vector));
        if translated.is_some() {
            self.drill.set_center(translated);
        }
        self.clear_derived_data();
    }

    /// Port of `Pin.clearDerivedData` (Pin.java:385-389): the `DrillItem` body plus this pin's
    /// cached shapes.
    pub fn clear_derived_data(&mut self) {
        self.hdr.clear_derived_data();
        self.drill.clear_derived_data();
        *self.shapes.borrow_mut() = None;
    }
}

/// The net number `Pin.swap` reads from a pin (Pin.java:443-454): its first net, or `0` for a
/// pin that is on no net at all.
///
/// Not a Java method — Java writes the `if (netCount() > 0)` twice over, once per pin.
fn net_no_of_pin(pin: &Pin) -> i32 {
    if pin.hdr.net_count() > 0 {
        pin.hdr.get_net_number(0)
    } else {
        0
    }
}

impl Connectable for Pin {
    fn header(&self) -> &ItemHeader {
        &self.hdr
    }

    /// Port of `DrillItem.getTraceConnectionShape` (DrillItem.java:358-362).
    fn get_trace_connection_shape(
        &self,
        _tree: TreeId,
        _index: usize,
        ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        Some(trace_connection_shape_of(&self.get_center(ctx)))
    }
}

/// Port of the nested `Pin.TraceExitRestriction` (Pin.java:694-705): one allowed trace exit
/// direction out of a pin pad, with the minimal trace line length into it.
// renamed: `Pin.TraceExitRestriction`'s constructor -> `TraceExitRestriction`'s struct literal,
// which is what every Rust caller writes instead.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceExitRestriction {
    /// Java `public final Direction direction` (Pin.java:697).
    pub direction: Direction,
    /// Java `public final double minLength` (Pin.java:698).
    pub min_length: f64,
}
