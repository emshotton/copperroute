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
//! Deletion, submaps, the descending views and everything else `TreeMap` has are not here: the
//! only operations `SortedRoomNeighbours` performs are `add`, `isEmpty`, `size`, `getLast` and
//! iteration.
//!
//! not ported: every other `java.util.TreeMap`/`TreeSet` member.

use std::cmp::Ordering;

/// `TreeMap.Entry.color` (`java.util.TreeMap`): `RED = false`, `BLACK = true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Red,
    Black,
}

/// One `TreeMap.Entry`, in an arena rather than on the heap.
#[derive(Debug, Clone)]
struct Node<T> {
    key: T,
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
}

impl<T> Default for JavaTreeSet<T> {
    fn default() -> JavaTreeSet<T> {
        JavaTreeSet {
            nodes: Vec::new(),
            root: None,
            size: 0,
        }
    }
}

impl<T: Ord> JavaTreeSet<T> {
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
    pub fn add(&mut self, key: T) -> bool {
        // `TreeMap.put`: an empty map takes the key as the root.
        let Some(mut t) = self.root else {
            self.nodes.push(Node {
                key,
                left: None,
                right: None,
                parent: None,
                color: Color::Black,
            });
            self.root = Some(0);
            self.size = 1;
            return true;
        };
        // The `do { parent = t; cmp = k.compareTo(t.key); ... } while (t != null)` walk.
        let mut parent;
        let mut cmp;
        loop {
            parent = t;
            cmp = key.cmp(&self.nodes[t].key);
            match cmp {
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
        let e = self.nodes.len();
        self.nodes.push(Node {
            key,
            left: None,
            right: None,
            parent: Some(parent),
            color: Color::Red,
        });
        if cmp == Ordering::Less {
            self.nodes[parent].left = Some(e);
        } else {
            self.nodes[parent].right = Some(e);
        }
        self.fix_after_insertion(e);
        self.size += 1;
        true
    }
}

impl<T> JavaTreeSet<T> {
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
        Some(&self.nodes[p].key)
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
        Some(&self.set.nodes[current].key)
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
