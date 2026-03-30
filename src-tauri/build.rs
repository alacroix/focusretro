fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_AUTO_UPDATE");
    println!("cargo:rerun-if-changed=capabilities/updater.json");

    let updater_cap = std::path::Path::new("capabilities/updater.json");
    let feature_enabled = std::env::var("CARGO_FEATURE_AUTO_UPDATE").is_ok();

    // In offline builds, temporarily remove the committed updater capability
    // so Tauri validation doesn't fail on the missing updater plugin.
    let saved_content = if !feature_enabled && updater_cap.exists() {
        let content = std::fs::read_to_string(updater_cap).ok();
        std::fs::remove_file(updater_cap).ok();
        content
    } else {
        None
    };

    tauri_build::build();

    // Restore so the working tree stays clean after offline builds.
    if let Some(content) = saved_content {
        let _ = std::fs::write(updater_cap, content);
    }
}
