import * as React from "react";
import { VStack } from "@astryxdesign/core/VStack";
import { defaultRangeExtractor, useVirtualizer } from "@tanstack/react-virtual";
import * as stylex from "@stylexjs/stylex";

export function TaskRows({ children, scrollRef }: {
  children: React.ReactNode;
  scrollRef: React.RefObject<HTMLDivElement | null>;
}) {
  const rows = React.Children.toArray(children);
  const listRef = React.useRef<HTMLUListElement>(null);
  const [scrollMargin, setScrollMargin] = React.useState(0);
  const [focusedKey, setFocusedKey] = React.useState<React.Key | null>(null);
  const focusedIndex = rows.findIndex((row) => React.isValidElement(row) && row.key === focusedKey);

  React.useLayoutEffect(() => {
    const list = listRef.current;
    const scroller = scrollRef.current;
    if (!list || !scroller) return;
    const measure = () => setScrollMargin(list.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop);
    measure();
    const observer = new ResizeObserver(measure);
    // Earlier groups and notices can move this list without resizing its rows.
    for (let parent: HTMLElement | null = list; parent; parent = parent.parentElement) {
      observer.observe(parent);
      for (const sibling of parent.parentElement?.children ?? []) observer.observe(sibling);
      if (parent === scroller) break;
    }
    return () => observer.disconnect();
  }, [scrollRef]);

  // TanStack Virtual owns live measurements outside React Compiler memoization.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    getItemKey: (index) => (rows[index] as React.ReactElement).key!,
    // Estimated card height; actual heights include the token-backed row gap.
    estimateSize: () => 90,
    scrollMargin,
    overscan: 4,
    rangeExtractor: (range) => {
      const indexes = new Set(defaultRangeExtractor(range));
      // Keep group boundaries reachable by Tab, even when the group is offscreen.
      if (rows.length) { indexes.add(0); indexes.add(rows.length - 1); }
      if (focusedIndex >= 0) {
        for (let index = Math.max(0, focusedIndex - 1); index <= Math.min(rows.length - 1, focusedIndex + 1); index++) indexes.add(index);
      }
      return [...indexes].sort((a, b) => a - b);
    }
  });

  return (
    <VStack
      as="ul"
      ref={listRef}
      gap={0}
      className={stylex.props(styles.list).className}
      style={{ height: virtualizer.getTotalSize() }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocusedKey(null);
      }}
    >
      {virtualizer.getVirtualItems().map((item) => (
        <li
          key={item.key}
          ref={virtualizer.measureElement}
          data-index={item.index}
          aria-posinset={item.index + 1}
          aria-setsize={rows.length}
          onFocus={() => setFocusedKey(item.key)}
          {...stylex.props(styles.row)}
          style={{ transform: `translateY(${item.start - scrollMargin}px)` }}
        >
          {rows[item.index]}
        </li>
      ))}
    </VStack>
  );
}

const styles = stylex.create({
  // Absolute positioning is required for rows that are outside the rendered range.
  list: { position: "relative", flexShrink: 0, minWidth: 0, margin: 0, padding: 0, listStyle: "none" },
  row: { position: "absolute", top: 0, left: 0, width: "100%", boxSizing: "border-box", paddingBottom: "var(--spacing-1-5)", ":last-child": { paddingBottom: 0 }, listStyle: "none" }
});
