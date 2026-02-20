import { useState, useEffect, useCallback } from "react";
import { listSkills, toggleSkill as toggleSkillCmd } from "../tauri";
import type { Skill } from "../tauri";
import "./skills.css";

interface SkillsHubProps {
  accountId: string;
  onBack: () => void;
  onNewSkill: () => void;
  onEditSkill: (skillId: string) => void;
}

export function SkillsHub({ accountId, onBack, onNewSkill, onEditSkill }: SkillsHubProps) {
  const [skills, setSkills] = useState<Skill[]>([]);

  const load = useCallback(() => {
    listSkills(accountId).then(setSkills).catch(console.error);
  }, [accountId]);

  useEffect(() => { load(); }, [load]);

  function handleToggle(id: string, currentEnabled: boolean) {
    setSkills((prev) =>
      prev.map((s) => (s.id === id ? { ...s, enabled: !currentEnabled } : s))
    );
    toggleSkillCmd(id, !currentEnabled).catch(() => load());
  }

  return (
    <div className="skills-screen">
      <div className="skills-header">
        <button className="skills-back" onClick={onBack}>
          {"\u2039"}
        </button>
        <span className="skills-header-title">Skills</span>
        <button className="skills-new-btn" onClick={onNewSkill}>
          + New
        </button>
      </div>

      <div className="skills-scroll">
        <div className="skills-section-label">MY SKILLS</div>

        {skills.map((s) => (
          <div key={s.id} className="skill-card" onClick={() => onEditSkill(s.id)}>
            <div className="skill-card-icon" style={{ background: s.icon_bg }}>
              {s.icon}
            </div>
            <div className="skill-card-info">
              <div className="skill-card-name">{s.name}</div>
              <div className="skill-card-meta">
                {!s.has_model ? "No model" : s.enabled ? "Enabled" : "Disabled"}
              </div>
            </div>
            <button
              className={`skill-toggle ${s.enabled ? "on" : "off"}${!s.has_model ? " no-model" : ""}`}
              onClick={(e) => {
                e.stopPropagation();
                if (!s.has_model) return;
                handleToggle(s.id, s.enabled);
              }}
            >
              <span className="skill-toggle-knob" />
            </button>
          </div>
        ))}

        <button className="skill-create-dashed" onClick={onNewSkill}>
          + Create a skill
        </button>

      </div>
    </div>
  );
}
