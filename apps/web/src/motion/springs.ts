import type { Transition } from "motion/react";

export type SpringPreset = "micro" | "standard" | "surface";

export const springs = {
  micro: { type: "spring", mass: 1, stiffness: 1296, damping: 72 },
  standard: { type: "spring", mass: 1, stiffness: 576, damping: 48 },
  surface: { type: "spring", mass: 1, stiffness: 324, damping: 36 }
} as const satisfies Record<SpringPreset, Transition>;
