import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Plus, Trash2 } from "lucide-react";
import type { OwnerExtractorDraft, ToolPermissionDraft } from "./McpToolPermissionsModal";

const extractorSourceOptions = [
  "arguments",
  "structured_content",
  "metadata",
  "resource_uri",
  "built_in_adapter"
] as const;
const selectorKindOptions = ["email", "phone", "domain"] as const;

export function McpToolOwnerResolutionFields({
  draft,
  onChange
}: {
  draft: ToolPermissionDraft;
  onChange: (updater: (current: ToolPermissionDraft) => ToolPermissionDraft) => void;
}) {
  return (
    <section {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.headerText)}>
          <h4 {...stylex.props(styles.title)}>Owner resolution</h4>
          <p {...stylex.props(styles.mutedText)}>
            Mixed tools need a deterministic field that resolves an email, phone, or domain owner.
          </p>
        </div>
        <Button
          type="button"
          variant="secondary"
          label="Add extractor"
          icon={<Plus {...stylex.props(styles.icon)} aria-hidden="true" />}
          {...stylex.props(styles.fitButton)}
          onClick={() =>
            onChange((current) => ({
              ...current,
              ownerExtractors: [
                ...current.ownerExtractors,
                {
                  id: `extractor:${Date.now()}`,
                  source: "arguments",
                  selectorKind: "email",
                  path: ""
                }
              ]
            }))
          }
        />
      </div>
      {draft.ownerExtractors.length === 0 ? (
        <p {...stylex.props(styles.mutedText)}>
          No owner extractor configured. Mixed tools stay blocked until one is added.
        </p>
      ) : null}
      {draft.ownerExtractors.map((extractor) => (
        <div key={extractor.id} {...stylex.props(styles.extractorRow)}>
          <SelectField
            label="Source"
            value={extractor.source}
            options={extractorSourceOptions}
            onChange={(source) => updateExtractor(onChange, extractor.id, { source })}
          />
          <SelectField
            label="Identity"
            value={extractor.selectorKind}
            options={selectorKindOptions}
            onChange={(selectorKind) => updateExtractor(onChange, extractor.id, { selectorKind })}
          />
          <TextField
            label="Path"
            value={extractor.path}
            onChange={(path) => updateExtractor(onChange, extractor.id, { path })}
          />
          <Button
            type="button"
            variant="ghost"
            label="Remove owner extractor"
            icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
            isIconOnly
          {...stylex.props(styles.selfEnd)}
            onClick={() =>
              onChange((current) => ({
                ...current,
                ownerExtractors: current.ownerExtractors.filter((item) => item.id !== extractor.id)
              }))
            }
          />
        </div>
      ))}
    </section>
  );
}

function updateExtractor(
  onChange: (updater: (current: ToolPermissionDraft) => ToolPermissionDraft) => void,
  id: string,
  patch: Partial<Omit<OwnerExtractorDraft, "id">>
) {
  onChange((current) => ({
    ...current,
    ownerExtractors: current.ownerExtractors.map((extractor) =>
      extractor.id === id ? { ...extractor, ...patch } : extractor
    )
  }));
}

function SelectField<T extends readonly string[]>({
  label,
  value,
  options,
  onChange
}: {
  label: string;
  value: string;
  options: T;
  onChange: (value: T[number]) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span>{label}</span>
      <select
        {...stylex.props(styles.select)}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value as T[number])}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    </label>
  );
}

function TextField({
  label,
  value,
  onChange
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span>{label}</span>
      <input
        {...stylex.props(styles.input)}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}

const styles = stylex.create({
  section: {
    display: "grid",
    gap: 8
  },
  header: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8
  },
  headerText: {
    display: "grid",
    gap: 4
  },
  title: {
    margin: 0,
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  extractorRow: {
    display: "grid",
    gap: 8,
    gridTemplateColumns: "1fr 1fr 2fr auto",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    padding: 12,
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr"
    }
  },
  field: {
    display: "grid",
    gap: 4,
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  input: {
    height: 36,
    minWidth: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    paddingInline: 12,
    fontSize: 14,
    fontWeight: 400,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  select: {
    height: 36,
    minWidth: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingInline: 8,
    fontSize: 14,
    fontWeight: 400,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  fitButton: {
    width: "fit-content"
  },
  selfEnd: {
    alignSelf: "end"
  },
  icon: {
    width: 16,
    height: 16
  }
});
