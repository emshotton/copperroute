//! A transcription of `java.util.TreeSet`'s insertion and iteration — the red-black tree of
//! `java.util.TreeMap` — for the one place in this crate where a `BTreeSet` gives a **different
//! answer**.
//!
//! # Why this exists (hazard F, quirk #160)
//!
//! `SortedRoomNeighbour.compareTo` (`autoroute/expansion/SortedRoomNeighbours.java:719-762`) is
//! not a total order: it can answer `a < b`, `b < c` and `c < a`. Both `TreeSet` and `BTreeSet`
//! stay memory-safe on such a comparator, but neither is *defined* on it, and what each does
//! depends on the comparison path its own shape produces — a root-to-leaf walk of a red-black
//! tree here, a binary search inside a B-tree node there. They diverge, and the divergence is
//! observable in both directions:
//!
//! * an element a `TreeSet` **drops** (its walk found an `Equal`) can be **kept** by a
//!   `BTreeSet`, whose search never compared it against the equal element; and
//! * the iteration order of the survivors differs, because an in-order walk of a red-black tree
//!   is not the sorted order when the comparator is inconsistent.
//!
//! `scripts/differential/run.sh p6t3 3` is the probe that showed it: with the port on a
//! `BTreeSet`, 2 000 corner-touch cases produced diffs of exactly those two kinds against the
//! HEAD jar; with this type they are byte-for-byte identical.
//!
//! # What is transcribed
//!
//! `TreeMap.put(key, value, true)` (OpenJDK `java.util.TreeMap`) and its `fixAfterInsertion`,
//! `rotateLeft` and `rotateRight`, plus the in-order traversal `TreeMap.getFirstEntry` /
//! `successor` that `TreeSet.iterator()` walks. `TreeSet.add` is `map.put(e, PRESENT) == null`,
//! so a comparison that answers `Equal` **keeps the element already in the tree** and answers
//! `false` — the silent drop.
//!
//! Submaps, the descending views and everything else `TreeMap` has are not here. The operations
//! that exist are the ones its two callers perform: `SortedRoomNeighbours` does `add`, `isEmpty`,
//! `size`, `getLast` and iteration, and Task 8's [`MazeQueue`] adds
//! `iterator().next()` + `it.remove()` — `TreeMap.deleteEntry` / `fixAfterDeletion`, transcribed
//! by [`JavaTreeSet::poll_first`].
//!
//! not ported: every other `java.util.TreeMap`/`TreeSet` member.
//!
//! # Why the comparator is a parameter (Task 8)
//!
//! [`JavaTreeSet::add_by`] takes the comparator rather than requiring `T: Ord`, because
//! `MazeListElement.compareTo` calls `door.getId()` — a virtual call this port answers by
//! looking the reference up in the engine's arenas, and one whose answer *moves* while the
//! element is in the tree (`DrillPage.getId`, quirk #167). `add` stays for a `T` that carries
//! its own order, which is what `SortedRoomNeighbour` does.
//!
//! [`MazeQueue`]: crate::autoroute::maze::MazeQueue

use std::cmp::Ordering;

/// `TreeMap.Entry.color` (`java.util.TreeMap`): `RED = false`, `BLACK = true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Red,
    Black,
}

/// One `TreeMap.Entry`, in an arena rather than on the heap.
///
/// `key` is an `Option` only so that `TreeMap.deleteEntry`'s `p.key = s.key` (the two-children
/// case) and `TreeSet.pollFirst`'s hand-back can *move* the key out; a node reachable from the
/// tree always holds `Some`. [`JavaTreeSet::key`] is the checked accessor.
#[derive(Debug, Clone)]
struct Node<T> {
    key: Option<T>,
    left: Option<usize>,
    right: Option<usize>,
    parent: Option<usize>,
    color: Color,
}

/// `java.util.TreeSet<T>` over a `T: Ord`, with the subset of operations
/// `autoroute.expansion.SortedRoomNeighbours` uses.
///
/// The point of the type is that its behaviour on a **non-total** `Ord` matches Java's; on a
/// well-behaved one it is an ordinary sorted set and a `BTreeSet` would do.
#[derive(Debug, Clone)]
pub struct JavaTreeSet<T> {
    nodes: Vec<Node<T>>,
    root: Option<usize>,
    size: usize,
    /// Slots freed by [`JavaTreeSet::delete_entry`], reused by the next insertion.
    ///
    /// Java's entries are garbage-collected; this is the equivalent. The index is internal, so
    /// reuse is invisible — nothing outside this module ever sees one.
    free: Vec<usize>,
}

impl<T> Default for JavaTreeSet<T> {
    fn default() -> JavaTreeSet<T> {
        JavaTreeSet {
            nodes: Vec::new(),
            root: None,
            size: 0,
            free: Vec::new(),
        }
    }
}

impl<T: Ord> JavaTreeSet<T> {
    /// `TreeSet.add(E)` for a `T` whose own `Ord` is the comparator, i.e. Java's
    /// `new TreeSet<E extends Comparable<E>>()`.
    pub fn add(&mut self, key: T) -> bool {
        self.add_by(key, T::cmp)
    }
}

impl<T> JavaTreeSet<T> {
    /// `new TreeSet<>()`.
    pub fn new() -> JavaTreeSet<T> {
        JavaTreeSet::default()
    }

    /// `TreeSet.size()`.
    pub fn len(&self) -> usize {
        self.size
    }

    /// `TreeSet.isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// `TreeSet.add(E)` = `map.put(e, PRESENT) == null` — `true` if the element was inserted,
    /// `false` if the walk found one that compares `Equal`, **which is kept in preference to the
    /// new one** (`TreeMap.put` assigns only the value, never the key).
    ///
    /// The comparator is a parameter rather than `T: Ord` because `MazeListElement.compareTo`
    /// (MazeListElement.java:95-96) calls `door.getId()`, a **virtual** call whose answer this
    /// port has to look up in the engine's arenas — and, for a `DrillPage`, an answer that
    /// *moves* while the element sits in the tree (quirk #167). Snapshotting the id into the
    /// element would freeze a key Java re-reads on every comparison; passing the resolver in at
    /// `add` time does not. `cmp` is `FnMut` so a caller may memoise inside it, exactly as a
    /// Java comparator may.
    pub fn add_by<F>(&mut self, key: T, mut cmp: F) -> bool
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        // `TreeMap.put`: an empty map takes the key as the root.
        let Some(mut t) = self.root else {
            let e = self.alloc(key, None, Color::Black);
            self.root = Some(e);
            self.size = 1;
            return true;
        };
        // The `do { parent = t; cmp = k.compareTo(t.key); ... } while (t != null)` walk.
        let mut parent;
        let mut ord;
        loop {
            parent = t;
            ord = cmp(&key, self.key(t));
            match ord {
                Ordering::Less => match self.nodes[t].left {
                    Some(next) => t = next,
                    None => break,
                },
                Ordering::Greater => match self.nodes[t].right {
                    Some(next) => t = next,
                    None => break,
                },
                // `t.value = value; return oldValue` — the key already in the tree stays.
                Ordering::Equal => return false,
            }
        }
        let e = self.alloc(key, Some(parent), Color::Red);
        if ord == Ordering::Less {
            self.nodes[parent].left = Some(e);
        } else {
            self.nodes[parent].right = Some(e);
        }
        self.fix_after_insertion(e);
        self.size += 1;
        true
    }

    /// Takes a free slot (or grows the arena) and fills it with a fresh, unlinked entry.
    fn alloc(&mut self, key: T, parent: Option<usize>, color: Color) -> usize {
        let node = Node {
            key: Some(key),
            left: None,
            right: None,
            parent,
            color,
        };
        match self.free.pop() {
            Some(slot) => {
                self.nodes[slot] = node;
                slot
            }
            None => {
                self.nodes.push(node);
                self.nodes.len() - 1
            }
        }
    }

    /// The key of a node that is reachable from the tree.
    fn key(&self, index: usize) -> &T {
        self.nodes[index]
            .key
            .as_ref()
            .expect("a node reachable from the tree always holds its key")
    }

    /// `TreeMap.pollFirstEntry()`, i.e. what `MazeSearchEngine.occupyNextElement`
    /// (MazeSearchEngine.java:327-329) does with `iterator().next()` + `it.remove()`.
    ///
    /// The key is read out **before** `deleteEntry` runs, exactly as Java's `exportEntry` /
    /// `Iterator.next` do — `deleteEntry` may overwrite `p.key` on its way (`TreeMap.deleteEntry`,
    /// the two-children case), though not for the first entry, which has no left child.
    pub fn poll_first(&mut self) -> Option<T> {
        let p = self.first_entry()?;
        let key = self.nodes[p]
            .key
            .take()
            .expect("the first entry holds its key");
        self.delete_entry(p);
        Some(key)
    }

    /// `TreeMap.deleteEntry(Entry)`, transcribed.
    ///
    /// The caller has already taken the key out of `p`, which is what Java's callers do too
    /// (they read it through `exportEntry` or `Iterator.next` first). The node that ends up
    /// unlinked therefore always holds `None`, and its slot goes back on the free list.
    fn delete_entry(&mut self, p: usize) {
        let mut p = p;
        self.size -= 1;

        // If strictly internal, copy successor's element to p and then make p point to successor.
        if self.nodes[p].left.is_some() && self.nodes[p].right.is_some() {
            let s = self
                .successor(p)
                .expect("a node with a right child has a successor");
            self.nodes[p].key = self.nodes[s].key.take();
            p = s;
        }

        // Start fixup at replacement node, if it exists.
        let replacement = self.nodes[p].left.or(self.nodes[p].right);
        let p_parent = self.nodes[p].parent;

        if let Some(replacement) = replacement {
            // Link replacement to parent.
            self.nodes[replacement].parent = p_parent;
            match p_parent {
                None => self.root = Some(replacement),
                Some(parent) => {
                    if self.nodes[parent].left == Some(p) {
                        self.nodes[parent].left = Some(replacement);
                    } else {
                        self.nodes[parent].right = Some(replacement);
                    }
                }
            }
            // Null out links so they are OK to use by fixAfterDeletion.
            self.nodes[p].left = None;
            self.nodes[p].right = None;
            self.nodes[p].parent = None;
            // Fix replacement.
            if self.nodes[p].color == Color::Black {
                self.fix_after_deletion(replacement);
            }
        } else if p_parent.is_none() {
            // Return if we are the only node.
            self.root = None;
        } else {
            // No children. Use self as phantom replacement and unlink.
            if self.nodes[p].color == Color::Black {
                self.fix_after_deletion(p);
            }
            if let Some(parent) = self.nodes[p].parent {
                if self.nodes[parent].left == Some(p) {
                    self.nodes[parent].left = None;
                } else if self.nodes[parent].right == Some(p) {
                    self.nodes[parent].right = None;
                }
                self.nodes[p].parent = None;
            }
        }
        debug_assert!(self.nodes[p].key.is_none(), "the freed node kept a key");
        self.nodes[p].left = None;
        self.nodes[p].right = None;
        self.nodes[p].parent = None;
        self.free.push(p);
    }

    /// The in-order traversal `TreeSet.iterator()` performs (`TreeMap.getFirstEntry` then
    /// `TreeMap.successor` repeatedly).
    ///
    /// On a consistent comparator this is ascending order; on an inconsistent one it is whatever
    /// the tree's shape makes it, which is the point.
    pub fn iter(&self) -> JavaTreeSetIter<'_, T> {
        JavaTreeSetIter {
            set: self,
            next: self.first_entry(),
        }
    }

    /// `SortedSet.getLast()` / `TreeSet.last()`: the rightmost entry. `None` where Java throws
    /// `NoSuchElementException`.
    pub fn last(&self) -> Option<&T> {
        let mut p = self.root?;
        while let Some(right) = self.nodes[p].right {
            p = right;
        }
        Some(self.key(p))
    }

    /// `TreeMap.getFirstEntry`.
    fn first_entry(&self) -> Option<usize> {
        let mut p = self.root?;
        while let Some(left) = self.nodes[p].left {
            p = left;
        }
        Some(p)
    }

    /// `TreeMap.successor(Entry)`.
    fn successor(&self, t: usize) -> Option<usize> {
        if let Some(right) = self.nodes[t].right {
            let mut p = right;
            while let Some(left) = self.nodes[p].left {
                p = left;
            }
            return Some(p);
        }
        let mut p = self.nodes[t].parent;
        let mut ch = t;
        while let Some(parent) = p
            && self.nodes[parent].right == Some(ch)
        {
            ch = parent;
            p = self.nodes[parent].parent;
        }
        p
    }

    // --- the red-black machinery (`TreeMap.fixAfterInsertion` and its two rotations) --------

    /// `TreeMap.colorOf(Entry)`: `null` is `BLACK`.
    fn color_of(&self, p: Option<usize>) -> Color {
        p.map_or(Color::Black, |p| self.nodes[p].color)
    }

    /// `TreeMap.parentOf(Entry)`.
    fn parent_of(&self, p: Option<usize>) -> Option<usize> {
        p.and_then(|p| self.nodes[p].parent)
    }

    /// `TreeMap.leftOf(Entry)`.
    fn left_of(&self, p: Option<usize>) -> Option<usize> {
        p.and_then(|p| self.nodes[p].left)
    }

    /// `TreeMap.rightOf(Entry)`.
    fn right_of(&self, p: Option<usize>) -> Option<usize> {
        p.and_then(|p| self.nodes[p].right)
    }

    /// `TreeMap.setColor(Entry, boolean)`: a `null` entry is silently ignored.
    fn set_color(&mut self, p: Option<usize>, color: Color) {
        if let Some(p) = p {
            self.nodes[p].color = color;
        }
    }

    /// `TreeMap.rotateLeft(Entry)`.
    fn rotate_left(&mut self, p: Option<usize>) {
        let Some(p) = p else { return };
        let Some(r) = self.nodes[p].right else { return };
        let r_left = self.nodes[r].left;
        self.nodes[p].right = r_left;
        if let Some(r_left) = r_left {
            self.nodes[r_left].parent = Some(p);
        }
        let p_parent = self.nodes[p].parent;
        self.nodes[r].parent = p_parent;
        match p_parent {
            None => self.root = Some(r),
            Some(parent) => {
                if self.nodes[parent].left == Some(p) {
                    self.nodes[parent].left = Some(r);
                } else {
                    self.nodes[parent].right = Some(r);
                }
            }
        }
        self.nodes[r].left = Some(p);
        self.nodes[p].parent = Some(r);
    }

    /// `TreeMap.rotateRight(Entry)`.
    fn rotate_right(&mut self, p: Option<usize>) {
        let Some(p) = p else { return };
        let Some(l) = self.nodes[p].left else { return };
        let l_right = self.nodes[l].right;
        self.nodes[p].left = l_right;
        if let Some(l_right) = l_right {
            self.nodes[l_right].parent = Some(p);
        }
        let p_parent = self.nodes[p].parent;
        self.nodes[l].parent = p_parent;
        match p_parent {
            None => self.root = Some(l),
            Some(parent) => {
                if self.nodes[parent].right == Some(p) {
                    self.nodes[parent].right = Some(l);
                } else {
                    self.nodes[parent].left = Some(l);
                }
            }
        }
        self.nodes[l].right = Some(p);
        self.nodes[p].parent = Some(l);
    }

    /// `TreeMap.fixAfterInsertion(Entry)`.
    fn fix_after_insertion(&mut self, x: usize) {
        let mut x = Some(x);
        self.set_color(x, Color::Red);
        while let Some(node) = x
            && Some(node) != self.root
            && self.color_of(self.parent_of(x)) == Color::Red
        {
            if self.parent_of(x) == self.left_of(self.parent_of(self.parent_of(x))) {
                let y = self.right_of(self.parent_of(self.parent_of(x)));
                if self.color_of(y) == Color::Red {
                    self.set_color(self.parent_of(x), Color::Black);
                    self.set_color(y, Color::Black);
                    self.set_color(self.parent_of(self.parent_of(x)), Color::Red);
                    x = self.parent_of(self.parent_of(x));
                } else {
                    if x == self.right_of(self.parent_of(x)) {
                        x = self.parent_of(x);
                        self.rotate_left(x);
                    }
                    self.set_color(self.parent_of(x), Color::Black);
                    self.set_color(self.parent_of(self.parent_of(x)), Color::Red);
                    let grandparent = self.parent_of(self.parent_of(x));
                    self.rotate_right(grandparent);
                }
            } else {
                let y = self.left_of(self.parent_of(self.parent_of(x)));
                if self.color_of(y) == Color::Red {
                    self.set_color(self.parent_of(x), Color::Black);
                    self.set_color(y, Color::Black);
                    self.set_color(self.parent_of(self.parent_of(x)), Color::Red);
                    x = self.parent_of(self.parent_of(x));
                } else {
                    if x == self.left_of(self.parent_of(x)) {
                        x = self.parent_of(x);
                        self.rotate_right(x);
                    }
                    self.set_color(self.parent_of(x), Color::Black);
                    self.set_color(self.parent_of(self.parent_of(x)), Color::Red);
                    let grandparent = self.parent_of(self.parent_of(x));
                    self.rotate_left(grandparent);
                }
            }
        }
        let root = self.root;
        self.set_color(root, Color::Black);
    }

    /// `TreeMap.fixAfterDeletion(Entry)`.
    fn fix_after_deletion(&mut self, x: usize) {
        let mut x = Some(x);
        while x != self.root && self.color_of(x) == Color::Black {
            if x == self.left_of(self.parent_of(x)) {
                let mut sib = self.right_of(self.parent_of(x));
                if self.color_of(sib) == Color::Red {
                    self.set_color(sib, Color::Black);
                    self.set_color(self.parent_of(x), Color::Red);
                    let parent = self.parent_of(x);
                    self.rotate_left(parent);
                    sib = self.right_of(self.parent_of(x));
                }
                if self.color_of(self.left_of(sib)) == Color::Black
                    && self.color_of(self.right_of(sib)) == Color::Black
                {
                    self.set_color(sib, Color::Red);
                    x = self.parent_of(x);
                } else {
                    if self.color_of(self.right_of(sib)) == Color::Black {
                        self.set_color(self.left_of(sib), Color::Black);
                        self.set_color(sib, Color::Red);
                        self.rotate_right(sib);
                        sib = self.right_of(self.parent_of(x));
                    }
                    let parent_color = self.color_of(self.parent_of(x));
                    self.set_color(sib, parent_color);
                    self.set_color(self.parent_of(x), Color::Black);
                    self.set_color(self.right_of(sib), Color::Black);
                    let parent = self.parent_of(x);
                    self.rotate_left(parent);
                    x = self.root;
                }
            } else {
                // Symmetric.
                let mut sib = self.left_of(self.parent_of(x));
                if self.color_of(sib) == Color::Red {
                    self.set_color(sib, Color::Black);
                    self.set_color(self.parent_of(x), Color::Red);
                    let parent = self.parent_of(x);
                    self.rotate_right(parent);
                    sib = self.left_of(self.parent_of(x));
                }
                if self.color_of(self.right_of(sib)) == Color::Black
                    && self.color_of(self.left_of(sib)) == Color::Black
                {
                    self.set_color(sib, Color::Red);
                    x = self.parent_of(x);
                } else {
                    if self.color_of(self.left_of(sib)) == Color::Black {
                        self.set_color(self.right_of(sib), Color::Black);
                        self.set_color(sib, Color::Red);
                        self.rotate_left(sib);
                        sib = self.left_of(self.parent_of(x));
                    }
                    let parent_color = self.color_of(self.parent_of(x));
                    self.set_color(sib, parent_color);
                    self.set_color(self.parent_of(x), Color::Black);
                    self.set_color(self.left_of(sib), Color::Black);
                    let parent = self.parent_of(x);
                    self.rotate_right(parent);
                    x = self.root;
                }
            }
        }
        self.set_color(x, Color::Black);
    }
}

/// The iterator [`JavaTreeSet::iter`] returns.
pub struct JavaTreeSetIter<'a, T> {
    set: &'a JavaTreeSet<T>,
    next: Option<usize>,
}

impl<'a, T> Iterator for JavaTreeSetIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let current = self.next?;
        self.next = self.set.successor(current);
        Some(self.set.key(current))
    }
}

impl<'a, T> IntoIterator for &'a JavaTreeSet<T> {
    type Item = &'a T;
    type IntoIter = JavaTreeSetIter<'a, T>;

    fn into_iter(self) -> JavaTreeSetIter<'a, T> {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn a_consistent_comparator_gives_an_ordinary_sorted_set() {
        let mut set = JavaTreeSet::new();
        for value in [5, 1, 9, 3, 7, 1, 9] {
            set.add(value);
        }
        assert_eq!(set.len(), 5);
        assert_eq!(set.iter().copied().collect::<Vec<_>>(), vec![1, 3, 5, 7, 9]);
        assert_eq!(set.last(), Some(&9));
    }

    #[test]
    fn a_duplicate_is_dropped_and_the_first_one_is_kept() {
        // `TreeMap.put` assigns the *value* on an equal key, never the key, so `TreeSet.add`
        // keeps the element already in the set. The wrapper below makes that observable.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Tagged(i32, char);
        impl Ord for Tagged {
            fn cmp(&self, other: &Tagged) -> Ordering {
                self.0.cmp(&other.0)
            }
        }
        impl PartialOrd for Tagged {
            fn partial_cmp(&self, other: &Tagged) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        let mut set = JavaTreeSet::new();
        assert!(set.add(Tagged(1, 'a')));
        assert!(!set.add(Tagged(1, 'b')));
        assert_eq!(
            set.iter().copied().collect::<Vec<_>>(),
            vec![Tagged(1, 'a')]
        );
    }

    /// `TreeMap.deleteEntry` + `fixAfterDeletion` (Task 8). On a **consistent** comparator a
    /// red-black tree and a `BTreeSet` must agree exactly, so this cross-checks the transcription
    /// against the standard library over an interleaved add/remove workload — the shape
    /// `MazeSearchEngine.occupyNextElement` produces, which pops one element and pushes several.
    #[test]
    fn poll_first_agrees_with_a_btreeset_over_interleaved_adds_and_removes() {
        use std::collections::BTreeSet;

        let mut set = JavaTreeSet::new();
        let mut reference = BTreeSet::new();
        // A deterministic spread that is not sorted and revisits values, so `add` also exercises
        // the duplicate path.
        let mut value: i64 = 1;
        for round in 0..500 {
            for _ in 0..3 {
                value = (value * 48_271) % 2_147_483_647;
                let key = (value % 977) as i32;
                assert_eq!(set.add(key), reference.insert(key), "add({key})");
            }
            if round % 2 == 0 {
                let mine = set.poll_first();
                let theirs = reference.iter().next().copied();
                if let Some(theirs) = theirs {
                    reference.remove(&theirs);
                }
                assert_eq!(mine, theirs, "poll_first at round {round}");
            }
            assert_eq!(set.len(), reference.len(), "size at round {round}");
            assert_eq!(
                set.iter().copied().collect::<Vec<_>>(),
                reference.iter().copied().collect::<Vec<_>>(),
                "iteration order at round {round}"
            );
            assert_eq!(set.last(), reference.iter().next_back(), "last at {round}");
        }

        // And draining answers ascending order, then `None`.
        let drained: Vec<i32> = std::iter::from_fn(|| set.poll_first()).collect();
        assert_eq!(drained, reference.into_iter().collect::<Vec<_>>());
        assert!(set.is_empty());
        assert_eq!(set.poll_first(), None);
    }

    /// The two-children arm of `TreeMap.deleteEntry` is unreachable from `poll_first` (the first
    /// entry has no left child by construction), but the freed slot has to go back on the free
    /// list either way — otherwise a long maze search grows the arena without bound.
    #[test]
    fn a_drained_set_reuses_its_slots() {
        let mut set = JavaTreeSet::new();
        for value in 0..64 {
            set.add(value);
        }
        let peak = set.nodes.len();
        assert_eq!(peak, 64);
        while set.poll_first().is_some() {}
        for value in 0..64 {
            set.add(value + 1000);
        }
        assert_eq!(
            set.nodes.len(),
            peak,
            "the arena did not grow a second time"
        );
        assert_eq!(set.len(), 64);
        assert_eq!(set.iter().next(), Some(&1000));
    }

    #[test]
    fn many_insertions_stay_balanced_and_ordered() {
        let mut set = JavaTreeSet::new();
        for value in 0..200 {
            assert!(set.add(value));
        }
        for value in 0..200 {
            assert!(!set.add(value));
        }
        assert_eq!(set.len(), 200);
        assert_eq!(
            set.iter().copied().collect::<Vec<_>>(),
            (0..200).collect::<Vec<i32>>()
        );
        // The tree really is a tree: no node is its own ancestor and the black height is uniform.
        let mut seen = 0;
        let mut node = set.first_entry();
        while let Some(current) = node {
            seen += 1;
            node = set.successor(current);
        }
        assert_eq!(seen, 200);
    }
}
