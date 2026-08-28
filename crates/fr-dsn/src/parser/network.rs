//! `io/specctra/parser/{Net,NetList,Network}.java` — the `network` scope.

use crate::error::DsnError;
use crate::parser::scope_parameter::{ReadScopeParameter, skip_scope};

/// `Net.Id` (Net.java:86-104 — the DSN-parser's own `Net`, not `rules.Net`): the `TreeMap`/
/// `BTreeMap` key for [`ReadScopeParameter::netlist`](crate::parser::scope_parameter::ReadScopeParameter).
///
/// Field order matters: deriving `Ord` over `(name, subnet_no)` in this order reproduces
/// `Id.compareTo` exactly — `this.name.compareTo(other.name)` (plain, **not**
/// `compareToIgnoreCase` — unlike `rules.Net.compareTo`, this one is case-sensitive), falling
/// back to `this.subnetNumber - other.subnetNumber` only when the names are equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetId {
    /// `Net.Id.name` (Net.java:88).
    pub name: String,
    /// `Net.Id.subnetNumber` (Net.java:89).
    pub subnet_no: i32,
}

/// `io/specctra/parser/Net.java` (the DSN-parser's `Net`, distinct from `rules.Net`): a net as
/// read from a `network` scope, before it is resolved against `rules.Nets`.
///
/// Placeholder — body (the pin set, `Net.setPins`/`getPins`) arrives with `Network.readScope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsnNet {
    /// `Net.id` (Net.java:16).
    pub id: NetId,
}

impl DsnNet {
    /// `Net(Id)` (Net.java:22-24).
    #[must_use]
    pub fn new(id: NetId) -> DsnNet {
        DsnNet { id }
    }
}

// added in Plan 3: Network.readScope
/// Stub for `Network.readScope` (Network.java) — replaced with the real reader by a later task;
/// for now this just discards the scope's body.
pub fn read_network_scope(p: &mut ReadScopeParameter<'_>) -> Result<bool, DsnError> {
    skip_scope(&mut p.scanner)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_id_orders_by_name_then_subnet() {
        let a = NetId {
            name: "GND".to_string(),
            subnet_no: 1,
        };
        let b = NetId {
            name: "GND".to_string(),
            subnet_no: 2,
        };
        let c = NetId {
            name: "VCC".to_string(),
            subnet_no: 0,
        };
        assert!(a < b);
        assert!(b < c);
    }
}
