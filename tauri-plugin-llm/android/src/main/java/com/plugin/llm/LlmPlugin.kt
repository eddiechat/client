package com.plugin.llm

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import kotlinx.coroutines.*
import org.json.JSONArray

@TauriPlugin
class LlmPlugin(private val activity: Activity) : Plugin(activity) {

    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    // -- listModels --

    @Command
    fun listModels(invoke: Invoke) {
        scope.launch {
            try {
                // Gemini Nano via ML Kit GenAI requires the dependency
                // com.google.mlkit:genai-prompt which may not be available
                // on all devices. For now, return empty if not available.
                val arr = JSONArray()

                // TODO: When ML Kit GenAI is available, check model status:
                // val generativeModel = Generation.getClient()
                // val status = generativeModel.checkStatus()
                // Add model info based on status

                val result = JSObject()
                result.put("value", arr)
                invoke.resolve(result)
            } catch (e: Exception) {
                invoke.resolve(JSObject().put("value", JSONArray()))
            }
        }
    }

    // -- generate --

    @Command
    fun generate(invoke: Invoke) {
        val args = invoke.parseArgs(GenerateArgs::class.java)
        scope.launch {
            try {
                // TODO: Implement with ML Kit GenAI when available
                // val generativeModel = Generation.getClient()
                // val response = generativeModel.generateContent(...)
                invoke.reject("Native Android model not available on this device")
            } catch (e: Exception) {
                invoke.reject("Generation failed: ${e.message}")
            }
        }
    }
}
