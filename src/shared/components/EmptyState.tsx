import type { ReactNode } from "react";

interface EmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className = "",
}: EmptyStateProps) {
  return (
    <div className={`flex flex-col h-full items-center justify-center ${className}`}>
      <div className="text-center p-10 max-w-xs">
        {icon && (
          <div className="w-20 h-20 mx-auto mb-6 bg-bg-tertiary rounded-full flex items-center justify-center">
            {icon}
          </div>
        )}
        <h3 className="text-xl font-semibold text-text-primary mb-2">{title}</h3>
        {description && (
          <p className="text-sm text-text-muted leading-relaxed">{description}</p>
        )}
        {action && <div className="mt-4">{action}</div>}
      </div>
    </div>
  );
}
