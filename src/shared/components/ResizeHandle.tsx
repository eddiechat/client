interface ResizeHandleProps {
  onMouseDown: (e: React.MouseEvent) => void;
  isDragging: boolean;
}

export function ResizeHandle({ onMouseDown, isDragging }: ResizeHandleProps) {
  return (
    <div
      onMouseDown={onMouseDown}
      className={`
        hidden md:flex
        w-1 hover:w-1.5
        cursor-col-resize
        items-center justify-center
        bg-divider
        transition-all duration-150
        hover:bg-text-muted
        flex-shrink-0
        ${isDragging ? "w-1.5 bg-text-muted" : ""}
      `}
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize sidebar"
    />
  );
}
