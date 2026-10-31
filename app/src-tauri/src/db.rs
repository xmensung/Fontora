//! Persistent index for the font library.
//!
//! Two things live here, both keyed by file identity (path + size + mtime):
//!
//! 1. `file_cache` — the parsed metadata of every font file, so a rescan can
//!    skip re-opening files that have not changed since the last run.
//! 2. `file_hash` — SHA-256 of file contents, computed only for files whose
//!    size collides with another file (the only possible duplicates).
//!
//! SQLite is statically linked (`rusqlite/bundled`), so the portable build
//! still ships a single .exe with no external DLLs.

use crate::cjk::CjkStats;
use crate::font_types::FontFace;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

const HASH_CHUNK: usize = 256 * 1024;

/// A background hash pass is already running (guards against rescan storms).
pub static HASHING: AtomicBool = AtomicBool::new(false);

/// Same guard for the character-coverage pass.
pub static CJK_RUNNING: AtomicBool = AtomicBool::new(false);

pub fn cache_path() -> PathBuf {
    crate::store::app_data_dir().join("cache.db")
}

/// Handle to the index. Cheap to clone; clones share one connection.
#[derive(Clone)]
pub struct FontCache {
    conn: Arc<Mutex<Connection>>,
    hits: Arc<AtomicUsize>,
    misses: Arc<AtomicUsize>,
}

/// File identity used as the cache key.
pub fn file_stamp(path: &Path) -> Option<(u64, i64)> {
    let meta = path.metadata().ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some((meta.len(), mtime))
}

/// 导出清单要用的「文件事实」：哈希与字符数都来自索引，不去重新解析字体。
#[derive(Debug, Clone, Default)]
pub struct FileFacts {
    pub hash: Option<String>,
    pub total_chars: Option<u32>,
    pub han: Option<u32>,
}

/// One row of the index: cache entry plus its content hash (when known).
pub struct IndexedFile {
    pub path: String,
    pub size: u64,
    pub mtime: i64,
    pub hash: Option<String>,
    pub faces: Vec<FontFace>,
}

impl FontCache {
    pub fn open() -> Option<FontCache> {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path).ok()?;
        let _ = conn.busy_timeout(Duration::from_secs(5));
        let _ = conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS file_cache (
                 path  TEXT PRIMARY KEY,
                 size  INTEGER NOT NULL,
                 mtime INTEGER NOT NULL,
                 faces TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS file_hash (
                 path  TEXT PRIMARY KEY,
                 size  INTEGER NOT NULL,
                 mtime INTEGER NOT NULL,
                 hash  TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_file_hash_hash ON file_hash(hash);
             CREATE TABLE IF NOT EXISTS file_cjk (
                 path  TEXT PRIMARY KEY,
                 size  INTEGER NOT NULL,
                 mtime INTEGER NOT NULL,
                 stats TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_file_cjk_path ON file_cjk(path);",
        );
        // v2: 族名改为优先取简体中文名，旧缓存里的英文族名必须作废重解析。
        // v3: 字体分类（衬线/无衬线/书写）判定改过，旧缓存里的分类也要重算。
        // v4: FontFace 多了 modified（文件修改时间），旧缓存里没有这个字段。
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or(0);
        if version < 4 {
            let _ = conn.execute_batch("DELETE FROM file_cache; PRAGMA user_version = 4;");
        }
        Some(FontCache {
            conn: Arc::new(Mutex::new(conn)),
            hits: Arc::new(AtomicUsize::new(0)),
            misses: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Cached faces for an unchanged file.
    pub fn get(&self, path: &str, size: u64, mtime: i64) -> Option<Vec<FontFace>> {
        let conn = self.conn.lock().ok()?;
        let json: Option<String> = conn
            .query_row(
                "SELECT faces FROM file_cache WHERE path = ?1 AND size = ?2 AND mtime = ?3",
                params![path, size as i64, mtime],
                |row| row.get(0),
            )
            .optional()
            .ok()?;
        let faces = json.and_then(|s| serde_json::from_str::<Vec<FontFace>>(&s).ok());
        match faces {
            Some(f) if !f.is_empty() => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(f)
            }
            _ => None,
        }
    }

    pub fn put(&self, path: &str, size: u64, mtime: i64, faces: &[FontFace]) {
        if faces.is_empty() {
            return;
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let Ok(json) = serde_json::to_string(faces) else {
            return;
        };
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO file_cache (path, size, mtime, faces) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(path) DO UPDATE SET size = ?2, mtime = ?3, faces = ?4",
                params![path, size as i64, mtime, json],
            );
        }
    }

    pub fn stats(&self) -> (usize, usize) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    /// Forget files that are no longer part of the library.
    pub fn prune(&self, alive: &HashSet<String>) {
        let Ok(conn) = self.conn.lock() else { return };
        let _ = conn.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS seen (path TEXT PRIMARY KEY);
             DELETE FROM seen;",
        );
        if let Ok(mut stmt) = conn.prepare("INSERT OR IGNORE INTO seen (path) VALUES (?1)") {
            let _ = conn.execute_batch("BEGIN");
            for path in alive {
                let _ = stmt.execute(params![path]);
            }
            drop(stmt);
            let _ = conn.execute_batch("COMMIT");
        }
        let _ = conn.execute(
            "DELETE FROM file_cache WHERE path NOT IN (SELECT path FROM seen)",
            [],
        );
        let _ = conn.execute(
            "DELETE FROM file_hash WHERE path NOT IN (SELECT path FROM seen)",
            [],
        );
    }

    /// Paths whose size collides with at least one other file and whose
    /// content hash is missing or stale.
    pub fn hash_candidates(&self, limit: usize) -> Vec<(String, u64, i64)> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT c.path, c.size, c.mtime
               FROM file_cache c
               JOIN (SELECT size FROM file_cache GROUP BY size HAVING COUNT(*) > 1) dup
                 ON dup.size = c.size
               LEFT JOIN file_hash h
                 ON h.path = c.path AND h.size = c.size AND h.mtime = c.mtime
              WHERE h.path IS NULL
              LIMIT ?1",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)?,
            ))
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn put_hash(&self, path: &str, size: u64, mtime: i64, hash: &str) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO file_hash (path, size, mtime, hash) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(path) DO UPDATE SET size = ?2, mtime = ?3, hash = ?4",
                params![path, size as i64, mtime, hash],
            );
        }
    }

    /// Every indexed file with its hash (when known). Used by the duplicate scan.
    pub fn all_files(&self) -> Vec<IndexedFile> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT c.path, c.size, c.mtime, c.faces, h.hash
               FROM file_cache c
               LEFT JOIN file_hash h
                 ON h.path = c.path AND h.size = c.size AND h.mtime = c.mtime",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        });
        let Ok(iter) = rows else { return Vec::new() };
        iter.filter_map(|r| r.ok())
            .filter_map(|(path, size, mtime, faces_json, hash)| {
                let faces: Vec<FontFace> = serde_json::from_str(&faces_json).ok()?;
                Some(IndexedFile { path, size, mtime, hash, faces })
            })
            .collect()
    }

    /// Hash every pending duplicate candidate (bounded by `limit`).
    /// Runs off the UI thread; safe to call on a rescan.
    pub fn hash_pending(&self, limit: usize) -> usize {        if HASHING.swap(true, Ordering::SeqCst) {
            return 0;
        }
        let mut done = 0;
        for (path, size, mtime) in self.hash_candidates(limit) {
            if let Some(hash) = sha256_file(Path::new(&path)) {
                self.put_hash(&path, size, mtime, &hash);
                done += 1;
            } else {
                // Unreadable now: store an empty marker so we do not retry
                // every launch. A changed file gets a new stamp.
                self.put_hash(&path, size, mtime, "");
            }
        }
        HASHING.store(false, Ordering::SeqCst);
        done
    }

    /// 一个文件的哈希与字符数（索引里没有的就留空）。导出清单/字体包用。
    pub fn facts_for(&self, path: &str) -> FileFacts {
        let mut facts = FileFacts::default();
        let Ok(conn) = self.conn.lock() else {
            return facts;
        };
        let row = conn
            .query_row(
                "SELECT h.hash, j.stats
                   FROM file_cache c
                   LEFT JOIN file_hash h
                     ON h.path = c.path AND h.size = c.size AND h.mtime = c.mtime
                   LEFT JOIN file_cjk j
                     ON j.path = c.path AND j.size = c.size AND j.mtime = c.mtime
                  WHERE c.path = ?1",
                params![path],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .optional();
        let Ok(Some((hash, stats))) = row else {
            return facts;
        };
        facts.hash = hash;
        if let Some(stats) = stats {
            if let Ok(stats) = serde_json::from_str::<CjkStats>(&stats) {
                if stats.total_chars > 0 {
                    facts.total_chars = Some(stats.total_chars);
                }
                facts.han = Some(stats.han);
            }
        }
        facts
    }

    /// Files whose character coverage has not been counted yet.
    pub fn cjk_candidates(&self, limit: usize) -> Vec<(String, u64, i64)> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT c.path, c.size, c.mtime
               FROM file_cache c
               LEFT JOIN file_cjk j
                 ON j.path = c.path AND j.size = c.size AND j.mtime = c.mtime
              WHERE j.path IS NULL
              LIMIT ?1",
        ) else {
            return Vec::new();
        };
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, i64>(2)?,
            ))
        });
        match rows {
            Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn put_cjk(&self, path: &str, size: u64, mtime: i64, stats: &CjkStats) {
        let Ok(json) = serde_json::to_string(stats) else {
            return;
        };
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO file_cjk (path, size, mtime, stats) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(path) DO UPDATE SET size = ?2, mtime = ?3, stats = ?4",
                params![path, size as i64, mtime, json],
            );
        }
    }

    /// Coverage already stored for an unchanged file.
    #[allow(dead_code)] // 预留给详情页直接从索引取统计，目前仍走 font_detail 实时计算
    pub fn get_cjk(&self, path: &str, size: u64, mtime: i64) -> Option<CjkStats> {
        let conn = self.conn.lock().ok()?;
        let json: Option<String> = conn
            .query_row(
                "SELECT stats FROM file_cjk WHERE path = ?1 AND size = ?2 AND mtime = ?3",
                params![path, size as i64, mtime],
                |row| row.get(0),
            )
            .optional()
            .ok()?;
        json.and_then(|s| serde_json::from_str::<CjkStats>(&s).ok())
    }

    /// Count characters for pending files (bounded by `limit`).
    /// Runs off the UI thread; one pass at a time.
    pub fn cjk_pending(&self, limit: usize) -> usize {
        if CJK_RUNNING.swap(true, Ordering::SeqCst) {
            return 0;
        }
        let mut done = 0;
        for (path, size, mtime) in self.cjk_candidates(limit) {
            let stats = count_file_cjk(Path::new(&path));
            match stats {
                Some(stats) => {
                    self.put_cjk(&path, size, mtime, &stats);
                    done += 1;
                }
                None => {
                    // Unreadable / unparsable: store an empty result so this
                    // file is not retried on every launch.
                    self.put_cjk(&path, size, mtime, &CjkStats::default());
                }
            }
        }
        CJK_RUNNING.store(false, Ordering::SeqCst);
        done
    }
}

/// Coverage for a whole file. Collections (TTC/OTC) are summed across faces;
/// the block table comes from the face that covers the most characters.
fn count_file_cjk(path: &Path) -> Option<CjkStats> {
    let data = std::fs::read(path).ok()?;
    let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    if count == 0 {
        return None;
    }
    let mut merged: Option<CjkStats> = None;
    for index in 0..count {
        let Ok(face) = ttf_parser::Face::parse(&data, index) else {
            continue;
        };
        let one = crate::cjk::stats_for_face(&face);
        merged = Some(match merged {
            None => one,
            Some(mut acc) => {
                let widest = acc.total_chars >= one.total_chars;
                let blocks = if widest { acc.blocks.clone() } else { one.blocks.clone() };
                acc.total_chars += one.total_chars;
                acc.han += one.han;
                acc.han_basic += one.han_basic;
                acc.han_ext_a += one.han_ext_a;
                acc.han_ext_b += one.han_ext_b;
                acc.han_ext_c += one.han_ext_c;
                acc.han_ext_d += one.han_ext_d;
                acc.han_ext_e += one.han_ext_e;
                acc.han_ext_f += one.han_ext_f;
                acc.han_ext_g += one.han_ext_g;
                acc.han_ext_h += one.han_ext_h;
                acc.han_ext_i += one.han_ext_i;
                acc.han_compat += one.han_compat;
                acc.kana += one.kana;
                acc.hangul += one.hangul;
                acc.latin += one.latin;
                acc.fullwidth += one.fullwidth;
                acc.blocks = blocks;
                acc
            }
        });
    }
    merged
}

fn sha256_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; HASH_CHUNK];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        out.push_str(&format!("{byte:02x}"));
    }
    Some(out)
}

/// Local wall-clock stamp, `YYYY-MM-DD HH:MM:SS`.
pub(crate) fn local_stamp() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut st = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut st) };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

/// Append one line per scan to `Data\scan.log`（程序所在文件夹）so the
/// indexing behaviour can be checked without a debugger.
pub fn log_scan(line: &str) {
    let path = crate::store::app_data_dir().join("scan.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{} | {line}", local_stamp());
    }
}
