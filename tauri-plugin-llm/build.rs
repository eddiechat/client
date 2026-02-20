const COMMANDS: &[&str] = &["list_models", "generate", "configure_ollama"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
