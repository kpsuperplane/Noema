import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";

export function WorkHistoryLoadMore({ label, loading, error = false, onClick }: { label: string; loading: boolean; error?: boolean; onClick: () => void }) {
  return <div {...stylex.props(styles.root)}><Button type="button" size="sm" variant="ghost" label={error ? `Retry ${label.toLowerCase()}` : label} isLoading={loading} isDisabled={loading} onClick={onClick} />{error ? <span role="alert">That page could not be loaded.</span> : null}</div>;
}

const styles = stylex.create({ root: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: 7, color: "var(--destructive)", fontSize: 11 } });
