use crate::font_types::FontSource;
use crate::store::AppState;

pub fn can_deactivate(source: FontSource) -> bool {
    // Windows: every font that is not a protected system font can be toggled.
    source != FontSource::System
}

pub fn sync(state: &mut AppState, path: &str, active: bool) -> Result<(), String> {
    if active {
        state.deactivated.remove(path);
    } else {
        state.deactivated.insert(path.to_string());
    }
    apply(state, path, active)
}

/// Windows keeps no filesystem alias for linked fonts, so dropping a link
/// needs no cleanup beyond removing the registry entry (handled elsewhere).
pub fn drop_linked_alias(_state: &AppState, _path: &str) {}

/// 这条路径在 HKCU 里还没有登记，启用时需要为它新写一条。
///
/// 记账和真正写入都看这一个判断 —— 两边结论必须一致，否则会出现
/// 「账本记了、其实没写」或反过来的漏账。
pub fn needs_registry_write(path: &str) -> bool {
    crate::registry::user_entries_for(path).is_empty()
}

fn apply(state: &mut AppState, path: &str, active: bool) -> Result<(), String> {
    use crate::registry;

    if active {
        let remembered = state
            .registry_backup
            .iter()
            .find(|(_, v)| v.as_str() == path)
            .map(|(k, _)| k.clone());
        let name = match remembered {
            Some(name) => name,
            None => registry::unique_user_name(&registry::value_name_for(path), path),
        };
        if needs_registry_write(path) {
            registry::set_user_entry(&name, path)?;
        }
        state.registry_backup.remove(&name);
        registry::add_font_resource(path);
    } else {
        if registry::is_machine_registered(path) {
            return Err(format!(
                "this font is installed for all users; deactivating it needs administrator rights: {path}"
            ));
        }

        let names = registry::user_entries_for(path);
        for name in &names {
            registry::delete_user_entry(name)?;
        }
        if let Some(first) = names.into_iter().next() {
            state.registry_backup.insert(first, path.to_string());
        }
        registry::remove_font_resource(path);
    }
    registry::broadcast_font_change();
    Ok(())
}

pub fn reconcile(state: &mut AppState) {
    let managed = crate::scanner::effective_managed_dir(state.active_library_dir());
    std::thread::spawn(move || {
        crate::registry::load_registered_outside(&managed);
    });
}

pub struct Probe {
    registered: std::collections::HashSet<String>,
}

pub fn probe() -> Probe {
    Probe {
        registered: crate::registry::registered_paths(),
    }
}

pub fn is_active(probe: &Probe, _state: &AppState, face: &crate::font_types::FontFace) -> bool {
    face.source == FontSource::System
        || probe
            .registered
            .contains(&crate::registry::normalize(&face.path))
}
