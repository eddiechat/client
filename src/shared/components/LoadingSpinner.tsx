interface LoadingSpinnerProps {
  size?: "sm" | "md" | "lg";
  className?: string;
  message?: string;
}

const sizeClasses = {
  sm: "w-4 h-4",
  md: "w-6 h-6",
  lg: "w-8 h-8",
};

export function LoadingSpinner({
  size = "md",
  className = "",
  message,
}: LoadingSpinnerProps) {
  return (
    <div
      className={`flex flex-col items-center justify-center gap-3 text-text-muted text-sm ${className}`}
    >
      <div className={`spinner ${sizeClasses[size]}`} />
      {message && <span>{message}</span>}
    </div>
  );
}
