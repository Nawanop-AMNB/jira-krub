//! Settings screen: Global tab (every year) and Year tab (per year).
//! `Tab` switches tabs; `↑`/`↓` walk fields; `Enter` activates.

mod model;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, update};
pub use view::view;
#[cfg(test)]
mod model_tests;
