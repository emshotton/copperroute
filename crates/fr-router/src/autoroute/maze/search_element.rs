//! Port of `autoroute.maze.MazeSearchElement` (MazeSearchElement.java:1-40) — "describes the
//! structure of a section of an `ExpandableObject`".

use crate::autoroute::expansion::ExpandableRef;

/// Port of `MazeSearchElement.Adjustment` (MazeSearchElement.java:34-39): the adjustment
/// direction the maze search recorded when it entered this section.
///
/// The `Default` is `NONE`, which is what Java's field initialiser at `:16` says and what
/// [`MazeSearchElement::reset`] restores (`:30`).
// renamed: MazeSearchElement.Adjustment -> MazeAdjustment (Java's nested enum; a bare
// `Adjustment` at the crate's re-export surface would say nothing about what it adjusts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum MazeAdjustment {
    /// `MazeSearchElement.Adjustment.NONE` (:36).
    #[default]
    None,
    /// `MazeSearchElement.Adjustment.RIGHT` (:37).
    Right,
    /// `MazeSearchElement.Adjustment.LEFT` (:38).
    Left,
}

impl MazeAdjustment {
    /// Java's `Adjustment.values()`, in declaration order — so the index of each is its
    /// `ordinal()`.
    pub const ALL: [MazeAdjustment; 3] = [
        MazeAdjustment::None,
        MazeAdjustment::Right,
        MazeAdjustment::Left,
    ];
}

/// Port of `maze.MazeSearchElement` (MazeSearchElement.java:6-39).
///
/// # Java-vs-brief
///
/// The task brief lists an `already_checked: bool` field and no `ripup_cost`. HEAD declares
/// neither of those the brief's way: the six fields are `isOccupied` (`:9`), `backtrackDoor`
/// (`:12`), `sectionNoOfBacktrackDoor` (`:14`), `roomRipped` (`:15`), `adjustment` (`:16`) and
/// `ripupCost` (`:22`) — there is no `alreadyChecked` anywhere in the class, and `ripupCost` is
/// read by the ripup resolver (Task 13). Java wins, so this is HEAD's field list verbatim.
///
/// Every field is `pub`, because Java's are: `MazeSearchEngine` writes them directly rather
/// than through accessors.
///
/// [`Default`] is exactly the state [`reset`](MazeSearchElement::reset) restores, which is also
/// the state Java's `new MazeSearchElement()` starts in (every field is a Java default except
/// `adjustment`, whose initialiser at `:16` is `NONE` — the enum's `Default` here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MazeSearchElement {
    /// `public boolean isOccupied` (:9): "true if this door is already occupied by the maze
    /// expanding algorithm".
    pub is_occupied: bool,
    /// `public ExpandableObject backtrackDoor` (:12), "used for backtracking in the maze
    /// expanding algorithm". `None` is Java's `null`.
    pub backtrack_door: Option<ExpandableRef>,
    /// `public int sectionNoOfBacktrackDoor` (:14).
    pub section_no_of_backtrack_door: i32,
    /// `public boolean roomRipped` (:15).
    pub room_ripped: bool,
    /// `public Adjustment adjustment = Adjustment.NONE` (:16).
    pub adjustment: MazeAdjustment,
    /// `public int ripupCost` (:22): "the ripup cost paid to enter this door's room via the
    /// maze search. Zero when roomRipped is false."
    pub ripup_cost: i32,
}

impl MazeSearchElement {
    /// `new MazeSearchElement()` — every field at its Java default (see [`Default`]).
    pub fn new() -> MazeSearchElement {
        MazeSearchElement::default()
    }

    /// Port of `MazeSearchElement.reset` (MazeSearchElement.java:25-32): "resets this
    /// MazeSearchElement for autorouting the next connection."
    ///
    /// Transcribed field by field rather than written as `*self = Self::default()`, so that a
    /// field added to the struct without a matching line at `:25-32` is a compile error here
    /// rather than a silent behaviour change.
    pub fn reset(&mut self) {
        self.is_occupied = false; // :26
        self.backtrack_door = None; // :27
        self.section_no_of_backtrack_door = 0; // :28
        self.room_ripped = false; // :29
        self.adjustment = MazeAdjustment::None; // :30
        self.ripup_cost = 0; // :31
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DoorId;
    use crate::autoroute::expansion::ExpandableRef;

    #[test]
    fn reset_clears_every_one_of_the_six_fields() {
        let mut e = MazeSearchElement::new();
        e.is_occupied = true;
        e.backtrack_door = Some(ExpandableRef::Door(DoorId(3)));
        e.section_no_of_backtrack_door = 9;
        e.room_ripped = true;
        e.adjustment = MazeAdjustment::Left;
        e.ripup_cost = 1234;
        e.reset();
        assert_eq!(e, MazeSearchElement::default());
    }

    #[test]
    fn the_adjustment_default_is_none_like_the_field_initialiser() {
        assert_eq!(MazeAdjustment::default(), MazeAdjustment::None);
        assert_eq!(MazeAdjustment::ALL[0], MazeAdjustment::None);
        assert_eq!(MazeAdjustment::ALL[1], MazeAdjustment::Right);
        assert_eq!(MazeAdjustment::ALL[2], MazeAdjustment::Left);
    }
}
