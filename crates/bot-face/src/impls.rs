//! The functions of the player.

mod affine;
mod bot_model;
mod bot_runtime_new;
mod bot_runtime_tick;
mod face_set;
mod habit_pose;
mod mood;
mod mood_pose;
mod paint_bot;
mod palette;
mod path_data;
mod pose;
mod shape_parse;
mod shape_trace;

pub use paint_bot::paint_bot;

#[cfg(test)]
pub(crate) use path_data::parse_path_data as parse_path_data_for_tests;
#[cfg(test)]
pub(crate) use shape_parse::parse_fragment as parse_fragment_for_tests;
#[cfg(test)]
pub(crate) use shape_trace::trace as trace_for_tests;
