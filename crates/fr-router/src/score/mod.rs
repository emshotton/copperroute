pub mod dtos;
pub mod normalized;
pub mod statistics;

pub use dtos::{
    BoardStatisticsBends, BoardStatisticsBoard, BoardStatisticsComponents,
    BoardStatisticsConnections, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};
pub use statistics::{
    BoardStatistics, BoardStatisticsFanout, java_double_stream_sum, unescape_unicode,
};

pub use fr_drc::{BoardStatisticsClearanceViolations, DrcViolation};
