import { animate } from "motion/react";
import { springs } from "./springs";

export function animateScrollToBottom(element: HTMLElement, onComplete?: () => void) {
  const startScrollTop = element.scrollTop;
  const controls = animate(0, 1, {
    ...springs.standard,
    onUpdate: (progress) => {
      const targetScrollTop = scrollBottomTop(element);
      const distance = targetScrollTop - startScrollTop;
      element.scrollTop = Math.min(
        targetScrollTop,
        Math.max(0, startScrollTop + distance * progress)
      );
    },
    onComplete: () => {
      element.scrollTop = scrollBottomTop(element);
      onComplete?.();
    }
  });

  return () => controls.stop();
}

function scrollBottomTop(element: HTMLElement) {
  return Math.max(0, element.scrollHeight - element.clientHeight);
}
