//! Electrical nets and the board's net list.
//!
//! Java: `rules/Net.java`, `rules/Nets.java`.

use std::cmp::Ordering;
use std::fmt;

use crate::ids::NetClassId;

use super::{compare_to_ignore_case, equals_ignore_case};

/// Port of `Net` (`rules/Net.java`): the properties of one electrical net.
///
/// not ported: `Net.netList` (Net.java:35), the back-pointer to the owning [`Nets`] — and
/// through it to the board. Every method that used it is listed below.
///
/// All five of the following walk `netList.getBoard().itemList`, which does not exist in this
/// crate yet:
///
/// not ported: `Net.getTerminalItems` (Net.java:75-91)
/// not ported: `Net.getPins` (Net.java:94-110)
/// not ported: `Net.getItems` (Net.java:113-127)
/// not ported: `Net.getTraceLength` (Net.java:130-140)
/// not ported: `Net.getViaCount` (Net.java:143-152)
// added in Task 11: `Board::net_terminal_items`, `Board::net_pins`, `Board::net_items`,
// `Board::net_trace_length` and `Board::net_via_count` (the board owns the item list, so these
// become board queries taking a net number). Their live callers are
// `drc/NetIncompletes.java:265` (`getTraceLength`) and `board/state/BoardComparator.java:270`
// (`getPins`); the other three are reached only from `Net.printInfo`.
///
/// not ported: `Net.printInfo` (Net.java:169-192) — `ItemInfoPrinter.Printable`, GUI only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
    /// `Net.name` (Net.java:23).
    pub name: String,
    /// `Net.subnetNumber` (Net.java:29): only used when a net is split internally by a from-to
    /// rule; 1 for a normal net.
    pub subnet_number: i32,
    /// `Net.netNumber` (Net.java:32): the unique strictly positive number of the net.
    pub net_number: i32,
    /// `Net.containsPlane` (Net.java:38). Private in Java behind
    /// `containsPlane()`/`setContainsPlane`, which are also ported below.
    pub contains_plane: bool,
    /// `Net.netClass` (Net.java:41), as an index rather than an object reference.
    ///
    /// Java's constructor initialises it to `netList.getBoard().rules.getDefaultNetClass()`
    /// (Net.java:50); without a board back-pointer the caller supplies it — see
    /// [`Nets::add`].
    pub net_class: NetClassId,
}

impl Net {
    /// Port of the `Net(String, int, int, Nets, boolean)` constructor (Net.java:44-51), with the
    /// `Nets` back-pointer replaced by the explicit default net class it was used to look up.
    pub fn new(
        name: impl Into<String>,
        subnet_number: i32,
        number: i32,
        contains_plane: bool,
        net_class: NetClassId,
    ) -> Net {
        Net {
            name: name.into(),
            subnet_number,
            net_number: number,
            contains_plane,
            net_class,
        }
    }

    /// Port of `Net.getNetClass` (Net.java:65-67).
    pub fn get_net_class(&self) -> NetClassId {
        self.net_class
    }

    /// Port of `Net.setClass` (Net.java:70-72).
    pub fn set_class(&mut self, net_class: NetClassId) {
        self.net_class = net_class;
    }

    /// Port of `Net.containsPlane` (Net.java:164-166).
    pub fn contains_plane(&self) -> bool {
        self.contains_plane
    }

    /// Port of `Net.setContainsPlane` (Net.java:155-157).
    pub fn set_contains_plane(&mut self, value: bool) {
        self.contains_plane = value;
    }

    /// Port of `Net.compareTo` (Net.java:60-62): `name.compareToIgnoreCase(other.name)`, which
    /// orders nets alphabetically for display.
    ///
    /// Deliberately *not* an `Ord` impl: Java's ordering says two nets whose names differ only in
    /// case are equal, while `PartialEq` here is structural, so an `Ord` impl would break the
    /// `a == b <=> cmp(a, b) == Equal` contract.
    pub fn compare_to(&self, other: &Net) -> Ordering {
        compare_to_ignore_case(&self.name, &other.name)
    }
}

impl fmt::Display for Net {
    // renamed: Net.toString -> Display::fmt (Net.java:54-56: `"Net #" + netNumber + " (" + name
    // + ")"`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Net #{} ({})", self.net_number, self.name)
    }
}

/// Port of `Nets` (`rules/Nets.java`): the electrical nets on a board.
///
/// A net's number is its 1-based position in the list, so `nets[n - 1].net_number == n`.
///
/// not ported: `Nets.board` with `getBoard` (Nets.java:99-101) and `setBoard`
/// (Nets.java:104-106). The board back-pointer is a Plan 2 design rule; its only external
/// setter is `board/facade/BasicBoard.java:134` (`rules.nets.setBoard(this)`), and every reader
/// is one of the five `Net` methods listed on [`Net`] plus the `Net` constructor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nets {
    /// `Nets.nets` (Nets.java:22), a `Vector<Net>`.
    nets: Vec<Net>,
}

impl Nets {
    /// `Nets.max_legal_net_number` (Nets.java:16): the largest net number a real net may have.
    pub const MAX_LEGAL_NET_NUMBER: i32 = 9_999_999;

    /// `Nets.hidden_net_number` (Nets.java:19): the auxiliary net number for internal use.
    pub const HIDDEN_NET_NUMBER: i32 = 10_000_001;

    /// Port of the `Nets()` constructor (Nets.java:27-29): an empty net list.
    pub fn new() -> Nets {
        Nets::default()
    }

    /// Port of `Nets.isNormalNetNumber` (Nets.java:32-34): false for the internally used
    /// special-purpose net numbers.
    pub fn is_normal_net_number(net_number: i32) -> bool {
        net_number > 0 && net_number <= Nets::MAX_LEGAL_NET_NUMBER
    }

    /// Port of `Nets.maxNetNumber` (Nets.java:37-39): the biggest net number on the board, which
    /// is just the list length because numbers are consecutive from 1.
    pub fn max_net_number(&self) -> i32 {
        self.nets.len() as i32
    }

    /// Port of `Nets.get(String, int)` (Nets.java:42-51): the net with the given name (ignoring
    /// case) and subnet number.
    pub fn get_by_name_and_subnet(&self, name: &str, subnet_number: i32) -> Option<&Net> {
        self.nets
            .iter()
            .find(|n| equals_ignore_case(&n.name, name) && n.subnet_number == subnet_number)
    }

    /// Port of `Nets.get(String)` (Nets.java:54-62): all subnets with the given name, ignoring
    /// case.
    pub fn get_by_name(&self, name: &str) -> Vec<&Net> {
        self.nets
            .iter()
            .filter(|n| equals_ignore_case(&n.name, name))
            .collect()
    }

    /// Port of `Nets.get(int)` (Nets.java:65-74): the net with the given number, or `None`
    /// (Java: `null`) if there is none.
    ///
    /// Java's `FRLogger.warn("Nets.get: inconsistent netNumber")` guard becomes a `debug_assert!`
    /// per `global-constraints.md`.
    pub fn get(&self, net_number: i32) -> Option<&Net> {
        if net_number < 1 || net_number > self.nets.len() as i32 {
            return None;
        }
        let result = &self.nets[(net_number - 1) as usize];
        debug_assert_eq!(
            result.net_number, net_number,
            "Nets.get: inconsistent netNumber (Nets.java:70-72)"
        );
        Some(result)
    }

    /// Mutable counterpart of [`Self::get`]. Java needs none: `get` hands back a mutable object.
    pub fn get_mut(&mut self, net_number: i32) -> Option<&mut Net> {
        if net_number < 1 || net_number > self.nets.len() as i32 {
            return None;
        }
        Some(&mut self.nets[(net_number - 1) as usize])
    }

    /// Every net, in net-number order.
    ///
    /// Not a Java method: `Nets.nets` is private and Java's own loops
    /// (Nets.java:43,56) iterate it directly.
    pub fn iter(&self) -> std::slice::Iter<'_, Net> {
        self.nets.iter()
    }

    /// The number of nets in the list. Not a Java method — Java reads `nets.size()` directly
    /// (Nets.java:38,80,89).
    pub fn count(&self) -> usize {
        self.nets.len()
    }

    /// Port of `Nets.newNet(Locale)` (Nets.java:77-82): appends a net with a generated name.
    ///
    /// not ported: the `Locale` parameter. Java builds the name from
    /// `new TextManager(NetClasses.class, locale).getText("net#")`, but no resource bundle is
    /// registered for `app.freerouting.rules.NetClasses` and the shared `app.freerouting.Common`
    /// bundle has no `net#` key either, so `TextManager.getText` falls through to
    /// `return key` (TextManager.java:252-254) in every locale and the name is always
    /// `"net#" + (nets.size() + 1)`.
    ///
    /// The method has no caller anywhere in the Java tree (`grep -rn "newNet("` finds only this
    /// definition); it is ported because it is cheap and unambiguous.
    pub fn new_net(&mut self, net_class: NetClassId) -> &mut Net {
        let net_name = format!("net#{}", self.nets.len() + 1);
        self.add(net_name, 1, false, net_class)
    }

    /// Port of `Nets.add` (Nets.java:88-96): appends a net with the next free number.
    ///
    /// `net_class` replaces Java's `netList.getBoard().rules.getDefaultNetClass()` lookup inside
    /// the `Net` constructor (Net.java:50). Java's `FRLogger.warn` on running out of legal net
    /// numbers becomes a `debug_assert!` per `global-constraints.md`; note that Java only warns
    /// and still creates the net, so the port must not refuse it either.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        subnet_number: i32,
        contains_plane: bool,
        net_class: NetClassId,
    ) -> &mut Net {
        let new_net_no = self.nets.len() as i32 + 1;
        debug_assert!(
            new_net_no < Nets::MAX_LEGAL_NET_NUMBER,
            "Nets.add_net: maxNetNo out of range (Nets.java:90-92)"
        );
        self.nets.push(Net::new(
            name,
            subnet_number,
            new_net_no,
            contains_plane,
            net_class,
        ));
        self.nets.last_mut().expect("just pushed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_CLASS: NetClassId = NetClassId(0);

    #[test]
    fn add_numbers_nets_from_one() {
        let mut nets = Nets::new();
        assert_eq!(nets.max_net_number(), 0);
        assert_eq!(nets.add("GND", 1, false, DEFAULT_CLASS).net_number, 1);
        assert_eq!(nets.add("VCC", 1, true, DEFAULT_CLASS).net_number, 2);
        assert_eq!(nets.max_net_number(), 2);
    }

    #[test]
    fn get_is_one_based_and_bounds_checked() {
        // Nets.java:66-69: `netNumber < 1 || netNumber > nets.size()` is null.
        let mut nets = Nets::new();
        nets.add("GND", 1, false, DEFAULT_CLASS);
        assert_eq!(nets.get(1).map(|n| n.name.as_str()), Some("GND"));
        assert!(nets.get(0).is_none());
        assert!(nets.get(2).is_none());
        assert!(nets.get(-5).is_none());
    }

    #[test]
    fn get_by_name_ignores_case_and_collects_subnets() {
        // Nets.java:44,57 both use `equalsIgnoreCase`.
        let mut nets = Nets::new();
        nets.add("GND", 1, false, DEFAULT_CLASS);
        nets.add("gnd", 2, false, DEFAULT_CLASS);
        nets.add("VCC", 1, false, DEFAULT_CLASS);

        assert_eq!(nets.get_by_name("gnd").len(), 2);
        assert_eq!(nets.get_by_name("GND").len(), 2);
        assert_eq!(
            nets.get_by_name_and_subnet("GND", 2).map(|n| n.net_number),
            Some(2)
        );
        assert!(nets.get_by_name_and_subnet("GND", 3).is_none());
        assert!(nets.get_by_name("missing").is_empty());
    }

    #[test]
    fn new_net_generates_the_untranslated_key_name() {
        // Nets.java:80: `tm.getText("net#") + (nets.size() + 1)`, and `getText` falls back to
        // the key itself (TextManager.java:252-254).
        let mut nets = Nets::new();
        assert_eq!(nets.new_net(DEFAULT_CLASS).name, "net#1");
        assert_eq!(nets.new_net(DEFAULT_CLASS).name, "net#2");
    }

    #[test]
    fn net_display_matches_java_to_string() {
        // Net.java:55.
        let net = Net::new("GND", 1, 7, false, DEFAULT_CLASS);
        assert_eq!(net.to_string(), "Net #7 (GND)");
    }

    #[test]
    fn compare_to_ignores_case() {
        // Net.java:61.
        let a = Net::new("gnd", 1, 1, false, DEFAULT_CLASS);
        let b = Net::new("GND", 1, 2, false, DEFAULT_CLASS);
        let c = Net::new("VCC", 1, 3, false, DEFAULT_CLASS);
        assert_eq!(a.compare_to(&b), std::cmp::Ordering::Equal);
        assert_eq!(a.compare_to(&c), std::cmp::Ordering::Less);
        assert_eq!(c.compare_to(&a), std::cmp::Ordering::Greater);
    }

    #[test]
    fn contains_plane_round_trips() {
        let mut net = Net::new("VCC", 1, 1, false, DEFAULT_CLASS);
        assert!(!net.contains_plane());
        net.set_contains_plane(true);
        assert!(net.contains_plane());
    }

    #[test]
    fn set_class_replaces_the_net_class_index() {
        let mut net = Net::new("GND", 1, 1, false, DEFAULT_CLASS);
        assert_eq!(net.get_net_class(), NetClassId(0));
        net.set_class(NetClassId(3));
        assert_eq!(net.get_net_class(), NetClassId(3));
    }
}
