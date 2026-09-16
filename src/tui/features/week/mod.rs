//! Main screen: one row per day of the week.

mod model;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, update};
pub use view::view;
