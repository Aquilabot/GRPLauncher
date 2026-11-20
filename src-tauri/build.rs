fn main() {
    // Our manifest instead of Tauri's: it also says the program runs as the user (asInvoker) and on Windows 10/11.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri-build failed");
}
