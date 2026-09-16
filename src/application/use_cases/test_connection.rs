use crate::application::ports::{GatewayErrorKind, JiraGateway, Me, classify};

#[derive(Debug, Clone)]
pub enum Outcome {
    Ok(Me),
    Unauthorized,
    NotJira,
    Network(String),
    Other(String),
}

pub fn run(gateway: &dyn JiraGateway) -> Outcome {
    match gateway.myself() {
        Ok(me) => Outcome::Ok(me),
        Err(e) => match classify(&e) {
            GatewayErrorKind::Unauthorized => Outcome::Unauthorized,
            GatewayErrorKind::NotFound => Outcome::NotJira,
            GatewayErrorKind::Network => Outcome::Network(e.to_string()),
            GatewayErrorKind::Other => Outcome::Other(e.to_string()),
        },
    }
}
