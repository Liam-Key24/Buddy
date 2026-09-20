import type { ReactNode } from "react";

type SectionHeadProps = {
  icon?: ReactNode;
  title: ReactNode;
  action?: ReactNode;
};

export function SectionHead({ icon, title, action }: SectionHeadProps) {
  return (
    <div className="mb-3 flex items-center justify-between gap-2">
      <h2 className="m-0 flex items-center gap-2 text-sm font-medium text-ink">
        {icon && <span className="text-mint-dim">{icon}</span>}
        {title}
      </h2>
      {action}
    </div>
  );
}
