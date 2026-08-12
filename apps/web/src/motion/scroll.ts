import { animate } from "motion/react";
import { springs } from "./springs";

type ScrollAnimationOptions = {
  onComplete?: () => void;
  translatedElement?: HTMLElement | null;
  initialTranslateY?: number;
};

export function animateScrollToBottom(
  element: HTMLElement,
  { onComplete, translatedElement, initialTranslateY = 0 }: ScrollAnimationOptions = {}
) {
  const startScrollTop = element.scrollTop;
  if (translatedElement && initialTranslateY > 0) {
    translatedElement.style.transform = `translate3d(0, ${initialTranslateY}px, 0)`;
  }
  const controls = animate(0, 1, {
    ...springs.standard,
    onUpdate: (progress) => {
      const targetScrollTop = scrollBottomTop(element);
      const distance = targetScrollTop - startScrollTop;
      element.scrollTop = Math.min(
        targetScrollTop,
        Math.max(0, startScrollTop + distance * progress)
      );
      if (translatedElement) {
        translatedElement.style.transform = `translate3d(0, ${initialTranslateY * (1 - progress)}px, 0)`;
      }
    },
    onComplete: () => {
      element.scrollTop = scrollBottomTop(element);
      translatedElement?.style.removeProperty("transform");
      onComplete?.();
    }
  });

  return () => {
    controls.stop();
    translatedElement?.style.removeProperty("transform");
  };
}

function scrollBottomTop(element: HTMLElement) {
  return Math.max(0, element.scrollHeight - element.clientHeight);
}
