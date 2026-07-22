import type { Transition } from "motion/react";

export type SpringPreset = "micro" | "standard" | "surface";

export const springs = {
  micro: { type: "spring", mass: 1, stiffness: 1568.16, damping: 79.2 },
  standard: { type: "spring", mass: 1, stiffness: 696.96, damping: 52.8 },
  surface: { type: "spring", mass: 1, stiffness: 392.04, damping: 39.6 }
} as const satisfies Record<SpringPreset, Transition>;
