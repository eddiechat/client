import { useState, useEffect } from "react";
import { getSkill, createSkill, updateSkill, deleteSkill, getSetting, getOllamaModels, fetchRecentMessages, ollamaComplete } from "../tauri";
import type { SkillModifiers, SkillSettings, Message } from "../tauri";
import type { StudioTab } from "./types";
import "./skills.css";

const MAX_MATCHES = 5;
const MAX_CHECKED = 50;
const SYSTEM_PROMPT = "You are an email classifier. Given a classification prompt and an email, decide if the email matches. Respond with exactly one word: true or false. Do not explain.";

interface SkillStudioProps {
  accountId: string;
  skillId?: string;
  initialPrompt?: string;
  onBack: () => void;
  onSaved: () => void;
  onDeleted: () => void;
}

const DEFAULT_MODIFIERS: SkillModifiers = {
  excludeNewsletters: false,
  onlyKnownSenders: false,
  hasAttachments: false,
  recentSixMonths: false,
  excludeAutoReplies: false,
};

const DEFAULT_SETTINGS: SkillSettings = {};

const MODIFIER_LABELS: { key: keyof SkillModifiers; label: string }[] = [
  { key: "excludeNewsletters", label: "+ Exclude newsletters" },
  { key: "onlyKnownSenders", label: "+ Only from known senders" },
  { key: "hasAttachments", label: "+ Has attachments" },
  { key: "recentSixMonths", label: "+ Recent 6 months" },
  { key: "excludeAutoReplies", label: "+ Exclude auto-replies" },
];

export function SkillStudio({ accountId, skillId, initialPrompt, onBack, onSaved, onDeleted }: SkillStudioProps) {
  const [tab, setTab] = useState<StudioTab>("edit");
  const [name, setName] = useState("My New Skill");
  const [icon, setIcon] = useState("\u26A1");
  const [iconBg, setIconBg] = useState("#5b4fc7");
  const [prompt, setPrompt] = useState(initialPrompt || "");
  const [modifiers, setModifiers] = useState<SkillModifiers>(DEFAULT_MODIFIERS);
  const [settings, setSettings] = useState<SkillSettings>(DEFAULT_SETTINGS);
  const [globalOllamaUrl, setGlobalOllamaUrl] = useState("");
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [temperature, setTemperature] = useState(0);
  const [loading, setLoading] = useState(!!skillId);

  // Preview state
  const [previewStatus, setPreviewStatus] = useState<"idle" | "running" | "done">("idle");
  const [previewMatches, setPreviewMatches] = useState<Message[]>([]);
  const [previewMisses, setPreviewMisses] = useState<Message[]>([]);
  const [previewProgress, setPreviewProgress] = useState({ checked: 0, total: 0 });
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewTab, setPreviewTab] = useState<"matches" | "misses">("matches");

  useEffect(() => {
    getSetting("ollama_url").then((v) => { if (v) setGlobalOllamaUrl(v); });
    getOllamaModels(skillId || "__DEFAULT__").then((data) => {
      setOllamaModels(data.models);
      setSelectedModel(data.selected_model);
    });
  }, [skillId]);

  useEffect(() => {
    if (!skillId) return;
    getSkill(skillId)
      .then((s) => {
        setName(s.name);
        setIcon(s.icon);
        setIconBg(s.icon_bg);
        setPrompt(s.prompt);
        try { setModifiers(JSON.parse(s.modifiers)); } catch { /* keep defaults */ }
        try {
          const parsed = JSON.parse(s.settings);
          setSettings(parsed);
          if (parsed.ollamaModel) setSelectedModel(parsed.ollamaModel);
          if (typeof parsed.temperature === "number") setTemperature(parsed.temperature);
        } catch { /* keep defaults */ }
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [skillId]);

  // Preview runner — fires each time user switches to the Preview tab
  useEffect(() => {
    if (tab !== "preview") return;
    if (!prompt.trim()) {
      setPreviewStatus("idle");
      setPreviewMatches([]);
      setPreviewMisses([]);
      setPreviewError("Enter a classification prompt in the Edit tab first.");
      return;
    }
    if (!selectedModel) {
      setPreviewStatus("idle");
      setPreviewMatches([]);
      setPreviewMisses([]);
      setPreviewError("Select a model in the Settings tab first.");
      return;
    }

    let cancelled = false;
    const model = selectedModel;

    async function runPreview() {
      setPreviewStatus("running");
      setPreviewMatches([]);
      setPreviewMisses([]);
      setPreviewError(null);
      setPreviewProgress({ checked: 0, total: 0 });

      try {
        const messages = await fetchRecentMessages(accountId, MAX_CHECKED);
        if (cancelled) return;
        setPreviewProgress({ checked: 0, total: messages.length });

        const effectiveUrl = globalOllamaUrl || "http://localhost:11434";
        const matches: Message[] = [];
        const misses: Message[] = [];

        for (let i = 0; i < messages.length; i++) {
          if (cancelled) return;
          if (matches.length >= MAX_MATCHES) break;

          const msg = messages[i];
          const bodySnippet = (msg.body_text || "").slice(0, 2000);
          const userPrompt = `Classification prompt: ${prompt}\n\nEmail subject: ${msg.subject || ""}\nEmail body: ${bodySnippet}`;

          try {
            const response = await ollamaComplete(effectiveUrl, model, SYSTEM_PROMPT, userPrompt, temperature);

            if (response.toLowerCase().includes("true")) {
              matches.push(msg);
              setPreviewMatches([...matches]);
            } else {
              misses.push(msg);
              setPreviewMisses([...misses]);
            }
          } catch {
            // skip on failure
          }

          if (cancelled) return;
          setPreviewProgress({ checked: i + 1, total: messages.length });
        }
      } catch (err) {
        if (!cancelled) {
          setPreviewError(String(err));
        }
      } finally {
        if (!cancelled) {
          setPreviewStatus("done");
        }
      }
    }

    runPreview();

    return () => {
      cancelled = true;
    };
  }, [tab]);

  function toggleModifier(key: keyof SkillModifiers) {
    setModifiers((prev) => ({ ...prev, [key]: !prev[key] }));
  }

  async function handleSave() {
    const mods = JSON.stringify(modifiers);
    const sets = JSON.stringify(settings);
    if (skillId) {
      await updateSkill(skillId, name, icon, iconBg, prompt, mods, sets);
    } else {
      await createSkill(accountId, name, icon, iconBg, prompt, mods, sets);
    }
    onSaved();
  }

  async function handleDelete() {
    if (!skillId) return;
    await deleteSkill(skillId);
    onDeleted();
  }

  if (loading) {
    return (
      <div className="skills-screen">
        <div className="skills-header" style={{ paddingTop: 'calc(12px + env(safe-area-inset-top, 0px))' }}>
          <button className="skills-back" onClick={onBack}>{"\u2039"}</button>
          <span className="skills-header-title">Skill Studio</span>
        </div>
      </div>
    );
  }

  return (
    <div className="skills-screen">
      <div className="skills-header" style={{ paddingTop: 'calc(12px + env(safe-area-inset-top, 0px))' }}>
        <button className="skills-back" onClick={onBack}>
          {"\u2039"}
        </button>
        <span className="skills-header-title">Skill Studio</span>
        <button className="skills-save-btn" onClick={handleSave}>
          Save
        </button>
      </div>

      {/* Icon + Name */}
      <div className="studio-icon-name">
        <div className="studio-icon" style={{ background: iconBg }}>
          {icon}
        </div>
        <div className="studio-name-group">
          <div className="studio-name-label">NAME</div>
          <input
            className="studio-name-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </div>
      </div>

      {/* Tabs */}
      <div className="studio-tabs">
        <button
          className={`studio-tab${tab === "edit" ? " active" : ""}`}
          onClick={() => setTab("edit")}
        >
          Edit
        </button>
        <button
          className={`studio-tab${tab === "preview" ? " active" : ""}`}
          onClick={() => setTab("preview")}
        >
          Preview
        </button>
        <button
          className={`studio-tab${tab === "settings" ? " active" : ""}`}
          onClick={() => setTab("settings")}
        >
          Settings
        </button>
      </div>

      <div className="skills-scroll" style={{ paddingBottom: 'calc(40px + env(safe-area-inset-bottom, 0px))' }}>
        {/* ── Edit Tab ── */}
        {tab === "edit" && (
          <>
            <div className="skills-section-label">CLASSIFICATION PROMPT</div>
            <textarea
              className="studio-prompt-textarea"
              placeholder="Describe what emails this skill should match..."
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
            />
            <p className="studio-prompt-note">
              This prompt runs locally via on-device AI against each incoming
              email
            </p>

            <div className="skills-section-label">QUICK MODIFIERS</div>
            <div className="modifier-chips">
              {MODIFIER_LABELS.map(({ key, label }) => (
                <button
                  key={key}
                  className={`modifier-chip${modifiers[key] ? " active" : ""}`}
                  onClick={() => toggleModifier(key)}
                >
                  {label}
                </button>
              ))}
            </div>

            <button className="studio-test-btn">
              {"\u25B6"} Test against my inbox
            </button>
          </>
        )}

        {/* ── Preview Tab ── */}
        {tab === "preview" && (
          <>
            {previewError && previewStatus === "idle" && (
              <div className="preview-empty">
                <p className="preview-empty-text">{previewError}</p>
              </div>
            )}

            {previewStatus === "running" && (
              <div className="preview-progress">
                <div className="preview-progress-text">
                  Checking messages... {previewProgress.checked}/{previewProgress.total}
                </div>
                {previewMatches.length > 0 && (
                  <div className="preview-match-count">
                    {previewMatches.length} match{previewMatches.length !== 1 ? "es" : ""} found
                  </div>
                )}
              </div>
            )}

            {previewError && previewStatus !== "idle" && (
              <div className="preview-error">
                <p className="preview-error-text">{previewError}</p>
              </div>
            )}

            {(previewMatches.length > 0 || previewMisses.length > 0) && (
              <>
                <div className="preview-subtabs">
                  <button
                    className={`preview-subtab${previewTab === "matches" ? " active" : ""}`}
                    onClick={() => setPreviewTab("matches")}
                  >
                    Matches ({previewMatches.length})
                  </button>
                  <button
                    className={`preview-subtab${previewTab === "misses" ? " active" : ""}`}
                    onClick={() => setPreviewTab("misses")}
                  >
                    Misses ({previewMisses.length})
                  </button>
                </div>

                {previewTab === "matches" && previewMatches.length === 0 && previewStatus === "done" && (
                  <div className="preview-empty">
                    <p className="preview-empty-text">
                      No matching emails found in the {previewProgress.checked} most recent messages.
                    </p>
                  </div>
                )}

                {previewTab === "matches" && previewMatches.map((msg) => (
                  <div key={msg.id} className="preview-card">
                    <div className="preview-card-sender">
                      {msg.from_name || msg.from_address}
                    </div>
                    <div className="preview-card-subject">
                      {msg.subject || "(no subject)"}
                    </div>
                    {msg.body_text && (
                      <div className="preview-card-snippet">
                        {msg.body_text.slice(0, 120)}
                        {msg.body_text.length > 120 ? "..." : ""}
                      </div>
                    )}
                  </div>
                ))}

                {previewTab === "misses" && previewMisses.length === 0 && previewStatus === "done" && (
                  <div className="preview-empty">
                    <p className="preview-empty-text">
                      All checked emails matched the classifier.
                    </p>
                  </div>
                )}

                {previewTab === "misses" && previewMisses.map((msg) => (
                  <div key={msg.id} className="preview-card preview-card-miss">
                    <div className="preview-card-sender">
                      {msg.from_name || msg.from_address}
                    </div>
                    <div className="preview-card-subject">
                      {msg.subject || "(no subject)"}
                    </div>
                    {msg.body_text && (
                      <div className="preview-card-snippet">
                        {msg.body_text.slice(0, 120)}
                        {msg.body_text.length > 120 ? "..." : ""}
                      </div>
                    )}
                  </div>
                ))}
              </>
            )}

            {previewStatus === "done" && previewMatches.length === 0 && previewMisses.length === 0 && !previewError && (
              <div className="preview-empty">
                <p className="preview-empty-text">
                  No matching emails found in the {previewProgress.checked} most recent messages.
                </p>
              </div>
            )}

            {previewStatus === "done" && (
              <div className="preview-summary">
                Checked {previewProgress.checked} messages &middot; {previewMatches.length} match{previewMatches.length !== 1 ? "es" : ""} &middot; {previewMisses.length} miss{previewMisses.length !== 1 ? "es" : ""}
              </div>
            )}
          </>
        )}

        {/* ── Settings Tab ── */}
        {tab === "settings" && (
          <>
            <div className="setting-section-header">Ollama</div>
            <div className="setting-row setting-input-row">
              <div className="setting-info">
                <div className="setting-name">Model</div>
                <div className="setting-desc">Classification model</div>
              </div>
              {ollamaModels.length > 0 ? (
                <select
                  className="setting-input"
                  value={selectedModel ?? ""}
                  onChange={(e) => {
                    setSelectedModel(e.target.value);
                    setSettings((prev) => ({ ...prev, ollamaModel: e.target.value }));
                  }}
                >
                  {!selectedModel && <option value="" disabled>Select model</option>}
                  {ollamaModels.map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              ) : (
                <select
                  className="setting-input setting-input-disabled"
                  disabled
                >
                  <option>Ollama not found</option>
                </select>
              )}
            </div>

            <div className="setting-row setting-input-row">
              <div className="setting-info">
                <div className="setting-name">Temperature</div>
                <div className="setting-desc">Lower = more deterministic, higher = more creative</div>
              </div>
              <div className="setting-slider-row">
                <input
                  type="range"
                  className="setting-slider"
                  min="0"
                  max="1"
                  step="0.1"
                  value={temperature}
                  onChange={(e) => {
                    const t = parseFloat(e.target.value);
                    setTemperature(t);
                    setSettings((prev) => ({ ...prev, temperature: t }));
                  }}
                />
                <span className="setting-slider-value">{temperature.toFixed(1)}</span>
              </div>
            </div>

            {skillId && (
              <div className="setting-row setting-delete-row">
                <div className="setting-info">
                  <div className="setting-name setting-delete-name">Delete this skill</div>
                  <div className="setting-desc">
                    Permanently remove this skill and its configuration
                  </div>
                </div>
                <button className="skill-delete-btn" onClick={handleDelete}>
                  Delete
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
