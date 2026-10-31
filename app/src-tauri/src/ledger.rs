//! 启用账本：把「临时启用」的意图先写进磁盘，再动注册表。
//!
//! 「启用至关闭」走用户级注册（HKCU 登记 + `AddFontResourceW`），这件事本身
//! 是持久的 —— 所以崩溃之后注册表项会一直留着，字体变成永久安装。以前这条
//! 记录只存在内存里（`SESSION_ACTIVATED`），进程一崩就忘了自己改过什么，
//! 也就无从收拾。
//!
//! 这里把它落盘，按两阶段提交处理：
//!
//! 1. 记账并落盘（**先于**写注册表）
//! 2. 写注册表项
//! 3. 正常退出 → 撤销字体、销账
//! 4. 崩溃 → 条目留在磁盘，下次启动 [`self_heal`] 回滚
//!
//! 回滚只信任一件事：账本里有这条，且注册表里有值精确等于这条路径的项。
//! 只看「项名存在」不够 —— 项名会撞车，也可能被别的程序重建。两条都中
//! 才动手，缺一条就不碰：宁可漏清，不能误删。

use crate::store::{ActivationEntry, AppState};
use serde::Serialize;
use std::collections::HashSet;

/// 回滚结果，交给调用方决定要不要写日志。
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealReport {
    /// 成功清掉的字体路径。
    pub rolled_back: Vec<String>,
    /// 账本有、注册表里却已经没有的条数（上次其实撤销成功，只是没来得及销账）。
    pub already_gone: usize,
    /// 删注册表项失败、留着下次再试的条数。
    pub kept: usize,
}

impl HealReport {
    /// 什么都没发生 —— 常见情况，账本本来就是空的。
    pub fn is_empty(&self) -> bool {
        self.rolled_back.is_empty() && self.already_gone == 0 && self.kept == 0
    }
}

/// 记账。返回真正新增的条数；同一条路径不会记两次。
pub fn begin(state: &mut AppState, paths: Vec<String>) -> usize {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut added = 0;
    for path in paths {
        if state.activation_ledger.iter().any(|e| e.path == path) {
            continue;
        }
        state.activation_ledger.push(ActivationEntry { path, at: now });
        added += 1;
    }
    added
}

/// 销账：正常撤销之后把这条去掉。返回是否真的删了一条。
pub fn commit(state: &mut AppState, path: &str) -> bool {
    let before = state.activation_ledger.len();
    state.activation_ledger.retain(|e| e.path != path);
    state.activation_ledger.len() != before
}

/// 判定结果，和注册表操作隔开，方便单独测。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// 要删的注册表项名，以及它对应的账本路径。
    pub to_delete: Vec<(String, String)>,
    /// 账本有、注册表已没有的条数。
    pub gone: usize,
}

/// 拿账本和当前注册表登记对一遍，算出该删哪些项。
fn plan(ledger: &[ActivationEntry], registered: &[(String, String)]) -> Plan {
    let mut out = Plan::default();
    for entry in ledger {
        let wanted = crate::registry::normalize(&entry.path);
        let mut hit = false;
        for (name, value) in registered {
            if crate::registry::normalize(value) == wanted {
                out.to_delete.push((name.clone(), entry.path.clone()));
                hit = true;
            }
        }
        if !hit {
            out.gone += 1;
        }
    }
    out
}

/// 回滚上次崩溃留下的启用，并把账本收敛到「只剩还没清掉的」。
pub fn self_heal(state: &mut AppState) -> HealReport {
    let mut report = HealReport::default();
    if state.activation_ledger.is_empty() {
        return report;
    }

    let registered = crate::registry::user_entries();
    let planned = plan(&state.activation_ledger, &registered);

    let mut failed: HashSet<String> = HashSet::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (name, path) in &planned.to_delete {
        match crate::registry::delete_user_entry(name) {
            Ok(()) => {
                if seen.insert(crate::registry::normalize(path)) {
                    report.rolled_back.push(path.clone());
                }
            }
            Err(_) => {
                failed.insert(crate::registry::normalize(path));
            }
        }
    }

    // 删成功的顺手从 GDI 的表里摘掉：崩溃后如果没重启过，字体还在本次
    // 登录的会话里加载着，光删注册表项它照样能被枚举到。
    for path in &report.rolled_back {
        crate::registry::remove_font_resource(path);
    }

    let entries = std::mem::take(&mut state.activation_ledger);
    state.activation_ledger = entries
        .into_iter()
        .filter(|e| failed.contains(&crate::registry::normalize(&e.path)))
        .collect();

    report.kept = state.activation_ledger.len();
    report.already_gone = planned.gone;
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> ActivationEntry {
        ActivationEntry {
            path: path.to_string(),
            at: 1_700_000_000,
        }
    }

    fn state_with(paths: &[&str]) -> AppState {
        AppState {
            activation_ledger: paths.iter().map(|p| entry(p)).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn begin_skips_duplicates() {
        let mut state = state_with(&[r"E:\字体\a.ttf"]);
        let added = begin(&mut state, vec![r"E:\字体\a.ttf".into(), r"E:\字体\b.ttf".into()]);
        assert_eq!(added, 1);
        assert_eq!(state.activation_ledger.len(), 2);
    }

    #[test]
    fn commit_removes_only_that_path() {
        let mut state = state_with(&[r"E:\字体\a.ttf", r"E:\字体\b.ttf"]);
        assert!(commit(&mut state, r"E:\字体\a.ttf"));
        assert_eq!(state.activation_ledger.len(), 1);
        assert_eq!(state.activation_ledger[0].path, r"E:\字体\b.ttf");
        assert!(!commit(&mut state, r"E:\字体\z.ttf"));
    }

    #[test]
    fn plan_matches_the_registry_value_not_the_name() {
        let ledger = vec![entry(r"E:\字体\a.ttf")];
        // 项名和路径完全无关，仍要靠值对上
        let registered = vec![("某某字体 (TrueType)".to_string(), r"e:\字体\A.TTF".to_string())];
        let planned = plan(&ledger, &registered);
        assert_eq!(planned.to_delete.len(), 1);
        assert_eq!(planned.to_delete[0].0, "某某字体 (TrueType)");
        assert_eq!(planned.gone, 0);
    }

    #[test]
    fn plan_leaves_other_fonts_alone() {
        let ledger = vec![entry(r"E:\字体\a.ttf")];
        let registered = vec![
            ("SimSun (TrueType)".to_string(), r"C:\Windows\Fonts\simsun.ttc".to_string()),
            ("别的字体 (TrueType)".to_string(), r"E:\字体\c.ttf".to_string()),
        ];
        let planned = plan(&ledger, &registered);
        assert!(planned.to_delete.is_empty());
        // 账本有、注册表没有 → 记成已经不在，销账即可
        assert_eq!(planned.gone, 1);
    }

    #[test]
    fn plan_counts_every_entry_that_no_longer_exists() {
        let ledger = vec![entry(r"E:\字体\a.ttf"), entry(r"E:\字体\b.ttf")];
        let planned = plan(&ledger, &[]);
        assert_eq!(planned.gone, 2);
        assert!(planned.to_delete.is_empty());
    }

    #[test]
    fn report_is_empty_when_nothing_happened() {
        assert!(HealReport::default().is_empty());
        let report = HealReport {
            already_gone: 1,
            ..Default::default()
        };
        assert!(!report.is_empty());
    }

    /// 端到端：真的写一条 HKCU 项，再让自愈把它清掉。
    ///
    /// 拿系统字体的一份**临时副本**做靶子，注册表项名也用一次性的，
    /// 失败时只可能留下一份 tmp 文件和一条自造项，不碰任何真实字体。
    #[test]
    #[ignore = "writes to the real HKCU font registry"]
    fn self_heal_removes_a_real_registry_entry() {
        let src = std::path::Path::new(r"C:\Windows\Fonts\arial.ttf");
        if !src.exists() {
            eprintln!("no arial.ttf on this machine, skipping");
            return;
        }
        let probe = std::env::temp_dir().join("zfm_heal_probe.ttf");
        std::fs::copy(src, &probe).expect("copy probe font");
        let path = probe.to_string_lossy().into_owned();

        let name = "ZFM Self Heal Probe (TrueType)";
        let _ = crate::registry::delete_user_entry(name);
        crate::registry::set_user_entry(name, &path).expect("write probe entry");
        assert!(
            crate::registry::user_entries_for(&path).iter().any(|n| n == name),
            "probe entry should be in the registry before healing"
        );

        let mut state = state_with(&[path.as_str()]);
        let report = self_heal(&mut state);

        assert_eq!(report.rolled_back, vec![path.clone()]);
        assert_eq!(report.kept, 0);
        assert!(state.activation_ledger.is_empty(), "ledger should be settled");
        assert!(
            !crate::registry::user_entries().iter().any(|(n, _)| n == name),
            "probe entry should be gone after healing"
        );

        let _ = std::fs::remove_file(&probe);
    }
}
