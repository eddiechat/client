import type { ReactNode } from "react";

const URL_RE =
  /https?:\/\/[^\s<>]+/gi;

function extractDomain(url: string): string {
  try {
    const host = new URL(url).hostname;
    return host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

export function LinkifiedText({ text }: { text: string }) {
  const parts: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  URL_RE.lastIndex = 0;
  while ((match = URL_RE.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index));
    }
    const url = match[0];
    const domain = extractDomain(url);
    parts.push(
      <a
        key={match.index}
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        onClick={(e) => e.stopPropagation()}
        className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded-md text-[12px] font-semibold no-underline align-baseline leading-none"
        style={{
          background: "color-mix(in srgb, var(--color-accent-blue) 12%, var(--color-bg-tertiary))",
          color: "var(--color-accent-blue)",
          border: "1px solid color-mix(in srgb, var(--color-accent-blue) 25%, transparent)",
        }}
      >
        <svg width="11" height="11" viewBox="0 0 16 16" fill="none" className="shrink-0 opacity-70">
          <path
            d="M6.5 3.5H3.5A1.5 1.5 0 002 5v7.5A1.5 1.5 0 003.5 14H11a1.5 1.5 0 001.5-1.5v-3M9.5 2H14v4.5M14 2L6.5 9.5"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        {domain}
      </a>,
    );
    lastIndex = URL_RE.lastIndex;
  }

  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }

  return <>{parts}</>;
}
