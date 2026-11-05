//! Quick day-off modal on the week summary: add, edit or remove the day off
//! of the selected day without a trip through Settings.

mod model;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, update};
pub use view::view;
