import { ArrowLeft, Save, Sparkles } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";

export function ToolPermissionsFooter({
  canSave,
  saving,
  loading,
  autofilling,
  onAutofill,
  onSave,
  onBack
}: {
  canSave: boolean;
  saving: boolean;
  loading: boolean;
  autofilling: boolean;
  onAutofill: () => void;
  onSave: () => void;
  onBack?: () => void;
}) {
  return (
    <div {...stylex.props(styles.footer)}>
      {onBack ? (
        <Button
          type="button"
          variant="ghost"
          label="Back"
          icon={<ArrowLeft {...stylex.props(styles.icon)} aria-hidden="true" />}
          {...stylex.props(styles.fitButton)}
          onClick={onBack}
        />
      ) : (
        <span aria-hidden="true" />
      )}
      <div {...stylex.props(styles.actions)}>
        <Button
          type="button"
          variant="secondary"
          label="Autofill"
          icon={!autofilling ? <Sparkles {...stylex.props(styles.icon)} aria-hidden="true" /> : undefined}
          {...stylex.props(styles.fitButton)}
          isDisabled={saving || loading || autofilling || !canSave}
          isLoading={autofilling}
          onClick={onAutofill}
        />
        <Button
          type="button"
          label="Save"
          icon={!saving ? <Save {...stylex.props(styles.icon)} aria-hidden="true" /> : undefined}
          {...stylex.props(styles.fitButton)}
          isDisabled={saving || loading || autofilling || !canSave}
          isLoading={saving}
          onClick={onSave}
        />
      </div>
    </div>
  );
}

const styles = stylex.create({
  footer: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingBlock: 16,
    paddingInline: 24
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: 8
  },
  fitButton: {
    width: "fit-content"
  },
  icon: {
    width: 16,
    height: 16
  }
});
