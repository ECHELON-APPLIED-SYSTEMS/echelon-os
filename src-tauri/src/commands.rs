use crate::services::metadata::{self, AppMetadata};

/// Compile-time application information only. No caller input or external access.
#[tauri::command]
pub fn get_app_metadata() -> AppMetadata {
    metadata::read()
}

/// Concrete Appearance value only; paths and arbitrary settings are never accepted.
#[tauri::command]
pub fn get_theme(
    store: tauri::State<'_, crate::services::portable::PortableStore>,
) -> Result<crate::services::portable::ThemeId, String> {
    store.theme().map_err(|error| error.message().to_owned())
}

#[tauri::command]
pub fn set_theme(
    theme: crate::services::portable::ThemeId,
    store: tauri::State<'_, crate::services::portable::PortableStore>,
) -> Result<(), String> {
    store
        .set_theme(theme)
        .map_err(|error| error.message().to_owned())
}

/// Explicit read only; no arguments or caller-selected paths.
#[tauri::command]
pub fn get_navcom_snapshot(
    store: tauri::State<'_, crate::services::portable::PortableStore>,
) -> crate::services::navcom::SnapshotRead {
    crate::services::navcom::read(&store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_preserves_the_frontend_wire_contract() {
        let result = serde_json::to_value(get_app_metadata()).expect("metadata must serialize");
        assert_eq!(result["name"], "ECHELON OS");
        assert_eq!(result["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(result["platform"], std::env::consts::OS);
        assert_eq!(result["milestone"], "M1-B");
        assert_eq!(result.as_object().expect("object").len(), 4);
    }
}
