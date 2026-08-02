import { Button } from "@astryxdesign/core/Button";
import { FileInput } from "@astryxdesign/core/FileInput";
import { FormLayout } from "@astryxdesign/core/FormLayout";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";

export type AdapterCredentialSetup = {
  credentialType: string;
  setupUrl: string;
  instructions: readonly string[];
  inputKind: string;
  fields: readonly { fieldId: string; label: string }[];
  documentMediaType?: string | null;
  redirectUri?: string | null;
  normalizationTransform?: AdapterCredentialTransform | null;
  requestAuthTransform?: AdapterCredentialTransform | null;
};

type AdapterCredentialTransform = {
  language: string;
  sourceDigest: string;
  source: string;
};

export type AdapterCredentialSubmission = {
  fieldValues: { fieldId: string; value: string }[];
  document: File | null;
};

export function AdapterCredentialSetupDialog({
  serviceName,
  setup,
  scopes,
  open,
  submitting,
  error,
  onOpenChange,
  onSubmit
}: {
  serviceName: string;
  setup: AdapterCredentialSetup | null | undefined;
  scopes: readonly string[];
  open: boolean;
  submitting: boolean;
  error?: string | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (submission: AdapterCredentialSubmission) => Promise<void>;
}) {
  const [values, setValues] = useState<Record<string, string>>({});
  const [document, setDocument] = useState<File | null>(null);

  if (!setup) return null;
  const isDocument = setup.inputKind === "document";
  const complete = isDocument
    ? document !== null
    : setup.fields.every((field) => Boolean(values[field.fieldId]));

  return (
    <Dialog
      isOpen={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) {
          setValues({});
          setDocument(null);
        }
        onOpenChange(nextOpen);
      }}
      purpose="form"
      width={660}
      maxHeight="min(780px, calc(100dvh - var(--spacing-8)))"
      aria-label={`Add ${serviceName} credentials`}
    >
      <Layout
        height="auto"
        header={<DialogHeader title={`Connect ${serviceName}`} subtitle={setup.credentialType} onOpenChange={onOpenChange} />}
        content={
          <LayoutContent>
            <VStack
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                if (!complete) return;
                void (async () => {
                  try {
                    await onSubmit({
                      fieldValues: setup.fields.map((field) => ({
                        fieldId: field.fieldId,
                        value: values[field.fieldId] ?? ""
                      })),
                      document
                    });
                    setValues({});
                    setDocument(null);
                  } catch {
                    // The owning surface renders its safe mutation error in the dialog.
                  }
                })();
              }}
            >
              <VStack gap={2}>
                <p {...stylex.props(styles.intro)}>
                  Create the exact reviewed credential below. Noema stores only the declared private fields.
                </p>
                <ol {...stylex.props(styles.instructions)}>
                  {setup.instructions.map((instruction, index) => <li key={`${index}-${instruction}`}>{instruction}</li>)}
                </ol>
                <a href={setup.setupUrl} target="_blank" rel="noreferrer" {...stylex.props(styles.link)}>
                  Open official credential setup
                </a>
                {setup.redirectUri ? (
                  <VStack gap={1}>
                    <strong {...stylex.props(styles.label)}>Authorized redirect URI</strong>
                    <span {...stylex.props(styles.muted)}>Copy this exact value into the provider client.</span>
                    <code {...stylex.props(styles.codeValue)}>{setup.redirectUri}</code>
                  </VStack>
                ) : null}
              </VStack>
              {isDocument ? (
                <FileInput
                  label={`${setup.credentialType} document`}
                  description={`Required ${setup.documentMediaType ?? "credential document"}; processed once and not retained. Maximum 128 KB.`}
                  value={document}
                  onChange={(file) => setDocument(Array.isArray(file) ? file[0] ?? null : file)}
                  accept={setup.documentMediaType === "application/json" ? "application/json,.json" : undefined}
                  maxSize={128 * 1024}
                  isRequired
                  isDisabled={submitting}
                  mode="dropzone"
                />
              ) : (
                <FormLayout>
                  {setup.fields.map((field, index) => (
                    <TextInput
                      key={field.fieldId}
                      label={field.label}
                      type="password"
                      value={values[field.fieldId] ?? ""}
                      isRequired
                      hasAutoFocus={index === 0}
                      isDisabled={submitting}
                      onChange={(value) => setValues((current) => ({ ...current, [field.fieldId]: value }))}
                    />
                  ))}
                </FormLayout>
              )}
              <details>
                <summary {...stylex.props(styles.summary)}>Technical disclosure</summary>
                <VStack gap={2} {...stylex.props(styles.technical)}>
                  <TechnicalSection title="Scopes" value={scopes.length ? scopes.join("\n") : "None"} />
                  {setup.normalizationTransform ? <TransformSection title="Credential normalization" transform={setup.normalizationTransform} /> : null}
                  {setup.requestAuthTransform ? <TransformSection title="Request authentication" transform={setup.requestAuthTransform} /> : null}
                </VStack>
              </details>
              {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
              <HStack gap={2} hAlign="end" wrap="wrap">
                <Button type="button" variant="secondary" label="Cancel" isDisabled={submitting} onClick={() => onOpenChange(false)} />
                <Button type="submit" label="Add connection" isLoading={submitting} isDisabled={submitting || !complete} />
              </HStack>
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

function TechnicalSection({ title, value }: { title: string; value: string }) {
  return <VStack gap={1}><strong {...stylex.props(styles.label)}>{title}</strong><pre {...stylex.props(styles.source)}>{value}</pre></VStack>;
}

function TransformSection({ title, transform }: { title: string; transform: AdapterCredentialTransform }) {
  return <TechnicalSection title={`${title} · ${transform.language} · ${transform.sourceDigest}`} value={transform.source} />;
}

const styles = stylex.create({
  intro: { margin: 0, color: "var(--foreground)", fontSize: 14, lineHeight: 1.5 },
  instructions: { margin: 0, paddingInlineStart: "var(--spacing-5)", color: "var(--foreground)", fontSize: 13, lineHeight: 1.6 },
  link: { width: "fit-content", color: "var(--text-accent)", fontSize: 13, fontWeight: 600 },
  label: { color: "var(--foreground)", fontSize: 12, fontWeight: 650 },
  muted: { color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.5 },
  codeValue: { padding: "var(--spacing-2)", borderRadius: "var(--radius-sm)", backgroundColor: "var(--noema-surface-subtle)", fontFamily: "var(--font-mono)", fontSize: 12, overflowWrap: "anywhere", userSelect: "all" },
  summary: { cursor: "pointer", color: "var(--foreground)", fontSize: 12, fontWeight: 600 },
  technical: { marginBlockStart: "var(--spacing-2)" },
  source: { maxHeight: 220, margin: 0, padding: "var(--spacing-2)", overflow: "auto", borderRadius: "var(--radius-sm)", backgroundColor: "var(--noema-surface-subtle)", color: "var(--foreground)", fontFamily: "var(--font-mono)", fontSize: 11, whiteSpace: "pre-wrap", overflowWrap: "anywhere" },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13 }
});
