#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rectangle2DFloat {
        pub x: f32,
        pub y: f32,
        pub width: f32,
        pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsBoard {
        pub bounding_box: Option<Rectangle2DFloat>,
        pub size: Option<Rectangle2DFloat>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsLayers {
        pub total_count: Option<i32>,
        pub signal_count: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsItems {
        pub total_count: Option<i32>,
        pub trace_count: Option<i32>,
        pub via_count: Option<i32>,
        pub conduction_area_count: Option<i32>,
                        pub drill_item_count: Option<i32>,
        pub pin_count: Option<i32>,
        pub component_outline_count: Option<i32>,
        pub other_count: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsComponents {
        pub total_count: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsPads {
        pub total_count: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsNets {
            pub total_count: Option<i32>,
        pub class_count: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsConnections {
        pub maximum_count: Option<i32>,
        pub incomplete_count: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsTraces {
        pub total_count: Option<i32>,
        pub total_segment_count: Option<i32>,
        pub total_length: Option<f32>,
                    pub total_length_mm: Option<f32>,
                pub total_weighted_length: Option<f32>,
        pub average_length: Option<f32>,
        pub total_vertical_length: Option<f32>,
        pub total_horizontal_length: Option<f32>,
        pub total_angled_length: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsBends {
        pub total_count: Option<i32>,
        pub ninety_degree_count: Option<i32>,
        pub forty_five_degree_count: Option<i32>,
        pub other_angle_count: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatisticsVias {
        pub total_count: Option<i32>,
        pub through_hole_count: Option<i32>,
        pub blind_count: Option<i32>,
        pub buried_count: Option<i32>,
}
