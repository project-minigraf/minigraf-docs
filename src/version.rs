//! Versions and version time.
//!
//! A release `vMAJOR.MINOR.PATCH` maps to a point on Minigraf's valid-time axis:
//! `(2000 + MAJOR)-01-01T00:00:00Z` plus MINOR days plus PATCH seconds. The mapping follows
//! semver order, so a v2.x patch released after v3.0.0 still sorts before it.

use anyhow::{Context, Result, bail};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn parse(s: &str) -> Result<Self> {
        let body = s
            .strip_prefix('v')
            .with_context(|| format!("version `{s}` must start with `v`"))?;
        let parts: Vec<&str> = body.split('.').collect();
        if parts.len() != 3 {
            bail!("version `{s}` must be vMAJOR.MINOR.PATCH");
        }
        let num = |p: &str| -> Result<u32> {
            p.parse()
                .with_context(|| format!("version `{s}`: `{p}` is not a number"))
        };
        let v = Version {
            major: num(parts[0])?,
            minor: num(parts[1])?,
            patch: num(parts[2])?,
        };
        if v.minor > 300 || v.patch > 86_399 || v.major > 7_000 {
            bail!("version `{s}` is outside the version-time range");
        }
        Ok(v)
    }

    /// Version time in Unix milliseconds.
    pub fn millis(self) -> i64 {
        let days = days_from_civil(2000 + self.major as i64, 1, 1) + self.minor as i64;
        (days * 86_400 + self.patch as i64) * 1000
    }

    /// Version time as an ISO 8601 UTC string, as Minigraf's `:valid-at` takes it.
    pub fn time(self) -> String {
        iso_from_millis(self.millis())
    }

    /// The release line, e.g. `2.0` for v2.0.4.
    pub fn line(self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A half-open version range `[from, to)`. `None` means unbounded on that side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    pub from: Option<Version>,
    pub to: Option<Version>,
}

impl Window {
    pub const ALL: Window = Window {
        from: None,
        to: None,
    };

    pub fn intersect(self, other: Window) -> Window {
        let from = match (self.from, other.from) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let to = match (self.to, other.to) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        Window { from, to }
    }

    #[cfg(test)]
    pub fn contains(self, v: Version) -> bool {
        self.from.is_none_or(|f| f <= v) && self.to.is_none_or(|t| v < t)
    }

    pub fn is_empty(self) -> bool {
        matches!((self.from, self.to), (Some(f), Some(t)) if f >= t)
    }
}

// Howard Hinnant's civil-date algorithms.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn iso_from_millis(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        (s / 60) % 60,
        s % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_time_follows_semver() {
        assert_eq!(
            Version::parse("v2.0.4").unwrap().time(),
            "2002-01-01T00:00:04Z"
        );
        assert_eq!(
            Version::parse("v3.0.0").unwrap().time(),
            "2003-01-01T00:00:00Z"
        );
        assert_eq!(
            Version::parse("v0.25.0").unwrap().time(),
            "2000-01-26T00:00:00Z"
        );
        assert_eq!(
            Version::parse("v1.40.1").unwrap().time(),
            "2001-02-10T00:00:01Z"
        );
        let a = Version::parse("v2.0.9").unwrap();
        let b = Version::parse("v3.0.0").unwrap();
        assert!(a.millis() < b.millis());
    }

    #[test]
    fn rejects_bad_versions() {
        assert!(Version::parse("2.0.0").is_err());
        assert!(Version::parse("v2.0").is_err());
        assert!(Version::parse("v2.x.0").is_err());
    }

    #[test]
    fn windows() {
        let v = |s| Version::parse(s).unwrap();
        let w = Window {
            from: Some(v("v2.0.3")),
            to: Some(v("v3.0.0")),
        };
        assert!(w.contains(v("v2.0.3")));
        assert!(w.contains(v("v2.0.9")));
        assert!(!w.contains(v("v2.0.2")));
        assert!(!w.contains(v("v3.0.0")));
        let since3 = Window {
            from: Some(v("v3.0.0")),
            to: None,
        };
        assert!(w.intersect(since3).is_empty());
        assert_eq!(Window::ALL.intersect(w), w);
    }
}
