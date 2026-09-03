import React from "react";

export const mobileMenuRevealHeightProperty = "--shell-mobile-nav-reveal-height";

export function useMobileMenuRevealHeight(
  enabled: boolean,
  shellRootRef: React.RefObject<HTMLElement | null>
) {
  const sidebarRef = React.useRef<HTMLElement | null>(null);

  React.useLayoutEffect(() => {
    const root = shellRootRef.current;
    const sidebar = sidebarRef.current;
    if (!enabled || !root || !sidebar) {
      root?.style.removeProperty(mobileMenuRevealHeightProperty);
      return;
    }

    let frame = 0;
    const measure = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        const nav = sidebar.querySelector<HTMLElement>("[data-slot='shell-sidebar-nav']");
        if (!nav) {
          root.style.removeProperty(mobileMenuRevealHeightProperty);
          return;
        }

        const height = Math.ceil(nav.getBoundingClientRect().height);
        root.style.setProperty(mobileMenuRevealHeightProperty, `${Math.max(0, height)}px`);
      });
    };

    const resizeObserver = new ResizeObserver(measure);
    const observeMenu = () => {
      resizeObserver.disconnect();
      resizeObserver.observe(sidebar);
      const nav = sidebar.querySelector<HTMLElement>("[data-slot='shell-sidebar-nav']");
      if (nav) resizeObserver.observe(nav);
    };
    const mutationObserver = new MutationObserver(() => {
      observeMenu();
      measure();
    });

    observeMenu();
    mutationObserver.observe(sidebar, {
      characterData: true,
      childList: true,
      subtree: true
    });
    measure();

    return () => {
      window.cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      root.style.removeProperty(mobileMenuRevealHeightProperty);
    };
  }, [enabled, shellRootRef]);

  return sidebarRef;
}
