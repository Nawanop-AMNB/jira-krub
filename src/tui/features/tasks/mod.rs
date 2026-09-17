//! My tasks tab: the synced "work for me" issues, grouped by Jira status.

mod model;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, update};
pub use view::view;
/// What `Model::selected` indexes; the view and update use it internally.
#[cfg(test)]
pub use model::visible_issues;
#[cfg(test)]
mod model_tests;
