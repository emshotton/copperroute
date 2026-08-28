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
use fr_geometry::{IntBox, IntOctagon, IntPoint, IntVector, Point, Polyline, Shape, TileShape};

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
