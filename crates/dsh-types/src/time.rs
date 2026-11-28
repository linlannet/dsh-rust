//! UTC 时间戳格式化。
//!
//! 事件与投影都需要 `timestamp`，而契约只要求 ISO-8601 秒级 UTC，
//! 因此这里用纯标准库实现，避免为格式化引入时间库依赖。

use std::time::{SystemTime, UNIX_EPOCH};

/// 当前 UTC 时间，ISO-8601 秒级精度。
pub fn now_rfc3339() -> String {
    format_rfc3339(SystemTime::now())
}

/// 将 `SystemTime` 格式化为 `YYYY-MM-DDTHH:MM:SSZ`。
pub fn format_rfc3339(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    let days = (seconds / 86_400) as i64;
    let remainder = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remainder / 3_600,
        (remainder % 3_600) / 60,
        remainder % 60
    )
}

/// 由 Unix 天数换算公历日期（Howard Hinnant 的 civil_from_days 算法）。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u32;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn formats_known_timestamp() {
        let epoch = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(format_rfc3339(epoch), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn now_is_iso8601_utc() {
        let now = now_rfc3339();
        assert_eq!(now.len(), 20, "秒级 ISO-8601 固定 20 字符: {now}");
        assert!(now.ends_with('Z'));
        assert_eq!(&now[4..5], "-");
        assert_eq!(&now[10..11], "T");
    }
}
