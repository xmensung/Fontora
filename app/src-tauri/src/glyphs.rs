//! 缺字检测：拿一段文字去问字体「这些字你都有吗」。
//!
//! 为什么不用 `get_charset`：那里为了渲染字符地图把码点截断到 4096 个，
//! 中文字库动辄两三万码点，拿截断后的列表判缺字会大面积误报。
//! 这里改成逐字 `face.glyph_index()`，并且把 `GlyphId(0)`（.notdef 空盒子）
//! 也算作缺 —— 否则「有 cmap 条目但指向空字形」的残字体会被判成有字。

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlyphReport {
    pub path: String,
    pub face_index: u32,
    /// 这段文字里该字面缺的码点（升序、去重）
    pub missing: Vec<u32>,
    /// 实际参与判定的字符数（去掉空白与控制符后）
    pub checked: usize,
}

/// key = `path#faceIndex`，value = 码点 -> 有没有字形。
/// 只缓存被问过的码点，所以内存跟「预览文本长度 × 字体数」成正比，不会爆。
static CACHE: Mutex<Option<HashMap<String, HashMap<u32, bool>>>> = Mutex::new(None);

fn face_key(path: &str, index: u32) -> String {
    format!("{path}#{index}")
}

fn dedup_codepoints(text: &str) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for ch in text.chars() {
        if ch.is_whitespace() || ch.is_control() {
            continue;
        }
        // 变体选择符（VS1~VS16、VS17~VS256）本身不占字形，不参与缺字判定。
        let cp = ch as u32;
        if (0xFE00..=0xFE0F).contains(&cp) || (0xE0100..=0xE01EF).contains(&cp) {
            continue;
        }
        out.push(cp);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// 读一个字体文件，把这批码点的有无一次性问清楚。
fn resolve(path: &str, face_index: u32, cps: &[u32]) -> HashMap<u32, bool> {
    let mut out = HashMap::with_capacity(cps.len());
    let Ok(data) = std::fs::read(path) else {
        // 文件读不到（被移走 / 权限），全部按缺处理，至少不会假装有字。
        for cp in cps {
            out.insert(*cp, false);
        }
        return out;
    };
    let Ok(face) = ttf_parser::Face::parse(&data, face_index) else {
        for cp in cps {
            out.insert(*cp, false);
        }
        return out;
    };
    for cp in cps {
        let has = char::from_u32(*cp)
            .and_then(|ch| face.glyph_index(ch))
            .map(|g| g.0 != 0)
            .unwrap_or(false);
        out.insert(*cp, has);
    }
    out
}

pub fn check_faces(faces: &[(String, u32)], text: &str) -> Vec<GlyphReport> {
    let cps = dedup_codepoints(text);
    let total = cps.len();
    if total == 0 || faces.is_empty() {
        return faces
            .iter()
            .map(|(path, index)| GlyphReport {
                path: path.clone(),
                face_index: *index,
                missing: Vec::new(),
                checked: 0,
            })
            .collect();
    }

    // 1. 先看缓存，把还缺的码点攒出来。
    let mut pending: Vec<(usize, Vec<u32>)> = Vec::new();
    {
        let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(HashMap::new);
        for (i, (path, index)) in faces.iter().enumerate() {
            let key = face_key(path, *index);
            let known = map.get(&key);
            let todo: Vec<u32> = match known {
                None => cps.clone(),
                Some(known) => cps
                    .iter()
                    .copied()
                    .filter(|cp| !known.contains_key(cp))
                    .collect(),
            };
            if !todo.is_empty() {
                pending.push((i, todo));
            }
        }
    }

    // 2. 缺的并行读盘；每个字体文件只读一次。
    if !pending.is_empty() {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8)
            .max(1);
        let chunk = pending.len().div_ceil(workers).max(1);
        let resolved: Vec<(usize, HashMap<u32, bool>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = pending
                .chunks(chunk)
                .map(|group| {
                    scope.spawn(move || {
                        group
                            .iter()
                            .map(|(i, cps)| {
                                let (path, index) = &faces[*i];
                                (*i, resolve(path, *index, cps))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        });
        if let Ok(mut guard) = CACHE.lock() {
            let map = guard.get_or_insert_with(HashMap::new);
            for (i, found) in resolved {
                let (path, index) = &faces[i];
                map.entry(face_key(path, *index)).or_default().extend(found);
            }
        }
    }

    // 3. 汇总。
    let guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.as_ref();
    faces
        .iter()
        .map(|(path, index)| {
            let known = map.and_then(|m| m.get(&face_key(path, *index)));
            let mut missing: Vec<u32> = cps
                .iter()
                .copied()
                .filter(|cp| !known.and_then(|k| k.get(cp)).copied().unwrap_or(false))
                .collect();
            missing.sort_unstable();
            missing.dedup();
            GlyphReport {
                path: path.clone(),
                face_index: *index,
                missing,
                checked: total,
            }
        })
        .collect()
}

/// 换字体库 / 重扫之后，缓存里的路径可能已经失效，直接清空最省事。
pub fn clear_cache() {
    if let Ok(mut guard) = CACHE.lock() {
        if let Some(map) = guard.as_mut() {
            map.clear();
        }
    }
}
