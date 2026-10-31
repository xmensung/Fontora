use crate::db::{self, FontCache};
use crate::font_types::{FontFace, FontSource};
use crate::parser;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::Emitter;
use walkdir::WalkDir;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub done: usize,
    pub total: usize,
}

/// Default folder moved imports are stored in (per-user fonts on Windows).
pub fn managed_font_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Microsoft\\Windows\\Fonts")
}

/// Effective library folder: the settings override when set, else the default.
pub fn effective_managed_dir(library_dir: Option<&str>) -> PathBuf {
    match library_dir.map(str::trim).filter(|s| !s.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => managed_font_dir(),
    }
}

fn font_dirs() -> Vec<(PathBuf, FontSource)> {
    let mut dirs_list: Vec<(PathBuf, FontSource)> = Vec::new();

    dirs_list.push((PathBuf::from("C:\\Windows\\Fonts"), FontSource::System));
    if let Some(local) = dirs::data_local_dir() {
        dirs_list.push((local.join("Microsoft\\Windows\\Fonts"), FontSource::User));
    }

    dirs_list
}

fn classify(path: &std::path::Path, base_source: FontSource, managed: &Path) -> FontSource {
    if base_source == FontSource::User && path.starts_with(managed) {
        FontSource::Managed
    } else {
        base_source
    }
}

pub fn all_dirs(extra: &[String]) -> Vec<(PathBuf, FontSource)> {
    let mut dirs_list = font_dirs();
    for d in extra {
        dirs_list.push((PathBuf::from(d), FontSource::User));
    }
    dirs_list
}

/// Walk the library and parse every font file.
///
/// When `cache` is present, files whose path + size + mtime are unchanged
/// reuse the metadata stored by a previous scan instead of being re-parsed.
pub fn scan_all(
    app: &tauri::AppHandle,
    extra: &[String],
    managed: &Path,
    cache: Option<&FontCache>,
) -> Vec<FontFace> {
    let files: Vec<(PathBuf, FontSource)> = all_dirs(extra)
        .into_iter()
        .flat_map(|(dir, source)| {
            WalkDir::new(dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(move |e| (e.into_path(), source))
                .filter(|(p, _)| parser::is_font_file(p))
                .collect::<Vec<_>>()
        })
        .collect();

    let total = files.len();
    let started = std::time::Instant::now();
    let mut faces: Vec<FontFace> = Vec::with_capacity(total);
    let mut alive: HashSet<String> = HashSet::with_capacity(total);

    for (done, (path, base_source)) in files.into_iter().enumerate() {
        let source = classify(&path, base_source, managed);
        let path_str = path.to_string_lossy().into_owned();
        alive.insert(path_str.clone());

        let stamp = db::file_stamp(&path);
        let mut cached: Option<Vec<FontFace>> = None;
        if let (Some(cache), Some((size, mtime))) = (cache, stamp) {
            if let Some(list) = cache.get(&path_str, size, mtime) {
                // Moving the library folder changes the source label.
                if list.iter().all(|f| f.source == source) {
                    cached = Some(list);
                }
            }
        }

        match cached {
            Some(list) => faces.extend(list),
            None => {
                let parsed = parser::parse_font_file(&path, source);
                if let (Some(cache), Some((size, mtime))) = (cache, stamp) {
                    cache.put(&path_str, size, mtime, &parsed);
                }
                faces.extend(parsed);
            }
        }

        if done % 25 == 0 || done + 1 == total {
            let _ = app.emit("scan:progress", ScanProgress { done: done + 1, total });
        }
    }

    faces.sort_by(|a, b| a.id.cmp(&b.id));
    faces.dedup_by(|a, b| a.id == b.id);

    let (hits, parsed) = cache.map(|c| c.stats()).unwrap_or((0, 0));
    if let Some(cache) = cache {
        cache.prune(&alive);
    }
    db::log_scan(&format!(
        "files={total} cached={hits} parsed={parsed} faces={} index={} {:.2}s",
        faces.len(),
        if cache.is_some() { "on" } else { "off" },
        started.elapsed().as_secs_f64()
    ));

    faces
}
