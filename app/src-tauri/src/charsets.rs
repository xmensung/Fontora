//! Named character-set coverage — the detail page's 「字数统计」 table.
//!
//! Each row answers one question: *of the N characters this standard defines,
//! how many does the font actually contain?* A single "汉字数" total hides the
//! thing people actually care about: a font can carry 27k ideographs and still
//! miss characters from the 3500 常用字表.
//!
//! The lists live in `data/charsets/` and are compiled into the binary with
//! `include_str!`, so coverage works offline with no external dependency.

use serde::{Deserialize, Serialize};

/// Which column / sub-group a row belongs to.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    /// 简体中文编码
    Simp,
    /// 简繁中文编码
    Both,
    /// 繁体中文编码
    Trad,
    /// 统一码区段
    Block,
}

/// One row: `covered / total`, plus the star rating.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRow {
    pub id: String,
    pub label: String,
    pub group: Group,
    pub covered: u32,
    pub total: u32,
    /// 0–5, `None` for block rows (the reference only grades the named lists).
    pub stars: Option<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    /// 字表 rows, in reading order.
    pub listed: Vec<CoverageRow>,
    /// 统一码区段 rows, in reading order.
    pub blocks: Vec<CoverageRow>,
}

/// Five stars means the standard is essentially complete, one star means the
/// font barely participates. Matches the grading used by the reference page.
pub fn stars(covered: u32, total: u32) -> u8 {
    if total == 0 || covered == 0 {
        return 0;
    }
    let pct = covered as f32 / total as f32;
    (((pct * 5.0).floor()) as u8).clamp(1, 5)
}

// ---------------------------------------------------------------------------
// Named standards (vendored). Order is the reading order of the table.
// ---------------------------------------------------------------------------

struct Listed {
    id: &'static str,
    label: &'static str,
    group: Group,
    data: &'static str,
}

macro_rules! listed {
    ($id:expr, $label:expr, $group:expr, $file:expr) => {
        Listed {
            id: $id,
            label: $label,
            group: $group,
            data: include_str!(concat!("../data/charsets/", $file)),
        }
    };
}

const LISTED: &[Listed] = &[
    // 简体中文编码
    listed!("gb2312", "GB/T 2312", Group::Simp, "gb2312.txt"),
    listed!(
        "changyong3500",
        "现代汉语常用字表",
        Group::Simp,
        "changyong3500.txt"
    ),
    listed!(
        "tongyong7000",
        "现代汉语通用字表",
        Group::Simp,
        "tongyong7000.txt"
    ),
    listed!(
        "yiwu3500",
        "义务教育语文课程常用字表",
        Group::Simp,
        "yiwu3500.txt"
    ),
    listed!(
        "guifan8105",
        "通用规范汉字表",
        Group::Simp,
        "guifan8105.txt"
    ),
    listed!("pinyin-pro", "汉语拼音字母（专业）", Group::Simp, "pinyin-pro.txt"),
    listed!("pinyin", "汉语拼音字母", Group::Simp, "pinyin.txt"),
    // 简繁中文编码
    listed!("hanyi", "汉仪简繁字表", Group::Both, "hanyi.txt"),
    listed!("fangzheng", "方正简繁字表", Group::Both, "fangzheng.txt"),
    listed!(
        "iicore",
        "国际表意文字核心 (IICore)",
        Group::Both,
        "iicore.txt"
    ),
    listed!("gbk", "GBK", Group::Both, "gbk.txt"),
    listed!("gb18030", "GB18030", Group::Both, "gb18030.txt"),
    // 繁体中文编码
    listed!(
        "tw4808",
        "常用国字标准字体表",
        Group::Trad,
        "tw4808.txt"
    ),
    listed!(
        "tw6343",
        "次常用国字标准字体表",
        Group::Trad,
        "tw6343.txt"
    ),
    listed!("big5", "五大码 (Big5)", Group::Trad, "big5.txt"),
    listed!(
        "big5common",
        "五大码 (Big5) 常用汉字表",
        Group::Trad,
        "big5common.txt"
    ),
    listed!("hkchangyong", "常用字字形表", Group::Trad, "hkchangyong.txt"),
    listed!(
        "hkscs",
        "香港增补字符集 (HKSCS)",
        Group::Trad,
        "hkscs.txt"
    ),
    listed!(
        "suppchara",
        "常用香港外字表 (1-6级)",
        Group::Trad,
        "suppchara.txt"
    ),
    listed!("gb12345", "GB/T 12345", Group::Trad, "gb12345.txt"),
];

// ---------------------------------------------------------------------------
// Unicode blocks. `total` is the official block size, not the number of
// assigned characters, so the denominator stays stable across Unicode versions.
// ---------------------------------------------------------------------------

struct Block {
    id: &'static str,
    label: &'static str,
    total: u32,
    ranges: &'static [(u32, u32)],
}

const BLOCKS: &[Block] = &[
    Block {
        id: "kangxi",
        label: "康熙部首",
        total: 214,
        ranges: &[(0x2F00, 0x2FD5)],
    },
    Block {
        id: "radicals-supp",
        label: "汉字部首补充",
        total: 115,
        ranges: &[(0x2E80, 0x2EF3)],
    },
    Block {
        id: "ling",
        label: "〇",
        total: 1,
        ranges: &[(0x3007, 0x3007)],
    },
    Block {
        id: "cjk-unified",
        label: "中日韩统一表意文字",
        total: 20992,
        ranges: &[(0x4E00, 0x9FFF)],
    },
    Block {
        id: "cjk-ext-a",
        label: "中日韩统一表意文字扩展A",
        total: 6592,
        ranges: &[(0x3400, 0x4DBF)],
    },
    Block {
        id: "cjk-ext-b",
        label: "中日韩统一表意文字扩展B",
        total: 42720,
        ranges: &[(0x20000, 0x2A6DF)],
    },
    Block {
        id: "cjk-ext-c",
        label: "中日韩统一表意文字扩展C",
        total: 4154,
        ranges: &[(0x2A700, 0x2B739)],
    },
    Block {
        id: "cjk-ext-d",
        label: "中日韩统一表意文字扩展D",
        total: 222,
        ranges: &[(0x2B740, 0x2B81D)],
    },
    Block {
        id: "cjk-ext-e",
        label: "中日韩统一表意文字扩展E",
        total: 5762,
        ranges: &[(0x2B820, 0x2CEA1)],
    },
    Block {
        id: "cjk-ext-f",
        label: "中日韩统一表意文字扩展F",
        total: 7473,
        ranges: &[(0x2CEB0, 0x2EBE0)],
    },
    Block {
        id: "cjk-ext-g",
        label: "中日韩统一表意文字扩展G",
        total: 4939,
        ranges: &[(0x30000, 0x3134A)],
    },
    Block {
        id: "cjk-ext-h",
        label: "中日韩统一表意文字扩展H",
        total: 4192,
        ranges: &[(0x31350, 0x323AF)],
    },
    Block {
        id: "cjk-ext-i",
        label: "中日韩统一表意文字扩展I",
        total: 622,
        ranges: &[(0x2EBF0, 0x2EE5D)],
    },
    Block {
        id: "cjk-compat",
        label: "中日韩兼容表意文字",
        total: 512,
        ranges: &[(0xF900, 0xFAFF)],
    },
    Block {
        id: "cjk-compat-supp",
        label: "中日韩兼容表意文字补充",
        total: 542,
        ranges: &[(0x2F800, 0x2FA1D)],
    },
    Block {
        id: "hiragana",
        label: "平假名",
        total: 96,
        ranges: &[(0x3040, 0x309F)],
    },
    Block {
        id: "katakana",
        label: "片假名",
        total: 96,
        ranges: &[(0x30A0, 0x30FF)],
    },
    Block {
        id: "bopomofo",
        label: "注音符号",
        total: 43,
        ranges: &[(0x3100, 0x312F)],
    },
    Block {
        id: "hangul-syllables",
        label: "谚文音节",
        total: 11172,
        ranges: &[(0xAC00, 0xD7A3)],
    },
    Block {
        id: "hangul-compat",
        label: "谚文兼容字母",
        total: 96,
        ranges: &[(0x3130, 0x318F)],
    },
    Block {
        id: "cjk-symbols",
        label: "中日韩符号和标点",
        total: 64,
        ranges: &[(0x3000, 0x303F)],
    },
    Block {
        id: "cjk-strokes",
        label: "中日韩笔画",
        total: 48,
        ranges: &[(0x31C0, 0x31EF)],
    },
    Block {
        id: "cjk-compat-chars",
        label: "中日韩兼容字符",
        total: 256,
        ranges: &[(0x3300, 0x33FF)],
    },
    Block {
        id: "cjk-compat-forms",
        label: "中日韩兼容形式",
        total: 32,
        ranges: &[(0xFE30, 0xFE4F)],
    },
    Block {
        id: "fulllwidth",
        label: "半角及全角形式",
        total: 240,
        ranges: &[(0xFF00, 0xFFEF)],
    },
];

fn count_in_ranges(points: &[u32], ranges: &[(u32, u32)]) -> u32 {
    // Codepoints are sorted ascending and deduped, so each range is two
    // binary searches. Ranges never overlap, so the counts simply add up.
    ranges
        .iter()
        .map(|&(start, end)| {
            let from = points.partition_point(|&cp| cp < start);
            let to = points.partition_point(|&cp| cp <= end);
            to.saturating_sub(from) as u32
        })
        .sum()
}

/// Build the full table for a font's codepoint set.
///
/// 下面的二分查找要求输入有序去重；调用方（cmap 遍历）本来就是有序的，
/// 这里再兜一层：万一传进来的是乱序切片，先排序复制，避免静默漏计。
pub fn coverage_from_codepoints(points: &[u32]) -> Coverage {
    let sorted: Option<Vec<u32>> = if points.windows(2).all(|w| w[0] < w[1]) {
        None
    } else {
        let mut v = points.to_vec();
        v.sort_unstable();
        v.dedup();
        Some(v)
    };
    let points: &[u32] = sorted.as_deref().unwrap_or(points);

    let listed = LISTED
        .iter()
        .map(|l| {
            let total = l.data.chars().count() as u32;
            let covered = l
                .data
                .chars()
                .filter(|c| {
                    let cp = *c as u32;
                    points.binary_search(&cp).is_ok()
                })
                .count() as u32;
            CoverageRow {
                id: l.id.to_string(),
                label: l.label.to_string(),
                group: l.group,
                covered,
                total,
                stars: Some(stars(covered, total)),
            }
        })
        .collect();

    let blocks = BLOCKS
        .iter()
        .map(|b| {
            let covered = count_in_ranges(points, b.ranges);
            CoverageRow {
                id: b.id.to_string(),
                label: b.label.to_string(),
                group: Group::Block,
                covered,
                total: b.total,
                stars: None,
            }
        })
        .collect();

    Coverage { listed, blocks }
}

/// A named list by id — used to pick sample characters the font actually has.
pub fn list_chars(id: &str) -> Option<&'static str> {
    LISTED.iter().find(|l| l.id == id).map(|l| l.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_what_the_font_has() {
        // 一 二 三 are all in GB/T 2312; a font with only those three must
        // report 3/6763 and one star.
        let points = vec![0x4E00u32, 0x4E8C, 0x4E09];
        let cov = coverage_from_codepoints(&points);
        let gb = cov.listed.iter().find(|r| r.id == "gb2312").unwrap();
        assert_eq!(gb.covered, 3);
        assert_eq!(gb.total, 6763);
        assert_eq!(gb.stars, Some(1));
    }

    #[test]
    fn empty_font_covers_nothing() {
        let cov = coverage_from_codepoints(&[]);
        assert!(cov.listed.iter().all(|r| r.covered == 0));
        assert!(cov.listed.iter().all(|r| r.stars == Some(0)));
        assert!(cov.blocks.iter().all(|r| r.covered == 0));
    }

    #[test]
    fn block_totals_match_official_sizes() {
        let cov = coverage_from_codepoints(&[]);
        let basic = cov.blocks.iter().find(|r| r.id == "cjk-unified").unwrap();
        assert_eq!(basic.total, 20992);
        let ext_b = cov.blocks.iter().find(|r| r.id == "cjk-ext-b").unwrap();
        assert_eq!(ext_b.total, 42720);
    }

    #[test]
    fn star_scale_is_one_to_five() {
        assert_eq!(stars(0, 100), 0);
        assert_eq!(stars(1, 100), 1);
        assert_eq!(stars(50, 100), 2);
        assert_eq!(stars(100, 100), 5);
    }
}
