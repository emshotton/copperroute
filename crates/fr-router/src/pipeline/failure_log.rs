//! Port of `autoroute/RoutingFailureLog.java` (161 lines) — the per-item routing logbook.
//!
//! Two of the class's seven public members have a live caller and are ported;
//! [`RoutingFailureLog`]'s own doc lists the five that do not, each with its grep.

use std::collections::BTreeMap;

use fr_board::{Board, ItemId};

use crate::autoroute::AutorouteAttemptState;

/// Port of `autoroute.RoutingFailureLog` (RoutingFailureLog.java:13-161): "thread-safe logbook for
/// tracking routing failures per item. Helps detect when the router is stuck attempting the same
/// impossible connection repeatedly."
///
/// # `BTreeMap`, not a `ConcurrentHashMap`
///
/// Java's store is a `ConcurrentHashMap<Integer, ItemFailureInfo>` (`:19`). The port is
/// single-threaded (plan-7 §Tech Stack), and **both live readers** — `recordFailure` and
/// `getFailureCount` — are keyed lookups, so the iteration-order difference between a hash map
/// and a `BTreeMap` is unobservable (survey §3.1). The three members that *would* observe it
/// (`getUnroutableItems`, `hasUnroutableItems` and the `values()` stream behind them) are the
/// unported ones.
///
/// # Ownership note (scan ruling 2): the log is a parameter here, a board field in Java
///
/// Java hangs it off the board — `RoutingBoard.java:64` declares
/// `public final RoutingFailureLog failureLog`, `:91` constructs it, and
/// `AutoroutePassRunner.java:269, 272` reach it as `router.board.failureLog`. The port lifts it to
/// a caller-owned parameter of [`crate::pipeline::AutoroutePassRunner::run_single_thread`],
/// because `fr-board` must not depend on `fr-router` and this type is `fr-router`'s. That is a
/// deliberate `// renamed:` divergence, recorded at `crates/fr-board/src/board/mod.rs`; the
/// `Vec<String>` hook that stood in for the field there is **deleted** in the same commit, not
/// re-pointed, because nothing ever read or wrote it.
///
/// The divergence is not observable: `failureLog` is `final`, so no code path can swap one board's
/// log for another's, and the log is write-only apart from `getFailureCount`, whose only reader is
/// a log-message guard (`AutoroutePassRunner.java:273`).
///
/// # The five members with no live caller
///
/// * `shouldSkip` (`:56-63`) — one caller, `BatchAutorouterThread.java:324`. That class is
///   constructed only by `AutoroutePassRunner.runMultiThread` (`:62`), whose only caller is
///   `BatchAutorouter.autoroutePassMultiThread` (`:412`), which has **zero** callers in
///   `src/main` or `src/test` (survey §3.4). The whole multithreaded path is dead.
/// * `ItemFailureInfo.shouldGiveUp` (`:147-149`) — **not a top-level method**: it is declared
///   inside the nested class. Its three readers are `shouldSkip:62`, `getUnroutableItems:73` and
///   `hasUnroutableItems:86`, all three of which are themselves unported.
/// * `getUnroutableItems` (`:70-78`) and `hasUnroutableItems` (`:85-87`) — `grep -rn` over
///   `src/main src/test` finds nothing but the declarations.
/// * `clear` (`:104-106`) — likewise; `grep -rn "failureLog.clear"` is empty.
/// * `ItemFailureInfo.toString` (`:151-159`) — also **not a top-level method**, and unreachable:
///   `grep -rn ItemFailureInfo src/main src/test` outside `RoutingFailureLog.java` finds nothing,
///   so no caller can hold one to print.
///
// not ported: `RoutingFailureLog.shouldSkip` (`:56-63`) — its one caller `BatchAutorouterThread.java:324` is on the dead multithreaded path (`autoroutePassMultiThread:412` has zero callers).
// not ported: `RoutingFailureLog.shouldGiveUp` — there is no such top-level method; it is `RoutingFailureLog.ItemFailureInfo.shouldGiveUp` (`:147-149`), whose three readers (`:62`, `:73`, `:86`) are all themselves unported.
// not ported: `RoutingFailureLog.getUnroutableItems` (`:70-78`) — declaration only; `grep -rn getUnroutableItems src/main src/test` finds no call site.
// not ported: `RoutingFailureLog.hasUnroutableItems` (`:85-87`) — declaration only; same grep.
// not ported: `RoutingFailureLog.clear` (`:104-106`) — declaration only; `grep -rn "failureLog.clear" src/main src/test` is empty.
// not ported: `RoutingFailureLog.toString` — there is no such top-level method; it is `RoutingFailureLog.ItemFailureInfo.toString` (`:151-159`), and no caller outside the file can hold an `ItemFailureInfo` to print.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoutingFailureLog {
    /// `private final ConcurrentHashMap<Integer, ItemFailureInfo> failures` (`:19`), keyed by
    /// `item.getId()` (`:40`).
    failures: BTreeMap<ItemId, ItemFailureInfo>,
}

impl RoutingFailureLog {
    /// `public static final int FAILURE_THRESHOLD = 50` (`:16`) — "give up after this many
    /// failures for the same item".
    ///
    /// Read only by `ItemFailureInfo.shouldGiveUp` (`:148`), which is unported; the constant is
    /// transcribed because it is `public` and therefore an audit surface.
    pub const FAILURE_THRESHOLD: i32 = 50;

    /// `RoutingFailureLog()` (`:22-24`) — an empty map.
    pub fn new() -> RoutingFailureLog {
        RoutingFailureLog::default()
    }

    /// Port of `recordFailure(Item, int, AutorouteAttemptState, String)` (`:34-48`).
    ///
    /// Java's `failures.compute(item.getId(), …)` inserts a fresh `ItemFailureInfo(item)` when the
    /// key is absent and then calls `ItemFailureInfo.recordFailure(passNo, state, reason)` on it
    /// either way (`:39-47`); the port's entry API does the same in one lookup.
    ///
    /// `board` is what Java's `Item` reference is: the nested constructor reads
    /// `item.netCount() > 0 ? item.getNetNumber(0) : -1` (`:120`) to fill `netNumber`, and only on
    /// the **first** failure for that item — a later failure never revisits it, so an item whose
    /// nets changed in between keeps the number it was first logged under. The port reproduces
    /// that by deriving the number inside the vacant arm only.
    ///
    /// `item == null → return` (`:35-37`) has no counterpart: [`ItemId`] is not nullable. An item
    /// id that is not on the board is *not* the same thing and is still recorded, exactly as Java
    /// records a non-null `Item` that has been removed from `itemList`.
    ///
    /// `reason` is `Option<&str>` because Java's parameter is a nullable `String` and `:139`
    /// stores `reason != null ? reason : ""`.
    pub fn record_failure(
        &mut self,
        board: &Board,
        item: ItemId,
        pass_no: i32,
        state: AutorouteAttemptState,
        reason: Option<&str>,
    ) {
        // :39-47.
        let info = self.failures.entry(item).or_insert_with(|| {
            // :43 -> :118-125, the nested constructor.
            ItemFailureInfo::new(board, item)
        });
        // :45 -> :134-140.
        info.record_failure(pass_no, state, reason);
    }

    /// Port of `getFailureCount(Item)` (`:95-101`): "the number of failures, or 0 if no failures
    /// recorded".
    ///
    /// Its one live caller is `AutoroutePassRunner.java:272`, which uses it to decide whether a
    /// failure is worth a log line (`itemsToGoCount <= 5 || failureCount >= 3`, `:273`). No
    /// routing decision reads it.
    ///
    /// `item == null → 0` (`:96-98`) again has no counterpart; the absent-key arm at `:100` is
    /// the one the port keeps.
    ///
    // renamed: `RoutingFailureLog.getFailureCount` (`:95-101`) -> `RoutingFailureLog::failure_count`; the crate drops the `get` prefix on a bare reader with no setter beside it.
    pub fn failure_count(&self, item: ItemId) -> i32 {
        // :99-100.
        self.failures
            .get(&item)
            .map_or(0, |info| info.failure_count)
    }

    /// The recorded entry for an item, or `None`. **Not a Java method** — Java's callers reach the
    /// nested object through `failures.get(...)` directly (`:61`, `:99`), which is `private` there
    /// and so has no accessor to port. It exists so `crates/fr-router/tests/pass_runner.rs` can
    /// assert the fields `recordFailure` writes without a second copy of them on this type.
    pub fn entry(&self, item: ItemId) -> Option<&ItemFailureInfo> {
        self.failures.get(&item)
    }

    /// The number of items with at least one recorded failure. **Not a Java method**; it is
    /// `failures.size()`, which Java never calls. Test surface only.
    pub fn len(&self) -> usize {
        self.failures.len()
    }

    /// Whether nothing has been recorded. **Not a Java method** — see [`RoutingFailureLog::len`].
    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Port of the nested `RoutingFailureLog.ItemFailureInfo` (`:109-160`) — "information about
/// routing failures for a specific item", and the map's value type.
///
/// Six fields at `:110-115`; the port holds Java's `public final Item item` as its [`ItemId`],
/// which is what the map is keyed by anyway (`:40`).
///
/// Java's two methods are `synchronized` (`:134`, `:147`) because the map is a
/// `ConcurrentHashMap` shared by the dead multithreaded path. The port is single-threaded and the
/// keyword has no counterpart.
///
// renamed: `RoutingFailureLog.ItemFailureInfo` (`:109-160`) -> `fr_router::pipeline::ItemFailureInfo`; Java's nested class becomes a sibling type, because Rust has no nested classes and the map's value type has to be nameable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFailureInfo {
    /// `public final Item item` (`:110`), as its id.
    pub item: ItemId,
    /// `public final int netNumber` (`:111`), `item.getNetNumber(0)` or `-1` (`:120`).
    pub net_number: i32,
    /// `public int failureCount` (`:112`).
    pub failure_count: i32,
    /// `public AutorouteAttemptState lastFailureState` (`:113`), `null` until the first failure
    /// (`:122`) — which cannot happen through [`RoutingFailureLog::record_failure`], since that
    /// records one immediately after constructing this. `None` is Java's `null` all the same.
    pub last_failure_state: Option<AutorouteAttemptState>,
    /// `public String lastFailureReason` (`:114`), `""` until the first failure (`:123`).
    pub last_failure_reason: String,
    /// `public long lastAttemptPass` (`:115`) — a **`long`**, although `recordFailure`'s only
    /// caller passes the `int` `passNo` (`AutoroutePassRunner.java:270`).
    pub last_attempt_pass: i64,
}

impl ItemFailureInfo {
    /// Port of `ItemFailureInfo(Item)` (`:118-125`) — "creates failure tracking info for an
    /// item".
    ///
    /// `:120`'s `item.netCount() > 0 ? item.getNetNumber(0) : -1` is read off the board here;
    /// an id that is not on the board answers `-1`, which is the same value Java's `netCount() ==
    /// 0` arm produces.
    fn new(board: &Board, item: ItemId) -> ItemFailureInfo {
        // :120.
        let net_number = board
            .get_item(item)
            .filter(|i| i.net_count() > 0)
            .map_or(-1, |i| i.get_net_number(0));
        ItemFailureInfo {
            // :119.
            item,
            net_number,
            // :121.
            failure_count: 0,
            // :122.
            last_failure_state: None,
            // :123.
            last_failure_reason: String::new(),
            // :124.
            last_attempt_pass: 0,
        }
    }

    /// Port of `ItemFailureInfo.recordFailure(long, AutorouteAttemptState, String)`
    /// (`:134-140`) — increment, then overwrite the three "last" fields.
    fn record_failure(&mut self, pass_no: i32, state: AutorouteAttemptState, reason: Option<&str>) {
        // :136.
        self.failure_count += 1;
        // :137. Java widens the `int` argument to the `long` field.
        self.last_attempt_pass = i64::from(pass_no);
        // :138.
        self.last_failure_state = Some(state);
        // :139.
        self.last_failure_reason = reason.unwrap_or("").to_string();
    }
}
