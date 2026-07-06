//! Human-readable duration parser.
//!
//! Accepts `<integer><suffix>` where suffix ∈ `s` / `m` / `h` / `d`. Chosen
//! shape mirrors the `floating-perms` skill vocabulary — same syntax the user
//! already knows from setting temporary permission grants. Used by
//! `notif listen --idle-timeout` and any future flag that carries a wall-clock
//! interval.
//!
//! No fractional support (`1.5h` rejected). No compound expressions (`1h30m`
//! rejected). Callers who need finer granularity pass seconds (`5400s`).

use std::time::Duration;

/// Parsed duration expressed as a [`Duration`]. See module docs for accepted
/// syntax.
///
/// # Errors
/// Returns [`DurationParseError`] on empty / malformed input, on a missing or
/// unknown suffix, or on integer overflow.
pub fn parse_duration(s: &str) -> Result<Duration, DurationParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(DurationParseError::Empty);
    }
    let (num_str, unit) = match s.chars().last() {
        Some(c) if c.is_ascii_digit() => (s, 's'),
        Some(c) => (&s[..s.len() - c.len_utf8()], c),
        None => return Err(DurationParseError::Empty),
    };
    if num_str.is_empty() {
        return Err(DurationParseError::NoDigits(s.to_string()));
    }
    let n: u64 = num_str
        .parse()
        .map_err(|_| DurationParseError::BadInteger(num_str.to_string()))?;
    let secs_per_unit: u64 = match unit {
        's' => 1,
        'm' => 60,
        'h' => 60 * 60,
        'd' => 24 * 60 * 60,
        other => return Err(DurationParseError::BadSuffix(other)),
    };
    n.checked_mul(secs_per_unit)
        .map(Duration::from_secs)
        .ok_or(DurationParseError::Overflow)
}

/// Ways [`parse_duration`] can reject an input string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationParseError {
    Empty,
    /// Suffix present but no digits before it (`"h"`).
    NoDigits(String),
    /// Digit prefix could not be parsed as `u64`.
    BadInteger(String),
    /// Unknown or missing suffix character.
    BadSuffix(char),
    /// `n * unit` overflowed [`u64`].
    Overflow,
}

impl std::fmt::Display for DurationParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("duration is empty"),
            Self::NoDigits(s) => write!(f, "duration {s:?} has no digits before the suffix"),
            Self::BadInteger(s) => write!(f, "duration {s:?} is not a non-negative integer"),
            Self::BadSuffix(c) => write!(
                f,
                "duration suffix {c:?} not recognized (expected s / m / h / d)",
            ),
            Self::Overflow => f.write_str("duration overflows u64 seconds"),
        }
    }
}

impl std::error::Error for DurationParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_seconds() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn parse_minutes() {
        assert_eq!(parse_duration("15m").unwrap(), Duration::from_secs(15 * 60));
    }

    #[test]
    fn parse_hours() {
        assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(2 * 60 * 60));
    }

    #[test]
    fn parse_days() {
        assert_eq!(parse_duration("1d").unwrap(), Duration::from_secs(24 * 60 * 60));
    }

    #[test]
    fn parse_default_suffix_is_seconds() {
        assert_eq!(parse_duration("45").unwrap(), Duration::from_secs(45));
    }

    #[test]
    fn parse_trims_whitespace() {
        assert_eq!(parse_duration("  24h  ").unwrap(), Duration::from_secs(24 * 60 * 60));
    }

    #[test]
    fn empty_rejected() {
        assert_eq!(parse_duration(""), Err(DurationParseError::Empty));
        assert_eq!(parse_duration("   "), Err(DurationParseError::Empty));
    }

    #[test]
    fn no_digits_rejected() {
        assert!(matches!(
            parse_duration("h"),
            Err(DurationParseError::NoDigits(_)),
        ));
    }

    #[test]
    fn bad_suffix_rejected() {
        assert!(matches!(
            parse_duration("5w"),
            Err(DurationParseError::BadSuffix('w')),
        ));
    }

    #[test]
    fn fractional_rejected() {
        assert!(matches!(
            parse_duration("1.5h"),
            Err(DurationParseError::BadInteger(_)),
        ));
    }

    #[test]
    fn overflow_rejected() {
        // u64::MAX seconds × 60 overflows.
        assert_eq!(
            parse_duration(&format!("{}m", u64::MAX)),
            Err(DurationParseError::Overflow),
        );
    }
}
