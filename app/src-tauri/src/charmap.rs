//! 字符地图：把 `cmap` 压成「区间表」，让界面能一口气铺开几万个格子。
//!
//! 一款中文字体的 cmap 动辄两万多个码位。如果直接把它们塞进 JSON，光传输就是
//! 几百 KB，界面还要为每个码位建一个 DOM 节点 —— 翻两页就开始卡。
//!
//! 这里换个存法：相邻码位合并成 `[start, end]` 区间，两万个连续汉字只占一条记录。
//! 于是「这个字有没有」变成对区间表做一次二分查找，格子按需渲染，滚动始终顺滑。
//!
//! 私用区（PUA）单独把字形名带出来 —— 图标字体没有语义，`uE900` 这种码位本身
//! 说明不了问题，`icon-home` 才是人看得懂的名字。

use serde::{Deserialize, Serialize};

/// 一个 Unicode 区段在界面上的呈现。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CharBlock {
    pub id: String,
    pub label: String,
    pub start: u32,
    pub end: u32,
    /// 这款字体在这个区段里实际有多少个字符。
    pub covered: u32,
}

/// 私用区码位 → 字形名（来自 `post` 表）。
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GlyphName {
    pub cp: u32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CharMap {
    /// cmap 里的字符总数。
    pub total: u32,
    /// 有序、互不重叠的闭区间表。
    pub ranges: Vec<[u32; 2]>,
    /// 只包含真正有字符的区段，按码位升序。
    pub blocks: Vec<CharBlock>,
    pub glyph_names: Vec<GlyphName>,
    /// 是否存在 `liga` / `rlig` 连字特性 —— 图标字体靠它把 `home` 变成一个图标。
    pub has_ligatures: bool,
    /// 落在上面所有区段之外、没被展示出来的字符数。
    pub outside: u32,
}

/// 界面要铺开的区段。只列常用与中文字体真正会碰到的，顺序即码位顺序。
const BLOCKS: &[(&str, &str, u32, u32)] = &[
    ("basic-latin", "基本拉丁字母", 0x0020, 0x007E),
    ("latin-1", "拉丁-1 补充", 0x00A0, 0x00FF),
    ("latin-ext-a", "拉丁扩展 A", 0x0100, 0x017F),
    ("latin-ext-b", "拉丁扩展 B", 0x0180, 0x024F),
    ("ipa", "国际音标", 0x0250, 0x02AF),
    ("combining", "组合附加符号", 0x0300, 0x036F),
    ("greek", "希腊字母", 0x0370, 0x03FF),
    ("cyrillic", "西里尔字母", 0x0400, 0x04FF),
    ("hebrew", "希伯来文", 0x0590, 0x05FF),
    ("arabic", "阿拉伯文", 0x0600, 0x06FF),
    ("general-punct", "通用标点", 0x2000, 0x206F),
    ("super-sub", "上标与下标", 0x2070, 0x209F),
    ("currency", "货币符号", 0x20A0, 0x20CF),
    ("letterlike", "字母式符号", 0x2100, 0x214F),
    ("number-forms", "数字形式", 0x2150, 0x218F),
    ("arrows", "箭头", 0x2190, 0x21FF),
    ("math", "数学运算符", 0x2200, 0x22FF),
    ("box-drawing", "制表符", 0x2500, 0x257F),
    ("geometric", "几何图形", 0x25A0, 0x25FF),
    ("misc-symbols", "杂项符号", 0x2600, 0x26FF),
    ("dingbats", "装饰符号", 0x2700, 0x27BF),
    ("cjk-rad-supp", "CJK 部首补充", 0x2E80, 0x2EFF),
    ("kangxi", "康熙部首", 0x2F00, 0x2FDF),
    ("cjk-symbols", "CJK 符号与标点", 0x3000, 0x303F),
    ("hiragana", "平假名", 0x3040, 0x309F),
    ("katakana", "片假名", 0x30A0, 0x30FF),
    ("bopomofo", "注音符号", 0x3100, 0x312F),
    ("hangul-compat", "谚文兼容字母", 0x3130, 0x318F),
    ("cjk-strokes", "CJK 笔画", 0x31C0, 0x31EF),
    ("enclosed-cjk", "带圈 CJK", 0x3200, 0x32FF),
    ("cjk-compat", "CJK 兼容", 0x3300, 0x33FF),
    ("cjk-ext-a", "CJK 扩展 A", 0x3400, 0x4DBF),
    ("cjk", "中日韩统一表意文字", 0x4E00, 0x9FFF),
    ("hangul", "谚文音节", 0xAC00, 0xD7AF),
    ("pua", "私用区", 0xE000, 0xF8FF),
    ("cjk-compat-ideo", "CJK 兼容汉字", 0xF900, 0xFAFF),
    ("vertical-forms", "竖排标点", 0xFE10, 0xFE1F),
    ("cjk-compat-forms", "CJK 兼容形式", 0xFE30, 0xFE4F),
    ("small-forms", "小写变体", 0xFE50, 0xFE6F),
    ("halfwidth-fullwidth", "半角及全角形式", 0xFF00, 0xFFEF),
    ("special", "特殊符号", 0xFFF0, 0xFFFF),
    ("cjk-ext-b", "CJK 扩展 B", 0x20000, 0x2A6DF),
    ("cjk-ext-c", "CJK 扩展 C", 0x2A700, 0x2B739),
    ("cjk-ext-d", "CJK 扩展 D", 0x2B740, 0x2B81D),
    ("cjk-ext-e", "CJK 扩展 E", 0x2B820, 0x2CEA1),
    ("cjk-ext-f", "CJK 扩展 F", 0x2CEB0, 0x2EBE0),
    ("cjk-ext-i", "CJK 扩展 I", 0x2EBF0, 0x2EE5D),
    ("cjk-ext-g", "CJK 扩展 G", 0x30000, 0x3134A),
    ("cjk-ext-h", "CJK 扩展 H", 0x31350, 0x323AF),
    ("pua-15", "私用区 15 平面", 0xF0000, 0xFFFFD),
    ("pua-16", "私用区 16 平面", 0x100000, 0x10FFFD),
];

/// 私用区：图标字体把图形放在这里，没有 Unicode 语义。
const PUA_RANGES: &[(u32, u32)] = &[
    (0xE000, 0xF8FF),
    (0xF0000, 0xFFFFD),
    (0x100000, 0x10FFFD),
];

/// 字形名最多带这么多条。图标字体很少有超过两千个图标的，再多也没人看得过来。
const MAX_GLYPH_NAMES: usize = 4096;

/// 相邻码位合并成闭区间。
pub fn compress(points: &[u32]) -> Vec<[u32; 2]> {
    let mut ranges: Vec<[u32; 2]> = Vec::new();
    for &cp in points {
        match ranges.last_mut() {
            Some(last) if cp == last[1] + 1 => last[1] = cp,
            Some(last) if cp == last[1] => {}
            _ => ranges.push([cp, cp]),
        }
    }
    ranges
}

/// 区间表里有没有这个码位。`ranges` 必须按上面的规则有序。
///
/// 界面那边有一份等价的实现（TS 里也是二分），这份留在 Rust 侧作为语义基准，
/// 只在测试里用 —— 所以不需要进 release 产物。
#[cfg(test)]
pub fn contains(ranges: &[[u32; 2]], cp: u32) -> bool {
    let mut lo = 0usize;
    let mut hi = ranges.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let [start, end] = ranges[mid];
        if cp < start {
            hi = mid;
        } else if cp > end {
            lo = mid + 1;
        } else {
            return true;
        }
    }
    false
}

fn in_pua(cp: u32) -> bool {
    PUA_RANGES.iter().any(|&(a, b)| cp >= a && cp <= b)
}

fn has_ligature(table: Option<ttf_parser::opentype_layout::LayoutTable>) -> bool {
    let Some(table) = table else { return false };
    table
        .features
        .into_iter()
        .any(|f| matches!(&f.tag.to_bytes(), b"liga" | b"rlig"))
}

pub fn build(path: &str, face_index: u32) -> Result<CharMap, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let face = ttf_parser::Face::parse(&data, face_index).map_err(|e| e.to_string())?;
    Ok(build_from_face(&face))
}

pub fn build_from_face(face: &ttf_parser::Face) -> CharMap {
    let points = crate::cjk::face_codepoints(face);
    let ranges = compress(&points);

    let mut blocks = Vec::new();
    let mut shown = 0u32;
    for &(id, label, start, end) in BLOCKS {
        let from = points.partition_point(|&cp| cp < start);
        let to = points.partition_point(|&cp| cp <= end);
        let covered = (to.saturating_sub(from)) as u32;
        if covered == 0 {
            continue;
        }
        shown += covered;
        blocks.push(CharBlock {
            id: id.to_string(),
            label: label.to_string(),
            start,
            end,
            covered,
        });
    }

    // 只给私用区取字形名：别的区段有 Unicode 名字，不需要从 post 表里翻。
    let mut glyph_names = Vec::new();
    for &cp in points.iter().filter(|cp| in_pua(**cp)) {
        if glyph_names.len() >= MAX_GLYPH_NAMES {
            break;
        }
        let Some(ch) = char::from_u32(cp) else { continue };
        let Some(gid) = face.glyph_index(ch) else { continue };
        let Some(name) = face.glyph_name(gid) else { continue };
        // `post` 格式 3.0 的字形名是 `glyph00012` 这种占位符，没有信息量。
        if name.starts_with("glyph") && name[5..].bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        glyph_names.push(GlyphName {
            cp,
            name: name.to_string(),
        });
    }

    CharMap {
        total: points.len() as u32,
        outside: (points.len() as u32).saturating_sub(shown),
        ranges,
        blocks,
        glyph_names,
        has_ligatures: has_ligature(face.tables().gsub),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn system_font(name: &str) -> Option<String> {
        let path = format!("C:/Windows/Fonts/{name}");
        Path::new(&path).exists().then_some(path)
    }

    #[test]
    fn adjacent_codepoints_collapse_into_one_range() {
        let ranges = compress(&[0x41, 0x42, 0x43, 0x4E00, 0x4E01, 0x4E9F]);
        assert_eq!(ranges, vec![[0x41, 0x43], [0x4E00, 0x4E01], [0x4E9F, 0x4E9F]]);
    }

    #[test]
    fn duplicate_codepoints_do_not_open_a_new_range() {
        let ranges = compress(&[0x41, 0x41, 0x42]);
        assert_eq!(ranges, vec![[0x41, 0x42]]);
    }

    #[test]
    fn contains_finds_the_edges_and_misses_the_gaps() {
        let ranges = vec![[0x41, 0x43], [0x4E00, 0x4E10]];
        assert!(contains(&ranges, 0x41));
        assert!(contains(&ranges, 0x43));
        assert!(contains(&ranges, 0x4E00));
        assert!(contains(&ranges, 0x4E10));
        assert!(!contains(&ranges, 0x40));
        assert!(!contains(&ranges, 0x44));
        assert!(!contains(&ranges, 0x9FFF));
    }

    #[test]
    fn a_real_cjk_font_reports_the_han_block() {
        let Some(path) = system_font("simfang.ttf") else {
            return;
        };
        let map = build(&path, 0).expect("系统字体应该能读");
        let han = map
            .blocks
            .iter()
            .find(|b| b.id == "cjk")
            .expect("仿宋应该有汉字区段");
        assert!(han.covered > 6000, "仿宋汉字数偏少：{}", han.covered);
        assert_eq!(map.total, map.ranges.iter().map(|r| r[1] - r[0] + 1).sum::<u32>());
        assert!(contains(&map.ranges, '中' as u32));
        // 区间表是要过 IPC 传到界面的，压缩必须有效：两万多个码位不该摊成两万条记录。
        assert!(
            map.ranges.len() < 2000,
            "区间数偏多（{}），压缩没起到作用",
            map.ranges.len()
        );
    }

    #[test]
    fn blocks_are_ordered_and_non_overlapping() {
        let Some(path) = system_font("msyh.ttc") else {
            return;
        };
        let map = build(&path, 0).expect("微软雅黑应该能读");
        for pair in map.blocks.windows(2) {
            assert!(
                pair[0].end < pair[1].start,
                "区段重叠或乱序：{:?} 之后是 {:?}",
                pair[0],
                pair[1]
            );
        }
    }
}
