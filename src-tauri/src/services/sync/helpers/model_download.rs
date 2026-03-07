use crate::services::logger;
use std::path::PathBuf;
use tauri::Manager;
use tokio::io::AsyncWriteExt;

const MODEL_URL: &str =
    "https://github.com/eddiechat/classification/releases/download/v0.4.1-alpha/model_int8.onnx";
const MODEL_FILENAME: &str = "model_int8.onnx";

/// Return the path where the model should live inside the app data directory.
pub fn model_path(app: &tauri::AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("Failed to resolve app data directory");
    dir.join(MODEL_FILENAME)
}

/// Download the ONNX model to the app data directory.
/// Emits `sync:status` events so the onboarding screen can show progress.
pub async fn ensure_model(app: &tauri::AppHandle) -> Result<PathBuf, anyhow::Error> {
    let dest = model_path(app);

    // Clean up stale .rten model from previous versions
    if let Some(parent) = dest.parent() {
        let legacy = parent.join("model_int8.rten");
        if legacy.exists() {
            let _ = std::fs::remove_file(&legacy);
        }
    }

    if dest.exists() {
        logger::info("Classification model already downloaded");
        return Ok(dest);
    }

    // Make sure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    logger::info(&format!("Downloading classification model from {}", MODEL_URL));
    super::status_emit::emit_status(app, "downloading_model", "Downloading AI model...");

    let response = reqwest::get(MODEL_URL).await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download model: HTTP {}",
            response.status()
        ));
    }

    let total_size = response.content_length();
    if let Some(size) = total_size {
        logger::info(&format!(
            "Model size: {:.1} MB",
            size as f64 / 1_048_576.0
        ));
    }

    // Stream to a temp file, then rename (atomic-ish)
    let tmp_path = dest.with_extension("onnx.tmp");
    let mut file = tokio::fs::File::create(&tmp_path).await?;

    use futures::StreamExt;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_pct: u8 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if let Some(total) = total_size {
            let pct = ((downloaded as f64 / total as f64) * 100.0) as u8;
            // Emit every 5% to avoid flooding
            if pct >= last_pct + 5 || pct == 100 {
                last_pct = pct;
                logger::info(&format!("Model download: {}%", pct));
                super::status_emit::emit_status(
                    app,
                    "downloading_model",
                    &format!("Downloading AI model... {}%", pct),
                );
            }
        }
    }

    file.flush().await?;
    drop(file);

    // Rename tmp → final
    tokio::fs::rename(&tmp_path, &dest).await?;

    logger::info("Classification model download complete");
    super::status_emit::emit_status(app, "downloading_model_done", "AI model ready");

    Ok(dest)
}
