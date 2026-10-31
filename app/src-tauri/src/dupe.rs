//! 重复字体识别（原计划的 P4）。
//!
//! 两类重复，判据完全不同，所以分开报：
//!
//! 1. `identical` —— SHA-256 相同，字节级完全一样。删掉任何一份都不损失信息。
//! 2. `sameName`  —— 家族名 + 字重相同但内容不同（不同版本／不同厂商实现）。
//!    这类**不能**默认删，只是提示用户「你可能有两份同名字体」。
//!
//! 系统字体（`C:\Windows\Fonts`）一律标记为不可删：程序只会提示，不会动它们。
//! 真正删除走 `installer::uninstall`（注销注册表 + 移入应用回收站 + 广播
//! `WM_FONTCHANGE`），因此误删可以在「回收站」里还原。

use crate::db::{FontCache, IndexedFile};
use crate::font_types::FontSource;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DupeFile {
    pub path: String,
    pub file_name: String,
    pub family: String,
    pub style: String,
    pub size: u64,
    /// Unix 秒；0 表示读不到时间戳。
    pub modified: i64,
    /// system / user / managed
    pub source: String,
    pub system: bool,
    /// 能不能删：系统字体一律 false。
    pub deletable: bool,
    /// 本组建议保留的那一份。
    pub kept: bool,
    /// 同组里其它文件的 hash 与本文件相同（identical 组恒为 true）。
    pub identical: bool,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DupeGroup {
    pub id: String,
    /// identical | sameName
    pub kind: String,
    /// 展示用标题：家族名 · 字重
    pub label: String,
    /// 该组的大小（字节），用于「共占 xx MB」的提示。
    pub total_size: u64,
    /// 删掉「除保留项以外的可删项」能回收的字节。
    pub reclaimable: u64,
    pub files: Vec<DupeFile>,
}

pub fn find(cache: &FontCache) -> Vec<DupeGroup> {
    // 只有「大小撞车」的文件才会被算哈希，一般是个位数，这里同步补齐即可。
    let _ = cache.hash_pending(512);
    let files: Vec<IndexedFile> = cache
        .all_files()
        .into_iter()
        .filter(|f| Path::new(&f.path).exists())
        .collect();

    let mut groups = identical_groups(&files);
    groups.extend(same_name_groups(&files, &groups));

    // identical 优先（真的能清理），可回收空间大的排前面。
    groups.sort_by(|a, b| {
        let rank = |g: &DupeGroup| u8::from(g.kind != "identical");
        rank(a)
            .cmp(&rank(b))
            .then(b.reclaimable.cmp(&a.reclaimable))
            .then(a.label.cmp(&b.label))
    });
    groups
}

fn identical_groups(files: &[IndexedFile]) -> Vec<DupeGroup> {
    let mut by_hash: HashMap<&str, Vec<&IndexedFile>> = HashMap::new();
    for f in files {
        if let Some(hash) = f.hash.as_deref().filter(|h| !h.is_empty()) {
            by_hash.entry(hash).or_default().push(f);
        }
    }

    let mut out = Vec::new();
    for (hash, mut list) in by_hash {
        if list.len() < 2 {
            continue;
        }
        // 保留顺序：系统字体（删不掉）优先，其次修改时间最新的。
        list.sort_by_key(|f| (u8::from(!is_system(f)), -(f.mtime as i64), f.path.len()));
        let keep = list[0].path.clone();
        let files: Vec<DupeFile> = list.iter().map(|f| dupe_file(f, &keep, true)).collect();
        let total_size = files.iter().map(|f| f.size).sum();
        let reclaimable = files
            .iter()
            .filter(|f| f.deletable && !f.kept)
            .map(|f| f.size)
            .sum();
        out.push(DupeGroup {
            id: format!("hash:{hash}"),
            kind: "identical".to_string(),
            label: files
                .first()
                .map(|f| {
                    if f.style.is_empty() {
                        f.family.clone()
                    } else {
                        format!("{} · {}", f.family, f.style)
                    }
                })
                .unwrap_or_default(),
            total_size,
            reclaimable,
            files,
        });
    }
    out
}

/// 同名字体（家族名 + 字重相同、内容不同）。
///
/// 用并查集把「共享同一个 家族·字重」的文件连成一组：一个 TTC 集合里常同时有
/// 「宋体」和「新宋体」两个 face，如果按 face 直接分组，同一对文件会被报两遍，
/// 可回收空间也会被重复计算。连成一组后只报一次，标题列出它涉及的所有名字。
fn same_name_groups(files: &[IndexedFile], known: &[DupeGroup]) -> Vec<DupeGroup> {
    // 每个文件参与哪些 家族·字重
    let mut keys_of: Vec<Vec<(String, String)>> = Vec::with_capacity(files.len());
    let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        let mut own: Vec<(String, String)> = Vec::new();
        for face in &f.faces {
            let family = face.family.trim();
            if family.is_empty() {
                continue;
            }
            let style = face.style.trim().to_string();
            let key = format!("{}|{}", family.to_lowercase(), style.to_lowercase());
            if own.iter().any(|(k, _)| *k == key) {
                continue;
            }
            own.push((key.clone(), format!("{family}{}", if style.is_empty() { String::new() } else { format!(" · {style}") })));
            by_key.entry(key).or_default().push(i);
        }
        keys_of.push(own);
    }

    // 并查集：同一个 家族·字重 下的文件合并
    let mut parent: Vec<usize> = (0..files.len()).collect();
    fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for members in by_key.values() {
        for w in members.windows(2) {
            let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            if a != b {
                parent[b] = a;
            }
        }
    }

    let mut buckets: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..files.len() {
        let root = find(&mut parent, i);
        buckets.entry(root).or_default().push(i);
    }

    let already: HashSet<&str> = known
        .iter()
        .flat_map(|g| g.files.iter().map(|f| f.path.as_str()))
        .collect();

    let mut out = Vec::new();
    for (_, mut idxs) in buckets {
        if idxs.len() < 2 {
            continue;
        }
        // 字节完全相同的一对，上面 identical 已经报过，这里不重复。
        let hashes: HashSet<&str> = idxs
            .iter()
            .filter_map(|&i| files[i].hash.as_deref().filter(|h| !h.is_empty()))
            .collect();
        if hashes.len() == 1 && idxs.iter().all(|&i| already.contains(files[i].path.as_str())) {
            continue;
        }
        idxs.sort_by_key(|&i| {
            let f = &files[i];
            (u8::from(!is_system(f)), -(f.mtime as i64))
        });
        let keep = files[idxs[0]].path.clone();
        let files_out: Vec<DupeFile> = idxs
            .iter()
            .map(|&i| dupe_file(&files[i], &keep, false))
            .collect();
        let total_size = files_out.iter().map(|f| f.size).sum();
        let reclaimable = files_out
            .iter()
            .filter(|f| f.deletable && !f.kept)
            .map(|f| f.size)
            .sum();

        // 标题：把组内涉及的名字去重列出来，最多三个
        let mut names: Vec<String> = Vec::new();
        for &i in &idxs {
            for (_, label) in &keys_of[i] {
                if !names.contains(label) {
                    names.push(label.clone());
                }
            }
        }
        let mut label = names.iter().take(3).cloned().collect::<Vec<_>>().join("、");
        if names.len() > 3 {
            label.push_str(&format!(" 等 {} 个", names.len()));
        }
        out.push(DupeGroup {
            id: format!("name:{}", files[idxs[0]].path),
            kind: "sameName".to_string(),
            label,
            total_size,
            reclaimable,
            files: files_out,
        });
    }
    out
}

fn dupe_file(f: &IndexedFile, keep: &str, identical: bool) -> DupeFile {
    let system = is_system(f);
    let lead = f.faces.first();
    DupeFile {
        path: f.path.clone(),
        file_name: Path::new(&f.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.path.clone()),
        family: lead.map(|l| l.family.clone()).unwrap_or_default(),
        style: lead.map(|l| l.style.clone()).unwrap_or_default(),
        size: f.size,
        modified: f.mtime,
        source: source_label(f),
        system,
        deletable: !system,
        kept: f.path == keep,
        identical,
    }
}

fn is_system(f: &IndexedFile) -> bool {
    f.faces.first().map(|face| face.source == FontSource::System).unwrap_or(false)
}

fn source_label(f: &IndexedFile) -> String {
    match f.faces.first().map(|face| face.source) {
        Some(FontSource::System) => "system".to_string(),
        Some(FontSource::User) => "user".to_string(),
        Some(FontSource::Managed) => "managed".to_string(),
        None => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, size: u64, mtime: i64, hash: Option<&str>, source: FontSource) -> IndexedFile {
        IndexedFile {
            path: path.to_string(),
            size,
            mtime,
            hash: hash.map(|h| h.to_string()),
            faces: vec![crate::font_types::FontFace {
                id: path.to_string(),
                path: path.to_string(),
                preview_path: None,
                face_index: 0,
                family: "仿宋_GB2312".to_string(),
                style: "Regular".to_string(),
                postscript_name: None,
                foundry: None,
                license: None,
                license_url: None,
                format: crate::font_types::FontFormat::Ttf,
                is_variable: false,
                axes: Vec::new(),
                weight: 400,
                italic: false,
                monospaced: false,
                classification: crate::font_types::Classification::Script,
                scripts: Vec::new(),
                features: Vec::new(),
                glyph_count: 100,
                file_size: size,
                modified: mtime as u64,
                source,
                deactivatable: false,
                active: true,
            }],
        }
    }

    #[test]
    fn identical_files_group_and_keep_the_system_copy() {
        let files = vec![
            file("C:/Windows/Fonts/a.ttf", 100, 10, Some("abc"), FontSource::System),
            file("C:/Users/x/fonts/a_0.ttf", 100, 99, Some("abc"), FontSource::User),
        ];
        let groups = identical_groups(&files);
        assert_eq!(groups.len(), 1);
        let g = &groups[0];
        assert_eq!(g.kind, "identical");
        // 系统字体删不掉，所以保留它，清掉用户那份才是安全的。
        let kept = g.files.iter().find(|f| f.kept).unwrap();
        assert!(kept.system);
        assert_eq!(g.reclaimable, 100);
    }

    #[test]
    fn same_name_but_different_bytes_is_flagged_separately() {
        let files = vec![
            file("C:/Users/x/a.ttf", 100, 10, Some("aaa"), FontSource::User),
            file("C:/Users/x/b.ttf", 120, 20, Some("bbb"), FontSource::User),
        ];
        let id = identical_groups(&files);
        assert!(id.is_empty());
        let groups = same_name_groups(&files, &id);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, "sameName");
        // 保留最新修改的那份
        assert!(groups[0].files.iter().find(|f| f.kept).unwrap().path.ends_with("b.ttf"));
    }

    #[test]
    fn system_only_group_is_reported_without_reclaimable_bytes() {
        let files = vec![
            file("C:/Windows/Fonts/a.ttf", 100, 10, Some("abc"), FontSource::System),
            file("C:/Windows/Fonts/a_0.ttf", 100, 20, Some("abc"), FontSource::System),
        ];
        let groups = identical_groups(&files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].reclaimable, 0);
        assert!(groups[0].files.iter().all(|f| !f.deletable));
    }
}
