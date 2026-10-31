//! 设计软件 / 前端工程里的字体引用分析。
//!
//! 目的很实在：翻一遍设计稿和项目源码，回答两个问题——
//!
//! * 这些文件里用到了本机已经装的哪些字体（删之前先看看谁在用它）；
//! * 用到了哪些本机**没有**的字体（缺字体导致设计稿变成 fallback 的那一类）。
//!
//! 取名字的办法分三种：
//!
//! * PDF / AI / EPS：找 `/BaseFont`、`/FontName` 后面的名字（AI 文件本身是 PDF 兼容格式）；
//! * SVG / HTML / CSS / JS / Vue 等文本：找 `font-family`、`fontFamily` 后面的字族列表；
//! * 二进制设计文件（PSD/IND 等）：没有可靠的结构可读，就按「已知字体名」双向子串搜索
//!   （ASCII 与 UTF-16LE 各扫一遍），只能答第一问，不会凭空造出名字。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use walkdir::WalkDir;

/// 一次扫描最多看到的文件数，防止误选整个盘符时卡死。
const MAX_FILES: usize = 30_000;
/// 单个文件最多读这么多字节（超大 PSD 直接跳过内容，只计数）。
const MAX_BYTES: u64 = 96 * 1024 * 1024;
const MAX_DEPTH: usize = 8;

const SKIP_DIRS: [&str; 8] = [
    "node_modules",
    ".git",
    ".svn",
    "target",
    ".next",
    ".cache",
    "__pycache__",
    ".venv",
];

/// CSS 里那些「不是字体」的字族关键字，别当成缺字体报出去。
const GENERIC: [&str; 26] = [
    "serif",
    "sansserif",
    "monospace",
    "cursive",
    "fantasy",
    "systemui",
    "ui serif",
    "uisansserif",
    "uisansserif",
    "uimonospace",
    "uirounded",
    "apple system",
    "applesystem",
    "blinksystemfont",
    "inherit",
    "initial",
    "unset",
    "revert",
    "auto",
    "none",
    "normal",
    "var",
    "sans",
    "emoji",
    "math",
    "fangsong",
];

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefFile {
    pub path: String,
    pub file_name: String,
    pub size: u64,
    pub modified: u64,
    /// pdf / web / zip / binary
    pub kind: String,
    /// 命中的已安装字体（显示名）。
    pub installed: Vec<String>,
    /// 文件里出现、但本机没装的字体名。
    pub missing: Vec<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefFont {
    pub name: String,
    pub files: u32,
    pub installed: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceReport {
    pub root: String,
    pub scanned: u32,
    pub elapsed_ms: u64,
    pub files: Vec<RefFile>,
    pub fonts: Vec<RefFont>,
    /// 有字体引用但本机没装的那些（按出现文件数降序）。
    pub missing_count: u32,
    pub installed_count: u32,
}

fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 去掉 PDF 里的子集前缀（`ABCDEF+Arial` → `Arial`）。
fn strip_subset_prefix(name: &str) -> &str {
    match name.split_once('+') {
        Some((head, tail))
            if head.len() == 6 && head.chars().all(|c| c.is_ascii_uppercase()) => tail,
        _ => name,
    }
}

/// 常见字重/字形后缀，比对前先削掉；PDF 里的名字几乎总带着它们。
const STYLE_SUFFIXES: [&str; 23] = [
    "bolditalic", "boldoblique", "bold", "italic", "oblique", "regular", "book", "roman", "medium",
    "semibold", "demibold", "extrabold", "ultrabold", "black", "heavy", "light", "extralight",
    "ultralight", "thin", "hairline", "condensed", "narrow", "wide",
];

fn strip_style_suffix(norm: &str) -> String {
    let mut out = norm.to_string();
    loop {
        let Some(hit) = STYLE_SUFFIXES
            .iter()
            .filter(|s| out.len() > s.len() + 2 && out.ends_with(**s))
            .max_by_key(|s| s.len())
        else {
            return out;
        };
        out.truncate(out.len() - hit.len());
    }
}

struct Index {
    /// 归一化名字 → 显示名（已安装的字体）。
    by_norm: BTreeMap<String, String>,
}

impl Index {
    fn new(names: &[String]) -> Self {
        let mut by_norm = BTreeMap::new();
        for name in names {
            let key = normalize(name);
            if key.len() >= 2 {
                by_norm.entry(key).or_insert_with(|| name.clone());
            }
        }
        Self { by_norm }
    }

    /// 判断一个候选名是不是某款已安装字体。
    fn lookup(&self, candidate: &str) -> Option<&String> {
        let norm = normalize(strip_subset_prefix(candidate));
        if norm.len() < 2 {
            return None;
        }
        if let Some(hit) = self.by_norm.get(&norm) {
            return Some(hit);
        }
        let trimmed = strip_style_suffix(&norm);
        if let Some(hit) = self.by_norm.get(&trimmed) {
            return Some(hit);
        }
        // `MicrosoftYaHeiUI` 这类：已装名字是候选的前缀，且长度差不多。
        self.by_norm
            .iter()
            .filter(|(key, _)| {
                norm.starts_with(key.as_str()) && norm.len() - key.len() <= 16 && key.len() * 5 >= norm.len() * 3
            })
            .max_by_key(|(key, _)| key.len())
            .map(|(_, name)| name)
    }
}

fn is_plausible_family(token: &str) -> bool {
    let token = token.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    if token.len() < 2 || token.len() > 64 {
        return false;
    }
    if token.contains("var(") || token.starts_with("--") || token.contains('{') {
        return false;
    }
    let norm = normalize(token);
    if norm.is_empty() || GENERIC.contains(&norm.as_str()) {
        return false;
    }
    token
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '+' | '\u{4e00}'..='\u{9fff}'))
}

/// 从 PDF / AI / EPS 里抽字体名。
fn from_pdf(bytes: &[u8], out: &mut BTreeSet<String>) {
    let text = String::from_utf8_lossy(bytes);
    for marker in ["/BaseFont", "/FontName"] {
        let mut from = 0usize;
        while let Some(at) = text[from..].find(marker) {
            let abs = from + at + marker.len();
            let rest = text[abs..].trim_start();
            let rest = rest.strip_prefix('/').unwrap_or(rest);
            let end = rest
                .find(|c: char| {
                    c.is_whitespace() || matches!(c, '/' | '[' | ']' | '(' | ')' | '<' | '>' | ',')
                })
                .unwrap_or(rest.len());
            let token = rest[..end].trim();
            if token.len() >= 2 && !token.contains('#') {
                out.insert(strip_subset_prefix(token).to_string());
            }
            from = abs;
        }
    }
}

/// 从文本（CSS / SVG / 代码）里抽 `font-family` 的字族列表。
///
/// `strict` 用于 JS/TS/JSON 这类「值可能是变量名」的文件：只有带引号的才算字族，
/// 否则 `fontFamily: titleFont` 这种主题变量会被误判成一款缺失字体。
fn from_text(text: &str, strict: bool, out: &mut BTreeSet<String>) {
    let lowered = text.to_lowercase();
    for marker in ["font-family", "fontfamily"] {
        let mut from = 0usize;
        while let Some(at) = lowered[from..].find(marker) {
            let mut rest = &text[from + at + marker.len()..];
            // 跳过 `:`、`=`、引号、空白。
            rest = rest.trim_start_matches(|c: char| {
                c.is_whitespace() || matches!(c, ':' | '=' | '"' | '\'')
            });
            // 单引号是「字族」自己的引号，不能当结束符，否则 `'A', 'B'` 只能抓到第一个。
            let end = rest
                .find(|c: char| matches!(c, ';' | '}' | '\n' | '\r' | '"' | '<' | ')'))
                .unwrap_or(rest.len().min(120));
            for part in rest[..end].split(',') {
                let part = part.trim();
                let unquoted = part.trim_matches(|c| c == '"' || c == '\'');
                // strict 模式下不带引号的一律当变量名丢掉。
                if strict && unquoted == part {
                    continue;
                }
                if is_plausible_family(part) {
                    out.insert(unquoted.to_string());
                }
            }
            from += at + marker.len();
        }
    }
}

/// 二进制文件里按「已安装字体名」做双向子串搜索（ASCII + UTF-16LE）。
fn scan_known(bytes: &[u8], index: &Index, out: &mut BTreeSet<String>) {
    let lowered: Vec<u8> = bytes.to_ascii_lowercase();
    for (key, name) in &index.by_norm {
        if key.len() < 4 {
            continue;
        }
        let ascii = key.as_bytes();
        let hit_ascii = lowered
            .windows(ascii.len())
            .any(|w| w == ascii);
        if hit_ascii {
            out.insert(name.clone());
            continue;
        }
        // UTF-16LE：字符之间夹一个 0，多见于 PSD 的字体描述块。
        let utf16: Vec<u8> = key
            .chars()
            .flat_map(|c| [c as u8, 0u8])
            .collect();
        if utf16.len() >= 8 && lowered.windows(utf16.len()).any(|w| w == utf16.as_slice()) {
            out.insert(name.clone());
        }
    }
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn modified_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 扫描一个目录，返回引用报告。`installed` 是本机所有字体的家族名/全名。
pub fn scan(root: &str, installed: &[String]) -> Result<ReferenceReport, String> {
    let started = std::time::Instant::now();
    let index = Index::new(installed);
    let root_path = Path::new(root);
    if !root_path.is_dir() {
        return Err("请选择一个文件夹".into());
    }

    let mut files: Vec<RefFile> = Vec::new();
    let mut tally_installed: BTreeMap<String, u32> = BTreeMap::new();
    let mut tally_missing: BTreeMap<String, u32> = BTreeMap::new();
    let mut scanned = 0u32;

    let walker = WalkDir::new(root_path)
        .max_depth(MAX_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && SKIP_DIRS.contains(&e.file_name().to_string_lossy().to_lowercase().as_str()))
        });

    for entry in walker.flatten() {
        if scanned as usize >= MAX_FILES {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = ext_of(path);
        let kind = match ext.as_str() {
            "pdf" | "ai" | "eps" => "pdf",
            "svg" | "html" | "htm" | "xhtml" | "xml" | "css" | "scss" | "sass" | "less" | "js"
            | "jsx" | "ts" | "tsx" | "vue" | "svelte" | "astro" | "json" | "md" | "txt" => "web",
            "sketch" | "docx" | "pptx" | "xlsx" | "odt" | "odp" | "epub" => "zip",
            "psd" | "psb" | "indd" | "cdr" | "afdesign" | "afpub" | "afphoto" | "xd" | "fig"
            | "idml" => "binary",
            _ => continue,
        };
        let Ok(meta) = entry.metadata() else { continue };
        scanned += 1;
        if meta.len() > MAX_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else { continue };

        let mut found: BTreeSet<String> = BTreeSet::new();
        let mut candidates: BTreeSet<String> = BTreeSet::new();
        // 代码文件里 `fontFamily: someVar` 很常见，必须带引号才当字族。
        let strict = matches!(
            ext.as_str(),
            "js" | "jsx" | "ts" | "tsx" | "vue" | "svelte" | "astro" | "json"
        );
        match kind {
            "pdf" => from_pdf(&bytes, &mut candidates),
            "web" => from_text(&String::from_utf8_lossy(&bytes), strict, &mut candidates),
            "zip" => {
                if let Ok(archive) = zip::ZipArchive::new(std::io::Cursor::new(&bytes)) {
                    let mut archive = archive;
                    for i in 0..archive.len().min(400) {
                        let Ok(mut item) = archive.by_index(i) else { continue };
                        if item.size() > 8 * 1024 * 1024 {
                            continue;
                        }
                        let name = item.name().to_lowercase();
                        if !(name.ends_with(".xml") || name.ends_with(".json") || name.ends_with(".rels")) {
                            continue;
                        }
                        let mut buf = String::new();
                        use std::io::Read;
                        if item.read_to_string(&mut buf).is_ok() {
                            from_text(&buf, true, &mut candidates);
                        }
                    }
                }
            }
            _ => {}
        }

        // 候选名里哪些是本机已装的、哪些是缺的。
        for candidate in &candidates {
            if GENERIC.contains(&normalize(candidate).as_str()) {
                continue;
            }
            match index.lookup(candidate) {
                Some(name) => {
                    found.insert(name.clone());
                }
                None => {
                    *tally_missing.entry(candidate.clone()).or_insert(0) += 1;
                    found.insert(candidate.clone());
                }
            }
        }
        // 二进制文件没有可读的结构，只能靠已知名字反查。
        if kind == "binary" {
            let mut hits = BTreeSet::new();
            scan_known(&bytes, &index, &mut hits);
            found.extend(hits);
        }

        if found.is_empty() {
            continue;
        }

        let mut hit_installed: Vec<String> = Vec::new();
        let mut hit_missing: Vec<String> = Vec::new();
        for name in &found {
            if index.lookup(name).is_some() {
                hit_installed.push(name.clone());
                *tally_installed.entry(name.clone()).or_insert(0) += 1;
            } else {
                hit_missing.push(name.clone());
            }
        }
        hit_installed.sort();
        hit_missing.sort();
        files.push(RefFile {
            path: path.to_string_lossy().into_owned(),
            file_name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size: meta.len(),
            modified: modified_of(&meta),
            kind: kind.to_string(),
            installed: hit_installed,
            missing: hit_missing,
        });
    }

    // 缺字体排前面，同组内按出现的文件数降序。
    let mut fonts: Vec<RefFont> = Vec::new();
    for (name, files) in &tally_missing {
        fonts.push(RefFont {
            name: name.clone(),
            files: *files,
            installed: false,
        });
    }
    for (name, files) in &tally_installed {
        fonts.push(RefFont {
            name: name.clone(),
            files: *files,
            installed: true,
        });
    }
    fonts.sort_by(|a, b| {
        a.installed
            .cmp(&b.installed)
            .then(b.files.cmp(&a.files))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    files.sort_by(|a, b| b.missing.len().cmp(&a.missing.len()).then(a.file_name.cmp(&b.file_name)));

    Ok(ReferenceReport {
        root: root.to_string(),
        scanned,
        elapsed_ms: started.elapsed().as_millis() as u64,
        missing_count: tally_missing.len() as u32,
        installed_count: tally_installed.len() as u32,
        fonts,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subset_prefix_and_style_suffix_are_stripped() {
        assert_eq!(strip_subset_prefix("ABCDEF+Arial"), "Arial");
        assert_eq!(strip_subset_prefix("Arial"), "Arial");
        assert_eq!(strip_style_suffix("microsoftyaheibold"), "microsoftyahei");
        assert_eq!(strip_style_suffix("sourcehansanssc"), "sourcehansanssc");
    }

    #[test]
    fn lookup_matches_installed_families() {
        let index = Index::new(&[
            "Microsoft YaHei".to_string(),
            "思源黑体".to_string(),
            "Arial".to_string(),
        ]);
        assert!(index.lookup("ABCDEF+MicrosoftYaHei-Bold").is_some());
        assert!(index.lookup("ArialMT").is_some());
        assert!(index.lookup("思源黑体").is_some());
        assert!(index.lookup("Comic Sans MS").is_none());
    }

    #[test]
    fn reads_basefont_from_a_pdf_like_buffer() {
        let mut out = BTreeSet::new();
        from_pdf(
            b"<< /Type /Font /BaseFont /ABCDEF+SourceHanSans-Bold >> /FontName /SimSun ",
            &mut out,
        );
        assert!(out.contains("SourceHanSans-Bold"));
        assert!(out.contains("SimSun"));
    }

    #[test]
    fn reads_font_family_declarations() {
        let mut out = BTreeSet::new();
        from_text(
            "body { font-family: 'Noto Serif SC', serif; }\n.a{fontFamily:\"Inter\", sans-serif}\n",
            false,
            &mut out,
        );
        assert!(out.contains("Noto Serif SC"), "实际：{out:?}");
        assert!(out.contains("Inter"));
        assert!(!out.iter().any(|n| n == "serif"), "通用族不该出现");
    }

    #[test]
    fn generic_keywords_are_rejected() {
        assert!(!is_plausible_family("sans-serif"));
        assert!(!is_plausible_family("var(--font)"));
        assert!(!is_plausible_family("a"));
        assert!(is_plausible_family("思源宋体 CN"));
    }

    #[test]
    fn scans_a_real_project_without_panicking() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src");
        if !root.is_dir() {
            return;
        }
        let report = scan(root.to_str().unwrap(), &["Inter".to_string(), "Arial".to_string()]).unwrap();
        println!(
            "scanned={} 相关文件={} 缺字体={} 已装={} 耗时={}ms",
            report.scanned,
            report.files.len(),
            report.missing_count,
            report.installed_count,
            report.elapsed_ms
        );
        for f in report.fonts.iter().take(10) {
            println!("  {} · {} 个文件 · 已装={}", f.name, f.files, f.installed);
        }
        assert!(report.scanned > 0);
    }

    #[test]
    fn scan_reports_installed_and_missing_fonts() {
        let dir = std::env::temp_dir().join(format!("zfm-ref-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(
            dir.join("style.css"),
            "body { font-family: 'Microsoft YaHei', 'Noto Sans SC', sans-serif; }",
        )
        .unwrap();
        // 模拟一个 AI 文件（AI 本身是 PDF 兼容格式，字体名在 BaseFont 里）。
        std::fs::write(
            dir.join("sub/poster.ai"),
            b"%PDF-1.5\n<< /Type /Font /BaseFont /ABCDEF+SimSun /Subtype /TrueType >>",
        )
        .unwrap();
        // 不相关的文件应该被忽略。
        std::fs::write(dir.join("note.bin"), [0u8; 32]).unwrap();

        let installed = vec!["Microsoft YaHei".to_string(), "SimSun".to_string()];
        let report = scan(dir.to_str().unwrap(), &installed).unwrap();
        assert_eq!(report.scanned, 2, "只该扫到 css 与 ai");
        assert_eq!(report.installed_count, 2, "两款已装字体都该被认出来");
        assert_eq!(report.missing_count, 1, "Noto Sans SC 应算缺失");
        let missing: Vec<&str> = report
            .fonts
            .iter()
            .filter(|f| !f.installed)
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(missing, vec!["Noto Sans SC"]);
        assert_eq!(report.files.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
