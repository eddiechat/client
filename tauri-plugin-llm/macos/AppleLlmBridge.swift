import Foundation

// macOS Swift FFI bridge for Apple FoundationModels.
// This file is compiled into a static library that exports C-callable functions.
// Requires macOS 26+ and Xcode 26 for FoundationModels framework.

@_cdecl("apple_llm_check_availability")
public func checkAvailability() -> Int32 {
    if #available(macOS 26, *) {
        // FoundationModels availability check
        // Requires compilation with Xcode 26 SDK
        // For now, return 1 (not eligible) on older SDKs
        return 1
    } else {
        return 1 // macOS < 26
    }
}

@_cdecl("apple_llm_generate")
public func generate(
    prompt: UnsafePointer<CChar>,
    temperature: Double,
    maxTokens: Int32
) -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26, *) else { return nil }

    // FoundationModels generation requires Xcode 26 SDK
    // When compiled with the right SDK, this will use:
    //   let session = LanguageModelSession()
    //   let response = try await session.respond(to: promptString, options: options)
    return nil
}

@_cdecl("apple_llm_free_string")
public func freeString(ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}
