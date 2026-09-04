//! |-----|------|------|
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Guard {
        G1aTreeEntryOutOfRange = 0,
        G1bNullConnectionShape = 1,
        G2RoomArrayResized = 2,
        G2RoomIndexOutOfRange = 3,
        G3TraceCornerOutOfRange = 4,
}

impl Guard {
        pub const ALL: [Guard; 5] = [
        Guard::G1aTreeEntryOutOfRange,
        Guard::G1bNullConnectionShape,
        Guard::G2RoomArrayResized,
        Guard::G2RoomIndexOutOfRange,
        Guard::G3TraceCornerOutOfRange,
    ];

        pub fn tag(self) -> &'static str {
        match self {
            Guard::G1aTreeEntryOutOfRange => "G1a-tree-entry-out-of-range",
            Guard::G1bNullConnectionShape => "G1b-null-connection-shape",
            Guard::G2RoomArrayResized => "G2-room-array-resized",
            Guard::G2RoomIndexOutOfRange => "G2-room-index-out-of-range",
            Guard::G3TraceCornerOutOfRange => "G3-trace-corner-out-of-range",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mutation {
        None = 0,
            TiePinReduction = 1,
        RipupRemoveItems = 2,
        RemoveTraceTails = 3,
        ForcedTraceInsert = 4,
        OptChangedArea = 5,
}

impl Mutation {
    const COUNT: usize = 6;

        pub fn tag(self) -> &'static str {
        match Mutation::from_u8(self as u8) {
            Mutation::None => "none-yet",
            Mutation::TiePinReduction => "reduceTraceShapesAtTiePins",
            Mutation::RipupRemoveItems => "removeItems (ripup)",
            Mutation::RemoveTraceTails => "removeTraceTails",
            Mutation::ForcedTraceInsert => "insertForcedTracePolyline",
            Mutation::OptChangedArea => "optChangedArea",
        }
    }

    fn from_u8(value: u8) -> Mutation {
        match value {
            1 => Mutation::TiePinReduction,
            2 => Mutation::RipupRemoveItems,
            3 => Mutation::RemoveTraceTails,
            4 => Mutation::ForcedTraceInsert,
            5 => Mutation::OptChangedArea,
            _ => Mutation::None,
        }
    }
}

pub fn on() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P9T17_STALE").is_some());
    *ON || force_on()
}

fn force_on() -> bool {
    FORCED.load(Ordering::Relaxed)
}

static FORCED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_on(value: bool) {
    FORCED.store(value, Ordering::Relaxed);
}

static FIRES: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

static VISITS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

static FIRES_BY_MUTATION: [AtomicU64; 5 * Mutation::COUNT] = [const { AtomicU64::new(0) }; 30];

static MUTATIONS: [AtomicU64; Mutation::COUNT] = [const { AtomicU64::new(0) }; 6];

static LAST_MUTATION: AtomicU8 = AtomicU8::new(0);

static RECOVERABLE: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

static SAMPLES: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub const SAMPLE_LIMIT: usize = 64;

pub fn note_mutation(mutation: Mutation) {
    if !on() {
        return;
    }
    LAST_MUTATION.store(mutation as u8, Ordering::Relaxed);
    MUTATIONS[mutation as usize].fetch_add(1, Ordering::Relaxed);
}

pub fn record_visit(guard: Guard) {
    if !on() {
        return;
    }
    VISITS[guard as usize].fetch_add(1, Ordering::Relaxed);
}

pub fn record_guard(guard: Guard, item: u64, index: usize, available: usize, recoverable: bool) {
    if !on() {
        return;
    }
    let slot = guard as usize;
    FIRES[slot].fetch_add(1, Ordering::Relaxed);
    let last = LAST_MUTATION.load(Ordering::Relaxed);
    FIRES_BY_MUTATION[slot * Mutation::COUNT + last as usize].fetch_add(1, Ordering::Relaxed);
    if recoverable {
        RECOVERABLE[slot].fetch_add(1, Ordering::Relaxed);
    }
    if let Ok(mut samples) = SAMPLES.lock()
        && samples.len() < SAMPLE_LIMIT
    {
        samples.push(format!(
            "{} item={item} index={index} available={available} recoverable={recoverable} after={}",
            guard.tag(),
            Mutation::from_u8(last).tag(),
        ));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardRow {
        pub guard: Guard,
        pub visits: u64,
        pub fires: u64,
        pub recoverable: u64,
        pub by_mutation: Vec<(Mutation, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
        pub guards: Vec<GuardRow>,
        pub mutations: Vec<(Mutation, u64)>,
        pub samples: Vec<String>,
}

impl Snapshot {
        pub fn total_fires(&self) -> u64 {
        self.guards.iter().map(|row| row.fires).sum()
    }

        pub fn fires(&self, guard: Guard) -> u64 {
        self.guards
            .iter()
            .find(|row| row.guard == guard)
            .map_or(0, |row| row.fires)
    }

        pub fn visits(&self, guard: Guard) -> u64 {
        self.guards
            .iter()
            .find(|row| row.guard == guard)
            .map_or(0, |row| row.visits)
    }
}

pub fn snapshot() -> Snapshot {
    let guards = Guard::ALL
        .iter()
        .map(|guard| {
            let slot = *guard as usize;
            let by_mutation = (0..Mutation::COUNT)
                .map(|m| {
                    (
                        Mutation::from_u8(u8::try_from(m).unwrap_or(0)),
                        FIRES_BY_MUTATION[slot * Mutation::COUNT + m].load(Ordering::Relaxed),
                    )
                })
                .filter(|(_, count)| *count > 0)
                .collect();
            GuardRow {
                guard: *guard,
                visits: VISITS[slot].load(Ordering::Relaxed),
                fires: FIRES[slot].load(Ordering::Relaxed),
                recoverable: RECOVERABLE[slot].load(Ordering::Relaxed),
                by_mutation,
            }
        })
        .collect();
    let mutations = (0..Mutation::COUNT)
        .map(|m| {
            (
                Mutation::from_u8(u8::try_from(m).unwrap_or(0)),
                MUTATIONS[m].load(Ordering::Relaxed),
            )
        })
        .filter(|(_, count)| *count > 0)
        .collect();
    let samples = SAMPLES.lock().map(|s| s.clone()).unwrap_or_default();
    Snapshot {
        guards,
        mutations,
        samples,
    }
}

pub fn reset() {
    for slot in 0..Guard::ALL.len() {
        FIRES[slot].store(0, Ordering::Relaxed);
        VISITS[slot].store(0, Ordering::Relaxed);
        RECOVERABLE[slot].store(0, Ordering::Relaxed);
        for m in 0..Mutation::COUNT {
            FIRES_BY_MUTATION[slot * Mutation::COUNT + m].store(0, Ordering::Relaxed);
        }
    }
    for counter in &MUTATIONS {
        counter.store(0, Ordering::Relaxed);
    }
    LAST_MUTATION.store(0, Ordering::Relaxed);
    if let Ok(mut samples) = SAMPLES.lock() {
        samples.clear();
    }
}

pub fn render(stem: &str, snapshot: &Snapshot) -> String {
    let mut out = format!("=== stem {stem} ===\n");
    for row in &snapshot.guards {
        out.push_str(&format!(
            "{:<30} visits={:<10} fires={:<8} recoverable={:<8} by-mutation={}\n",
            row.guard.tag(),
            row.visits,
            row.fires,
            row.recoverable,
            if row.by_mutation.is_empty() {
                "-".to_string()
            } else {
                row.by_mutation
                    .iter()
                    .map(|(m, c)| format!("{}={c}", m.tag()))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
    }
    out.push_str("mutations ");
    if snapshot.mutations.is_empty() {
        out.push('-');
    } else {
        out.push_str(
            &snapshot
                .mutations
                .iter()
                .map(|(m, c)| format!("{}={c}", m.tag()))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    out.push('\n');
    for sample in &snapshot.samples {
        out.push_str("sample ");
        out.push_str(sample);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

            static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn a_recorder_is_inert_while_the_instrumentation_is_off() {
        let _serial = SERIAL.lock();
        set_on(false);
        reset();
        record_guard(Guard::G1aTreeEntryOutOfRange, 7, 3, 1, false);
        note_mutation(Mutation::TiePinReduction);
        assert_eq!(snapshot().total_fires(), 0);
        assert!(snapshot().mutations.is_empty());
    }

    #[test]
    fn a_fire_is_attributed_to_the_last_mutation() {
        let _serial = SERIAL.lock();
        set_on(true);
        reset();
        note_mutation(Mutation::TiePinReduction);
        record_guard(Guard::G1aTreeEntryOutOfRange, 7, 3, 1, false);
        note_mutation(Mutation::RipupRemoveItems);
        record_guard(Guard::G1aTreeEntryOutOfRange, 8, 4, 2, true);
        let snapshot = snapshot();
        set_on(false);
        assert_eq!(snapshot.fires(Guard::G1aTreeEntryOutOfRange), 2);
        let row = &snapshot.guards[0];
        assert_eq!(row.recoverable, 1);
        assert_eq!(
            row.by_mutation,
            vec![
                (Mutation::TiePinReduction, 1),
                (Mutation::RipupRemoveItems, 1)
            ]
        );
    }
}
