mod activation;
mod adobe;
mod affinity;
mod audit;
mod charmap;
mod charsets;
mod cjk;
mod db;
mod dupe;
mod export;
mod font_detail;
mod font_types;
mod glyphs;
mod health;
mod installer;
mod ledger;
mod migrate;
mod net;
mod online;
mod organize;
mod parser;
mod pinyin;
mod pinyin_data;
mod reference;
mod registry;
mod scanner;
mod sfnt;
mod sound;
mod store;
mod subset;
mod usage;
mod watcher;

use font_types::{FontFace, FontSource, TrashEntry};
use std::collections::{HashMap, HashSet};
use store::Store;
use tauri::{Manager, State};

#[tauri::command]
async fn scan_fonts(app: tauri::AppHandle) -> Result<Vec<FontFace>, String> {
    db::log_scan("scan_fonts() called");
    let (extra, library_dir, linked) = {
        let store: State<Store> = app.state();
        let state = store.0.lock().map_err(|e| e.to_string())?;
        (
            state.extra_dirs.clone(),
            state.active_library_dir().map(|s| s.to_owned()),
            state.linked.clone(),
        )
    };
    let managed = scanner::effective_managed_dir(library_dir.as_deref());
    let cache = db::FontCache::open();
    let mut faces = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        let cache = cache.clone();
        move || scanner::scan_all(&app, &extra, &managed, cache.as_ref())
    })
    .await
    .map_err(|e| e.to_string())?;

    // Content hashes are only needed for duplicate detection and only for
    // files whose size collides with another file: do them off the UI path.
    if let Some(cache) = cache.clone() {
        std::thread::spawn(move || {
            let hashed = cache.hash_pending(4000);
            if hashed > 0 {
                db::log_scan(&format!("hashed={hashed} duplicate-candidates"));
            }
            // Character coverage is only needed for the detail view, so it
            // fills in progressively instead of delaying the first paint.
            let counted = cache.cjk_pending(600);
            if counted > 0 {
                db::log_scan(&format!("cjk={counted} fonts counted"));
            }
        });
    }

    // Linked fonts live outside the scanned dirs: parse them in place.
    // A linked file keeps its library (managed) identity over a scanned hit.
    let mut linked_faces = Vec::new();
    for path in &linked {
        let p = std::path::Path::new(path);
        if !p.is_file() {
            continue;
        }
        linked_faces.extend(parser::parse_font_file(p, FontSource::Managed));
    }
    if !linked_faces.is_empty() {
        let ids: std::collections::HashSet<&str> =
            linked_faces.iter().map(|f| f.id.as_str()).collect();
        faces.retain(|f| !ids.contains(f.id.as_str()));
        faces.extend(linked_faces);
    }

    let probe = activation::probe();
    let store: State<Store> = app.state();
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    // Drop links whose file is gone.
    let before = state.linked.len();
    state.linked.retain(|p| std::path::Path::new(p).is_file());
    if state.linked.len() != before {
        let _ = store::save(&state);
    }
    let mut faces: Vec<FontFace> = faces
        .into_iter()
        .map(|mut f| {
            f.active = activation::is_active(&probe, &state, &f);
            f
        })
        .collect();

    for (orig, parked) in &state.parked {
        for mut f in parser::parse_font_file(std::path::Path::new(parked), FontSource::User) {
            f.id = format!("{}#{}", orig, f.face_index);
            f.path = orig.clone();
            f.preview_path = Some(parked.clone());
            f.active = false;
            faces.push(f);
        }
    }
    // 文件增删之后，缺字缓存里记的路径可能已经不存在，整批作废重来。
    glyphs::clear_cache();
    Ok(faces)
}

#[tauri::command]
fn set_font_active(store: State<Store>, path: String, active: bool) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    activation::sync(&mut state, &path, active)?;
    store::save(&state)
}

#[tauri::command]
fn set_fonts_active(store: State<Store>, paths: Vec<String>, active: bool) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;

    let mut first_err = None;
    for path in &paths {
        if let Err(e) = activation::sync(&mut state, path, active) {
            first_err.get_or_insert(e);
        }
    }
    store::save(&state)?;
    audit::record(
        if active { audit::ACTIVATE } else { audit::DEACTIVATE },
        paths.len(),
        serde_json::json!({}),
    );
    first_err.map_or(Ok(()), Err)
}

#[tauri::command]
async fn install_fonts(
    app: tauri::AppHandle,
    store: State<'_, Store>,
    paths: Vec<String>,
    existing: Vec<String>,
    mode: String,
) -> Result<installer::InstallResult, String> {
    let mode = installer::InstallMode::parse(&mode)?;
    let library_dir = {
        let state = store.0.lock().map_err(|e| e.to_string())?;
        state.active_library_dir().map(|s| s.to_owned())
    };
    let library = scanner::effective_managed_dir(library_dir.as_deref());
    let known: std::collections::HashSet<String> = existing.into_iter().collect();
    let scope_app = app.clone();
    let mut result =
        tauri::async_runtime::spawn_blocking(move || installer::install(&app, paths, &known, mode, &library))
            .await
            .map_err(|e| e.to_string())?;
    // New fonts start deactivated on every OS: run them through the regular
    // deactivate path so a later toggle can bring them back.
    // The Settings toggle can opt back into auto-activation for small batches.
    let mut auto = false;
    if let Ok(mut state) = store.0.lock() {
        if mode == installer::InstallMode::Link {
            for face in &result.installed {
                state.linked.insert(face.path.clone());
            }
        }
        auto = state.auto_activate_imports && result.installed.len() < 64;
        for face in &result.installed {
            let _ = activation::sync(&mut state, &face.path, auto);
        }
        let _ = store::save(&state);
    }
    // Previews load through the asset protocol: allow the folders new files live in.
    for face in &result.installed {
        let p = std::path::Path::new(&face.path);
        if let Some(parent) = p.parent() {
            allow_dir(&scope_app, parent);
        }
    }
    // Report the state activation::sync just put these fonts in. Faces come out
    // of the parser active by default, so this has to be set either way.
    for face in &mut result.installed {
        face.active = auto;
    }
    audit::record(
        audit::INSTALL,
        result.installed.len(),
        serde_json::json!({
            "mode": if mode == installer::InstallMode::Link { "link" } else { "copy" },
            "active": auto,
        }),
    );
    Ok(result)
}

#[tauri::command]
fn uninstall_font(store: State<Store>, path: String, family: String) -> Result<TrashEntry, String> {

    {
        let mut state = store.0.lock().map_err(|e| e.to_string())?;
        if state.deactivated.contains(&path) {
            activation::sync(&mut state, &path, true)?;
            store::save(&state)?;
        }
        if state.linked.contains(&path) {
            // Linked file: never touch the original, only unregister.
            activation::drop_linked_alias(&state, &path);
            let entry = installer::unlink(&path, &family)?;
            state.linked.remove(&path);
            store::save(&state)?;
            // 这款字体已经不在库里了，统计里别留一行没人认领的记录。
            usage::forget(std::slice::from_ref(&family));
            audit::record(
                audit::UNINSTALL,
                1,
                serde_json::json!({ "family": family, "path": path, "linked": true }),
            );
            return Ok(entry);
        }
    }
    let entry = installer::uninstall(&path, &family)?;
    usage::forget(std::slice::from_ref(&family));
    audit::record(
        audit::UNINSTALL,
        1,
        serde_json::json!({ "family": family, "path": path, "linked": false }),
    );
    Ok(entry)
}

#[tauri::command]
async fn font_duplicates() -> Result<Vec<dupe::DupeGroup>, String> {
    // 索引里已经有哈希，这里再补一次，避免刚导入的字体因为还没算哈希而漏判；
    // 补哈希要读文件，放到阻塞线程池里跑，别卡住界面。
    tauri::async_runtime::spawn_blocking(|| {
        let Some(cache) = db::FontCache::open() else {
            return Vec::new();
        };
        dupe::find(&cache)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_trash() -> Vec<TrashEntry> {
    installer::list_trash()
}

#[tauri::command]
async fn affinity_connection() -> Result<affinity::AffinityConnection, String> {
    Ok(affinity::connection().await)
}

#[tauri::command]
fn affinity_link() -> Option<affinity::AffinityConnection> {
    affinity::last_link()
}

#[tauri::command]
fn affinity_session_activate(
    store: State<Store>,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    let mut first_err = None;
    let mut activated = Vec::with_capacity(paths.len());
    for path in paths {
        match activation::sync(&mut state, &path, true) {
            Ok(()) => activated.push(path),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    store::save(&state)?;
    affinity::note_activated(activated.clone());
    first_err.map_or(Ok(activated), Err)
}

#[tauri::command]
async fn online_catalogue(store: State<'_, Store>, force: bool) -> Result<online::CatalogueResponse, String> {
    let client = {
        let state = store.0.lock().map_err(|e| e.to_string())?;
        net::client(&state)?
    };
    online::fontsource::catalogue(&client, force).await
}

#[tauri::command]
async fn online_family(store: State<'_, Store>, id: String) -> Result<online::FamilyDetail, String> {
    let client = {
        let state = store.0.lock().map_err(|e| e.to_string())?;
        net::client(&state)?
    };
    online::github::detail(&client, &id).await
}

#[tauri::command]
async fn online_download(
    app: tauri::AppHandle,
    store: State<'_, Store>,
    id: String,
) -> Result<online::DownloadResult, String> {
    let client = {
        let state = store.0.lock().map_err(|e| e.to_string())?;
        net::client(&state)?
    };
    online::github::download(&app, &client, &id).await
}

#[tauri::command]
async fn online_preview(app: tauri::AppHandle, store: State<'_, Store>, id: String) -> Result<String, String> {
    let client = {
        let state = store.0.lock().map_err(|e| e.to_string())?;
        net::client(&state)?
    };
    let path = online::github::preview(&client, &id).await?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        allow_dir(&app, parent);
    }
    Ok(path)
}

#[tauri::command]
fn online_clear_staging(id: String) {
    online::github::clear_staging(&id);
}

#[tauri::command]
fn online_hosts() -> Vec<String> {
    net::ALLOWED_HOSTS.iter().map(|h| h.to_string()).collect()
}

#[tauri::command]
fn restore_from_trash(store: State<Store>, entry_id: String) -> Result<(), String> {
    let outcome = installer::restore(&entry_id)?;
    let detail = serde_json::json!({ "entry": entry_id });
    match outcome {
        installer::RestoreOutcome::Moved => {
            audit::record(audit::RESTORE, 1, detail);
            Ok(())
        }
        installer::RestoreOutcome::Relinked(path) => {
            let mut state = store.0.lock().map_err(|e| e.to_string())?;
            state.linked.insert(path);
            store::save(&state)?;
            audit::record(audit::RESTORE, 1, detail);
            Ok(())
        }
    }
}

#[tauri::command]
fn delete_trash_entry(entry_id: String) -> Result<(), String> {
    installer::delete_trash_entry(&entry_id)?;
    audit::record(
        audit::TRASH_DELETE,
        1,
        serde_json::json!({ "entry": entry_id }),
    );
    Ok(())
}

#[tauri::command]
fn empty_trash() -> Result<(), String> {
    let count = installer::list_trash().len();
    installer::empty_trash()?;
    audit::record(audit::TRASH_EMPTY, count.max(1), serde_json::json!({}));
    Ok(())
}

#[tauri::command]
fn get_tags(store: State<Store>) -> Result<HashMap<String, Vec<String>>, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(state.tags.clone())
}

#[tauri::command]
fn set_tags(store: State<Store>, family: String, tags: Vec<String>) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    if tags.is_empty() {
        state.tags.remove(&family);
    } else {
        state.tags.insert(family, tags);
    }
    store::save(&state)
}

/// AI 名录回读：输入「家族名 → 标签列表」，只影响包里提到的家族，
/// 未提到的保持原样；空列表 = 清除该家族全部标签。标签逐项做 trim/去重。
#[tauri::command]
fn import_tags(
    store: State<Store>,
    entries: HashMap<String, Vec<String>>,
) -> Result<usize, String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    let mut applied = 0usize;
    for (family, tags) in entries {
        let family = family.trim().to_string();
        if family.is_empty() {
            continue;
        }
        let clean: Vec<String> = tags
            .into_iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if clean.is_empty() {
            state.tags.remove(&family);
        } else {
            state.tags.insert(family, clean);
        }
        applied += 1;
    }
    store::save(&state)?;
    Ok(applied)
}

#[tauri::command]
fn get_collections(store: State<Store>) -> Result<HashMap<String, Vec<String>>, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(state.collections.clone())
}

#[tauri::command]
fn set_collection(store: State<Store>, name: String, families: Vec<String>) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    state.collections.insert(name, families);
    store::save(&state)
}

#[tauri::command]
fn delete_collection(store: State<Store>, name: String) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    state.collections.remove(&name);
    store::save(&state)
}

#[tauri::command]
fn rename_collection(store: State<Store>, from: String, to: String) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    if state.collections.contains_key(&to) {
        return Err(format!("a collection named \"{to}\" already exists"));
    }
    let families = state
        .collections
        .remove(&from)
        .ok_or_else(|| format!("collection \"{from}\" not found"))?;
    state.collections.insert(to, families);
    store::save(&state)
}

#[tauri::command]
fn play_sound(kind: String, level: String) {
    sound::play(&kind, &level);
}

#[tauri::command]
fn get_notes(store: State<Store>) -> Result<HashMap<String, String>, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(state.notes.clone())
}

#[tauri::command]
fn set_note(store: State<Store>, family: String, note: String) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    if note.trim().is_empty() {
        state.notes.remove(&family);
    } else {
        state.notes.insert(family, note);
    }
    store::save(&state)
}

#[tauri::command]
fn get_favorites(store: State<Store>) -> Result<Vec<String>, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(state.favorites.iter().cloned().collect())
}

#[tauri::command]
fn set_favorite(store: State<Store>, family: String, favorite: bool) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    if favorite {
        state.favorites.insert(family);
    } else {
        state.favorites.remove(&family);
    }
    store::save(&state)
}

#[tauri::command]
fn get_ratings(store: State<Store>) -> Result<HashMap<String, u8>, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(state.ratings.clone())
}

/// 评分 1–5；0（或越界）就是取消评分，不留在文件里。
#[tauri::command]
fn set_rating(store: State<Store>, family: String, score: u8) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    if score == 0 || score > 5 {
        state.ratings.remove(&family);
    } else {
        state.ratings.insert(family, score);
    }
    store::save(&state)
}

#[tauri::command]
fn get_aliases(store: State<Store>) -> Result<HashMap<String, Vec<String>>, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(state.aliases.clone())
}

#[tauri::command]
fn set_aliases(store: State<Store>, family: String, aliases: Vec<String>) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    let mut clean: Vec<String> = Vec::new();
    for alias in aliases {
        let alias = alias.trim();
        if alias.is_empty() || alias.eq_ignore_ascii_case(&family) {
            continue;
        }
        if clean.iter().any(|a: &String| a.eq_ignore_ascii_case(alias)) {
            continue;
        }
        clean.push(alias.to_string());
        if clean.len() >= 8 {
            break;
        }
    }
    if clean.is_empty() {
        state.aliases.remove(&family);
    } else {
        state.aliases.insert(family, clean);
    }
    store::save(&state)
}

/// 一批名字 → 拼音检索键。前端在扫描完 / 改别名后一次性取回，搜索时纯本地匹配。
#[tauri::command]
fn pinyin_keys(names: Vec<String>) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::with_capacity(names.len());
    for name in names {
        if out.contains_key(&name) {
            continue;
        }
        let keys = pinyin::keys(&name);
        if !keys.is_empty() {
            out.insert(name, keys);
        }
    }
    out
}

const DESIGN_W: f64 = 1280.0;
const DESIGN_H: f64 = 820.0;
const SCALE_STEPS: [f64; 4] = [0.8, 0.85, 0.9, 1.0];

fn auto_ui_scale(avail_w: f64, avail_h: f64) -> f64 {
    let raw = (avail_w / DESIGN_W).min(avail_h / DESIGN_H);
    let mut chosen = SCALE_STEPS[0];
    for step in SCALE_STEPS {
        if step <= raw + 0.0001 {
            chosen = step;
        }
    }
    chosen
}

fn fit_window(window: &tauri::WebviewWindow, scale_pref: Option<f64>) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        if let Some(scale) = scale_pref {
            let _ = window.set_zoom(scale);
        }
        return;
    };
    let dpi = monitor.scale_factor();
    if !(dpi > 0.0) {
        return;
    }
    let work = monitor.work_area();
    let avail_w = work.size.width as f64 / dpi;
    let avail_h = work.size.height as f64 / dpi;
    if !(avail_w > 0.0) || !(avail_h > 0.0) {
        return;
    }

    if let Ok(size) = window.inner_size() {
        let cur_w = size.width as f64 / dpi;
        let cur_h = size.height as f64 / dpi;
        let w = cur_w.min(avail_w);
        let h = cur_h.min(avail_h);
        if w < cur_w - 0.5 || h < cur_h - 0.5 {
            let _ = window.set_size(tauri::LogicalSize::new(w, h));
        }
        let x = work.position.x as f64 / dpi + (avail_w - w).max(0.0) / 2.0;
        let y = work.position.y as f64 / dpi + (avail_h - h).max(0.0) / 2.0;
        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    }

    let scale = scale_pref.unwrap_or_else(|| auto_ui_scale(avail_w, avail_h));
    let _ = window.set_zoom(scale);
}

#[tauri::command]
fn set_ui_scale(window: tauri::WebviewWindow, scale: Option<f64>) -> Result<f64, String> {
    let clamped = scale.map(|s| s.clamp(0.5, 2.0));
    fit_window_scale_only(&window, clamped)
}

fn fit_window_scale_only(window: &tauri::WebviewWindow, scale_pref: Option<f64>) -> Result<f64, String> {
    let resolved = match scale_pref {
        Some(s) => s,
        None => {
            let monitor = window
                .current_monitor()
                .map_err(|e| e.to_string())?
                .ok_or("no monitor")?;
            let dpi = monitor.scale_factor();
            let work = monitor.work_area();
            auto_ui_scale(
                work.size.width as f64 / dpi,
                work.size.height as f64 / dpi,
            )
        }
    };
    window.set_zoom(resolved).map_err(|e| e.to_string())?;
    Ok(resolved)
}

#[tauri::command]
async fn fonts_with_char(paths: Vec<String>, codepoint: u32) -> Result<Vec<String>, String> {
    let Some(ch) = char::from_u32(codepoint) else {
        return Err("not a Unicode character".into());
    };
    tauri::async_runtime::spawn_blocking(move || sweep_charset(&paths, ch))
        .await
        .map_err(|e| e.to_string())
}

fn sweep_charset(paths: &[String], ch: char) -> Vec<String> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8)
        .max(1);
    let chunk = paths.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|group| {
                scope.spawn(move || {
                    let mut found = Vec::new();
                    for path in group {
                        let Ok(data) = std::fs::read(path) else {
                            continue;
                        };
                        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
                        let covered = (0..count).any(|i| {
                            ttf_parser::Face::parse(&data, i)
                                .map(|f| f.glyph_index(ch).is_some())
                                .unwrap_or(false)
                        });
                        if covered {
                            found.push(path.clone());
                        }
                    }
                    found
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .flatten()
            .collect::<Vec<String>>()
    })
}

#[tauri::command]
async fn check_glyphs(
    faces: Vec<(String, u32)>,
    text: String,
) -> Result<Vec<glyphs::GlyphReport>, String> {
    tauri::async_runtime::spawn_blocking(move || glyphs::check_faces(&faces, &text))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_glyph_cache() {
    glyphs::clear_cache();
}

#[tauri::command]
fn get_charset(path: String, face_index: u32) -> Result<Vec<u32>, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let face = ttf_parser::Face::parse(&data, face_index).map_err(|e| e.to_string())?;
    let mut cps: Vec<u32> = Vec::new();
    if let Some(cmap) = face.tables().cmap {
        for sub in cmap.subtables {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| {

                if cp > 0x20 && cp != 0x7f {
                    cps.push(cp);
                }
            });
        }
    }
    cps.sort_unstable();
    cps.dedup();
    cps.truncate(4096);
    Ok(cps)
}

/// Everything the detail page renders, read straight from the font file.
#[tauri::command]
async fn font_detail(
    app: tauri::AppHandle,
    path: String,
    face_index: u32,
) -> Result<font_detail::FontDetail, String> {
    // 样张是磁盘上的图片，webview 要用 asset 协议才显示得出来；
    // 库目录是用户自己加的，只能运行时放行。
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = app.asset_protocol_scope().allow_directory(dir, true);
    }
    tauri::async_runtime::spawn_blocking(move || font_detail::build(&path, face_index))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn export_font(src: String, dest: String) -> Result<(), String> {
    std::fs::copy(&src, &dest).map(|_| ()).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FaceExport {
    path: String,
    family: String,
    style: String,
    byte_size: u64,
    faces_in_file: u32,
    renamed: bool,
}

/// 从文件里读第 `index` 个 face 的家族名与字重（用于给导出文件命名）。
fn face_identity(data: &[u8], index: u32) -> (String, String) {
    let Ok(face) = ttf_parser::Face::parse(data, index) else {
        return (String::new(), "Regular".to_string());
    };
    let family = crate::parser::family_name(&face).unwrap_or_default();
    let style = [
        ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY,
        ttf_parser::name_id::SUBFAMILY,
    ]
    .iter()
    .find_map(|&id| {
        face.names()
            .into_iter()
            .filter(|n| n.name_id == id)
            .find_map(|n| n.to_string())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
    .unwrap_or_else(|| "Regular".to_string());
    (family, style)
}

/// 导出一个 face（TTC/OTC 会真正拆成独立文件；单字体文件则原样复制）。
#[tauri::command]
async fn export_font_face(path: String, face_index: u32, dest: String) -> Result<FaceExport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data = sfnt::read_file(&path)?;
        let container = sfnt::read_face(&data, face_index)?;
        let (family, style) = face_identity(&data, face_index);
        let bytes = if container.faces_in_file <= 1 {
            data.clone()
        } else {
            sfnt::build_sfnt(container.version, &container.tables)
        };
        std::fs::write(&dest, &bytes).map_err(|e| format!("写入失败：{e}"))?;
        Ok(FaceExport {
            path: dest,
            family,
            style,
            byte_size: bytes.len() as u64,
            faces_in_file: container.faces_in_file,
            renamed: false,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 拆出单个 face，并把 name 表里的家族名 / 字重改成指定的中文名。
#[tauri::command]
async fn export_font_face_renamed(
    path: String,
    face_index: u32,
    dest: String,
    family: String,
    style: String,
) -> Result<FaceExport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data = sfnt::read_file(&path)?;
        let mut container = sfnt::read_face(&data, face_index)?;
        sfnt::rename_tables(&mut container.tables, &family, &style)?;
        let bytes = sfnt::build_sfnt(container.version, &container.tables);
        std::fs::write(&dest, &bytes).map_err(|e| format!("写入失败：{e}"))?;
        Ok(FaceExport {
            path: dest,
            family: family.clone(),
            style: style.clone(),
            byte_size: bytes.len() as u64,
            faces_in_file: container.faces_in_file,
            renamed: true,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 把集合里的每个字体都拆成独立文件，写进 `dest_dir`。
#[tauri::command]
async fn export_font_collection(path: String, dest_dir: String) -> Result<Vec<FaceExport>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = std::path::Path::new(&dest_dir);
        if !dir.is_dir() {
            return Err("目标不是文件夹".into());
        }
        let data = sfnt::read_file(&path)?;
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            let container = sfnt::read_face(&data, index)?;
            let (family, style) = face_identity(&data, index);
            let stem = sfnt::safe_file_stem(
                if family.is_empty() { "font" } else { &family },
                &style,
            );
            let ext = sfnt::extension_for(container.version);
            let mut dest = dir.join(format!("{stem}.{ext}"));
            let mut n = 2;
            while dest.exists() {
                dest = dir.join(format!("{stem} ({n}).{ext}"));
                n += 1;
            }
            let bytes = sfnt::build_sfnt(container.version, &container.tables);
            std::fs::write(&dest, &bytes).map_err(|e| format!("写入失败：{e}"))?;
            out.push(FaceExport {
                path: dest.to_string_lossy().into_owned(),
                family,
                style,
                byte_size: bytes.len() as u64,
                faces_in_file: count,
                renamed: false,
            });
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 这个文件里到底有几个 face（判断要不要提示「拆包」）。
#[tauri::command]
async fn font_file_faces(path: String) -> Result<u32, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data = sfnt::read_file(&path)?;
        Ok(ttf_parser::fonts_in_collection(&data).unwrap_or(1))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 子集化后按 `format` 封装：`ttf` / `woff` / `woff2`。
#[tauri::command]
async fn export_font_subset(
    path: String,
    face_index: u32,
    dest: String,
    charset: String,
    text: String,
    format: String,
) -> Result<FaceExport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let data = sfnt::read_file(&path)?;
        let keep = if charset == "all" {
            subset::all_codepoints(&data, face_index)?
        } else {
            subset::charset_preset(&charset, &text)
        };
        let kept_chars = keep.len();
        let subset_bytes = subset::subset(&data, face_index, &keep)?;
        let container = sfnt::read_face(&subset_bytes, 0)?;
        let bytes = match format.as_str() {
            "woff" => subset::to_woff(container.version, &container.tables)?,
            "woff2" => subset::to_woff2(container.version, &container.tables)?,
            _ => subset_bytes,
        };
        std::fs::write(&dest, &bytes).map_err(|e| format!("写入失败：{e}"))?;
        let (family, style) = face_identity(&data, face_index);
        let _ = kept_chars;
        Ok(FaceExport {
            path: dest,
            family,
            style,
            byte_size: bytes.len() as u64,
            faces_in_file: container.faces_in_file.max(1),
            renamed: false,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 扫描一个目录，看设计稿 / 源码引用了哪些字体。
#[tauri::command]
async fn font_references(root: String) -> Result<reference::ReferenceReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut names: Vec<String> = Vec::new();
        if let Some(cache) = db::FontCache::open() {
            for file in cache.all_files() {
                for face in file.faces {
                    names.push(face.family.clone());
                    if !face.style.is_empty() {
                        names.push(format!("{} {}", face.family, face.style));
                    }
                }
            }
        }
        names.sort();
        names.dedup();
        reference::scan(&root, &names)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 字符地图：整款字体的码位覆盖，压缩成区间表交给界面。
#[tauri::command]
async fn font_charmap(path: String, face_index: u32) -> Result<charmap::CharMap, String> {
    tauri::async_runtime::spawn_blocking(move || charmap::build(&path, face_index))
        .await
        .map_err(|e| e.to_string())?
}

/// 字体体检：表结构、命名、字符覆盖逐项检查。
#[tauri::command]
async fn font_health(path: String, face_index: u32) -> Result<health::Health, String> {
    tauri::async_runtime::spawn_blocking(move || health::build(&path, face_index))
        .await
        .map_err(|e| e.to_string())?
}

/// 全部使用统计（一款字体一条）。
#[tauri::command]
async fn font_usage() -> Result<Vec<usage::UsageRow>, String> {
    tauri::async_runtime::spawn_blocking(usage::all)
        .await
        .map_err(|e| e.to_string())
}

/// 记一笔：kind = view / install / reference。
#[tauri::command]
async fn font_usage_touch(families: Vec<String>, kind: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || usage::touch(&families, &kind))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn font_usage_clear() -> Result<(), String> {
    let out = tauri::async_runtime::spawn_blocking(usage::clear)
        .await
        .map_err(|e| e.to_string())?;
    if out.is_ok() {
        audit::record(audit::USAGE_CLEAR, 1, serde_json::json!({}));
    }
    out
}

/// 归档来源扫描：把任意文件夹里的字体文件解析成清单，只读。
#[tauri::command]
async fn organize_scan(dir: String) -> Result<Vec<organize::OrganizeItem>, String> {
    tauri::async_runtime::spawn_blocking(move || organize::scan_items(&dir))
        .await
        .map_err(|e| e.to_string())?
}

/// 归档方案预览：只读文件系统，一个字节都不写。
#[tauri::command]
async fn organize_plan(
    items: Vec<organize::OrganizeItem>,
    dest: String,
    key: String,
    mode: String,
) -> Result<organize::OrganizePlan, String> {
    tauri::async_runtime::spawn_blocking(move || organize::build_plan(&items, &dest, &key, &mode))
        .await
        .map_err(|e| e.to_string())?
}

/// 真正动手：后端重算一遍方案再执行，单个失败不影响其它。
#[tauri::command]
async fn organize_apply(
    items: Vec<organize::OrganizeItem>,
    dest: String,
    key: String,
    mode: String,
) -> Result<organize::OrganizeResult, String> {
    let label = mode.clone();
    let target = dest.clone();
    let out = tauri::async_runtime::spawn_blocking(move || organize::apply(&items, &dest, &key, &mode))
        .await
        .map_err(|e| e.to_string())?;
    if let Ok(result) = &out {
        audit::record(
            audit::ORGANIZE,
            result.done.max(1),
            serde_json::json!({ "mode": label, "dest": target }),
        );
    }
    out
}

/// 导出迁移包：用户资产 + 设置，可选连字体文件与使用统计。
#[tauri::command]
async fn bundle_export(
    dest: String,
    rows: Vec<export::ExportRow>,
    with_files: bool,
    with_usage: bool,
    store: State<'_, Store>,
) -> Result<migrate::BundleExport, String> {
    // 只把状态抄一份出来，不长时间占着锁 —— 打包要读几十兆字体。
    let state = {
        let guard = store.0.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };
    let usage_rows = if with_usage { usage::all() } else { Vec::new() };
    let out = tauri::async_runtime::spawn_blocking(move || {
        let cache = db::FontCache::open();
        let rows = export::enrich(rows, cache.as_ref());
        migrate::write_bundle(&dest, &state, &rows, &usage_rows, with_files, with_usage)
    })
    .await
    .map_err(|e| e.to_string())??;
    audit::record(
        audit::BUNDLE_OUT,
        out.files.max(1),
        serde_json::json!({
            "families": out.families,
            "files": out.files,
            "bytes": out.bytes,
            "withFiles": with_files,
            "withUsage": with_usage,
            "path": out.path,
        }),
    );
    Ok(out)
}

/// 只看一眼：包是哪台机器导的、里面有多少东西、有多少是本机没有的。
#[tauri::command]
async fn bundle_inspect(path: String, known: Vec<String>) -> Result<migrate::ImportPlan, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let known: std::collections::HashSet<String> = known.into_iter().collect();
        migrate::inspect(&path, &known)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 恢复迁移包。合并只是补空缺，替换会把同名条目整个换成包里的。
#[tauri::command]
async fn bundle_import(
    app: tauri::AppHandle,
    path: String,
    mode: String,
    store: State<'_, Store>,
) -> Result<migrate::ImportResult, String> {
    let mode = migrate::MergeMode::parse(&mode)?;
    let snapshot = {
        let guard = store.0.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };
    let lib_dir = scanner::effective_managed_dir(snapshot.active_library_dir());
    let lib_for_job = lib_dir.clone();
    let (out, updated) = tauri::async_runtime::spawn_blocking(move || {
        let mut next = snapshot;
        let out = migrate::apply(&path, mode, &mut next, &lib_for_job)?;
        Ok::<_, String>((out, next))
    })
    .await
    .map_err(|e| e.to_string())??;
    // 内存里那份必须一起换掉，否则下一次保存会把刚恢复的东西覆盖回去。
    {
        let mut guard = store.0.lock().map_err(|e| e.to_string())?;
        *guard = updated;
        store::save(&guard)?;
    }
    allow_dir(&app, &lib_dir);
    audit::record(
        audit::BUNDLE_IN,
        out.fonts_written.max(1),
        serde_json::json!({
            "mode": out.mode,
            "tags": out.tags,
            "collections": out.collections,
            "favorites": out.favorites,
            "ratings": out.ratings,
            "aliases": out.aliases,
            "notes": out.notes,
            "fonts": out.fonts_written,
        }),
    );
    Ok(out)
}

/// 操作日志：最近 `limit` 条。
#[tauri::command]
async fn audit_log(limit: usize) -> Vec<audit::AuditEntry> {
    tauri::async_runtime::spawn_blocking(move || audit::all(limit))
        .await
        .unwrap_or_default()
}

#[tauri::command]
async fn audit_clear() -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(audit::clear)
        .await
        .map_err(|e| e.to_string())?
}

/// 导出清单：CSV（带 BOM）或 JSON。哈希/字符数从索引补齐，不重新解析字体。
#[tauri::command]
async fn export_catalog(
    dest: String,
    format: String,
    rows: Vec<export::ExportRow>,
    chinese: bool,
) -> Result<export::CatalogExport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = db::FontCache::open();
        let rows = export::enrich(rows, cache.as_ref());
        let body = match format.as_str() {
            "json" => export::json_text(&rows)?,
            _ => export::csv_text(&rows, chinese),
        };
        std::fs::write(&dest, body.as_bytes()).map_err(|e| format!("写入失败：{e}"))?;
        Ok(export::CatalogExport {
            path: dest,
            rows: rows.len(),
            bytes: body.len() as u64,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 打包项目字体包：字体文件 + fonts.css + manifest.json + 授权说明。
#[tauri::command]
async fn export_font_package(
    dest: String,
    name: String,
    rows: Vec<export::ExportRow>,
    with_files: bool,
    web: bool,
    css: bool,
) -> Result<export::PackageExport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cache = db::FontCache::open();
        let rows = export::enrich(rows, cache.as_ref());
        export::build_package(
            &dest,
            &rows,
            &export::PackageOptions {
                name,
                with_files,
                web,
                css,
            },
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 生成 `@font-face` 片段（面板里预览 / 复制用，不落盘）。
#[tauri::command]
fn font_face_css(rows: Vec<export::ExportRow>, web: bool) -> String {
    let stems = export::file_stems(&rows);
    export::css_text(&rows, &stems, web)
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > 8 * 1024 * 1024 {
        return Err("file is too large to be a library export".into());
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

static SESSION_ACTIVATED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

#[tauri::command]
fn set_fonts_active_session(store: State<Store>, paths: Vec<String>) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;

    // 先记账、先落盘，再动注册表。顺序反过来就会留出「注册表已经改了、
    // 账本还没写上」的窗口，那种情况崩溃之后谁都查不出来。
    // 只记真要写注册表项的那些：本来就有登记的，不是这次改出来的。
    let pending: Vec<String> = paths
        .iter()
        .filter(|p| activation::needs_registry_write(p))
        .cloned()
        .collect();
    if ledger::begin(&mut state, pending) > 0 {
        store::save_durable(&state)?;
    }

    let mut first_err = None;
    let mut activated = Vec::with_capacity(paths.len());
    for path in paths {
        match activation::sync(&mut state, &path, true) {
            Ok(()) => activated.push(path),
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    store::save(&state)?;
    if let Ok(mut session) = SESSION_ACTIVATED.lock() {
        session.extend(activated);
    }
    first_err.map_or(Ok(()), Err)
}

fn revert_session_activations(app: &tauri::AppHandle) {
    use tauri::Manager;
    let paths: Vec<String> = match SESSION_ACTIVATED.lock() {
        Ok(mut s) => std::mem::take(&mut *s),
        Err(_) => return,
    };
    if paths.is_empty() {
        return;
    }
    let store = app.state::<Store>();
    let mut guard = match store.0.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    for p in &paths {
        // 撤销成功才销账。失败的话条目留在账本里，下次启动自愈再试一次 ——
        // 宁可多试一遍，也不能让一条没卸掉的字体悄悄从账上消失。
        if activation::sync(&mut guard, p, false).is_ok() {
            ledger::commit(&mut guard, p);
        }
    }
    affinity::revert_session(&mut guard);
    let _ = store::save(&guard);
}

#[tauri::command]
fn write_binary_file(path: String, data: Vec<u8>) -> Result<(), String> {
    std::fs::write(&path, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_features(path: String, face_index: u32) -> Result<Vec<String>, String> {
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    let face = ttf_parser::Face::parse(&data, face_index).map_err(|e| e.to_string())?;
    let mut tags: Vec<String> = Vec::new();
    let mut collect = |table: Option<ttf_parser::opentype_layout::LayoutTable>| {
        if let Some(t) = table {
            for f in t.features {
                tags.push(String::from_utf8_lossy(&f.tag.to_bytes()).into_owned());
            }
        }
    };
    collect(face.tables().gsub);
    collect(face.tables().gpos);
    tags.sort_unstable();
    tags.dedup();
    Ok(tags)
}

#[tauri::command]
fn export_fonts(paths: Vec<String>, dest_dir: String) -> Result<u32, String> {
    let dir = std::path::Path::new(&dest_dir);
    if !dir.is_dir() {
        return Err("destination is not a directory".into());
    }
    let mut n = 0;
    for p in paths {
        let src = std::path::Path::new(&p);
        let name = src.file_name().ok_or("invalid font path")?;
        std::fs::copy(src, dir.join(name)).map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    extra_dirs: Vec<String>,
    watch_enabled: bool,
    auto_activate_imports: bool,
    library_dir: Option<String>,
    library_dir_enabled: bool,
    affinity_enabled: bool,
    affinity_deactivate_on_quit: bool,
    online_fonts_enabled: bool,
}

#[tauri::command]
fn get_settings(store: State<Store>) -> Result<Settings, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(Settings {
        extra_dirs: state.extra_dirs.clone(),
        watch_enabled: state.watch_enabled,
        auto_activate_imports: state.auto_activate_imports,
        library_dir: state.library_dir.clone(),
        library_dir_enabled: state.library_dir_enabled,
        affinity_enabled: state.affinity_enabled,
        affinity_deactivate_on_quit: state.affinity_deactivate_on_quit,
        online_fonts_enabled: state.online_fonts_enabled,
    })
}

#[tauri::command]
fn set_settings(
    app: tauri::AppHandle,
    store: State<Store>,
    settings: Settings,
) -> Result<(), String> {
    {
        let mut state = store.0.lock().map_err(|e| e.to_string())?;
        state.extra_dirs = settings.extra_dirs.clone();
        state.watch_enabled = settings.watch_enabled;
        state.auto_activate_imports = settings.auto_activate_imports;
        state.library_dir = settings.library_dir.clone();
        state.library_dir_enabled = settings.library_dir_enabled;
        state.affinity_enabled = settings.affinity_enabled;
        state.affinity_deactivate_on_quit = settings.affinity_deactivate_on_quit;
        state.online_fonts_enabled = settings.online_fonts_enabled;
        store::save(&state)?;
    }
    allow_dir(
        &app,
        &scanner::effective_managed_dir(
            settings
                .library_dir_enabled
                .then(|| settings.library_dir.as_deref())
                .flatten(),
        ),
    );
    allow_previews(&app, &settings.extra_dirs);
    apply_watch(&app, settings.watch_enabled, &settings.extra_dirs)
}

#[tauri::command]
fn default_library_dir() -> String {
    scanner::managed_font_dir().to_string_lossy().into_owned()
}

fn allow_dir(app: &tauri::AppHandle, dir: &std::path::Path) {
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let _ = app.asset_protocol_scope().allow_directory(&dir, true);
}

fn allow_previews(app: &tauri::AppHandle, extra: &[String]) {
    let scope = app.asset_protocol_scope();
    for (dir, _) in scanner::all_dirs(extra) {

        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        let _ = scope.allow_directory(&dir, true);
    }
}

fn apply_watch(app: &tauri::AppHandle, enabled: bool, extra: &[String]) -> Result<(), String> {
    let handle: State<watcher::WatchHandle> = app.state();
    let mut slot = handle.0.lock().map_err(|e| e.to_string())?;
    *slot = None;
    if enabled {
        let dirs = scanner::all_dirs(extra).into_iter().map(|(d, _)| d).collect();
        *slot = Some(watcher::start(app.clone(), dirs)?);
    }
    Ok(())
}

fn close_to_tray_enabled(app: &tauri::AppHandle) -> bool {
    app.try_state::<Store>()
        .and_then(|store| {
            store
                .0
                .lock()
                .ok()
                .and_then(|s| s.prefs.get("closeToTray").and_then(|v| v.as_bool()))
        })
        .unwrap_or(false)
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItem::with_id(app, "show", "Show Fontora", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::InvalidIcon(std::io::Error::other("no bundled window icon")))?
        .clone();

    let builder = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("Fontora")
        .menu(&menu);

    let builder = builder.show_menu_on_left_click(false);

    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let mgr = app.autolaunch();
    if enabled {
        mgr.enable().map_err(|e| e.to_string())
    } else {
        mgr.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn get_autostart(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_prefs(store: State<Store>) -> Result<serde_json::Value, String> {
    let state = store.0.lock().map_err(|e| e.to_string())?;
    Ok(state.prefs.clone())
}

#[tauri::command]
fn set_prefs(store: State<Store>, prefs: serde_json::Value) -> Result<(), String> {
    let mut state = store.0.lock().map_err(|e| e.to_string())?;
    state.prefs = prefs;
    store::save(&state)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(format!("refusing to open non-web URL: {url}"));
    }

    let spawned = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
    };

    spawned.map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
fn adobe_available() -> bool {
    adobe::available()
}

#[tauri::command]
async fn apply_font_in_app(
    app: String,
    postscript_name: String,
    label: String,
) -> Result<String, String> {
    let target = adobe::Target::parse(&app)?;
    tauri::async_runtime::spawn_blocking(move || adobe::apply(target, &postscript_name, &label))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        db::log_scan(&format!("PANIC: {info}"));
        default_hook(info);
    }));
    db::log_scan("boot");

    let mut state = store::load();
    activation::reconcile(&mut state);

    let watch_enabled = state.watch_enabled;
    let start_minimized = state
        .prefs
        .get("startMinimized")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ui_scale_pref = state
        .prefs
        .get("uiScale")
        .and_then(|v| v.as_f64())
        .map(|s| s.clamp(0.5, 2.0));
    let extra_dirs = state.extra_dirs.clone();
    let library_dir = state.active_library_dir().map(|s| s.to_owned());
    let linked_dirs: Vec<String> = state.linked.iter().cloned().collect();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(Store(std::sync::Mutex::new(state)))
        .manage(watcher::WatchHandle(std::sync::Mutex::new(None)))
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                fit_window(&window, ui_scale_pref);
            }
            // 上次若是崩溃/强杀退出的，账本里会留着没销账的条目。趁窗口还没
            // 开始接活先回滚掉，免得用户一打开就看见字体还挂在「已启用」上。
            // 平时账本是空的，这里就是一次空判断，不拖慢启动。
            if let Some(store) = app.try_state::<Store>() {
                if let Ok(mut guard) = store.0.lock() {
                    let report = ledger::self_heal(&mut guard);
                    if !report.is_empty() {
                        let _ = store::save(&guard);
                        audit::record(
                            audit::SELF_HEAL,
                            report.rolled_back.len().max(1),
                            serde_json::json!({
                                "rolledBack": report.rolled_back.len(),
                                "alreadyGone": report.already_gone,
                                "kept": report.kept,
                            }),
                        );
                    }
                }
            }
            let tray_ok = build_tray(app.handle()).is_ok();
            // 窗口在 tauri.conf.json 里以 visible:false 起步，好让用户第一眼
            // 看到的是画好的界面而不是白底。正常由前端就绪后调用 show；但
            // 冷启动时 WebView2 光是把前端脚本跑起来就可能要几秒，这段时间
            // 窗口一直不显示，用户只看得见托盘图标，以为程序没启动。
            //
            // 这里 1.2 秒就把窗口亮出来 —— index.html 里自带一个启动画面，
            // 所以露出来的不会是一块白底，而是「FONTORA + 一条走动的进度线」。
            // 前端就绪后 React 接管，画面自然消失。
            if !start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(1200));
                        if !window.is_visible().unwrap_or(false) {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    });
                }
            }
            if let Some(window) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if tray_ok && close_to_tray_enabled(&handle) {
                            api.prevent_close();
                            if let Some(w) = handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                    }
                });
            }
            allow_previews(app.handle(), &extra_dirs);
            allow_dir(
                app.handle(),
                &scanner::effective_managed_dir(library_dir.as_deref()),
            );
            for path in &linked_dirs {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    allow_dir(app.handle(), parent);
                }
            }
            if watch_enabled {
                let _ = apply_watch(app.handle(), true, &extra_dirs);
            }
            // Affinity watcher: session-activate doc fonts while it runs.
            let affinity_app = app.handle().clone();
            std::thread::spawn(move || loop {
                affinity::tick(&affinity_app);
                std::thread::sleep(std::time::Duration::from_secs(2));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_autostart,
            get_autostart,
            scan_fonts,
            set_font_active,
            set_fonts_active,
            set_fonts_active_session,
            install_fonts,
            uninstall_font,
            list_trash,
            font_duplicates,
            affinity_connection,
            affinity_link,
            affinity_session_activate,
            online_catalogue,
            online_family,
            online_download,
            online_preview,
            online_clear_staging,
            online_hosts,
            restore_from_trash,
            delete_trash_entry,
            empty_trash,
            get_tags,
            set_tags,
            import_tags,
            get_collections,
            set_collection,
            delete_collection,
            rename_collection,
            export_font,
            export_fonts,
            export_font_face,
            export_font_face_renamed,
            export_font_collection,
            font_file_faces,
            export_font_subset,
            font_references,
            font_charmap,
            font_health,
            font_usage,
            font_usage_touch,
            font_usage_clear,
            organize_scan,
            organize_plan,
            organize_apply,
            bundle_export,
            bundle_inspect,
            bundle_import,
            audit_log,
            audit_clear,
            export_catalog,
            export_font_package,
            font_face_css,
            write_text_file,
            read_text_file,
            write_binary_file,
            get_features,
            get_favorites,
            set_favorite,
            get_ratings,
            set_rating,
            get_aliases,
            set_aliases,
            pinyin_keys,
            get_notes,
            set_note,
            play_sound,
            get_charset,
            check_glyphs,
            clear_glyph_cache,
            font_detail,
            fonts_with_char,
            set_ui_scale,
            get_prefs,
            set_prefs,
            get_settings,
            set_settings,
            default_library_dir,
            open_url,
            adobe_available,
            apply_font_in_app
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                revert_session_activations(app);
            }
        });
}
