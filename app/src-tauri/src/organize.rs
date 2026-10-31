//! 文件夹规则：按厂商 / 语言 / 格式 / 分类把字体文件分到子文件夹里。
//!
//! 三件事是有意为之的：
//! 1. **先算方案，再动手**。`build_plan` 只读文件系统、不写一个字节，界面把
//!    每一对「源 → 目标」先摆出来给人看，点确认才走 `apply`。
//! 2. **系统字体永远不碰**。`C:\Windows\Fonts` 下的文件归 Windows 管，
//!    搬走会直接让系统字体失效。
//! 3. **默认复制，不默认移动**。移动是不可逆的，除了界面上那句红字，这里也把
//!    「正在启用中的文件不许移动」当成硬规则 —— 注册表指着原路径，搬走就断。

use crate::font_types::{Classification, FontFormat, FontSource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// 前端把库里的文件摊平后传过来（一个字重一条，同路径的会被后端合并）。
/// 也可以由 `scan_items` 直接扫任意文件夹生成。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeItem {
    pub path: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub foundry: Option<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub classification: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeEntry {
    pub path: String,
    pub target: String,
    /// 目标子文件夹名（界面上按它分组）。
    pub folder: String,
    /// copy / move / link / skip
    pub action: String,
    pub reason: String,
    pub size: u64,
    pub family: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCount {
    pub name: String,
    pub count: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizePlan {
    pub dest: String,
    pub key: String,
    pub mode: String,
    pub entries: Vec<OrganizeEntry>,
    /// 真的会动的文件数。
    pub ready: usize,
    pub skipped: usize,
    pub bytes: u64,
    /// 因为目标已有同名文件而改名/跳过的数量。
    pub collisions: usize,
    pub folders: Vec<FolderCount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeOutcome {
    pub family: String,
    pub path: String,
    pub target: String,
    /// ok / failed
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeResult {
    pub dest: String,
    pub done: usize,
    pub failed: usize,
    pub bytes: u64,
    pub outcomes: Vec<OrganizeOutcome>,
}

/// Windows 文件名里不能出现的字符，顺便把结尾的点和空格去掉
/// （`Foo.` 这种目录名在资源管理器里建不出来）。
pub fn safe_folder(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            continue;
        }
        out.push(ch);
    }
    let trimmed = out.trim().trim_matches('.').trim();
    let mut name: String = trimmed.chars().take(60).collect();
    if name.is_empty() {
        name = "Unknown".to_string();
    }
    name
}

const SCRIPT_ORDER: &[(&str, &str)] = &[
    ("cjk", "CJK"),
    ("korean", "Korean"),
    ("arabic", "Arabic"),
    ("hebrew", "Hebrew"),
    ("devanagari", "Devanagari"),
    ("thai", "Thai"),
    ("cyrillic", "Cyrillic"),
    ("greek", "Greek"),
    ("vietnamese", "Vietnamese"),
    ("latin", "Latin"),
];

/// 一款字体可能同时覆盖好几种文字，归档按「最有辨识度的那一种」放：
/// 中文排前面，拉丁永远垫底 —— 否则每款字体都会掉进 Latin 里。
pub fn language_of(scripts: &[String]) -> String {
    for (key, label) in SCRIPT_ORDER {
        if scripts.iter().any(|s| s.eq_ignore_ascii_case(key)) {
            return label.to_string();
        }
    }
    "Unknown".to_string()
}

pub fn format_of(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "otf" => "OTF".into(),
        "ttf" => "TTF".into(),
        "woff" => "WOFF".into(),
        "woff2" => "WOFF2".into(),
        "" | "unknown" => "Unknown".into(),
        other => safe_folder(&other.to_ascii_uppercase()),
    }
}

pub fn classification_of(classification: &str) -> String {
    match classification.trim().to_ascii_lowercase().as_str() {
        "serif" => "Serif".into(),
        "sans" => "Sans".into(),
        "mono" => "Mono".into(),
        "display" => "Display".into(),
        "script" => "Script".into(),
        _ => "Unknown".into(),
    }
}

pub fn folder_for(item: &OrganizeItem, key: &str) -> String {
    match key {
        "vendor" => safe_folder(item.foundry.as_deref().unwrap_or("")),
        "language" => language_of(&item.scripts),
        "format" => format_of(&item.format),
        "classification" => classification_of(&item.classification),
        _ => "All".to_string(),
    }
}

/// 路径是否落在某个目录里。按分隔符边界判断 —— 纯字符串前缀会把
/// `E:\字体重新分类` 当成 `E:\字体` 的子目录，那是错的。
fn under(path: &Path, dir: &Path) -> bool {
    let p = path.to_string_lossy().to_lowercase().replace('/', "\\");
    let d = dir.to_string_lossy().to_lowercase().replace('/', "\\");
    let d = d.trim_end_matches('\\');
    !d.is_empty() && (p == d || p.starts_with(&format!("{d}\\")))
}

fn format_name(format: FontFormat) -> &'static str {
    match format {
        FontFormat::Otf => "otf",
        FontFormat::Ttf => "ttf",
        FontFormat::Woff => "woff",
        FontFormat::Woff2 => "woff2",
        FontFormat::Unknown => "unknown",
    }
}

fn classification_name(classification: Classification) -> &'static str {
    match classification {
        Classification::Serif => "serif",
        Classification::Sans => "sans",
        Classification::Mono => "mono",
        Classification::Display => "display",
        Classification::Script => "script",
        Classification::Unknown => "unknown",
    }
}

/// 扫描任意文件夹里的字体文件，产出可以直接交给 `build_plan` 的清单。
///
/// 只读，不写任何东西。`C:\Windows\Fonts` 下的文件标成 system，和库里
/// 一样会被跳过；「是否已安装」按注册表登记判断，不是按目录猜。
pub fn scan_items(dir: &str) -> Result<Vec<OrganizeItem>, String> {
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        return Err("先选一个来源文件夹".into());
    }
    let root = Path::new(trimmed);
    if !root.is_dir() {
        return Err("这个路径不是文件夹".into());
    }

    let registered = crate::registry::registered_paths();
    let system_root = Path::new("C:\\Windows\\Fonts");
    let mut out: Vec<OrganizeItem> = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() || !crate::parser::is_font_file(entry.path()) {
            continue;
        }
        let path = entry.path();
        let is_system = under(path, system_root);
        let source = if is_system {
            FontSource::System
        } else {
            FontSource::User
        };
        let faces = crate::parser::parse_font_file(path, source);
        let Some(first) = faces.first() else {
            continue;
        };
        let path_str = path.to_string_lossy().into_owned();
        // 一个文件可能装好几个字重（TTC），文字覆盖取并集，否则语言归档会偏。
        let mut scripts: Vec<String> = Vec::new();
        for face in &faces {
            for script in &face.scripts {
                if !scripts.iter().any(|s| s.eq_ignore_ascii_case(script)) {
                    scripts.push(script.clone());
                }
            }
        }
        let size = entry
            .metadata()
            .map(|m| m.len())
            .unwrap_or(first.file_size);
        out.push(OrganizeItem {
            path: path_str.clone(),
            family: first.family.clone(),
            foundry: first.foundry.clone(),
            scripts,
            format: format_name(first.format).to_string(),
            classification: classification_name(first.classification).to_string(),
            size,
            source: if is_system { "system" } else { "user" }.to_string(),
            active: is_system || registered.contains(&crate::registry::normalize(&path_str)),
        });
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// 目标目录不能是系统目录，否则「整理字体」会变成「弄坏 Windows」。
fn check_dest(dest: &str) -> Result<PathBuf, String> {
    let trimmed = dest.trim();
    if trimmed.is_empty() {
        return Err("先选一个目标文件夹".into());
    }
    let path = PathBuf::from(trimmed);
    if path.is_file() {
        return Err("目标是一个文件，不是文件夹".into());
    }
    if let Ok(windir) = std::env::var("WINDIR") {
        if under(&path, Path::new(&windir)) {
            return Err("不能归档到 Windows 系统目录".into());
        }
    }
    Ok(path)
}

/// 同名冲突时往后找 `名称 (2).ttf`、`名称 (3).ttf`……最多试到 999。
fn unique_target(dir: &Path, name: &str, taken: &HashSet<String>) -> (PathBuf, bool) {
    let candidate = dir.join(name);
    let key = candidate.to_string_lossy().to_lowercase();
    if !candidate.exists() && !taken.contains(&key) {
        return (candidate, false);
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..=999 {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        let key = candidate.to_string_lossy().to_lowercase();
        if !candidate.exists() && !taken.contains(&key) {
            return (candidate, true);
        }
    }
    (dir.join(name), true)
}

/// 只算不改：每一对源 → 目标都摆出来，跳过的写清原因。
pub fn build_plan(
    items: &[OrganizeItem],
    dest: &str,
    key: &str,
    mode: &str,
) -> Result<OrganizePlan, String> {
    let mode = match mode {
        "move" | "link" => mode,
        _ => "copy",
    };
    let dest_path = check_dest(dest)?;
    let dest_str = dest_path.to_string_lossy().into_owned();

    let mut seen: HashSet<String> = HashSet::new();
    let mut taken: HashSet<String> = HashSet::new();
    let mut entries: Vec<OrganizeEntry> = Vec::new();
    let mut collisions = 0usize;

    for item in items {
        let source = PathBuf::from(&item.path);
        let entry_key = source.to_string_lossy().to_lowercase();
        // 同一个文件被多个字重带进来时只处理一次。
        if !seen.insert(entry_key) {
            continue;
        }
        let folder = folder_for(item, key);
        let mut entry = OrganizeEntry {
            path: item.path.clone(),
            target: String::new(),
            folder: folder.clone(),
            action: mode.to_string(),
            reason: String::new(),
            size: item.size,
            family: item.family.clone(),
        };

        let skip = |mut e: OrganizeEntry, reason: &str| {
            e.action = "skip".to_string();
            e.reason = reason.to_string();
            e
        };

        if item.source == "system" {
            entries.push(skip(entry, "系统字体，不动"));
            continue;
        }
        if !source.is_file() {
            entries.push(skip(entry, "文件不见了"));
            continue;
        }
        if mode == "move" && item.active {
            entries.push(skip(entry, "正在启用，先停用再移动"));
            continue;
        }

        let Some(name) = source.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            entries.push(skip(entry, "文件名读不出来"));
            continue;
        };
        let dir = dest_path.join(&folder);
        let plain = dir.join(&name);
        // 已经在这层目录里（对同一个库重复执行规则）：原地不动，别给个 (2)。
        if plain == source {
            entries.push(skip(entry, "已经在这个位置"));
            continue;
        }
        // 目标已经有同名同大小的文件：八成就是上一次复制过去的，别再塞一份 (2)。
        if let Ok(meta) = std::fs::metadata(&plain) {
            if meta.is_file() && meta.len() == item.size {
                entries.push(skip(entry, "目标已有同名同大小的文件"));
                continue;
            }
        }
        let (target, renamed) = unique_target(&dir, &name, &taken);
        if renamed {
            collisions += 1;
        }
        taken.insert(target.to_string_lossy().to_lowercase());
        entry.target = target.to_string_lossy().into_owned();
        entries.push(entry);
    }

    let mut folders: Vec<FolderCount> = Vec::new();
    for entry in entries.iter().filter(|e| e.action != "skip") {
        match folders.iter_mut().find(|f| f.name == entry.folder) {
            Some(found) => {
                found.count += 1;
                found.bytes += entry.size;
            }
            None => folders.push(FolderCount {
                name: entry.folder.clone(),
                count: 1,
                bytes: entry.size,
            }),
        }
    }
    folders.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));

    let ready: Vec<&OrganizeEntry> = entries.iter().filter(|e| e.action != "skip").collect();
    let ready_count = ready.len();
    let bytes = ready.iter().map(|e| e.size).sum();
    let skipped = entries.len() - ready_count;

    Ok(OrganizePlan {
        dest: dest_str,
        key: key.to_string(),
        mode: mode.to_string(),
        entries,
        ready: ready_count,
        skipped,
        bytes,
        collisions,
        folders,
    })
}

fn move_file(source: &Path, target: &Path) -> Result<(), String> {
    if std::fs::rename(source, target).is_ok() {
        return Ok(());
    }
    // 跨盘：先拷再删，删之前确认拷出来的字节数对得上。
    std::fs::copy(source, target).map_err(|e| e.to_string())?;
    let same = std::fs::metadata(source).ok().map(|m| m.len())
        == std::fs::metadata(target).ok().map(|m| m.len());
    if !same {
        return Err("复制后大小不一致，源文件保留".into());
    }
    std::fs::remove_file(source).map_err(|e| format!("复制完成但源文件删不掉：{e}"))
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::copy(source, target).map_err(|e| e.to_string())?;
    let src_len = std::fs::metadata(source).ok().map(|m| m.len());
    let dst_len = std::fs::metadata(target).ok().map(|m| m.len());
    if src_len.is_some() && src_len != dst_len {
        let _ = std::fs::remove_file(target);
        return Err("复制后大小不一致，已撤回".into());
    }
    Ok(())
}

/// 真的动手。重算一遍方案（防止预览之后文件又变了），逐个文件处理，
/// 单个失败不影响其它的，最后一起报。
pub fn apply(
    items: &[OrganizeItem],
    dest: &str,
    key: &str,
    mode: &str,
) -> Result<OrganizeResult, String> {
    let plan = build_plan(items, dest, key, mode)?;
    let mut outcomes = Vec::new();
    let mut done = 0usize;
    let mut failed = 0usize;
    let mut bytes = 0u64;

    for entry in plan.entries.iter().filter(|e| e.action != "skip") {
        let source = PathBuf::from(&entry.path);
        let target = PathBuf::from(&entry.target);
        let mut outcome = OrganizeOutcome {
            family: entry.family.clone(),
            path: entry.path.clone(),
            target: entry.target.clone(),
            status: "ok".to_string(),
            message: String::new(),
        };
        let result = (|| -> Result<(), String> {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("建文件夹失败：{e}"))?;
            }
            if target.exists() {
                return Err("目标文件已存在".into());
            }
            match entry.action.as_str() {
                "move" => move_file(&source, &target),
                "link" => std::fs::hard_link(&source, &target)
                    .map_err(|e| format!("建链接失败（可能不在同一个盘）：{e}")),
                _ => copy_file(&source, &target),
            }
        })();
        match result {
            Ok(()) => {
                done += 1;
                bytes += entry.size;
            }
            Err(message) => {
                failed += 1;
                outcome.status = "failed".to_string();
                outcome.message = message;
            }
        }
        outcomes.push(outcome);
    }

    Ok(OrganizeResult {
        dest: plan.dest,
        done,
        failed,
        bytes,
        outcomes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &Path, family: &str, foundry: Option<&str>) -> OrganizeItem {
        OrganizeItem {
            path: path.to_string_lossy().into_owned(),
            family: family.to_string(),
            foundry: foundry.map(str::to_string),
            scripts: vec!["latin".into(), "cjk".into()],
            format: "ttf".into(),
            classification: "serif".into(),
            size: 100,
            source: "user".into(),
            active: false,
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zfm-organize-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn folder_names_are_filesystem_safe() {
        assert_eq!(safe_folder("Adobe Systems Inc."), "Adobe Systems Inc");
        assert_eq!(safe_folder("a/b:c*d"), "abcd");
        assert_eq!(safe_folder("   "), "Unknown");
        assert_eq!(safe_folder("foo."), "foo");
    }

    #[test]
    fn cjk_wins_over_latin() {
        // 拉丁永远垫底：会写 ABC 的字体太多了，按它归档等于没归档。
        assert_eq!(language_of(&["cyrillic".to_string(), "latin".to_string()]), "Cyrillic");
        assert_eq!(language_of(&["latin".to_string(), "cjk".to_string()]), "CJK");
        assert_eq!(language_of(&["latin".to_string()]), "Latin");
        assert_eq!(language_of(&[]), "Unknown");
    }

    #[test]
    fn formats_and_classes_are_normalised() {
        assert_eq!(format_of("OTF"), "OTF");
        assert_eq!(format_of("woff2"), "WOFF2");
        assert_eq!(format_of(""), "Unknown");
        assert_eq!(classification_of("sans"), "Sans");
        assert_eq!(classification_of("nonsense"), "Unknown");
    }

    #[test]
    fn path_boundary_is_checked_not_just_a_prefix() {
        assert!(under(Path::new("E:\\字体\\a.ttf"), Path::new("E:\\字体")));
        assert!(
            !under(Path::new("E:\\字体重新分类\\a.ttf"), Path::new("E:\\字体")),
            "E:\\字体重新分类 不是 E:\\字体 的子目录"
        );
        assert!(under(Path::new("E:\\字体\\a\\b.ttf"), Path::new("E:\\字体\\")));
        assert!(under(Path::new("C:\\WINDOWS\\FONTS\\a.ttf"), Path::new("C:\\Windows\\Fonts")));
    }

    #[test]
    fn scanning_a_folder_needs_a_real_folder() {
        assert!(scan_items("").is_err());
        assert!(scan_items("E:\\这个路径肯定不存在\\zzz").is_err());
        let dir = scratch("scan-empty");
        std::fs::write(dir.join("readme.txt"), b"not a font").unwrap();
        let items = scan_items(dir.to_str().unwrap()).unwrap();
        assert!(items.is_empty(), "非字体文件不该进清单");
        let _ = fs_cleanup(&dir);
    }

    #[test]
    #[ignore]
    fn scan_items_on_a_real_folder() {
        // 用一个真带字体的目录验证解析：ZFM_TEST_DIR=E:\字体 cargo test -- --ignored
        let dir = std::env::var("ZFM_TEST_DIR").expect("set ZFM_TEST_DIR to a folder with fonts");
        let items = scan_items(&dir).unwrap();
        assert!(!items.is_empty(), "目录里应该有字体文件");
        let no_foundry = items.iter().filter(|i| i.foundry.is_none()).count();
        let active = items.iter().filter(|i| i.active).count();
        println!(
            "{} 个文件 / {} 个无厂商信息 / {} 个已安装；示例 {:?}",
            items.len(),
            no_foundry,
            active,
            items.first().map(|i| (&i.family, &i.format, &i.classification, &i.source))
        );
    }

    #[test]
    fn system_fonts_are_never_touched() {
        let dir = scratch("system");
        let file = dir.join("a.ttf");
        std::fs::write(&file, b"font").unwrap();
        let mut it = item(&file, "Arial", Some("Monotype"));
        it.source = "system".into();
        let plan = build_plan(&[it], dir.to_str().unwrap(), "vendor", "copy").unwrap();
        assert_eq!(plan.ready, 0);
        assert_eq!(plan.entries[0].action, "skip");
        assert!(plan.entries[0].reason.contains("系统"));
    }

    #[test]
    fn active_files_are_not_moved() {
        let dir = scratch("active");
        let file = dir.join("b.ttf");
        std::fs::write(&file, b"font").unwrap();
        let mut it = item(&file, "B", Some("Foundry"));
        it.active = true;
        let plan = build_plan(&[it.clone()], dir.to_str().unwrap(), "vendor", "move").unwrap();
        assert_eq!(plan.ready, 0);
        it.active = false;
        let plan = build_plan(&[it], dir.to_str().unwrap(), "vendor", "copy").unwrap();
        assert_eq!(plan.ready, 1);
    }

    #[test]
    fn copy_lands_in_the_vendor_folder() {
        let dir = scratch("copy");
        let file = dir.join("c.ttf");
        std::fs::write(&file, b"0123456789").unwrap();
        let plan = build_plan(&[item(&file, "C", Some("Adobe"))], dir.to_str().unwrap(), "vendor", "copy")
            .unwrap();
        assert_eq!(plan.folders.len(), 1);
        assert_eq!(plan.folders[0].name, "Adobe");
        let result = apply(&[item(&file, "C", Some("Adobe"))], dir.to_str().unwrap(), "vendor", "copy")
            .unwrap();
        assert_eq!(result.done, 1, "{:?}", result.outcomes);
        assert!(dir.join("Adobe").join("c.ttf").is_file());
        assert!(file.is_file(), "复制模式不能动源文件");
        assert_eq!(plan.entries[0].target, dir.join("Adobe").join("c.ttf").to_string_lossy());
    }

    #[test]
    fn moving_removes_the_source() {
        let dir = scratch("move");
        let file = dir.join("d.ttf");
        std::fs::write(&file, b"0123456789").unwrap();
        let result = apply(&[item(&file, "D", Some("Google"))], dir.to_str().unwrap(), "vendor", "move")
            .unwrap();
        assert_eq!(result.done, 1, "{:?}", result.outcomes);
        assert!(!file.exists());
        assert!(dir.join("Google").join("d.ttf").is_file());
    }

    #[test]
    fn hardlink_points_at_the_same_bytes() {
        let dir = scratch("link");
        let file = dir.join("e.ttf");
        std::fs::write(&file, b"0123456789").unwrap();
        let result = apply(&[item(&file, "E", Some("Fontworks"))], dir.to_str().unwrap(), "vendor", "link")
            .unwrap();
        assert_eq!(result.done, 1, "{:?}", result.outcomes);
        let link = dir.join("Fontworks").join("e.ttf");
        assert_eq!(std::fs::read(&link).unwrap(), b"0123456789");
        assert!(file.is_file());
    }

    #[test]
    fn same_name_collision_gets_a_suffix() {
        let dir = scratch("collide");
        let file = dir.join("f.ttf");
        std::fs::write(&file, b"111111").unwrap();
        let plan = build_plan(&[item(&file, "F", Some("Acme"))], dir.to_str().unwrap(), "vendor", "copy")
            .unwrap();
        let first = PathBuf::from(&plan.entries[0].target);
        std::fs::create_dir_all(first.parent().unwrap()).unwrap();
        std::fs::write(&first, b"already-here").unwrap();
        let plan = build_plan(&[item(&file, "F", Some("Acme"))], dir.to_str().unwrap(), "vendor", "copy")
            .unwrap();
        assert_eq!(plan.collisions, 1);
        assert!(plan.entries[0].target.ends_with("f (2).ttf"), "{}", plan.entries[0].target);
    }

    #[test]
    fn copying_twice_does_not_make_a_second_copy() {
        let dir = scratch("twice");
        let file = dir.join("j.ttf");
        // item() 里写的是 100 字节，这里也写 100 字节，才能走到「同大小」分支。
        std::fs::write(&file, vec![b'x'; 100]).unwrap();
        let dest = dir.to_string_lossy().into_owned();
        super::apply(&[item(&file, "J", Some("Acme"))], &dest, "vendor", "copy").unwrap();
        let again = build_plan(&[item(&file, "J", Some("Acme"))], &dest, "vendor", "copy").unwrap();
        assert_eq!(again.ready, 0, "第二次不该再复制一份");
        assert!(again.entries[0].reason.contains("同名同大小"));
        let _ = fs_cleanup(&dir);
    }

    #[test]
    fn one_file_listed_twice_is_planned_once() {
        let dir = scratch("dupe");
        let file = dir.join("g.ttc");
        std::fs::write(&file, b"font").unwrap();
        let a = item(&file, "G1", Some("X"));
        let b = item(&file, "G2", Some("X"));
        let plan = build_plan(&[a, b], dir.to_str().unwrap(), "vendor", "copy").unwrap();
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.ready, 1);
    }

    #[test]
    fn destination_must_not_be_a_file_or_system_dir() {
        let dir = scratch("dest");
        let file = dir.join("h.ttf");
        std::fs::write(&file, b"font").unwrap();
        let err = build_plan(&[item(&file, "H", None)], file.to_str().unwrap(), "vendor", "copy")
            .unwrap_err();
        assert!(err.contains("文件"));
        let err = build_plan(&[item(&file, "H", None)], "", "vendor", "copy").unwrap_err();
        assert!(err.contains("先选"));
    }

    #[test]
    fn missing_source_is_skipped_with_a_reason() {
        let dir = scratch("missing");
        let gone = dir.join("nope.ttf");
        let plan = build_plan(&[item(&gone, "N", None)], dir.to_str().unwrap(), "vendor", "copy").unwrap();
        assert_eq!(plan.ready, 0);
        assert!(plan.entries[0].reason.contains("不见了"));
    }

    #[test]
    fn keys_pick_the_right_folder() {
        let dir = scratch("keys");
        let file = dir.join("i.ttf");
        std::fs::write(&file, b"font").unwrap();
        let it = item(&file, "I", Some("Acme"));
        assert_eq!(folder_for(&it, "vendor"), "Acme");
        assert_eq!(folder_for(&it, "language"), "CJK");
        assert_eq!(folder_for(&it, "format"), "TTF");
        assert_eq!(folder_for(&it, "classification"), "Serif");
        let _ = fs_cleanup(&dir);
    }

    fn fs_cleanup(dir: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(dir)
    }
}

