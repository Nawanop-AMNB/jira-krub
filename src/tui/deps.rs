use crate::application::{ConfigStore, JiraGatewayFactory, StateStore, UrlOpener};
use std::sync::Arc;

/// Everything the UI needs from the outside world, injected at startup.
pub struct Deps {
    pub config_store: Arc<dyn ConfigStore>,
    pub state_store: Arc<dyn StateStore>,
    pub gateway_factory: JiraGatewayFactory,
    pub opener: Arc<dyn UrlOpener>,
}
