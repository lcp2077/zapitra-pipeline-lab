//! ZAPITRA: bounded state-space exploration for the BLAKE3-Servil Pipeline design.

pub mod explorer;
pub mod model;

pub use explorer::{ExploreConfig, ExploreReport, explore, render_json, render_text};
pub use model::{
    EventKind, EventRecord, InputPath, LentGuardState, Limits, MessageRecord, MessageToken,
    ROOM_LEN, RoomState, RoomToken, State, WaitClass,
};
