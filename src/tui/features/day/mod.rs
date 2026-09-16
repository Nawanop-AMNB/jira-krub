//! Day view: tickets pane (search + watchlist + mine) → prepare-logwork pane.

mod model;
pub mod rows;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, on_search_done, tick, update};
pub use view::view;
