use crate::models::*;
use tauri::{plugin::PluginHandle, AppHandle, Runtime};

pub struct NativeBackend<R: Runtime> {
    handle: PluginHandle<R>,
}

pub fn init<R: Runtime>(
    _app: &AppHandle<R>,
    api: &tauri::plugin::PluginApi<R, Option<crate::LlmConfig>>,
) -> crate::Result<NativeBackend<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin(crate::PLUGIN_IDENTIFIER, "LlmPlugin")?;

    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(crate::init_plugin_llm)?;

    Ok(NativeBackend { handle })
}

impl<R: Runtime> NativeBackend<R> {
    pub fn list_models(&self) -> Vec<ModelInfo> {
        self.handle
            .run_mobile_plugin::<Vec<ModelInfo>>("listModels", ())
            .unwrap_or_else(|e| {
                log::warn!("Native listModels failed: {}", e);
                vec![]
            })
    }

    pub fn generate(&self, req: GenerateRequest) -> crate::Result<GenerateResponse> {
        self.handle
            .run_mobile_plugin::<GenerateResponse>("generate", &req)
            .map_err(|e| crate::Error::GenerationFailed(e.to_string()))
    }
}
