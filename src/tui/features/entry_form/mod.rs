//! Popup form for a new entry: issue, date, start, duration, description.

mod model;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, update};
pub use view::view;
