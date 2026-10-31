fn main() {
    // Listing commands here generates application ACL permissions. Without this,
    // custom commands would be callable without an explicit capability grant.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            // Embed the Windows manifest separately for all linkable targets,
            // including lib unit tests; Tauri's default resource is bins-only.
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
            .app_manifest(tauri_build::AppManifest::new().commands(&["get_app_metadata"])),
    )
    .expect("failed to build ECHELON OS application resources");

    println!("cargo:rerun-if-changed=windows-manifest.rc");
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for_everything("windows-manifest.rc", embed_resource::NONE)
            .manifest_required()
            .expect("Windows executables and tests require the Common Controls v6 manifest");
    }
}
