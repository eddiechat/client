import { useState, useEffect, useCallback, useRef } from "react";

interface UseResizableSidebarOptions {
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  storageKey?: string;
}

interface UseResizableSidebarResult {
  sidebarWidth: number;
  isDesktop: boolean;
  isDragging: boolean;
  handleMouseDown: (e: React.MouseEvent) => void;
}

const MD_BREAKPOINT = 768;

export function useResizableSidebar(
  options: UseResizableSidebarOptions = {},
): UseResizableSidebarResult {
  const {
    defaultWidth = 320,
    minWidth = 200,
    maxWidth = 500,
    storageKey = "eddie-sidebar-width",
  } = options;

  const [sidebarWidth, setSidebarWidth] = useState<number>(() => {
    try {
      const stored = localStorage.getItem(storageKey);
      if (stored) {
        const parsed = parseInt(stored, 10);
        if (!isNaN(parsed) && parsed >= minWidth && parsed <= maxWidth) {
          return parsed;
        }
      }
    } catch {
      /* localStorage unavailable */
    }
    return defaultWidth;
  });

  const [isDesktop, setIsDesktop] = useState<boolean>(
    () => window.matchMedia(`(min-width: ${MD_BREAKPOINT}px)`).matches,
  );
  const [isDragging, setIsDragging] = useState(false);

  const dragState = useRef<{ startX: number; startWidth: number } | null>(
    null,
  );
  const latestWidth = useRef(sidebarWidth);

  // Keep the ref in sync with state
  useEffect(() => {
    latestWidth.current = sidebarWidth;
  }, [sidebarWidth]);

  // Media query listener
  useEffect(() => {
    const mql = window.matchMedia(`(min-width: ${MD_BREAKPOINT}px)`);
    const handler = (e: MediaQueryListEvent) => setIsDesktop(e.matches);
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, []);

  // Drag handlers on document
  useEffect(() => {
    if (!isDragging) return;

    const onMouseMove = (e: MouseEvent) => {
      if (!dragState.current) return;
      const delta = e.clientX - dragState.current.startX;
      const newWidth = Math.max(
        minWidth,
        Math.min(maxWidth, dragState.current.startWidth + delta),
      );
      setSidebarWidth(newWidth);
    };

    const onMouseUp = () => {
      setIsDragging(false);
      dragState.current = null;
      document.body.style.removeProperty("user-select");
      document.body.style.removeProperty("cursor");
      try {
        localStorage.setItem(storageKey, String(latestWidth.current));
      } catch {
        /* localStorage unavailable */
      }
    };

    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";

    return () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
      document.body.style.removeProperty("user-select");
      document.body.style.removeProperty("cursor");
    };
  }, [isDragging, minWidth, maxWidth, storageKey]);

  // Clamp sidebar width on window resize
  useEffect(() => {
    const onResize = () => {
      const maxAllowed = Math.min(maxWidth, window.innerWidth * 0.6);
      setSidebarWidth((w) => Math.min(w, maxAllowed));
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [maxWidth]);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragState.current = { startX: e.clientX, startWidth: sidebarWidth };
      setIsDragging(true);
    },
    [sidebarWidth],
  );

  return { sidebarWidth, isDesktop, isDragging, handleMouseDown };
}
