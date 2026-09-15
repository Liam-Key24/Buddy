import { Barbell, Circle, Lock, Mountains } from "@phosphor-icons/react";

export function CategoryIcon({ name, size = 14 }: { name: string; size?: number }) {
  const n = name.toLowerCase();
  if (n.includes("climb") || n.includes("mountain")) {
    return <Mountains size={size} weight="duotone" />;
  }
  if (n.includes("strength") || n.includes("barbell") || n.includes("workout")) {
    return <Barbell size={size} weight="duotone" />;
  }
  if (n.includes("fixed") || n.includes("lock")) {
    return <Lock size={size} weight="duotone" />;
  }
  return <Circle size={size} weight="fill" />;
}
