import { CheckCircle, Circle, CircleHalf } from "@phosphor-icons/react";

export const TASK_STATUSES = [
  { id: "not_started", label: "Not started", Icon: Circle, active: "text-blue-400" },
  { id: "in_progress", label: "In progress", Icon: CircleHalf, active: "text-blue-400" },
  { id: "completed", label: "Completed", Icon: CheckCircle, active: "text-blue-400" },
] as const;
