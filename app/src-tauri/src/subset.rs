//! 子集化 + 网页字体封装（WOFF / WOFF2）。
//!
//! 只处理 TrueType 轮廓（有 `glyf`/`loca` 的字体）。CFF/OTF 的轮廓在 `CFF ` 表里，
//! 子集化要写整个 charstring 解释器，代价和收益不成比例，所以直接给出明确报错。
//!
//! 子集化的做法是「重建而不是就地改」：
//!
//! 1. 用保留的字符集查 cmap 得到 glyph id，再把复合字形递归引用到的字形补进来；
//! 2. 按旧 id 升序重新编号，重写 `glyf` / `loca` / `hmtx` / `hhea` / `maxp` / `head`；
//! 3. 重建 `cmap`（format 4 + format 12）；
//! 4. `post` 统一降级成 3.0（2.0 里是字形名列表，序号一变就全错位）；
//! 5. 凡是内部带 glyph id 的表（GDEF/GPOS/GSUB/kern/hdmx…）一律丢掉。
//!
//! WOFF2 这里用的是「null transform」：`glyf`/`loca` 按 transformVersion=3 原样存，
//! 这样不需要实现 glyf 的三段式重编码，仍然是完全合规的 WOFF2，浏览器都能读。

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use super::sfnt::{build_sfnt, read_face, SfntTable};

/// 依赖 glyph 序号、子集后必然对不上的表。
///
/// GDEF/GPOS/GSUB 看着诱人，但里面的 coverage / class set 全是 glyph id，不重映射
/// 就会指向错误字形（看起来像乱码）；重映射又要写掉大半个 OpenType 布局引擎。
/// 网页字体绝大多数只用到 cmap + 轮廓，所以这里选择丢掉，而不是留一个坏掉的表。
const DROP: [&str; 24] = [
    "DSIG", "GDEF", "GPOS", "GSUB", "MATH", "BASE", "JSTF", "MORX", "mort", "kerx", "kern",
    "hdmx", "LTSH", "EBDT", "EBLC", "EBSC", "CBDT", "CBLC", "sbix", "VORG", "vhea", "vmtx",
    "gvar", "HVAR",
];

fn is_dropped(tag: &[u8; 4]) -> bool {
    let name = String::from_utf8_lossy(tag);
    DROP.contains(&name.as_ref())
}

fn u16at(d: &[u8], at: usize) -> u16 {
    d.get(at..at + 2)
        .and_then(|s| s.try_into().ok())
        .map(u16::from_be_bytes)
        .unwrap_or(0)
}

fn i16at(d: &[u8], at: usize) -> i16 {
    u16at(d, at) as i16
}

fn u32at(d: &[u8], at: usize) -> u32 {
    d.get(at..at + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_be_bytes)
        .unwrap_or(0)
}

fn put_u16(d: &mut [u8], at: usize, v: u16) {
    if at + 2 <= d.len() {
        d[at..at + 2].copy_from_slice(&v.to_be_bytes());
    }
}

fn range_into(set: &mut BTreeSet<u32>, a: u32, b: u32) {
    for cp in a..=b {
        set.insert(cp);
    }
}

/// 保留哪些字符。`all` 表示不裁字符（只做格式转换）。
pub fn charset_preset(kind: &str, custom: &str) -> BTreeSet<u32> {
    let mut set = BTreeSet::new();
    match kind {
        "all" => {}
        "latin" => {
            range_into(&mut set, 0x0020, 0x007E);
            range_into(&mut set, 0x00A0, 0x024F);
            range_into(&mut set, 0x2000, 0x206F);
            range_into(&mut set, 0x20A0, 0x20BF);
            range_into(&mut set, 0x2100, 0x214F);
            set.insert(0xFEFF);
        }
        // 简体常用：基本汉字 + 中文标点 + 全角 + 假名 + 拉丁。
        "zh" => {
            range_into(&mut set, 0x0020, 0x007E);
            range_into(&mut set, 0x00A0, 0x024F);
            range_into(&mut set, 0x2000, 0x206F);
            range_into(&mut set, 0x20A0, 0x20BF);
            range_into(&mut set, 0x3000, 0x303F);
            range_into(&mut set, 0x3040, 0x30FF);
            range_into(&mut set, 0x4E00, 0x9FFF);
            range_into(&mut set, 0xFF00, 0xFFEF);
            set.insert(0xFEFF);
        }
        // 大字符集：再加扩展 A、兼容汉字、注音、货币符号的完整区。
        "zhFull" => {
            range_into(&mut set, 0x0020, 0x007E);
            range_into(&mut set, 0x00A0, 0x024F);
            range_into(&mut set, 0x2000, 0x206F);
            range_into(&mut set, 0x20A0, 0x20CF);
            range_into(&mut set, 0x2100, 0x214F);
            range_into(&mut set, 0x2E80, 0x2EFF);
            range_into(&mut set, 0x3000, 0x303F);
            range_into(&mut set, 0x3040, 0x30FF);
            range_into(&mut set, 0x3100, 0x312F);
            range_into(&mut set, 0x3400, 0x4DBF);
            range_into(&mut set, 0x4E00, 0x9FFF);
            range_into(&mut set, 0xF900, 0xFAFF);
            range_into(&mut set, 0xFE30, 0xFE4F);
            range_into(&mut set, 0xFF00, 0xFFEF);
            set.insert(0xFEFF);
        }
        _ => {
            for c in custom.chars() {
                set.insert(c as u32);
            }
        }
    }
    set
}

/// 字体 cmap 里实际覆盖到的全部码位（用于「全量」导出）。
pub fn all_codepoints(data: &[u8], face_index: u32) -> Result<BTreeSet<u32>, String> {
    let face = ttf_parser::Face::parse(data, face_index).map_err(|e| format!("字体解析失败：{e}"))?;
    let mut set = BTreeSet::new();
    if let Some(cmap) = face.tables().cmap {
        for subtable in cmap.subtables {
            subtable.codepoints(|cp| {
                set.insert(cp);
            });
        }
    }
    Ok(set)
}

fn loca_offsets(loca: &[u8], num_glyphs: u16, long: bool) -> Vec<u32> {
    let n = num_glyphs as usize + 1;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(if long {
            u32at(loca, i * 4)
        } else {
            u16at(loca, i * 2) as u32 * 2
        });
    }
    out
}

/// 复合字形里引用到的字形 id；普通字形什么也不做。
fn component_gids(glyf: &[u8], start: usize, end: usize, out: &mut BTreeSet<u16>) {
    if end <= start + 10 || i16at(glyf, start) >= 0 {
        return;
    }
    let mut pos = start + 10;
    loop {
        if pos + 4 > end {
            break;
        }
        let flags = u16at(glyf, pos);
        out.insert(u16at(glyf, pos + 2));
        let mut step = 4 + if flags & 0x0001 != 0 { 4 } else { 2 };
        if flags & 0x0008 != 0 {
            step += 2;
        } else if flags & 0x0040 != 0 {
            step += 4;
        } else if flags & 0x0080 != 0 {
            step += 8;
        }
        pos += step;
        if flags & 0x0020 == 0 {
            break;
        }
    }
}

/// 把复合字形里的组件序号换成新序号（找不到的当作 .notdef）。
fn remap_components(glyph: &mut [u8], map: &BTreeMap<u16, u16>) {
    if glyph.len() < 12 || i16at(glyph, 0) >= 0 {
        return;
    }
    let mut pos = 10;
    loop {
        if pos + 4 > glyph.len() {
            break;
        }
        let flags = u16at(glyph, pos);
        let old = u16at(glyph, pos + 2);
        put_u16(glyph, pos + 2, map.get(&old).copied().unwrap_or(0));
        let mut step = 4 + if flags & 0x0001 != 0 { 4 } else { 2 };
        if flags & 0x0008 != 0 {
            step += 2;
        } else if flags & 0x0040 != 0 {
            step += 4;
        } else if flags & 0x0080 != 0 {
            step += 8;
        }
        pos += step;
        if flags & 0x0020 == 0 {
            break;
        }
    }
}

/// 连续段：(起始码位, 结束码位, 起始字形)。
fn glyph_runs(pairs: &[(u32, u16)]) -> Vec<(u32, u32, u16)> {
    let mut runs: Vec<(u32, u32, u16)> = Vec::new();
    for &(cp, gid) in pairs {
        match runs.last_mut() {
            Some(last)
                if cp == last.1 + 1
                    && gid == last.2.wrapping_add(((last.1 - last.0) as u16).wrapping_add(1)) =>
            {
                last.1 = cp;
            }
            _ => runs.push((cp, cp, gid)),
        }
    }
    runs
}

/// cmap format 4：只在 BMP 内、且段数放得下时才产出。
fn format4(pairs: &[(u32, u16)]) -> Option<Vec<u8>> {
    let bmp: Vec<(u32, u16)> = pairs.iter().copied().filter(|&(cp, _)| cp <= 0xFFFF).collect();
    if bmp.is_empty() {
        return None;
    }
    // 段的条件是「码位连续、字形也连续」，这样每一段可以用一个 idDelta 表示。
    let mut segs: Vec<(u16, u16, u16)> = Vec::new();
    for &(cp, gid) in &bmp {
        let cp = cp as u16;
        let delta = gid.wrapping_sub(cp);
        match segs.last_mut() {
            Some(last) if cp == last.1.wrapping_add(1) && delta == last.2 => last.1 = cp,
            _ => segs.push((cp, cp, delta)),
        }
    }
    if segs.last().map(|s| s.1) != Some(0xFFFF) {
        segs.push((0xFFFF, 0xFFFF, 1));
    }
    let seg_count = segs.len();
    let length = 16 + seg_count * 8;
    if length > 0xFFFF {
        return None;
    }

    let mut entry_selector = 0u16;
    while (1u16 << (entry_selector + 1)) <= seg_count as u16 {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector) * 2;
    let range_shift = (seg_count as u16) * 2 - search_range;

    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(&4u16.to_be_bytes());
    out.extend_from_slice(&(length as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // language
    out.extend_from_slice(&((seg_count * 2) as u16).to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    for s in &segs {
        out.extend_from_slice(&s.1.to_be_bytes());
    }
    out.extend_from_slice(&0u16.to_be_bytes()); // reservedPad
    for s in &segs {
        out.extend_from_slice(&s.0.to_be_bytes());
    }
    for s in &segs {
        out.extend_from_slice(&s.2.to_be_bytes());
    }
    for _ in &segs {
        out.extend_from_slice(&0u16.to_be_bytes()); // idRangeOffset
    }
    Some(out)
}

/// cmap format 12：全平面，覆盖 format 4 装不下的情况。
fn format12(pairs: &[(u32, u16)]) -> Vec<u8> {
    let runs = glyph_runs(pairs);
    let length = 16 + runs.len() * 12;
    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(&12u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(length as u32).to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes()); // language
    out.extend_from_slice(&(runs.len() as u32).to_be_bytes());
    for (start, end, gid) in runs {
        out.extend_from_slice(&start.to_be_bytes());
        out.extend_from_slice(&end.to_be_bytes());
        out.extend_from_slice(&(gid as u32).to_be_bytes());
    }
    out
}

fn build_cmap(pairs: &[(u32, u16)]) -> Vec<u8> {
    let mut subtables: Vec<(u16, u16, Vec<u8>)> = Vec::new();
    if let Some(f4) = format4(pairs) {
        subtables.push((3, 1, f4));
    }
    let has_non_bmp = pairs.iter().any(|&(cp, _)| cp > 0xFFFF);
    // 没有 format 4 时也必须有 format 12，否则 BMP 字符会一个都查不到。
    if has_non_bmp || subtables.is_empty() {
        subtables.push((3, 10, format12(pairs)));
    }

    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(subtables.len() as u16).to_be_bytes());
    let mut offset = 4 + subtables.len() * 8;
    for (platform, encoding, data) in &subtables {
        out.extend_from_slice(&platform.to_be_bytes());
        out.extend_from_slice(&encoding.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        offset += data.len();
    }
    for (_, _, data) in &subtables {
        out.extend_from_slice(data);
    }
    out
}

fn build_hmtx(hmtx: &[u8], num_hmetrics: u16, gids: &[u16]) -> (Vec<u8>, u16) {
    let mut out = Vec::with_capacity(gids.len() * 4);
    let last_advance = if num_hmetrics == 0 {
        0
    } else {
        u16at(hmtx, (num_hmetrics as usize - 1) * 4)
    };
    for &gid in gids {
        let gid = gid as usize;
        let (advance, lsb) = if num_hmetrics == 0 {
            (0, 0)
        } else if gid < num_hmetrics as usize {
            (u16at(hmtx, gid * 4), i16at(hmtx, gid * 4 + 2))
        } else {
            let tail = num_hmetrics as usize * 4 + (gid - num_hmetrics as usize) * 2;
            (last_advance, i16at(hmtx, tail))
        };
        out.extend_from_slice(&advance.to_be_bytes());
        out.extend_from_slice(&lsb.to_be_bytes());
    }
    (out, gids.len() as u16)
}

/// `post` 统一降到 3.0：只要四个基本字段，没有字形名，序号怎么排都无所谓。
fn flatten_post(post: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    if u32at(post, 0) == 0x0001_0000 {
        // 1.0 是 258 个固定标准名，子集后顺序变了同样不能保留。
        out.extend_from_slice(&1u32.to_be_bytes());
    } else {
        out.extend_from_slice(&0x0003_0000u32.to_be_bytes());
    }
    let italic = if post.len() >= 16 { &post[4..16] } else { &[0u8; 12] };
    out.extend_from_slice(italic);
    out.extend_from_slice(&[0u8; 16]);
    out
}

/// 对第 `face_index` 个 face 做子集化，返回新的 sfnt 字节。
pub fn subset(data: &[u8], face_index: u32, keep: &BTreeSet<u32>) -> Result<Vec<u8>, String> {
    let container = read_face(data, face_index)?;
    let find = |tag: &[u8; 4]| {
        container
            .tables
            .iter()
            .find(|t| &t.tag == tag)
            .map(|t| t.data.as_slice())
    };

    if find(b"CFF ").is_some() || find(b"CFF2").is_some() {
        return Err("这是 CFF 轮廓（OTF）字体，暂不支持子集化；可以改用「导出字体文件」。".into());
    }
    let glyf = find(b"glyf").ok_or("这个字体没有 glyf 表，无法子集化")?;
    let loca = find(b"loca").ok_or("这个字体没有 loca 表，无法子集化")?;
    let head = find(b"head").ok_or("这个字体没有 head 表")?;
    let maxp = find(b"maxp").ok_or("这个字体没有 maxp 表")?;
    let hhea = find(b"hhea").ok_or("这个字体没有 hhea 表")?;
    let hmtx = find(b"hmtx").ok_or("这个字体没有 hmtx 表")?;

    let num_glyphs = u16at(maxp, 4);
    let long_loca = i16at(head, 50) != 0;
    let offsets = loca_offsets(loca, num_glyphs, long_loca);
    let face = ttf_parser::Face::parse(data, face_index).map_err(|e| format!("字体解析失败：{e}"))?;

    // 1) 字符 → 字形，再补上复合字形引用到的组件（组件还可能嵌套，迭代到不再增长）。
    let mut wanted: BTreeSet<u16> = BTreeSet::new();
    wanted.insert(0);
    let mut pairs: Vec<(u32, u16)> = Vec::new();
    for &cp in keep {
        let Some(ch) = char::from_u32(cp) else { continue };
        if let Some(gid) = face.glyph_index(ch) {
            if gid.0 < num_glyphs {
                wanted.insert(gid.0);
                pairs.push((cp, gid.0));
            }
        }
    }
    if pairs.is_empty() {
        return Err("所选字符集在这个字体里一个都覆盖不到".into());
    }
    let mut frontier: Vec<u16> = wanted.iter().copied().collect();
    while !frontier.is_empty() {
        let mut extra = BTreeSet::new();
        for gid in frontier {
            let (s, e) = match (offsets.get(gid as usize), offsets.get(gid as usize + 1)) {
                (Some(&s), Some(&e)) => (s as usize, e as usize),
                _ => continue,
            };
            if e > glyf.len() || s >= e {
                continue;
            }
            component_gids(glyf, s, e, &mut extra);
        }
        extra.retain(|g| *g < num_glyphs && !wanted.contains(g));
        frontier = extra.iter().copied().collect();
        wanted.extend(extra);
    }

    // 2) 旧序号 → 新序号（升序重排，字母序无关）。
    let gids: Vec<u16> = wanted.into_iter().collect();
    let map: BTreeMap<u16, u16> = gids
        .iter()
        .enumerate()
        .map(|(new, &old)| (old, new as u16))
        .collect();

    // 3) 重写 glyf / loca。
    let mut new_glyf: Vec<u8> = Vec::new();
    let mut new_loca: Vec<u32> = Vec::with_capacity(gids.len() + 1);
    for &gid in &gids {
        new_loca.push(new_glyf.len() as u32);
        let (s, e) = match (offsets.get(gid as usize), offsets.get(gid as usize + 1)) {
            (Some(&s), Some(&e)) => (s as usize, e as usize),
            _ => continue,
        };
        if e <= s || e > glyf.len() {
            continue;
        }
        let mut glyph = glyf[s..e].to_vec();
        remap_components(&mut glyph, &map);
        new_glyf.extend_from_slice(&glyph);
        // 统一 4 字节对齐：短格式（偏移/2）与长格式都能用同一份偏移表。
        while new_glyf.len() % 4 != 0 {
            new_glyf.push(0);
        }
    }
    new_loca.push(new_glyf.len() as u32);

    let short_loca = *new_loca.last().unwrap() / 2 <= 0xFFFF;
    let new_loca_bytes: Vec<u8> = if short_loca {
        new_loca
            .iter()
            .flat_map(|&o| ((o / 2) as u16).to_be_bytes())
            .collect()
    } else {
        new_loca.iter().flat_map(|&o| o.to_be_bytes()).collect()
    };

    // 4) 度量、maxp、head、cmap、post、OS/2。
    let (new_hmtx, num_hmetrics) = build_hmtx(hmtx, u16at(hhea, 34), &gids);

    let mut new_head = head.to_vec();
    put_u16(&mut new_head, 50, if short_loca { 0 } else { 1 });
    new_head.resize(new_head.len().max(54), 0);

    let mut new_maxp = maxp.to_vec();
    put_u16(&mut new_maxp, 4, gids.len() as u16);

    let mut new_hhea = hhea.to_vec();
    put_u16(&mut new_hhea, 34, num_hmetrics);

    let mut sorted_pairs = pairs.clone();
    sorted_pairs.sort_unstable_by_key(|p| p.0);
    sorted_pairs.dedup_by_key(|p| p.0);
    let new_cmap = build_cmap(&sorted_pairs);

    let replace: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"glyf", new_glyf),
        (b"loca", new_loca_bytes),
        (b"hmtx", new_hmtx),
        (b"head", new_head),
        (b"maxp", new_maxp),
        (b"hhea", new_hhea),
        (b"cmap", new_cmap),
    ];

    let mut tables: Vec<SfntTable> = Vec::with_capacity(container.tables.len());
    for table in &container.tables {
        if is_dropped(&table.tag) {
            continue;
        }
        if &table.tag == b"post" {
            tables.push(SfntTable {
                tag: *b"post",
                data: flatten_post(&table.data),
            });
            continue;
        }
        match replace.iter().find(|(tag, _)| *tag == &table.tag) {
            Some((_, data)) => tables.push(SfntTable {
                tag: table.tag,
                data: data.clone(),
            }),
            None => tables.push(SfntTable {
                tag: table.tag,
                data: table.data.clone(),
            }),
        }
    }
    // 原来的表目录里若缺这些（例如没有 post/OS2），也要补上重建结果。
    for (tag, data) in &replace {
        if !tables.iter().any(|t| &t.tag == *tag) {
            tables.push(SfntTable {
                tag: **tag,
                data: data.clone(),
            });
        }
    }

    // usFirstCharIndex / usLastCharIndex 跟着新字符集走，否则系统会以为字体覆盖很广。
    if let Some(os2) = tables.iter_mut().find(|t| &t.tag == b"OS/2") {
        if os2.data.len() >= 68 {
            let first = sorted_pairs.first().map(|p| p.0).unwrap_or(0x20);
            let last = sorted_pairs.last().map(|p| p.0).unwrap_or(0x20);
            put_u16(&mut os2.data, 64, first.min(0xFFFF) as u16);
            put_u16(&mut os2.data, 66, if last > 0xFFFF { 0xFFFF } else { last as u16 });
        }
    }

    Ok(build_sfnt(container.version, &tables))
}

// --- 网页字体封装 -----------------------------------------------------------

fn pad4(n: usize) -> usize {
    (n + 3) & !3
}

fn sorted<'a>(tables: &'a [SfntTable]) -> Vec<&'a SfntTable> {
    let mut list: Vec<&SfntTable> = tables.iter().collect();
    list.sort_by(|a, b| a.tag.cmp(&b.tag));
    list
}

fn sfnt_size(list: &[&SfntTable]) -> u32 {
    12 + 16 * list.len() as u32 + list.iter().map(|t| pad4(t.data.len()) as u32).sum::<u32>()
}

fn table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for chunk in data.chunks(4) {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}

/// WOFF 1.0：每张表单独 zlib 压缩。
pub fn to_woff(version: u32, tables: &[SfntTable]) -> Result<Vec<u8>, String> {
    let list = sorted(tables);
    let count = list.len();
    let mut records: Vec<([u8; 4], u32, u32, u32, u32)> = Vec::with_capacity(count);
    let mut body: Vec<u8> = Vec::new();

    for table in &list {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(9));
        encoder
            .write_all(&table.data)
            .map_err(|e| format!("压缩失败：{e}"))?;
        let packed = encoder.finish().map_err(|e| format!("压缩失败：{e}"))?;
        let (payload, stored_len) = if packed.len() < table.data.len() {
            (packed, None)
        } else {
            (table.data.clone(), Some(table.data.len()))
        };
        let offset = (44 + count * 20 + body.len()) as u32;
        records.push((
            table.tag,
            table_checksum(&table.data),
            offset,
            stored_len.unwrap_or(payload.len()) as u32,
            table.data.len() as u32,
        ));
        body.extend_from_slice(&payload);
        while body.len() % 4 != 0 {
            body.push(0);
        }
    }

    let total = 44 + count * 20 + body.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"wOFF");
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&(total as u32).to_be_bytes());
    out.extend_from_slice(&(count as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(&sfnt_size(&list).to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes()); // majorVersion
    out.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
    for _ in 0..5 {
        out.extend_from_slice(&0u32.to_be_bytes()); // meta*/priv* 全留空
    }
    for (tag, checksum, offset, comp_len, orig_len) in &records {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum.to_be_bytes());
        out.extend_from_slice(&offset.to_be_bytes());
        out.extend_from_slice(&comp_len.to_be_bytes());
        out.extend_from_slice(&orig_len.to_be_bytes());
    }
    out.extend_from_slice(&body);
    Ok(out)
}

/// WOFF2 的已知表标签表，索引就是目录里的 6 bit tagIndex（63 = 自填标签）。
const KNOWN_TAGS: [&[u8; 4]; 63] = [
    b"cmap", b"head", b"hhea", b"hmtx", b"maxp", b"name", b"OS/2", b"post", b"cvt ", b"fpgm",
    b"glyf", b"loca", b"prep", b"CFF ", b"VORG", b"EBDT", b"EBLC", b"gasp", b"hdmx", b"kern",
    b"LTSH", b"PCLT", b"VDMX", b"vhea", b"vmtx", b"BASE", b"GDEF", b"GPOS", b"GSUB", b"EBSC",
    b"JSTF", b"MATH", b"CBDT", b"CBLC", b"COLR", b"CPAL", b"SVG ", b"sbix", b"acnt", b"avar",
    b"bdat", b"bloc", b"bsln", b"cvar", b"fdsc", b"feat", b"fmtx", b"fvar", b"gvar", b"hsty",
    b"just", b"lcar", b"mort", b"morx", b"opbd", b"prop", b"trak", b"Zapf", b"Silf", b"Glat",
    b"Gloc", b"Feat", b"Sill",
];

fn tag_index(tag: &[u8; 4]) -> Option<u8> {
    KNOWN_TAGS.iter().position(|t| *t == tag).map(|i| i as u8)
}

/// WOFF2 的 UIntBase128 变长整数。
fn push_base128(out: &mut Vec<u8>, value: u32) {
    let mut buf = [0u8; 5];
    let mut i = 4;
    let mut v = value;
    buf[4] = (v & 0x7F) as u8;
    v >>= 7;
    while v > 0 {
        i -= 1;
        buf[i] = ((v & 0x7F) as u8) | 0x80;
        v >>= 7;
    }
    out.extend_from_slice(&buf[i..]);
}

/// WOFF2：所有表拼成一条流做 brotli；`glyf`/`loca` 用 null transform（version=3）。
pub fn to_woff2(version: u32, tables: &[SfntTable]) -> Result<Vec<u8>, String> {
    to_woff2_q(version, tables, 11)
}

/// 同 `to_woff2`，但可以指定 brotli 质量。整份 CJK 字体在质量 11 上要压很久，
/// 打包项目字体时降一档会更实用。
pub fn to_woff2_q(version: u32, tables: &[SfntTable], quality: u32) -> Result<Vec<u8>, String> {
    let list = sorted(tables);
    let count = list.len();

    let mut directory: Vec<u8> = Vec::with_capacity(count * 6);
    let mut stream: Vec<u8> = Vec::new();
    for table in &list {
        let index = tag_index(&table.tag);
        // glyf / loca：0 = 重编码，3 = 原样；其余表只有 0（原样）。
        let transform: u8 = if &table.tag == b"glyf" || &table.tag == b"loca" {
            3
        } else {
            0
        };
        directory.push(index.unwrap_or(63) | (transform << 6));
        if index.is_none() {
            directory.extend_from_slice(&table.tag);
        }
        push_base128(&mut directory, table.data.len() as u32);
        stream.extend_from_slice(&table.data);
    }

    let mut packed: Vec<u8> = Vec::new();
    {
        let mut writer = brotli::CompressorWriter::new(&mut packed, 4096, quality, 22);
        writer
            .write_all(&stream)
            .map_err(|e| format!("brotli 压缩失败：{e}"))?;
        writer.flush().map_err(|e| format!("brotli 压缩失败：{e}"))?;
    }

    let total = 48 + directory.len() + packed.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"wOF2");
    out.extend_from_slice(&version.to_be_bytes());
    out.extend_from_slice(&(total as u32).to_be_bytes());
    out.extend_from_slice(&(count as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(&sfnt_size(&list).to_be_bytes());
    out.extend_from_slice(&(packed.len() as u32).to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes()); // majorVersion
    out.extend_from_slice(&0u16.to_be_bytes()); // minorVersion
    for _ in 0..5 {
        out.extend_from_slice(&0u32.to_be_bytes()); // meta*/priv* 全留空
    }
    out.extend_from_slice(&directory);
    out.extend_from_slice(&packed);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn system_font(name: &str) -> Option<Vec<u8>> {
        let path = format!("C:/Windows/Fonts/{name}");
        Path::new(&path).exists().then(|| std::fs::read(&path).unwrap())
    }

    #[test]
    fn subset_keeps_only_the_requested_characters() {
        let Some(data) = system_font("msyh.ttc") else { return };
        let keep = charset_preset("latin", "");
        let bytes = subset(&data, 0, &keep).unwrap();
        assert!(bytes.len() < data.len(), "子集应该更小");
        let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
        assert!(face.glyph_index('A').is_some(), "拉丁字母要保留");
        assert!(face.glyph_index('中').is_none(), "没要求收汉字，不该留下");
        assert!(face.number_of_glyphs() > 10);
    }

    #[test]
    fn subset_keeps_han_when_asked() {
        let Some(data) = system_font("simfang.ttf") else { return };
        let keep = charset_preset("zh", "");
        let bytes = subset(&data, 0, &keep).unwrap();
        let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
        for ch in ['中', '国', '字', 'A', '，', '。'] {
            assert!(face.glyph_index(ch).is_some(), "缺字：{ch}");
        }
        // 复合字形（如「鑫」这类由多个部件拼的）也要能查到。
        let gid = face.glyph_index('鑫').expect("汉字应保留");
        assert!(gid.0 > 0);
        // 轮廓能被真正解析出来，说明 glyf/loca 的偏移表是对的。
        let mut outline_ok = 0;
        for ch in ['中', '国', '永'] {
            if let Some(gid) = face.glyph_index(ch) {
                let mut builder = CountingOutline::default();
                if face.outline_glyph(gid, &mut builder).is_some() {
                    outline_ok += 1;
                }
            }
        }
        assert!(outline_ok >= 2, "轮廓解析失败，loca/glyf 可能写坏了");
    }

    #[test]
    fn custom_charset_uses_the_given_text() {
        let set = charset_preset("text", "你好ABC");
        assert!(set.contains(&('你' as u32)));
        assert!(set.contains(&('C' as u32)));
        assert!(!set.contains(&('好' as u32 + 100)));
    }

    #[test]
    fn woff_wraps_every_table_losslessly() {
        let Some(data) = system_font("simfang.ttf") else { return };
        let bytes = subset(&data, 0, &charset_preset("latin", "")).unwrap();
        let container = read_face(&bytes, 0).unwrap();
        let woff = to_woff(container.version, &container.tables).unwrap();
        assert_eq!(&woff[0..4], b"wOFF");
        assert_eq!(u32at(&woff, 8) as usize, woff.len(), "length 字段要对得上总长");
        assert_eq!(u16at(&woff, 12) as usize, container.tables.len());

        // 逐个表解压回原文，确认压缩没丢东西。
        let count = u16at(&woff, 12) as usize;
        let mut ok = 0;
        for i in 0..count {
            let rec = 44 + i * 20;
            let mut tag = [0u8; 4];
            tag.copy_from_slice(&woff[rec..rec + 4]);
            let offset = u32at(&woff, rec + 8) as usize;
            let comp_len = u32at(&woff, rec + 12) as usize;
            let orig_len = u32at(&woff, rec + 16) as usize;
            let slice = &woff[offset..offset + comp_len];
            let mut out = Vec::new();
            if comp_len < orig_len {
                let mut decoder = flate2::read::ZlibDecoder::new(slice);
                std::io::Read::read_to_end(&mut decoder, &mut out).unwrap();
            } else {
                out.extend_from_slice(slice);
            }
            assert_eq!(out.len(), orig_len, "{} 解压后长度不符", String::from_utf8_lossy(&tag));
            let original = container.tables.iter().find(|t| t.tag == tag).unwrap();
            assert_eq!(out, original.data, "{} 内容不符", String::from_utf8_lossy(&tag));
            ok += 1;
        }
        assert_eq!(ok, count);
    }

    #[test]
    fn woff2_header_and_stream_are_consistent() {
        let Some(data) = system_font("simfang.ttf") else { return };
        let bytes = subset(&data, 0, &charset_preset("latin", "")).unwrap();
        let container = read_face(&bytes, 0).unwrap();
        let woff2 = to_woff2(container.version, &container.tables).unwrap();
        assert_eq!(&woff2[0..4], b"wOF2");
        assert_eq!(u32at(&woff2, 8) as usize, woff2.len());
        let count = u16at(&woff2, 12) as usize;
        assert_eq!(count, container.tables.len());
        let compressed = u32at(&woff2, 20) as usize;
        assert!(48 + compressed <= woff2.len());

        // 目录里每条记录的 origLength 之和应该等于总表长，且 brotli 流能解回这么长。
        let mut pos = 48usize;
        let mut total = 0usize;
        for _ in 0..count {
            let flags = woff2[pos];
            pos += 1;
            let index = (flags & 0x3F) as usize;
            if index == 63 {
                pos += 4;
            }
            let len = read_base128(&woff2, &mut pos) as usize;
            total += len;
        }
        let mut decoded = Vec::new();
        let mut reader = brotli::Decompressor::new(&woff2[pos..], 4096);
        std::io::Read::read_to_end(&mut reader, &mut decoded).unwrap();
        assert_eq!(decoded.len(), total, "brotli 流长度应与目录声明的总长一致");
        // 解出来的第一张表（tag 升序里最靠前的）内容要和原表字节一致。
        let first = sorted(&container.tables)[0];
        assert_eq!(&decoded[..first.data.len()], first.data.as_slice());
    }

    fn read_base128(data: &[u8], pos: &mut usize) -> u32 {
        let mut value = 0u32;
        loop {
            let byte = data[*pos];
            *pos += 1;
            value = (value << 7) | (byte & 0x7F) as u32;
            if byte & 0x80 == 0 {
                return value;
            }
        }
    }

    /// 用 Windows 自己的字体引擎（GDI）加载重建出来的文件。
    /// 这比 ttf-parser 更能说明问题：GDI 才是 Word / Photoshop 实际用的那条路。
    #[cfg(windows)]
    #[test]
    fn windows_gdi_accepts_the_rebuilt_font() {
        use windows_sys::Win32::Graphics::Gdi::{AddFontResourceExW, RemoveFontResourceExW};

        const FR_PRIVATE: u32 = 0x10;
        let Some(data) = system_font("simfang.ttf") else { return };
        let bytes = subset(&data, 0, &charset_preset("zh", "")).unwrap();

        let dest = std::env::temp_dir().join(format!("zfm-subset-{}.ttf", std::process::id()));
        std::fs::write(&dest, &bytes).unwrap();
        let wide: Vec<u16> = dest
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // FR_PRIVATE：只给本进程用，不进系统字体表，进程退出自动撤销。
        let added = unsafe { AddFontResourceExW(wide.as_ptr(), FR_PRIVATE, std::ptr::null()) };
        if added > 0 {
            unsafe { RemoveFontResourceExW(wide.as_ptr(), FR_PRIVATE, std::ptr::null()) };
        }
        let _ = std::fs::remove_file(&dest);
        assert!(added > 0, "GDI 拒绝了重建的子集字体，说明表结构有问题");
    }

    #[derive(Default)]
    struct CountingOutline {
        points: usize,
    }

    impl ttf_parser::OutlineBuilder for CountingOutline {
        fn move_to(&mut self, _x: f32, _y: f32) {
            self.points += 1;
        }
        fn line_to(&mut self, _x: f32, _y: f32) {
            self.points += 1;
        }
        fn quad_to(&mut self, _x1: f32, _y1: f32, _x: f32, _y: f32) {
            self.points += 2;
        }
        fn curve_to(&mut self, _x1: f32, _y1: f32, _x2: f32, _y2: f32, _x: f32, _y: f32) {
            self.points += 3;
        }
        fn close(&mut self) {}
    }
}
