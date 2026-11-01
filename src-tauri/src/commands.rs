use crate::services::metadata::{self, AppMetadata};

/// Compile-time application information only. No caller input or external access.
#[tauri::command]
pub fn get_app_metadata() -> AppMetadata {
    metadata::read()
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
