use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Node<T> {
    key: Option<T>,
    left: Option<usize>,
    right: Option<usize>,
    parent: Option<usize>,
    color: Color,
}

#[derive(Debug, Clone)]
pub struct JavaTreeSet<T> {
    nodes: Vec<Node<T>>,
    root: Option<usize>,
    size: usize,
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
    pub fn add(&mut self, key: T) -> bool {
        self.add_by(key, T::cmp)
    }
}

impl<T> JavaTreeSet<T> {
    pub fn new() -> JavaTreeSet<T> {
        JavaTreeSet::default()
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn add_by<F>(&mut self, key: T, mut cmp: F) -> bool
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        let Some(mut t) = self.root else {
            let e = self.alloc(key, None, Color::Black);
            self.root = Some(e);
            self.size = 1;
            return true;
        };
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

    fn key(&self, index: usize) -> &T {
        self.nodes[index]
            .key
            .as_ref()
            .expect("a node reachable from the tree always holds its key")
    }

    pub fn poll_first(&mut self) -> Option<T> {
        let p = self.first_entry()?;
        let key = self.nodes[p]
            .key
            .take()
            .expect("the first entry holds its key");
        self.delete_entry(p);
        Some(key)
    }

    fn delete_entry(&mut self, p: usize) {
        let mut p = p;
        self.size -= 1;

        if self.nodes[p].left.is_some() && self.nodes[p].right.is_some() {
            let s = self
                .successor(p)
                .expect("a node with a right child has a successor");
            self.nodes[p].key = self.nodes[s].key.take();
            p = s;
        }

        let replacement = self.nodes[p].left.or(self.nodes[p].right);
        let p_parent = self.nodes[p].parent;

        if let Some(replacement) = replacement {
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
            self.nodes[p].left = None;
            self.nodes[p].right = None;
            self.nodes[p].parent = None;
            if self.nodes[p].color == Color::Black {
                self.fix_after_deletion(replacement);
            }
        } else if p_parent.is_none() {
            self.root = None;
        } else {
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

    pub fn iter(&self) -> JavaTreeSetIter<'_, T> {
        JavaTreeSetIter {
            set: self,
            next: self.first_entry(),
        }
    }

    pub fn last(&self) -> Option<&T> {
        let mut p = self.root?;
        while let Some(right) = self.nodes[p].right {
            p = right;
        }
        Some(self.key(p))
    }

    fn first_entry(&self) -> Option<usize> {
        let mut p = self.root?;
        while let Some(left) = self.nodes[p].left {
            p = left;
        }
        Some(p)
    }

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

    fn color_of(&self, p: Option<usize>) -> Color {
        p.map_or(Color::Black, |p| self.nodes[p].color)
    }

    fn parent_of(&self, p: Option<usize>) -> Option<usize> {
        p.and_then(|p| self.nodes[p].parent)
    }

    fn left_of(&self, p: Option<usize>) -> Option<usize> {
        p.and_then(|p| self.nodes[p].left)
    }

    fn right_of(&self, p: Option<usize>) -> Option<usize> {
        p.and_then(|p| self.nodes[p].right)
    }

    fn set_color(&mut self, p: Option<usize>, color: Color) {
        if let Some(p) = p {
            self.nodes[p].color = color;
        }
    }

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
    fn poll_first_agrees_with_a_btreeset_over_interleaved_adds_and_removes() {
        use std::collections::BTreeSet;

        let mut set = JavaTreeSet::new();
        let mut reference = BTreeSet::new();
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

        let drained: Vec<i32> = std::iter::from_fn(|| set.poll_first()).collect();
        assert_eq!(drained, reference.into_iter().collect::<Vec<_>>());
        assert!(set.is_empty());
        assert_eq!(set.poll_first(), None);
    }

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
        let mut seen = 0;
        let mut node = set.first_entry();
        while let Some(current) = node {
            seen += 1;
            node = set.successor(current);
        }
        assert_eq!(seen, 200);
    }
}
