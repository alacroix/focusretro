fn main() {
    let caps_dir = std::path::Path::new("capabilities");
    let updater_cap = caps_dir.join("updater.json");
    let feature_enabled = std::env::var("CARGO_FEATURE_AUTO_UPDATE").is_ok();

    if feature_enabled {
        let content = r#"{
  "$schema": "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-utils/schema.json",
  "identifier": "updater",
  "description": "Update check capability",
  "windows": ["main"],
  "permissions": ["updater:default"]
}
"#;
        // Only write if content differs — avoids triggering Tauri's file watcher on every build
        let needs_write = std::fs::read_to_string(&updater_cap)
            .map(|existing| existing != content)
            .unwrap_or(true);
        if needs_write {
            std::fs::create_dir_all(caps_dir).ok();
            std::fs::write(&updater_cap, content).ok();
        }
    } else {
        std::fs::remove_file(&updater_cap).ok();
    }

    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_AUTO_UPDATE");
    println!("cargo:rerun-if-changed=capabilities/updater.json");

    tauri_build::build()
}
