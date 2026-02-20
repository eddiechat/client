use crate::models::*;
use tauri::{AppHandle, Runtime};

/// The native (non-Ollama) backend for desktop platforms.
pub struct NativeBackend<R: Runtime> {
    #[allow(dead_code)]
    app: AppHandle<R>,
}

pub fn init<R: Runtime>(
    app: &AppHandle<R>,
    _api: &tauri::plugin::PluginApi<R, Option<crate::LlmConfig>>,
) -> crate::Result<NativeBackend<R>> {
    Ok(NativeBackend { app: app.clone() })
}

impl<R: Runtime> NativeBackend<R> {
    /// Return OS-native models. Each platform returns 0 or 1 models.
    pub fn list_models(&self) -> Vec<ModelInfo> {
        let mut models = Vec::new();

        #[cfg(target_os = "macos")]
        {
            models.extend(self.list_macos_models());
        }

        #[cfg(target_os = "windows")]
        {
            models.extend(self.list_windows_models());
        }

        // Linux: no native LLM API - return empty

        models
    }

    /// Route a generate request to the correct native backend.
    pub fn generate(&self, req: GenerateRequest) -> crate::Result<GenerateResponse> {
        #[cfg(target_os = "macos")]
        if req.model == "apple-foundation-model" {
            return self.generate_macos(req);
        }

        #[cfg(target_os = "windows")]
        if req.model == "phi-silica" {
            return self.generate_windows(req);
        }

        Err(crate::Error::ModelUnavailable(format!(
            "No native backend for model '{}'",
            req.model
        )))
    }
}

// -- macOS: Apple FoundationModels via Swift FFI --

#[cfg(target_os = "macos")]
extern "C" {
    /// Returns: 0=available, 1=device not eligible,
    ///          2=Apple Intelligence not enabled, 3=model not ready
    fn apple_llm_check_availability() -> i32;

    /// Synchronous wrapper around FoundationModels.LanguageModelSession.respond().
    /// Caller must free the returned string with apple_llm_free_string().
    fn apple_llm_generate(
        prompt: *const std::ffi::c_char,
        temperature: f64,
        max_tokens: i32,
    ) -> *mut std::ffi::c_char;

    fn apple_llm_free_string(ptr: *mut std::ffi::c_char);
}

#[cfg(target_os = "macos")]
impl<R: Runtime> NativeBackend<R> {
    fn list_macos_models(&self) -> Vec<ModelInfo> {
        let status = unsafe { apple_llm_check_availability() };
        vec![ModelInfo {
            id: "apple-foundation-model".into(),
            name: "Apple Foundation Model".into(),
            available: status == 0,
            reason: match status {
                0 => None,
                1 => Some("Device not eligible (requires Apple Silicon)".into()),
                2 => Some("Apple Intelligence not enabled in System Settings".into()),
                3 => Some("Model not ready (downloading or initializing)".into()),
                _ => Some("Unknown unavailability reason".into()),
            },
            provider: "apple".into(),
            metadata: None,
        }]
    }

    fn generate_macos(&self, req: GenerateRequest) -> crate::Result<GenerateResponse> {
        use std::ffi::{CStr, CString};

        let c_prompt =
            CString::new(req.prompt).map_err(|e| crate::Error::GenerationFailed(e.to_string()))?;

        let result_ptr = unsafe {
            apple_llm_generate(
                c_prompt.as_ptr(),
                req.temperature,
                req.max_tokens.unwrap_or(256) as i32,
            )
        };

        if result_ptr.is_null() {
            return Err(crate::Error::GenerationFailed(
                "FoundationModels returned null".into(),
            ));
        }

        let text = unsafe { CStr::from_ptr(result_ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { apple_llm_free_string(result_ptr) };

        Ok(GenerateResponse {
            text,
            model: "apple-foundation-model".into(),
            provider: "apple".into(),
        })
    }
}

// -- Windows: Phi Silica via C++/WinRT DLL --

#[cfg(target_os = "windows")]
#[link(name = "windows_llm_bridge")]
extern "C" {
    /// Returns: 0=ready, 1=ensuring ready, 2=not ready, 3=not supported
    fn windows_llm_check_availability() -> i32;

    fn windows_llm_generate(
        prompt: *const std::ffi::c_char,
        temperature: f32,
        max_tokens: i32,
    ) -> *mut std::ffi::c_char;

    fn windows_llm_free_string(ptr: *mut std::ffi::c_char);
}

#[cfg(target_os = "windows")]
impl<R: Runtime> NativeBackend<R> {
    fn list_windows_models(&self) -> Vec<ModelInfo> {
        let status = unsafe { windows_llm_check_availability() };
        vec![ModelInfo {
            id: "phi-silica".into(),
            name: "Phi Silica".into(),
            available: status == 0,
            reason: match status {
                0 => None,
                3 => Some("Copilot+ PC with NPU required".into()),
                _ => Some("Phi Silica not ready".into()),
            },
            provider: "windows".into(),
            metadata: None,
        }]
    }

    fn generate_windows(&self, req: GenerateRequest) -> crate::Result<GenerateResponse> {
        use std::ffi::{CStr, CString};

        let c_prompt =
            CString::new(req.prompt).map_err(|e| crate::Error::GenerationFailed(e.to_string()))?;

        let result_ptr = unsafe {
            windows_llm_generate(
                c_prompt.as_ptr(),
                req.temperature as f32,
                req.max_tokens.unwrap_or(256) as i32,
            )
        };

        if result_ptr.is_null() {
            return Err(crate::Error::GenerationFailed(
                "Phi Silica returned null".into(),
            ));
        }

        let text = unsafe { CStr::from_ptr(result_ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { windows_llm_free_string(result_ptr) };

        Ok(GenerateResponse {
            text,
            model: "phi-silica".into(),
            provider: "windows".into(),
        })
    }
}
