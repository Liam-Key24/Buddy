import type { ReactNode } from "react";

type SectionHeadProps = {
  title: ReactNode;
  action?: ReactNode;
};

export function SectionHead({ title, action }: SectionHeadProps) {
  return (
    <div className="page-section-head">
      <h2>{title}</h2>
      {action}
    </div>
  );
}
