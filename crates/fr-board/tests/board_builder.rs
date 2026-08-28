//! A two-layer board fixture with two pins, two traces and a board outline, for the search-tree
//! tests.
//!
//! There is no `Board` yet (Task 11), so this assembles the pieces `ShapeSearchTree` and
//! `SearchTreeManager` need by hand: a [`BoardLibrary`], [`Components`], [`BoardRules`], the
//! board bounding box, and the item map keyed by [`ItemId`].
//!
//! # Provenance
//!
//! Every number here is the input of `scripts/differential/java/P2T10.java`, which builds the
//! *same* board through the real `app.freerouting.board.facade.BasicBoard` on JDK 25 and prints
//! the trees it produces. The item ids match Java's (`BasicBoard.insertOutline` takes 1, the two
//! pins 2 and 3, the two traces 4 and 5), so the expectations in `tests/search_tree.rs` can be
//! copied from that driver's output verbatim.

#![allow(dead_code)]

use std::collections::BTreeMap;

use fr_board::prelude::*;
use fr_geometry::{
    IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, PolylineShapeRef, Shape, TileShape,
    Vector,
};

/// The board bounding box the Java driver passes to `new BasicBoard(...)`.
pub const BOUNDING_BOX: IntBox = IntBox {
    ll: IntPoint {
        x: -10_000,
        y: -10_000,
    },
    ur: IntPoint {
        x: 10_000,
        y: 10_000,
    },
};

/// The clearance class the Java driver appends as `"wide"`.
pub const WIDE_CLEARANCE_CLASS: usize = 2;

/// Everything an [`ItemCtx`] needs, plus the item map the search-tree queries take.
pub struct BoardFixture {
    pub library: BoardLibrary,
    pub components: Components,
    pub rules: BoardRules,
    pub bounding_box: IntBox,
    pub items: BTreeMap<ItemId, Item>,
    pub manager: SearchTreeManager,
}

impl BoardFixture {
    /// The default board: `AngleRestriction::FortyFiveDegree`, which is `BoardRules`' own
    /// default (BoardRules.java:31).
    pub fn new() -> BoardFixture {
        BoardFixture::with_angle(AngleRestriction::FortyFiveDegree)
    }

    /// The same board with `rules.traceAngleRestriction` set to `angle` — the Java driver's
    /// modes 0 (`FORTYFIVE_DEGREE`), 1 (`NINETY_DEGREE`) and 2 (`NONE`).
    pub fn with_angle(angle: AngleRestriction) -> BoardFixture {
        let layer_structure = layers();
        // ClearanceMatrix.getDefaultInstance(ls, 200), then `appendClass("wide")` and the two
        // asymmetric `setValue` calls the driver makes. Java's `setValue(classI, classJ, value)`
        // writes `row[classJ].column[classI]` (ClearanceMatrix.java:100-101), so
        // `setValue(2, 1, 600)` is read back as `getValue(2, 1, ...)`, not `getValue(1, 2, ...)`.
        let mut clearance_matrix = ClearanceMatrix::get_default_instance(&layer_structure, 200);
        assert!(clearance_matrix.append_class("wide"));
        clearance_matrix.set_value_on_all_layers(2, 1, 600);
        clearance_matrix.set_value_on_all_layers(2, 2, 800);
        let mut rules = BoardRules::new(layers(), clearance_matrix);
        rules.trace_angle_restriction = angle;

        let mut padstacks = Padstacks::new(layers());
        // `{new IntBox(-50,-50,50,50), null}` — an SMD pad on layer 0 only.
        let smd = padstacks.add(
            "smd",
            vec![
                Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                    -50, -50, 50, 50,
                )))),
                None,
            ],
            false,
            false,
        );
        // A through octagon on both layers.
        let through_shape = Shape::Tile(TileShape::Octagon(IntOctagon::new(
            -70, -70, 70, 70, -140, 140, -140, 140,
        )));
        let through = padstacks.add(
            "thru",
            vec![Some(through_shape.clone()), Some(through_shape)],
            true,
            false,
        );

        let mut packages = Packages::new();
        let package = packages.add(
            "pkg",
            vec![
                PackagePin::new("P1", smd, IntVector::new(-500, 0).into(), 0.0),
                PackagePin::new("P2", through, IntVector::new(500, 0).into(), 0.0),
            ],
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            true,
        );
        let library = BoardLibrary::new(padstacks, packages);

        let mut components = Components::new();
        // `components.add(new IntPoint(0, 0), 0, true, pkg)` — component id 1.
        components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, package);

        let mut items = BTreeMap::new();
        // `BasicBoard.insertOutline(new PolylineShape[0], 0)` — id 1, no outline polygons, so it
        // contributes no tree shapes at all.
        items.insert(
            ItemId(1),
            Item::BoardOutline(BoardOutline::new(
                ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
                Vec::new(),
            )),
        );
        // `board.insertPin(1, 0, new int[]{1}, 1, UNFIXED)` — id 2, the SMD pad.
        items.insert(
            ItemId(2),
            Item::Pin(Pin::new(
                ItemHeader::new(ItemId(2), vec![1], 1, 1, FixedState::Unfixed),
                0,
            )),
        );
        // `board.insertPin(1, 1, new int[]{1}, 1, UNFIXED)` — id 3, the through pad.
        items.insert(
            ItemId(3),
            Item::Pin(Pin::new(
                ItemHeader::new(ItemId(3), vec![1], 1, 1, FixedState::Unfixed),
                1,
            )),
        );
        // `insertTraceWithoutCleaning(..., 0, 30, new int[]{1}, 1, UNFIXED)` — id 4.
        items.insert(ItemId(4), Item::Trace(signal_trace()));
        // The second trace, on net 2 and clearance class 2 — id 5.
        items.insert(ItemId(5), Item::Trace(wide_trace()));

        BoardFixture {
            library,
            components,
            rules,
            bounding_box: BOUNDING_BOX,
            items,
            manager: SearchTreeManager::new(),
        }
    }

    pub fn ctx(&self) -> ItemCtx<'_> {
        ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        }
    }

    /// Inserts every item into every tree of the fixture's manager, the way
    /// `SearchTreeManager.insertAllBoardItems` (SearchTreeManager.java:217-231) does.
    ///
    /// The item map and the manager are borrowed disjointly by moving the map out and back, so
    /// the `ItemCtx` (which borrows the library, the components and the rules) stays valid.
    pub fn insert_all(&mut self) {
        let mut items = std::mem::take(&mut self.items);
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in items.values_mut() {
            self.manager.insert(item, &ctx);
        }
        self.items = items;
    }

    /// `SearchTreeManager.getAutorouteTree(clearanceClassIndex)` over the fixture's items.
    ///
    /// The items are handed over in **descending** id order, because that is the order
    /// `board.itemList` iterates in: `UndoableObjects` stores them in a
    /// `ConcurrentSkipListMap<Storable, …>` (UndoableObjects.java:21,37) keyed by
    /// `Item.compareTo`, whose subtraction is reversed (Item.java:98, quirk #44). Verified on
    /// the JVM — `board.itemList` and `board.getItems()` both yield `5 4 3 2 1` for this board.
    /// The order decides the tree's *structure*, so it is load-bearing; see
    /// [`Self::items_in_board_order`].
    pub fn build_autoroute_tree(&mut self, clearance_class_index: usize) -> TreeId {
        let mut items = std::mem::take(&mut self.items);
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
        let id = self
            .manager
            .get_autoroute_tree(clearance_class_index, &mut refs, &ctx)
            .id();
        drop(refs);
        self.items = items;
        id
    }

    /// The items in `board.itemList` order — **descending** id (see
    /// [`Self::build_autoroute_tree`]). Every Java method that walks the board's item list uses
    /// this order; `Board` (Task 11) must too.
    pub fn items_in_board_order(&mut self) -> Vec<&mut Item> {
        self.items.values_mut().rev().collect()
    }

    /// The tree with this id, whichever slot of the manager holds it.
    pub fn tree(&self, id: TreeId) -> &ShapeSearchTree {
        self.manager
            .trees()
            .find(|tree| tree.id() == id)
            .expect("the fixture only asks for trees it built")
    }
}

impl Default for BoardFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// `{new Layer("front", true), new Layer("back", true)}`.
pub fn layers() -> LayerStructure {
    LayerStructure::new(vec![Layer::new("front", true), Layer::new("back", true)])
}

/// Item 4: net 1, layer 0, half width 30, clearance class 1.
pub fn signal_trace() -> PolylineTrace {
    PolylineTrace::new(
        ItemHeader::new(ItemId(4), vec![1], 1, 0, FixedState::Unfixed),
        Polyline::from_points(&[
            Point::new(-500, 0),
            Point::new(0, 0),
            Point::new(0, 400),
            Point::new(500, 400),
        ]),
        0,
        30,
        None,
    )
}

/// Item 5: net 2, layer 0, half width 40, clearance class 2 (the wide class).
pub fn wide_trace() -> PolylineTrace {
    PolylineTrace::new(
        ItemHeader::new(ItemId(5), vec![2], 2, 0, FixedState::Unfixed),
        Polyline::from_points(&[
            Point::new(-800, 300),
            Point::new(-800, 900),
            Point::new(300, 900),
        ]),
        0,
        40,
        None,
    )
}

#[test]
fn the_fixture_matches_the_java_driver_board() {
    // P2T10.java mode 0, the `item ...` lines: ids, nets, clearance classes, layer spans, tile
    // shape counts and bounding boxes, all read back from the real `BasicBoard`.
    let f = BoardFixture::new();
    let ctx = f.ctx();
    let describe = |id: u32| {
        let item = &f.items[&ItemId(id)];
        (
            item.net_nos().to_vec(),
            item.header().clearance_class(),
            item.first_layer(&ctx),
            item.last_layer(&ctx),
            item.tile_shape_count(&ctx),
            item.bounding_box(&ctx),
        )
    };
    assert_eq!(
        describe(2),
        (
            vec![1],
            1,
            0,
            0,
            1,
            IntBox::from_coords(-550, -50, -450, 50)
        )
    );
    assert_eq!(
        describe(3),
        (vec![1], 1, 0, 1, 2, IntBox::from_coords(430, -70, 570, 70))
    );
    assert_eq!(
        describe(4),
        (
            vec![1],
            1,
            0,
            0,
            3,
            IntBox::from_coords(-530, -30, 530, 430)
        )
    );
    assert_eq!(
        describe(5),
        (
            vec![2],
            2,
            0,
            0,
            2,
            IntBox::from_coords(-840, 260, 340, 940)
        )
    );
    // The outline has no polygons, so `lineCount() * layerCount` is 0.
    assert_eq!(f.items[&ItemId(1)].tile_shape_count(&ctx), 0);
}

// ---------------------------------------------------------------------------------------------
// The area/outline/via board (`P2T10.java` mode 4)
// ---------------------------------------------------------------------------------------------

/// A three-layer board with a real outline polygon, an L-shaped obstacle area, a conduction
/// area and a through via whose **middle layer has no pad** — the four `calculateTreeShapes`
/// overloads [`BoardFixture`] leaves untouched, plus the hole-clearance path.
///
/// `P2T10.java` mode 4 builds the same board through `BasicBoard`; the ids match
/// (outline 1, via 2, obstacle area 3, conduction area 4).
pub struct AreaFixture {
    pub library: BoardLibrary,
    pub components: Components,
    pub rules: BoardRules,
    pub bounding_box: IntBox,
    pub items: BTreeMap<ItemId, Item>,
    pub manager: SearchTreeManager,
}

impl AreaFixture {
    pub fn new() -> AreaFixture {
        let layers = || {
            LayerStructure::new(vec![
                Layer::new("front", true),
                Layer::new("inner", true),
                Layer::new("back", true),
            ])
        };
        let clearance_matrix = ClearanceMatrix::get_default_instance(&layers(), 200);
        let mut rules = BoardRules::new(layers(), clearance_matrix);
        // Non-zero, so `drillHoleObstacle` (ShapeSearchTree.java:1012-1028) and
        // `drillHoleClearanceDelta` (:1035-1074) both do something.
        rules.set_hole_clearance(300);

        let mut padstacks = Padstacks::new(layers());
        let pad = Shape::Tile(TileShape::Box(IntBox::from_coords(-80, -80, 80, 80)));
        let via_pad = padstacks.add("via", vec![Some(pad.clone()), None, Some(pad)], true, false);
        let library = BoardLibrary::new(padstacks, Packages::new());

        let mut items = BTreeMap::new();
        items.insert(
            ItemId(1),
            Item::BoardOutline(BoardOutline::new(
                ItemHeader::new(ItemId(1), Vec::new(), 1, 0, FixedState::SystemFixed),
                vec![PolylineShapeRef::Polygon(
                    fr_geometry::PolygonShape::from_points(&[
                        Point::new(-3000, -2000),
                        Point::new(3000, -2000),
                        Point::new(3000, 2000),
                        Point::new(-3000, 2000),
                    ]),
                )],
            )),
        );
        items.insert(
            ItemId(2),
            Item::Via(Via::new(
                ItemHeader::new(ItemId(2), vec![1], 1, 0, FixedState::Unfixed),
                via_pad,
                Point::new(1000, 0),
                true,
            )),
        );
        let l_shape =
            fr_geometry::Area::Shape(Shape::Polygon(fr_geometry::PolygonShape::from_points(&[
                Point::new(0, 0),
                Point::new(2000, 0),
                Point::new(2000, 1000),
                Point::new(1000, 1000),
                Point::new(1000, 2000),
                Point::new(0, 2000),
            ])));
        items.insert(
            ItemId(3),
            Item::ObstacleArea(ObstacleArea::new(
                ItemHeader::new(ItemId(3), Vec::new(), 1, 0, FixedState::Unfixed),
                ObstacleAreaData::new(l_shape, 0, Vector::ZERO, 0.0, false, None),
            )),
        );
        let square = fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -2000, -1500, -1000, -500,
        ))));
        items.insert(
            ItemId(4),
            Item::ConductionArea(ConductionArea::new(
                ItemHeader::new(ItemId(4), vec![2], 1, 0, FixedState::Unfixed),
                ObstacleAreaData::new(square, 2, Vector::ZERO, 0.0, false, None),
                true,
            )),
        );

        let mut fixture = AreaFixture {
            library,
            components: Components::new(),
            rules,
            bounding_box: IntBox::from_coords(-5000, -5000, 5000, 5000),
            items,
            manager: SearchTreeManager::new(),
        };
        fixture.insert_all();
        fixture
    }

    pub fn ctx(&self) -> ItemCtx<'_> {
        ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        }
    }

    fn insert_all(&mut self) {
        let mut items = std::mem::take(&mut self.items);
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in items.values_mut() {
            self.manager.insert(item, &ctx);
        }
        self.items = items;
    }

    pub fn build_autoroute_tree(&mut self, clearance_class_index: usize) -> TreeId {
        let mut items = std::mem::take(&mut self.items);
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
        let id = self
            .manager
            .get_autoroute_tree(clearance_class_index, &mut refs, &ctx)
            .id();
        drop(refs);
        self.items = items;
        id
    }

    /// `BoardOutline.generateKeepoutOutside(true)` (BoardOutline.java:229-243), whose search-tree
    /// half is Task 11's `Board::generate_keepout_outside`.
    pub fn generate_keepout_outside(&mut self) {
        let mut outline = self.items.remove(&ItemId(1)).expect("the outline");
        self.manager.remove(&mut outline);
        if let Item::BoardOutline(o) = &mut outline {
            o.generate_keepout_outside(true);
        }
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        self.manager.insert(&mut outline, &ctx);
        self.items.insert(ItemId(1), outline);
    }

    pub fn tree(&self, id: TreeId) -> &ShapeSearchTree {
        self.manager
            .trees()
            .find(|tree| tree.id() == id)
            .expect("the fixture only asks for trees it built")
    }
}

impl Default for AreaFixture {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------------------------
// The two-trace board for the entry-surgery methods (`P2T10.java` mode 5)
// ---------------------------------------------------------------------------------------------

/// Two traces meeting head-to-tail at `(0, 400)`, on a two-layer board with an empty outline —
/// the fixture `mergeEntriesAtEnd`, `mergeEntriesInFront` and `reuseEntriesAfterCutout` are
/// exercised on. Ids match `P2T10.java` mode 5: outline 1, `trace_a` 2, `trace_b` 3.
pub struct TraceFixture {
    pub library: BoardLibrary,
    pub components: Components,
    pub rules: BoardRules,
    pub bounding_box: IntBox,
    pub items: BTreeMap<ItemId, Item>,
    pub manager: SearchTreeManager,
}

impl TraceFixture {
    pub fn new() -> TraceFixture {
        let rules = BoardRules::new(
            layers(),
            ClearanceMatrix::get_default_instance(&layers(), 200),
        );
        let mut items = BTreeMap::new();
        items.insert(
            ItemId(1),
            Item::BoardOutline(BoardOutline::new(
                ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
                Vec::new(),
            )),
        );
        items.insert(
            ItemId(2),
            Item::Trace(trace_piece(2, &[(-500, 0), (0, 0), (0, 400)])),
        );
        items.insert(
            ItemId(3),
            Item::Trace(trace_piece(3, &[(0, 400), (500, 400), (500, 900)])),
        );
        let mut fixture = TraceFixture {
            library: BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
            components: Components::new(),
            rules,
            bounding_box: BOUNDING_BOX,
            items,
            manager: SearchTreeManager::new(),
        };
        let mut items = std::mem::take(&mut fixture.items);
        let ctx = ItemCtx {
            library: &fixture.library,
            components: &fixture.components,
            rules: &fixture.rules,
            bounding_box: &fixture.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in items.values_mut() {
            fixture.manager.insert(item, &ctx);
        }
        fixture.items = items;
        fixture
    }

    /// The same two-layer board carrying **only** a board outline whose edges run in none of the
    /// trees' directions, so the bands `calculateTreeShapes(BoardOutline)` builds around them are
    /// `Simplex`es. `P2T10.java` mode 7 builds the same board.
    pub fn skewed_outline() -> TraceFixture {
        let rules = BoardRules::new(
            layers(),
            ClearanceMatrix::get_default_instance(&layers(), 200),
        );
        let mut items = BTreeMap::new();
        items.insert(
            ItemId(1),
            Item::BoardOutline(BoardOutline::new(
                ItemHeader::new(ItemId(1), Vec::new(), 1, 0, FixedState::SystemFixed),
                vec![PolylineShapeRef::Polygon(
                    fr_geometry::PolygonShape::from_points(&[
                        Point::new(0, 0),
                        Point::new(3000, 500),
                        Point::new(1000, 2500),
                    ]),
                )],
            )),
        );
        let mut fixture = TraceFixture {
            library: BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
            components: Components::new(),
            rules,
            bounding_box: IntBox::from_coords(-5000, -5000, 5000, 5000),
            items,
            manager: SearchTreeManager::new(),
        };
        let mut items = std::mem::take(&mut fixture.items);
        let ctx = ItemCtx {
            library: &fixture.library,
            components: &fixture.components,
            rules: &fixture.rules,
            bounding_box: &fixture.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in items.values_mut() {
            fixture.manager.insert(item, &ctx);
        }
        fixture.items = items;
        fixture
    }

    /// The same two-layer board carrying exactly the two traces described by `a` and `b`, so a
    /// test can build the head-to-head and tail-to-tail pairs `mergeEntriesInFront`'s and
    /// `mergeEntriesAtEnd`'s `changeOrder` branches (ShapeSearchTree.java:176,251) need.
    /// `P2T10.java` mode 8 builds the same two boards.
    pub fn merge_pair(a: &[Point], b: &[Point]) -> TraceFixture {
        let rules = BoardRules::new(
            layers(),
            ClearanceMatrix::get_default_instance(&layers(), 200),
        );
        let mut items = BTreeMap::new();
        items.insert(
            ItemId(1),
            Item::BoardOutline(BoardOutline::new(
                ItemHeader::new(ItemId(1), Vec::new(), 0, 0, FixedState::SystemFixed),
                Vec::new(),
            )),
        );
        for (index, corners) in [a, b].into_iter().enumerate() {
            let id = ItemId(index as u32 + 2);
            items.insert(
                id,
                Item::Trace(PolylineTrace::new(
                    ItemHeader::new(id, vec![1], 1, 0, FixedState::Unfixed),
                    Polyline::from_points(corners),
                    0,
                    30,
                    None,
                )),
            );
        }
        let mut fixture = TraceFixture {
            library: BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
            components: Components::new(),
            rules,
            bounding_box: BOUNDING_BOX,
            items,
            manager: SearchTreeManager::new(),
        };
        let mut items = std::mem::take(&mut fixture.items);
        let ctx = ItemCtx {
            library: &fixture.library,
            components: &fixture.components,
            rules: &fixture.rules,
            bounding_box: &fixture.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        for item in items.values_mut() {
            fixture.manager.insert(item, &ctx);
        }
        fixture.items = items;
        fixture
    }

    /// `SearchTreeManager.getAutorouteTree(clearanceClassIndex)` over this fixture's items.
    pub fn build_autoroute_tree(&mut self, clearance_class_index: usize) -> TreeId {
        let mut items = std::mem::take(&mut self.items);
        let ctx = ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        };
        let mut refs: Vec<&mut Item> = items.values_mut().rev().collect();
        let id = self
            .manager
            .get_autoroute_tree(clearance_class_index, &mut refs, &ctx)
            .id();
        drop(refs);
        self.items = items;
        id
    }

    pub fn ctx(&self) -> ItemCtx<'_> {
        ItemCtx {
            library: &self.library,
            components: &self.components,
            rules: &self.rules,
            bounding_box: &self.bounding_box,
            max_tree_shape_width: DEFAULT_MAX_TREE_SHAPE_WIDTH,
        }
    }

    pub fn take_trace(&mut self, id: u32) -> PolylineTrace {
        match self.items.remove(&ItemId(id)).expect("a trace") {
            Item::Trace(trace) => trace,
            _ => panic!("item {id} is not a trace"),
        }
    }

    pub fn put_trace(&mut self, id: u32, trace: PolylineTrace) {
        self.items.insert(ItemId(id), Item::Trace(trace));
    }

    pub fn tree(&self) -> &ShapeSearchTree {
        self.manager.get_default_tree()
    }
}

impl Default for TraceFixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A half-width-30 trace on layer 0, net 1, clearance class 1.
pub fn trace_piece(id: u32, corners: &[(i32, i32)]) -> PolylineTrace {
    PolylineTrace::new(
        ItemHeader::new(ItemId(id), vec![1], 1, 0, FixedState::Unfixed),
        Polyline::from_points(
            &corners
                .iter()
                .map(|(x, y)| Point::new(*x, *y))
                .collect::<Vec<_>>(),
        ),
        0,
        30,
        None,
    )
}

// ---------------------------------------------------------------------------------------------
// The `Board` fixture (`P2T11.java`)
// ---------------------------------------------------------------------------------------------

/// The board `scripts/differential/java/P2T11.java` builds through the real `RoutingBoard`, and
/// against which every expectation in `tests/board.rs` is transcribed.
///
/// Two layers, a 10000-square bounding box, a 5000-square outline polygon, one two-pin component
/// (an SMD pad on layer 0 at `(-1000, 0)`, a through pad on both layers at `(1000, 1000)`), and:
///
/// | id | item |
/// |---|---|
/// | 1 | the `BoardOutline`, clearance class 1 |
/// | 2 | pin P1 (SMD, layer 0), net 1 |
/// | 3 | pin P2 (through, layers 0-1), net 1 |
/// | 4 | a trace on layer 0 from the SMD pin to `(0, 0)`, net 1, half width 30 |
/// | 5 | a trace on layer 1 from `(0, 0)` to the through pin, net 1, half width 30 |
/// | 6 | a via at `(0, 0)` joining the two, net 1 |
/// | 7 | an obstacle area on layer 0, well away from everything |
/// | 8 | a conduction area on layer 0, net 2 |
pub fn p2t11_board() -> Board {
    p2t11_board_with_host_cad(false)
}

/// The same board with `mode == 4`'s two changes: `AngleRestriction::NinetyDegree`, and a
/// [`Communication`] that names a host CAD system at resolution 10 — which lowers
/// `ShapeSearchTree.calculateTreeShapes(ObstacleArea)`'s section width from 50000 to
/// `min(500 * 10, 50000) = 5000` (ShapeSearchTree.java:916-920). It also carries a ninth item: an
/// 18000-wide obstacle area, so that section width actually splits something.
pub fn p2t11_host_cad_board() -> Board {
    p2t11_board_with_host_cad(true)
}

fn p2t11_board_with_host_cad(host_cad: bool) -> Board {
    let ls = layers();
    let mut clearance_matrix = ClearanceMatrix::get_default_instance(&ls, 200);
    assert!(clearance_matrix.append_class("wide"));
    clearance_matrix.set_value_on_all_layers(2, 1, 600);
    clearance_matrix.set_value_on_all_layers(2, 2, 800);
    let mut rules = BoardRules::new(layers(), clearance_matrix);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    if host_cad {
        rules.trace_angle_restriction = AngleRestriction::NinetyDegree;
    }

    let mut padstacks = Padstacks::new(layers());
    let smd_pad = padstacks.add(
        "smd",
        vec![
            Some(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -50, -50, 50, 50,
            )))),
            None,
        ],
        false,
        false,
    );
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru_pad = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let pkg = packages.add(
        "pkg",
        vec![
            PackagePin::new("P1", smd_pad, IntVector::new(-1000, 0).into(), 0.0),
            PackagePin::new("P2", thru_pad, IntVector::new(1000, 1000).into(), 0.0),
        ],
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);

    let outline = vec![PolylineShapeRef::Polygon(
        fr_geometry::PolygonShape::from_points(&[
            Point::new(-5000, -5000),
            Point::new(5000, -5000),
            Point::new(5000, 5000),
            Point::new(-5000, 5000),
        ]),
    )];
    let mut board = Board::new(
        outline,
        1,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        if host_cad {
            Communication::new(
                Unit::Mil,
                10,
                ItemIdGenerator::new(),
                Some("KiCad".to_string()),
                Some("7.0".to_string()),
            )
        } else {
            Communication::default()
        },
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);

    board.insert_pin(1, 0, vec![1], 1, FixedState::Unfixed);
    board.insert_pin(1, 1, vec![1], 1, FixedState::Unfixed);
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(-1000, 0), Point::new(0, 0)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(0, 0),
            Point::new(1000, 0),
            Point::new(1000, 1000),
        ]),
        1,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board
        .insert_via(
            thru_pad,
            Point::new(0, 0),
            vec![1],
            1,
            FixedState::Unfixed,
            true,
        )
        .expect("no normalisation failure");
    board.insert_obstacle(
        fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            2000, 2000, 3000, 3000,
        )))),
        0,
        1,
        FixedState::Unfixed,
    );
    board.insert_conduction_area(
        fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            -3000, -3000, -2000, -2000,
        )))),
        0,
        vec![2],
        1,
        true,
        FixedState::Unfixed,
    );
    if host_cad {
        // Item 9: wider than the lowered section width, so `divideIntoSections` splits it.
        board.insert_obstacle(
            fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
                -9000, -9000, 9000, -8000,
            )))),
            0,
            1,
            FixedState::Unfixed,
        );
    }
    board
}

/// The board `P2T11.java` mode 5 builds for `ShapeTraceEntries`: two layers, no components, and
/// three traces crossing a square at the origin — item 2 on the own net (1), items 3 and 4 on a
/// foreign net (2).
pub fn shove_board() -> Board {
    let ls = layers();
    let cm = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(layers(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(Padstacks::new(layers()), Packages::new()),
        Components::new(),
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);
    board.rules.nets.add("N2", 1, false, default_class);
    for (corners, net) in [
        ([(-3000, 0), (3000, 0)], 1),
        ([(0, -3000), (0, 3000)], 2),
        ([(-3000, 200), (3000, 200)], 2),
    ] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&corners.map(|(x, y)| Point::new(x, y))),
            0,
            30,
            vec![net],
            1,
            FixedState::Unfixed,
        );
    }
    board
}

/// The board `P2T11.java` mode 6 builds: a genuine cycle (two traces between the same pair of
/// vias) plus a trace whose two ends both land inside one conduction area, and two pinless
/// components (one per side) so `ComponentObstacleArea.isFront` has something to read.
///
/// Ids: 1 outline, 2 via A at `(0, 0)`, 3 via B at `(2000, 0)`, 4 the direct trace, 5 the
/// detour trace, 6 the conduction area, 7 the trace inside it.
pub fn cycle_board() -> (Board, PadstackId) {
    let ls = layers();
    let cm = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(layers(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut padstacks = Padstacks::new(layers());
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru_pad = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let mut packages = Packages::new();
    let pkg = packages.add(
        "pkg",
        Vec::new(),
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    );
    let mut components = Components::new();
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, true, pkg);
    components.add_with_generated_name(Some(Point::new(0, 0)), 0.0, false, pkg);
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, packages),
        components,
        Communication::default(),
    );
    for name in ["N1", "N2", "N3"] {
        board.rules.nets.add(name, 1, false, default_class);
    }
    for x in [0, 2000] {
        board
            .insert_via(
                thru_pad,
                Point::new(x, 0),
                vec![1],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("no normalisation failure");
    }
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(2000, 0)]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(0, 0),
            Point::new(0, 1000),
            Point::new(2000, 1000),
            Point::new(2000, 0),
        ]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board.insert_conduction_area(
        fr_geometry::Area::Shape(Shape::Tile(TileShape::Box(IntBox::from_coords(
            4000, 0, 6000, 2000,
        )))),
        0,
        vec![3],
        1,
        true,
        FixedState::Unfixed,
    );
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(4200, 200), Point::new(5800, 1800)]),
        0,
        30,
        vec![3],
        1,
        FixedState::Unfixed,
    );
    (board, thru_pad)
}

/// A board built for one purpose: to make `Item.isCycleRecu`'s contact walk visibly
/// order-sensitive. Nothing in `P2T11.java` reaches this shape, so the numbers below are the
/// fixture's own, not a transcription.
///
/// **Three** signal layers (so that three traces can meet at one via without contacting each
/// other), no outline, no components, everything on net 1:
///
/// | id | item |
/// |---|---|
/// | 1 | via `V` at `(0, 0)`, all three layers |
/// | 2 | via `W` at `(3000, 0)`, all three layers |
/// | 3 | trace `T` on layer 0, `(0,0) -> (0,-1000) -> (3000,-1000) -> (3000,0)` |
/// | 4 | trace `B` on layer 2, `(0,0) -> (0,2000)` — a dead-end stub off `V` |
/// | 5 | trace `A` on layer 1, `(0,0) -> (3000,0)` — the short way from `V` to `W` |
///
/// `T` is a cycle (`T`'s start contact `V`, then `A`, then `W`, which contacts `T` again), and
/// `V` is the via with three contacts: `{3, 4, 5}`. Walking them **descending** (Java's
/// `TreeSet<Item>` order, quirk #44) enters `A` first and returns before `B` is ever visited;
/// walking them ascending enters the dead end `B` first. Either way the answer is `true` — see
/// [`fr_board::Board::is_cycle_recu`]'s doc comment for why the boolean cannot differ — but the
/// set of visited items does, which is what `tests/board.rs` pins.
pub fn cycle_order_board() -> Board {
    let three_layers = LayerStructure::new(vec![
        Layer::new("front", true),
        Layer::new("inner", true),
        Layer::new("back", true),
    ]);
    let cm = ClearanceMatrix::get_default_instance(&three_layers, 200);
    let mut rules = BoardRules::new(three_layers.clone(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut padstacks = Padstacks::new(three_layers.clone());
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru_pad = padstacks.add(
        "thru",
        vec![
            Some(thru_shape.clone()),
            Some(thru_shape.clone()),
            Some(thru_shape),
        ],
        true,
        false,
    );
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);

    // 1: via V, 2: via W.
    for x in [0, 3000] {
        board
            .insert_via(
                thru_pad,
                Point::new(x, 0),
                vec![1],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("no normalisation failure");
    }
    // 3: the trace under test, layer 0, both ends on a via.
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[
            Point::new(0, 0),
            Point::new(0, -1000),
            Point::new(3000, -1000),
            Point::new(3000, 0),
        ]),
        0,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    // 4: the dead-end stub on layer 2 — reachable only from V.
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(0, 2000)]),
        2,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    // 5: the return path on layer 1, from V to W.
    board.insert_trace_without_cleaning(
        Polyline::from_points(&[Point::new(0, 0), Point::new(3000, 0)]),
        1,
        30,
        vec![1],
        1,
        FixedState::Unfixed,
    );
    board
}

/// A board built for one purpose: to make `Item.getConnectionItems`' outer contact walk visibly
/// order-sensitive under [`fr_board::StopConnectionOption::FanoutVia`], which Plan 2 Task 11
/// deferred for want of a fixture. Its numbers are the fixture's own, not a `P2T11.java`
/// transcription.
///
/// Two signal layers, no outline, no components, everything on net 1:
///
/// | id | item |
/// |---|---|
/// | 1 | via `S` at `(5000, 5000)`, both layers — the item the connection is asked for |
/// | 2 | via `F` at `(0, 0)`, both layers — the candidate fanout via |
/// | 3 | trace `Z` on layer 0, `(1000,1000) -> (1000,0)`, **shove-fixed**, two corners |
/// | 4 | trace `Y` on layer 0, `(1000,0) -> (0,0)` — short, and the only fanout evidence |
/// | 5 | trace `E` on layer 0, `(0,0) -> (0,-1000)` — a third arm at `F`, so the walk forks |
/// | 6 | trace `B` on layer 1, `(5000,5000) -> (0,0)` — `S` straight to `F` |
/// | 7 | trace `A` on layer 0, `(5000,5000) -> (1000,1000)` — `S` the long way round |
///
/// `isFanoutVia(F, result)` (Item.java:1206-1239) is true only through `Y`: `Y` is a short
/// contact trace of `F` whose own contacts include the shove-fixed two-corner trace `Z`
/// (Item.java:1227-1234). `E` and `B` are short too but their contacts hold no evidence. So `F`
/// stops the walk *unless* `Y` is already in `result`, which `Item.java:735` passes as
/// `ignoreItems`.
///
/// `S`'s contacts are `{6, 7}`. Descending (Java's order) walks branch `A` first — `7, 3, 4`,
/// forking at `Y` because `F` and `E` are both new contacts at `(0,0)` — and only then branch
/// `B`, which reaches `F` with `Y` already ignored and so **passes through** it. Ascending walks
/// `B` first, stops dead at `F`, and `F` never enters the result.
pub fn fanout_order_board() -> Board {
    let ls = layers();
    let cm = ClearanceMatrix::get_default_instance(&ls, 200);
    let mut rules = BoardRules::new(layers(), cm);
    rules.create_default_net_class();
    let default_class = rules.get_default_net_class();
    let mut padstacks = Padstacks::new(layers());
    let thru_shape = Shape::Tile(TileShape::Box(IntBox::from_coords(-70, -70, 70, 70)));
    let thru_pad = padstacks.add(
        "thru",
        vec![Some(thru_shape.clone()), Some(thru_shape)],
        true,
        false,
    );
    let mut board = Board::new(
        Vec::new(),
        0,
        BOUNDING_BOX,
        rules,
        BoardLibrary::new(padstacks, Packages::new()),
        Components::new(),
        Communication::default(),
    );
    board.rules.nets.add("N1", 1, false, default_class);

    // 1: S, 2: F.
    for (x, y) in [(5000, 5000), (0, 0)] {
        board
            .insert_via(
                thru_pad,
                Point::new(x, y),
                vec![1],
                1,
                FixedState::Unfixed,
                true,
            )
            .expect("no normalisation failure");
    }
    for (corners, layer, fixed) in [
        // 3: Z, the shove-fixed two-corner trace that is the fanout evidence.
        ([(1000, 1000), (1000, 0)], 0usize, FixedState::ShoveFixed),
        // 4: Y, the short contact trace of F that carries the evidence.
        ([(1000, 0), (0, 0)], 0, FixedState::Unfixed),
        // 5: E, the third arm at F — it makes the A branch fork at Y.
        ([(0, 0), (0, -1000)], 0, FixedState::Unfixed),
        // 6: B, S straight to F on layer 1.
        ([(5000, 5000), (0, 0)], 1, FixedState::Unfixed),
        // 7: A, S the long way round on layer 0.
        ([(5000, 5000), (1000, 1000)], 0, FixedState::Unfixed),
    ] {
        board.insert_trace_without_cleaning(
            Polyline::from_points(&corners.map(|(x, y)| Point::new(x, y))),
            layer,
            30,
            vec![1],
            1,
            fixed,
        );
    }
    board
}

/// A set of item ids in Java's `TreeSet<Item>` order — **descending** id (quirk #44) — as the
/// bare numbers, so a test can transcribe `P2T11.java`'s output verbatim.
pub fn descending(set: std::collections::BTreeSet<ItemId>) -> Vec<u32> {
    set.into_iter().rev().map(|id| id.0).collect()
}

/// The same for the `Vec<ItemId>` the item-list queries return (already descending).
pub fn nums(ids: impl IntoIterator<Item = ItemId>) -> Vec<u32> {
    ids.into_iter().map(|id| id.0).collect()
}
