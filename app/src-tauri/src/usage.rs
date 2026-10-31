//! 使用统计：打开详情页（预览）、安装、被项目引用，各记一笔。
//!
//! 落在 `cache.db` 里的一张独立表，和 `file_cache` / `file_hash` 互不相干 ——
//! 重扫、清缓存都不会把这几个计数抹掉（只有「清空统计」会）。
//!
//! 表结构按 (family, kind) 聚合，不写事件流水：一个人一天点开同一款字体十次，
//! 只会让 `count` 加十，不会让表变大。够用来回答「哪些从来没用过、哪些最近翻过」。

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 打开详情页 = 认真看过一眼。
pub const VIEW: &str = "view";
/// 装进系统 / 从库里装载。
pub const INSTALL: &str = "install";
/// 设计稿或源码里出现了这款字体。
pub const REFERENCE: &str = "reference";

fn known(kind: &str) -> bool {
    matches!(kind, VIEW | INSTALL | REFERENCE)
}

static CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

fn schema(conn: &Connection) {
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS usage (
             family   TEXT NOT NULL,
             kind     TEXT NOT NULL,
             count    INTEGER NOT NULL DEFAULT 0,
             first_at INTEGER NOT NULL DEFAULT 0,
             last_at  INTEGER NOT NULL DEFAULT 0,
             PRIMARY KEY (family, kind)
         );
         CREATE INDEX IF NOT EXISTS idx_usage_kind ON usage(kind);",
    );
}

/// 共享连接。第一次调用时建表；之后所有事件都写这一条连接。
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

/// 一款字体的全部计数，一次给前端。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UsageRow {
    pub family: String,
    pub views: u32,
    pub last_view: u64,
    pub installs: u32,
    pub last_install: u64,
    pub refs: u32,
    pub last_ref: u64,
    /// 第一次被记录的时间（任何一类事件）。
    pub first_seen: u64,
}

fn touch_into(conn: &Connection, families: &[String], kind: &str, at: u64) -> usize {
    if !known(kind) {
        return 0;
    }
    let Ok(mut stmt) = conn.prepare(
        "INSERT INTO usage (family, kind, count, first_at, last_at) VALUES (?1, ?2, 1, ?3, ?3)
         ON CONFLICT(family, kind) DO UPDATE SET count = count + 1, last_at = ?3",
    ) else {
        return 0;
    };
    let mut done = 0;
    for family in families {
        let family = family.trim();
        if family.is_empty() {
            continue;
        }
        if stmt.execute(params![family, kind, at as i64]).is_ok() {
            done += 1;
        }
    }
    done
}

fn all_from(conn: &Connection) -> Vec<UsageRow> {
    let Ok(mut stmt) = conn.prepare("SELECT family, kind, count, first_at, last_at FROM usage")
    else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)? as u32,
            row.get::<_, i64>(3)? as u64,
            row.get::<_, i64>(4)? as u64,
        ))
    }) else {
        return Vec::new();
    };
    let mut out: BTreeMap<String, UsageRow> = BTreeMap::new();
    for (family, kind, count, first_at, last_at) in rows.filter_map(|r| r.ok()) {
        let row = out.entry(family.clone()).or_insert_with(|| UsageRow {
            family,
            ..Default::default()
        });
        if first_at > 0 && (row.first_seen == 0 || first_at < row.first_seen) {
            row.first_seen = first_at;
        }
        match kind.as_str() {
            VIEW => {
                row.views = count;
                row.last_view = last_at;
            }
            INSTALL => {
                row.installs = count;
                row.last_install = last_at;
            }
            REFERENCE => {
                row.refs = count;
                row.last_ref = last_at;
            }
            _ => {}
        }
    }
    out.into_values().collect()
}

/// 记一笔。`families` 可以一次给一批（批量安装、引用分析）。
pub fn touch(families: &[String], kind: &str) -> usize {
    if !known(kind) || families.is_empty() {
        return 0;
    }
    let Some(lock) = conn() else { return 0 };
    let Ok(conn) = lock.lock() else { return 0 };
    touch_into(&conn, families, kind, now_secs())
}

pub fn all() -> Vec<UsageRow> {
    let Some(lock) = conn() else {
        return Vec::new();
    };
    let Ok(conn) = lock.lock() else {
        return Vec::new();
    };
    all_from(&conn)
}

pub fn clear() -> Result<(), String> {
    let Some(lock) = conn() else {
        return Err("统计库打不开".into());
    };
    let conn = lock.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM usage", [])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 某款字体从库里去掉了，顺手把它的记录也收走（避免统计页里挂着一堆幽灵）。
pub fn forget(families: &[String]) -> usize {
    let Some(lock) = conn() else { return 0 };
    let Ok(conn) = lock.lock() else { return 0 };
    let Ok(mut stmt) = conn.prepare("DELETE FROM usage WHERE family = ?1") else {
        return 0;
    };
    families
        .iter()
        .filter(|f| stmt.execute(params![f]).is_ok())
        .count()
}

fn merge_into(conn: &Connection, rows: &[UsageRow], at: u64) -> usize {
    let Ok(mut stmt) = conn.prepare(
        "INSERT INTO usage (family, kind, count, first_at, last_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(family, kind) DO UPDATE SET
             count = MAX(usage.count, excluded.count),
             last_at = MAX(usage.last_at, excluded.last_at),
             first_at = CASE
                 WHEN usage.first_at = 0 THEN excluded.first_at
                 WHEN excluded.first_at = 0 THEN usage.first_at
                 ELSE MIN(usage.first_at, excluded.first_at)
             END",
    ) else {
        return 0;
    };
    let mut done = 0;
    let mut bump = |family: &str, kind: &str, count: u32, first: u64, last: u64| {
        if family.trim().is_empty() || !known(kind) || count == 0 {
            return;
        }
        let first = if first == 0 { at } else { first };
        let last = last.max(first);
        if stmt
            .execute(params![family, kind, count as i64, first as i64, last as i64])
            .is_ok()
        {
            done += 1;
        }
    };
    for row in rows {
        if row.family.trim().is_empty() {
            continue;
        }
        bump(&row.family, VIEW, row.views, row.first_seen, row.last_view);
        bump(&row.family, INSTALL, row.installs, row.first_seen, row.last_install);
        bump(&row.family, REFERENCE, row.refs, row.first_seen, row.last_ref);
    }
    done
}

/// 从迁移包里把计数并进来。
///
/// 次数取两边较大的那份、时间取极值 —— 换机恢复时本机几乎是空的，等于原样搬过来；
/// 同一个包重复导入也不会把次数翻倍。
pub fn merge_rows(rows: &[UsageRow]) -> usize {
    if rows.is_empty() {
        return 0;
    }
    let Some(lock) = conn() else { return 0 };
    let Ok(conn) = lock.lock() else { return 0 };
    merge_into(&conn, rows, now_secs())
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
    fn touching_the_same_family_accumulates() {
        let conn = mem();
        let fams = vec!["思源宋体".to_string()];
        allow_first(&conn, &fams, VIEW, 100);
        allow_first(&conn, &fams, VIEW, 200);
        let rows = all_from(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].views, 2);
        assert_eq!(rows[0].last_view, 200);
        assert_eq!(rows[0].first_seen, 100);
    }

    fn allow_first(conn: &Connection, fams: &[String], kind: &str, at: u64) {
        assert_eq!(touch_into(conn, fams, kind, at), fams.len());
    }

    #[test]
    fn kinds_are_kept_apart() {
        let conn = mem();
        let fams = vec!["Inter".to_string()];
        touch_into(&conn, &fams, VIEW, 10);
        touch_into(&conn, &fams, INSTALL, 20);
        touch_into(&conn, &fams, REFERENCE, 30);
        let rows = all_from(&conn);
        let row = &rows[0];
        assert_eq!((row.views, row.installs, row.refs), (1, 1, 1));
        assert_eq!(row.last_view, 10);
        assert_eq!(row.last_install, 20);
        assert_eq!(row.last_ref, 30);
        assert_eq!(row.first_seen, 10);
    }

    #[test]
    fn unknown_kind_is_ignored() {
        let conn = mem();
        let fams = vec!["Inter".to_string()];
        assert_eq!(touch_into(&conn, &fams, "print", 10), 0);
        assert!(all_from(&conn).is_empty());
    }

    #[test]
    fn blank_family_is_skipped() {
        let conn = mem();
        assert_eq!(touch_into(&conn, &["  ".to_string()], VIEW, 10), 0);
    }

    #[test]
    fn a_batch_counts_once_each() {
        let conn = mem();
        let fams = vec!["A".to_string(), "B".to_string(), "A".to_string()];
        assert_eq!(touch_into(&conn, &fams, INSTALL, 5), 3);
        let rows = all_from(&conn);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].installs, 2);
    }

    #[test]
    fn a_default_row_is_all_zeroes() {
        let row = UsageRow::default();
        assert_eq!((row.views, row.installs, row.refs, row.first_seen), (0, 0, 0, 0));
        assert!(row.family.is_empty());
    }
}
