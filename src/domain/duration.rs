use anyhow::{Result, bail};

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 8 * HOUR; // Jira default working day
const WEEK: u64 = 5 * DAY; // Jira default working week
const SNAP: u64 = 15 * MINUTE;

/// Parse a Jira-style duration: integer + unit, units `w d h m`, any order,
/// spaces optional. `2h`, `1h30m`, `1h 30m`, `90m`, `1d 2h`.
/// Rejects decimals, bare numbers, and trailing numbers without a unit.
/// Result is snapped to the nearest 15 minutes, minimum 15 minutes.
pub fn parse(input: &str) -> Result<u64> {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        bail!("empty duration");
    }
    let mut total = 0u64;
    let mut num = String::new();
    for c in s.chars() {
        match c {
            '0'..='9' => num.push(c),
            ' ' => {
                if !num.is_empty() {
                    bail!("missing unit after '{num}' (e.g. 1h30m, 90m)");
                }
            }
            'w' | 'd' | 'h' | 'm' => {
                if num.is_empty() {
                    bail!("unit '{c}' without a number (e.g. 1h30m, 90m)");
                }
                let n: u64 = num.parse().map_err(|_| anyhow::anyhow!("bad number '{num}'"))?;
                num.clear();
                let unit = match c {
                    'w' => WEEK,
                    'd' => DAY,
                    'h' => HOUR,
                    _ => MINUTE,
                };
                total = total
                    .checked_add(n.checked_mul(unit).ok_or_else(|| anyhow::anyhow!("too large"))?)
                    .ok_or_else(|| anyhow::anyhow!("too large"))?;
            }
            '.' | ',' => bail!("decimals not allowed (e.g. 1h30m, 90m)"),
            _ => bail!("unexpected '{c}' (e.g. 1h30m, 90m)"),
        }
    }
    if !num.is_empty() {
        bail!("missing unit after '{num}' (e.g. 1h30m, 90m)");
    }
    if total == 0 {
        bail!("duration must be > 0");
    }
    Ok(snap(total))
}

/// Round to nearest 15 minutes, never below 15 minutes.
pub fn snap(seconds: u64) -> u64 {
    let snapped = (seconds + SNAP / 2) / SNAP * SNAP;
    snapped.max(SNAP)
}

/// Jira-style display: 5400 -> "1h 30m", 7200 -> "2h", 1800 -> "30m", 0 -> "".
pub fn format(seconds: u64) -> String {
    if seconds == 0 {
        return String::new();
    }
    let h = seconds / HOUR;
    let m = (seconds % HOUR) / MINUTE;
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_jira_grammar() {
        assert_eq!(parse("2h").unwrap(), 7200);
        assert_eq!(parse("1h30m").unwrap(), 5400);
        assert_eq!(parse("1h 30m").unwrap(), 5400);
        assert_eq!(parse("90m").unwrap(), 5400);
        assert_eq!(parse("30m 1h").unwrap(), 5400);
        assert_eq!(parse("1d").unwrap(), 8 * 3600);
        assert_eq!(parse("1d 2h").unwrap(), 10 * 3600);
        assert_eq!(parse("1w").unwrap(), 40 * 3600);
        assert_eq!(parse(" 2H ").unwrap(), 7200);
    }

    #[test]
    fn rejects_non_jira() {
        assert!(parse("").is_err());
        assert!(parse("2").is_err(), "bare number");
        assert!(parse("1h30").is_err(), "trailing number without unit");
        assert!(parse("1.5h").is_err(), "decimal");
        assert!(parse("h").is_err(), "unit without number");
        assert!(parse("2x").is_err(), "unknown unit");
        assert!(parse("0m").is_err(), "zero");
    }

    #[test]
    fn snaps_to_quarter_hour() {
        assert_eq!(parse("1h20m").unwrap(), 4500, "80m -> 1h15m");
        assert_eq!(parse("1h23m").unwrap(), 5400, "83m -> 1h30m");
        assert_eq!(parse("7m").unwrap(), 900, "min 15m");
        assert_eq!(parse("8m").unwrap(), 900, "rounds up to 15m");
        assert_eq!(parse("22m").unwrap(), 900, "22m -> 15m");
        assert_eq!(parse("23m").unwrap(), 1800, "23m -> 30m");
    }

    #[test]
    fn formats_like_jira() {
        assert_eq!(format(7200), "2h");
        assert_eq!(format(5400), "1h 30m");
        assert_eq!(format(1800), "30m");
        assert_eq!(format(0), "");
    }
}
