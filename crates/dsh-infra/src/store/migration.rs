use dsh_types::{Error, now_rfc3339};
use rusqlite::Connection;

use super::map_sqlite_error;

/// 基线迁移脚本。与 `contracts/v1/persistence/migrations/0001-baseline.md` 配套，
/// 代码不内联 DDL（见 spec §5.7）。
const BASELINE_SQL: &str =
    include_str!("../../../../contracts/v1/persistence/migrations/0001-baseline.sql");

/// 迁移版本号，写入 `schema_migration` 表。
const BASELINE_VERSION: &str = "0001-baseline";

/// 执行基线迁移；脚本本身幂等，重复调用安全。
pub(crate) fn apply(connection: &mut Connection) -> Result<(), Error> {
    let transaction = connection.transaction().map_err(map_sqlite_error)?;
    transaction
        .execute_batch(BASELINE_SQL)
        .map_err(map_sqlite_error)?;
    transaction
        .execute(
            "INSERT OR IGNORE INTO schema_migration (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![BASELINE_VERSION, now_rfc3339()],
        )
        .map_err(map_sqlite_error)?;
    transaction.commit().map_err(map_sqlite_error)
}
