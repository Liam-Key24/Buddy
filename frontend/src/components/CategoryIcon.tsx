import {
  Barbell,
  Circle,
  FolderSimple,
  Lightning,
  Lock,
  Mountains,
  Target,
} from "@phosphor-icons/react";
import type { ComponentType } from "react";

const ICONS: Record<string, ComponentType<{ size?: number; weight?: "duotone" | "fill" | "regular" }>> = {
  mountain: Mountains,
  mountains: Mountains,
  climb: Mountains,
  barbell: Barbell,
  strength: Barbell,
  workout: Barbell,
  lock: Lock,
  fixed: Lock,
  target: Target,
  lightning: Lightning,
  folder: FolderSimple,
  circle: Circle,
};

export function categoryIconFor(name: string) {
  const n = name.toLowerCase();
  for (const [key, Icon] of Object.entries(ICONS)) {
    if (n.includes(key)) return Icon;
  }
  return Circle;
}

export function CategoryIcon({ name, size = 14 }: { name: string; size?: number }) {
  const Icon = categoryIconFor(name);
  return <Icon size={size} weight={Icon === Circle ? "fill" : "duotone"} />;
}
