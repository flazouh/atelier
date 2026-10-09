//! One message as the Messages screen draws it: the author's letter, name and age, the text in the markdown style of the agent
//! view, the file chips, the reaction chips, and the link to the thread. It draws what a [`Line`](super::map::Line) says and
//! knows no provider; the pane decides what a provider may show (the thread link, for one) and passes it in.
mod helpers;

pub use helpers::{OpenThread, draw_line};
