use anyhow::{Context, Result};
use std::path::PathBuf;

fn xdg(var: &str, fallback: &[&str]) -> Result<PathBuf> {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() => Ok(PathBuf::from(v)),
        _ => {
            let mut p = dirs::home_dir().context("no home dir")?;
            for seg in fallback {
                p.push(seg);
            }
            Ok(p)
        }
    }
}

/// `$XDG_CONFIG_HOME/jira-krub/config.toml` or `~/.config/jira-krub/config.toml`
pub fn config_file() -> Result<PathBuf> {
    Ok(xdg("XDG_CONFIG_HOME", &[".config"])?.join("jira-krub").join("config.toml"))
}

/// `$XDG_DATA_HOME/jira-krub/state.json` or `~/.local/share/jira-krub/state.json`
pub fn state_file() -> Result<PathBuf> {
    Ok(xdg("XDG_DATA_HOME", &[".local", "share"])?.join("jira-krub").join("state.json"))
}

/// Write via temp file + rename so a crash never leaves a half-written file.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
    let dir = path.parent().context("path has no parent")?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}.tmp", path.file_name().and_then(|s| s.to_str()).unwrap_or("file")));
    std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    #[cfg(unix)]
    if let Some(m) = mode {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(m))?;
    }
    std::fs::rename(&tmp, path).with_context(|| format!("rename to {}", path.display()))?;
    Ok(())
}
