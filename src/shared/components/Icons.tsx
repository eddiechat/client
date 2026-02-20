export function ComposeIcon() {
  return (
    <svg viewBox="0 0 24 24" className="w-5 h-5 stroke-text-dim fill-none [stroke-width:1.5] [stroke-linecap:round] [stroke-linejoin:round]">
      <path d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
    </svg>
  );
}

export function PointsIcon() {
  return (
    <svg width="8" height="8" viewBox="0 0 8 8">
      <circle cx="4" cy="4" r="4" fill="currentColor" />
    </svg>
  );
}

export function CirclesIcon() {
  return (
    <svg width="20" height="20" viewBox="0 0 20 20">
      <circle cx="10" cy="10" r="8" fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

export function LinesIcon() {
  return (
    <svg width="18" height="14" viewBox="0 0 18 14">
      <line x1="1" y1="2" x2="17" y2="2" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <line x1="1" y1="7" x2="17" y2="7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <line x1="1" y1="12" x2="17" y2="12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

export function SettingsToggle({ label, desc, value, onChange }: { label: string; desc: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="flex justify-between items-center py-3 border-b border-divider">
      <div>
        <div className="text-[13px] font-medium text-text-primary">{label}</div>
        <div className="text-[11px] text-text-dim mt-px">{desc}</div>
      </div>
      <button
        className={`relative w-[44px] h-[26px] rounded-full border cursor-pointer transition-colors shrink-0 ${value ? "bg-accent-green border-accent-green" : "bg-bg-tertiary border-divider"}`}
        onClick={() => onChange(!value)}
      >
        <span
          className={`absolute top-[2px] w-5 h-5 rounded-full bg-white transition-transform ${value ? "left-[21px]" : "left-[2px]"}`}
          style={{ boxShadow: "0 1px 3px rgba(0,0,0,0.15)" }}
        />
      </button>
    </div>
  );
}
