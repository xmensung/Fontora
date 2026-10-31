//! 备份与迁移：把「这一台机器上的库」整个带走，换台机器接着用。
//!
//! 和 P7 的「项目字体包」不是一回事：那边的产物是给**别人**或**项目**用的
//! （字体 + CSS + 授权说明），这边是给**自己**用的 —— 产物能再读回来，
//! 把标签、集合、收藏、评分、别名、备注、开关设置原样接上。
//!
//! 一个 `*.zfontmigrate.zip` 里：
//!
//! ```text
//! manifest.json   格式标记 / 版本 / 导出时间 / 来源机器 / 内容摘要
//! library.json    用户资产 + 设置（不含注册表备份这类机器相关的缓存）
//! catalog.csv     清单（纯给人看，导入不读）
//! usage.json      使用统计，可选
//! fonts/…         字体文件本身，可选
//! ```
//!
//! 关键设计：所有用户资产都以**家族名**为键（`tags` / `collections` / `favorites` /
//! `ratings` / `aliases` / `notes` 都是），所以换机后路径全变了也照样对得上号。
//! 只有 `deactivated` / `linked` / `parked` 存的是路径，导入时逐条验存在性，
//! 失效的丢掉而不是留一堆指向空气的条目。

use crate::export::ExportRow;
use crate::store::AppState;
use crate::usage::UsageRow;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const FORMAT: &str = "zfontmanager-bundle";
const VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";
const LIBRARY: &str = "library.json";
const CATALOG: &str = "catalog.csv";
const USAGE: &str = "usage.json";
const FONTS: &str = "fonts/";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_default()
}

fn sorted(set: &HashSet<String>) -> Vec<String> {
    let mut list: Vec<String> = set.iter().cloned().collect();
    list.sort();
    list
}

// ---------------------------------------------------------------- 包内结构

/// 包里能装回去的那部分状态。
///
/// 刻意**不**包含 `registry_backup`：那是「卸载前把注册表值抄一份」的临时账本，
/// 跟具体机器绑定，搬到另一台机器上只会指向不存在的键。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BundleState {
    pub tags: HashMap<String, Vec<String>>,
    pub collections: HashMap<String, Vec<String>>,
    pub favorites: Vec<String>,
    pub ratings: HashMap<String, u8>,
    pub aliases: HashMap<String, Vec<String>>,
    pub notes: HashMap<String, String>,
    pub deactivated: Vec<String>,
    pub parked: HashMap<String, String>,
    pub linked: Vec<String>,
    pub settings: BundleSettings,
    pub prefs: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BundleSettings {
    pub extra_dirs: Vec<String>,
    pub watch_enabled: bool,
    pub auto_activate_imports: bool,
    pub library_dir: Option<String>,
    pub library_dir_enabled: bool,
    pub affinity_enabled: bool,
    pub affinity_deactivate_on_quit: bool,
    pub online_fonts_enabled: bool,
}

impl Default for BundleSettings {
    fn default() -> Self {
        Self {
            extra_dirs: Vec::new(),
            watch_enabled: false,
            auto_activate_imports: false,
            library_dir: None,
            library_dir_enabled: false,
            affinity_enabled: false,
            affinity_deactivate_on_quit: true,
            online_fonts_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BundleManifest {
    pub format: String,
    pub version: u32,
    pub app: String,
    pub exported_at: u64,
    pub machine: String,
    pub families: usize,
    pub faces: usize,
    pub includes_fonts: bool,
    pub includes_usage: bool,
    /// 包里确实带了多少个字体文件（读包时数出来的）。
    pub files: usize,
}

impl BundleState {
    fn from_state(state: &AppState) -> Self {
        let mut favorites: Vec<String> = state.favorites.iter().cloned().collect();
        favorites.sort();
        Self {
            tags: state.tags.clone(),
            collections: state.collections.clone(),
            favorites,
            ratings: state.ratings.clone(),
            aliases: state.aliases.clone(),
            notes: state.notes.clone(),
            deactivated: sorted(&state.deactivated),
            parked: state.parked.clone(),
            linked: sorted(&state.linked),
            settings: BundleSettings {
                extra_dirs: state.extra_dirs.clone(),
                watch_enabled: state.watch_enabled,
                auto_activate_imports: state.auto_activate_imports,
                library_dir: state.library_dir.clone(),
                library_dir_enabled: state.library_dir_enabled,
                affinity_enabled: state.affinity_enabled,
                affinity_deactivate_on_quit: state.affinity_deactivate_on_quit,
                online_fonts_enabled: state.online_fonts_enabled,
            },
            prefs: state.prefs.clone(),
        }
    }
}

// ---------------------------------------------------------------- 导出

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleExport {
    pub path: String,
    pub bytes: u64,
    pub families: usize,
    pub files: usize,
}

fn distinct_families(rows: &[ExportRow]) -> usize {
    rows.iter()
        .map(|r| r.family.as_str())
        .collect::<HashSet<_>>()
        .len()
}

/// 写一个迁移包。`rows` 已是补全过哈希的清单（拿它写 `catalog.csv` 和字体文件）。
pub fn write_bundle(
    dest: &str,
    state: &AppState,
    rows: &[ExportRow],
    usage_rows: &[UsageRow],
    with_files: bool,
    with_usage: bool,
) -> Result<BundleExport, String> {
    let bundle = BundleState::from_state(state);
    let manifest = BundleManifest {
        format: FORMAT.to_string(),
        version: VERSION,
        app: env!("CARGO_PKG_VERSION").to_string(),
        exported_at: now_secs(),
        machine: host_name(),
        families: distinct_families(rows),
        faces: rows.len(),
        includes_fonts: with_files,
        includes_usage: with_usage,
        files: 0,
    };

    let handle = fs::File::create(dest).map_err(|e| format!("无法创建迁移包：{e}"))?;
    let mut zip = zip::ZipWriter::new(handle);
    let entry = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let put = |zip: &mut zip::ZipWriter<fs::File>, name: &str, bytes: &[u8]| -> Result<(), String> {
        zip.start_file(name, entry)
            .map_err(|e| format!("写入迁移包失败：{e}"))?;
        zip.write_all(bytes)
            .map_err(|e| format!("写入迁移包失败：{e}"))
    };

    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    put(&mut zip, MANIFEST, &json)?;
    let json = serde_json::to_vec_pretty(&bundle).map_err(|e| e.to_string())?;
    put(&mut zip, LIBRARY, &json)?;
    put(&mut zip, CATALOG, crate::export::csv_text(rows, true).as_bytes())?;
    if with_usage {
        let json = serde_json::to_vec_pretty(usage_rows).map_err(|e| e.to_string())?;
        put(&mut zip, USAGE, &json)?;
    }

    let mut files = 0usize;
    if with_files {
        let stems = crate::export::file_stems(rows);
        // 同一个文件里的多个字重只读一次。
        let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (i, row) in rows.iter().enumerate() {
            groups.entry(row.path.as_str()).or_default().push(i);
        }
        for (path, indices) in groups {
            let Ok(data) = fs::read(path) else { continue };
            let faces = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
            for &i in &indices {
                let row = &rows[i];
                let (bytes, ext) = if faces <= 1 {
                    (data.clone(), row.ext().to_string())
                } else {
                    match crate::sfnt::read_face(&data, row.face_index) {
                        Ok(container) => (
                            crate::sfnt::build_sfnt(container.version, &container.tables),
                            crate::sfnt::extension_for(container.version).to_string(),
                        ),
                        // 某个 face 拆不出来不影响其它 face 进包。
                        Err(_) => continue,
                    }
                };
                put(&mut zip, &format!("{FONTS}{}.{ext}", stems[i]), &bytes)?;
                files += 1;
            }
        }
    }

    zip.finish().map_err(|e| format!("收尾压缩包失败：{e}"))?;
    let bytes = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    Ok(BundleExport {
        path: dest.to_string(),
        bytes,
        families: distinct_families(rows),
        files,
    })
}

// ---------------------------------------------------------------- 读包

fn open(path: &str) -> Result<zip::ZipArchive<fs::File>, String> {
    let file = fs::File::open(path).map_err(|e| format!("打不开迁移包：{e}"))?;
    zip::ZipArchive::new(file).map_err(|e| format!("这不是一个有效的 zip 包：{e}"))
}

fn read_json<T: serde::de::DeserializeOwned>(
    archive: &mut zip::ZipArchive<fs::File>,
    name: &str,
) -> Result<T, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| format!("迁移包里缺少 {name}"))?;
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|e| format!("读取 {name} 失败：{e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{name} 内容损坏：{e}"))
}

fn font_entry_count(archive: &mut zip::ZipArchive<fs::File>) -> usize {
    (0..archive.len())
        .filter(|&i| {
            archive
                .by_index_raw(i)
                .map(|f| {
                    let name = f.name().to_string();
                    name.starts_with(FONTS) && name.len() > FONTS.len()
                })
                .unwrap_or(false)
        })
        .count()
}

// ---------------------------------------------------------------- 导入

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMode {
    /// 只补空缺：本机已有的标签/评分/备注一律不动，集合取并集。
    Merge,
    /// 全部换成包里的：同名条目被覆盖。
    Replace,
}

impl MergeMode {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim().to_ascii_lowercase().as_str() {
            "merge" => Ok(Self::Merge),
            "replace" => Ok(Self::Replace),
            other => Err(format!("不认识的恢复方式：{other}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Replace => "replace",
        }
    }

    fn replaces(self) -> bool {
        matches!(self, Self::Replace)
    }
}

/// 包里的家底 —— 恢复前给界面摆一排数字。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleCounts {
    /// 打了标签的家族数。
    pub tags: usize,
    /// 标签总数（一个家族三个标签算三个）。
    pub tag_items: usize,
    pub collections: usize,
    pub favorites: usize,
    pub ratings: usize,
    pub aliases: usize,
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlan {
    pub manifest: BundleManifest,
    pub counts: BundleCounts,
    /// 包里有、本机暂时没有的家族名（最多列 24 个，全量数目另给）。
    pub unknown: Vec<String>,
    pub unknown_total: usize,
    /// 路径类数据在本机对不上的条数（这些不会导入）。
    pub stale_paths: usize,
}

fn count(entries: &HashMap<String, Vec<String>>) -> (usize, usize) {
    let families = entries.values().filter(|v| !v.is_empty()).count();
    let items = entries.values().map(|v| v.len()).sum();
    (families, items)
}

fn counts_of(bundle: &BundleState) -> BundleCounts {
    let (tags, tag_items) = count(&bundle.tags);
    let (aliases, _) = count(&bundle.aliases);
    BundleCounts {
        tags,
        tag_items,
        collections: bundle
            .collections
            .values()
            .filter(|members| !members.is_empty())
            .count(),
        favorites: bundle.favorites.iter().filter(|f| !f.trim().is_empty()).count(),
        ratings: bundle.ratings.values().filter(|&&v| valid_rating(v)).count(),
        aliases,
        notes: bundle
            .notes
            .values()
            .filter(|note| !note.trim().is_empty())
            .count(),
    }
}

fn mentioned_families(bundle: &BundleState) -> HashSet<String> {
    let mut names: HashSet<String> = HashSet::new();
    for map in [&bundle.tags, &bundle.aliases] {
        names.extend(map.keys().filter(|k| !k.trim().is_empty()).cloned());
    }
    names.extend(bundle.notes.keys().filter(|k| !k.trim().is_empty()).cloned());
    names.extend(bundle.favorites.iter().cloned());
    names.extend(bundle.ratings.keys().cloned());
    for members in bundle.collections.values() {
        names.extend(members.iter().cloned());
    }
    names.retain(|n| !n.trim().is_empty());
    names
}

fn stale_paths(bundle: &BundleState) -> usize {
    let gone = |p: &str| !p.trim().is_empty() && !Path::new(p).exists();
    bundle.deactivated.iter().filter(|p| gone(p)).count()
        + bundle.linked.iter().filter(|p| gone(p)).count()
        + bundle
            .parked
            .values()
            .filter(|p| gone(p))
            .count()
}

/// 先看包，不动任何东西。
pub fn inspect(path: &str, known: &HashSet<String>) -> Result<ImportPlan, String> {
    let mut archive = open(path)?;
    let mut manifest: BundleManifest = read_json(&mut archive, MANIFEST)?;
    if manifest.format != FORMAT {
        return Err("这不是字体管理器导出的迁移包".into());
    }
    if manifest.version > VERSION {
        return Err(format!(
            "这个包来自更新的版本（格式 {}），当前版本只认到 {}",
            manifest.version, VERSION
        ));
    }
    manifest.files = font_entry_count(&mut archive);
    let bundle: BundleState = read_json(&mut archive, LIBRARY)?;

    let mentioned = mentioned_families(&bundle);
    let mut unknown: Vec<String> = mentioned
        .iter()
        .filter(|n| !known.contains(*n))
        .cloned()
        .collect();
    unknown.sort();

    Ok(ImportPlan {
        counts: counts_of(&bundle),
        unknown_total: unknown.len(),
        unknown: unknown.into_iter().take(24).collect(),
        stale_paths: stale_paths(&bundle),
        manifest,
    })
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub mode: String,
    pub tags: usize,
    pub collections: usize,
    pub favorites: usize,
    pub ratings: usize,
    pub aliases: usize,
    pub notes: usize,
    /// 设置是否被包里的值覆盖（合并模式下本机设置不动）。
    pub settings: bool,
    pub usage: usize,
    pub fonts_written: usize,
    pub fonts_skipped: usize,
    pub stale_paths: usize,
    pub dropped_dirs: usize,
}

fn dedupe(values: &[String]) -> Vec<String> {    let mut out: Vec<String> = Vec::with_capacity(values.len());
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !out.iter().any(|v| v == value) {
            out.push(value.to_string());
        }
    }
    out
}

/// 标签 / 集合 / 别名这类「名字 → 字符串列表」的表。
fn merge_list_map(
    dst: &mut HashMap<String, Vec<String>>,
    src: &HashMap<String, Vec<String>>,
    replace: bool,
) {
    for (key, values) in src {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let clean = dedupe(values);
        if replace {
            if clean.is_empty() {
                dst.remove(key);
            } else {
                dst.insert(key.to_string(), clean);
            }
            continue;
        }
        let slot = dst.entry(key.to_string()).or_default();
        for value in clean {
            if !slot.contains(&value) {
                slot.push(value);
            }
        }
    }
}

fn valid_rating(value: u8) -> bool {
    (1..=5).contains(&value)
}

fn merge_paths(dst: &mut HashSet<String>, src: &[String]) -> usize {
    let mut dropped = 0;
    for path in src {
        if path.trim().is_empty() {
            continue;
        }
        if Path::new(path).exists() {
            dst.insert(path.clone());
        } else {
            dropped += 1;
        }
    }
    dropped
}

fn keep_dirs(dirs: &[String]) -> (Vec<String>, usize) {
    let clean = dedupe(dirs);
    let mut kept = Vec::with_capacity(clean.len());
    let mut dropped = 0;
    for dir in clean {
        if Path::new(&dir).is_dir() {
            kept.push(dir);
        } else {
            dropped += 1;
        }
    }
    (kept, dropped)
}

/// 把包里的东西并进当前状态，必要时解出字体文件。调用方负责落盘 `state.json`。
pub fn apply(path: &str, mode: MergeMode, state: &mut AppState, lib_dir: &Path) -> Result<ImportResult, String> {
    let mut archive = open(path)?;
    let manifest: BundleManifest = read_json(&mut archive, MANIFEST)?;
    if manifest.format != FORMAT {
        return Err("这不是字体管理器导出的迁移包".into());
    }
    if manifest.version > VERSION {
        return Err(format!(
            "这个包来自更新的版本（格式 {}），当前版本只认到 {}",
            manifest.version, VERSION
        ));
    }
    let bundle: BundleState = read_json(&mut archive, LIBRARY)?;
    let replace = mode.replaces();
    let counts = counts_of(&bundle);

    // 用户资产。替换模式下这几张表整个换掉，包内没提到的条目跟着消失。
    if replace {
        state.tags.clear();
        state.collections.clear();
        state.aliases.clear();
    }
    merge_list_map(&mut state.tags, &bundle.tags, replace);
    merge_list_map(&mut state.collections, &bundle.collections, replace);
    merge_list_map(&mut state.aliases, &bundle.aliases, replace);

    for name in dedupe(&bundle.favorites) {
        state.favorites.insert(name);
    }
    if replace {
        // 替换模式下，包内没提到的收藏要清掉，才算「换成包里的」。
        let keep: HashSet<String> = bundle
            .favorites
            .iter()
            .filter(|f| !f.trim().is_empty())
            .cloned()
            .collect();
        state.favorites.retain(|name| keep.contains(name));
    }

    for (family, value) in &bundle.ratings {
        if !valid_rating(*value) || family.trim().is_empty() {
            continue;
        }
        if replace {
            state.ratings.insert(family.clone(), *value);
        } else if !state.ratings.contains_key(family) {
            state.ratings.insert(family.clone(), *value);
        }
    }
    if replace {
        let keep: HashSet<String> = bundle
            .ratings
            .iter()
            .filter(|(_, v)| valid_rating(**v))
            .map(|(k, _)| k.clone())
            .collect();
        state.ratings.retain(|family, _| keep.contains(family));
    }

    for (family, note) in &bundle.notes {
        let family = family.trim();
        if family.is_empty() {
            continue;
        }
        let note = note.trim();
        if replace {
            if note.is_empty() {
                state.notes.remove(family);
            } else {
                state.notes.insert(family.to_string(), note.to_string());
            }
        } else if state.notes.get(family).map(|n| n.trim().is_empty()).unwrap_or(true) {
            if !note.is_empty() {
                state.notes.insert(family.to_string(), note.to_string());
            }
        }
    }

    // 路径类：只认本机真的有的那些。
    if replace {
        state.deactivated.clear();
        state.linked.clear();
        state.parked.clear();
    }
    let mut dropped = merge_paths(&mut state.deactivated, &bundle.deactivated);
    dropped += merge_paths(&mut state.linked, &bundle.linked);
    for (orig, parked) in &bundle.parked {
        if Path::new(parked).exists() {
            state.parked.insert(orig.clone(), parked.clone());
        } else {
            dropped += 1;
        }
    }

    // 设置：合并模式保留本机开关，只补空着的库目录；替换模式整套照搬。
    let mut dropped_dirs = 0usize;
    let (dirs, dropped_dir_count) = keep_dirs(&bundle.settings.extra_dirs);
    dropped_dirs += dropped_dir_count;
    if replace {
        state.extra_dirs = dirs;
    } else {
        let mut merged = state.extra_dirs.clone();
        for dir in dirs {
            if !merged.contains(&dir) {
                merged.push(dir);
            }
        }
        state.extra_dirs = merged;
    }
    let local_library_empty = state
        .library_dir
        .as_deref()
        .map(|d| d.trim().is_empty())
        .unwrap_or(true);
    if let Some(dir) = &bundle.settings.library_dir {
        let usable = !dir.trim().is_empty() && Path::new(dir).is_dir();
        if !usable {
            dropped_dirs += 1;
        } else if replace || local_library_empty {
            state.library_dir = Some(dir.clone());
            state.library_dir_enabled = bundle.settings.library_dir_enabled;
        }
    }
    let settings_applied = replace;
    if replace {
        state.watch_enabled = bundle.settings.watch_enabled;
        state.auto_activate_imports = bundle.settings.auto_activate_imports;
        state.affinity_enabled = bundle.settings.affinity_enabled;
        state.affinity_deactivate_on_quit = bundle.settings.affinity_deactivate_on_quit;
        state.online_fonts_enabled = bundle.settings.online_fonts_enabled;
        if !bundle.prefs.is_null() {
            state.prefs = bundle.prefs.clone();
        }
    }

    // 使用统计。
    let mut usage = 0usize;
    if manifest.includes_usage {
        if let Ok(rows) = read_json::<Vec<UsageRow>>(&mut archive, USAGE) {
            usage = crate::usage::merge_rows(&rows);
        }
    }

    // 字体文件。
    let mut written = 0usize;
    let mut skipped = 0usize;
    let has_files = manifest.includes_fonts || font_entry_count(&mut archive) > 0;
    if has_files {
        if let Err(e) = fs::create_dir_all(lib_dir) {
            return Err(format!("建不了字体库目录：{e}"));
        }
        for i in 0..archive.len() {
            let Ok(mut entry) = archive.by_index(i) else {
                continue;
            };
            let name = entry.name().to_string();
            let Some(base) = name.strip_prefix(FONTS) else {
                continue;
            };
            if base.is_empty() || base.contains('/') || base.contains('\\') {
                continue;
            }
            let target = lib_dir.join(base);
            if let Ok(meta) = fs::metadata(&target) {
                if meta.is_file() && meta.len() == entry.size() {
                    skipped += 1;
                    continue;
                }
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            if entry.read_to_end(&mut bytes).is_err() {
                continue;
            }
            match fs::write(&target, &bytes) {
                Ok(()) => written += 1,
                Err(_) => skipped += 1,
            }
        }
    }

    Ok(ImportResult {
        mode: mode.as_str().to_string(),
        tags: counts.tags,
        collections: counts.collections,
        favorites: counts.favorites,
        ratings: counts.ratings,
        aliases: counts.aliases,
        notes: counts.notes,
        settings: settings_applied,
        usage,
        fonts_written: written,
        fonts_skipped: skipped,
        stale_paths: dropped,
        dropped_dirs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zfm-migrate-{tag}-{}-{}",
            std::process::id(),
            now_secs()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(values: &[String]) -> Vec<&str> {
        values.iter().map(|v| v.as_str()).collect()
    }

    fn state_with_assets() -> AppState {
        let mut state = AppState::default();
        state.tags.insert("思源黑体".into(), vec!["正文".into(), "品牌".into()]);
        state.tags.insert("Inter".into(), vec!["UI".into()]);
        state
            .collections
            .insert("项目 A".into(), vec!["思源黑体".into()]);
        state.favorites.insert("Inter".into());
        state.ratings.insert("思源黑体".into(), 5);
        state.aliases.insert("思源黑体".into(), vec!["syht".into()]);
        state.notes.insert("Inter".into(), "正文备选".into());
        state.watch_enabled = true;
        state
    }

    #[test]
    fn round_trip_restores_every_asset() {
        let dir = temp_dir("round");
        let dest = dir.join("bundle.zip");
        let dest_text = dest.to_string_lossy().to_string();
        let src = state_with_assets();
        let out = write_bundle(&dest_text, &src, &[], &[], false, false).unwrap();
        assert_eq!(out.families, 0);

        let mut known = HashSet::new();
        known.insert("思源黑体".to_string());
        let plan = inspect(&dest_text, &known).unwrap();
        assert_eq!(plan.counts.tags, 2);
        assert_eq!(plan.counts.tag_items, 3);
        assert_eq!(plan.counts.collections, 1);
        assert_eq!(plan.counts.favorites, 1);
        assert_eq!(plan.counts.ratings, 1);
        assert_eq!(plan.counts.aliases, 1);
        assert_eq!(plan.counts.notes, 1);
        assert_eq!(plan.unknown, vec!["Inter".to_string()]);
        assert_eq!(plan.unknown_total, 1);

        let mut target = AppState::default();
        let result = apply(&dest_text, MergeMode::Merge, &mut target, &dir).unwrap();
        assert_eq!(result.tags, 2);
        assert_eq!(names(&target.tags["思源黑体"]), vec!["正文", "品牌"]);
        assert_eq!(names(&target.tags["Inter"]), vec!["UI"]);
        assert_eq!(names(&target.collections["项目 A"]), vec!["思源黑体"]);
        assert!(target.favorites.contains("Inter"));
        assert_eq!(target.ratings["思源黑体"], 5);
        assert_eq!(names(&target.aliases["思源黑体"]), vec!["syht"]);
        assert_eq!(target.notes["Inter"], "正文备选");
        // 合并模式不碰本机开关。
        assert!(!target.watch_enabled);
        assert!(!result.settings);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn merge_keeps_local_values_and_replace_overwrites_them() {
        let dir = temp_dir("modes");
        let dest = dir.join("bundle.zip");
        let text = dest.to_string_lossy().to_string();
        write_bundle(&text, &state_with_assets(), &[], &[], false, false).unwrap();

        let mut local = AppState::default();
        local.ratings.insert("思源黑体".into(), 2);
        local.notes.insert("Inter".into(), "本机备注".into());
        local.tags.insert("思源黑体".into(), vec!["本机标签".into()]);
        let merged = apply(&text, MergeMode::Merge, &mut local, &dir).unwrap();
        assert_eq!(merged.mode, "merge");
        assert_eq!(local.ratings["思源黑体"], 2, "合并模式不该覆盖已有评分");
        assert_eq!(local.notes["Inter"], "本机备注");
        // 集合式字段取并集：本机的标签留着，包里的补进来。
        assert_eq!(names(&local.tags["思源黑体"]), vec!["本机标签", "正文", "品牌"]);

        let mut replaced = AppState::default();
        replaced.ratings.insert("思源黑体".into(), 2);
        replaced.notes.insert("Inter".into(), "本机备注".into());
        replaced.tags.insert("思源黑体".into(), vec!["本机标签".into()]);
        replaced.tags.insert("只在本地".into(), vec!["孤立".into()]);
        replaced.favorites.insert("本地收藏".into());
        let done = apply(&text, MergeMode::Replace, &mut replaced, &dir).unwrap();
        assert_eq!(done.mode, "replace");
        assert_eq!(replaced.ratings["思源黑体"], 5);
        assert_eq!(replaced.notes["Inter"], "正文备选");
        assert_eq!(names(&replaced.tags["思源黑体"]), vec!["正文", "品牌"]);
        assert!(!replaced.tags.contains_key("只在本地"), "替换模式清掉包内没有的标签");
        assert!(!replaced.favorites.contains("本地收藏"));
        assert!(replaced.watch_enabled, "替换模式连开关一起还原");
        assert!(done.settings);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dead_paths_are_dropped_not_imported() {
        let dir = temp_dir("paths");
        let dest = dir.join("bundle.zip");
        let text = dest.to_string_lossy().to_string();
        let mut src = AppState::default();
        src.deactivated.insert("C:\\nowhere\\gone.ttf".into());
        src.linked.insert("C:\\nowhere\\gone2.ttf".into());
        src.extra_dirs.push("C:\\nowhere\\fonts".into());
        write_bundle(&text, &src, &[], &[], false, false).unwrap();

        let plan = inspect(&text, &HashSet::new()).unwrap();
        assert_eq!(plan.stale_paths, 2);

        let mut target = AppState::default();
        let result = apply(&text, MergeMode::Merge, &mut target, &dir).unwrap();
        assert!(target.deactivated.is_empty());
        assert!(target.linked.is_empty());
        assert!(target.extra_dirs.is_empty());
        assert_eq!(result.stale_paths, 2);
        assert_eq!(result.dropped_dirs, 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_zip_that_is_not_ours_is_refused() {
        let dir = temp_dir("foreign");
        let dest = dir.join("random.zip");
        let handle = fs::File::create(&dest).unwrap();
        let mut zip = zip::ZipWriter::new(handle);
        let entry = zip::write::SimpleFileOptions::default();
        zip.start_file("hello.txt", entry).unwrap();
        zip.write_all(b"hi").unwrap();
        zip.finish().unwrap();
        let err = inspect(&dest.to_string_lossy(), &HashSet::new()).unwrap_err();
        assert!(err.contains("缺少 manifest.json"), "实际：{err}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_bundle_with_a_bad_library_is_refused() {
        let dir = temp_dir("badlib");
        let dest = dir.join("bad.zip");
        let handle = fs::File::create(&dest).unwrap();
        let mut zip = zip::ZipWriter::new(handle);
        let entry = zip::write::SimpleFileOptions::default();
        zip.start_file(MANIFEST, entry).unwrap();
        zip.write_all(
            br#"{"format":"zfontmanager-bundle","version":1,"app":"0","exportedAt":0,"machine":"","families":0,"faces":0,"includesFonts":false,"includesUsage":false,"files":0}"#,
        )
        .unwrap();
        zip.start_file(LIBRARY, entry).unwrap();
        zip.write_all(b"{ not json").unwrap();
        zip.finish().unwrap();
        let err = inspect(&dest.to_string_lossy(), &HashSet::new()).unwrap_err();
        assert!(err.contains("内容损坏"), "实际：{err}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn fonts_ride_along_and_land_in_the_library() {
        let dir = temp_dir("fonts");
        let source = dir.join("local");
        fs::create_dir_all(&source).unwrap();
        let font = source.join("tiny.ttf");
        fs::write(&font, vec![b'x'; 512]).unwrap();
        let rows = vec![ExportRow {
            family: "Tiny".into(),
            style: "Regular".into(),
            path: font.to_string_lossy().to_string(),
            format: "ttf".into(),
            file_size: 512,
            ..Default::default()
        }];
        let dest = dir.join("bundle.zip");
        let text = dest.to_string_lossy().to_string();
        let out = write_bundle(&text, &AppState::default(), &rows, &[], true, false).unwrap();
        assert_eq!(out.files, 1);

        let lib = dir.join("library");
        let mut target = AppState::default();
        let result = apply(&text, MergeMode::Merge, &mut target, &lib).unwrap();
        assert_eq!(result.fonts_written, 1);
        let landed = fs::read_dir(&lib).unwrap().count();
        assert_eq!(landed, 1);

        // 再导一次：同一个文件已经在库里，跳过而不是再写一份。
        let again = apply(&text, MergeMode::Merge, &mut AppState::default(), &lib).unwrap();
        assert_eq!(again.fonts_written, 0);
        assert_eq!(again.fonts_skipped, 1);
        assert_eq!(fs::read_dir(&lib).unwrap().count(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_merge_modes_are_rejected() {
        assert_eq!(MergeMode::parse(" MERGE ").unwrap(), MergeMode::Merge);
        assert_eq!(MergeMode::parse("replace").unwrap(), MergeMode::Replace);
        assert!(MergeMode::parse("append").is_err());
    }

    /// 拿系统里的真 TTC 走一遍：包里应该是拆开的单字体文件，不是整个 ttc。
    #[test]
    #[ignore]
    fn a_real_collection_is_split_into_separate_files() {
        let source = Path::new("C:\\Windows\\Fonts\\msyh.ttc");
        if !source.exists() {
            return;
        }
        let data = fs::read(source).unwrap();
        let faces = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        assert!(faces >= 2, "msyh.ttc 应该有至少两个 face");

        let rows: Vec<ExportRow> = (0..faces)
            .map(|i| ExportRow {
                family: format!("MSYH{i}"),
                style: "Regular".into(),
                path: source.to_string_lossy().to_string(),
                face_index: i,
                format: "ttf".into(),
                file_size: data.len() as u64,
                ..Default::default()
            })
            .collect();

        let dir = temp_dir("realttc");
        let dest = dir.join("bundle.zip");
        let text = dest.to_string_lossy().to_string();
        let out = write_bundle(&text, &AppState::default(), &rows, &[], true, false).unwrap();
        assert_eq!(out.files as u32, faces);

        let mut archive = open(&text).unwrap();
        assert_eq!(font_entry_count(&mut archive), faces as usize);
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            if !entry.name().starts_with(FONTS) {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            assert_eq!(
                ttf_parser::fonts_in_collection(&bytes).unwrap_or(1),
                1,
                "包里的每个文件都该是单 face"
            );
            assert!(ttf_parser::Face::parse(&bytes, 0).is_ok());
        }
        fs::remove_dir_all(&dir).ok();
    }
}
