mod commands;
mod services;

pub fn run() {
    // Validate storage BEFORE Builder::run creates a native window or WebView profile.
    let storage = std::env::current_exe()
        .map_err(|_| services::portable::StorageError::Unavailable)
        .and_then(|executable| services::portable::PortableStore::open(&executable));
    let storage = match storage {
        Ok(storage) => storage,
        Err(error) => {
            startup_error(error.message());
            std::process::exit(1);
        }
    };
    let mut context = tauri::generate_context!();
    // Pin the supported override to the validated absolute executable-adjacent root.
    context.config_mut().app.app_directories_override = Some(
        tauri::utils::config::AppDirectoriesOverride::Root(storage.root().to_path_buf()),
    );
    if configure(tauri::Builder::default())
        .manage(storage)
        .run(context)
        .is_err()
    {
        startup_error("ECHELON OS could not start its desktop window. Ensure Microsoft Edge WebView2 Evergreen Runtime is installed, close other copies, and retry from a writable extracted folder. No alternative storage location was selected.");
        std::process::exit(1);
    }
}

fn startup_error(message: &str) {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(
                window: *mut std::ffi::c_void,
                text: *const u16,
                title: *const u16,
                flags: u32,
            ) -> i32;
        }
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "ECHELON OS — startup recovery"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // Valid terminated buffers; no owner window exists yet. Error dialog only, no paths/OS error details.
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
        }
    }
    #[cfg(not(windows))]
    eprintln!("{message}");
}

fn configure<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        commands::get_app_metadata,
        commands::get_theme,
        commands::set_theme
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test::{get_ipc_response, mock_builder, INVOKE_KEY},
        webview::InvokeRequest,
        WebviewWindowBuilder,
    };

    fn request(command: &str, origin: &str) -> InvokeRequest {
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().expect("valid test origin"),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        }
    }

    #[test]
    fn portable_override_covers_all_app_paths_without_window_override() {
        use tauri::Manager;
        let root = std::env::temp_dir().join("echelon-path-contract-test");
        let mut context = tauri::generate_context!();
        assert!(context
            .config()
            .app
            .windows
            .iter()
            .all(|window| window.data_directory.is_none()));
        context.config_mut().app.app_directories_override = Some(
            tauri::utils::config::AppDirectoriesOverride::Root(root.clone()),
        );
        let app = configure(mock_builder()).build(context).expect("mock app");
        assert_eq!(app.path().app_config_dir().unwrap(), root);
        assert_eq!(app.path().app_data_dir().unwrap(), root);
        assert_eq!(app.path().app_local_data_dir().unwrap(), root);
        assert_eq!(app.path().app_cache_dir().unwrap(), root.join("caches"));
        assert_eq!(app.path().app_log_dir().unwrap(), root.join("logs"));
    }

    #[test]
    fn appearance_ipc_is_typed_and_main_local_only() {
        use crate::services::portable::{PortableStore, ThemeId};
        let root = std::env::temp_dir().join(format!(
            "echelon-ipc-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        {
            let store = PortableStore::open(&root.join("echelon-os.exe")).unwrap();
            let mut context = tauri::generate_context!();
            context.config_mut().app.app_directories_override = Some(
                tauri::utils::config::AppDirectoriesOverride::Root(store.root().to_path_buf()),
            );
            let app = configure(mock_builder())
                .manage(store)
                .build(context)
                .unwrap();
            let main = WebviewWindowBuilder::new(&app, "main", Default::default())
                .build()
                .unwrap();
            let other = WebviewWindowBuilder::new(&app, "other", Default::default())
                .build()
                .unwrap();
            let local = "http://tauri.localhost";
            let mut write = request("set_theme", local);
            write.body = InvokeBody::Json(serde_json::json!({"theme": "amarr"}));
            assert!(get_ipc_response(&main, write).is_ok());
            assert_eq!(
                get_ipc_response(&main, request("get_theme", local))
                    .unwrap()
                    .deserialize::<ThemeId>()
                    .unwrap(),
                ThemeId::Amarr
            );
            for command in ["get_theme", "set_theme"] {
                let mut forbidden = request(command, local);
                forbidden.body = InvokeBody::Json(serde_json::json!({"theme":"caldari"}));
                assert!(get_ipc_response(&other, forbidden).is_err());
                let mut remote = request(command, "https://example.com");
                remote.body = InvokeBody::Json(serde_json::json!({"theme":"caldari"}));
                assert!(get_ipc_response(&main, remote).is_err());
            }
            let mut invalid = request("set_theme", local);
            invalid.body = InvokeBody::Json(serde_json::json!({"theme":"../../outside"}));
            assert!(get_ipc_response(&main, invalid).is_err());
            assert_eq!(
                get_ipc_response(&main, request("get_theme", local))
                    .unwrap()
                    .deserialize::<ThemeId>()
                    .unwrap(),
                ThemeId::Amarr
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ipc_dispatch_obeys_the_real_application_capability() {
        // The runtime is mocked; command dispatch and generated application ACL
        // come from the same configuration as production. No real WebView starts.
        let app = configure(mock_builder())
            .build(tauri::generate_context!())
            .expect("test app must build");
        let main = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("main test window");
        let other = WebviewWindowBuilder::new(&app, "other", Default::default())
            .build()
            .expect("unprivileged test window");
        let local = "http://tauri.localhost";

        let response = get_ipc_response(&main, request("get_app_metadata", local))
            .expect("local main window may read metadata")
            .deserialize::<serde_json::Value>()
            .expect("metadata must deserialize");
        assert_eq!(response["name"], "ECHELON OS");
        assert_eq!(response["milestone"], "M1-B");
        assert!(get_ipc_response(&other, request("get_app_metadata", local)).is_err());
        assert!(
            get_ipc_response(&main, request("get_app_metadata", "https://example.com")).is_err()
        );
        assert!(get_ipc_response(&main, request("unknown_command", local)).is_err());
    }
}
