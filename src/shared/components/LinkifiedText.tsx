import type { ReactNode } from "react";

const TOKEN_RE =
  /https?:\/\/[^\s<>]+|\[image:\s*[^\]]*\]/gi;

function extractDomain(url: string): string {
  try {
    const host = new URL(url).hostname;
    return host.replace(/^www\./, "");
  } catch {
    return url;
  }
}

function LinkBadge({ url, index }: { url: string; index: number }) {
  const domain = extractDomain(url);
  return (
    <a
      key={index}
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
    </a>
  );
}

function ImageBadge() {
  return (
    <svg width="1em" height="1em" viewBox="0 0 16 16" fill="none" className="inline align-middle opacity-50" style={{ verticalAlign: "-0.1em" }}>
      <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" stroke="currentColor" strokeWidth="1.3" />
      <circle cx="5.5" cy="6.5" r="1.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="M1.5 11l3.5-3.5L8 10.5l2.5-2.5 4 4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function LinkifiedText({ text }: { text: string }) {
  const parts: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  TOKEN_RE.lastIndex = 0;
  while ((match = TOKEN_RE.exec(text)) !== null) {
    if (match.index > lastIndex) {
      parts.push(text.slice(lastIndex, match.index));
    }
    const token = match[0];
    if (token.startsWith("[image:")) {
      parts.push(<ImageBadge key={match.index} />);
    } else {
      parts.push(<LinkBadge key={match.index} url={token} index={match.index} />);
    }
    lastIndex = TOKEN_RE.lastIndex;
  }

  if (lastIndex < text.length) {
    parts.push(text.slice(lastIndex));
  }

  return <>{parts}</>;
}
