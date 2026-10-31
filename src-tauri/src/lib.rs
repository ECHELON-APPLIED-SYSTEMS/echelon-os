mod commands;
mod services;

pub fn run() {
    configure(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("failed to run ECHELON OS");
}

fn configure<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![commands::get_app_metadata])
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
        assert_eq!(response["milestone"], "M1-A");
        assert!(get_ipc_response(&other, request("get_app_metadata", local)).is_err());
        assert!(
            get_ipc_response(&main, request("get_app_metadata", "https://example.com")).is_err()
        );
        assert!(get_ipc_response(&main, request("unknown_command", local)).is_err());
    }
}
