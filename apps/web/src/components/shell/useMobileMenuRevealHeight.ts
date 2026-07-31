import React from "react";

const revealHeightProperty = "--shell-mobile-nav-reveal-height";

export function useMobileMenuRevealHeight(
  enabled: boolean,
  shellRootRef: React.RefObject<HTMLElement | null>
) {
  const sidebarRef = React.useRef<HTMLElement | null>(null);

  React.useLayoutEffect(() => {
    const root = shellRootRef.current;
    const sidebar = sidebarRef.current;
    if (!enabled || !root || !sidebar) {
      root?.style.removeProperty(revealHeightProperty);
      return;
    }

    let frame = 0;
    const measure = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        const nav = sidebar.querySelector<HTMLElement>("[data-slot='shell-sidebar-nav']");
        const items = sidebar.querySelector<HTMLElement>("[data-slot='shell-sidebar-items']");
        const lastItem = items?.lastElementChild;
        if (!nav || !items || !(lastItem instanceof HTMLElement)) {
          root.style.removeProperty(revealHeightProperty);
          return;
        }

        const navStyle = window.getComputedStyle(nav);
        const itemsStyle = window.getComputedStyle(items);
        const height = Math.ceil(
          lastItem.getBoundingClientRect().bottom - nav.getBoundingClientRect().top
            + items.scrollTop
            + pixels(navStyle.paddingBottom)
            + pixels(itemsStyle.paddingBottom)
        );
        root.style.setProperty(revealHeightProperty, `${Math.max(0, height)}px`);
      });
    };

    const resizeObserver = new ResizeObserver(measure);
    const observeMenu = () => {
      resizeObserver.disconnect();
      resizeObserver.observe(sidebar);
      const items = sidebar.querySelector<HTMLElement>("[data-slot='shell-sidebar-items']");
      if (!items) return;
      resizeObserver.observe(items);
      for (const item of items.children) {
        if (item instanceof HTMLElement) resizeObserver.observe(item);
      }
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
      root.style.removeProperty(revealHeightProperty);
    };
  }, [enabled, shellRootRef]);

  return sidebarRef;
}

function pixels(value: string) {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}
