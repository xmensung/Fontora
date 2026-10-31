//! 导出层：把库里的信息与文件「带出去」。
//!
//! 三种产物，全部在本地生成：
//!
//! 1. **清单** —— 每个字重一行，CSV（带 BOM，Excel 双击就能正确识别中文）
//!    或 JSON。哈希、字符数、汉字数取自 SQLite 索引，不重新解析字体。
//! 2. **@font-face CSS** —— 网页/项目里直接可用；`web` 为真时指向 WOFF2。
//! 3. **项目字体包** —— 一个 zip：`fonts/`（集合会按 face 拆成单字体文件）、
//!    可选 `web/`（WOFF2）、`fonts.css`、`manifest.json`、`README.txt`（授权说明）。
//!
//! 打包时才真正读盘，并且**按文件分组只读一次**：选了同一 TTC 里的 5 个字重
//! 不会把 40MB 的文件读 5 遍。

use crate::db::FontCache;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// 清单里的一行 —— 一个字重（face）。
///
/// 前一段由界面填（库里/界面上的信息），后一段（`file_name` 起）由后端
/// 从索引补齐，界面不用传。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportRow {
    pub family: String,
    pub style: String,
    pub postscript_name: Option<String>,
    pub path: String,
    pub face_index: u32,
    pub format: String,
    pub weight: u16,
    pub italic: bool,
    pub variable: bool,
    pub classification: String,
    pub foundry: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub glyph_count: u16,
    pub file_size: u64,
    pub source: String,
    pub active: bool,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub favorite: bool,
    /// 1–5，0 = 没评分。
    pub rating: u8,
    pub note: Option<String>,
    // —— 后端补齐 ——
    pub file_name: String,
    pub sha256: Option<String>,
    pub total_chars: Option<u32>,
    pub han: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogExport {
    pub path: String,
    pub rows: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageExport {
    pub path: String,
    pub fonts: usize,
    pub web_fonts: usize,
    pub bytes: u64,
    /// 生成过程中跳过的项（例如 CFF 字体转 WOFF2 失败），供界面提示。
    pub errors: Vec<String>,
}

impl ExportRow {
    pub(crate) fn ext(&self) -> &'static str {
        match self.format.as_str() {
            "otf" => "otf",
            "woff" => "woff",
            "woff2" => "woff2",
            _ => "ttf",
        }
    }

    fn weight_or_400(&self) -> u16 {
        if self.weight == 0 {
            400
        } else {
            self.weight
        }
    }
}

/// 用索引里的哈希与字符数把行补齐（索引没有就留空，不现场解析）。
pub fn enrich(mut rows: Vec<ExportRow>, cache: Option<&FontCache>) -> Vec<ExportRow> {
    for row in &mut rows {
        if row.file_name.is_empty() {
            row.file_name = Path::new(&row.path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        let Some(cache) = cache else { continue };
        let facts = cache.facts_for(&row.path);
        if row.sha256.as_deref().unwrap_or("").is_empty() {
            row.sha256 = facts.hash.filter(|h| !h.is_empty());
        }
        if row.total_chars.is_none() {
            row.total_chars = facts.total_chars;
        }
        if row.han.is_none() {
            row.han = facts.han;
        }
    }
    rows
}

/// 每个字重写进包里时用的文件名（不含扩展名）。重名的自动加 `(2)`。
pub fn file_stems(rows: &[ExportRow]) -> Vec<String> {
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    rows.iter()
        .map(|row| {
            let base = crate::sfnt::safe_file_stem(&row.family, &row.style);
            let count = used.entry(base.clone()).or_insert(0);
            *count += 1;
            if *count == 1 {
                base
            } else {
                format!("{base} ({count})")
            }
        })
        .collect()
}

fn csv_cell(value: &str) -> String {
    if value
        .chars()
        .any(|c| matches!(c, ',' | '"' | '\n' | '\r' | '\t'))
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn join(values: &[String]) -> String {
    values.join("、")
}

const HEADER_ZH: &[&str] = &[
    "家族",
    "样式",
    "PostScript 名",
    "格式",
    "字重",
    "斜体",
    "可变",
    "分类",
    "厂商",
    "许可",
    "许可链接",
    "字形数",
    "字符数",
    "汉字数",
    "文件大小(KB)",
    "文件名",
    "路径",
    "来源",
    "已启用",
    "收藏",
    "评分",
    "标签",
    "集合",
    "备注",
    "SHA-256",
];

const HEADER_EN: &[&str] = &[
    "Family",
    "Style",
    "PostScript",
    "Format",
    "Weight",
    "Italic",
    "Variable",
    "Classification",
    "Foundry",
    "License",
    "License URL",
    "Glyphs",
    "Characters",
    "Han",
    "Size (KB)",
    "File",
    "Path",
    "Source",
    "Active",
    "Favorite",
    "Rating",
    "Tags",
    "Collections",
    "Note",
    "SHA-256",
];

/// CSV 正文：BOM + 表头 + CRLF 行。BOM 是为了 Excel 直接双击不乱码。
pub fn csv_text(rows: &[ExportRow], chinese: bool) -> String {
    let header = if chinese { HEADER_ZH } else { HEADER_EN };
    let yes_no = |b: bool, zh: bool| {
        match (b, zh) {
            (true, true) => "是",
            (false, true) => "否",
            (true, false) => "yes",
            (false, false) => "no",
        }
        .to_string()
    };
    let mut out = String::from('\u{feff}');
    out.push_str(
        &header
            .iter()
            .map(|h| csv_cell(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("\r\n");
    for row in rows {
        let size_kb = format!("{:.1}", row.file_size as f64 / 1024.0);
        let cells = [
            row.family.clone(),
            row.style.clone(),
            row.postscript_name.clone().unwrap_or_default(),
            row.format.to_uppercase(),
            row.weight_or_400().to_string(),
            yes_no(row.italic, chinese),
            yes_no(row.variable, chinese),
            row.classification.clone(),
            row.foundry.clone().unwrap_or_default(),
            row.license.clone().unwrap_or_default(),
            row.license_url.clone().unwrap_or_default(),
            row.glyph_count.to_string(),
            row.total_chars.map(|n| n.to_string()).unwrap_or_default(),
            row.han.map(|n| n.to_string()).unwrap_or_default(),
            size_kb,
            row.file_name.clone(),
            row.path.clone(),
            row.source.clone(),
            yes_no(row.active, chinese),
            yes_no(row.favorite, chinese),
            if row.rating > 0 { row.rating.to_string() } else { String::new() },
            join(&row.tags),
            join(&row.collections),
            row.note.clone().unwrap_or_default(),
            row.sha256.clone().unwrap_or_default(),
        ];
        out.push_str(
            &cells
                .iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("\r\n");
    }
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Catalog<'a> {
    app: &'static str,
    kind: &'static str,
    version: u32,
    exported_at: String,
    count: usize,
    fonts: &'a [ExportRow],
}

pub fn json_text(rows: &[ExportRow]) -> Result<String, String> {
    let payload = Catalog {
        app: "Fontora",
        kind: "catalog",
        version: 1,
        exported_at: crate::db::local_stamp(),
        count: rows.len(),
        fonts: rows,
    };
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

fn css_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// `@font-face` 片段。`web` 为真时 `src` 指向 `web/*.woff2`，否则 `fonts/*`。
pub fn css_text(rows: &[ExportRow], stems: &[String], web: bool) -> String {
    let mut out = String::new();
    out.push_str("/* Fontora · @font-face · ");
    out.push_str(&crate::db::local_stamp());
    out.push_str(" */\n");
    out.push_str("/* 路径相对于本文件所在目录；字号字重可直接用数值选择。 */\n\n");
    let mut last_family = String::new();
    for (row, stem) in rows.iter().zip(stems.iter()) {
        if row.family != last_family {
            out.push_str(&format!("/* {} */\n", row.family));
            last_family = row.family.clone();
        }
        let (url, format) = if web {
            (format!("web/{stem}.woff2"), "woff2")
        } else {
            (format!("fonts/{stem}.{}", row.ext()), row.ext())
        };
        out.push_str("@font-face {\n");
        out.push_str(&format!("  font-family: \"{}\";\n", css_string(&row.family)));
        out.push_str(&format!(
            "  font-style: {};\n",
            if row.italic { "italic" } else { "normal" }
        ));
        out.push_str(&format!("  font-weight: {};\n", row.weight_or_400()));
        out.push_str("  font-display: swap;\n");
        out.push_str(&format!(
            "  src: url(\"{url}\") format(\"{format}\");\n"
        ));
        out.push_str("}\n\n");
    }
    out
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFont {
    family: String,
    style: String,
    weight: u16,
    italic: bool,
    variable: bool,
    format: String,
    file: Option<String>,
    web: Option<String>,
    sha256: Option<String>,
    total_chars: Option<u32>,
    han: Option<u32>,
    license: Option<String>,
    license_url: Option<String>,
    foundry: Option<String>,
    source: String,
    tags: Vec<String>,
    collections: Vec<String>,
    note: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    app: &'static str,
    kind: &'static str,
    version: u32,
    package: String,
    exported_at: String,
    count: usize,
    with_files: bool,
    with_web: bool,
    fonts: Vec<ManifestFont>,
}

/// 包里的 `README.txt`：一眼能看懂的目录说明 + 逐个字重的授权信息。
pub fn readme_text(name: &str, rows: &[ExportRow], with_files: bool, web: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("{name}\n"));
    out.push_str(&format!(
        "由 Fontora 生成 · {}\n",
        crate::db::local_stamp()
    ));
    out.push_str(&format!("共 {} 个字重\n\n", rows.len()));
    if with_files {
        out.push_str("fonts/        字体文件，可直接拖进系统或设计软件安装\n");
    }
    if web {
        out.push_str("web/          WOFF2 网页字体（字体轮廓不支持时该字重会被跳过）\n");
    }
    out.push_str("fonts.css     @font-face 声明，网页里直接引入\n");
    out.push_str("manifest.json 机器可读的完整清单\n\n");
    out.push_str("—— 授权 ——\n");
    out.push_str("以下信息摘自字体文件内部，商用前请再核对官方许可条款。\n\n");
    let mut last_family = String::new();
    for row in rows {
        if row.family != last_family {
            out.push_str(&format!("{}\n", row.family));
            last_family = row.family.clone();
        }
        out.push_str(&format!("  · {}", row.style));
        if let Some(foundry) = row.foundry.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!("　厂商：{foundry}"));
        }
        out.push('\n');
        if let Some(license) = row.license.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!("    许可：{license}\n"));
        }
        if let Some(url) = row.license_url.as_deref().filter(|s| !s.is_empty()) {
            out.push_str(&format!("    许可链接：{url}\n"));
        }
    }
    out
}

fn manifest_json(name: &str, rows: &[ExportRow], files: &[Option<String>], webs: &[Option<String>], with_files: bool, web: bool) -> Result<String, String> {
    let fonts = rows
        .iter()
        .enumerate()
        .map(|(i, row)| ManifestFont {
            family: row.family.clone(),
            style: row.style.clone(),
            weight: row.weight_or_400(),
            italic: row.italic,
            variable: row.variable,
            format: row.format.to_uppercase(),
            file: files.get(i).cloned().flatten(),
            web: webs.get(i).cloned().flatten(),
            sha256: row.sha256.clone(),
            total_chars: row.total_chars,
            han: row.han,
            license: row.license.clone(),
            license_url: row.license_url.clone(),
            foundry: row.foundry.clone(),
            source: row.source.clone(),
            tags: row.tags.clone(),
            collections: row.collections.clone(),
            note: row.note.clone(),
        })
        .collect();
    let payload = Manifest {
        app: "Fontora",
        kind: "font-package",
        version: 1,
        package: name.to_string(),
        exported_at: crate::db::local_stamp(),
        count: rows.len(),
        with_files,
        with_web: web,
        fonts,
    };
    serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())
}

/// 转一份 WOFF2。优先走子集化（顺带丢掉带字形编号的表），失败再整表压缩，
/// 这样 CFF 轮廓的 OTF 也能转。
fn web_font(data: &[u8], face_index: u32) -> Result<Vec<u8>, String> {
    if let Ok(keep) = crate::subset::all_codepoints(data, face_index) {
        if let Ok(sub) = crate::subset::subset(data, face_index, &keep) {
            if let Ok(container) = crate::sfnt::read_face(&sub, 0) {
                if let Ok(bytes) = pack_woff2(&container) {
                    return Ok(bytes);
                }
            }
        }
    }
    let container = crate::sfnt::read_face(data, face_index)?;
    pack_woff2(&container)
}

/// brotli 质量按体量自适应：一份 10MB 的 CJK 字体在质量 11 上要压两分多钟，
/// 降到 6 只要几秒、体积只大 0.5%。小字体（拉丁等）继续用 11 换最小体积。
fn pack_woff2(container: &crate::sfnt::FaceContainer) -> Result<Vec<u8>, String> {
    let raw: usize = container.tables.iter().map(|t| t.data.len()).sum();
    let quality = if raw > 4 * 1024 * 1024 { 6 } else { 11 };
    crate::subset::to_woff2_q(container.version, &container.tables, quality)
}

pub struct PackageOptions {
    pub name: String,
    pub with_files: bool,
    pub web: bool,
    pub css: bool,
}

impl Default for PackageOptions {
    fn default() -> Self {
        PackageOptions {
            name: "字体包".to_string(),
            with_files: true,
            web: false,
            css: true,
        }
    }
}

/// 打包。`rows` 里的 `path` 必须真实存在，否则该字重被跳过并记进 `errors`。
pub fn build_package(
    dest: &str,
    rows: &[ExportRow],
    opts: &PackageOptions,
) -> Result<PackageExport, String> {
    if rows.is_empty() {
        return Err("没有可打包的字体".to_string());
    }
    if opts.web && rows.len() > 200 {
        return Err(format!(
            "一次最多给 200 个字重生成 WOFF2（当前 {} 个），请缩小范围",
            rows.len()
        ));
    }

    let stems = file_stems(rows);
    let mut files: Vec<Option<String>> = vec![None; rows.len()];
    let mut webs: Vec<Option<String>> = vec![None; rows.len()];
    let mut errors: Vec<String> = Vec::new();

    let handle = fs::File::create(dest).map_err(|e| format!("无法创建压缩包：{e}"))?;
    let mut zip = zip::ZipWriter::new(handle);
    let entry = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // 同一个文件里的多个字重只读一次。
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, row) in rows.iter().enumerate() {
        groups.entry(row.path.as_str()).or_default().push(i);
    }

    let mut font_count = 0usize;
    let mut web_count = 0usize;

    for (path, indices) in groups {
        if !opts.with_files && !opts.web {
            break;
        }
        let data = match fs::read(path) {
            Ok(data) => data,
            Err(e) => {
                errors.push(format!("{}：{e}", rows[indices[0]].family));
                continue;
            }
        };
        let faces = ttf_parser::fonts_in_collection(&data).unwrap_or(1);

        for &i in &indices {
            let row = &rows[i];
            let stem = &stems[i];
            let mut per_row_face: Option<(Vec<u8>, String)> = None;

            if opts.with_files {
                let (bytes, ext) = if faces <= 1 {
                    (data.clone(), row.ext().to_string())
                } else {
                    match crate::sfnt::read_face(&data, row.face_index) {
                        Ok(container) => (
                            crate::sfnt::build_sfnt(container.version, &container.tables),
                            crate::sfnt::extension_for(container.version).to_string(),
                        ),
                        Err(e) => {
                            errors.push(format!("{} {}：{e}", row.family, row.style));
                            continue;
                        }
                    }
                };
                let name = format!("fonts/{stem}.{ext}");
                zip.start_file(name.clone(), entry)
                    .map_err(|e| format!("写入压缩包失败：{e}"))?;
                zip.write_all(&bytes)
                    .map_err(|e| format!("写入压缩包失败：{e}"))?;
                files[i] = Some(name);
                font_count += 1;
                if faces > 1 {
                    per_row_face = Some((bytes, ext));
                }
            }

            if opts.web {
                let result = match &per_row_face {
                    Some((bytes, _)) => web_font(bytes, 0),
                    None if faces <= 1 => web_font(&data, row.face_index),
                    None => crate::sfnt::read_face(&data, row.face_index)
                        .and_then(|c| pack_woff2(&c)),
                };
                match result {
                    Ok(bytes) => {
                        let name = format!("web/{stem}.woff2");
                        zip.start_file(name.clone(), entry)
                            .map_err(|e| format!("写入压缩包失败：{e}"))?;
                        zip.write_all(&bytes)
                            .map_err(|e| format!("写入压缩包失败：{e}"))?;
                        webs[i] = Some(name);
                        web_count += 1;
                    }
                    Err(e) => errors.push(format!("{} {} 转 WOFF2 跳过：{e}", row.family, row.style)),
                }
            }
        }
    }

    if opts.css {
        let body = css_text(rows, &stems, opts.web && web_count > 0);
        zip.start_file("fonts.css", entry)
            .map_err(|e| format!("写入压缩包失败：{e}"))?;
        zip.write_all(body.as_bytes())
            .map_err(|e| format!("写入压缩包失败：{e}"))?;
    }

    let manifest = manifest_json(&opts.name, rows, &files, &webs, opts.with_files, opts.web)?;
    zip.start_file("manifest.json", entry)
        .map_err(|e| format!("写入压缩包失败：{e}"))?;
    zip.write_all(manifest.as_bytes())
        .map_err(|e| format!("写入压缩包失败：{e}"))?;

    let readme = readme_text(&opts.name, rows, opts.with_files, opts.web);
    zip.start_file("README.txt", entry)
        .map_err(|e| format!("写入压缩包失败：{e}"))?;
    zip.write_all(readme.as_bytes())
        .map_err(|e| format!("写入压缩包失败：{e}"))?;

    zip.finish().map_err(|e| format!("收尾失败：{e}"))?;
    let bytes = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    errors.truncate(5);
    Ok(PackageExport {
        path: dest.to_string(),
        fonts: font_count,
        web_fonts: web_count,
        bytes,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(family: &str, style: &str, weight: u16) -> ExportRow {
        ExportRow {
            family: family.to_string(),
            style: style.to_string(),
            path: format!("C:/fonts/{family}-{style}.ttf"),
            face_index: 0,
            format: "ttf".to_string(),
            weight,
            glyph_count: 1234,
            file_size: 1024 * 1024,
            source: "user".to_string(),
            active: true,
            tags: vec!["标题".to_string(), "品牌".to_string()],
            collections: vec!["项目 A".to_string()],
            ..Default::default()
        }
    }

    fn system_font(name: &str) -> Option<String> {
        let path = format!("C:/Windows/Fonts/{name}");
        Path::new(&path).exists().then_some(path)
    }

    #[test]
    fn csv_has_bom_header_and_quotes_tricky_cells() {
        let mut a = row("思源黑体", "Regular", 400);
        a.note = Some("含逗号,引号\"和\n换行".to_string());
        let text = csv_text(&[a], true);
        assert!(text.starts_with('\u{feff}'), "Excel 需要 BOM");
        assert!(text.contains("家族,样式,PostScript 名"));
        assert!(text.contains("\"含逗号,引号\"\"和\n换行\""), "逗号/引号/换行都要转义");
        assert!(text.ends_with("\r\n"));
        // 标签用顿号连
        assert!(text.contains("标题、品牌"));
    }

    #[test]
    fn english_header_when_not_chinese() {
        let text = csv_text(&[row("Inter", "Bold", 700)], false);
        assert!(text.contains("Family,Style,PostScript"));
        assert!(text.contains("Inter,Bold"));
    }

    #[test]
    fn json_reports_count_and_rows() {
        let text = json_text(&[row("Inter", "Bold", 700)]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["count"], 1);
        assert_eq!(value["fonts"][0]["family"], "Inter");
        assert_eq!(value["fonts"][0]["weight"], 700);
    }

    #[test]
    fn stems_dedupe_repeated_family_and_style() {
        let rows = vec![
            row("Inter", "Regular", 400),
            row("Inter", "Regular", 400),
            row("Inter", "Bold", 700),
        ];
        let stems = file_stems(&rows);
        assert_eq!(stems, vec!["Inter", "Inter (2)", "Inter Bold"]);
    }

    #[test]
    fn css_targets_web_fonts_only_when_asked() {
        let rows = vec![row("Inter", "Bold", 700)];
        let stems = file_stems(&rows);
        let local = css_text(&rows, &stems, false);
        assert!(local.contains("url(\"fonts/Inter Bold.ttf\") format(\"ttf\")"));
        assert!(local.contains("font-weight: 700"));
        let web = css_text(&rows, &stems, true);
        assert!(web.contains("url(\"web/Inter Bold.woff2\") format(\"woff2\")"));
    }

    #[test]
    fn css_escapes_quotes_in_family_names() {
        let rows = vec![row("A\"B", "Regular", 400)];
        let stems = file_stems(&rows);
        let css = css_text(&rows, &stems, false);
        assert!(css.contains("font-family: \"A\\\"B\""));
    }

    #[test]
    fn readme_carries_license_lines() {
        let mut a = row("思源黑体", "Regular", 400);
        a.license = Some("SIL Open Font License 1.1".to_string());
        a.foundry = Some("Adobe".to_string());
        let text = readme_text("项目字体包", &[a.clone()], true, false);
        assert!(text.contains("项目字体包"));
        assert!(text.contains("厂商：Adobe"));
        assert!(text.contains("许可：SIL Open Font License 1.1"));
        assert!(text.contains("fonts/"));
        assert!(!text.contains("web/"), "没生成网页字体时不提 web/");
    }

    /// 真打一次包：读一个系统字体，产出的字体文件要能被 ttf-parser 认出来。
    #[test]
    fn package_contains_font_css_manifest_and_readme() {
        let Some(src) = system_font("consola.ttf") else {
            return;
        };
        let data = fs::read(&src).unwrap();
        let face = ttf_parser::Face::parse(&data, 0).unwrap();
        let Some(family) = face
            .names()
            .into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
            .and_then(|n| n.to_string())
        else {
            return;
        };
        let mut one = row(&family, "Regular", 400);
        one.path = src.clone();
        one.file_size = data.len() as u64;

        let dir = std::env::temp_dir().join(format!("zfm-pkg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("pkg.zip");
        let out = build_package(
            dest.to_str().unwrap(),
            &[one],
            &PackageOptions {
                name: "测试字体包".to_string(),
                with_files: true,
                web: false,
                css: true,
            },
        )
        .unwrap();
        assert_eq!(out.fonts, 1);
        assert!(out.bytes > 0);

        let file = fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "fonts.css"));
        assert!(names.iter().any(|n| n == "manifest.json"));
        assert!(names.iter().any(|n| n == "README.txt"));
        let font_entry = names.iter().find(|n| n.starts_with("fonts/")).unwrap().clone();

        let mut bytes = Vec::new();
        {
            let mut entry = archive.by_name(&font_entry).unwrap();
            std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
        }
        let parsed = ttf_parser::Face::parse(&bytes, 0).expect("包里的字体要能被解析");
        assert!(parsed.number_of_glyphs() > 0);
        let _ = fs::remove_dir_all(&dir);
    }

    /// 集合（TTC）里的字重要按 face 拆成独立文件，包里不能出现 .ttc。
    #[test]
    fn collection_faces_are_unpacked_inside_the_package() {
        let Some(src) = system_font("msyh.ttc") else {
            return;
        };
        let data = fs::read(&src).unwrap();
        if ttf_parser::fonts_in_collection(&data).unwrap_or(1) < 2 {
            return;
        }
        let mut rows = Vec::new();
        for index in 0..2u32 {
            let face = ttf_parser::Face::parse(&data, index).unwrap();
            let family = face
                .names()
                .into_iter()
                .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
                .and_then(|n| n.to_string())
                .unwrap_or_else(|| format!("face{index}"));
            rows.push(ExportRow {
                family,
                style: format!("Face {index}"),
                path: src.clone(),
                face_index: index,
                format: "ttf".to_string(),
                weight: 400,
                ..Default::default()
            });
        }
        let dir = std::env::temp_dir().join(format!("zfm-ttc-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("ttc.zip");
        let out = build_package(
            dest.to_str().unwrap(),
            &rows,
            &PackageOptions {
                name: "集合拆包".to_string(),
                with_files: true,
                web: false,
                css: true,
            },
        )
        .unwrap();
        assert_eq!(out.fonts, 2);

        let archive = zip::ZipArchive::new(fs::File::open(&dest).unwrap()).unwrap();
        let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        let fonts: Vec<&String> = names.iter().filter(|n| n.starts_with("fonts/")).collect();
        assert_eq!(fonts.len(), 2);
        assert!(fonts.iter().all(|n| n.ends_with(".ttf")), "{names:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn too_many_rows_for_web_is_refused() {
        let rows: Vec<ExportRow> = (0..201).map(|i| row(&format!("F{i}"), "Regular", 400)).collect();
        let dir = std::env::temp_dir().join(format!("zfm-pkg-cap-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("cap.zip");
        let err = build_package(
            dest.to_str().unwrap(),
            &rows,
            &PackageOptions {
                name: "cap".to_string(),
                with_files: false,
                web: true,
                css: false,
            },
        )
        .unwrap_err();
        assert!(err.contains("200"));
        let _ = fs::remove_dir_all(&dir);
    }
}



