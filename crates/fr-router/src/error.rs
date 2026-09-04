use thiserror::Error;

#[derive(Debug, Error)]
pub enum RouterError {
        #[error(transparent)]
    Board(#[from] fr_board::BoardError),

                #[error("a ported geometry operation panicked: {0}")]
    Panicked(String),

                                                        #[error("a ported geometry operation panicked after committing {n} room(s): {message}", n = rooms.len())]
    PanickedWithRooms {
                message: String,
                        rooms: Vec<fr_board::RoomId>,
    },

                                #[error("cannot start autorouter: all layers are disabled")]
    NoRoutableLayer,

            #[error("the routing run was stopped")]
    Stopped,

            #[error(transparent)]
    Timespan(#[from] crate::pipeline::TimespanError),
}
