import type { Transition } from "motion/react";

export type SpringPreset = "micro" | "standard" | "surface";

export const springs = {
  micro: { type: "spring", mass: 1, stiffness: 900, damping: 60 },
  standard: { type: "spring", mass: 1, stiffness: 400, damping: 40 },
  surface: { type: "spring", mass: 1, stiffness: 225, damping: 30 }
} as const satisfies Record<SpringPreset, Transition>;
