use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// 一次「临时启用」的意图记录。
///
/// 写在 HKCU 之前先落盘，正常退出时销账；进程崩溃则条目留在磁盘上，
/// 下次启动由 [`crate::ledger::self_heal`] 按三重校验回滚。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationEntry {
    /// 字体文件路径，按写入时的原样保存。
    pub path: String,
    /// 记账时间（Unix 秒），只用于日志展示。
    pub at: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppState {
    pub deactivated: HashSet<String>,

    pub tags: HashMap<String, Vec<String>>,

    pub collections: HashMap<String, Vec<String>>,

    pub favorites: HashSet<String>,

    /// 五档评分：家族名 → 1–5。0 不落库（清掉评分就是移除这一项）。
    pub ratings: HashMap<String, u8>,

    /// 用户自己起的别名，参与搜索（含拼音）。家族名 → 别名列表。
    pub aliases: HashMap<String, Vec<String>>,

    pub extra_dirs: Vec<String>,

    pub watch_enabled: bool,

    pub auto_activate_imports: bool,

    pub notes: HashMap<String, String>,

    pub prefs: serde_json::Value,

    pub parked: HashMap<String, String>,

    pub registry_backup: HashMap<String, String>,

    /// 临时启用留下的账本，见 [`crate::ledger`]。
    pub activation_ledger: Vec<ActivationEntry>,

    /// Override for the folder moved imports are stored in (None = default).
    pub library_dir: Option<String>,

    /// Whether the custom library folder is in effect (the path is kept
    /// even while disabled, so re-enabling restores it without asking).
    pub library_dir_enabled: bool,

    /// Font files linked in place: part of the library, never copied or moved.
    pub linked: HashSet<String>,

    /// Auto-activate fonts for open Affinity documents (needs its MCP server).
    pub affinity_enabled: bool,

    /// Deactivate Affinity session fonts when Affinity quits.
    #[serde(default = "default_true")]
    pub affinity_deactivate_on_quit: bool,

    pub online_fonts_enabled: bool,
}

pub struct Store(pub Mutex<AppState>);

fn default_true() -> bool {
    true
}

impl AppState {
    /// The custom library folder actually in effect, if one is enabled.
    pub fn active_library_dir(&self) -> Option<&str> {
        if self.library_dir_enabled {
            self.library_dir.as_deref()
        } else {
            None
        }
    }
}

/// 数据目录：放在**程序所在文件夹**的 `Data` 子目录里，跟着文件夹一起走。
///
/// 只有那里写不进去（比如被放进了只读位置）才退回系统用户目录，
/// 免得程序变成「能开、但不能存设置」。
pub fn app_data_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(resolve_data_dir).clone()
}

fn resolve_data_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("Data")))
    {
        if fs::create_dir_all(&dir).is_ok() {
            migrate_legacy(&dir);
            return dir;
        }
    }
    let fallback = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Fontora");
    let _ = fs::create_dir_all(&fallback);
    migrate_legacy(&fallback);
    fallback
}

/// 0.8.0 之前数据放在 `%APPDATA%\ZFontManager`。第一次用新版时搬过来，
/// 否则界面会像是「设置、标签、收藏全丢了」。
fn migrate_legacy(dir: &std::path::Path) {
    let Some(legacy) = dirs::data_dir().map(|d| d.join("ZFontManager")) else {
        return;
    };
    if legacy == dir || !legacy.is_dir() {
        return;
    }
    // 新目录里已经有配置就别动，避免覆盖用户在新版里做的改动。
    if dir.join("state.json").is_file() {
        return;
    }
    copy_missing(&legacy, dir);
}

fn copy_missing(from: &std::path::Path, to: &std::path::Path) {
    let Ok(entries) = fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            if fs::create_dir_all(&target).is_ok() {
                copy_missing(&source, &target);
            }
            continue;
        }
        if target.exists() {
            continue;
        }
        let _ = fs::copy(&source, &target);
    }
}

fn state_path() -> PathBuf {
    app_data_dir().join("state.json")
}

pub fn trash_dir() -> PathBuf {
    app_data_dir().join("Trash")
}

pub fn load() -> AppState {
    fs::read(state_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save(state: &AppState) -> Result<(), String> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// 和 [`save`] 一样，但等内容真正落到设备再返回。
///
/// 只给「记账」用：账本要在动注册表之前落地，写到内核页缓存还不够稳妥，
/// 所以这里多一次 `sync_all`。其他地方继续走 [`save`]，不值得为每条设置
/// 付一次磁盘 flush。
pub fn save_durable(state: &AppState) -> Result<(), String> {
    use std::io::Write;

    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(&json).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_sits_next_to_the_executable() {
        let dir = app_data_dir();
        let exe_dir = std::env::current_exe()
            .expect("test binary path")
            .parent()
            .expect("test binary parent")
            .to_path_buf();
        assert_eq!(dir, exe_dir.join("Data"), "数据目录应跟程序放一起");
        assert!(dir.is_dir(), "目录要真的建出来");
    }

    #[test]
    fn copy_missing_only_fills_the_gaps() {
        let root = std::env::temp_dir().join(format!("zfm-copy-{}", std::process::id()));
        let from = root.join("from");
        let to = root.join("to");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(from.join("sub")).unwrap();
        fs::create_dir_all(&to).unwrap();
        fs::write(from.join("a.txt"), b"new").unwrap();
        fs::write(from.join("b.txt"), b"from-old").unwrap();
        fs::write(from.join("sub/c.txt"), b"nested").unwrap();
        fs::write(to.join("b.txt"), b"keep-me").unwrap();

        copy_missing(&from, &to);

        assert_eq!(fs::read(to.join("a.txt")).unwrap(), b"new");
        assert_eq!(fs::read(to.join("b.txt")).unwrap(), b"keep-me", "已有的不许覆盖");
        assert_eq!(fs::read(to.join("sub/c.txt")).unwrap(), b"nested");
        let _ = fs::remove_dir_all(&root);
    }
}
