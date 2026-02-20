import UIKit
import WebKit
import Tauri

// Argument type for the generate command
class GenerateArgs: Decodable {
    let model: String
    let prompt: String
    let temperature: Double?
    let maxTokens: Int?

    enum CodingKeys: String, CodingKey {
        case model, prompt, temperature
        case maxTokens = "max_tokens"
    }
}

class LlmPlugin: Plugin {

    // -- listModels --

    @objc public func listModels(_ invoke: Invoke) {
        if #available(iOS 26, *) {
            // FoundationModels is available on iOS 26+
            // For now, report availability check via a placeholder
            // since FoundationModels import requires Xcode 26 SDK
            let info: [String: Any] = [
                "id": "apple-foundation-model",
                "name": "Apple Foundation Model",
                "provider": "apple",
                "available": false,
                "reason": "Requires iOS 26 with Apple Intelligence enabled"
            ]
            invoke.resolve([info])
        } else {
            // iOS < 26: no FoundationModels framework
            invoke.resolve([])
        }
    }

    // -- generate --

    @objc public func generate(_ invoke: Invoke) {
        guard #available(iOS 26, *) else {
            invoke.reject("FoundationModels requires iOS 26+")
            return
        }

        do {
            let args = try invoke.parseArgs(GenerateArgs.self)

            // FoundationModels generation requires Xcode 26 SDK
            // This is a placeholder that will work once compiled with the right SDK
            invoke.reject("Apple Foundation Model generation not yet available on this device")
        } catch {
            invoke.reject("Failed to parse arguments: \(error.localizedDescription)")
        }
    }
}

@_cdecl("init_plugin_llm")
func initPlugin(name: SRString, webview: WKWebView?) {
    Tauri.registerPlugin(webview: webview, name: name.toString(), plugin: LlmPlugin())
}
