use crate::font_types::{Classification, FontFace, FontFormat, FontSource, VariationAxis};
use std::fs;
use std::path::Path;

const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc", "woff", "woff2"];

pub fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| FONT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub(crate) fn detect_format(data: &[u8], path: &Path) -> FontFormat {
    if data.len() >= 4 {
        match &data[0..4] {
            b"OTTO" => return FontFormat::Otf,
            [0x00, 0x01, 0x00, 0x00] | b"true" | b"ttcf" => return FontFormat::Ttf,
            b"wOFF" => return FontFormat::Woff,
            b"wOF2" => return FontFormat::Woff2,
            _ => {}
        }
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("otf" | "otc") => FontFormat::Otf,
        Some("ttf" | "ttc") => FontFormat::Ttf,
        Some("woff") => FontFormat::Woff,
        Some("woff2") => FontFormat::Woff2,
        _ => FontFormat::Unknown,
    }
}

fn name_string(face: &ttf_parser::Face, id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .filter(|n| n.name_id == id)
        .find_map(|n| n.to_string())
}

/// 在给定的一组 name id 里挑最合适的一条记录：简体中文 > 繁体中文 > 其它语言 > 英文。
///
/// 同一个 id 在字体里往往有 en-US / zh-CN 多份记录，`face.names()` 的返回顺序由文件
/// 决定，直接取第一条会拿到英文名（FZFangSong-Z02S 而不是 方正仿宋简体）。这里显式排序。
/// 语言相同时 Windows 平台记录优先——Mac 平台的单字节编码容易出乱码。
pub(crate) fn localized_name(face: &ttf_parser::Face, ids: &[u16]) -> Option<String> {
    fn lang_rank(language_id: u16) -> u8 {
        match language_id {
            0x0804 | 0x1004 => 0,          // 简体中文 zh-CN / zh-SG
            0x0404 | 0x0C04 | 0x1404 => 1, // 繁体中文
            0x0409 | 0x0809 => 3,          // 英文
            _ => 2,                        // 其他语言
        }
    }

    let mut best: Option<(u8, usize, u8, String)> = None;
    for n in face.names() {
        let Some(id_rank) = ids.iter().position(|&id| id == n.name_id) else {
            continue;
        };
        let Some(text) = n.to_string() else { continue };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let platform_rank = u8::from(n.platform_id != ttf_parser::PlatformId::Windows);
        let key = (lang_rank(n.language_id), id_rank, platform_rank);
        let better = best.as_ref().map_or(true, |b| key < (b.0, b.1, b.2));
        if better {
            best = Some((key.0, key.1, key.2, text.to_string()));
        }
    }
    best.map(|b| b.3)
}

/// 字体族名（family）的选名策略：文件里带了简体中文名就用中文名
/// （仿宋 / 宋体 / 微软雅黑），没有中文名的西文字体（Ebrima 等）才回落到英文名。
///
/// 只在 TYPOGRAPHIC_FAMILY / FAMILY 两个字段里挑，subfamily 一律保留文件原文，
/// 避免 "Regular" 这类内部判定（面板默认字重、详情页样例）被翻译后的名字打断。
pub(crate) fn family_name(face: &ttf_parser::Face) -> Option<String> {
    localized_name(
        face,
        &[
            ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
            ttf_parser::name_id::FAMILY,
        ],
    )
}

const SCRIPT_PROBES: &[(&str, &[u32])] = &[
    ("latin", &[0x0041, 0x007A]),
    ("cyrillic", &[0x0410, 0x044F]),
    ("greek", &[0x0391, 0x03C9]),
    ("arabic", &[0x0627, 0x0645]),
    ("hebrew", &[0x05D0, 0x05EA]),
    ("devanagari", &[0x0915, 0x093E]),
    ("thai", &[0x0E01, 0x0E32]),
    ("cjk", &[0x4E00, 0x3042]),
    ("korean", &[0xAC00, 0xD55C]),
    ("vietnamese", &[0x1EA1, 0x01B0]),
];

fn script_coverage(face: &ttf_parser::Face) -> Vec<String> {
    SCRIPT_PROBES
        .iter()
        .filter(|(_, probes)| {
            probes
                .iter()
                .all(|&cp| char::from_u32(cp).and_then(|c| face.glyph_index(c)).is_some())
        })
        .map(|(name, _)| (*name).to_string())
        .collect()
}

/// Reused by the detail page so its "支持的文字系统" line agrees with the card.
pub(crate) fn scripts_of(face: &ttf_parser::Face) -> Vec<String> {
    script_coverage(face)
}

/// Reused by the detail page for its OpenType feature list.
pub(crate) fn features_of(face: &ttf_parser::Face) -> Vec<String> {
    layout_features(face)
}

/// `is_monospaced()` 只看 post 表的 isFixedPitch 位，很多中文字体（仿宋、宋体）
/// 为了兼容压缩/排版会把它置 1，实际并不是等宽。再用 i / W 的字宽复核一次。
fn really_monospaced(face: &ttf_parser::Face) -> bool {
    if !face.is_monospaced() {
        return false;
    }
    let (Some(i), Some(w)) = (face.glyph_index('i'), face.glyph_index('W')) else {
        return false;
    };
    match (face.glyph_hor_advance(i), face.glyph_hor_advance(w)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

pub(crate) fn classify(face: &ttf_parser::Face, family: &str) -> Classification {
    if really_monospaced(face) {
        return Classification::Mono;
    }
    if let Some(os2) = face.raw_face().table(ttf_parser::Tag::from_bytes(b"OS/2")) {
        if os2.len() >= 42 {
            let class = os2[30];
            match class {
                1..=5 | 7 => return Classification::Serif,
                8 => return Classification::Sans,
                9 | 12 => return Classification::Display,
                10 => return Classification::Script,
                _ => {}
            }
            let panose = &os2[32..42];
            // panose 的比例位（第 4 字节）写 9 表示等宽，但中文黑体/宋体/仿宋
            // 常常整段填 9 却并不是等宽，所以要同时满足 post 表的等宽标记。
            if panose[0] == 2 && panose[3] == 9 && face.is_monospaced() {
                return Classification::Mono;
            }
            match panose[0] {
                3 => return Classification::Script,
                4 | 5 => return Classification::Display,
                2 => {
                    if (11..=15).contains(&panose[1]) {
                        return Classification::Sans;
                    }
                    if (2..=10).contains(&panose[1]) {
                        return Classification::Serif;
                    }
                    // 中文黑体/宋体/仿宋的 panose 常写成 (2,1,6,9,…)，这一档信息
                    // 不足以下结论，继续用族名兜底（宋 → 衬线、黑 → 无衬线、仿宋 → 书写）。
                }
                _ => {}
            }
        }
    }
    classify_by_name(family)
}

/// 族名关键字兜底：西文字体看 mono / script / sans / serif，
/// 中文字体看 仿宋 / 楷 / 隶 / 黑 / 宋 这些叫法（中文名不会因为 lowercase 变化）。
pub(crate) fn classify_by_name(family: &str) -> Classification {
    let name = family.to_lowercase();
    // 书写类要排在「宋」前面：仿宋里也带「宋」字，但它不是衬线。
    if ["仿宋", "楷", "隶", "行书", "草书", "手写", "舒体", "硬笔"]
        .iter()
        .any(|k| name.contains(k))
    {
        return Classification::Script;
    }
    if ["黑", "圆", "等线", "幼圆"].iter().any(|k| name.contains(k)) {
        return Classification::Sans;
    }
    if ["宋", "明", "报", "书宋"].iter().any(|k| name.contains(k)) {
        return Classification::Serif;
    }
    if name.contains("mono") {
        Classification::Mono
    } else if name.contains("script") || name.contains("hand") {
        Classification::Script
    } else if name.contains("sans") {
        Classification::Sans
    } else if name.contains("serif") {
        Classification::Serif
    } else {
        Classification::Unknown
    }
}

fn layout_features(face: &ttf_parser::Face) -> Vec<String> {
    const KEEP: [&str; 18] = [
        "liga", "dlig", "calt", "smcp", "c2sc", "onum", "lnum", "tnum", "pnum", "frac", "ordn",
        "sups", "subs", "zero", "salt", "swsh", "titl", "hist",
    ];
    let mut tags: Vec<String> = Vec::new();
    let mut collect = |table: Option<ttf_parser::opentype_layout::LayoutTable>| {
        if let Some(t) = table {
            for f in t.features {
                let tag = String::from_utf8_lossy(&f.tag.to_bytes()).into_owned();
                let stylistic_set = tag.len() == 4
                    && tag.starts_with("ss")
                    && tag[2..].chars().all(|c| c.is_ascii_digit())
                    && tag[2..] != *"00";
                if KEEP.contains(&tag.as_str()) || stylistic_set {
                    tags.push(tag);
                }
            }
        }
    };
    collect(face.tables().gsub);
    collect(face.tables().gpos);
    tags.sort_unstable();
    tags.dedup();
    tags
}

fn parse_face(
    data: &[u8],
    index: u32,
    path: &Path,
    format: FontFormat,
    file_size: u64,
    modified: u64,
    source: FontSource,
) -> Option<FontFace> {
    let face = ttf_parser::Face::parse(data, index).ok()?;

    let family =
        family_name(&face).unwrap_or_else(|| fallback_family(path));
    let style = name_string(&face, ttf_parser::name_id::TYPOGRAPHIC_SUBFAMILY)
        .or_else(|| name_string(&face, ttf_parser::name_id::SUBFAMILY))
        .unwrap_or_else(|| "Regular".to_string());
    let postscript_name = name_string(&face, ttf_parser::name_id::POST_SCRIPT_NAME);
    let foundry = name_string(&face, ttf_parser::name_id::MANUFACTURER)
        .or_else(|| name_string(&face, ttf_parser::name_id::DESIGNER));
    let license = name_string(&face, ttf_parser::name_id::LICENSE);
    let license_url = name_string(&face, ttf_parser::name_id::LICENSE_URL);

    let axes: Vec<VariationAxis> = face
        .variation_axes()
        .into_iter()
        .map(|a| VariationAxis {
            tag: String::from_utf8_lossy(&a.tag.to_bytes()).into_owned(),
            min: a.min_value,
            default: a.def_value,
            max: a.max_value,
        })
        .collect();

    let classification = classify(&face, &family);
    let scripts = script_coverage(&face);
    let features = layout_features(&face);
    let path_str = path.to_string_lossy().into_owned();
    Some(FontFace {
        id: format!("{}#{}", path_str, index),
        path: path_str,
        preview_path: None,
        face_index: index,
        family,
        style,
        postscript_name,
        foundry,
        license,
        license_url,
        format,
        is_variable: face.is_variable(),
        axes,
        weight: face.weight().to_number(),
        italic: face.is_italic(),
        monospaced: face.is_monospaced(),
        classification,
        scripts,
        features,
        glyph_count: face.number_of_glyphs(),
        file_size,
        modified,
        source,
        deactivatable: crate::activation::can_deactivate(source),
        active: true,
    })
}

fn fallback_family(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub fn parse_font_file(path: &Path, source: FontSource) -> Vec<FontFace> {
    let modified = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut faces = parse_font_file_at(path, source);
    for face in &mut faces {
        face.modified = modified;
    }
    faces
}

fn parse_font_file_at(path: &Path, source: FontSource) -> Vec<FontFace> {
    let Ok(data) = fs::read(path) else {
        return Vec::new();
    };
    let file_size = data.len() as u64;
    let format = detect_format(&data, path);

    if matches!(format, FontFormat::Woff | FontFormat::Woff2) {
        let path_str = path.to_string_lossy().into_owned();
        return vec![FontFace {
            id: format!("{}#0", path_str),
            path: path_str,
            preview_path: None,
            face_index: 0,
            family: fallback_family(path),
            style: "Regular".to_string(),
            postscript_name: None,
            foundry: None,
            license: None,
            license_url: None,
            format,
            is_variable: false,
            axes: Vec::new(),
            weight: 400,
            italic: false,
            monospaced: false,
            classification: Classification::Unknown,
            scripts: Vec::new(),
            features: Vec::new(),
            glyph_count: 0,
            file_size,
            modified: 0,
            source,
            deactivatable: crate::activation::can_deactivate(source),
            active: true,
        }];
    }

    let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    (0..count)
        .filter_map(|i| parse_face(&data, i, path, format, file_size, 0, source))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_font() -> Option<std::path::PathBuf> {
        walkdir::WalkDir::new("/usr/share/fonts")
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| e.into_path())
            .find(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
            })
    }

    #[test]
    fn parses_a_real_system_font() {
        let Some(path) = system_font() else { return };
        let faces = parse_font_file(&path, FontSource::System);
        assert!(!faces.is_empty(), "no faces parsed from {path:?}");
        let f = &faces[0];
        assert!(!f.family.is_empty());
        assert!(f.file_size > 0);
    }

    #[test]
    fn rejects_non_font_files() {
        assert!(!is_font_file(Path::new("/tmp/readme.txt")));
        assert!(is_font_file(Path::new("/tmp/Inter.ttf")));
        assert!(is_font_file(Path::new("/tmp/Inter.WOFF2")));
    }

    /// Windows 上没有中文名就别指望中文，但有中文名就必须用中文名。
    #[test]
    fn prefers_chinese_family_names() {
        let dir = Path::new("C:/Windows/Fonts");
        if !dir.exists() {
            return;
        }
        let mut checked = 0;
        for name in ["simfang.ttf", "simsun.ttc", "msyh.ttc", "Deng.ttf"] {
            let path = dir.join(name);
            if !path.exists() {
                continue;
            }
            let faces = parse_font_file(&path, FontSource::System);
            let Some(face) = faces.first() else { continue };
            assert!(
                face.family.chars().any(|c| c as u32 >= 0x2E80),
                "{} 的族名没有取到中文：{}",
                name,
                face.family
            );
            checked += 1;
        }
        assert!(checked > 0, "系统里没有可用于校验的字体");
    }
}

