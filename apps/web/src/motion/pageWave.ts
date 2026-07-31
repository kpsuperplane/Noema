import type { Transition } from "motion/react";

export const iosPageFadeTransition = {
  type: "tween",
  duration: 0.08,
  ease: [0.42, 0, 0.58, 1]
} as const satisfies Transition;

export const pageWaveLeadingTransition = {
  type: "tween",
  duration: 0.2,
  ease: [0.42, 0, 0.58, 1],
  opacity: { duration: 0, delay: 0.2 }
} as const satisfies Transition;

export const pageWaveTrailingTransition = {
  ...pageWaveLeadingTransition,
  delay: 0.08,
  opacity: { duration: 0, delay: 0.28 }
} as const satisfies Transition;

export function shouldUseIosPageFade() {
  if (typeof navigator === "undefined") return false;

  return /iP(?:ad|hone|od)/.test(navigator.userAgent) ||
    (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
}
