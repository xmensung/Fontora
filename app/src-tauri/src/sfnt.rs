//! sfnt 容器的读写：从 TTC/OTC 集合里拆出单个 face、按新名字重写后另存。
//!
//! 这里不依赖任何字体库做「导出」——只是按 OpenType 规范搬表：
//!
//! * 读：找到 face 的 sfnt 头 → 逐条表记录（TTC 里的偏移是相对整个文件的绝对偏移）
//!       → 把表内容整块拷出来；
//! * 写：按 tag 升序重排表目录、4 字节对齐、重算 `head.checkSumAdjustment`。
//!
//! 拆包/改名之后 `DSIG`（数字签名）必然失效，所以直接丢弃，避免系统做完整性校验时报错。

use std::path::Path;

pub struct SfntTable {
    pub tag: [u8; 4],
    pub data: Vec<u8>,
}

pub struct FaceContainer {
    /// sfnt 版本：0x00010000（TrueType）或 `OTTO`（CFF）。原样保留。
    pub version: u32,
    pub tables: Vec<SfntTable>,
    /// 这个文件里一共有多少个 face（TTC 才有意义）。
    pub faces_in_file: u32,
}

/// 重新打包时要替换掉的 name 记录：家族名、字重、唯一标识、全名、PostScript 名、
/// 以及排版用的 family / subfamily。版权、商标、许可证这些一律保留原样。
const REPLACED_NAME_IDS: &[u16] = &[1, 2, 3, 4, 6, 16, 17];

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// 读出某个 face 的表。`data` 是整个文件的字节（TTC 的偏移都相对文件头）。
pub fn read_face(data: &[u8], face_index: u32) -> Result<FaceContainer, String> {
    let is_collection = data.len() >= 12 && &data[0..4] == b"ttcf";
    let (base, faces_in_file) = if is_collection {
        let count = be32(data, 8).ok_or("TTC 头被截断")?;
        if face_index >= count {
            return Err(format!("这个集合里只有 {count} 个字体"));
        }
        let offset = be32(data, 12 + face_index as usize * 4).ok_or("TTC 偏移表被截断")?;
        (offset as usize, count)
    } else {
        if face_index != 0 {
            return Err("这个文件不是字体集合，只有第 1 个字体".into());
        }
        (0usize, 1)
    };

    if base + 12 > data.len() {
        return Err("字体数据被截断".into());
    }
    let version = be32(data, base).ok_or("sfnt 版本号缺失")?;
    let num_tables = be16(data, base + 4).ok_or("sfnt 表目录缺失")? as usize;
    if num_tables == 0 || num_tables > 600 {
        return Err(format!("表数量异常：{num_tables}"));
    }
    if base + 12 + num_tables * 16 > data.len() {
        return Err("表目录被截断".into());
    }

    let mut tables = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let rec = base + 12 + i * 16;
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&data[rec..rec + 4]);
        // TTC 里的偏移相对整个文件；普通字体相对文件头（base = 0），两者一致。
        let offset = be32(data, rec + 8).ok_or("表偏移损坏")? as usize;
        let length = be32(data, rec + 12).ok_or("表长度损坏")? as usize;
        if offset.checked_add(length).is_none_or(|end| end > data.len()) {
            return Err(format!(
                "表 {} 越界（offset={offset}, length={length}）",
                String::from_utf8_lossy(&tag)
            ));
        }
        if &tag == b"DSIG" {
            continue;
        }
        tables.push(SfntTable {
            tag,
            data: data[offset..offset + length].to_vec(),
        });
    }
    if tables.is_empty() {
        return Err("没有可用的字体表".into());
    }
    Ok(FaceContainer {
        version,
        tables,
        faces_in_file,
    })
}

/// 把表重新打包成一个独立的 sfnt 文件。
pub fn build_sfnt(version: u32, tables: &[SfntTable]) -> Vec<u8> {
    let mut list: Vec<&SfntTable> = tables.iter().collect();
    list.sort_by(|a, b| a.tag.cmp(&b.tag));

    let count = list.len() as u16;
    let mut entry_selector = 0u16;
    while (1u16 << (entry_selector + 1)) <= count {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector) * 16;
    let range_shift = count * 16 - search_range;

    let mut bodies: Vec<Vec<u8>> = Vec::with_capacity(list.len());
    let mut records: Vec<([u8; 4], u32, u32, u32)> = Vec::with_capacity(list.len());
    let mut offset = 12 + count as usize * 16;
    let mut head_adjust_at: Option<usize> = None;

    for table in &list {
        let mut body = table.data.clone();
        if &table.tag == b"head" && body.len() >= 12 {
            // checkSumAdjustment 必须最后按整个文件算，先清零。
            body[8..12].fill(0);
            head_adjust_at = Some(offset + 8);
        }
        records.push((
            table.tag,
            table_checksum(&body),
            offset as u32,
            body.len() as u32,
        ));
        offset += (body.len() + 3) & !3;
        bodies.push(body);
    }

    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    for (tag, checksum, table_offset, length) in &records {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&table_offset.to_be_bytes());
        out.extend_from_slice(&length.to_be_bytes());
    }
    for body in &bodies {
        out.extend_from_slice(body);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }

    if let Some(at) = head_adjust_at {
        let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[at..at + 4].copy_from_slice(&adjust.to_be_bytes());
    }
    out
}

fn table_checksum(data: &[u8]) -> u32 {
    let mut padded = data.to_vec();
    while padded.len() % 4 != 0 {
        padded.push(0);
    }
    checksum(&padded)
}

fn checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks_exact(4) {
        sum = sum.wrapping_add(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    sum
}

struct NameRecord {
    platform: u16,
    encoding: u16,
    language: u16,
    name_id: u16,
    bytes: Vec<u8>,
}

fn parse_name_table(data: &[u8]) -> Vec<NameRecord> {
    let count = be16(data, 2).unwrap_or(0) as usize;
    let string_base = be16(data, 4).unwrap_or(0) as usize;
    let mut out = Vec::new();
    for i in 0..count {
        let at = 6 + i * 12;
        let (Some(platform), Some(encoding), Some(language), Some(name_id), Some(len), Some(off)) = (
            be16(data, at),
            be16(data, at + 2),
            be16(data, at + 4),
            be16(data, at + 6),
            be16(data, at + 8),
            be16(data, at + 10),
        ) else {
            break;
        };
        let start = string_base + off as usize;
        let Some(bytes) = data.get(start..start + len as usize) else {
            continue;
        };
        out.push(NameRecord {
            platform,
            encoding,
            language,
            name_id,
            bytes: bytes.to_vec(),
        });
    }
    out
}

fn build_name_table(records: &[NameRecord]) -> Result<Vec<u8>, String> {
    let mut list: Vec<&NameRecord> = records.iter().collect();
    list.sort_by_key(|r| (r.platform, r.encoding, r.language, r.name_id));

    let string_offset = 6 + list.len() * 12;
    let mut strings: Vec<u8> = Vec::new();
    let mut out = Vec::with_capacity(string_offset);
    out.extend_from_slice(&0u16.to_be_bytes()); // format 0
    out.extend_from_slice(&(list.len() as u16).to_be_bytes());
    out.extend_from_slice(&(string_offset as u16).to_be_bytes());

    for record in list {
        let offset = strings.len();
        if offset > u16::MAX as usize {
            return Err("name 表过大，无法写入".into());
        }
        strings.extend_from_slice(&record.bytes);
        out.extend_from_slice(&record.platform.to_be_bytes());
        out.extend_from_slice(&record.encoding.to_be_bytes());
        out.extend_from_slice(&record.language.to_be_bytes());
        out.extend_from_slice(&record.name_id.to_be_bytes());
        out.extend_from_slice(&(record.bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(&(offset as u16).to_be_bytes());
    }
    if strings.len() > u16::MAX as usize {
        return Err("name 表字符串总长超出限制".into());
    }
    out.extend_from_slice(&strings);
    Ok(out)
}

fn utf16be(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()
}

fn is_ascii_printable(text: &str) -> bool {
    text.chars().all(|c| c.is_ascii() && !c.is_ascii_control())
}

/// PostScript 名只能是 ASCII、不含空格、不超过 63 字节；中文族名没法直接转，
/// 这种情况下保留文件原本的 PostScript 名（它在整个系统里本来就是唯一的）。
fn postscript_name(family: &str, style: &str, fallback: Option<&str>) -> String {
    if is_ascii_printable(family) {
        let mut name: String = format!("{family}-{style}")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        while name.contains("--") {
            name = name.replace("--", "-");
        }
        let name = name.trim_matches('-').to_string();
        if !name.is_empty() {
            return name.chars().take(63).collect();
        }
    }
    fallback
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Renamed-Font".to_string())
}

/// 用新的家族名 / 字重重写 name 表（原记录的其它字段全部保留）。
pub fn rename_tables(tables: &mut [SfntTable], family: &str, style: &str) -> Result<(), String> {
    let family = family.trim();
    let style = if style.trim().is_empty() { "Regular" } else { style.trim() };
    if family.is_empty() {
        return Err("家族名不能为空".into());
    }
    let index = tables
        .iter()
        .position(|t| &t.tag == b"name")
        .ok_or("这个字体没有 name 表")?;
    let mut records = parse_name_table(&tables[index].data);

    let old_postscript = records
        .iter()
        .find(|r| r.name_id == 6)
        .map(|r| String::from_utf16_lossy(
            &r.bytes
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect::<Vec<u16>>(),
        ));
    let ps = postscript_name(family, style, old_postscript.as_deref());

    records.retain(|r| !REPLACED_NAME_IDS.contains(&r.name_id));

    // 简体中文与英文各写一份：中文系统里显示中文，英文环境也不会变成方块。
    let full = if style.eq_ignore_ascii_case("regular") {
        family.to_string()
    } else {
        format!("{family} {style}")
    };
    let unique = format!("{family};{style};1.000;ZFM");
    for language in [0x0409u16, 0x0804u16] {
        for (name_id, text) in [
            (1u16, family),
            (2, style),
            (3, unique.as_str()),
            (4, full.as_str()),
            (6, ps.as_str()),
            (16, family),
            (17, style),
        ] {
            records.push(NameRecord {
                platform: 3,
                encoding: 1,
                language,
                name_id,
                bytes: utf16be(text),
            });
        }
    }

    tables[index].data = build_name_table(&records)?;
    Ok(())
}

/// 文件名里不能出现的字符替换成下划线。
pub fn safe_file_stem(family: &str, style: &str) -> String {
    let raw = if style.eq_ignore_ascii_case("regular") {
        family.to_string()
    } else {
        format!("{family} {style}")
    };
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        "font".to_string()
    } else {
        cleaned
    }
}

pub fn extension_for(version: u32) -> &'static str {
    if version == u32::from_be_bytes(*b"OTTO") {
        "otf"
    } else {
        "ttf"
    }
}

pub fn read_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(Path::new(path)).map_err(|e| format!("读取字体失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system_font(name: &str) -> Option<String> {
        let path = format!("C:/Windows/Fonts/{name}");
        Path::new(&path).exists().then_some(path)
    }

    fn family_of(bytes: &[u8]) -> (String, String) {
        let face = ttf_parser::Face::parse(bytes, 0).expect("打包结果应该能被解析");
        let mut family = String::new();
        let mut style = String::new();
        for n in face.names() {
            let Some(text) = n.to_string() else { continue };
            match n.name_id {
                ttf_parser::name_id::FAMILY => {
                    if family.is_empty() {
                        family = text;
                    }
                }
                ttf_parser::name_id::SUBFAMILY => {
                    if style.is_empty() {
                        style = text;
                    }
                }
                _ => {}
            }
        }
        (family, style)
    }

    #[test]
    fn extracts_each_face_of_a_collection() {
        let Some(path) = system_font("msyh.ttc") else { return };
        let data = std::fs::read(&path).unwrap();
        let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
        assert!(count >= 2, "微软雅黑应该是集合");

        for index in 0..count.min(3) {
            let expected = {
                let face = ttf_parser::Face::parse(&data, index).unwrap();
                let mut family = String::new();
                for n in face.names() {
                    if n.name_id == ttf_parser::name_id::FAMILY {
                        if let Some(text) = n.to_string() {
                            family = text;
                            break;
                        }
                    }
                }
                family
            };
            let container = read_face(&data, index).unwrap();
            let bytes = build_sfnt(container.version, &container.tables);
            assert_eq!(ttf_parser::fonts_in_collection(&bytes).unwrap_or(1), 1);
            let (family, _) = family_of(&bytes);
            assert!(!family.is_empty());
            println!("face {index}: expected={expected} got={family}");
        }
    }

    #[test]
    fn rebuilt_file_has_a_valid_checksum() {
        let Some(path) = system_font("simfang.ttf") else { return };
        let data = std::fs::read(&path).unwrap();
        let container = read_face(&data, 0).unwrap();
        let bytes = build_sfnt(container.version, &container.tables);
        assert_eq!(bytes.len() % 4, 0);
        assert_eq!(checksum(&bytes), 0xB1B0_AFBA);
    }

    #[test]
    fn rename_rewrites_family_and_style() {
        let Some(path) = system_font("simfang.ttf") else { return };
        let data = std::fs::read(&path).unwrap();
        let mut container = read_face(&data, 0).unwrap();
        rename_tables(&mut container.tables, "测试字体", "粗体").unwrap();
        let bytes = build_sfnt(container.version, &container.tables);
        let (family, style) = family_of(&bytes);
        assert_eq!(family, "测试字体");
        assert_eq!(style, "粗体");
        assert_eq!(checksum(&bytes), 0xB1B0_AFBA);
    }

    #[test]
    fn postscript_name_stays_ascii() {
        // 中文族名无法直接当 PostScript 名，应回落到原有的名字。
        assert_eq!(postscript_name("仿宋", "Regular", Some("FZFangSong-Z02S")), "FZFangSong-Z02S");
        // 英文族名则按 Family-Style 生成，空格与非法字符被去掉。
        assert_eq!(postscript_name("Source Han Sans", "Bold", None), "SourceHanSans-Bold");
        assert_eq!(postscript_name("My Font", "Italic", None), "MyFont-Italic");
    }

    #[test]
    fn safe_file_stem_strips_windows_reserved_chars() {
        assert_eq!(safe_file_stem("方正仿宋", "Regular"), "方正仿宋");
        assert_eq!(safe_file_stem("A/B:C", "Bold"), "A_B_C Bold");
    }

    /// 拆包与改名的产物交给 Windows 自己的字体引擎加载一遍。
    /// GDI 对 name 表记录非常挑剔，这条通过就说明写出来的文件能直接安装。
    #[cfg(windows)]
    #[test]
    fn windows_gdi_accepts_unpacked_and_renamed_faces() {
        use windows_sys::Win32::Graphics::Gdi::{AddFontResourceExW, RemoveFontResourceExW};
        const FR_PRIVATE: u32 = 0x10;

        fn gdi_accepts(bytes: &[u8], tag: &str) {
            let dest = std::env::temp_dir().join(format!(
                "zfm-{tag}-{}.ttf",
                std::process::id()
            ));
            std::fs::write(&dest, bytes).unwrap();
            let wide: Vec<u16> = dest
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let added = unsafe { AddFontResourceExW(wide.as_ptr(), FR_PRIVATE, std::ptr::null()) };
            if added > 0 {
                unsafe { RemoveFontResourceExW(wide.as_ptr(), FR_PRIVATE, std::ptr::null()) };
            }
            let _ = std::fs::remove_file(&dest);
            assert!(added > 0, "GDI 拒绝了 {tag}");
        }

        // 1) TTC 拆出来的第一个 face
        if let Some(path) = system_font("msyh.ttc") {
            let data = std::fs::read(&path).unwrap();
            let container = read_face(&data, 0).unwrap();
            gdi_accepts(&build_sfnt(container.version, &container.tables), "unpack");
        }
        // 2) 重命名后的产物（name 表是重写的，最容易写错）
        if let Some(path) = system_font("simfang.ttf") {
            let data = std::fs::read(&path).unwrap();
            let mut container = read_face(&data, 0).unwrap();
            rename_tables(&mut container.tables, "ZFM 测试体", "Regular").unwrap();
            gdi_accepts(&build_sfnt(container.version, &container.tables), "renamed");
        }
    }
}
