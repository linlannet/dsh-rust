//! 时钟工具。

use std::time::{SystemTime, UNIX_EPOCH};

use dsh_domain::Tool;
use dsh_types::{Error, JsonSchema, JsonValue, ToolDescriptor};

/// 返回当前 UTC 时间的 RFC3339 文本。
#[derive(Debug, Default)]
pub struct ClockTool;

impl Tool for ClockTool {
    fn descriptor(&self) -> ToolDescriptor {
        ToolDescriptor::new(
            "clock",
            "Return the current UTC time in RFC3339 format.",
            JsonSchema::from_object(serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }))
            .expect("valid object schema"),
        )
    }

    fn call(&self, _arguments: JsonValue) -> Result<JsonValue, Error> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
            Error::new(
                dsh_types::ErrorCode::Internal,
                "system clock is before epoch",
            )
        })?;
        Ok(serde_json::json!({ "now": rfc3339(now.as_secs()) }))
    }
}

/// 将 Unix 秒转换为 `YYYY-MM-DDTHH:MM:SSZ`。
pub fn rfc3339(unix_seconds: u64) -> String {
    let days = (unix_seconds / 86_400) as i64;
    let seconds_of_day = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_epoch_and_known_instant() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
    }
}
