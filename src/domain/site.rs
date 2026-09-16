use anyhow::{Result, bail};
use std::fmt;

/// Normalised Jira site origin: `https://host[:port]`, no path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteUrl(String);

impl SiteUrl {
    /// Accepts `company.atlassian.net`, `https://jira.co.com/browse/X-1`, etc.
    /// Rejects `http://` and hosts without a dot.
    pub fn parse(input: &str) -> Result<Self> {
        let s = input.trim().to_lowercase();
        if s.is_empty() {
            bail!("site is required");
        }
        let (scheme, rest) = if let Some(r) = s.strip_prefix("https://") {
            ("https", r)
        } else if let Some(r) = s.strip_prefix("http://") {
            if is_loopback(r) {
                ("http", r)
            } else {
                bail!("must be https");
            }
        } else if s.contains("://") {
            bail!("must be https");
        } else {
            ("https", s.as_str())
        };
        let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
        if host_port.is_empty() {
            bail!("host is required");
        }
        let (host, port) = match host_port.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (host_port, None),
        };
        if !host.contains('.') && host != "localhost" {
            bail!("host needs a domain, e.g. company.atlassian.net");
        }
        if !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            || host.starts_with('.')
            || host.ends_with('.')
        {
            bail!("invalid host '{host}'");
        }
        if let Some(p) = port
            && (p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
                bail!("invalid port '{p}'");
            }
        Ok(Self(format!("{scheme}://{host_port}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Plain http is tolerated only for a local dev/fake server.
fn is_loopback(rest: &str) -> bool {
    let host = rest.split(['/', ':', '?', '#']).next().unwrap_or("");
    host == "localhost" || host == "127.0.0.1"
}

impl fmt::Display for SiteUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalises() {
        assert_eq!(SiteUrl::parse("acme.atlassian.net").unwrap().as_str(), "https://acme.atlassian.net");
        assert_eq!(
            SiteUrl::parse("https://acme.atlassian.net/jira/software/projects/X").unwrap().as_str(),
            "https://acme.atlassian.net"
        );
        assert_eq!(SiteUrl::parse("HTTPS://Jira.Co.com/").unwrap().as_str(), "https://jira.co.com");
        assert_eq!(SiteUrl::parse("jira.co.com:8443/x").unwrap().as_str(), "https://jira.co.com:8443");
    }
    #[test]
    fn rejects() {
        assert!(SiteUrl::parse("http://acme.atlassian.net").is_err());
        assert!(SiteUrl::parse("").is_err());
        assert_eq!(SiteUrl::parse("http://localhost:8080/x").unwrap().as_str(), "http://localhost:8080");
        assert!(SiteUrl::parse("http://jira.co.com").is_err());
        assert!(SiteUrl::parse("https://").is_err());
        assert!(SiteUrl::parse("ftp://a.b").is_err());
        assert!(SiteUrl::parse("a.b:xx").is_err());
    }
}
