// Windows C++/WinRT bridge for Phi Silica.
// This is a thin DLL exporting C functions for the Rust plugin.
// Requires Windows App SDK and a Copilot+ PC with NPU.
//
// Build with:
//   cl /EHsc /std:c++20 /LD windows_llm_bridge.cpp /link /OUT:windows_llm_bridge.dll
//
// This file is a reference implementation. It requires:
// - Windows 11 24H2+
// - Windows App SDK 1.7+
// - Copilot+ PC hardware (NPU)
// - winrt/Microsoft.Windows.AI.Text.h headers

#ifdef _WIN32

#include <Windows.h>
#include <cstring>
#include <cstdlib>

// Stub implementations that report "not supported" when the
// WinRT AI SDK is not available at compile time.

extern "C" __declspec(dllexport)
int windows_llm_check_availability() {
    // 3 = Not supported (no WinRT AI SDK at compile time)
    return 3;
}

extern "C" __declspec(dllexport)
char* windows_llm_generate(const char* prompt, float temperature, int maxTokens) {
    // Not supported without WinRT AI SDK
    return nullptr;
}

extern "C" __declspec(dllexport)
void windows_llm_free_string(char* ptr) {
    if (ptr) {
        free(ptr);
    }
}

#endif // _WIN32
