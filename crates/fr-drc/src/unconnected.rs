//! `drc.UnconnectedItems`: one unconnected **net**, or one dangling item.
//!
//! Java: `drc/UnconnectedItems.java`. The class carries four fields and four constructors and no
//! methods at all; `DesignRulesChecker.getAllUnconnectedItems` (ported in
//! [`crate::checker`]) is its only producer and `generateReport` its only consumer.

use fr_board::ItemId;

/// Port of `drc.UnconnectedItems` (UnconnectedItems.java:15-52) — despite the name, one instance
/// is one unconnected **net** (its own javadoc says so, :8-13), or, for the two dangling kinds,
/// one item.
#[derive(Debug, Clone, PartialEq)]
pub struct UnconnectedItems {
    /// Java `firstItem` (UnconnectedItems.java:18): the representative of the first connected
    /// group, or the dangling item itself.
    pub first_item: ItemId,
    /// Java `secondItem` (UnconnectedItems.java:21): the representative of the second connected
    /// group. `null` — here `None` — for the two dangling kinds
    /// (DesignRulesChecker.java:161, :172).
    pub second_item: Option<ItemId>,
    /// Java `allItems` (UnconnectedItems.java:24): every item of the two disconnected groups
    /// (DesignRulesChecker.java:143-146).
    pub all_items: Vec<ItemId>,
    /// Java `type` (UnconnectedItems.java:27), a `String` with three literal values.
    pub kind: UnconnectedKind,
}

/// Java's `String type` with its three literal values (UnconnectedItems.java:31, :36, :41 and
/// DesignRulesChecker.java:161, :172).
///
/// `"unconnectedItems"` is the HEAD spelling (plan-5 ruling 1); ruling 2's KiCad flavor writes
/// `unconnected_items` for the same thing. The literals themselves belong to the report layer's
/// key table (Task 7), not here — this enum is the closed set they name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnconnectedKind {
    /// `"unconnectedItems"` — a net whose items fall into two or more connected groups.
    UnconnectedItems,
    /// `"track_dangling"` — a trace with a contact-free end (DesignRulesChecker.java:161).
    TrackDangling,
    /// `"via_dangling"` — a via that `isTail()` (DesignRulesChecker.java:172).
    ViaDangling,
}

impl UnconnectedItems {
    /// Port of `UnconnectedItems(Item, Item)` (UnconnectedItems.java:30-32): two representatives,
    /// `allItems` defaulted to the pair, kind [`UnconnectedKind::UnconnectedItems`].
    // renamed: the four constructors -> new_pair / new_with_all_items / new_typed. Rust has no
    // overloading, and the two `type`-less ones differ only in whether `allItems` is given.
    //
    // Note — not a marker: this constructor is ported, but nothing in the Java tree calls it.
    // `getAllUnconnectedItems` uses the three-argument `allItems` form
    // (`DesignRulesChecker.java:146`) and the `type` form (`:161`, `:172`). It is kept because it
    // is public API of the ported class and costs one line; if Task 11's audit prefers a smaller
    // surface it can go.
    pub fn new_pair(first_item: ItemId, second_item: ItemId) -> Self {
        UnconnectedItems {
            first_item,
            second_item: Some(second_item),
            all_items: vec![first_item, second_item],
            kind: UnconnectedKind::UnconnectedItems,
        }
    }

    /// Port of `UnconnectedItems(Item, Item, List<Item>)` (UnconnectedItems.java:35-37) — the
    /// form `getAllUnconnectedItems` uses for a disconnected net (DesignRulesChecker.java:146).
    pub fn new_with_all_items(
        first_item: ItemId,
        second_item: ItemId,
        all_items: Vec<ItemId>,
    ) -> Self {
        UnconnectedItems {
            first_item,
            second_item: Some(second_item),
            all_items,
            kind: UnconnectedKind::UnconnectedItems,
        }
    }

    /// Port of `UnconnectedItems(Item, Item, String)` (UnconnectedItems.java:40-42) — the form
    /// the two dangling phases use, always with a `null` second item
    /// (DesignRulesChecker.java:161, :172).
    ///
    /// Java's `allItems` is then `Arrays.asList(firstItem, secondItem)` (`:41`), i.e. a
    /// **two-element list whose second element is `null`**. Nothing ever reads it: the report's
    /// converter branches on the type first and returns before touching `allItems`
    /// (DesignRulesChecker.java:365-394). A `Vec<ItemId>` cannot hold a null, so the port stores
    /// just the items there are — the pair when a second item is given, the singleton when it is
    /// not.
    ///
    /// **Hand-off, Task 10:** `p5t2`'s algorithm-level dump prints each entry as
    /// `(type, firstId, secondId, itemIds)`. For the two dangling kinds the Java side's
    /// `itemIds` is `[<id>, null]` and the port's is `[<id>]`, so the driver must render Java's
    /// trailing `null` away (or render the port's list as `[<id>, null]`) rather than reporting
    /// a length mismatch. No other consumer sees the difference.
    // not ported: `UnconnectedItems(Item, Item, List<Item>, String)`'s
    // `allItems != null ? new ArrayList<>(allItems) : Arrays.asList(firstItem, secondItem)`
    // fallback (UnconnectedItems.java:48-49) — the port's `Vec` is never null, so the second arm
    // is unreachable and the first is the plain copy every constructor above performs.
    pub fn new_typed(
        first_item: ItemId,
        second_item: Option<ItemId>,
        kind: UnconnectedKind,
    ) -> Self {
        UnconnectedItems {
            first_item,
            second_item,
            all_items: std::iter::once(first_item).chain(second_item).collect(),
            kind,
        }
    }
}
