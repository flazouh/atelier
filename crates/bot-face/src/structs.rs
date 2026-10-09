//! The structs of the player: the data as written, the model built from it, and what a frame holds.

mod affine;
mod bot_def;
mod bot_model;
mod bot_runtime;
mod face_data;
mod face_set;
mod frame;
mod habit_def;
mod palette;
mod part_def;
mod part_model;
mod pose;
mod shape;
mod state_def;
mod stroke_spec;

pub use affine::Affine;
pub use bot_def::BotDef;
pub use bot_model::BotModel;
pub use bot_runtime::BotRuntime;
pub use face_data::FaceData;
pub use face_set::FaceSet;
pub use frame::Frame;
pub use habit_def::HabitDef;
pub use palette::Palette;
pub use part_def::PartDef;
pub use part_model::PartModel;
pub use pose::Pose;
pub use shape::Shape;
pub use state_def::StateDef;
pub use stroke_spec::StrokeSpec;
