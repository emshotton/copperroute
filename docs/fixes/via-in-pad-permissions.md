# Via-in-pad permissions

KiCad board JSON uses an explicit opt-in for newly routed vias to share SMD pad
copper. Add `"viaInPadAllowed": true` to the top-level board object to allow this.
An absent or false field disallows attachment. Each entry in `netClasses` can
override the board value with its own `viaInPadAllowed` boolean.

For example, merge these properties into a complete KiCad board JSON:

```json
{
  "viaInPadAllowed": false,
  "netClasses": [
    { "name": "Default" },
    { "name": "DenseBGA", "viaInPadAllowed": true }
  ]
}
```

This keeps attachment off for Default and allows it for nets assigned to
DenseBGA. Preserve the other board and net-class properties when editing an
export. These are extensions to the Rust JSON bridge; KiCad exporters may omit
them. They are not imported from a .kicad_pro project.

CLI users can edit the JSON before running
`freerouting route board.json -o routed.json`. MCP users can route the same
edited JSON through `route_board`. There is no separate CLI permission flag.
The browser integration supplies the top-level value from its checkbox.

The importer sets each via template's attachment permission and enables
`BoardRules.strict_smd_via_attachment`. Maze search and final insertion respect
the template permissions. `smd_via_relaxation` still discounts pure-SMD via costs
but does not override these KiCad permissions. Imported existing vias are
preserved in place.

DSN imports retain their established behavior: via rules carry the
`via_at_smd`/padstack permissions, and `smd_via_relaxation` additionally relaxes
the legacy pure-SMD search gate. That search relaxation is separate from the
per-via insertion masks. Disabling the setting removes the search relaxation
and cost discount, without revoking explicitly allowed via rules.

KiCad JSON export takes each class's permission from the same first via in its
effective net-class rule used for its exported via diameter and drill. It does
not use the separately stored via catalogue. The default class supplies the
top-level permission and differing classes carry overrides. Mixed class
permissions therefore survive a JSON round trip. The JSON format represents
one via template per class; additional DSN via templates are not representable.
DSN and SES output retain their native via rules and attachment controls.

Disallowing via-in-pad can require more routing space and leave connections
unrouted on dense boards.
