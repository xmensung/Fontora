//! Character coverage statistics for a font face.
//!
//! Reads the font's `cmap` table, deduplicates the codepoints, and counts how
//! many fall into each Unicode block. The Han ("CJK") totals are what the
//! original Python tool `CJK-character-count` reports, reimplemented here in
//! Rust so the app needs no Python runtime.
//!
//! Everything in this module is pure: it depends only on `ttf-parser` and
//! `serde`, so it can also be compiled into the standalone `cjk_report` tool.

use serde::{Deserialize, Serialize};

/// One Unicode block and how many of its codepoints the font covers.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BlockCount {
    /// Chinese name, matching the wording used by CJK-character-count.
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub count: u32,
}

/// Coverage summary for a single face.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CjkStats {
    /// Distinct codepoints present in the cmap.
    pub total_chars: u32,
    /// Every Han ideograph, all blocks summed.
    pub han: u32,
    /// U+4E00..U+9FFF — the everyday block most people mean by "汉字数".
    pub han_basic: u32,
    pub han_ext_a: u32,
    pub han_ext_b: u32,
    pub han_ext_c: u32,
    pub han_ext_d: u32,
    pub han_ext_e: u32,
    pub han_ext_f: u32,
    pub han_ext_g: u32,
    pub han_ext_h: u32,
    pub han_ext_i: u32,
    /// CJK Compatibility Ideographs + its supplement.
    pub han_compat: u32,
    pub kana: u32,
    pub hangul: u32,
    pub latin: u32,
    /// Full-width / half-width forms.
    pub fullwidth: u32,
    /// Blocks with at least one covered codepoint, largest first.
    pub blocks: Vec<BlockCount>,
}

/// (name, first, last) — ordered roughly by size so the common ones come first.
const BLOCKS: &[(&str, u32, u32)] = &[
    ("基本拉丁字母", 0x0000, 0x007F),
    ("拉丁字母补充", 0x0080, 0x00FF),
    ("拉丁字母扩展-A", 0x0100, 0x017F),
    ("拉丁字母扩展-B", 0x0180, 0x024F),
    ("国际音标扩展", 0x0250, 0x02AF),
    ("希腊字母", 0x0370, 0x03FF),
    ("西里尔字母", 0x0400, 0x04FF),
    ("亚美尼亚字母", 0x0530, 0x058F),
    ("希伯来字母", 0x0590, 0x05FF),
    ("阿拉伯字母", 0x0600, 0x06FF),
    ("泰文", 0x0E00, 0x0E7F),
    ("藏文", 0x0F00, 0x0FFF),
    ("缅甸文", 0x1000, 0x109F),
    ("格鲁吉亚字母", 0x10A0, 0x10FF),
    ("朝鲜文综合扩展", 0x1100, 0x11FF),
    ("切罗基字母", 0x13A0, 0x13FF),
    ("越南文扩展", 0x1E00, 0x1EFF),
    ("希腊字母扩展", 0x1F00, 0x1FFF),
    ("标点符号", 0x2000, 0x206F),
    ("上标与下标", 0x2070, 0x209F),
    ("货币符号", 0x20A0, 0x20CF),
    ("字母式符号", 0x2100, 0x214F),
    ("数字形式", 0x2150, 0x218F),
    ("箭头符号", 0x2190, 0x21FF),
    ("数学运算符", 0x2200, 0x22FF),
    ("技术符号", 0x2300, 0x23FF),
    ("制表符", 0x2500, 0x257F),
    ("方块元素", 0x2580, 0x259F),
    ("几何图形", 0x25A0, 0x25FF),
    ("杂项符号", 0x2600, 0x26FF),
    ("装饰符号", 0x2700, 0x27BF),
    ("中日韩符号和标点", 0x3000, 0x303F),
    ("平假名", 0x3040, 0x309F),
    ("片假名", 0x30A0, 0x30FF),
    ("注音符号", 0x3100, 0x312F),
    ("谚文兼容字母", 0x3130, 0x318F),
    ("中日韩笔画", 0x31C0, 0x31EF),
    ("中日韩字母及注音扩展", 0x31F0, 0x31FF),
    ("带圈中日韩字母", 0x3200, 0x32FF),
    ("中日韩兼容字符", 0x3300, 0x33FF),
    ("中日韩统一表意文字扩展A", 0x3400, 0x4DBF),
    ("中日韩统一表意文字", 0x4E00, 0x9FFF),
    ("朝鲜文音节", 0xAC00, 0xD7AF),
    ("中日韩兼容表意文字", 0xF900, 0xFAFF),
    ("字母表现形式", 0xFB00, 0xFB4F),
    ("阿拉伯字母表现形式-A", 0xFB50, 0xFDFF),
    ("竖排形式", 0xFE10, 0xFE1F),
    ("中日韩兼容形式", 0xFE30, 0xFE4F),
    ("小写变体形式", 0xFE50, 0xFE6F),
    ("半角及全角形式", 0xFF00, 0xFFEF),
    ("特殊字符", 0xFFF0, 0xFFFF),
    ("中日韩统一表意文字扩展B", 0x20000, 0x2A6DF),
    ("中日韩统一表意文字扩展C", 0x2A700, 0x2B73F),
    ("中日韩统一表意文字扩展D", 0x2B740, 0x2B81F),
    ("中日韩统一表意文字扩展E", 0x2B820, 0x2CEAF),
    ("中日韩统一表意文字扩展F", 0x2CEB0, 0x2EBEF),
    ("中日韩统一表意文字扩展I", 0x2EBF0, 0x2EE5F),
    ("中日韩兼容表意文字补充", 0x2F800, 0x2FA1F),
    ("中日韩统一表意文字扩展G", 0x30000, 0x3134F),
    ("中日韩统一表意文字扩展H", 0x31350, 0x323AF),
    ("私用区", 0xE000, 0xF8FF),
    ("补充私用区-A", 0xF0000, 0xFFFFD),
];

/// Han ranges, narrowest first. `han` is the sum over all of them.
const HAN_RANGES: &[(u32, u32)] = &[
    (0x3400, 0x4DBF),   // Ext A
    (0x4E00, 0x9FFF),   // basic
    (0xF900, 0xFAFF),   // compatibility
    (0x20000, 0x2A6DF), // Ext B
    (0x2A700, 0x2B73F), // Ext C
    (0x2B740, 0x2B81F), // Ext D
    (0x2B820, 0x2CEAF), // Ext E
    (0x2CEB0, 0x2EBEF), // Ext F
    (0x2EBF0, 0x2EE5F), // Ext I
    (0x2F800, 0x2FA1F), // compatibility supplement
    (0x30000, 0x3134F), // Ext G
    (0x31350, 0x323AF), // Ext H
];

fn in_range(cp: u32, start: u32, end: u32) -> bool {
    cp >= start && cp <= end
}

/// Collect every distinct codepoint the face maps, sorted ascending.
pub fn face_codepoints(face: &ttf_parser::Face) -> Vec<u32> {
    let mut points: Vec<u32> = Vec::with_capacity(1024);
    if let Some(cmap) = face.tables().cmap {
        for subtable in cmap.subtables {
            // Unicode subtables only: symbol / Mac-Roman ones would report
            // codepoints that mean nothing outside their own encoding.
            if !subtable.is_unicode() {
                continue;
            }
            subtable.codepoints(|cp| points.push(cp));
        }
    }
    points.sort_unstable();
    points.dedup();
    points
}

/// Count coverage over `points` (already sorted + deduped).
pub fn stats_from_codepoints(points: &[u32]) -> CjkStats {
    let mut stats = CjkStats {
        total_chars: points.len() as u32,
        ..Default::default()
    };

    let mut blocks: Vec<BlockCount> = Vec::new();
    for &(name, start, end) in BLOCKS {
        // Codepoints are sorted: binary search both ends of the range.
        let from = points.partition_point(|&cp| cp < start);
        let to = points.partition_point(|&cp| cp <= end);
        let count = (to.saturating_sub(from)) as u32;
        if count > 0 {
            blocks.push(BlockCount {
                name: name.to_string(),
                start,
                end,
                count,
            });
        }
    }
    blocks.sort_by(|a, b| b.count.cmp(&a.count).then(a.start.cmp(&b.start)));

    for &cp in points {
        if in_range(cp, 0x4E00, 0x9FFF) {
            stats.han_basic += 1;
        } else if in_range(cp, 0x3400, 0x4DBF) {
            stats.han_ext_a += 1;
        } else if in_range(cp, 0x20000, 0x2A6DF) {
            stats.han_ext_b += 1;
        } else if in_range(cp, 0x2A700, 0x2B73F) {
            stats.han_ext_c += 1;
        } else if in_range(cp, 0x2B740, 0x2B81F) {
            stats.han_ext_d += 1;
        } else if in_range(cp, 0x2B820, 0x2CEAF) {
            stats.han_ext_e += 1;
        } else if in_range(cp, 0x2CEB0, 0x2EBEF) {
            stats.han_ext_f += 1;
        } else if in_range(cp, 0x2EBF0, 0x2EE5F) {
            stats.han_ext_i += 1;
        } else if in_range(cp, 0x30000, 0x3134F) {
            stats.han_ext_g += 1;
        } else if in_range(cp, 0x31350, 0x323AF) {
            stats.han_ext_h += 1;
        } else if in_range(cp, 0xF900, 0xFAFF) || in_range(cp, 0x2F800, 0x2FA1F) {
            stats.han_compat += 1;
        }

        if in_range(cp, 0x3040, 0x30FF) {
            stats.kana += 1;
        }
        if in_range(cp, 0xAC00, 0xD7AF) || in_range(cp, 0x1100, 0x11FF) {
            stats.hangul += 1;
        }
        if in_range(cp, 0x0000, 0x024F) {
            stats.latin += 1;
        }
        if in_range(cp, 0xFF00, 0xFFEF) {
            stats.fullwidth += 1;
        }
    }

    stats.han = HAN_RANGES
        .iter()
        .map(|&(start, end)| {
            let from = points.partition_point(|&cp| cp < start);
            let to = points.partition_point(|&cp| cp <= end);
            to.saturating_sub(from) as u32
        })
        .sum();

    stats.blocks = blocks;
    stats
}

/// Convenience wrapper: parse the face, then count.
pub fn stats_for_face(face: &ttf_parser::Face) -> CjkStats {
    stats_from_codepoints(&face_codepoints(face))
}

/// One-line summary for logs and lists.
impl CjkStats {
    #[allow(dead_code)] // used by the app's log line and future detail view
    pub fn summary(&self) -> String {
        format!(
            "汉字 {}（基本 {} / 扩展A {} / 扩展B {}）/ 总字符 {}",
            self.han, self.han_basic, self.han_ext_a, self.han_ext_b, self.total_chars
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_consistent() {
        // A hand-made set: 3 basic ideographs, 1 Ext A, 2 latin, 1 kana.
        let points = vec![0x0041, 0x0042, 0x3042, 0x3400, 0x4E00, 0x4E8C, 0x9FFF];
        let s = stats_from_codepoints(&points);
        assert_eq!(s.total_chars, 7);
        assert_eq!(s.han_basic, 3);
        assert_eq!(s.han_ext_a, 1);
        assert_eq!(s.han, 4);
        assert_eq!(s.latin, 2);
        assert_eq!(s.kana, 1);
    }

    #[test]
    fn empty_font_is_zero() {
        let s = stats_from_codepoints(&[]);
        assert_eq!(s.total_chars, 0);
        assert_eq!(s.han, 0);
        assert!(s.blocks.is_empty());
    }

    #[test]
    fn blocks_are_sorted_by_count() {
        let points = vec![0x0041, 0x4E00, 0x4E8C, 0x4E09];
        let s = stats_from_codepoints(&points);
        assert_eq!(s.blocks[0].count, 3);
        assert!(s.blocks.windows(2).all(|w| w[0].count >= w[1].count));
    }
}
