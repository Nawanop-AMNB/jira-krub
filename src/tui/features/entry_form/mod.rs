//! Popup form for a single entry: issue, date, start, duration, title, detail.

mod model;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, update};
pub use view::view;
