import md5 from "md5";
import { useState, useEffect } from "react";

// Module-level cache: survives re-renders and route changes
const cache = new Map<string, "loading" | "found" | "none">();

export function gravatarUrl(email: string, size: number): string {
  const hash = md5(email.trim().toLowerCase());
  return `https://www.gravatar.com/avatar/${hash}?d=404&s=${size}`;
}

export function useGravatar(
  email: string | undefined,
  size: number,
): string | null {
  const key = email?.trim().toLowerCase();
  const [status, setStatus] = useState<"loading" | "found" | "none">(
    () => (key ? cache.get(key) ?? "loading" : "none"),
  );

  useEffect(() => {
    if (!key) {
      setStatus("none");
      return;
    }
    const cached = cache.get(key);
    if (cached === "found") {
      setStatus("found");
      return;
    }
    if (cached === "none") {
      setStatus("none");
      return;
    }
    if (cached === "loading") {
      // Another component already started the preflight — wait for it
      // by polling the cache briefly
      const interval = setInterval(() => {
        const v = cache.get(key);
        if (v === "found" || v === "none") {
          setStatus(v);
          clearInterval(interval);
        }
      }, 100);
      return () => clearInterval(interval);
    }

    // First encounter — fire preflight
    cache.set(key, "loading");
    setStatus("loading");

    const img = new Image();
    const url = gravatarUrl(key, size);
    img.onload = () => {
      cache.set(key, "found");
      setStatus("found");
    };
    img.onerror = () => {
      cache.set(key, "none");
      setStatus("none");
    };
    img.src = url;
  }, [key, size]);

  if (!key || status !== "found") return null;
  return gravatarUrl(key, size);
}
