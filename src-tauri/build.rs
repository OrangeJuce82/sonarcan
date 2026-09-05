fn main() {
    println!("cargo:rerun-if-env-changed=SONARCAN_EDITION");
    if let Ok(edition) = std::env::var("SONARCAN_EDITION") {
        assert!(
            matches!(edition.as_str(), "full" | "light"),
            "SONARCAN_EDITION must be either full or light"
        );
    }
    tauri_build::build()
}
