//! Standalone character-coverage report — no window, no Tauri runtime.
//!
//! Usage:
//!   cargo run --release --bin cjk_report -- [--format tsv|md] [dir ...]
//!
//! With no dirs it scans the two Windows font folders. Output goes to stdout,
//! so redirect it wherever you like.

#[path = "../cjk.rs"]
mod cjk;

use std::path::{Path, PathBuf};

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("C:\\Windows\\Fonts")];
    if let Some(local) = dirs::data_local_dir() {
        dirs.push(local.join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

fn is_font(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc" | "otc"))
        .unwrap_or(false)
}

struct Row {
    file: String,
    face_index: u32,
    family: String,
    stats: cjk::CjkStats,
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut format = "tsv".to_string();
    if let Some(i) = args.iter().position(|a| a == "--format") {
        if i + 1 < args.len() {
            format = args.remove(i + 1);
            args.remove(i);
        }
    }

    let dirs: Vec<PathBuf> = if args.is_empty() {
        font_dirs()
    } else {
        args.iter().map(PathBuf::from).collect()
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut scanned = 0usize;

    for dir in &dirs {
        for entry in walkdir::WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !entry.file_type().is_file() || !is_font(path) {
                continue;
            }
            scanned += 1;
            let Ok(data) = std::fs::read(path) else {
                continue;
            };
            let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
            for index in 0..count {
                let Ok(face) = ttf_parser::Face::parse(&data, index) else {
                    continue;
                };
                let family = face
                    .names()
                    .into_iter()
                    .find(|n| n.name_id == ttf_parser::name_id::TYPOGRAPHIC_FAMILY)
                    .or_else(|| {
                        face.names()
                            .into_iter()
                            .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
                    })
                    .and_then(|n| n.to_string())
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    });
                rows.push(Row {
                    file: path.to_string_lossy().into_owned(),
                    face_index: index,
                    family,
                    stats: cjk::stats_for_face(&face),
                });
            }
        }
    }

    rows.sort_by(|a, b| b.stats.han.cmp(&a.stats.han).then(a.family.cmp(&b.family)));

    eprintln!(
        "scanned {} files -> {} faces from {} dir(s)",
        scanned,
        rows.len(),
        dirs.len()
    );

    if format == "md" {
        println!("| 字体家族 | 文件名 | 汉字数 | 基本区 | 扩展A | 扩展B | 总字符 | 假名 | 谚文 |");
        println!("|---|---|---:|---:|---:|---:|---:|---:|---:|");
        for r in &rows {
            let name = Path::new(&r.file)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let suffix = if r.face_index > 0 {
                format!(" (#{})", r.face_index)
            } else {
                String::new()
            };
            println!(
                "| {} | {}{} | {} | {} | {} | {} | {} | {} | {} |",
                r.family,
                name,
                suffix,
                r.stats.han,
                r.stats.han_basic,
                r.stats.han_ext_a,
                r.stats.han_ext_b,
                r.stats.total_chars,
                r.stats.kana,
                r.stats.hangul
            );
        }
    } else {
        println!("family\tfile\tface\than\than_basic\than_ext_a\than_ext_b\than_compat\ttotal\tkana\thangul\tlatin");
        for r in &rows {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                r.family,
                r.file,
                r.face_index,
                r.stats.han,
                r.stats.han_basic,
                r.stats.han_ext_a,
                r.stats.han_ext_b,
                r.stats.han_compat,
                r.stats.total_chars,
                r.stats.kana,
                r.stats.hangul,
                r.stats.latin
            );
        }
    }
}
