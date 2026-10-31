//! 操作日志：把「动过文件」的动作记一笔，事后能查。
//!
//! 和 `usage` 落在同一个 `cache.db`，但那边是**聚合计数**（同一款字体看十次只加一列），
//! 这边是**事件流水**（一条一行）。理由很简单：统计要回答「哪款没用过」，
//! 日志要回答「周三下午我到底删了什么」，聚合对后者毫无意义。
//!
//! 表只留最近 [`KEEP`] 条，超了裁掉最老的。日志是给人翻的，不是审计系统，
//! 没必要为几年前的安装记录付存储代价。

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 最多留多少条。
pub const KEEP: i64 = 2000;

pub const INSTALL: &str = "install";
pub const UNINSTALL: &str = "uninstall";
pub const ACTIVATE: &str = "activate";
pub const DEACTIVATE: &str = "deactivate";
pub const ORGANIZE: &str = "organize";
pub const RESTORE: &str = "restore";
pub const TRASH_DELETE: &str = "trash-delete";
pub const TRASH_EMPTY: &str = "trash-empty";
pub const USAGE_CLEAR: &str = "usage-clear";
pub const BUNDLE_OUT: &str = "bundle-out";
pub const BUNDLE_IN: &str = "bundle-in";
/// 上次异常退出留下的临时启用，启动时被回滚掉了。
pub const SELF_HEAL: &str = "self-heal";

fn known(kind: &str) -> bool {
    matches!(
        kind,
        INSTALL
            | UNINSTALL
            | ACTIVATE
            | DEACTIVATE
            | ORGANIZE
            | RESTORE
            | TRASH_DELETE
            | TRASH_EMPTY
            | USAGE_CLEAR
            | BUNDLE_OUT
            | BUNDLE_IN
            | SELF_HEAL
    )
}

static CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

fn schema(conn: &Connection) {
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS audit (
             id     INTEGER PRIMARY KEY AUTOINCREMENT,
             at     INTEGER NOT NULL,
             kind   TEXT NOT NULL,
             count  INTEGER NOT NULL DEFAULT 1,
             detail TEXT NOT NULL DEFAULT '{}'
         );
         CREATE INDEX IF NOT EXISTS idx_audit_at ON audit(at);",
    );
}

fn conn() -> Option<&'static Mutex<Connection>> {
    if let Some(existing) = CONN.get() {
        return Some(existing);
    }
    let path = crate::db::cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let opened = Connection::open(&path).ok()?;
    let _ = opened.busy_timeout(Duration::from_secs(5));
    let _ = opened.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;");
    schema(&opened);
    let _ = CONN.set(Mutex::new(opened));
    CONN.get()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 一条日志。`detail` 是给界面拼句子用的结构化数据，后端不写中文文案 ——
/// 这样切换界面语言时，历史记录也跟着变。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: i64,
    pub at: u64,
    pub kind: String,
    /// 这次动作涉及几个（文件 / 字体）。单条动作为 1。
    pub count: u32,
    pub detail: Value,
}

fn insert_into(conn: &Connection, kind: &str, count: usize, detail: &Value, at: u64) -> usize {
    if !known(kind) {
        return 0;
    }
    let text = serde_json::to_string(detail).unwrap_or_else(|_| "{}".to_string());
    let ok = conn
        .execute(
            "INSERT INTO audit (at, kind, count, detail) VALUES (?1, ?2, ?3, ?4)",
            params![at as i64, kind, count.max(1) as i64, text],
        )
        .is_ok();
    if ok {
        trim(conn);
    }
    usize::from(ok)
}

/// 裁掉超出上限的老记录。挂在写入之后，成本一条 SQL，不需要定时任务。
fn trim(conn: &Connection) {
    let _ = conn.execute(
        "DELETE FROM audit WHERE id <= (SELECT MAX(id) FROM audit) - ?1",
        params![KEEP],
    );
}

fn all_from(conn: &Connection, limit: usize) -> Vec<AuditEntry> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, at, kind, count, detail FROM audit ORDER BY id DESC LIMIT ?1",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(params![limit.max(1) as i64], |row| {
        let text: String = row.get(4)?;
        Ok(AuditEntry {
            id: row.get(0)?,
            at: row.get::<_, i64>(1)? as u64,
            kind: row.get(2)?,
            count: row.get::<_, i64>(3)? as u32,
            detail: serde_json::from_str(&text).unwrap_or_else(|_| json!({})),
        })
    }) else {
        return Vec::new();
    };
    rows.filter_map(|r| r.ok()).collect()
}

/// 记一笔。写不进去就算了 —— 日志失败不该让正常操作跟着失败。
pub fn record(kind: &str, count: usize, detail: Value) -> usize {
    let Some(lock) = conn() else { return 0 };
    let Ok(conn) = lock.lock() else { return 0 };
    insert_into(&conn, kind, count, &detail, now_secs())
}

/// 最近 `limit` 条，新的在前。
pub fn all(limit: usize) -> Vec<AuditEntry> {
    let Some(lock) = conn() else {
        return Vec::new();
    };
    let Ok(conn) = lock.lock() else {
        return Vec::new();
    };
    all_from(&conn, limit)
}

pub fn clear() -> Result<usize, String> {
    let Some(lock) = conn() else {
        return Err("日志库打不开".into());
    };
    let conn = lock.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM audit", [])
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema(&conn);
        conn
    }

    #[test]
    fn newest_comes_first() {
        let conn = mem();
        insert_into(&conn, INSTALL, 3, &json!({"mode": "copy"}), 100);
        insert_into(&conn, UNINSTALL, 1, &json!({"family": "Inter"}), 200);
        let rows = all_from(&conn, 10);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, UNINSTALL);
        assert_eq!(rows[0].at, 200);
        assert_eq!(rows[1].count, 3);
        assert_eq!(rows[1].detail["mode"], "copy");
    }

    #[test]
    fn unknown_kind_is_dropped() {
        let conn = mem();
        assert_eq!(insert_into(&conn, "sneeze", 1, &json!({}), 1), 0);
        assert!(all_from(&conn, 10).is_empty());
    }

    #[test]
    fn count_is_at_least_one() {
        let conn = mem();
        insert_into(&conn, TRASH_EMPTY, 0, &json!({}), 5);
        assert_eq!(all_from(&conn, 10)[0].count, 1);
    }

    #[test]
    fn only_the_newest_survive_trimming() {
        let conn = mem();
        for i in 0..(KEEP + 12) {
            insert_into(&conn, INSTALL, 1, &json!({ "i": i }), i as u64);
        }
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, KEEP);
        let rows = all_from(&conn, 1);
        assert_eq!(rows[0].detail["i"], KEEP + 11);
    }

    #[test]
    fn limit_caps_the_result() {
        let conn = mem();
        for i in 0..30 {
            insert_into(&conn, ACTIVATE, 1, &json!({}), i as u64);
        }
        assert_eq!(all_from(&conn, 8).len(), 8);
    }

    #[test]
    fn broken_detail_falls_back_to_an_empty_object() {
        let conn = mem();
        conn.execute(
            "INSERT INTO audit (at, kind, count, detail) VALUES (1, 'install', 1, 'not json')",
            [],
        )
        .unwrap();
        assert_eq!(all_from(&conn, 1)[0].detail, json!({}));
    }
}
