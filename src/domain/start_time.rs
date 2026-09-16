use anyhow::{Result, bail};
use std::fmt;

/// Minutes since midnight, always on a whole minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StartTime(u16);

impl StartTime {
    pub const NINE: StartTime = StartTime(9 * 60);

    pub fn from_hm(h: u16, m: u16) -> Result<Self> {
        if h > 23 || m > 59 {
            bail!("time out of range");
        }
        Ok(Self(h * 60 + m))
    }

    /// Accepts `HH:MM`, `H:MM`, `HHMM`, `HMM`, `HH`, `H`.
    pub fn parse(input: &str) -> Result<Self> {
        let s = input.trim();
        if s.is_empty() {
            bail!("empty time");
        }
        let (h, m) = if let Some((h, m)) = s.split_once(':') {
            (h, m)
        } else if s.len() <= 2 {
            (s, "0")
        } else {
            s.split_at(s.len() - 2)
        };
        let h: u16 = h.parse().map_err(|_| anyhow::anyhow!("bad hour '{h}'"))?;
        let m: u16 = m.parse().map_err(|_| anyhow::anyhow!("bad minute '{m}'"))?;
        Self::from_hm(h, m)
    }

    pub fn hour(self) -> u16 {
        self.0 / 60
    }
    pub fn minute(self) -> u16 {
        self.0 % 60
    }

    /// Step by `delta` minutes, clamped to the day.
    pub fn stepped(self, delta: i32) -> Self {
        let v = (self.0 as i32 + delta).clamp(0, 23 * 60 + 59);
        Self(v as u16)
    }

    pub fn plus_seconds(self, secs: u64) -> Self {
        self.stepped((secs / 60) as i32)
    }
}

impl fmt::Display for StartTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour(), self.minute())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_forms() {
        assert_eq!(StartTime::parse("09:00").unwrap(), StartTime::NINE);
        assert_eq!(StartTime::parse("9:00").unwrap(), StartTime::NINE);
        assert_eq!(StartTime::parse("0900").unwrap(), StartTime::NINE);
        assert_eq!(StartTime::parse("900").unwrap(), StartTime::NINE);
        assert_eq!(StartTime::parse("9").unwrap(), StartTime::NINE);
        assert_eq!(StartTime::parse("1330").unwrap().to_string(), "13:30");
        assert!(StartTime::parse("25:00").is_err());
        assert!(StartTime::parse("12:60").is_err());
        assert!(StartTime::parse("ab").is_err());
    }
    #[test]
    fn steps_and_clamps() {
        assert_eq!(StartTime::NINE.stepped(15).to_string(), "09:15");
        assert_eq!(StartTime::NINE.stepped(-60).to_string(), "08:00");
        assert_eq!(StartTime::from_hm(0, 5).unwrap().stepped(-15).to_string(), "00:00");
        assert_eq!(StartTime::from_hm(23, 50).unwrap().stepped(15).to_string(), "23:59");
    }
}
