#![forbid(unsafe_code)]

//! HEAD is the authority for both the sources and the parity jar: HEAD's `autoroute/**` has been
//! * **`#![forbid(unsafe_code)]`** in this crate root and in every other workspace crate. The only
pub mod arena;
pub mod autoroute;
pub mod board_ext;
pub mod error;
pub mod java_tree_set;
pub mod pipeline;
pub mod score;

pub use arena::{Arena, DoorId, DrillId, IncompleteRoomId, PageId, TargetDoorId};
pub use autoroute::{
    AutorouteAttemptResult, AutorouteAttemptState, AutorouteControl, AutorouteEngine,
    AutorouteSearchTreeExt, CompleteFreeSpaceExpansionRoom, Connection, DestinationDistance,
    DrillPage, DrillPageArray, ExpandableRef, ExpansionDoor, ExpansionDrill, ExpansionRoomStore,
    FoundConnectionInserter, FoundConnectionLocator, FreeSpaceExpansionRoom,
    IncompleteFreeSpaceExpansionRoom, MazeAdjustment, MazeExpansionEngine, MazeListElement,
    MazeQueue, MazeResult, MazeRipupResolver, MazeSearchElement, MazeSearchEngine,
    ObstacleExpansionRoom, ResultItem, RoomRef, ShoveResult, TargetItemExpansionDoor, ViaMask,
    route_connection, route_connection_full,
};
pub use board_ext::{
    CheckDrillResult, DrillItemMover, ForcedPadRouter, ForcedViaInserter, RoutingBoardExt,
    SpringOverOutcome, TraceShover,
};
pub use error::RouterError;
pub use java_tree_set::JavaTreeSet;
pub use pipeline::{
    AutoroutePassRunner, BatchAutorouter, BoardHistory, BoardHistoryEntry, ItemFailureInfo,
    ItemRouteResult, NamedAlgorithmType, NoopProgressSink, PassRecord, PipelineResult,
    ProgressSink, ProgressThrottler, RouterBudget, RouterCounters, RouterStop, RoutingEvent,
    RoutingFailureLog, StopRequestState, TaskState, build_unrouted_report, calculate_airline,
    normalize_router_algorithm, run_pipeline,
};
pub use score::{
    BoardStatistics, BoardStatisticsBends, BoardStatisticsBoard,
    BoardStatisticsClearanceViolations, BoardStatisticsComponents, BoardStatisticsConnections,
    BoardStatisticsFanout, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};

pub use fr_settings::ExpansionCostFactor;

pub mod prelude {
    pub use crate::{
        Arena, AutorouteAttemptResult, AutorouteAttemptState, AutorouteControl, AutorouteEngine,
        AutorouteSearchTreeExt, BatchAutorouter, BoardHistory, BoardHistoryEntry, BoardStatistics,
        BoardStatisticsBends, BoardStatisticsBoard, BoardStatisticsClearanceViolations,
        BoardStatisticsComponents, BoardStatisticsConnections, BoardStatisticsFanout,
        BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets, BoardStatisticsPads,
        BoardStatisticsTraces, BoardStatisticsVias, CompleteFreeSpaceExpansionRoom, Connection,
        DestinationDistance, DoorId, DrillId, DrillItemMover, DrillPage, DrillPageArray,
        ExpandableRef, ExpansionCostFactor, ExpansionDoor, ExpansionDrill, ExpansionRoomStore,
        FoundConnectionInserter, FoundConnectionLocator, FreeSpaceExpansionRoom,
        IncompleteFreeSpaceExpansionRoom, IncompleteRoomId, JavaTreeSet, MazeAdjustment,
        MazeExpansionEngine, MazeListElement, MazeQueue, MazeResult, MazeRipupResolver,
        MazeSearchElement, MazeSearchEngine, NamedAlgorithmType, NoopProgressSink,
        ObstacleExpansionRoom, PageId, PassRecord, PipelineResult, ProgressSink, ProgressThrottler,
        Rectangle2DFloat, ResultItem, RoomRef, RouterBudget, RouterCounters, RouterError,
        RouterStop, RoutingBoardExt, RoutingEvent, RoutingFailureLog, ShoveResult,
        SpringOverOutcome, StopRequestState, TargetDoorId, TargetItemExpansionDoor, TaskState,
        TraceShover, ViaMask, build_unrouted_report, normalize_router_algorithm, route_connection,
        route_connection_full, run_pipeline,
    };
}

// not ported: `BoardHistoryEntry.compareTo` — `autoroute/BoardHistoryEntry.java:29-32`, the shadowed and unreachable top-level twin (quirk #199).

// a javadoc `@see`, an import, or the one `instanceof` in `gui/workspace/progress/GuiRoutingJobWorker.java:305`
