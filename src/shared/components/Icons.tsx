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

export function SettingsSelect({ label, desc, value, options, onChange }: { label: string; desc: string; value: string; options: { value: string; label: string }[]; onChange: (v: string) => void }) {
  return (
    <div className="flex justify-between items-center py-3 border-b border-divider">
      <div>
        <div className="text-[14px] font-medium text-text-primary">{label}</div>
        <div className="text-[12px] text-text-dim mt-px">{desc}</div>
      </div>
      <select
        className="px-3 h-[34px] rounded-lg border border-divider bg-bg-tertiary text-[14px] text-text-primary outline-none transition-colors focus:border-accent-green appearance-none cursor-pointer pr-7"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={{ backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%239B9DAA' d='M3 4.5L6 7.5L9 4.5'/%3E%3C/svg%3E")`, backgroundRepeat: "no-repeat", backgroundPosition: "right 8px center" }}
      >
        {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
      </select>
    </div>
  );
}

export function SettingsToggle({ label, desc, value, onChange }: { label: string; desc: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="flex justify-between items-center py-3 border-b border-divider">
      <div>
        <div className="text-[14px] font-medium text-text-primary">{label}</div>
        <div className="text-[12px] text-text-dim mt-px">{desc}</div>
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
