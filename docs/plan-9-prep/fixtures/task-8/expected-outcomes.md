# Task 8 — rooms, doors and expandable identity: independent ground truth

Every derivation below is independent of the jar. Where a number is quoted from the port's
own pinning test the test's file:line is named, and the derivation that *re-derives* it from
the geometry follows, so the implementer never has to trust a literal.

Source of truth for the current behaviour: the repo at **tag `v1.0.0`** (`ecc0abf`), cloned
read-only to the scratchpad and built there. See `current-port-behavior.txt`.

---

## #159 — `ShapeSearchTree90Degree.completeShape` drops a room it decided to ignore

### The fixture
`fixtures/p9t8-ninety-degree.dsn` — a hand-written 200 mm × 200 mm two-layer board carrying
`(snap_angle ninety_degree)`. Four pads in the corners on two nets that must cross, and one
40 mm × 40 mm through-board blocker pad in the middle so the maze has to build rooms around an
obstacle. Verified against the v1.0.0 binary: both nets route, **every emitted segment is
axis-aligned** (`fixtures/p9t8-ninety-degree.expected.ses` — the path corners differ only in x
or only in y at every step), so the board really does drive the 90-degree regime.
Nothing in the committed corpus does; `Issue103`, `Issue187` and `Issue413` all declare
`fortyfive_degree`.

### The expectation, derived
The plan's binding test asserts `[4, 4, 4]` across `AngleRestriction::{None, FortyFiveDegree,
NinetyDegree}` on the synthetic `TestBoard` in `crates/fr-router/tests/tree_ext.rs`. Here is
where the **4** comes from, so the implementer can check it rather than copy it.

The test's seed room is a *whole-plane* `IncompleteFreeSpaceExpansionRoom` (shape `None`,
contained shape `box(100,100,200,200)`). The only object it can meet on layer 0 is the
complete expansion room `box(400,400,600,600)`, and the `ignore` shape passed to `complete`
is `box(300,300,700,700)` ⊇ that room. So the ignore decision fires on the room's only
overlap, `somethingChanged` stays false, and the base class's fallthrough
(`ShapeSearchTree.java:683-687`) re-adds the untouched whole-plane room. **One room.**

`ShapeSearchTree.divideLargeRoom` (`:1095-1118`, ported at
`crates/fr-router/src/autoroute/tree_ext.rs:199`) then runs, because `room_list.len() == 1`:

    board bounding box  B = [-10000, -10000] .. [10000, 10000]     (TestBoard's default)
    room bounding box   R = B                                       (whole-plane room)
    guard: R.height * 2 <= B.height ?  20000*2 = 40000 <= 20000 ?   NO
           R.width  * 2 <= B.width  ?  40000 <= 20000 ?             NO   -> proceed
    max_section_width = 0.5 * max(B.height, B.width) = 0.5 * 20000 = 10000
    IntBox::divide_into_sections(10000):
        length = 20000, height = 20000
        xcount = ceil(20000 / 10000) = 2
        ycount = ceil(20000 / 10000) = 2
        sections = xcount * ycount = 2 * 2 = 4

**4.** Confirmed empirically at v1.0.0: `IntBox[-10000,-10000..10000,10000]
.divide_into_sections(10000.0).len() == 4` (`current-port-behavior.txt`, `#18` block).

### ⚠ The plan's `[4, 4, 4]` does **not** follow from the fix as sketched — READ THIS
The plan's fix sketch is *"add the base class's fallthrough **verbatim** — the 45-degree
sibling **is** the specification"*. That restores the **room**, giving `[4, 4, 1]`, not
`[4, 4, 4]`, because `ShapeSearchTree90Degree.completeShape` **never calls
`divideLargeRoom` at all**. This is a third, separate divergence, and the port already
records it:

* `crates/fr-router/src/autoroute/tree_ext.rs:1261` — `// :190. This regime never calls divideLargeRoom.`
* `crates/fr-router/tests/tree_ext.rs:414-415` — *"The kept room is board-sized, so
  `divideLargeRoom` then cuts it into four sections — which is itself the third difference:
  the 90-degree override never divides at all."*

`complete_shape_base` calls it at `tree_ext.rs:491`; `complete_shape_45` at `:835`;
`complete_shape_90` does not. So the implementer must **decide and record** one of:

| decision | result | argument |
|---|---|---|
| **A** — restore the fallthrough only | `[4, 4, 1]` | literal reading of the fix sketch; keeps Java's 90-degree `:190` |
| **B** — restore the fallthrough *and* route the 90-degree path through `divide_large_room` | `[4, 4, 4]` | matches the plan's binding assertion; but it is a **second** behaviour change, not covered by "the 45-degree sibling is the specification" |

Recommendation: **B**, with the second change named explicitly in the commit message —
`divideLargeRoom` exists to stop one board-sized room from swamping the maze queue, and a
90-degree board has exactly the same need. But B is a bigger routing change than the plan's
one-liner implies, and its A/B must be read as such. If the controller prefers A, the binding
test's literal must be amended to `[4, 4, 1]` **before** Commit 2 lands.

---

## #160 + #161 — `SortedRoomNeighbour.compareTo` is not a total order

### The comparator, exactly (port: `crates/fr-router/src/autoroute/expansion/sorted_neighbours.rs:1387-1464`)

    1. d = self.touching_side_no_of_room - other.touching_side_no_of_room   (wrapping i32)
       if d != 0 -> sign(d)                                              [DECIDES]
    2. C = room_shape.corner_approx(touching_side_no_of_room)
       delta = |first_corner(self) - C| - |first_corner(other) - C|
    3. if |delta| <= C_DIST_TOLERANCE (= 1.0):
          if first_corner(self) == first_corner(other)   (EXACT point equality)
             delta = |last_corner(self) - C| - |last_corner(other) - C|
             if |delta| <= 1.0 and both .neighbour_room_touch_is_corner:
                delta = compare_from(...)  in {-1, 0, +1}
    4. res = Signum::as_int_f64(delta)    // exact sign: 0 only when delta == 0.0 exactly
    5. if res == 0: res = self.object_id - other.object_id   (wrapping i32)
    6. answer sign(res)

`object_id` (`:1510-1515`) is `ItemId.0 as i32` for an item and
`CompleteFreeSpaceExpansionRoom::get_id()` — a **per-engine counter** — for a room.

### Witness A — two neighbours of the SAME item compare Equal (the #160 drop)
Take one item that contributes two tree shapes touching the *same* side of the room, with
first corners **different but equidistant** from the compare corner, and **different last
corners**.

    room_shape         = Box[0,0 .. 1000,1000]
    touching_side_no_of_room = 0 for both   (the bottom side, from corner 0 = (0,0))
    C = corner(0) = (0, 0)

    N1: first_corner = (300, 400)   |N1.first - C| = sqrt(300^2 + 400^2) = sqrt(250000) = 500
        last_corner  = (300, 0)
    N2: first_corner = (400, 300)   |N2.first - C| = sqrt(400^2 + 300^2) = sqrt(250000) = 500
        last_corner  = (400, 0)

    step 1:  0 - 0 = 0                       -> fall through
    step 2:  delta = 500 - 500 = 0.0
    step 3:  |0.0| <= 1.0  -> enter; first_corner(N1) = (300,400) != (400,300) = first_corner(N2)
             -> the last-corner refinement is SKIPPED.  delta stays 0.0
    step 4:  Signum::as_int_f64(0.0) = 0
    step 5:  both neighbours come from the same item, so object_id(N1) == object_id(N2)
             res = 0
    step 6:  Ordering::Equal

`JavaTreeSet::add_by` then **discards N2 whole**. The room loses a door it really has, and
`(300,0)` and `(400,0)` are two *different* places on the same wall.

**Post-fix**: step 3's refinement must run **whenever the first-corner distances tie**, not
only when the first corners are byte-identical. Then
`|N1.last - C| = 300` vs `|N2.last - C| = 400`, `delta = -100`, **`Less`** — both kept.

### Witness B — an item id and a room id collide (#161)
`object_id` mixes two disjoint id spaces. Let the first item on the board have `ItemId(7)` and
let the engine's 8th `CompleteFreeSpaceExpansionRoom` have counter id `7` (they are allocated
independently and *will* collide on any non-trivial board — the item ids come from the DSN
read, the room ids from `AutorouteEngine`'s own counter, both starting near 1).

    N1: object = TreeObject::Item(ItemId(7))  -> object_id =  7
    N2: object = TreeObject::Room(room#8)     -> object_id =  7
    Suppose steps 1-4 all tie (same touching side, same first corner, same last corner —
    which is exactly the geometry of "a trace passes through a free-space room along the
    room's own wall": the item's tile shape and the room share the wall).
    step 5: 7 - 7 = 0   -> Ordering::Equal   -> one of them is DROPPED.

**Post-fix**: compare the **kind** first (`Item` < `Room`, or whichever order is chosen —
either is total), then the id inside the kind. N1 `Less` N2, both kept.

### The formal invariant the test must assert
> For every finite multiset `S` of `SortedRoomNeighbour`s, `|BTreeSet::from(S)| == |dedup(S)|`
> where two elements are duplicates only if they are *equal as values*; and `compare_to` is a
> total order: irreflexive-antisymmetric, transitive, and total.

The plan's 2 000-case generator is the right shape for this. The **481 drops in 2 000 cases**
is a jar/port measurement of the *current* comparator; the post-fix number is **0**, and 0 is
what the assertion should be — the 481 is only the fail-before headline.

### One more trap the fix must not step in
Step 3's `|delta| <= C_DIST_TOLERANCE` is a **tolerance** test, and a tolerance test is the
classic transitivity killer: `a≈b`, `b≈c`, `a≁c`. But note it does **not** short-circuit to
`Equal` here — the *only* thing the tolerance gates is whether the refinement runs, and step 4
takes the **exact** sign of `delta` afterwards. So a naive "make it total" that returns
`Equal` inside the tolerance band would *introduce* non-transitivity that is not there today.
The fix must keep step 4's exact sign and only widen which refinements can run.

---

## #163 — the eight-sided obstacle room gets seven doors — FULLY DERIVED

This one is derived end to end from geometry the port already prints, and the missing eighth
room and its door are computed below to the unit.

### The loop (port: `sorted_neighbours_45.rs:445-505`)

    current_corner = room_shape.corner(from_side_index)      // :464 — never reassigned
    current_side_index = from_side_index
    loop {
        next_side_no = (current_side_index + 1) % 8
        next_corner  = room_shape.corner(next_side_no)
        if current_corner != next_corner { build an incomplete room for current_side_index }
        if current_side_index == to_side_index { break }
        current_side_index = next_side_no
    }

`current_corner` is loop-invariant. On a full `0..7` walk the guard is therefore
`corner(0) != corner(i+1)`, which is true for `i = 0..6` and **false for `i = 7`**, because
`corner((7+1) % 8) = corner(0)`. **Exactly one side is lost, always the last of the walk.**

### The concrete room, from the port's own pinning test
`crates/fr-router/tests/sorted_neighbours_regimes.rs:679`
(`an_obstacle_room_with_no_neighbours_skips_the_last_side_of_its_octagon`), obstacle room
`obs5/0`:

    room_shape = Oct[-1340, -240, -260, 1440, -2464, -336, -1264, 864]
                     lx     by    rx    ty    ulx    lrx   llx    urx

    corners:  c0 (-1024, -240)   c4 ( -576, 1440)
              c1 ( -576, -240)   c5 (-1024, 1440)
              c2 ( -260,   76)   c6 (-1340, 1124)
              c3 ( -260, 1124)   c7 (-1340,   76)

    sides:  0 c0->c1 bottom      4 c4->c5 top
            1 c1->c2 lower-right 5 c5->c6 upper-left  diagonal
            2 c2->c3 right       6 c6->c7 left
            3 c3->c4 upper-right 7 c7->c0 lower-left  diagonal   <-- SKIPPED

All eight corners are distinct, so no side is degenerate. **Current: 7 incomplete rooms,
8 doors (7 of them dimension-1 edge doors plus 1 dimension-2 door to `obs5/1`).
Post-fix: 8 incomplete rooms, 9 doors.**

### The eighth incomplete room, computed by hand

The octagon convention (verified against the port, see below) is

    Oct[lx, by, rx, ty, ulx, lrx, llx, urx] = { (x,y) : lx <= x <= rx,  by <= y <= ty,
                                                ulx <= x-y <= lrx,  llx <= x+y <= urx }

For `current_side_index == 7` the switch's `_` arm (`:288-293`) sets
`upper_right_diagonal_x = room_shape.lower_left_diagonal_x = -1264`; every other parameter is
the board's raw bounding octagon `Oct[-10000, -10000, 10000, 10000, -20000, 20000, -20000, 20000]`.

Normalising `R = { -10000<=x<=10000, -10000<=y<=10000, -20000<=x-y<=20000, -20000<=x+y<=-1264 }`:

    rx  = max x : x <= -1264 - y, y >= -10000  ->  x <=  8736       ->  rx  =  8736
    ty  = max y : symmetric                    ->  ty  =  8736
    lx  = -10000     (feasible: x=-10000, y in [-10000, 8736])
    by  = -10000
    ulx = min(x-y) at (x,y) = (-10000, 8736)   =  -18736
    lrx = max(x-y) at (x,y) = ( 8736, -10000)  =   18736
    llx = min(x+y) at (x,y) = (-10000,-10000)  =  -20000
    urx = -1264

    EIGHTH INCOMPLETE ROOM  =  Oct[-10000, -10000, 8736, 8736, -18736, 18736, -20000, -1264]

**Cross-check of the normalisation model against a room the port already prints.** Side 5
sets `lower_right_diagonal_x = room_shape.upper_left_diagonal_x = -2464`, i.e. `x-y <= -2464`:

    rx  = max x : x <= -2464 + y, y <= 10000   ->   7536      (test prints  7536) OK
    by  = min y : y >=  2464 + x, x >= -10000  ->  -7536      (test prints -7536) OK
    llx = min(x+y) at ( -10000, -7536)         -> -17536      (test prints -17536) OK
    urx = max(x+y) at (   7536, 10000)         ->  17536      (test prints  17536) OK

The test's `incompleteRooms[5]` is `Oct[-10000,-7536,7536,10000,-20000,-2464,-17536,17536]`.
**Four of four match.** The model is sound, so the eighth room above is sound.

### The eighth door, computed by hand
The door is the intersection of `room_shape` with the new incomplete room. `room_shape` has
`llx = -1264`, i.e. `x + y >= -1264`; the new room has `x + y <= -1264`. The intersection is
therefore the line `x + y = -1264` clipped to the room — which is exactly side 7, the segment
`c7 (-1340, 76) -> c0 (-1024, -240)` (check: `-1340 + 76 = -1264`, `-1024 + (-240) = -1264`).

    lx  = -1340        by  = -240        rx  = -1024        ty  =  76
    ulx = min(x-y) = -1340 - 76   = -1416
    lrx = max(x-y) = -1024 + 240  =  -784
    llx = urx = -1264

    EIGHTH DOOR  =  Oct[-1340, -240, -1024, 76, -1416, -784, -1264, -1264],  dimension = 1

This is the exact shape of `doors[8]` (appended last, in loop order) and the `contained=` field
of `incompleteRooms[7]`. Compare `doors[7]` in the pinning test, the side-6 door
`Oct[-1340,76,-1340,1124,-2464,-1416,-1264,-216]`, which is built the same way and shares the
corner `(-1340, 76)`: the two doors meet at `c7`, exactly as two adjacent sides must.

### The invariant, for a test that is not a literal
> An obstacle expansion room whose octagon has **k distinct corners** and no touching neighbour
> gets **k** edge incomplete rooms, one per side, and each side's door is the side segment.
> No side of a non-degenerate octagon is unreachable.

`7 -> 8` is the instance; the invariant is `k -> k`.

---

## #164 — `otherRoom(room)` binds the narrowing overload

Two independent obligations; fixing one alone is worse than fixing neither.

1. **The overload.** `removeCompleteExpansionRoom` calls `door.otherRoom(room)` where `room`'s
   static type selects the `CompleteExpansionRoom` overload. Every door whose far side is an
   `IncompleteFreeSpaceExpansionRoom` returns `null` from the narrowing overload, the `null`
   check below `continue`s, and the door is skipped. On a freshly completed room **most** doors
   are onto incomplete rooms, so the method is a near-no-op.
2. **The length check.** The code past the `null` check indexes `touchingSides[1]`. Nothing
   guarantees `touchingSides.len() >= 2`; a dimension-1 door touching a single side gives
   length 1. Today the `null` skip is what keeps the index in range.

**Invariant to assert (no literal needed):**
> For a room `R` with a door `D` onto an incomplete room, `remove_complete_expansion_room(R)`
> visits `D` (it is not skipped) **and** does not panic, for every `touching_sides` length in
> `{0, 1, 2}`.

The plan's two named tests split exactly along these two obligations
(`a_door_onto_an_incomplete_room_is_not_skipped`, `touching_sides_is_length_checked`).
Build the `touching_sides.len() == 1` case deliberately — it is the one the current code
never reaches, so no fixture produces it by accident.

---

## #171 + #170 — `MazeListElement.compareTo`

Port: `crates/fr-router/src/autoroute/maze/list_element.rs:101-137`. Four keys, in order:
`sorting_value`, `expansion_value`, `door_id(door)`, `section_no_of_door`; a full tie answers
`Equal` and `JavaTreeSet::add_by` **keeps the incumbent and drops the newcomer entirely**.

### What the four keys do *not* cover
`MazeListElement` has 12 fields (`:33-73`). The four compared keys leave these uncompared:

    backtrack_door, section_no_of_backtrack_door, next_room, shape_entry,
    room_ripped, adjustment, already_checked, ripup_cost

`backtrack_door` is *the whole path*. So two genuinely different routes that arrive at the same
door section at the same cost are collapsed to one, and the survivor is whichever was inserted
first — not the cheaper, not the one with the lower ripup cost.

### Witness — two paths at the same cost, both real
    A = { door: D, section_no_of_door: 3, sorting_value: 100.0, expansion_value: 40.0,
          backtrack_door: Some(P), ripup_cost: 0,  room_ripped: false }
    B = { door: D, section_no_of_door: 3, sorting_value: 100.0, expansion_value: 40.0,
          backtrack_door: Some(Q), ripup_cost: 7,  room_ripped: true  }   with P != Q

    key 1: 100.0 vs 100.0 -> tie
    key 2:  40.0 vs  40.0 -> tie
    key 3: door_id(D) vs door_id(D) -> tie
    key 4:      3 vs 3    -> tie
    -> Equal. `add` returns false. B is discarded, including its ripup_cost of 7.

**Post-fix**: extend the comparison so that the two are ordered (or, per the plan's
alternative, *explicitly* keep the cheaper backtrack). The invariant:
> `queue.add(A)` then `queue.add(B)` with `A != B` as values leaves the queue with **2**
> elements, or with the element the tie-break policy names — and the policy is written down.

### The `door_id` collision (the other half of #171)
`ExpansionDoor::id(a, b) = min(a,b).wrapping_mul(31).wrapping_add(max(a,b))`
(`expansion/door.rs:156-162`). It is a **hash**, so two different doors collide.
Concrete collision:

    door X between rooms with ids (1, 63):  min=1, max=63  ->  1*31 + 63  =  94
    door Y between rooms with ids (2, 32):  min=2, max=32  ->  2*31 + 32  =  94

`31*a + b == 31*a' + b'` whenever `a' = a + k` and `b' = b - 31k`. So the family
`(a, b) -> (a+1, b-31)` collides for every `a` with `b >= 31 + a + 1`. Two *different* doors
therefore tie on key 3 and fall through to key 4 — and if their section numbers also match,
to `Equal`. This is why #156/#167/#158's injective ids are a prerequisite for #171 being
meaningfully fixed: a stable id space does not stop `31a+b` from colliding, but it does stop
the id from *moving*.

### #170 — the NaN
`:106-117` uses raw `<` / `>`. For `sorting_value = NaN`, both are false and the comparison
**falls through to `expansion_value`**. So with

    A = { sorting_value: NaN, expansion_value: 1.0, ... }
    B = { sorting_value: 5.0, expansion_value: 2.0, ... }
    C = { sorting_value: 3.0, expansion_value: 9.0, ... }

    A vs B: NaN<5 false, NaN>5 false -> key 2: 1.0 < 2.0 -> A < B
    B vs C: 5.0 > 3.0                                    -> B > C
    A vs C: NaN<3 false, NaN>3 false -> key 2: 1.0 < 9.0 -> A < C
    but transitivity via B demands A < B and C < B, and A < C says nothing consistent about
    the chain A < C < B < ... — the relation is not a strict weak ordering, and `TreeSet`'s
    red-black invariants are then meaningless: the same element can be found or not found
    depending on the tree's shape.

**Post-fix (the plan's ruling, and the right one): reject, do not order.** The invariant:
> `MazeQueue::add(e)` with `!e.sorting_value.is_finite()` returns an error / panics in debug /
> refuses the element at **all three** `add` sites; a non-finite cost is an upstream bug.
> `total_cmp` is the wrong fix: it would silently sort NaN last and keep the bug alive.

---

## #165 + #166 — rooms abandoned with live doors; `catch` returns an empty collection

Three separate obligations, all invariant-shaped, none needing a literal.

> **I1 (#165a)** After `complete_expansion_room(R)`, `R`'s id is taken **after** the commit, so
> `complete_expansion_rooms` contains exactly the rooms that are in the tree.
> Assert: `complete_expansion_rooms.iter().all(|r| tree.contains(r))` **and**
> `tree.rooms().all(|r| complete_expansion_rooms.contains(r))` — set equality, both directions.
>
> **I2 (#165b)** `add_complete_room`'s `null` path calls `remove_all_doors`.
> Assert: after an abandoned construction, no live room holds a door whose other side is the
> abandoned room. `rooms.all_doors().all(|d| tree.contains(d.first_room) && tree.contains(d.second_room))`.
>
> **I3 (#166)** `result` is hoisted out of the `try` and returned from the `catch`.
> Assert: force the fallible path (the plan's `a_committed_room_survives_the_catch`) and check
> the returned collection is **non-empty** and equals what was committed to the database.
> A room already committed and then reported as "no rooms" is silent data loss.

I2 is the one to build a fixture for; I1 and I3 are assertions over the arena that any board
reaching the path satisfies. Use the 90-degree fixture above or `p6t3` mode 5 seed rooms.

---

## #178 — `MazeSearchEngine.init` ignores `add`'s boolean

> **Invariant:** `MazeSearchEngine::get_instance(...)` answers `None` **iff** the expansion
> queue is empty after `init`. Equivalently: `start_ok` is exactly
> `mazeExpansionList.add(newListElement)`.

The reachable trigger is a fanout window too tight for any start element: `add` refuses the
element (its own comparator says an equal one is present, or — after #170 — the sorting value
is non-finite), `start_ok` stays `true` anyway, and the caller pays for a whole engine, a
`reduceTraceShapesAtTiePins` pass and a full round of room completion before discovering the
queue is empty. No numeric ground truth is needed: the test constructs the empty-queue
condition directly and asserts `None`.

Note the ordering: **this row is only observable after #171/#170**, because those decide when
`add` returns `false`. Land it after Commit 7, as the plan's step list already does.

---

## #156 + #167 + #158 — three ids that are hashes over mutable state

These are the cleanest hand-computations in the task: the collisions are arithmetic.

### #156 — `ObstacleExpansionRoom::get_id` (`expansion/obstacle_room.rs:97-100`)

    id(item, index) = (item.0 as i32).wrapping_shl(10) | (index as i32)

**Aliasing witness A — `index >= 1024` spills into the item's bits.** `|` never carries:

    id(ItemId(5),    0) = (5 << 10) | 0    = 5120 | 0    = 5120
    id(ItemId(5), 1024) = (5 << 10) | 1024 = 5120 | 1024 = 6144
    id(ItemId(6),    0) = (6 << 10) | 0    = 6144                     <-- SAME
    => room (item 5, shape 1024) and room (item 6, shape 0) share id 6144.

    Even simpler, a self-collision inside one item:
    id(ItemId(1), 1024) = 1024 | 1024 = 1024      (1 << 10 == 1024, OR is idempotent)
    id(ItemId(0), 1024) = 0    | 1024 = 1024      <-- SAME
    => item 1's shape 1024 and item 0's shape 1024 share id 1024.

**Aliasing witness B — `<< 10` overflows a Java `int` at `itemId >= 2^21`.**

    2^21 = 2097152
    id(ItemId(2097152), 0) = 2097152 << 10 = 2^31 -> wraps to i32::MIN = -2147483648
    id(ItemId(6291456), 0) = 6291456 << 10 = 3 * 2^31 -> wraps to -2147483648   <-- SAME
    (6291456 = 3 * 2^21; every itemId congruent to 2^21 mod 2^22 lands on i32::MIN)

    General law: id(item, index) == id(item + 2^22, index) for every item and index,
    because a 32-bit left shift by 10 is arithmetic mod 2^32 and 2^22 * 2^10 = 2^32.

**Post-fix invariant:** injective. `ids.len() == HashSet::from(ids).len()` over the property
test's domain, and the domain must include `index >= 1024` and `item >= 2^21` explicitly —
they are the two witnesses above and both are silently correct under a naive small-input test.

### #167 — `DrillPage::get_id` hashes a field `get_drills` overwrites
Port: `autoroute/drill/page.rs:445-455`, with `:447-453` naming it: half the hash is
`net_number`, and `DrillPage::get_drills` (Java `:65`) **writes** `net_number` while the page
may already be an element of the maze queue.

> **Invariant:** an object's id does not change while it is an element of an ordered
> collection. Assert: `let id0 = page.get_id(); page.get_drills(...); assert_eq!(page.get_id(), id0);`
> Today this fails whenever the second call's net number differs from the first's.

This is worse than a collision: it breaks the *container*, because a `TreeSet` cannot find an
element whose sort key moved under it. It is also the reason `MazeListElement::compare_to` takes
a `door_id` **resolver** rather than a snapshot (`list_element.rs:145-160`) — the port
faithfully re-reads the moving key at comparison time. Once ids are a stable counter, that
resolver can become a snapshot, and Task 24 can collect the `JavaTreeSet`.

### #158 — `IncompleteFreeSpaceExpansionRoom::get_id`
Port: `autoroute/expansion/incomplete_room.rs:56-91`. `31 * shape.get_id() + layer`.

    (a) The whole-plane room has shape `None`. `get_id` dereferences it -> NPE in Java,
        panic in the port. The port pins it at `incomplete_room.rs:175`.
        Post-fix invariant: `IncompleteFreeSpaceExpansionRoom::new(None, 0, Some(box)).get_id()`
        returns an id. (The plan's `the_whole_plane_room_has_an_id`.)
    (b) `set_shape` (`FreeSpaceExpansionRoom::set_shape`, `:70`) replaces the shape, so the id
        MOVES — and `complete_expansion_room` calls it. The port pins this at
        `incomplete_room.rs:167-169` (`assert_ne!(room.get_id(), before)`), which post-fix
        becomes `assert_eq!`.

### The single fix and what it costs
All three become **a per-engine counter**, as `CompleteFreeSpaceExpansionRoom` already has.
The ids feed `MazeListElement::compare_to`'s key 3 through `ExpansionDoor::get_id`, so
**routing moves on every board**. That is expected and is not a regression: the current ids
are *provably* wrong (the collisions above are arithmetic, not statistical), so the churn is
the fix landing, not noise.

**One caution.** `ExpansionDoor::id(a,b) = min*31 + max` stays a hash even over injective room
ids — see the `(1,63)`/`(2,32)` collision above. If the door id is to be a real identity, it
needs a counter too; otherwise the plan's #171 fix (extend the comparison) has to carry the
weight. Record which was chosen.

---

## #162 — `calculateNewIncompleteRooms` does not terminate

**Trigger:** a room whose shape has **more border lines than its `toSimplex()` does**. The loop
derives `touchingSideNo` from a shape whose side count is re-read each iteration against a
simplex with fewer sides, so the walk never closes and the room list grows until
`OutOfMemoryError`. Reachable at **0.4 % of room completions** on production boards.

**Fix:** compute `roomSimplex` **once, in the constructor**, and derive every `touchingSideNo`
from it. Explicitly **not** a loop bound: a bound terminates on the wrong side (it stops the
hang but leaves the room with a wrong door set, silently).

**Acceptance, and the thing to actually measure:** `p6t3` mode 5 currently *skips* these calls.
The acceptance is that **mode 5 becomes full-coverage** — i.e. the number of `calculate` calls
mode 5 executes rises to the number it enumerates — and the generators' `timeout(1)` is
removed, with the removal named in the commit message.

**Ground truth available without the jar:** the invariant is *termination*, which needs no
oracle. Add a hard iteration cap in the test (not in the product code) as a tripwire:
`assert!(iterations <= room_shape.border_line_count())` — the loop must visit each side at
most once, which is the actual specification.

---

## Cross-check summary — what the implementer can verify without any jar

| fix | ground truth | kind |
|---|---|---|
| #159 | 4 = 2×2 sections of a 20 000-unit square at max_section_width 10 000 | arithmetic |
| #160+#161 | two witnesses that compare `Equal`; total-order property | constructed |
| #163 | eighth room `Oct[-10000,-10000,8736,8736,-18736,18736,-20000,-1264]`, door `Oct[-1340,-240,-1024,76,-1416,-784,-1264,-1264]` | hand-computed, model cross-checked 4/4 |
| #164 | door-not-skipped + no panic at `touching_sides.len() == 1` | invariant |
| #171+#170 | two distinct elements compare `Equal`; NaN breaks transitivity; door-id collision `(1,63)`/`(2,32)` → 94 | arithmetic |
| #165+#166 | three set-equality / non-empty invariants | invariant |
| #178 | `get_instance == None` iff queue empty | invariant |
| #156+#167+#158 | `id(item,index) == id(item+2^22,index)`; `id(1,1024) == id(0,1024) == 1024` | arithmetic |
| #162 | termination; iterations ≤ border_line_count | invariant |
