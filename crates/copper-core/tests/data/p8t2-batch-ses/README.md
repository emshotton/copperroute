# `p8t2-batch-ses`

Eight routed sessions read for their bytes by `crates/copper-core/tests/stats.rs`: the byte
statistics transcript records what `BoardStatistics::from_bytes` scrapes from each of them, and
one of them is the only real file whose `(parser …)` scope spells `hostCad` in camelCase, which
is the shape the host scrape succeeds on. They are inputs, not references; nothing regenerates
them.
