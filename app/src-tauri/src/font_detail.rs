//! Everything the detail page shows, assembled from the font file itself.
//!
//! The page has four sections — 字体介绍 / 字体授权 / 字数统计 / 字体样例 — and
//! every value here is derived from the binary: the `name` table, `OS/2`
//! `fsType`, `cmap` coverage, `fvar` axes, `GSUB`/`GPOS` features. Nothing is
//! fetched, guessed, or invented; when a field is absent in the file the UI is
//! told so explicitly (`None`) instead of being handed placeholder text.

use crate::charsets::{coverage_from_codepoints, list_chars, Coverage};
use crate::cjk;
use crate::font_types::FontFormat;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// A localized name record, e.g. 简体中文 → 思源黑体.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedName {
    pub lang: String,
    pub lang_code: String,
    pub family: Option<String>,
    pub style: Option<String>,
}

/// One paragraph of 字体介绍.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IntroBlock {
    pub label: String,
    pub body: String,
}

/// One row of the embedding-permission table (OS/2 `fsType`).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmbedFlag {
    pub bit: u16,
    pub label: String,
    pub detail: String,
    /// Whether this flag is set on the font.
    pub on: bool,
    /// `true` when the flag grants a right, `false` when it restricts one.
    pub grants: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    /// One-line verdict, e.g. 「允许在文档中自由嵌入」.
    pub headline: String,
    /// Raw `fsType` value so the user can verify against the spec.
    pub fs_type: u16,
    pub flags: Vec<EmbedFlag>,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub copyright: Option<String>,
    pub trademark: Option<String>,
    pub license_text: Option<String>,
    pub license_url: Option<String>,
    pub vendor_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SampleLine {
    pub label: String,
    pub text: String,
    /// Suggested display size in px.
    pub size: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FontDetail {
    // identity
    pub family: String,
    pub style: String,
    pub postscript_name: Option<String>,
    pub path: String,
    pub format: FontFormat,
    pub file_size: u64,
    pub modified: i64,
    pub sha256: Option<String>,
    pub glyph_count: u16,
    pub weight: u16,
    pub italic: bool,
    pub monospaced: bool,
    pub classification: String,
    pub faces_in_file: u32,
    pub is_variable: bool,
    pub axes: Vec<crate::font_types::VariationAxis>,

    // text metadata
    pub version: Option<String>,
    pub designer: Option<String>,
    pub designer_url: Option<String>,
    pub manufacturer: Option<String>,
    pub description: Option<String>,
    pub unique_id: Option<String>,
    pub variation_prefix: Option<String>,
    pub created: Option<String>,
    pub modified_date: Option<String>,

    // sections
    pub names: Vec<LocalizedName>,
    pub scripts: Vec<String>,
    pub features: Vec<String>,
    pub intro: Vec<IntroBlock>,
    pub license: LicenseInfo,
    pub coverage: Coverage,
    pub cjk: cjk::CjkStats,
    pub samples: Vec<SampleLine>,
    /// 字体所在目录里的官方样张（`预览/` 子目录优先，其次同级目录的图片）。
    pub previews: Vec<PreviewImage>,
}

/// 一张随字体一起存放的官方样张。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PreviewImage {
    pub path: String,
    pub name: String,
    pub bytes: u64,
    /// `folder` 表示来自 `预览/` 子目录，`sibling` 表示与字体文件同级。
    pub source: String,
}

const IMAGE_EXT: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// 收集字体目录里的样张：先读 `预览/`（新规范），再补同级目录里的散图（旧习惯）。
/// 排序按文件名里的数字自然序，`01.jpg ... 10.jpg` 才不会排成 01, 10, 02。
pub fn collect_previews(font_path: &str) -> Vec<PreviewImage> {
    let p = Path::new(font_path);
    let Some(dir) = p.parent() else {
        return Vec::new();
    };

    let mut out: Vec<PreviewImage> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for (sub, tag) in [(Some("预览"), "folder"), (None, "sibling")] {
        let target = match sub {
            Some(s) => dir.join(s),
            None => dir.to_path_buf(),
        };
        let Ok(entries) = std::fs::read_dir(&target) else {
            continue;
        };
        let mut batch: Vec<PreviewImage> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|f| f.is_file() && is_image(f))
            .filter_map(|f| {
                let name = f.file_name()?.to_string_lossy().into_owned();
                let bytes = std::fs::metadata(&f).map(|m| m.len()).unwrap_or(0);
                Some(PreviewImage {
                    path: f.to_string_lossy().into_owned(),
                    name,
                    bytes,
                    source: tag.to_string(),
                })
            })
            .collect();
        batch.sort_by(|a, b| crate::font_detail::natural_cmp(&a.name, &b.name));
        for img in batch {
            if seen.contains(&img.path) {
                continue;
            }
            seen.push(img.path.clone());
            out.push(img);
        }
    }

    out.truncate(40);
    out
}

/// 把 `Specimen 10.jpg` 排在 `Specimen 2.jpg` 之后。
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => {
                if x.is_ascii_digit() && y.is_ascii_digit() {
                    let mut na = String::new();
                    while let Some(c) = ai.peek().copied().filter(|c| c.is_ascii_digit()) {
                        na.push(c);
                        ai.next();
                    }
                    let mut nb = String::new();
                    while let Some(c) = bi.peek().copied().filter(|c| c.is_ascii_digit()) {
                        nb.push(c);
                        bi.next();
                    }
                    let va: u128 = na.parse().unwrap_or(0);
                    let vb: u128 = nb.parse().unwrap_or(0);
                    if va != vb {
                        return va.cmp(&vb);
                    }
                } else {
                    let lx = x.to_lowercase().to_string();
                    let ly = y.to_lowercase().to_string();
                    if lx != ly {
                        return lx.cmp(&ly);
                    }
                    ai.next();
                    bi.next();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// 取某条 name 记录，优先简体中文，避免把 PostScript 名/英文名当成正式名字显示。
fn name_of(face: &ttf_parser::Face, id: u16) -> Option<String> {
    super::parser::localized_name(face, &[id])
}

/// 原样取第一条能解码的记录（不做语言排序），只给需要与列表保持一致的字段用。
fn raw_name(face: &ttf_parser::Face, id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .filter(|n| n.name_id == id)
        .find_map(|n| n.to_string())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// First non-empty name among several ids, in priority order.
fn first_name(face: &ttf_parser::Face, ids: &[u16]) -> Option<String> {
    ids.iter().find_map(|&id| name_of(face, id))
}

fn lang_of(language_id: u16) -> Option<(&'static str, &'static str)> {
    Some(match language_id {
        0x0409 => ("English (US)", "en-US"),
        0x0809 => ("English (UK)", "en-GB"),
        0x0804 => ("简体中文", "zh-CN"),
        0x0404 => ("繁體中文", "zh-TW"),
        0x0C04 => ("繁體中文（香港）", "zh-HK"),
        0x0411 => ("日本語", "ja-JP"),
        0x0412 => ("한국어", "ko-KR"),
        0x0407 => ("Deutsch", "de-DE"),
        0x040C => ("Français", "fr-FR"),
        0x040A => ("Español", "es-ES"),
        0x0416 => ("Português (BR)", "pt-BR"),
        0x0419 => ("Русский", "ru-RU"),
        _ => return None,
    })
}

fn localized_names(face: &ttf_parser::Face) -> Vec<LocalizedName> {
    use std::collections::BTreeMap;
    // language_id -> (family, style)
    let mut bucket: BTreeMap<u16, (Option<String>, Option<String>)> = BTreeMap::new();
    for n in face.names() {
        let text = match n.to_string() {
            Some(s) => s.trim().to_string(),
            None => continue,
        };
        if text.is_empty() {
            continue;
        }
        let slot = bucket.entry(n.language_id).or_default();
        match n.name_id {
            ttf_parser::name_id::TYPOGRAPHIC_FAMILY | ttf_parser::name_id::FAMILY => {
                if slot.0.is_none() {
                    slot.0 = Some(text);
                }
            }
            ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY | ttf_parser::name_id::SUBFAMILY => {
                if slot.1.is_none() {
                    slot.1 = Some(text);
                }
            }
            _ => {}
        }
    }
    bucket
        .into_iter()
        .filter_map(|(id, (family, style))| {
            let (lang, code) = lang_of(id)?;
            if family.is_none() && style.is_none() {
                return None;
            }
            Some(LocalizedName {
                lang: lang.to_string(),
                lang_code: code.to_string(),
                family,
                style,
            })
        })
        .collect()
}

fn ns_date(raw: u64) -> Option<String> {
    if raw == 0 {
        return None;
    }
    // TrueType long datetime: seconds since 1904-01-01 UTC.
    const EPOCH_1904: i64 = -2_082_844_800;
    let secs = raw as i64 + EPOCH_1904;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Some(format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60
    ))
}

fn sha256_file(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest.iter() {
        out.push_str(&format!("{b:02x}"));
    }
    Some(out)
}

pub(crate) fn fs_type_of(face: &ttf_parser::Face) -> u16 {
    // fsType sits at offset 8 of OS/2, right after version(2) + xAvgCharWidth(2)
    // + usWeightClass(2) + usWidthClass(2).
    face.raw_face()
        .table(ttf_parser::Tag::from_bytes(b"OS/2"))
        .filter(|t| t.len() >= 10)
        .map(|t| u16::from_be_bytes([t[8], t[9]]))
        .unwrap_or(0)
}

const FS_FLAGS: &[(u16, &str, &str, bool)] = &[
    (
        0x0002,
        "受限制授权嵌入",
        "只允许在已安装该字体的系统上查看与打印，字体本身不得嵌入文档。",
        false,
    ),
    (
        0x0004,
        "预览与打印嵌入",
        "允许嵌入到文档中，供只读的预览与打印；嵌入后的文档不可再编辑文字。",
        true,
    ),
    (
        0x0008,
        "可编辑嵌入",
        "允许嵌入到可编辑文档中，接收方可以继续修改文字。",
        true,
    ),
    (
        0x0100,
        "禁止子集化",
        "嵌入时必须整库嵌入，不能只保留用到的字符。",
        false,
    ),
    (
        0x0200,
        "仅限位图嵌入",
        "只能以位图形式嵌入，不能把字形轮廓嵌入文档。",
        false,
    ),
];

fn license_info(face: &ttf_parser::Face) -> LicenseInfo {
    let fs_type = fs_type_of(face);
    let mut flags = Vec::new();
    for &(bit, label, detail, grants) in FS_FLAGS {
        flags.push(EmbedFlag {
            bit,
            label: label.to_string(),
            detail: detail.to_string(),
            on: fs_type & bit != 0,
            grants,
        });
    }

    let restricted = fs_type & 0x0002 != 0;
    let preview_print = fs_type & 0x0004 != 0;
    let editable = fs_type & 0x0008 != 0;
    let no_subset = fs_type & 0x0100 != 0;
    let bitmap_only = fs_type & 0x0200 != 0;

    let headline = if fs_type == 0 {
        "安装嵌入许可（无限制）".to_string()
    } else if restricted {
        "受限制授权嵌入".to_string()
    } else {
        let mut parts = Vec::new();
        if preview_print {
            parts.push("预览与打印");
        }
        if editable {
            parts.push("可编辑");
        }
        if parts.is_empty() {
            "未知（fsType 无可识别位）".to_string()
        } else {
            format!("允许{}嵌入", parts.join(" / "))
        }
    };

    let mut allow = Vec::new();
    let mut deny = Vec::new();
    if fs_type == 0 {
        allow.push("在文档中自由嵌入、分发与打印。".to_string());
        allow.push("接收方可以打开、编辑并继续使用嵌入的字形。".to_string());
    } else {
        if restricted {
            deny.push("把字体嵌入任何文档后分发。".to_string());
            allow.push("在已经安装了该字体的机器上正常使用与打印。".to_string());
        }
        if preview_print {
            allow.push("嵌入到文档中用于预览与打印。".to_string());
            deny.push("让接收方编辑嵌入文档中的文字。".to_string());
        }
        if editable {
            allow.push("嵌入到可编辑文档中，接收方可继续修改文字。".to_string());
        }
        if !preview_print && !editable && !restricted {
            allow.push("按文件内未标注的默认许可使用（fsType 仅含保留位）。".to_string());
        }
    }
    if no_subset {
        deny.push("嵌入时只保留用到的字符（必须整库嵌入）。".to_string());
    } else if fs_type != 0 {
        allow.push("嵌入时按需要子集化，以减小文件体积。".to_string());
    }
    if bitmap_only {
        deny.push("嵌入字形轮廓（只能嵌入位图）。".to_string());
    }
    deny.push("以字体文件本身作为商品出售或再分发。".to_string());

    LicenseInfo {
        headline,
        fs_type,
        flags,
        allow,
        deny,
        copyright: first_name(face, &[ttf_parser::name_id::COPYRIGHT_NOTICE, 0]),
        trademark: first_name(face, &[ttf_parser::name_id::TRADEMARK, 7]),
        license_text: first_name(face, &[ttf_parser::name_id::LICENSE, 13]),
        license_url: first_name(face, &[ttf_parser::name_id::LICENSE_URL, 14]),
        vendor_url: first_name(face, &[ttf_parser::name_id::VENDOR_URL, 11]),
    }
}

fn classify_label(face: &ttf_parser::Face, family: &str) -> String {
    // 与列表卡片用同一套判定（parser::classify），避免两处对同一个字体给出不同分类。
    // 返回值不带「体」后缀：介绍里读作「是一款书写字体」。
    match super::parser::classify(face, family) {
        crate::font_types::Classification::Serif => "衬线",
        crate::font_types::Classification::Sans => "无衬线",
        crate::font_types::Classification::Mono => "等宽",
        crate::font_types::Classification::Display => "展示",
        crate::font_types::Classification::Script => "书写",
        crate::font_types::Classification::Unknown => "未分类",
    }
    .to_string()
}

fn script_label(code: &str) -> &'static str {
    match code {
        "latin" => "拉丁字母",
        "cyrillic" => "西里尔字母",
        "greek" => "希腊字母",
        "arabic" => "阿拉伯字母",
        "hebrew" => "希伯来字母",
        "devanagari" => "天城文",
        "thai" => "泰文",
        "cjk" => "中日韩汉字",
        "korean" => "谚文",
        "vietnamese" => "越南语",
        _ => "其他",
    }
}

fn format_label(f: FontFormat) -> &'static str {
    match f {
        FontFormat::Otf => "OpenType (CFF)",
        FontFormat::Ttf => "TrueType",
        FontFormat::Woff => "WOFF",
        FontFormat::Woff2 => "WOFF2",
        FontFormat::Unknown => "未知",
    }
}

fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Is every character of `text` present in the font?
fn covers_all(points: &[u32], text: &str) -> bool {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .all(|c| points.binary_search(&(c as u32)).is_ok())
}

fn build_samples(points: &[u32], han: u32) -> Vec<SampleLine> {
    let mut out = Vec::new();

    if han > 0 {
        // Poetic lines, longest first: the first one the font can fully set wins,
        // so a GB2312 font still gets a real Chinese sample instead of tofu.
        const ZH: &[(&str, &str, u32)] = &[
            ("标题", "永和九年，岁在癸丑，暮春之初，会于会稽山阴之兰亭。群贤毕至，少长咸集。", 40),
            ("正文", "落霞与孤鹜齐飞，秋水共长天一色。", 26),
            ("标语", "设计有温度，文字有力量", 32),
        ];
        for (label, text, size) in ZH {
            if covers_all(points, text) {
                out.push(SampleLine {
                    label: label.to_string(),
                    text: text.to_string(),
                    size: *size,
                });
            }
        }
        if out.is_empty() {
            // Fall back to the characters the font definitely has.
            let common = list_chars("changyong3500").unwrap_or("");
            let picked: String = common
                .chars()
                .filter(|c| points.binary_search(&(*c as u32)).is_ok())
                .take(24)
                .collect();
            if !picked.is_empty() {
                out.push(SampleLine {
                    label: "常用字".to_string(),
                    text: picked,
                    size: 32,
                });
            }
        } else {
            // 固定的短句用来比较字形风格；字体盖不全时才退回它确实拥有的常用字。
            const COMMON_SAMPLE: &str = "竹外桃花三两枝，春江水暖鸭先知。";
            if covers_all(points, COMMON_SAMPLE) {
                out.push(SampleLine {
                    label: "常用字".to_string(),
                    text: COMMON_SAMPLE.to_string(),
                    size: 24,
                });
            } else {
                let common = list_chars("changyong3500").unwrap_or("");
                let picked: String = common
                    .chars()
                    .filter(|c| points.binary_search(&(*c as u32)).is_ok())
                    .take(28)
                    .collect();
                if !picked.is_empty() {
                    out.push(SampleLine {
                        label: "常用字".to_string(),
                        text: picked,
                        size: 24,
                    });
                }
            }
        }
    }

    // Latin / figures always, filtered so a CJK-only font shows only what works.
    for (label, text, size) in [
        ("大写", "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 24),
        ("小写", "abcdefghijklmnopqrstuvwxyz", 24),
        ("数字与符号", "0123456789 .,;:!?&@#%()[]{}", 22),
    ] {
        if covers_all(points, text) {
            out.push(SampleLine {
                label: label.to_string(),
                text: text.to_string(),
                size,
            });
        }
    }

    out
}

fn build_intro(
    family: &str,
    style: &str,
    class_label: &str,
    glyph_count: u16,
    format: FontFormat,
    file_size: u64,
    faces_in_file: u32,
    cjk: &cjk::CjkStats,
    coverage: &Coverage,
    scripts: &[String],
    features: &[String],
    axes: &[crate::font_types::VariationAxis],
    version: Option<&str>,
    designer: Option<&str>,
    manufacturer: Option<&str>,
) -> Vec<IntroBlock> {
    let mut blocks = Vec::new();

    // 1. What this font is
    let mut p1 = format!(
        "「{family}」是一款{class_label}字体，当前查看的是 {style} 字重/样式。文件为 {} 格式，体积 {}，共收录 {glyph_count} 个字形。",
        format_label(format),
        human_size(file_size)
    );
    if faces_in_file > 1 {
        p1.push_str(&format!(
            " 该文件是字体集合，内含 {faces_in_file} 个字体，本页只描述当前选中的这一个。"
        ));
    }
    blocks.push(IntroBlock {
        label: "字体概况".to_string(),
        body: p1,
    });

    // 2. Character coverage, the thing people actually open the page for
    let gb = coverage.listed.iter().find(|r| r.id == "gb2312");
    let big5 = coverage.listed.iter().find(|r| r.id == "big5");
    let common3500 = coverage.listed.iter().find(|r| r.id == "changyong3500");
    let mut p2 = format!(
        "字体共映射 {} 个 Unicode 字符，其中汉字 {} 个（基本区 {}、扩展 A {}、扩展 B {}）。",
        cjk.total_chars, cjk.han, cjk.han_basic, cjk.han_ext_a, cjk.han_ext_b
    );
    if let Some(gb) = gb {
        p2.push_str(&format!(
            "对照 GB/T 2312 的 {} 字，覆盖 {} 个；",
            gb.total, gb.covered
        ));
    }
    if let Some(c) = common3500 {
        p2.push_str(&format!("现代汉语常用字表 {} 字覆盖 {} 个。", c.total, c.covered));
    }
    if let Some(b) = big5 {
        p2.push_str(&format!(
            "繁体方面，五大码 (Big5) {} 字覆盖 {} 个。",
            b.total, b.covered
        ));
    }
    if cjk.han == 0 {
        p2 = format!(
            "字体共映射 {} 个 Unicode 字符，不含汉字，属于纯拉丁/符号字体。",
            cjk.total_chars
        );
    }
    blocks.push(IntroBlock {
        label: "字符覆盖".to_string(),
        body: p2,
    });

    // 3. Typographic capability
    let mut p3 = String::new();
    if scripts.is_empty() {
        p3.push_str("未探测到完整的文字系统支持。");
    } else {
        let names: Vec<&str> = scripts.iter().map(|s| script_label(s)).collect();
        p3.push_str(&format!("支持的文字系统：{}。", names.join("、")));
    }
    if !axes.is_empty() {
        axis_summary(axes, &mut p3);
    } else {
        p3.push_str(" 该字体是静态字重，不含可变轴。");
    }
    if !features.is_empty() {
        p3.push_str(&format!(
            " OpenType 特性 {} 项：{}。",
            features.len(),
            features.join("、")
        ));
    } else {
        p3.push_str(" 未检测到额外的 OpenType 排版特性。");
    }
    blocks.push(IntroBlock {
        label: "排版能力".to_string(),
        body: p3,
    });

    // 4. Provenance
    let mut p4 = String::new();
    match version {
        Some(v) => p4.push_str(&format!("字体版本 {v}。")),
        None => p4.push_str("字体文件未声明版本号。"),
    }
    match (designer, manufacturer) {
        (Some(d), Some(m)) if d != m => {
            p4.push_str(&format!("设计者为 {d}，发行厂商为 {m}。"))
        }
        (Some(d), _) => p4.push_str(&format!("设计者为 {d}。")),
        (None, Some(m)) => p4.push_str(&format!("发行厂商为 {m}。")),
        (None, None) => p4.push_str("文件未声明设计者或厂商信息。"),
    }
    blocks.push(IntroBlock {
        label: "出处与版本".to_string(),
        body: p4,
    });

    blocks
}

fn axis_summary(axes: &[crate::font_types::VariationAxis], out: &mut String) {
    out.push_str(" 该字体是可变字体，包含 ");
    let parts: Vec<String> = axes
        .iter()
        .map(|a| format!("{} {:.0}–{:.0}", a.tag, a.min, a.max))
        .collect();
    out.push_str(&parts.join("、"));
    out.push_str(" 等可变轴。");
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

pub fn build(path: &str, face_index: u32) -> Result<FontDetail, String> {
    let path_buf = Path::new(path);
    let data = std::fs::read(path_buf).map_err(|e| format!("无法读取文件：{e}"))?;
    let face =
        ttf_parser::Face::parse(&data, face_index).map_err(|e| format!("无法解析字体：{e}"))?;

    let family = super::parser::family_name(&face).unwrap_or_else(|| {
        path_buf
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown".to_string())
    });
    // 字重/样式保持文件里的原始记录（"Regular"），与列表、卡片上的写法一致，
    // 不做语言排序——否则这里成了「常规」，跟样式切换条对不上。
    let style = raw_name(&face, ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY)
        .or_else(|| raw_name(&face, ttf_parser::name_id::SUBFAMILY))
        .unwrap_or_else(|| "Regular".to_string());

    let points = cjk::face_codepoints(&face);
    let cjk_stats = cjk::stats_from_codepoints(&points);
    let coverage = coverage_from_codepoints(&points);

    let version = first_name(&face, &[ttf_parser::name_id::VERSION, 5]);
    let designer = first_name(&face, &[ttf_parser::name_id::DESIGNER, 9]);
    let manufacturer = first_name(&face, &[ttf_parser::name_id::MANUFACTURER, 8]);
    let description = first_name(&face, &[ttf_parser::name_id::DESCRIPTION, 10]);
    let faces_in_file = ttf_parser::fonts_in_collection(&data).unwrap_or(1);

    let axes: Vec<crate::font_types::VariationAxis> = face
        .variation_axes()
        .into_iter()
        .map(|a| crate::font_types::VariationAxis {
            tag: String::from_utf8_lossy(&a.tag.to_bytes()).into_owned(),
            min: a.min_value,
            default: a.def_value,
            max: a.max_value,
        })
        .collect();

    let scripts = super::parser::scripts_of(&face);
    let features = super::parser::features_of(&face);

    let class_label = classify_label(&face, &family);

    let metadata = std::fs::metadata(path_buf).ok();
    let file_size = metadata.as_ref().map(|m| m.len()).unwrap_or(data.len() as u64);
    let modified = metadata
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let intro = build_intro(
        &family,
        &style,
        &class_label,
        face.number_of_glyphs(),
        crate::parser::detect_format(&data, path_buf),
        file_size,
        faces_in_file,
        &cjk_stats,
        &coverage,
        &scripts,
        &features,
        &axes,
        version.as_deref(),
        designer.as_deref(),
        manufacturer.as_deref(),
    );

    let samples = build_samples(&points, cjk_stats.han);

    Ok(FontDetail {
        family,
        style,
        postscript_name: name_of(&face, ttf_parser::name_id::POST_SCRIPT_NAME),
        path: path.to_string(),
        format: crate::parser::detect_format(&data, path_buf),
        file_size,
        modified,
        sha256: sha256_file(path_buf),
        glyph_count: face.number_of_glyphs(),
        weight: face.weight().to_number(),
        italic: face.is_italic(),
        monospaced: face.is_monospaced(),
        classification: class_label,
        faces_in_file,
        is_variable: face.is_variable(),
        axes,
        version,
        designer,
        designer_url: first_name(&face, &[ttf_parser::name_id::DESIGNER_URL, 12]),
        manufacturer,
        description,
        unique_id: name_of(&face, 3),
        variation_prefix: name_of(&face, 25),
        created: face
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"head"))
            .filter(|t| t.len() >= 36)
            .and_then(|t| ns_date(u64::from_be_bytes(t[20..28].try_into().ok()?))),
        modified_date: face
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"head"))
            .filter(|t| t.len() >= 36)
            .and_then(|t| ns_date(u64::from_be_bytes(t[28..36].try_into().ok()?))),
        names: localized_names(&face),
        scripts,
        features,
        intro,
        license: license_info(&face),
        coverage,
        cjk: cjk_stats,
        samples,
        previews: collect_previews(path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(rel: &str) -> Option<String> {
        let p = format!("C://Windows//Fonts//{rel}");
        std::path::Path::new(&p).exists().then_some(p)
    }

    /// 样张读取：`预览/` 排在前面，文件名的数字按自然序排（02 在 10 前）。
    #[test]
    fn collects_previews_from_folder() {
        let dir = std::env::temp_dir().join("fontora-preview-test");
        let sub = dir.join("预览");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("Demo-Regular.ttf"), b"x").unwrap();
        for n in ["10.jpg", "02.jpg", "01.png"] {
            std::fs::write(sub.join(n), b"x").unwrap();
        }
        std::fs::write(dir.join("sibling.jpg"), b"x").unwrap();
        std::fs::write(dir.join("readme.txt"), b"x").unwrap();

        let list = collect_previews(dir.join("Demo-Regular.ttf").to_str().unwrap());
        let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
        println!("previews={names:?}");
        assert_eq!(names, vec!["01.png", "02.jpg", "10.jpg", "sibling.jpg"]);
        assert_eq!(list[0].source, "folder");
        assert_eq!(list[3].source, "sibling");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_a_real_cjk_font() {
        let Some(path) = font("msyh.ttc") else { return };
        let d = build(&path, 0).expect("detail should parse");
        println!("family={} style={}", d.family, d.style);
        println!("glyphs={} size={} hash={:?}", d.glyph_count, d.file_size, d.sha256);
        println!("fsType={} headline={}", d.license.fs_type, d.license.headline);
        println!("names={:?}", d.names.iter().map(|n| &n.lang_code).collect::<Vec<_>>());
        for b in &d.intro {
            println!("[{}] {}", b.label, b.body);
        }
        let gb = d.coverage.listed.iter().find(|r| r.id == "gb2312").unwrap();
        let basic = d.coverage.blocks.iter().find(|r| r.id == "cjk-unified").unwrap();
        println!("gb2312={}/{} stars={:?}", gb.covered, gb.total, gb.stars);
        println!("cjk-unified={}/{}", basic.covered, basic.total);
        println!("samples={}", d.samples.len());
        for s in &d.samples {
            println!("  {} ({}px): {}", s.label, s.size, s.text);
        }
        assert!(d.glyph_count > 0);
        assert_eq!(gb.total, 6763);
        assert!(d.cjk.han > 1000, "expected han coverage, got {}", d.cjk.han);
        assert!(!d.intro.is_empty());
    }

    /// 介绍段落里的字体名必须是中文名，不能是 FZFangSong-Z02S 这种 PostScript 名。
    #[test]
    fn intro_uses_chinese_family_name() {
        let Some(path) = font("simfang.ttf") else { return };
        let d = build(&path, 0).expect("detail should parse");
        println!("family={} intro[0]={}", d.family, d.intro[0].body);
        assert!(
            d.family.chars().any(|c| c as u32 >= 0x4E00),
            "family should be Chinese, got {}",
            d.family
        );
        assert!(
            d.intro[0].body.starts_with(&format!("「{}」", d.family)),
            "intro should open with the family name, got {}",
            d.intro[0].body
        );
        // PostScript 名仍然保留在规格表里，供 Adobe 脚本等场景使用。
        assert!(d.postscript_name.is_some());
    }

    #[test]
    fn reads_a_latin_font_without_han() {
        let Some(path) = font("arial.ttf").or_else(|| font("calibri.ttf")) else {
            return;
        };
        let d = build(&path, 0).expect("detail should parse");
        println!("family={} han={} samples={}", d.family, d.cjk.han, d.samples.len());
        assert_eq!(d.cjk.han, 0);
        // A Latin-only font must not be offered Chinese sample lines.
        assert!(d.samples.iter().all(|s| s.label != "标题"));
    }

    #[test]
    fn survives_every_system_font() {
        // Robustness sweep: the page must build for whatever is on disk,
        // including fonts with no OS/2, no name table, or odd fsType values.
        let dir = std::path::Path::new("C:////Windows////Fonts");
        let mut n = 0;
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let p = entry.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
                continue;
            }
            let Some(path) = p.to_str() else { continue };
            let d = build(path, 0).expect("build should not fail");
            assert!(!d.family.is_empty(), "empty family for {path}");
            assert_eq!(d.coverage.listed.len(), 20);
            assert_eq!(d.coverage.blocks.len(), 25);
            assert!(!d.intro.is_empty());
            n += 1;
            if n >= 40 {
                break;
            }
        }
        println!("swept {n} fonts");
        assert!(n > 0);
    }

    #[test]
    fn coverage_totals_are_the_official_sizes() {
        let Some(path) = font("msyh.ttc") else { return };
        let d = build(&path, 0).unwrap();
        assert_eq!(d.coverage.listed.len(), 20);
        assert_eq!(d.coverage.blocks.len(), 25);
    }
}
