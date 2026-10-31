//! 字体体检：把「这个文件到底还能不能用」拆成一条条能看懂的话。
//!
//! 这里刻意不复用 `sfnt::read_face` —— 它是为「导出」写的，遇到截断会直接报错退出；
//! 体检恰恰要把这些错误**当成结果报出来**，而不是让整次检查失败。所以这里自己
//! 走一遍表目录，边读边记问题，坏一点也能继续往下看。
//!
//! 判定口径：
//! * `Error` —— 文件结构坏到系统装不上，或者装上也没字可用；
//! * `Warn`  —— 能装能用，但会踩坑（缺常用字、名字不规范、校验和对不上）；
//! * `Info`  —— 只是情况说明，不算缺陷（比如 `post` 3.0 没有字形名）。

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warn,
    Info,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub level: Level,
    /// 一句话结论，界面上直接当标题。
    pub title: String,
    /// 依据与影响；没有更多可说的时候为空串。
    pub detail: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub tag: String,
    pub length: u32,
    /// 表内容的校验和是否与目录里声明的一致。
    pub checksum_ok: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// 0–100，扣分制。100 表示没查出任何问题。
    pub score: u8,
    pub issues: Vec<Issue>,
    pub tables: Vec<TableInfo>,
    pub face_count: u32,
    pub face_index: u32,
}

struct RawTable {
    tag: [u8; 4],
    offset: usize,
    length: usize,
    checksum: u32,
    in_bounds: bool,
}

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn tag_str(tag: &[u8; 4]) -> String {
    String::from_utf8_lossy(tag).into_owned()
}

/// 表的校验和：按 4 字节大端累加，末尾不足 4 字节补零。
fn table_checksum(bytes: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i < bytes.len() {
        let mut word = [0u8; 4];
        let take = (bytes.len() - i).min(4);
        word[..take].copy_from_slice(&bytes[i..i + take]);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
        i += 4;
    }
    sum
}

/// 宽容地读表目录：越界的表只是标记出来，不中断。
fn table_records(
    data: &[u8],
    face_index: u32,
    issues: &mut Vec<Issue>,
) -> Option<(u32, u32, Vec<RawTable>)> {
    let is_collection = data.len() >= 12 && &data[0..4] == b"ttcf";
    let (base, faces) = if is_collection {
        let count = be32(data, 8)?;
        if face_index >= count {
            issues.push(Issue {
                level: Level::Error,
                title: format!("这个集合里没有第 {} 个字体", face_index + 1),
                detail: format!("文件声明共有 {count} 个字体。"),
            });
            return None;
        }
        (be32(data, 12 + face_index as usize * 4)? as usize, count)
    } else {
        if face_index != 0 {
            issues.push(Issue {
                level: Level::Error,
                title: "这个文件不是字体集合".into(),
                detail: "普通字体文件里只有第 1 个字体。".into(),
            });
            return None;
        }
        (0usize, 1)
    };

    if base + 12 > data.len() {
        issues.push(Issue {
            level: Level::Error,
            title: "字体头被截断".into(),
            detail: format!("表目录应该从第 {base} 字节开始，文件只有 {} 字节。", data.len()),
        });
        return None;
    }
    let version = be32(data, base)?;
    let num_tables = be16(data, base + 4)? as usize;
    if num_tables == 0 {
        issues.push(Issue {
            level: Level::Error,
            title: "文件里一张表都没有".into(),
            detail: "没有表目录就不是字体文件。".into(),
        });
        return None;
    }
    if base + 12 + num_tables * 16 > data.len() {
        issues.push(Issue {
            level: Level::Error,
            title: "表目录被截断".into(),
            detail: format!("声明有 {num_tables} 张表，但文件到不了那么远。"),
        });
        return None;
    }

    let mut tables = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let rec = base + 12 + i * 16;
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&data[rec..rec + 4]);
        let offset = be32(data, rec + 8)? as usize;
        let length = be32(data, rec + 12)? as usize;
        let checksum = be32(data, rec + 4)?;
        let in_bounds = offset
            .checked_add(length)
            .is_some_and(|end| end <= data.len());
        tables.push(RawTable {
            tag,
            offset,
            length,
            checksum,
            in_bounds,
        });
    }
    Some((faces, version, tables))
}

fn push(issues: &mut Vec<Issue>, level: Level, title: impl Into<String>, detail: impl Into<String>) {
    issues.push(Issue {
        level,
        title: title.into(),
        detail: detail.into(),
    });
}

/// 常用字里这个字体缺哪些，最多列 12 个。
fn missing_common(points: &[u32]) -> (u32, Vec<char>) {
    let Some(list) = crate::charsets::list_chars("changyong3500") else {
        return (0, Vec::new());
    };
    let mut missing = 0u32;
    let mut sample = Vec::new();
    for ch in list.chars() {
        let cp = ch as u32;
        let found = points
            .binary_search_by(|probe| probe.cmp(&cp))
            .is_ok();
        if !found {
            missing += 1;
            if sample.len() < 12 {
                sample.push(ch);
            }
        }
    }
    (missing, sample)
}

pub fn build(path: &str, face_index: u32) -> Result<Health, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(inspect(&data, face_index))
}

pub fn inspect(data: &[u8], face_index: u32) -> Health {
    let mut issues: Vec<Issue> = Vec::new();
    let mut tables: Vec<TableInfo> = Vec::new();
    let mut face_count = 1u32;

    let parsed = table_records(data, face_index, &mut issues);
    let Some((faces, _version, records)) = parsed else {
        return health(0, issues, tables, face_count, face_index);
    };
    face_count = faces;

    // ---- 1. 表目录本身 -----------------------------------------------------
    let mut broken = Vec::new();
    let mut bad_checksum = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut duplicated = Vec::new();

    for rec in &records {
        let tag = tag_str(&rec.tag);
        if !seen.insert(tag.clone()) {
            duplicated.push(tag.clone());
        }
        if !rec.in_bounds {
            broken.push(format!(
                "{tag}（偏移 {} + 长度 {} 超出文件末尾）",
                rec.offset, rec.length
            ));
            tables.push(TableInfo {
                tag,
                length: rec.length as u32,
                checksum_ok: false,
            });
            continue;
        }
        let mut body = data[rec.offset..rec.offset + rec.length].to_vec();
        // head 表在目录里的校验和是把 checkSumAdjustment 归零后算的。
        if &rec.tag == b"head" && body.len() >= 12 {
            body[8..12].fill(0);
        }
        let ok = table_checksum(&body) == rec.checksum;
        if !ok {
            bad_checksum.push(tag.clone());
        }
        tables.push(TableInfo {
            tag,
            length: rec.length as u32,
            checksum_ok: ok,
        });
    }

    if !broken.is_empty() {
        push(
            &mut issues,
            Level::Error,
            format!("{} 张表越界，文件被截断", broken.len()),
            broken.join("；"),
        );
    }
    if !duplicated.is_empty() {
        duplicated.sort();
        duplicated.dedup();
        push(
            &mut issues,
            Level::Error,
            "表目录里有重复的表",
            format!("{} 重复出现。规范要求每个 tag 只出现一次。", duplicated.join("、")),
        );
    }
    if !bad_checksum.is_empty() {
        push(
            &mut issues,
            Level::Warn,
            format!("{} 张表的校验和与声明不符", bad_checksum.len()),
            format!(
                "{}。文件很可能被第三方工具改写过（合并、汉化、加壳都会这样）。",
                bad_checksum.join("、")
            ),
        );
    }

    let present: std::collections::BTreeSet<String> = tables.iter().map(|t| t.tag.clone()).collect();
    let has = |t: &str| present.contains(t);

    // ---- 2. 必备表 ---------------------------------------------------------
    let mut missing_required: Vec<&str> = Vec::new();
    for t in ["head", "hhea", "maxp", "cmap", "name"] {
        if !has(t) {
            missing_required.push(t);
        }
    }
    let has_outline = (has("glyf") && has("loca")) || has("CFF ") || has("CFF2");
    if !missing_required.is_empty() {
        push(
            &mut issues,
            Level::Error,
            "缺少必备表",
            format!(
                "{} 不见了。缺这些表的文件装不上，装了也渲染不出字形。",
                missing_required.join("、")
            ),
        );
    }
    if !has_outline {
        push(
            &mut issues,
            Level::Error,
            "找不到字形轮廓数据",
            "既没有 glyf/loca（TrueType 轮廓），也没有 CFF/CFF2（PostScript 轮廓），这个文件里没有可显示的图形。",
        );
    }
    if !has("OS/2") {
        push(
            &mut issues,
            Level::Warn,
            "缺少 OS/2 表",
            "老字体常见。度量信息不全，个别软件里行高会算错。",
        );
    }
    if !has("post") {
        push(
            &mut issues,
            Level::Info,
            "缺少 post 表",
            "不影响显示，但拿不到字形名，图标字体没法按名字查图标。",
        );
    }

    // ---- 3. 真正解析一遍 ---------------------------------------------------
    let parsed_face = ttf_parser::Face::parse(data, face_index);
    let mut points: Vec<u32> = Vec::new();
    match &parsed_face {
        Ok(face) => {
            if face.number_of_glyphs() == 0 {
                push(&mut issues, Level::Error, "字形数为 0", "字体里没有任何字形。");
            }
            points = crate::cjk::face_codepoints(face);

            if face.tables().cmap.is_none() {
                push(
                    &mut issues,
                    Level::Error,
                    "没有 cmap 字符映射表",
                    "系统不知道该把哪个字符画成哪个字形，装上也是空白。",
                );
            } else if points.is_empty() {
                push(
                    &mut issues,
                    Level::Error,
                    "cmap 里没有 Unicode 字符",
                    "只有 Mac Roman 之类的旧编码子表，Windows 下基本用不了。",
                );
            }

            // 度量
            let upm = face.units_per_em();
            if !(16..=16384).contains(&upm) {
                push(
                    &mut issues,
                    Level::Warn,
                    format!("unitsPerEm 取值异常（{upm}）"),
                    "正常在 16–16384 之间，多数是 1000 或 2048。异常值会让字号换算全乱。",
                );
            }

            // 名字
            let family = crate::parser::family_name(face);
            if family.as_deref().unwrap_or("").trim().is_empty() {
                push(
                    &mut issues,
                    Level::Error,
                    "没有字体家族名",
                    "name 表里找不到 nameID 1 或 16。系统列表里会显示成「未知字体」。",
                );
            } else if family.as_ref().is_some_and(|n| n.chars().count() > 63) {
                push(
                    &mut issues,
                    Level::Warn,
                    "家族名过长",
                    "超过 63 个字符，部分旧软件会截断名字，导致同一款字出现多条记录。",
                );
            }

            // 同一条名字可能有多种平台的记录，先碰到的那条未必解得出来 ——
            // 用 find_map 继续往后找，别因为第一条是 Mac Roman 就判「没有」。
            let ps = face.names().into_iter().find_map(|n| {
                (n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
                    .then(|| n.to_string())
                    .flatten()
            });
            match ps.as_deref() {
                None => push(
                    &mut issues,
                    Level::Warn,
                    "缺少 PostScript 名",
                    "PDF、AI 里引用这款字时会退回用家族名，容易撞名。",
                ),
                Some(name) => {
                    if !name.is_ascii() || name.contains(' ') {
                        push(
                            &mut issues,
                            Level::Warn,
                            "PostScript 名含非法字符",
                            format!("「{name}」含空格或非 ASCII 字符。规范只允许 33–126 的可打印 ASCII。"),
                        );
                    }
                }
            }

            // 嵌入权限
            let fs_type = crate::font_detail::fs_type_of(face);
            if fs_type & !0x0F0Fu16 != 0 {
                push(
                    &mut issues,
                    Level::Warn,
                    format!("fsType 含未定义的位（0x{fs_type:04X}）"),
                    "OpenType 规范只定义到第 11 位。多余的位置位说明这个值是硬改出来的，嵌入判定不一定可信。",
                );
            }
        }
        Err(e) => {
            push(
                &mut issues,
                Level::Error,
                "字体无法解析",
                format!("{e}。文件可能已损坏，或使用的是一种不支持的容器格式。"),
            );
        }
    }

    // ---- 4. 字符覆盖 -------------------------------------------------------
    if !points.is_empty() {
        let ascii_missing: Vec<char> = (0x20u8..=0x7E)
            .filter_map(|b| {
                let ch = b as char;
                points.binary_search(&(b as u32)).is_err().then_some(ch)
            })
            .collect();
        if !ascii_missing.is_empty() {
            push(
                &mut issues,
                Level::Warn,
                format!("缺少 {} 个基本拉丁字符", ascii_missing.len()),
                format!(
                    "缺：{}。英文和标点会显示成方框或回落到别的字体。",
                    ascii_missing.iter().collect::<String>()
                ),
            );
        }

        // 拉丁字体天然没有汉字，拿 3500 常用字去量它只会制造噪音。
        // 只有汉字数量到了「这款字体确实在做中文」的量级才检查。
        let han = crate::cjk::stats_from_codepoints(&points).han;
        let (missing, sample) = if han >= 500 { missing_common(&points) } else { (0, Vec::new()) };
        if han >= 500 && missing > 0 {
            let level = if missing > 500 { Level::Warn } else { Level::Info };
            push(
                &mut issues,
                level,
                format!("现代汉语常用字表中缺 {missing} 字"),
                if sample.is_empty() {
                    "共 3500 字。".to_string()
                } else {
                    format!(
                        "例如：{}…（共 3500 字，缺 {missing} 个）。",
                        sample.iter().collect::<String>()
                    )
                },
            );
        } else if han >= 500 {
            push(
                &mut issues,
                Level::Info,
                "现代汉语常用字表 3500 字全部覆盖",
                String::new(),
            );
        }
    }

    // ---- 5. 多 face 的集合文件 --------------------------------------------
    if face_count > 1 {
        let broken_faces: Vec<u32> = (0..face_count)
            .filter(|i| ttf_parser::Face::parse(data, *i).is_err())
            .collect();
        if !broken_faces.is_empty() {
            push(
                &mut issues,
                Level::Warn,
                format!("集合里有 {} 个字体读不出来", broken_faces.len()),
                format!(
                    "第 {} 个。拆包导出时这些会被跳过。",
                    broken_faces
                        .iter()
                        .map(|i| (i + 1).to_string())
                        .collect::<Vec<_>>()
                        .join("、")
                ),
            );
        }
    }

    let score = score_of(&issues);
    health(score, issues, tables, face_count, face_index)
}

fn health(
    score: u8,
    issues: Vec<Issue>,
    tables: Vec<TableInfo>,
    face_count: u32,
    face_index: u32,
) -> Health {
    // 错误排前面，同级里保持检出顺序（先结构、后字符）。
    let mut sorted = issues;
    sorted.sort_by_key(|i| match i.level {
        Level::Error => 0,
        Level::Warn => 1,
        Level::Info => 2,
    });
    Health {
        score,
        issues: sorted,
        tables,
        face_count,
        face_index,
    }
}

/// 评分只反映「有没有毛病」：说明性条目（`Info`）不扣分。
/// 否则一款完全正常的字体会因为「post 是 3.0 版」这种正常写法被扣成 99，很奇怪。
fn score_of(issues: &[Issue]) -> u8 {
    let mut score = 100i32;
    for issue in issues {
        score -= match issue.level {
            Level::Error => 25,
            Level::Warn => 6,
            Level::Info => 0,
        };
    }
    score.clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn system_font(name: &str) -> Option<String> {
        let path = format!("C:/Windows/Fonts/{name}");
        Path::new(&path).exists().then_some(path)
    }

    fn levels(h: &Health) -> Vec<Level> {
        h.issues.iter().map(|i| i.level).collect()
    }

    #[test]
    fn a_healthy_system_font_scores_high() {
        let Some(path) = system_font("simfang.ttf") else {
            return;
        };
        let h = build(&path, 0).unwrap();
        assert!(h.score >= 90, "评分偏低：{}，问题：{:?}", h.score, h.issues);
        assert!(!levels(&h).contains(&Level::Error));
        assert!(h.tables.iter().any(|t| t.tag == "glyf"));
        assert!(h.tables.iter().all(|t| t.checksum_ok));
    }

    #[test]
    fn truncated_file_is_reported_not_panicked() {
        let Some(path) = system_font("simfang.ttf") else {
            return;
        };
        let data = std::fs::read(&path).unwrap();
        // 砍掉尾巴，表内容大面积越界。
        let h = inspect(&data[..data.len() / 3], 0);
        assert!(h.score <= 60, "评分应该被扣下来，实际 {}", h.score);
        assert!(h
            .issues
            .iter()
            .any(|i| i.level == Level::Error && i.title.contains("越界")));
        // 结构坏掉时不该再输出「缺某某表」这种二次推断出来的噪音。
        assert!(!h.issues.iter().any(|i| i.title == "缺少必备表"));
    }

    #[test]
    fn a_face_index_beyond_the_collection_is_an_error() {
        let Some(path) = system_font("msyh.ttc") else {
            return;
        };
        let data = std::fs::read(&path).unwrap();
        let h = inspect(&data, 99);
        assert_eq!(h.score, 0);
        assert!(h.issues.iter().any(|i| i.title.contains("没有第")));
    }

    #[test]
    fn garbage_bytes_fail_softly() {
        let h = inspect(&[0u8; 64], 0);
        assert_eq!(h.score, 0);
        assert!(!h.issues.is_empty());
    }

    /// 拉丁字体没有汉字是常态，不能因此扣分或报警 —— 这类误报比不报更烦人。
    #[test]
    fn latin_only_fonts_are_not_blamed_for_missing_han() {
        let Some(path) = system_font("arial.ttf") else {
            return;
        };
        let h = build(&path, 0).unwrap();
        assert!(!h.issues.iter().any(|i| i.title.contains("常用字表")));
        assert_eq!(h.score, 100, "问题：{:?}", h.issues);
    }

    #[test]
    fn score_drops_by_severity() {
        let mk = |level| Issue {
            level,
            title: String::new(),
            detail: String::new(),
        };
        assert_eq!(score_of(&[]), 100);
        assert_eq!(score_of(&[mk(Level::Error)]), 75);
        assert_eq!(score_of(&[mk(Level::Warn)]), 94);
        assert_eq!(score_of(&[mk(Level::Info)]), 100);
        let many: Vec<Issue> = (0..5).map(|_| mk(Level::Error)).collect();
        assert_eq!(score_of(&many), 0);
    }
}
