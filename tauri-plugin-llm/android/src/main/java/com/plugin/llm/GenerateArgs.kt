package com.plugin.llm

import app.tauri.annotation.InvokeArg

@InvokeArg
class GenerateArgs {
    lateinit var model: String
    lateinit var prompt: String
    var temperature: Float = 0.7f
    var max_tokens: Int = 256
}
