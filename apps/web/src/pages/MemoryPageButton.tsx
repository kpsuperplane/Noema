import * as stylex from "@stylexjs/stylex";
import { styles } from "@/pages/memoryPageStyles";

interface MemoryPageReference {
  id: string;
  path: string;
  title: string;
}

export function MemoryPageButton({
  page,
  selected,
  onSelect
}: {
  page: MemoryPageReference;
  selected: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      {...stylex.props(styles.pageButton, selected && styles.selectedPageButton)}
      onClick={() => onSelect(page.id)}
    >
      <span {...stylex.props(styles.pageTitle)}>{page.title}</span>
      <span {...stylex.props(styles.pagePath)}>{page.path}</span>
    </button>
  );
}
