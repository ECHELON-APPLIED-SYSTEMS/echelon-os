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

use crate::services::eve::{self, Action, Confirmation, Error, Eve, View};

#[tauri::command]
pub fn eve_status(eve: tauri::State<'_, Eve>) -> Result<View, Error> {
    eve.status()
}

#[tauri::command]
pub fn eve_disconnect(eve: tauri::State<'_, Eve>) -> Result<View, Error> {
    eve.disconnect()
}

#[tauri::command]
pub async fn eve_connect(eve: tauri::State<'_, Eve>) -> Result<View, Error> {
    let eve = eve.inner().clone();
    tauri::async_runtime::spawn_blocking(move || eve.connect())
        .await
        .map_err(|_| Error::Unavailable)?
}

#[tauri::command]
pub async fn eve_refresh_location(eve: tauri::State<'_, Eve>) -> Result<View, Error> {
    let eve = eve.inner().clone();
    tauri::async_runtime::spawn_blocking(move || eve.refresh_location())
        .await
        .map_err(|_| Error::Unavailable)?
}

#[tauri::command]
pub fn eve_prepare_waypoint(
    destination: u64,
    action: Action,
    store: tauri::State<'_, crate::services::portable::PortableStore>,
    eve: tauri::State<'_, Eve>,
) -> Result<Confirmation, Error> {
    eve::validate_destination(&store, destination)?;
    eve.prepare(destination, action)
}

#[tauri::command]
pub async fn eve_send_waypoint(
    confirmation_id: String,
    store: tauri::State<'_, crate::services::portable::PortableStore>,
    eve: tauri::State<'_, Eve>,
) -> Result<View, Error> {
    if confirmation_id.len() != 43 {
        return Err(Error::InvalidConfirmation);
    }
    // Re-read the pinned file at confirmation time. UI validation is not authority.
    let destination = eve.confirmation_destination(&confirmation_id)?;
    eve::validate_destination(&store, destination)?;
    let eve = eve.inner().clone();
    tauri::async_runtime::spawn_blocking(move || eve.send_waypoint(&confirmation_id))
        .await
        .map_err(|_| Error::Unavailable)?
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
