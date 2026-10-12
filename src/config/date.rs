use std::time::SystemTime;

use jiff::Timestamp;
use jiff::tz::TimeZone;

/// Local time in strftime `format`, e.g. `06 Aug 2026 02:20 CEST`.
/// Empty when it can't be shown.
pub fn format_time(t: SystemTime, tz: &TimeZone, format: &str) -> String {
    Timestamp::try_from(t)
        .ok()
        .and_then(|ts| jiff::fmt::strtime::format(format, &ts.to_zoned(tz.clone())).ok())
        .unwrap_or_default()
}

/// An error naming `date_format` when `format` can't format a time.
pub fn check_date_format(format: &str) -> anyhow::Result<()> {
    let zoned = Timestamp::UNIX_EPOCH.to_zoned(TimeZone::UTC);
    jiff::fmt::strtime::format(format, &zoned)
        .map(drop)
        .map_err(|e| anyhow::anyhow!("bad `date_format` {format:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn formats_absolute_local_time() {
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_786_000_000);
        let utc = TimeZone::UTC;
        assert_eq!(
            format_time(t, &utc, crate::config::DATE_FORMAT),
            "06 Aug 2026 07:06 UTC"
        );
        let berlin = TimeZone::get("Europe/Berlin").unwrap();
        assert_eq!(
            format_time(t, &berlin, crate::config::DATE_FORMAT),
            "06 Aug 2026 09:06 CEST"
        );
    }

    #[test]
    fn date_formats_are_checked() {
        assert!(check_date_format("%Y-%m-%d").is_ok());
        let err = check_date_format("%Y %").unwrap_err().to_string();
        assert!(
            err.contains("`date_format`") && err.contains("%Y %"),
            "{err}"
        );
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_786_000_000);
        assert_eq!(format_time(t, &TimeZone::UTC, "%Y-%m-%d"), "2026-08-06");
    }
}
