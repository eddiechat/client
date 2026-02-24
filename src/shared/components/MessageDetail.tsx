import { useState, useEffect } from "react";
import type { Message } from "../../tauri";
import { fetchMessageHtml } from "../../tauri";
import { fmtDate, parseAddresses, hasAddresses } from "../lib";
import { Avatar } from "./Avatar";

interface MessageDetailProps {
  message: Message;
  onBack: () => void;
}

export function MessageDetail({ message: m, onBack }: MessageDetailProps) {
  const needsFetch = !m.body_html || m.body_html.includes("cid:");
  const [html, setHtml] = useState<string | null>(needsFetch ? null : m.body_html);
  const [loading, setLoading] = useState(needsFetch);

  useEffect(() => {
    if (!needsFetch) return;
    setLoading(true);
    fetchMessageHtml(m.id)
      .then(setHtml)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, [m.id, needsFetch]);

  const sender = m.from_name || m.from_address;

  return (
    <div className="flex flex-col h-screen bg-bg-primary">
      {/* Header */}
      <div
        className="flex items-center gap-3 px-5 pb-3 border-b border-divider shrink-0 bg-bg-secondary"
        style={{ paddingTop: "calc(0.75rem + env(safe-area-inset-top, 0px))" }}
      >
        <button
          className="border-none bg-transparent text-[32px] cursor-pointer text-accent-green min-w-11 min-h-11 flex items-center justify-center -ml-2"
          onClick={onBack}
        >
          &#8249;
        </button>
        <div className="flex flex-col min-w-0">
          <span className="font-semibold text-[17px] text-text-primary leading-tight truncate">
            {m.subject || "(no subject)"}
          </span>
        </div>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto">
        {/* Sender row */}
        <div className="flex items-center gap-3 px-5 py-3">
          <Avatar
            name={sender}
            email={m.from_address}
            size={10}
            fontSize="text-[14px]"
            className="shrink-0"
          />
          <div className="flex flex-col min-w-0">
            <span className="font-semibold text-[15px] text-text-primary">
              {sender}
            </span>
            <span className="text-[12px] text-text-muted">
              {fmtDate(m.date)}
            </span>
          </div>
        </div>

        {/* Metadata rows */}
        <div className="px-5 py-2 border-b border-divider">
          <div className="flex gap-2 py-0.5 text-[13px] leading-snug">
            <span className="text-text-dim min-w-10 shrink-0 text-right">
              From
            </span>
            <span className="text-text-primary break-all">
              {m.from_name
                ? `${m.from_name} <${m.from_address}>`
                : m.from_address}
            </span>
          </div>
          <div className="flex gap-2 py-0.5 text-[13px] leading-snug">
            <span className="text-text-dim min-w-10 shrink-0 text-right">
              To
            </span>
            <span className="text-text-primary break-all">
              {parseAddresses(m.to_addresses)}
            </span>
          </div>
          {hasAddresses(m.cc_addresses) && (
            <div className="flex gap-2 py-0.5 text-[13px] leading-snug">
              <span className="text-text-dim min-w-10 shrink-0 text-right">
                Cc
              </span>
              <span className="text-text-primary break-all">
                {parseAddresses(m.cc_addresses)}
              </span>
            </div>
          )}
          <div className="flex gap-2 py-0.5 text-[13px] leading-snug">
            <span className="text-text-dim min-w-10 shrink-0 text-right">
              Date
            </span>
            <span className="text-text-primary break-all">
              {fmtDate(m.date)}
            </span>
          </div>
        </div>

        {/* Body */}
        <div className="px-5 py-4">
          {loading ? (
            <div className="text-text-muted text-[14px]">
              Loading full message&hellip;
            </div>
          ) : html ? (
            <HtmlBody html={html} />
          ) : (
            <div className="text-[14px] leading-relaxed text-text-secondary whitespace-pre-wrap break-words">
              {m.body_text || ""}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function sanitizeHtml(html: string): string {
  let clean = html;
  // Remove script tags and their content
  clean = clean.replace(/<script[\s\S]*?<\/script>/gi, "");
  // Remove event handler attributes
  clean = clean.replace(/\s+on\w+\s*=\s*["'][^"']*["']/gi, "");
  return clean;
}

function HtmlBody({ html }: { html: string }) {
  const cleanHtml = sanitizeHtml(html);

  return (
    <div
      className="html-email-body text-[14px] leading-relaxed text-text-secondary break-words [&_img]:max-w-full [&_img]:h-auto [&_a]:text-accent-green [&_pre]:overflow-x-auto [&_pre]:max-w-full [&_table]:max-w-full"
      dangerouslySetInnerHTML={{ __html: cleanHtml }}
    />
  );
}
