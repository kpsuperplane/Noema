import { HStack } from "@astryxdesign/core/HStack";
import { Spinner } from "@astryxdesign/core/Spinner";
import { VStack } from "@astryxdesign/core/VStack";
import { ChevronRight, Plug } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import { FaviconImage } from "@/components/FaviconImage";
import { ListCardButton } from "@/components/ListCardLink";

export function ServiceChoice({ name, description, hostname, disabled = false, loading = false, onClick }: {
  name: string;
  description: string;
  hostname: string;
  disabled?: boolean;
  loading?: boolean;
  onClick: () => void;
}) {
  return <ListCardButton xstyle={styles.choice} disabled={disabled || loading} aria-busy={loading} onClick={onClick}>
    <HStack as="span" gap={2} vAlign="start">
      <HStack as="span" hAlign="center" vAlign="center" {...stylex.props(styles.icon)}>
        <FaviconImage hostname={hostname} size="large" fallback={<Plug aria-hidden="true" {...stylex.props(styles.chevron)} />} />
      </HStack>
      <VStack as="span" gap={0.5} {...stylex.props(styles.copy)}>
        <strong {...stylex.props(styles.name)}>{name}</strong>
        <span {...stylex.props(styles.description)}>{description}</span>
      </VStack>
      <HStack as="span" hAlign="center" vAlign="center" {...stylex.props(styles.chevron)}>
        {loading ? <Spinner size="md" aria-label={`Setting up ${name}`} /> : <ChevronRight aria-hidden="true" size="100%" />}
      </HStack>
    </HStack>
  </ListCardButton>;
}

const styles = stylex.create({
  choice: { width: "100%", height: "100%", padding: "var(--spacing-3)" },
  icon: { width: "var(--spacing-8)", height: "var(--spacing-8)", flexShrink: 0 },
  copy: { flex: 1, minWidth: 0 },
  name: { color: "var(--foreground)", fontSize: "var(--text-body-size)", lineHeight: "var(--text-body-leading)" },
  description: { color: "var(--muted-foreground)", fontSize: "var(--text-supporting-size)", lineHeight: "var(--text-supporting-leading)" },
  chevron: { width: "var(--spacing-4)", height: "var(--spacing-4)", flexShrink: 0, marginTop: "var(--spacing-0-5)", color: "var(--muted-foreground)" }
});
