-- 0001 基线：会话、消息、事件、运行
--
-- 约束（对齐 docs/0.2.1/spec.md §4.3 / §5.7）：
--   1. 只建表与索引，不写入业务数据；
--   2. 语句幂等，重复执行不报错；
--   3. 时间列统一 UTC ISO-8601 文本；
--   4. seq 在单会话内严格递增，由 UNIQUE(session_id, seq) 兜底。

PRAGMA foreign_keys = ON;

-- 迁移记录：已应用的脚本版本，避免重复执行
CREATE TABLE IF NOT EXISTS schema_migration (
    version    TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL
);

-- 会话主体。agent_id 允许为空：0.2.1 的领域 Session 尚无该字段，由运行期按需补写。
CREATE TABLE IF NOT EXISTS session (
    id            TEXT PRIMARY KEY,
    agent_id      TEXT,
    workspace_id  TEXT,
    title         TEXT,
    status        TEXT NOT NULL DEFAULT 'active',
    active_run_id TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

-- 消息历史；ordinal 在会话内严格递增
CREATE TABLE IF NOT EXISTS session_message (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT    NOT NULL,
    ordinal    INTEGER NOT NULL,
    message_id TEXT    NOT NULL,
    role       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    created_at TEXT    NOT NULL,
    UNIQUE (session_id, ordinal),
    FOREIGN KEY (session_id) REFERENCES session (id) ON DELETE CASCADE
);

-- 事件日志；只追加，不更新不删除。
-- 不设外键：事件是溯源事实，允许先于会话投影写入（回放不依赖会话行存在）。
CREATE TABLE IF NOT EXISTS session_event (
    event_id     TEXT PRIMARY KEY,
    session_id   TEXT    NOT NULL,
    seq          INTEGER NOT NULL,
    type         TEXT    NOT NULL,
    timestamp    TEXT    NOT NULL,
    payload_json TEXT    NOT NULL,
    UNIQUE (session_id, seq)
);

-- 运行记录。同为只追加事实，不设外键。
CREATE TABLE IF NOT EXISTS run (
    run_id        TEXT PRIMARY KEY,
    session_id    TEXT NOT NULL,
    agent_id      TEXT NOT NULL,
    status        TEXT NOT NULL,
    finish_reason TEXT,
    error_json    TEXT,
    started_at    TEXT NOT NULL,
    ended_at      TEXT
);

-- 回放与续跑的主索引已由上面的 UNIQUE 约束提供，此处只补运行历史查询
CREATE INDEX IF NOT EXISTS idx_run_session_started ON run (session_id, started_at);
