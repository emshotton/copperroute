//! Areas and component outlines: the state and behaviour Java's `ObstacleArea` shares between
//! [`ObstacleArea`], [`ConductionArea`], [`ViaObstacleArea`] and [`ComponentObstacleArea`], plus
//! the closely related [`ComponentOutline`].
//!
//! Java: `board/model/items/{ObstacleArea,ConductionArea,ViaObstacleArea,ComponentObstacleArea,
//! ComponentOutline}.java`. (`BoardOutline` is a different shape of thing — a list of polygons
//! rather than one placed area — and lives in [`crate::structure::board_outline`], next to its
//! Java package.)
//!
//! # The placed-area model
//!
//! An `ObstacleArea` stores a **relative** area plus a placement (`translation`,
//! `rotationInDegree`, `sideChanged`), and `getArea()` (ObstacleArea.java:119-144) composes them
//! into the absolute area, memoised in `precalculatedAbsoluteArea`. The composition order is
//! load-bearing and asymmetric:
//!
//! ```text
//! mirrorVertical(ZERO)   if sideChanged && !flipStyleRotateFirst
//! turn90Degree / rotateApprox around the origin, if rotationInDegree != 0
//! mirrorVertical(ZERO)   if sideChanged && flipStyleRotateFirst
//! translateBy(translation)
//! ```
//!
//! `ComponentOutline.getArea` (ComponentOutline.java:191-216) is the same body with `!isFront`
//! everywhere `ObstacleArea` writes `sideChanged`, which is why the two share
//! [`absolute_area_of`].
//!
//! # What replaces Java's `board` back-pointer
//!
//! `getArea` reads `board.components.getFlipStyleRotateFirst()`, `changePlacementSide` reads
//! `board.getLayerCount()` and `ComponentOutline.getLayer` reads it too, so all three take an
//! [`ItemCtx`] — the same borrow-of-three-board-fields Task 6 introduced for the drill items.
//!
//! # The memo
//!
//! `precalculatedAbsoluteArea` is a [`OnceLock`], **not** a `Cell`/`RefCell`: `getArea` must stay
//! `&self` (two items are held at once by `is_obstacle`/`shares_layer`) and `Item` must stay
//! `Send + Sync`. It is emptied at exactly Java's five invalidation points — `clearDerivedData`,
//! `translateBy`, `turn90Degree`, `rotateApprox` and `changePlacementSide` — all of which are
//! `&mut self`. `PartialEq` skips it, the way the drill items' `PartialEq` skips theirs.
//!
//! not ported: `ConductionArea.getDetailedFillArea` (ConductionArea.java:107-110),
//! `ensureDetailedFillCache` (:112-234), `getAwtAreaInBoardUnits` (:236-275) and
//! `getAwtAreaFromShapeInBoardUnits` (:277-307) — renderer-only `java.awt.geom.Area` boolean
//! algebra (plan-rulings.md #3, quirk #59). [`ConductionArea::warm_detailed_fill_cache`] is kept
//! as an empty method because it is the only *public* entry point of that group; the group's two
//! `transient` fields (`cachedBoardRevision`, `cachedBoardFillArea`, ConductionArea.java:42-43)
//! and the paint constant `PLANE_HATCH_OPACITY` (ConductionArea.java:27) go with it.
//!
//! not ported: the protected `ObstacleArea.printShapeInfo` (ObstacleArea.java:298-318) — the
//! `ItemInfoPrinter` helper the four `printInfo` overrides share; GUI output, like the rest of
//! that family (see [`crate::items`]'s not-ported list).
//!
//! added in Task 10: the protected `ObstacleArea.calculateTreeShapes` (ObstacleArea.java:182-185)
//! and `ComponentOutline.calculateTreeShapes` (ComponentOutline.java:134-137) — the first
//! delegates to `ShapeSearchTree.calculateTreeShapes(ObstacleArea)` (which enlarges each convex
//! piece by the clearance compensation), the second is `return new TileShape[0]`.

use std::sync::OnceLock;

use fr_geometry::{Area, FloatPoint, IntBox, IntPoint, Point, TileShape, Vector};

use crate::ids::{ItemId, TreeId};
use crate::items::header::ItemHeader;
use crate::items::{Connectable, ItemCtx, copied_header};

/// Port of the state `ObstacleArea` (`board/model/items/ObstacleArea.java`) adds to `Item`: a
/// relative area, its placement, its layer, its optional name and the absolute-area memo.
///
/// Embedded in the four area variants as `area`, the way [`ItemHeader`] is embedded as `hdr`.
#[derive(Debug, Clone)]
pub struct ObstacleAreaData {
    /// Java `public final String name` (ObstacleArea.java:31): `null` — here `None` — when the
    /// area does not belong to a component.
    name: Option<String>,
    /// Java `private final Area relativeArea` (ObstacleArea.java:33).
    ///
    /// Java allows this to be `null` and both `getArea` (ObstacleArea.java:122-125) and
    /// `splitToConvex` (:321-324) warn and return `null` for it. Rust has no null, so the field
    /// is a plain [`Area`] and those two branches are unreachable rather than ported; every
    /// Java caller that could reach them (`boundingBox`, `tileShapeCount`, `printShapeInfo`)
    /// would NPE one line later anyway.
    relative_area: Area,
    /// Java `private int layer` (ObstacleArea.java:36) — not final: `changePlacementSide`
    /// rewrites it (ObstacleArea.java:250).
    layer: usize,
    /// Java `private Vector translation` (ObstacleArea.java:39).
    translation: Vector,
    /// Java `private double rotationInDegree` (ObstacleArea.java:40).
    rotation_in_degree: f64,
    /// Java `private boolean sideChanged` (ObstacleArea.java:41).
    side_changed: bool,
    /// Java `private transient Area precalculatedAbsoluteArea` (ObstacleArea.java:38).
    absolute_area: OnceLock<Area>,
    /// The convex division of [`Self::get_area`], memoised.
    ///
    /// Java memoises it one level down, in `PolylineArea.precalculatedConvexPieces`
    /// (PolylineArea.java:31) / `PolygonShape.precalculatedConvexPieces`
    /// (PolygonShape.java:17), so `splitToConvex` costs a division once per `Area` object and
    /// every later call is a field read. `fr-geometry` deliberately does *not* memoise there
    /// (the division draws from a `java.util.Random`, and Plan 1 made that generator per-call
    /// so the result is reproducible under concurrency — quirk #30), which left
    /// `tileShapeCount`/`getTileShape` loops re-dividing the area on every index. This lock
    /// restores Java's amortised cost at the item level; it is filled from the same
    /// [`Self::get_area`] the Java memo is derived from, so it holds exactly what Java's does.
    ///
    /// `Option` inside the lock is Java's `null` return from a failed division.
    // Plan-1 obligation: "memo cache for convex pieces" (docs/plan-1-handoff.md, Plan 2 list;
    // docs/java-quirks.md obligation register), discharged here per the Task 7 ruling.
    convex_pieces: OnceLock<Option<Vec<TileShape>>>,
}

/// Compares the six real fields; the absolute-area memo is derived state (see the module doc).
impl PartialEq for ObstacleAreaData {
    fn eq(&self, other: &ObstacleAreaData) -> bool {
        self.name == other.name
            && self.relative_area == other.relative_area
            && self.layer == other.layer
            && self.translation == other.translation
            && self.rotation_in_degree == other.rotation_in_degree
            && self.side_changed == other.side_changed
    }
}

impl ObstacleAreaData {
    /// The geometry half of `ObstacleArea(Area, int, Vector, double, boolean, int[], int, int,
    /// int, String, FixedState, BasicBoard)` (ObstacleArea.java:47-67); the `Item` half is
    /// [`ItemHeader::new`].
    pub fn new(
        relative_area: Area,
        layer: usize,
        translation: Vector,
        rotation_in_degree: f64,
        side_changed: bool,
        name: Option<String>,
    ) -> ObstacleAreaData {
        ObstacleAreaData {
            name,
            relative_area,
            layer,
            translation,
            rotation_in_degree,
            side_changed,
            absolute_area: OnceLock::new(),
            convex_pieces: OnceLock::new(),
        }
    }

    /// Java `ObstacleArea.name` (ObstacleArea.java:31), which is a public field rather than a
    /// getter.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Port of `ObstacleArea.getRelativeArea` (ObstacleArea.java:146-148).
    pub fn get_relative_area(&self) -> &Area {
        &self.relative_area
    }

    /// Port of `ObstacleArea.getLayer` (ObstacleArea.java:165-167).
    pub fn get_layer(&self) -> usize {
        self.layer
    }

    /// Port of `ObstacleArea.getTranslation` (ObstacleArea.java:270-272).
    pub fn get_translation(&self) -> &Vector {
        &self.translation
    }

    /// Port of `ObstacleArea.getRotationInDegree` (ObstacleArea.java:274-276).
    pub fn get_rotation_in_degree(&self) -> f64 {
        self.rotation_in_degree
    }

    /// Port of `ObstacleArea.getSideChanged` (ObstacleArea.java:278-280).
    pub fn get_side_changed(&self) -> bool {
        self.side_changed
    }

    /// Port of `ObstacleArea.getArea` (ObstacleArea.java:119-144): the relative area mirrored,
    /// rotated and translated into board coordinates, memoised in `precalculatedAbsoluteArea`.
    pub fn get_area(&self, ctx: &ItemCtx<'_>) -> &Area {
        self.absolute_area.get_or_init(|| {
            absolute_area_of(
                &self.relative_area,
                self.side_changed,
                self.rotation_in_degree,
                &self.translation,
                ctx.components.get_flip_style_rotate_first(),
            )
        })
    }

    /// Port of `ObstacleArea.boundingBox` (ObstacleArea.java:169-172).
    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        self.get_area(ctx).bounding_box()
    }

    /// Port of `ObstacleArea.splitToConvex` (ObstacleArea.java:320-326). `None` is Java's
    /// `null`, which `Area.splitToConvex` answers when the division fails.
    ///
    /// Memoised in [`Self::convex_pieces`], mirroring Java's `precalculatedConvexPieces`; the
    /// borrow is what makes the memo worth having, so this returns a slice where Java returns
    /// the array it cached.
    pub fn split_to_convex(&self, ctx: &ItemCtx<'_>) -> Option<&[TileShape]> {
        self.convex_pieces
            .get_or_init(|| self.get_area(ctx).split_to_convex())
            .as_deref()
    }

    /// Port of `ObstacleArea.tileShapeCount` (ObstacleArea.java:187-195): 0 when the division
    /// fails.
    pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
        self.split_to_convex(ctx).map_or(0, <[TileShape]>::len)
    }

    /// Port of `ObstacleArea.getTileShape(int)` (ObstacleArea.java:197-205), the override that
    /// bypasses the search tree and splits the area directly. Java's out-of-range warning path
    /// returns `null`.
    pub fn get_tile_shape(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<TileShape> {
        self.split_to_convex(ctx)?.get(index).cloned()
    }

    /// Port of `ObstacleArea.translateBy` (ObstacleArea.java:207-211), without the trailing
    /// `clearDerivedData()` — that call is virtual, so the variant wrapper makes it (dropping
    /// the header's caches too).
    fn translate_by(&mut self, vector: &Vector) {
        self.translation = self.translation.add(vector);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    /// Port of `ObstacleArea.turn90Degree` (ObstacleArea.java:213-225).
    fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        self.rotation_in_degree =
            wrap_into_a_full_turn(self.rotation_in_degree + f64::from(factor) * 90.0);
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .turn_90_degree(factor, &Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    /// Port of `ObstacleArea.rotateApprox` (ObstacleArea.java:227-244).
    ///
    /// Note the asymmetry Java has here and in `Component.rotate` (quirk #48): the *stored*
    /// rotation takes the complement `360 - angleInDegree` for a flipped area under
    /// `flipStyleRotateFirst`, but the translation is rotated by the original `angleInDegree`
    /// (ObstacleArea.java:240-241).
    fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint, ctx: &ItemCtx<'_>) {
        let mut turn_angle = angle_in_degree;
        if self.side_changed && ctx.components.get_flip_style_rotate_first() {
            turn_angle = 360.0 - angle_in_degree;
        }
        self.rotation_in_degree = wrap_into_a_full_turn(self.rotation_in_degree + turn_angle);
        let new_translation = self
            .translation
            .to_float()
            .rotate(angle_in_degree.to_radians(), pole);
        self.translation = Point::Int(new_translation.round()).difference_by(&Point::ZERO);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    /// Port of `ObstacleArea.changePlacementSide` (ObstacleArea.java:246-255).
    ///
    /// Java guards the layer flip with `if (this.board != null)` (ObstacleArea.java:249); an
    /// [`ItemCtx`] is always present here, so the guard is always taken.
    fn change_placement_side(&mut self, pole: &IntPoint, ctx: &ItemCtx<'_>) {
        self.side_changed = !self.side_changed;
        let layer_count = ctx.rules.layer_structure().count();
        self.layer = layer_count.checked_sub(self.layer + 1).expect(
            "ObstacleArea.changePlacementSide: layer >= board.getLayerCount() — Java stores a \
             negative layer here (ObstacleArea.java:250)",
        );
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .mirror_vertical(&Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    /// The `ObstacleArea` half of `ObstacleArea.clearDerivedData` (ObstacleArea.java:328-332):
    /// drop `precalculatedAbsoluteArea`. The `super.clearDerivedData()` half is the header's.
    fn clear_derived_data(&mut self) {
        self.absolute_area.take();
        self.convex_pieces.take();
    }

    /// The geometry half of `ObstacleArea.copy` (ObstacleArea.java:100-118): a fresh placement
    /// carrying every field the Java constructor is handed, and an empty memo.
    fn copied(&self) -> ObstacleAreaData {
        ObstacleAreaData::new(
            self.relative_area.clone(),
            self.layer,
            self.translation.clone(),
            self.rotation_in_degree,
            self.side_changed,
            self.name.clone(),
        )
    }
}

/// The shared body of `ObstacleArea.getArea` (ObstacleArea.java:126-141) and
/// `ComponentOutline.getArea` (ComponentOutline.java:198-213), which are the same four steps —
/// the outline passes `!isFront` where the area passes `sideChanged`.
fn absolute_area_of(
    relative_area: &Area,
    mirrored: bool,
    rotation_in_degree: f64,
    translation: &Vector,
    flip_style_rotate_first: bool,
) -> Area {
    let mut turned_area = relative_area.clone();
    if mirrored && !flip_style_rotate_first {
        turned_area = turned_area.mirror_vertical(&IntPoint::ZERO);
    }
    if rotation_in_degree != 0.0 {
        if rotation_in_degree % 90.0 == 0.0 {
            // ObstacleArea.java:133: `((int) rotation) / 90` truncates towards zero.
            turned_area =
                turned_area.turn_90_degree(rotation_in_degree as i32 / 90, &IntPoint::ZERO);
        } else {
            turned_area =
                turned_area.rotate_approx(rotation_in_degree.to_radians(), &FloatPoint::ZERO);
        }
    }
    if mirrored && flip_style_rotate_first {
        turned_area = turned_area.mirror_vertical(&IntPoint::ZERO);
    }
    turned_area.translate_by(translation)
}

/// Java's `while (x >= 360) x -= 360; while (x < 0) x += 360;` (ObstacleArea.java:216-221,
/// :234-239; ComponentOutline.java:165-170, :180-185), written once.
///
/// The loops are kept rather than replaced with `rem_euclid`: they are the Java text, and for a
/// non-finite input `rem_euclid` would answer `NaN` where Java's loops spin forever. No caller
/// can supply one — `factor * 90` is an `int` product and `rotateApprox`'s angle comes from the
/// GUI or a placement file.
fn wrap_into_a_full_turn(mut rotation_in_degree: f64) -> f64 {
    while rotation_in_degree >= 360.0 {
        rotation_in_degree -= 360.0;
    }
    while rotation_in_degree < 0.0 {
        rotation_in_degree += 360.0;
    }
    rotation_in_degree
}

/// Generates the shared `ObstacleArea` body for the four area variants, whose Java classes all
/// inherit it from `ObstacleArea` and differ only in their constructors and `isObstacle`.
///
/// The four mutators each end in Java's virtual `this.clearDerivedData()`, which for every one
/// of the four variants resolves to `ObstacleArea.clearDerivedData` (ObstacleArea.java:328-332)
/// — `super.clearDerivedData()` plus the absolute-area memo — or, for `ConductionArea`, to its
/// own override (ConductionArea.java:76-81), which adds only the not-ported fill cache.
macro_rules! obstacle_area_impl {
    ($ty:ident) => {
        impl $ty {
            /// Java `ObstacleArea.name` (ObstacleArea.java:31).
            pub fn name(&self) -> Option<&str> {
                self.area.name()
            }

            /// Port of `ObstacleArea.getArea` (ObstacleArea.java:119-144).
            pub fn get_area(&self, ctx: &ItemCtx<'_>) -> &Area {
                self.area.get_area(ctx)
            }

            /// Port of `ObstacleArea.getRelativeArea` (ObstacleArea.java:146-148).
            pub fn get_relative_area(&self) -> &Area {
                self.area.get_relative_area()
            }

            /// Port of `ObstacleArea.getLayer` (ObstacleArea.java:165-167).
            pub fn get_layer(&self) -> usize {
                self.area.get_layer()
            }

            /// Port of `ObstacleArea.getTranslation` (ObstacleArea.java:270-272).
            pub fn get_translation(&self) -> &Vector {
                self.area.get_translation()
            }

            /// Port of `ObstacleArea.getRotationInDegree` (ObstacleArea.java:274-276).
            pub fn get_rotation_in_degree(&self) -> f64 {
                self.area.get_rotation_in_degree()
            }

            /// Port of `ObstacleArea.getSideChanged` (ObstacleArea.java:278-280).
            pub fn get_side_changed(&self) -> bool {
                self.area.get_side_changed()
            }

            /// Port of `ObstacleArea.boundingBox` (ObstacleArea.java:169-172).
            pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
                self.area.bounding_box(ctx)
            }

            /// Port of `ObstacleArea.tileShapeCount` (ObstacleArea.java:187-195).
            pub fn tile_shape_count(&self, ctx: &ItemCtx<'_>) -> usize {
                self.area.tile_shape_count(ctx)
            }

            /// Port of `ObstacleArea.getTileShape(int)` (ObstacleArea.java:197-205).
            pub fn get_tile_shape(&self, index: usize, ctx: &ItemCtx<'_>) -> Option<TileShape> {
                self.area.get_tile_shape(index, ctx)
            }

            /// Port of `ObstacleArea.splitToConvex` (ObstacleArea.java:320-326).
            pub fn split_to_convex(&self, ctx: &ItemCtx<'_>) -> Option<&[TileShape]> {
                self.area.split_to_convex(ctx)
            }

            /// Port of `ObstacleArea.translateBy` (ObstacleArea.java:207-211).
            pub fn translate_by(&mut self, vector: &Vector) {
                self.area.translate_by(vector);
                self.hdr.clear_derived_data();
            }

            /// Port of `ObstacleArea.turn90Degree` (ObstacleArea.java:213-225).
            pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
                self.area.turn_90_degree(factor, pole);
                self.hdr.clear_derived_data();
            }

            /// Port of `ObstacleArea.rotateApprox` (ObstacleArea.java:227-244).
            pub fn rotate_approx(
                &mut self,
                angle_in_degree: f64,
                pole: &FloatPoint,
                ctx: &ItemCtx<'_>,
            ) {
                self.area.rotate_approx(angle_in_degree, pole, ctx);
                self.hdr.clear_derived_data();
            }

            /// Port of `ObstacleArea.changePlacementSide` (ObstacleArea.java:246-255).
            pub fn change_placement_side(&mut self, pole: &IntPoint, ctx: &ItemCtx<'_>) {
                self.area.change_placement_side(pole, ctx);
                self.hdr.clear_derived_data();
            }

            /// Port of `ObstacleArea.clearDerivedData` (ObstacleArea.java:328-332):
            /// `super.clearDerivedData()` plus the absolute-area memo.
            pub fn clear_derived_data(&mut self) {
                self.hdr.clear_derived_data();
                self.area.clear_derived_data();
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------
// ObstacleArea
// ---------------------------------------------------------------------------------------------

/// Port of `ObstacleArea` (`board/model/items/ObstacleArea.java`): a keepout area.
#[derive(Debug, Clone, PartialEq)]
pub struct ObstacleArea {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// The `ObstacleArea` base-class state (ObstacleArea.java:31-41).
    pub area: ObstacleAreaData,
}

impl ObstacleArea {
    /// Port of `ObstacleArea(Area, int, Vector, double, boolean, int[], int, int, int, String,
    /// FixedState, BasicBoard)` (ObstacleArea.java:47-67), split into its `Item` half (`hdr`)
    /// and its geometry half (`area`).
    ///
    /// Java's second constructor (ObstacleArea.java:73-98) only defaults the net numbers to
    /// `new int[0]`, which is what an empty `hdr.net_nos` already is.
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData) -> ObstacleArea {
        ObstacleArea { hdr, area }
    }

    /// Port of `ObstacleArea.copy` (ObstacleArea.java:100-118).
    pub fn copy(&self, new_id: ItemId) -> ObstacleArea {
        ObstacleArea {
            hdr: copied_header(&self.hdr, new_id),
            area: self.area.copied(),
        }
    }
}
obstacle_area_impl!(ObstacleArea);

// ---------------------------------------------------------------------------------------------
// ConductionArea
// ---------------------------------------------------------------------------------------------

/// Port of `ConductionArea` (`board/model/items/ConductionArea.java`): a copper pour, which
/// extends `ObstacleArea` **and** implements `Connectable`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConductionArea {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// The `ObstacleArea` base-class state (ObstacleArea.java:31-41).
    pub area: ObstacleAreaData,
    /// Java `private boolean isObstacle` (ConductionArea.java:29): whether this area is an
    /// obstacle to traces and vias of foreign nets. Read by four `isObstacle`/`isTraceObstacle`
    /// /`isDrillable` overrides.
    ///
    /// `Board::change_conduction_is_obstacle` (Task 11, `RoutingBoard.java:1252-1276`) is what
    /// couples it to `BoardRules::get_ignore_conduction` — see
    /// [`Item::is_obstacle`](crate::items::Item::is_obstacle), and **quirk #50 in
    /// `docs/java-quirks.md`**, which records that that method's guard
    /// (`if (rules.getIgnoreConduction() != value) return;`, :1254) and its closing
    /// `setIgnoreConduction(!value)` (:1273) must be ported verbatim rather than "fixed".
    is_obstacle: bool,
    /// Java `private boolean isFilled = true` (ConductionArea.java:30).
    is_filled: bool,
}

impl ConductionArea {
    /// Port of `ConductionArea(Area, int, Vector, double, boolean, int[], int, int, int, String,
    /// boolean, FixedState, BasicBoard)` (ConductionArea.java:46-74). `isFilled` starts `true`,
    /// as Java's field initialiser does (ConductionArea.java:30).
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData, is_obstacle: bool) -> ConductionArea {
        ConductionArea {
            hdr,
            area,
            is_obstacle,
            is_filled: true,
        }
    }

    /// Port of `ConductionArea.getIsObstacle` (ConductionArea.java:387-390).
    pub fn get_is_obstacle(&self) -> bool {
        self.is_obstacle
    }

    /// Port of `ConductionArea.setIsObstacle` (ConductionArea.java:392-395).
    pub fn set_is_obstacle(&mut self, value: bool) {
        self.is_obstacle = value;
    }

    /// Port of `ConductionArea.getIsFilled` (ConductionArea.java:32-35).
    pub fn get_is_filled(&self) -> bool {
        self.is_filled
    }

    /// Port of `ConductionArea.setIsFilled` (ConductionArea.java:36-40), which also clears the
    /// derived data.
    pub fn set_is_filled(&mut self, value: bool) {
        self.is_filled = value;
        self.clear_derived_data();
    }

    /// Port of `ConductionArea.copy` (ConductionArea.java:309-330).
    ///
    //  Java bug: an area on anything other than exactly one net is not copied at all — Java warns
    //  ("not yet implemented for areas with more than 1 net") and returns `null`
    //  (ConductionArea.java:310-313), which includes an area with **zero** nets despite the
    //  message. Reproduced as `None`; see docs/java-quirks.md.
    /// Returns `None` unless the area is on exactly one net.
    pub fn copy(&self, new_id: ItemId) -> Option<ConductionArea> {
        if self.hdr.net_count() != 1 {
            return None;
        }
        Some(ConductionArea {
            hdr: copied_header(&self.hdr, new_id),
            area: self.area.copied(),
            is_obstacle: self.is_obstacle,
            // `isFilled` is **not** carried over: `ConductionArea.copy` goes through the
            // constructor (ConductionArea.java:314-329 -> :46-73), whose parameter list has no
            // `isFilled`, so the new object gets the field initialiser `private boolean
            // isFilled = true` (ConductionArea.java:30). An unfilled area copies to a filled one.
            is_filled: true,
        })
    }

    /// Port of `ConductionArea.warmDetailedFillCache` (ConductionArea.java:83-99).
    ///
    /// Empty by design: everything it warms is the renderer-only `java.awt.geom.Area` fill cache
    /// (plan-rulings.md #3), which this port does not have. See the module doc for the four
    /// `not ported:` members it drives.
    pub fn warm_detailed_fill_cache(&self) {}
}
obstacle_area_impl!(ConductionArea);

impl Connectable for ConductionArea {
    fn header(&self) -> &ItemHeader {
        &self.hdr
    }

    /// Port of `ConductionArea.getTraceConnectionShape` (ConductionArea.java:358-365): the
    /// range-checked `getTreeShape(searchTree, index)`.
    ///
    /// Java's out-of-range warning path returns `null`; so does a cold cache, because
    /// `treeShapeCount` is then 0 and every index is out of range.
    // added in Task 10: the lazy fill behind `treeShapeCount`/`getTreeShape`
    // (Item.java:203-226), which needs the `ShapeSearchTree` itself.
    fn get_trace_connection_shape(
        &self,
        tree: TreeId,
        index: usize,
        _ctx: &ItemCtx<'_>,
    ) -> Option<TileShape> {
        self.hdr
            .get_precalculated_tree_shapes(tree)
            .and_then(|shapes| shapes.get(index))
            .cloned()
    }
}

// ---------------------------------------------------------------------------------------------
// ViaObstacleArea
// ---------------------------------------------------------------------------------------------

/// Port of `ViaObstacleArea` (`board/model/items/ViaObstacleArea.java`): a keepout for vias
/// only, extending `ObstacleArea`.
#[derive(Debug, Clone, PartialEq)]
pub struct ViaObstacleArea {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// The `ObstacleArea` base-class state (ObstacleArea.java:31-41).
    pub area: ObstacleAreaData,
}

impl ViaObstacleArea {
    /// Port of `ViaObstacleArea(Area, int, Vector, double, boolean, int[], int, int, int,
    /// String, FixedState, BasicBoard)` (ViaObstacleArea.java:16-42).
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData) -> ViaObstacleArea {
        ViaObstacleArea { hdr, area }
    }

    /// Port of `ViaObstacleArea.copy` (ViaObstacleArea.java:72-89).
    pub fn copy(&self, new_id: ItemId) -> ViaObstacleArea {
        ViaObstacleArea {
            hdr: copied_header(&self.hdr, new_id),
            area: self.area.copied(),
        }
    }
}
obstacle_area_impl!(ViaObstacleArea);

// ---------------------------------------------------------------------------------------------
// ComponentObstacleArea
// ---------------------------------------------------------------------------------------------

/// Port of `ComponentObstacleArea` (`board/model/items/ComponentObstacleArea.java`): a placement
/// keepout belonging to a component, extending `ObstacleArea`.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentObstacleArea {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// The `ObstacleArea` base-class state (ObstacleArea.java:31-41).
    pub area: ObstacleAreaData,
}

impl ComponentObstacleArea {
    /// Port of `ComponentObstacleArea(Area, int, Vector, double, boolean, int, int, int, String,
    /// FixedState, BasicBoard)` (ComponentObstacleArea.java:20-45).
    ///
    /// Java's constructor passes `new int[0]` for the net numbers
    /// (ComponentObstacleArea.java:38), so a component keepout is never on a net; the caller
    /// must hand in a `hdr` with no nets.
    pub fn new(hdr: ItemHeader, area: ObstacleAreaData) -> ComponentObstacleArea {
        ComponentObstacleArea { hdr, area }
    }

    /// Port of `ComponentObstacleArea.copy` (ComponentObstacleArea.java:47-61).
    pub fn copy(&self, new_id: ItemId) -> ComponentObstacleArea {
        ComponentObstacleArea {
            hdr: ItemHeader::new(
                new_id,
                Vec::new(),
                self.hdr.clearance_class(),
                self.hdr.get_component_id(),
                self.hdr.get_fixed_state(),
            ),
            area: self.area.copied(),
        }
    }

    /// Port of `ComponentObstacleArea.isFront` (ComponentObstacleArea.java:83-87):
    /// `Component component = board.components.get(getComponentId()); return component == null ||
    /// component.placedOnFront();` — it reads the **board's component list**, not any field of
    /// its own.
    // added in Task 11: `isFront` becomes `Board::component_obstacle_area_is_front(ItemId)`,
    // because it needs `board.components`. Note the two halves Task 11 must model: Java returns
    // `true` when `components.get(componentId)` is `null`, but this port's
    // `Components::get` *panics* on an id outside `1..=count` (quirk #49, Components.java:89) —
    // so `Board` has to bounds-check the component id itself and answer `true` for a missing
    // one rather than calling `Components::get` blind.
    pub fn is_front(&self) -> bool {
        unimplemented!(
            "ComponentObstacleArea::is_front needs the board's Components, added in Task 11 \
             (ComponentObstacleArea.java:83-87)"
        )
    }
}
obstacle_area_impl!(ComponentObstacleArea);

// ---------------------------------------------------------------------------------------------
// ComponentOutline
// ---------------------------------------------------------------------------------------------

/// Port of `ComponentOutline` (`board/model/items/ComponentOutline.java`): a component's
/// courtyard or fabrication outline.
///
/// It extends `Item` directly, not `ObstacleArea`, but carries the same placed-area state minus
/// the layer (which it derives from `isFront`) and the name.
#[derive(Debug, Clone)]
pub struct ComponentOutline {
    /// The `Item` base-class state (Item.java:41-67).
    pub hdr: ItemHeader,
    /// Java `private final Area relativeArea` (ComponentOutline.java:24). See
    /// [`ObstacleAreaData::relative_area`] on why it is not an `Option`.
    relative_area: Area,
    /// Java `private Vector translation` (ComponentOutline.java:26).
    translation: Vector,
    /// Java `private double rotationInDegree` (ComponentOutline.java:27).
    rotation_in_degree: f64,
    /// Java `private boolean isFront` (ComponentOutline.java:28) — not final:
    /// `changePlacementSide` flips it (ComponentOutline.java:152).
    is_front: bool,
    /// Java `private final boolean isCourtyard` (ComponentOutline.java:29).
    is_courtyard: bool,
    /// Java `private final boolean isFabrication` (ComponentOutline.java:30).
    is_fabrication: bool,
    /// Java `private final boolean isClosed` (ComponentOutline.java:31).
    is_closed: bool,
    /// Java `private transient Area precalculatedAbsoluteArea` (ComponentOutline.java:25).
    absolute_area: OnceLock<Area>,
}

/// Compares the seven real fields; the absolute-area memo is derived state.
impl PartialEq for ComponentOutline {
    fn eq(&self, other: &ComponentOutline) -> bool {
        self.hdr == other.hdr
            && self.relative_area == other.relative_area
            && self.translation == other.translation
            && self.rotation_in_degree == other.rotation_in_degree
            && self.is_front == other.is_front
            && self.is_courtyard == other.is_courtyard
            && self.is_fabrication == other.is_fabrication
            && self.is_closed == other.is_closed
    }
}

impl ComponentOutline {
    /// Port of `ComponentOutline(Area, boolean, Vector, double, int, int, boolean, boolean,
    /// boolean, FixedState, BasicBoard)` (ComponentOutline.java:33-54), in Java's parameter
    /// order with the two `Item` parameters folded into `hdr`.
    ///
    /// Java's constructor hard-codes `new int[0], 0` for the net numbers and the clearance class
    /// (ComponentOutline.java:46), so the caller must hand in a matching `hdr`.
    // The eight parameters are Java's; bundling them would only hide the constructor's shape.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        hdr: ItemHeader,
        relative_area: Area,
        is_front: bool,
        translation: Vector,
        rotation_in_degree: f64,
        is_courtyard: bool,
        is_fabrication: bool,
        is_closed: bool,
    ) -> ComponentOutline {
        ComponentOutline {
            hdr,
            relative_area,
            translation,
            rotation_in_degree,
            is_front,
            is_courtyard,
            is_fabrication,
            is_closed,
            absolute_area: OnceLock::new(),
        }
    }

    /// Port of `ComponentOutline.copy` (ComponentOutline.java:56-70).
    pub fn copy(&self, new_id: ItemId) -> ComponentOutline {
        ComponentOutline::new(
            // ComponentOutline.java:46: the constructor passes `new int[0], 0`, so a copy has no
            // nets and clearance class 0 whatever the original carried.
            ItemHeader::new(
                new_id,
                Vec::new(),
                0,
                self.hdr.get_component_id(),
                self.hdr.get_fixed_state(),
            ),
            self.relative_area.clone(),
            self.is_front,
            self.translation.clone(),
            self.rotation_in_degree,
            self.is_courtyard,
            self.is_fabrication,
            self.is_closed,
        )
    }

    /// Port of `ComponentOutline.isFront` (ComponentOutline.java:72-74).
    pub fn is_front(&self) -> bool {
        self.is_front
    }

    /// Port of `ComponentOutline.isCourtyard` (ComponentOutline.java:76-78).
    pub fn is_courtyard(&self) -> bool {
        self.is_courtyard
    }

    /// Port of `ComponentOutline.isFabrication` (ComponentOutline.java:80-82).
    pub fn is_fabrication(&self) -> bool {
        self.is_fabrication
    }

    /// Port of `ComponentOutline.isClosed` (ComponentOutline.java:84-86).
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    /// Java `ComponentOutline.translation` (ComponentOutline.java:26). Not a Java getter — the
    /// field is private and only the transforms read it — but the port needs it to be testable.
    pub fn get_translation(&self) -> &Vector {
        &self.translation
    }

    /// Java `ComponentOutline.rotationInDegree` (ComponentOutline.java:27). See
    /// [`Self::get_translation`].
    pub fn get_rotation_in_degree(&self) -> f64 {
        self.rotation_in_degree
    }

    /// Port of `ComponentOutline.getLayer` (ComponentOutline.java:93-102): layer 0 on the front,
    /// the last board layer on the back.
    pub fn get_layer(&self, ctx: &ItemCtx<'_>) -> usize {
        if self.is_front {
            0
        } else {
            ctx.rules.layer_structure().count().checked_sub(1).expect(
                "ComponentOutline.getLayer: the board has no layers — Java answers -1 here \
                     (ComponentOutline.java:99)",
            )
        }
    }

    /// Port of `ComponentOutline.getArea` (ComponentOutline.java:191-216): the same four steps
    /// as `ObstacleArea.getArea`, with `!isFront` where the obstacle area has `sideChanged`.
    pub fn get_area(&self, ctx: &ItemCtx<'_>) -> &Area {
        self.absolute_area.get_or_init(|| {
            absolute_area_of(
                &self.relative_area,
                !self.is_front,
                self.rotation_in_degree,
                &self.translation,
                ctx.components.get_flip_style_rotate_first(),
            )
        })
    }

    /// Port of `ComponentOutline.tileShapeCount` (ComponentOutline.java:129-132), whose whole
    /// body is `return 0;` — a component outline is drawn but never inserted into a search tree
    /// as tiles (its `calculateTreeShapes` returns `new TileShape[0]`,
    /// ComponentOutline.java:134-137).
    pub fn tile_shape_count(&self) -> usize {
        0
    }

    /// Port of `ComponentOutline.boundingBox` (ComponentOutline.java:139-142).
    pub fn bounding_box(&self, ctx: &ItemCtx<'_>) -> IntBox {
        self.get_area(ctx).bounding_box()
    }

    /// Port of `ComponentOutline.translateBy` (ComponentOutline.java:144-148).
    pub fn translate_by(&mut self, vector: &Vector) {
        self.translation = self.translation.add(vector);
        self.clear_derived_data();
    }

    /// Port of `ComponentOutline.turn90Degree` (ComponentOutline.java:177-189).
    pub fn turn_90_degree(&mut self, factor: i32, pole: &IntPoint) {
        self.rotation_in_degree =
            wrap_into_a_full_turn(self.rotation_in_degree + f64::from(factor) * 90.0);
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .turn_90_degree(factor, &Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.clear_derived_data();
    }

    /// Port of `ComponentOutline.rotateApprox` (ComponentOutline.java:158-175), the mirror image
    /// of `ObstacleArea.rotateApprox`: the complement is taken for a **back-side** outline.
    pub fn rotate_approx(&mut self, angle_in_degree: f64, pole: &FloatPoint, ctx: &ItemCtx<'_>) {
        let mut turn_angle = angle_in_degree;
        if !self.is_front && ctx.components.get_flip_style_rotate_first() {
            turn_angle = 360.0 - angle_in_degree;
        }
        self.rotation_in_degree = wrap_into_a_full_turn(self.rotation_in_degree + turn_angle);
        let new_translation = self
            .translation
            .to_float()
            .rotate(angle_in_degree.to_radians(), pole);
        self.translation = Point::Int(new_translation.round()).difference_by(&Point::ZERO);
        self.clear_derived_data();
    }

    /// Port of `ComponentOutline.changePlacementSide` (ComponentOutline.java:150-156).
    pub fn change_placement_side(&mut self, pole: &IntPoint) {
        self.is_front = !self.is_front;
        let rel_location = Point::ZERO.translate_by(&self.translation);
        self.translation = rel_location
            .mirror_vertical(&Point::Int(*pole))
            .difference_by(&Point::ZERO);
        self.clear_derived_data();
    }

    /// Port of `ComponentOutline.clearDerivedData` (ComponentOutline.java:218-221).
    //
    // Java quirk: alone among the six `clearDerivedData` overrides this one does **not** call
    // `super.clearDerivedData()` (Item.java:1060-1065), so the cached tree shapes and the
    // autoroute scratch of a component outline survive every transform. Reproduced — the header
    // is deliberately left alone. See docs/java-quirks.md.
    pub fn clear_derived_data(&mut self) {
        self.absolute_area.take();
    }
}

// not ported: a convex-pieces memo on [`ComponentOutline`] — the Task 7 ruling names this class
// alongside `ObstacleAreaData`, but nothing splits a component outline: `tileShapeCount` is
// literally `return 0` (ComponentOutline.java:129-132) and `calculateTreeShapes` is
// `return new TileShape[0]` (ComponentOutline.java:134-137), so a cache here would have no
// reader in either language.

#[cfg(test)]
mod tests {
    use super::*;

    /// `global-constraints.md` keeps `Item` shareable across threads, which is why the memo
    /// fields are `OnceLock` and not `Cell`/`RefCell`.
    #[test]
    fn area_types_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ObstacleAreaData>();
        assert_send_sync::<ObstacleArea>();
        assert_send_sync::<ConductionArea>();
        assert_send_sync::<ViaObstacleArea>();
        assert_send_sync::<ComponentObstacleArea>();
        assert_send_sync::<ComponentOutline>();
    }

    #[test]
    fn wrap_into_a_full_turn_matches_javas_two_while_loops() {
        // ObstacleArea.java:216-221.
        assert_eq!(wrap_into_a_full_turn(300.0 + 180.0), 120.0);
        assert_eq!(wrap_into_a_full_turn(30.0 - 180.0), 210.0);
        assert_eq!(wrap_into_a_full_turn(0.0), 0.0);
        assert_eq!(wrap_into_a_full_turn(360.0), 0.0);
        assert_eq!(wrap_into_a_full_turn(-360.0), 0.0);
        assert_eq!(wrap_into_a_full_turn(359.5), 359.5);
    }

    #[test]
    fn partial_eq_ignores_the_absolute_area_memo() {
        // The memo is derived state, so two areas that differ only in what has been computed so
        // far are equal.
        let data = || {
            ObstacleAreaData::new(
                Area::Shape(fr_geometry::Shape::Tile(TileShape::Box(
                    IntBox::from_coords(0, 0, 10, 10),
                ))),
                0,
                Vector::new(1, 2),
                0.0,
                false,
                None,
            )
        };
        let a = data();
        let b = data();
        a.absolute_area
            .set(Area::Shape(fr_geometry::Shape::Tile(TileShape::Box(
                IntBox::from_coords(1, 2, 11, 12),
            ))))
            .expect("a fresh OnceLock is empty");
        assert_eq!(a, b);
    }
}
