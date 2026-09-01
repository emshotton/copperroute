//! Via definitions and via rules.
//!
//! Java: `rules/ViaInfo.java`, `rules/ViaInfos.java`, `rules/ViaRule.java`.

use std::cmp::Ordering;
use std::fmt;
use std::num::NonZeroU64;

use crate::ids::{PadstackId, ViaInfoId, ViaRuleId};

use super::PadstackLookup;

/// Port of `ViaInfo` (`rules/ViaInfo.java`): a padstack + clearance class + attach-to-SMD
/// setting, usable for routing.
///
/// not ported: `ViaInfo.boardRules` (ViaInfo.java:15) — the back-pointer is read only by
/// `ViaInfo.printInfo` (ViaInfo.java:95-99), to name the clearance class in a GUI info panel.
///
/// not ported: `ViaInfo.printInfo` (ViaInfo.java:86-107) — `ItemInfoPrinter.Printable`, GUI
/// only.
///
/// # Object identity
///
/// `ViaInfo.java` declares neither `equals` nor `hashCode`, so **every `==` on a `ViaInfo` in
/// Java is object identity**, and `ViaRule.contains` (ViaRule.java:55-62) is written as exactly
/// that (`:57`). Plan 7 Task 0 made a [`ViaRule`] hold owned copies rather than indices, which is
/// the port's spelling of a Java reference — but a copy alone cannot say *which* object it is a
/// copy of. The private `serial` field does: see [`Self::is_same_object`].
#[derive(Clone, Eq)]
pub struct ViaInfo {
    /// `ViaInfo.name` (ViaInfo.java:16).
    name: String,
    /// `ViaInfo.padstack` (ViaInfo.java:17), as an index into the board library's padstacks.
    padstack: PadstackId,
    /// `ViaInfo.clearanceClassIndex` (ViaInfo.java:18).
    clearance_class_index: usize,
    /// `ViaInfo.attachSmdAllowed` (ViaInfo.java:19).
    attach_smd_allowed: bool,
    /// This port's spelling of **Java object identity**, and nothing else — see
    /// [`Self::is_same_object`] and quirk #218.
    ///
    /// # Why it is not a process-wide counter
    ///
    /// `fr_geometry::Line` carries the same idea through a `static AtomicU64` (controller ruling
    /// AE), and the plan's Global Constraints record that static as the *one* exception to "no
    /// static mutable state". This field needs no second exception: **every `ViaInfo` that can
    /// reach a [`ViaRule`] in production is first added to a [`ViaInfos`]**
    /// (`Network.addViaRule` (Network.java:405-408), `BoardRules::create_default_via_rule` and
    /// `Network.createViaRule` all fetch theirs out of `board.rules.viaInfos` by name), so
    /// [`ViaInfos::add`] is the single place a `ViaInfo` becomes an object with a life of its
    /// own, and the counter that stamps it lives on that container.
    ///
    /// A `ViaInfo` that has never been added to a `ViaInfos` carries `None` and is therefore
    /// **not the same object as anything, itself included**. Java has no such value — the field
    /// is the port's, not Java's — and the asymmetry is deliberate: answering "same object" for
    /// two freshly built, never-registered values would be the *false positive* this field
    /// exists to prevent. The serial takes no part in `PartialEq`/`Eq`/`Hash`, so no ordering,
    /// no golden file and no writer can observe it.
    serial: Option<NonZeroU64>,
}

impl ViaInfo {
    /// Port of the `ViaInfo(String, Padstack, int, boolean, BoardRules)` constructor
    /// (ViaInfo.java:22-33), minus the `BoardRules` back-pointer.
    pub fn new(
        name: impl Into<String>,
        padstack: PadstackId,
        clearance_class_index: usize,
        drill_to_smd_allowed: bool,
    ) -> ViaInfo {
        ViaInfo {
            name: name.into(),
            padstack,
            clearance_class_index,
            attach_smd_allowed: drill_to_smd_allowed,
            // Stamped by [`ViaInfos::add`], the one place a `ViaInfo` becomes a registered
            // object — see the `serial` field.
            serial: None,
        }
    }

    /// Java's `viaInfo == currentInfo` — **reference** identity, not the structural `==` this
    /// type derives for everything else. Read by [`ViaRule::contains`] (ViaRule.java:57), which
    /// is the dedup guard `RoutingBoard.fanout:1037` merges two rules through.
    ///
    /// True exactly for two values copied from the same [`ViaInfos::add`]; `false` for two
    /// value-equal via infos registered separately, which is the divergence quirk #218 records,
    /// and `false` for any via info that was never registered (see the `serial` field).
    pub fn is_same_object(&self, other: &ViaInfo) -> bool {
        self.serial.is_some() && self.serial == other.serial
    }

    /// Port of `ViaInfo.getName` (ViaInfo.java:36-38).
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Port of `ViaInfo.setName` (ViaInfo.java:41-43).
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Port of `ViaInfo.getPadstack` (ViaInfo.java:51-53).
    pub fn get_padstack(&self) -> PadstackId {
        self.padstack
    }

    /// Port of `ViaInfo.setPadstack` (ViaInfo.java:56-58).
    pub fn set_padstack(&mut self, padstack: PadstackId) {
        self.padstack = padstack;
    }

    /// Port of `ViaInfo.getClearanceClassIndex` (ViaInfo.java:61-63).
    pub fn get_clearance_class_index(&self) -> usize {
        self.clearance_class_index
    }

    /// Port of `ViaInfo.setClearanceClassIndex` (ViaInfo.java:66-68).
    pub fn set_clearance_class_index(&mut self, clearance_class_index: usize) {
        self.clearance_class_index = clearance_class_index;
    }

    /// Port of `ViaInfo.attachSmdAllowed` (ViaInfo.java:71-73).
    pub fn attach_smd_allowed(&self) -> bool {
        self.attach_smd_allowed
    }

    /// Port of `ViaInfo.setAttachSmdAllowed` (ViaInfo.java:76-78).
    pub fn set_attach_smd_allowed(&mut self, attach_smd_allowed: bool) {
        self.attach_smd_allowed = attach_smd_allowed;
    }

    /// Port of `ViaInfo.compareTo` (ViaInfo.java:81-83): `name.compareTo(other.name)` — case
    /// *sensitive*, unlike `Net.compareTo`.
    ///
    /// Not an `Ord` impl: two via infos with the same name but different padstacks compare Equal
    /// here while `PartialEq` is structural, which would break the `Ord`/`Eq` contract. Java's
    /// `String.compareTo` compares UTF-16 code units, so this uses `str`'s byte-wise `cmp` —
    /// the two orders agree on all of ASCII and differ only where a code point above U+FFFF is
    /// compared against one in U+E000..U+FFFF, which cannot occur in a DSN identifier.
    pub fn compare_to(&self, other: &ViaInfo) -> Ordering {
        self.name.cmp(&other.name)
    }
}

/// The identity serial is invisible in `Debug`, so no committed transcript, golden file or
/// panic message can see a token — the `fr_geometry::Line` contract (controller ruling AE),
/// applied to the same problem one crate over.
impl fmt::Debug for ViaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViaInfo")
            .field("name", &self.name)
            .field("padstack", &self.padstack)
            .field("clearance_class_index", &self.clearance_class_index)
            .field("attach_smd_allowed", &self.attach_smd_allowed)
            .finish()
    }
}

/// Structural, as Java's `ViaInfo` has no `equals` at all: the port's `==` is the *value*
/// comparison `BoardRules::create_default_via_rule` and the round-trip tests want, and
/// [`ViaInfo::is_same_object`] is the reference comparison Java's own `==` performs. The
/// identity serial is invisible here, so the `Eq`/`Hash` pair stays lawful and no committed
/// transcript can see a token.
impl PartialEq for ViaInfo {
    fn eq(&self, other: &ViaInfo) -> bool {
        self.name == other.name
            && self.padstack == other.padstack
            && self.clearance_class_index == other.clearance_class_index
            && self.attach_smd_allowed == other.attach_smd_allowed
    }
}

impl fmt::Display for ViaInfo {
    // renamed: ViaInfo.toString -> Display::fmt (ViaInfo.java:46-48 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Port of `ViaInfos` (`rules/ViaInfos.java`): the list of via definitions usable for routing.
///
/// not ported: `ViaInfos.printInfo` (ViaInfos.java:67-87) — `ItemInfoPrinter.Printable`, GUI
/// only.
#[derive(Debug, Clone, Eq)]
pub struct ViaInfos {
    /// `ViaInfos.list` (ViaInfos.java:16).
    list: Vec<ViaInfo>,
    /// The identity counter behind [`ViaInfo::is_same_object`] — see that method and the
    /// `ViaInfo::serial` field. Not a Java field: Java gets object identity from the allocator.
    ///
    /// It is deliberately **per container** rather than process-wide, so the port keeps its "no
    /// static mutable state" constraint (the `fr_geometry::Line` counter is its one recorded
    /// exception). Two different boards therefore mint the same serials; nothing compares via
    /// infos across boards, and a [`Board`](crate::Board) clone keeps its own consistent.
    next_serial: NonZeroU64,
}

impl Default for ViaInfos {
    fn default() -> ViaInfos {
        ViaInfos {
            list: Vec::new(),
            next_serial: NonZeroU64::MIN,
        }
    }
}

/// The counter is invisible here, exactly as the per-entry serial is: two via-info lists with
/// the same entries are equal however many replacements each has been through.
impl PartialEq for ViaInfos {
    fn eq(&self, other: &ViaInfos) -> bool {
        self.list == other.list
    }
}

impl ViaInfos {
    /// An empty via-info list (Java uses the field initialiser at ViaInfos.java:16).
    pub fn new() -> ViaInfos {
        ViaInfos::default()
    }

    /// Port of `ViaInfos.add` (ViaInfos.java:22-28): appends `via_info` unless its name is
    /// already taken, in which case it returns false and changes nothing.
    ///
    /// This is also the one place a `ViaInfo` acquires its identity serial — see
    /// [`ViaInfo::is_same_object`]. Java gets that from `new ViaInfo(...)`; the port cannot stamp
    /// in the constructor without a process-wide counter, and every via info a [`ViaRule`] can
    /// hold in production passes through here first.
    ///
    /// Adding a via info that already carries a serial — one cloned back out of a list — gives it
    /// a **fresh** one, i.e. treats it as a new object. No production path does that: the two
    /// callers are the DSN/`.rules` readers with a freshly parsed `ViaInfo`, and
    /// [`BoardRules::replace_via_info`](super::BoardRules::replace_via_info), whose replacement is
    /// likewise fresh. A [`Board`](crate::Board) clone copies the list rather than re-adding, so
    /// a cloned board's serials are its original's.
    pub fn add(&mut self, mut via_info: ViaInfo) -> bool {
        if self.name_exists(via_info.get_name()) {
            return false;
        }
        via_info.serial = Some(self.next_serial);
        self.next_serial = self
            .next_serial
            .checked_add(1)
            .expect("via-info serials cannot wrap");
        self.list.push(via_info);
        true
    }

    /// Port of `ViaInfos.count` (ViaInfos.java:31-33).
    pub fn count(&self) -> usize {
        self.list.len()
    }

    /// Port of `ViaInfos.get(int)` (ViaInfos.java:36-39). Java asserts the index is in range and
    /// would then throw; the port panics on the same out-of-range index.
    pub fn get(&self, index: ViaInfoId) -> &ViaInfo {
        &self.list[index.0]
    }

    /// Mutable counterpart of [`Self::get`]; Java's `get` already hands back a mutable object.
    pub fn get_mut(&mut self, index: ViaInfoId) -> &mut ViaInfo {
        &mut self.list[index.0]
    }

    /// Port of `ViaInfos.get(String)` (ViaInfos.java:42-49): the via with exactly this name
    /// (`equals`, case sensitive), or `None`.
    pub fn get_by_name(&self, name: &str) -> Option<&ViaInfo> {
        self.get_no(name).map(|id| self.get(id))
    }

    /// The index form of [`Self::get_by_name`]. Not a Java method: Java's callers keep the
    /// object reference, while this port needs the [`ViaInfoId`].
    pub fn get_no(&self, name: &str) -> Option<ViaInfoId> {
        self.list.iter().position(|v| v.name == name).map(ViaInfoId)
    }

    /// Every via info, in index order. Not a Java method — Java iterates `list` directly
    /// (ViaInfos.java:43,53,74).
    pub fn iter(&self) -> std::slice::Iter<'_, ViaInfo> {
        self.list.iter()
    }

    /// Port of `ViaInfos.nameExists` (ViaInfos.java:52-59).
    pub fn name_exists(&self, name: &str) -> bool {
        self.get_no(name).is_some()
    }

    /// Port of `ViaInfos.remove` (ViaInfos.java:62-64): removes the via with the given index,
    /// returning false (and changing nothing) if the index is out of range — Java's `false` for
    /// "was not contained in the list".
    ///
    /// **No longer a renumbering hazard, since Plan 7 Task 0.** Java's `ViaRule`s hold `ViaInfo`
    /// *object references* (ViaRule.java:21), so a removal here leaves them holding the removed
    /// object; [`ViaRule`] now holds **owned copies** of its `ViaInfo`s, so a removal here cannot
    /// reach into a rule either. Nothing outside this struct stores a [`ViaInfoId`] across a
    /// removal, so the index shift is no longer observable.
    ///
    /// The only non-GUI caller is `io/specctra/RulesReader.java:340-350`, which removes a via
    /// only to add a replacement with the same name, through
    /// [`BoardRules::replace_via_info`](super::BoardRules::replace_via_info) immediately below.
    /// (`gui/windows/routing/WindowEditVias.java:197` also calls it, but the GUI is out of scope
    /// for this port.)
    ///
    /// See the `docs/java-quirks.md` obligation-register rows "`ViaInfoId` renumbering across
    /// `ViaInfos.remove`" (discharged in Plan 3 Task 14) and "Via-info / via-rule re-pointing"
    /// (ruling H: the via-info half closed by Plan 7 Task 0).
    pub fn remove(&mut self, index: ViaInfoId) -> bool {
        if index.0 >= self.list.len() {
            return false;
        }
        self.list.remove(index.0);
        true
    }
}

impl super::BoardRules {
    /// Port of `RulesReader.applyViaInfo`'s replacement half (io/specctra/RulesReader.java:346-349):
    /// `viaInfos.remove(existing); viaInfos.add(viaInfo);` — a remove-then-append that moves the
    /// entry to the **tail** of the list. The list order is observable, because
    /// `Network.writeViaInfos` (and therefore both the DSN and the rules writer) emits the via
    /// infos in list order.
    ///
    /// **No rule is rewritten. Java rewrites none either, and that is the whole point.** Java's
    /// `ViaRule`s hold `ViaInfo` object references (ViaRule.java:21), so a rule that already held
    /// the removed object keeps holding it — a **detached original**, no longer in
    /// [`Self::via_infos`] at all. Since Plan 7 Task 0 a [`ViaRule`] holds owned copies, which is
    /// this port's spelling of that aliasing: the copy the rule took when it was built is
    /// untouched by anything that happens to the list afterwards.
    ///
    /// Returns the new tail id.
    ///
    /// Replaces `replace_via_info_renumbering_rules` (Plan 3 Task 14), whose renumbering half was
    /// only ever needed by the index model — see the `docs/java-quirks.md` obligation register.
    ///
    /// # Panics
    ///
    /// Panics if `old_id` is out of range for [`Self::via_infos`], or if `new_info`'s name is
    /// still taken after the removal (i.e. `new_info` does not carry the name `old_id` held). The
    /// only caller — the rules reader's `apply_via_info` — looks `old_id` up *by* `new_info`'s
    /// name, so neither can happen there.
    //
    // obligation: ViaInfos.remove — the *renumbering* half of the Plan 2 hand-off obligation was
    // discharged here in Plan 3 Task 14 and is now moot: nothing holds a `ViaInfoId` across the
    // removal (docs/java-quirks.md, docs/plan-2-handoff.md).
    // added in Plan 3: Task 14; rewritten in Plan 7 Task 0 (controller ruling AL closes ruling H).
    pub fn replace_via_info(&mut self, old_id: ViaInfoId, new_info: ViaInfo) -> ViaInfoId {
        assert!(
            self.via_infos.remove(old_id),
            "replace_via_info: old_id {} out of range",
            old_id.0
        );
        assert!(
            self.via_infos.add(new_info),
            "replace_via_info: the replacement's name is still taken"
        );
        ViaInfoId(self.via_infos.count() - 1)
    }

    /// Port of `Network.addViaRule`'s replacement half (Network.java:413-417):
    /// `board.rules.viaRules.remove(existingRule); board.rules.viaRules.add(currentRule);` — a
    /// remove-then-append that moves the rule to the **tail** of [`Self::via_rules`]. Reached
    /// from `io/specctra/RulesReader.java:352-357` for every `(via_rule …)` in a `.rules` file.
    ///
    /// **No net class is rewritten. Java rewrites none either, and that is the whole point.**
    /// `NetClass.viaRule` (NetClass.java:28) is an object reference, so a class that pointed at
    /// the removed rule keeps holding it — a **detached original**, no longer in
    /// [`Self::via_rules`] at all, and still the rule `AutorouteControl.initNet:210` reads.
    /// Since Plan 7 Task 11 a [`crate::NetClass`] holds an **owned** [`ViaRule`], which is this
    /// port's spelling of that aliasing, so this method is Java's two lines and nothing else.
    ///
    /// # Ruling H, closed — the via-rule half (Plan 7 Task 11, controller ruling AN)
    ///
    /// Until Task 11 this method was `replace_via_rule_renumbering_net_classes`, and it
    /// **re-pointed** every net class at the replacement. The task built the repro the Task 0
    /// review could not: `Issue143-rpi_splitter.dsn` is the one corpus stem whose net class holds
    /// the *first* rule of its name (every other stem carries two rules named `default` and binds
    /// the class to the second, so `BoardRules::get_via_rule` can only ever find the orphan), and
    /// a one-line `.rules` file re-declaring `(via_rule default)` **empty** made
    /// `scripts/differential/run.sh p6t1 …/Issue143-rpi_splitter.dsn 8 1 <rules>` DIFF on all
    /// eight connections: the jar still placed vias (its class kept the original one-via rule)
    /// while the port placed none. With the ownership change the same command MATCHes —
    /// `crates/fr-router/tests/data/p7t11-ruling-h-viarule.txt` is the transcript.
    ///
    /// Returns the new tail id.
    ///
    /// # Panics
    ///
    /// Panics if `old_id` is out of range for [`Self::via_rules`].
    //
    // renamed: Network.addViaRule's replacement half -> BoardRules::replace_via_rule (was
    // `replace_via_rule_renumbering_net_classes` before Plan 7 Task 11).
    // added in Plan 3: Task 14; rewritten in Plan 7 Task 11 (controller ruling AN closes the
    // via-rule half of ruling H).
    pub fn replace_via_rule(&mut self, old_id: ViaRuleId, new_rule: ViaRule) -> ViaRuleId {
        assert!(
            old_id.0 < self.via_rules.len(),
            "replace_via_rule: old_id {} out of range",
            old_id.0
        );
        self.via_rules.remove(old_id.0);
        self.via_rules.push(new_rule);
        ViaRuleId(self.via_rules.len() - 1)
    }
}

/// Port of `ViaRule` (ViaRule.java:15-129): an ordered list of vias usable for routing; vias
/// near the front are preferred. **Owns** its `ViaInfo`s.
///
/// Java's field is `private final List<ViaInfo> list` (:21) — object *references*.
/// `RulesReader.applyViaInfo` (io/specctra/RulesReader.java:340-350) replaces a `ViaInfo` in
/// `BoardRules.viaInfos` by name, and every `ViaRule` that already held the old object keeps the
/// **detached original**. An index model cannot express that, which is what made ruling H's
/// divergence unavoidable — measured at HEAD by `P6T8Probe viadiv` (`attach=true` on the
/// replacement against `attach=false` in the rule) and closed **against** re-pointing by Plan 6
/// Task 17 (the jar laying four traces to the port's two at connection k = 8 of
/// `Issue593-BBD_Mars-64.dsn`, cumulative trace length `1401450.8259119983` against
/// `1395031.4105961146`). Owned copies are this port's spelling of Java's aliasing: a copy taken
/// when the rule was built is untouched by anything that happens to [`ViaInfos`] afterwards, just
/// as Java's detached object is.
///
/// # The one place owned copies are *not* Java's aliasing
///
/// Java's rule and `BoardRules.viaInfos` normally share the same object, so a mutation through
/// either is seen by both. The only Java code that mutates a live `ViaInfo` in place is
/// `BoardRules.changeClearanceClassIndex` (BoardRules.java:263-289) and
/// `BoardRules.removeClearanceClass` (:295-349), which walk `viaInfos` and call
/// `setClearanceClassIndex`; a rule holding those objects sees the new index, and a rule holding a
/// *detached* one does not. This port's copies never see it. **Both Java methods have exactly one
/// caller, `gui/windows/routing/WindowClearanceMatrix.java:273-274`** — the GUI, which this port
/// does not have; nothing in `board`/`autoroute`/`drc`/`io` reaches them, and neither does any
/// port caller outside its own unit tests. Pinned by
/// `board_rules.rs::clearance_class_renumbering_does_not_reach_a_rules_copy`.
///
/// not ported: `ViaRule.swap` (ViaRule.java:92-104) — its only caller is
/// `gui/windows/routing/WindowViaRule.java:134`; no `board`/`autoroute`/`drc`/`io` code calls
/// it.
///
/// not ported: `ViaRule.printInfo` (ViaRule.java:107-129) — `ItemInfoPrinter.Printable`, GUI
/// only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViaRule {
    /// `ViaRule.name` (ViaRule.java:20); public and final in Java too.
    pub name: String,
    /// `ViaRule.list` (ViaRule.java:21) — owned copies, not [`ViaInfoId`] indices. See the struct
    /// doc for why.
    vias: Vec<ViaInfo>,
}

impl ViaRule {
    /// Port of the `ViaRule(String)` constructor (ViaRule.java:24-26).
    pub fn new(name: impl Into<String>) -> ViaRule {
        ViaRule {
            name: name.into(),
            vias: Vec::new(),
        }
    }

    /// Port of `ViaRule.EMPTY` (ViaRule.java:18): the shared, never-modified empty rule named
    /// `"empty"`.
    ///
    /// Java exposes it as a `static final` singleton whose identity matters nowhere (its one
    /// user, `gui/interactive/MakeSpaceState.java:42`, only reads it); a `String` field cannot be
    /// built in a `const`, so the port hands out a fresh equal value instead.
    pub fn empty() -> ViaRule {
        ViaRule::new("empty")
    }

    /// Port of `ViaRule.appendVia` (ViaRule.java:29-31). Java appends the caller's *reference*;
    /// the port takes the caller's copy, which is where the aliasing is cut.
    pub fn append_via(&mut self, via: ViaInfo) {
        self.vias.push(via);
    }

    /// Port of `ViaRule.removeVia` (ViaRule.java:34-36): removes the *first* element equal to
    /// `via`, returning false if the rule did not contain it (Java's `List.remove(Object)`).
    ///
    /// **Deviation, guarded.** `ViaInfo` does not override `equals` (ViaInfo.java:13-107 declares
    /// none), so Java's `List.remove(Object)` compares by **reference**; the port compares by
    /// value. The two agree wherever a rule cannot hold two equal `ViaInfo`s, which is true of
    /// **this method's** whole caller set: `removeVia` has exactly one non-GUI Java caller,
    /// `BoardRules.createDefaultViaRule` (BoardRules.java:189), which appends at most one via per
    /// `viaInfos` entry into a rule it has just built, and `ViaInfos::add` (ViaInfos.java:22-28)
    /// rejects duplicate names. (`gui/windows/routing/WindowViaRule.java:184` is the other caller
    /// and is out of scope.) Asserted by `a_rule_cannot_hold_two_equal_via_infos`.
    ///
    /// The guard is **this method's alone** — do not read it as covering [`Self::contains`], whose
    /// caller set is different and whose deviation is live. See that method.
    pub fn remove_via(&mut self, via: &ViaInfo) -> bool {
        match self.vias.iter().position(|v| v == via) {
            Some(index) => {
                self.vias.remove(index);
                true
            }
            None => false,
        }
    }

    /// Port of `ViaRule.viaCount` (ViaRule.java:39-41).
    pub fn via_count(&self) -> usize {
        self.vias.len()
    }

    /// Port of `ViaRule.getVia` (ViaRule.java:44-47). Java asserts the index is in range and
    /// would then throw; the port panics on the same out-of-range index.
    ///
    /// **Java-wins correction to the Plan 7 Task 0 brief**, which specified
    /// `-> Option<&ViaInfo>` on the grounds that `getVia` "returns **null** for an out-of-range
    /// index". It does not: `:45` is `assert index >= 0 && index < list.size();` and `:46` is
    /// `return list.get(index);`, and `LinkedList.get` throws `IndexOutOfBoundsException`. There
    /// is no `null` return, so the Global Constraint's "`Option` where Java returns `null`" does
    /// not apply, and an `Option` would have invented a `None` no Java caller can observe. The
    /// panic also matches [`ViaInfos::get`]'s existing convention for the identical Java shape.
    pub fn get_via(&self, index: usize) -> &ViaInfo {
        &self.vias[index]
    }

    /// The rule's vias in preference order. Not a Java method — Java iterates `list` directly
    /// (ViaRule.java:56,66,79,93).
    pub fn iter(&self) -> std::slice::Iter<'_, ViaInfo> {
        self.vias.iter()
    }

    /// Port of `ViaRule.contains` (ViaRule.java:55-62). Java's loop body is a literal
    /// `viaInfo == currentInfo` (`:57`) — object **identity**, because `ViaInfo.java` declares
    /// no `equals` — and since Plan 7 Task 11 so is this: the comparison is
    /// [`ViaInfo::is_same_object`], not `==`.
    ///
    /// # Why identity, and what it changes
    ///
    /// `contains` has exactly one non-GUI Java caller, and it is a *dedup* guard across **two
    /// separately built rules**:
    ///
    /// ```java
    /// // board/facade/RoutingBoard.java:1028-1041 (RoutingBoard.fanout, :978)
    /// ViaRule combinedViaRule = new ViaRule(ctrlSettings.viaRule.name + "_fallback");
    /// for (int i = 0; i < ctrlSettings.viaRule.viaCount(); i++)     // :1030-1032
    ///   combinedViaRule.appendVia(ctrlSettings.viaRule.getVia(i));
    /// ViaRule defaultViaRule = this.rules.viaRules.firstElement();  // :1035
    /// for (int i = 0; i < defaultViaRule.viaCount(); i++) {
    ///   ViaInfo defaultVia = defaultViaRule.getVia(i);
    ///   if (!combinedViaRule.contains(defaultVia))                  // :1037
    ///     combinedViaRule.appendVia(defaultVia);                    // :1038
    /// }
    /// ```
    ///
    /// Two rules built at different moments can hold value-equal-but-distinct `ViaInfo`s — a
    /// `.rules` file that re-declares a `(via …)` with identical values (`RulesReader.applyViaInfo`
    /// registers a **new** object) and then re-declares the `(via_rule …)` that names it — and
    /// there Java **appends a duplicate** where a value comparison silently skips it. That
    /// changes `combinedViaRule`'s length and order, which `:1042-1043` feeds straight into
    /// `AutorouteControl.rebuildViaInfo`. It is the same aliasing ruling H closed, one method
    /// over; quirk **#218** records it and
    /// `crates/fr-router/tests/fanout_order.rs`'s
    /// `the_combined_via_rule_appends_a_value_equal_via_from_a_second_rule` pins it.
    ///
    /// A via info that was never registered with a [`ViaInfos`] has no identity and is contained
    /// in nothing — see the `ViaInfo::serial` field for why that asymmetry is the safe one.
    ///
    /// (`gui/windows/routing/WindowEditVias.java:191` and `WindowViaRule.java:148` are the other
    /// two callers, both out of scope.)
    // added in Plan 7: Task 0; made reference-faithful in Task 11 (controller ruling AN).
    pub fn contains(&self, via_info: &ViaInfo) -> bool {
        self.vias.iter().any(|v| v.is_same_object(via_info))
    }

    /// Port of `ViaRule.containsPadstack` (ViaRule.java:65-72): true if any via in this rule uses
    /// `padstack`. Java reads `currentInfo.getPadstack()` straight off the held object, and so
    /// does the port now — no [`ViaInfos`] argument any more.
    pub fn contains_padstack(&self, padstack: PadstackId) -> bool {
        self.vias.iter().any(|v| v.get_padstack() == padstack)
    }

    /// Port of `ViaRule.getLayerRange` (ViaRule.java:78-86): the first via in this rule whose
    /// padstack starts on `from_layer` and ends on `to_layer`, or `None`.
    ///
    /// Java reads `currentInfo.getPadstack().fromLayer()`; the port reads the held `ViaInfo`
    /// directly and resolves the padstack index through `padstacks` (see [`PadstackLookup`]).
    pub fn get_layer_range(
        &self,
        from_layer: i32,
        to_layer: i32,
        padstacks: &impl PadstackLookup,
    ) -> Option<&ViaInfo> {
        self.vias.iter().find(|via| {
            let padstack = via.get_padstack();
            padstacks.padstack_from_layer(padstack) == from_layer
                && padstacks.padstack_to_layer(padstack) == to_layer
        })
    }
}

impl fmt::Display for ViaRule {
    // renamed: ViaRule.toString -> Display::fmt (ViaRule.java:50-52 returns `name`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for Task 4's `Padstacks`: padstack `n` spans layers `n ..= n + 1` and has a
    /// max width of `10 * (n + 1)`.
    struct TestPadstacks;

    impl PadstackLookup for TestPadstacks {
        fn padstack_from_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32
        }
        fn padstack_to_layer(&self, padstack: PadstackId) -> i32 {
            padstack.0 as i32 + 1
        }
        fn padstack_shape_max_width(&self, padstack: PadstackId, _layer: i32) -> Option<f64> {
            Some(10.0 * (padstack.0 as f64 + 1.0))
        }
    }

    fn via_infos() -> ViaInfos {
        let mut infos = ViaInfos::new();
        infos.add(ViaInfo::new("via0", PadstackId(0), 1, false));
        infos.add(ViaInfo::new("via1", PadstackId(1), 1, true));
        infos
    }

    #[test]
    fn add_rejects_duplicate_names() {
        // ViaInfos.java:23-25.
        let mut infos = via_infos();
        assert_eq!(infos.count(), 2);
        assert!(!infos.add(ViaInfo::new("via0", PadstackId(5), 2, false)));
        assert_eq!(infos.count(), 2);
        assert_eq!(infos.get(ViaInfoId(0)).get_padstack(), PadstackId(0));
    }

    #[test]
    fn name_lookup_is_case_sensitive() {
        // ViaInfos.java:44 uses `equals`.
        let infos = via_infos();
        assert_eq!(infos.get_no("via1"), Some(ViaInfoId(1)));
        assert_eq!(infos.get_no("VIA1"), None);
        assert!(infos.name_exists("via0"));
        assert!(!infos.name_exists("via2"));
        assert_eq!(
            infos.get_by_name("via1").map(ViaInfo::get_name),
            Some("via1")
        );
    }

    #[test]
    fn remove_reports_whether_the_via_was_present() {
        // ViaInfos.java:62-64.
        let mut infos = via_infos();
        assert!(infos.remove(ViaInfoId(0)));
        assert_eq!(infos.count(), 1);
        // The renumbering hazard: what was index 1 is now index 0.
        assert_eq!(infos.get(ViaInfoId(0)).get_name(), "via1");
        assert!(!infos.remove(ViaInfoId(7)));
    }

    /// `RulesReader.applyViaInfo` (RulesReader.java:340-350) as this port spells it since Plan 7
    /// Task 0: remove-then-append moves the entry to the tail of the list, and **no rule moves
    /// with it** — each rule keeps the copy it took, exactly as Java's rules keep the object they
    /// hold.
    ///
    /// Named `replace_via_info_renumbers_every_rule` until Plan 7 Task 0.
    #[test]
    fn replace_via_info_leaves_every_rule_alone() {
        let layer_structure = crate::structure::LayerStructure::new(vec![
            crate::structure::Layer::new("F.Cu", true),
            crate::structure::Layer::new("B.Cu", true),
        ]);
        let clearance_matrix =
            crate::rules::ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
        let mut rules = super::super::BoardRules::new(layer_structure, clearance_matrix);
        rules
            .via_infos
            .add(ViaInfo::new("A", PadstackId(1), 1, false));
        rules
            .via_infos
            .add(ViaInfo::new("B", PadstackId(2), 1, false));
        rules
            .via_infos
            .add(ViaInfo::new("C", PadstackId(3), 1, false));
        let mut rule = ViaRule::new("r");
        rule.append_via(rules.via_infos.get(ViaInfoId(0)).clone()); // A
        rule.append_via(rules.via_infos.get(ViaInfoId(2)).clone()); // C
        rule.append_via(rules.via_infos.get(ViaInfoId(1)).clone()); // B
        rules.via_rules.push(rule);

        // Re-apply "A" with a different padstack, exactly as a `(via A …)` scope in a `.rules`
        // file does on a board that already has an "A".
        let old_id = rules.via_infos.get_no("A").expect("A is present");
        let new_id = rules.replace_via_info(old_id, ViaInfo::new("A", PadstackId(9), 2, true));

        // The list order Java's remove-then-add produces: B, C, A.
        assert_eq!(new_id, ViaInfoId(2));
        let names: Vec<&str> = rules.via_infos.iter().map(ViaInfo::get_name).collect();
        assert_eq!(names, ["B", "C", "A"]);
        assert_eq!(rules.via_infos.get(new_id).get_padstack(), PadstackId(9));

        // The rule still names the same three vias, in the same order — and its "A" is the
        // *detached original*, padstack 1, not the replacement's padstack 9.
        let rule = &rules.via_rules[0];
        let resolved: Vec<&str> = rule.iter().map(ViaInfo::get_name).collect();
        assert_eq!(resolved, ["A", "C", "B"]);
        assert_eq!(rule.get_via(0).get_padstack(), PadstackId(1));
        assert_eq!(rule.get_via(0).get_clearance_class_index(), 1);
        assert!(!rule.get_via(0).attach_smd_allowed());
    }

    /// A rule takes a **copy**: mutating the entry in [`ViaInfos`] afterwards leaves the rule's
    /// via alone. Java shares the object here; the one place that matters is
    /// `BoardRules.changeClearanceClassIndex`/`removeClearanceClass`, both GUI-only — see the
    /// [`ViaRule`] doc.
    #[test]
    fn a_via_rule_holds_its_own_copy() {
        let mut infos = via_infos();
        let mut rule = ViaRule::new("r");
        rule.append_via(infos.get(ViaInfoId(0)).clone());

        infos.get_mut(ViaInfoId(0)).set_attach_smd_allowed(true);
        infos.get_mut(ViaInfoId(0)).set_padstack(PadstackId(7));
        infos.get_mut(ViaInfoId(0)).set_clearance_class_index(9);

        assert!(!rule.get_via(0).attach_smd_allowed());
        assert_eq!(rule.get_via(0).get_padstack(), PadstackId(0));
        assert_eq!(rule.get_via(0).get_clearance_class_index(), 1);
    }

    /// `ViaRule.removeVia` (ViaRule.java:34-36) is `List.remove(Object)`, which removes the
    /// **first** matching element and stops.
    #[test]
    fn remove_via_removes_the_first_equal_element() {
        let a = ViaInfo::new("a", PadstackId(0), 1, false);
        let b = ViaInfo::new("b", PadstackId(1), 1, false);
        let mut rule = ViaRule::new("r");
        rule.append_via(a.clone());
        rule.append_via(b.clone());
        rule.append_via(a.clone());
        assert_eq!(rule.via_count(), 3);

        assert!(rule.remove_via(&a));
        assert_eq!(
            rule.iter().map(ViaInfo::get_name).collect::<Vec<_>>(),
            ["b", "a"],
            "only the first equal element goes"
        );
        assert!(rule.remove_via(&a));
        assert!(!rule.remove_via(&a));
    }

    /// The guard on [`ViaRule::remove_via`]'s value-vs-reference deviation. `removeVia`'s only
    /// non-GUI Java caller is `BoardRules.createDefaultViaRule` (BoardRules.java:189), and the
    /// rule it calls it on cannot hold two equal `ViaInfo`s: it appends at most one via per
    /// `viaInfos` entry (`:177-196`) and `ViaInfos::add` (ViaInfos.java:22-28) rejects duplicate
    /// names.
    ///
    /// **This does not generalise to every via rule** — Java has six non-GUI `new ViaRule(…)`
    /// sites (`BoardRules.java:174`, `Network.java:402` and `:694`, `KiCadJsonReader.java:403`
    /// and `:439`, `RoutingBoard.java:1029`) and the last of them, `fanout`'s merge, is exactly
    /// where the same value-vs-reference difference *is* observable. That one is
    /// [`ViaRule::contains`]' problem, not [`ViaRule::remove_via`]'s; see its doc.
    #[test]
    fn a_rule_cannot_hold_two_equal_via_infos() {
        let layer_structure = crate::structure::LayerStructure::new(vec![
            crate::structure::Layer::new("F.Cu", true),
            crate::structure::Layer::new("B.Cu", true),
        ]);
        let clearance_matrix =
            crate::rules::ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
        let mut rules = super::super::BoardRules::new(layer_structure.clone(), clearance_matrix);
        // Three vias in the default via clearance class, two of them sharing a layer range so
        // the `removeVia`/`appendVia` branch at BoardRules.java:190-193 runs too.
        assert!(
            rules
                .via_infos
                .add(ViaInfo::new("A", PadstackId(0), 1, false))
        );
        assert!(
            rules
                .via_infos
                .add(ViaInfo::new("B", PadstackId(0), 1, false))
        );
        assert!(
            rules
                .via_infos
                .add(ViaInfo::new("C", PadstackId(1), 1, false))
        );
        // `add` is what forbids the duplicate in the first place.
        assert!(
            !rules
                .via_infos
                .add(ViaInfo::new("A", PadstackId(3), 1, false))
        );

        let net_class = rules.net_classes.append("default", &layer_structure, false);
        rules.create_default_via_rule(net_class, "default", &TestPadstacks);

        let rule = &rules.via_rules[0];
        for i in 0..rule.via_count() {
            for j in 0..i {
                assert_ne!(
                    rule.get_via(i),
                    rule.get_via(j),
                    "a via rule holds no two equal ViaInfos, so value equality is reference \
                     equality for `remove_via`"
                );
            }
        }
    }

    /// `Network.addViaRule`'s replacement half is Java's two lines and nothing else, since
    /// Plan 7 Task 11: the replaced rule leaves [`BoardRules::via_rules`] and the replacement is
    /// appended to the tail, and **no net class is touched** — a class that held the replaced
    /// rule keeps its own copy of the detached original, which is exactly what
    /// `NetClass.viaRule`'s object reference does in Java.
    #[test]
    fn replace_via_rule_leaves_every_net_class_holding_what_it_held() {
        let layer_structure = crate::structure::LayerStructure::new(vec![
            crate::structure::Layer::new("F.Cu", true),
            crate::structure::Layer::new("B.Cu", true),
        ]);
        let clearance_matrix =
            crate::rules::ClearanceMatrix::new(2, &layer_structure, &["null", "default"]);
        let mut rules = super::super::BoardRules::new(layer_structure.clone(), clearance_matrix);
        let mut original = ViaRule::new("default");
        rules
            .via_infos
            .add(ViaInfo::new("A", PadstackId(0), 1, false));
        original.append_via(rules.via_infos.get(ViaInfoId(0)).clone());
        rules.via_rules.push(original.clone());
        rules.via_rules.push(ViaRule::new("wide"));
        rules.via_rules.push(ViaRule::new("narrow"));

        let a = rules.net_classes.append("a", &layer_structure, false);
        let b = rules.net_classes.append("b", &layer_structure, false);
        let c = rules.net_classes.append("c", &layer_structure, false);
        rules.net_classes.get_mut(a).set_via_rule(Some(original));
        rules
            .net_classes
            .get_mut(b)
            .set_via_rule(Some(ViaRule::new("narrow")));
        rules.net_classes.get_mut(c).set_via_rule(None);

        let new_id = rules.replace_via_rule(ViaRuleId(0), ViaRule::new("default"));

        assert_eq!(new_id, ViaRuleId(2));
        let names: Vec<&str> = rules.via_rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["wide", "narrow", "default"]);
        // `a` held the replaced rule and still holds it, vias and all — the detached original
        // Java's `NetClass.viaRule` keeps (NetClass.java:28). The replacement is empty.
        let a_rule = rules.net_classes.get(a).get_via_rule().expect("a's rule");
        assert_eq!(a_rule.name, "default");
        assert_eq!(a_rule.via_count(), 1);
        assert_eq!(rules.via_rules[2].via_count(), 0);
        assert_eq!(
            rules.net_classes.get(b).get_via_rule().map(|r| &r.name),
            Some(&"narrow".to_string())
        );
        assert_eq!(rules.net_classes.get(c).get_via_rule(), None);
    }

    #[test]
    fn via_info_accessors_round_trip() {
        let mut info = ViaInfo::new("v", PadstackId(2), 3, false);
        assert_eq!(info.get_name(), "v");
        assert_eq!(info.get_padstack(), PadstackId(2));
        assert_eq!(info.get_clearance_class_index(), 3);
        assert!(!info.attach_smd_allowed());
        assert_eq!(info.to_string(), "v");

        info.set_name("w");
        info.set_padstack(PadstackId(4));
        info.set_clearance_class_index(5);
        info.set_attach_smd_allowed(true);
        assert_eq!(info.get_name(), "w");
        assert_eq!(info.get_padstack(), PadstackId(4));
        assert_eq!(info.get_clearance_class_index(), 5);
        assert!(info.attach_smd_allowed());
    }

    #[test]
    fn via_info_compare_to_is_case_sensitive() {
        // ViaInfo.java:82 uses `compareTo`, not `compareToIgnoreCase`.
        let a = ViaInfo::new("Via", PadstackId(0), 0, false);
        let b = ViaInfo::new("via", PadstackId(0), 0, false);
        assert_eq!(a.compare_to(&b), Ordering::Less);
        assert_eq!(a.compare_to(&a.clone()), Ordering::Equal);
    }

    #[test]
    fn via_rule_append_remove_and_contains() {
        let infos = via_infos();
        let (v0, v1) = (infos.get(ViaInfoId(0)), infos.get(ViaInfoId(1)));
        let absent = ViaInfo::new("via2", PadstackId(2), 1, false);
        let mut rule = ViaRule::new("default");
        assert_eq!(rule.name, "default");
        rule.append_via(v0.clone());
        rule.append_via(v1.clone());
        assert_eq!(rule.via_count(), 2);
        assert_eq!(rule.get_via(0), v0);
        assert!(rule.contains(v1));
        assert!(!rule.contains(&absent));

        assert!(rule.remove_via(v0));
        assert_eq!(rule.via_count(), 1);
        assert_eq!(rule.get_via(0), v1);
        // ViaRule.java:35: removing something absent answers false.
        assert!(!rule.remove_via(v0));
    }

    #[test]
    fn contains_padstack_reads_the_held_via() {
        // ViaRule.java:65-72.
        let infos = via_infos();
        let mut rule = ViaRule::new("default");
        rule.append_via(infos.get(ViaInfoId(1)).clone());
        assert!(rule.contains_padstack(PadstackId(1)));
        assert!(!rule.contains_padstack(PadstackId(0)));
    }

    #[test]
    fn get_layer_range_finds_the_first_matching_via() {
        // ViaRule.java:78-86.
        let infos = via_infos();
        let mut rule = ViaRule::new("default");
        rule.append_via(infos.get(ViaInfoId(0)).clone());
        rule.append_via(infos.get(ViaInfoId(1)).clone());

        assert_eq!(
            rule.get_layer_range(0, 1, &TestPadstacks)
                .map(ViaInfo::get_name),
            Some("via0")
        );
        assert_eq!(
            rule.get_layer_range(1, 2, &TestPadstacks)
                .map(ViaInfo::get_name),
            Some("via1")
        );
        assert!(rule.get_layer_range(3, 4, &TestPadstacks).is_none());
    }
}
