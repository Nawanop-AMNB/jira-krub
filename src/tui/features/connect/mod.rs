//! Connect screen: site, email, token, test, save. First run, 401, or from Settings.

mod model;
mod update;
mod view;

pub use model::{Action, Model};
pub use update::{keys, on_tested, update};
pub use view::view;
