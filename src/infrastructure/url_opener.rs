//! Handing a URL to the desktop. The only place in the crate allowed to spawn
//! a process.

use crate::application::ports::UrlOpener;
use anyhow::{Result, anyhow};
use std::process::Command;

/// The program that opens a URL on this platform. A free function so the
/// choice is testable without spawning anything.
pub fn opener_program() -> &'static str {
    if cfg!(target_os = "macos") { "open" } else { "xdg-open" }
}

pub struct SystemUrlOpener;

impl UrlOpener for SystemUrlOpener {
    fn open(&self, url: &str) -> Result<()> {
        // Fire and forget: the browser outlives us, so never wait on it.
        Command::new(opener_program()).arg(url).spawn().map(|_| ()).map_err(|e| anyhow!("{e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opener_program_matches_platform() {
        if cfg!(target_os = "macos") {
            assert_eq!(opener_program(), "open");
        } else if cfg!(target_os = "linux") {
            assert_eq!(opener_program(), "xdg-open");
        }
    }
}
