import { invoke } from '@tauri-apps/api/core';

// -- Types --

export interface ModelInfo {
  id: string;
  name: string;
  available: boolean;
  reason?: string;
  provider: 'apple' | 'android' | 'windows' | 'ollama';
  metadata?: Record<string, unknown>;
}

export interface GenerateOptions {
  model: string;
  prompt: string;
  temperature: number;
  maxTokens?: number;
}

export interface GenerateResponse {
  text: string;
  model: string;
  provider: 'apple' | 'android' | 'windows' | 'ollama';
}

export interface OllamaSettings {
  /** Base URL, e.g. "http://localhost:11434". Pass null to disable. */
  url: string | null;
  /** API key for reverse proxies. Pass null for no auth. */
  apiKey?: string | null;
  /** HTTP timeout in seconds. Default: 120. */
  timeoutSecs?: number;
}

// -- Commands --

/**
 * List all available models across OS-native and Ollama backends.
 * Returns an empty array if no backends are available.
 */
export async function listModels(): Promise<ModelInfo[]> {
  return invoke<ModelInfo[]>('plugin:llm|list_models');
}

/**
 * Generate a non-streaming completion.
 * The model ID determines which backend is used:
 *   "ollama:..." -> Ollama HTTP API
 *   anything else -> OS-native backend
 */
export async function generate(options: GenerateOptions): Promise<GenerateResponse> {
  return invoke<GenerateResponse>('plugin:llm|generate', {
    payload: {
      model: options.model,
      prompt: options.prompt,
      temperature: options.temperature,
      max_tokens: options.maxTokens,
    },
  });
}

/**
 * Hot-swap the Ollama connection at runtime.
 * Call this after loading user settings from your database,
 * and again whenever the user changes their Ollama config.
 *
 * Takes effect immediately - no restart required.
 * In-flight requests complete against the old config.
 */
export async function configureOllama(settings: OllamaSettings): Promise<void> {
  return invoke('plugin:llm|configure_ollama', {
    settings: {
      url: settings.url,
      api_key: settings.apiKey ?? null,
      timeout_secs: settings.timeoutSecs ?? 120,
    },
  });
}
