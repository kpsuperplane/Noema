import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import * as stylex from "@stylexjs/stylex";
import {
  RuntimeDebugProfileDocument,
  type RuntimeDebugProfileQuery,
  type RuntimeDebugSpanCategory
} from "@/generated/graphql";
import type { RuntimeDebugScope } from "@/shared/types";
import { providerUsageDebugRows, type ProviderUsageDebug } from "./debugUsage";

type Profile = NonNullable<RuntimeDebugProfileQuery["runtimeDebugProfile"]>;
type Span = Profile["spans"][number];

export type RuntimeDebugFocus =
  | { kind: "provider"; phase?: string; responseIndex?: number; roundIndex?: number }
  | { kind: "tool"; correlationId: string };

export type RuntimeDebugTarget = {
  scope?: RuntimeDebugScope;
  focus?: RuntimeDebugFocus;
  legacyUsage: ProviderUsageDebug | null;
};

const categories: RuntimeDebugSpanCategory[] = ["PROVIDER", "TOOL", "RUNTIME", "PERSISTENCE"];
export function RuntimeDebugDialog({
  target,
  onOpenChange
}: {
  target: RuntimeDebugTarget | null;
  onOpenChange: (open: boolean) => void;
}) {
  const scope = target?.scope;
  const query = useQuery(RuntimeDebugProfileDocument, {
    variables: { input: scope ?? { kind: "CONVERSATION_TURN", scopeId: "" } },
    skip: !target || !scope,
    fetchPolicy: "network-only",
    notifyOnNetworkStatusChange: true
  });
  const profile = query.dataState === "complete" ? query.data?.runtimeDebugProfile : undefined;
  const unavailable = query.dataState === "complete" && query.data?.runtimeDebugProfile === null;
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const { startPolling, stopPolling } = query;

  React.useEffect(() => {
    if (profile?.status === "RUNNING") {
      startPolling(1_000);
      return () => stopPolling();
    }
    stopPolling();
    return undefined;
  }, [profile?.status, startPolling, stopPolling]);

  const focused = profile ? focusedSpan(profile.spans, target?.focus) : undefined;
  const selected = profile?.spans.find((span) => span.id === selectedId) ?? focused ?? profile?.spans[0] ?? null;
  const hasProfile = Boolean(profile?.spans.length);
  const title = scope?.kind === "TASK_RUN" ? "Agent run runtime" : "Turn runtime";

  return (
    <Dialog
      isOpen={Boolean(target)}
      onOpenChange={onOpenChange}
      purpose="info"
      width="min(900px, calc(100vw - var(--spacing-8)))"
      maxHeight="min(760px, calc(100dvh - var(--spacing-8)))"
      aria-label={title}
    >
      <Layout
        header={<DialogHeader title={title} onOpenChange={onOpenChange} />}
        content={
          <LayoutContent {...stylex.props(styles.scrollContent)}>
            <div {...stylex.props(styles.body)}>
              {query.loading && !profile ? <p {...stylex.props(styles.state)}>Loading runtime profile…</p> : null}
              {query.error ? (
                <div role="alert" {...stylex.props(styles.errorState)}>
                  <span>Runtime profile could not be loaded.</span>
                  <Button type="button" size="sm" variant="secondary" label="Try again" onClick={() => void query.refetch()} />
                </div>
              ) : null}
              {profile ? <ProfileSummary profile={profile} /> : null}
              {profile && hasProfile ? (
                <HStack gap={4} {...stylex.props(styles.inspector)}>
                  <RuntimeTimeline profile={profile} selectedId={selected?.id ?? null} onSelect={setSelectedId} />
                  {selected ? (
                    <VStack {...stylex.props(styles.inspectorDetails)}>
                      <SpanDetails span={selected} />
                    </VStack>
                  ) : null}
                </HStack>
              ) : null}
              {profile && !hasProfile ? (
                <p {...stylex.props(styles.state)}>Runtime profiling wasn’t captured for this turn or run.</p>
              ) : null}
              {unavailable ? (
                <p {...stylex.props(styles.state)}>Runtime profiling wasn’t captured for this turn or run.</p>
              ) : null}
              {target && !scope ? (
                <p {...stylex.props(styles.state)}>Runtime profiling wasn’t captured for this historical message.</p>
              ) : null}
              {!hasProfile && target?.legacyUsage ? <LegacyUsage debug={target.legacyUsage} /> : null}
            </div>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

function ProfileSummary({ profile }: { profile: Profile }) {
  return (
    <section aria-label="Runtime summary" {...stylex.props(styles.summary)}>
      <div>
        <strong {...stylex.props(styles.total)}>{formatDuration(profile.elapsedMilliseconds)}</strong>
        <span {...stylex.props(styles.status)}>{humanize(profile.status)}</span>
      </div>
      <div {...stylex.props(styles.breakdown)}>
        {categories.map((category) => {
          const duration = coveredDuration(
            profile.spans.filter((span) => span.category === category),
            profile.elapsedMilliseconds
          );
          return duration > 0 ? <span key={category}>{humanize(category)} {formatDuration(duration)}</span> : null;
        })}
        <span>Uninstrumented {formatDuration(profile.uninstrumentedMilliseconds)}</span>
      </div>
    </section>
  );
}

function RuntimeTimeline({
  profile,
  selectedId,
  onSelect
}: {
  profile: Profile;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const spans = [...profile.spans].sort((left, right) =>
    left.startOffsetMilliseconds - right.startOffsetMilliseconds);
  return (
    <VStack as="section" gap={2} aria-label="Runtime timeline">
      <p {...stylex.props(styles.state)}>Steps by start time. Steps can overlap.</p>
      <ol {...stylex.props(styles.timeline)}>
        {spans.map((span) => (
          <li key={span.id} {...stylex.props(styles.timelineItem)}>
            <Button
              type="button"
              variant="ghost"
              width="100%"
              label={span.name}
              aria-pressed={span.id === selectedId}
              onClick={() => onSelect(span.id)}
              {...stylex.props(styles.timelineButton, span.id === selectedId && styles.timelineSelected)}
            >
              <VStack as="span" gap={1}>
                <HStack as="span" gap={2} hAlign="between" vAlign="start">
                  <strong {...stylex.props(styles.stepName)}>{span.name}</strong>
                  <span {...stylex.props(styles.duration)}>{formatDuration(timelineDurationMilliseconds(span))}</span>
                </HStack>
                <HStack as="span" gap={2} wrap="wrap" {...stylex.props(styles.stepMetadata)}>
                  <span>{humanize(span.category)}</span>
                  <span>Start {formatDuration(span.startOffsetMilliseconds)}</span>
                  <span>{humanize(span.status)}</span>
                </HStack>
              </VStack>
            </Button>
          </li>
        ))}
      </ol>
    </VStack>
  );
}

function SpanDetails({ span }: { span: Span }) {
  const timelineDuration = timelineDurationMilliseconds(span);
  const rows: Array<[string, string]> = [
    ["Span", span.name],
    ["Start", formatDuration(span.startOffsetMilliseconds)],
    ["Timeline duration", formatDuration(timelineDuration)],
    ["Status", humanize(span.status)]
  ];
  if (Math.abs(timelineDuration - span.durationMilliseconds) >= 10) {
    rows.splice(3, 0, ["Monotonic duration", formatDuration(span.durationMilliseconds)]);
  }
  for (const [label, value] of [
    ["Provider", span.provider], ["Model", span.model], ["Phase", span.phase],
    ["Tool", span.toolName], ["Response", span.responseIndex], ["Round", span.roundIndex],
    ["Input tokens", span.inputTokens], ["Cached input", span.cachedInputTokens],
    ["Output tokens", span.outputTokens], ["Total tokens", span.totalTokens]
  ] as const) {
    if (value !== null) rows.push([label, typeof value === "number" ? formatInteger(value) : value]);
  }
  return <Details title="Selected span" rows={rows} />;
}

function LegacyUsage({ debug }: { debug: ProviderUsageDebug }) {
  return <Details title="Provider usage" rows={providerUsageDebugRows(debug).map((row) => [row.label, row.value])} />;
}

function Details({ title, rows }: { title: string; rows: Array<[string, string]> }) {
  return (
    <section aria-label={title} {...stylex.props(styles.detailsSection)}>
      <h3 {...stylex.props(styles.sectionTitle)}>{title}</h3>
      <dl {...stylex.props(styles.details)}>
        {rows.map(([label, value]) => (
          <div key={label} {...stylex.props(styles.detailRow)}>
            <dt {...stylex.props(styles.detailLabel)}>{label}</dt>
            <dd {...stylex.props(styles.detailValue)}>{value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function focusedSpan(spans: Span[], focus?: RuntimeDebugFocus): Span | undefined {
  if (!focus) return undefined;
  if (focus.kind === "tool") return spans.find((span) => span.correlationId === focus.correlationId);
  const providerSpans = spans.filter((span) => span.category === "PROVIDER");
  return providerSpans.find((span) =>
    (focus.phase === undefined || span.phase === focus.phase)
    && (focus.responseIndex === undefined || span.responseIndex === focus.responseIndex)
    && (focus.roundIndex === undefined || span.roundIndex === focus.roundIndex))
    ?? providerSpans.find((span) => focus.phase !== undefined && span.phase === focus.phase)
    ?? providerSpans[0];
}

function coveredDuration(spans: Span[], elapsed: number): number {
  const intervals = spans
    .map((span) => {
      const start = Math.max(0, Math.min(elapsed, span.startOffsetMilliseconds));
      const end = Math.max(start, Math.min(elapsed, start + timelineDurationMilliseconds(span)));
      return { start, end };
    })
    .filter((interval) => interval.end > interval.start)
    .sort((left, right) => left.start - right.start);
  if (intervals.length === 0) return 0;
  let covered = 0;
  let currentStart = intervals[0].start;
  let currentEnd = intervals[0].end;
  for (const interval of intervals.slice(1)) {
    if (interval.start <= currentEnd) {
      currentEnd = Math.max(currentEnd, interval.end);
    } else {
      covered += currentEnd - currentStart;
      currentStart = interval.start;
      currentEnd = interval.end;
    }
  }
  return covered + currentEnd - currentStart;
}

function timelineDurationMilliseconds(span: Span): number {
  if (!span.endedAt) return span.durationMilliseconds;
  const started = Date.parse(span.startedAt);
  const ended = Date.parse(span.endedAt);
  return Number.isFinite(started) && Number.isFinite(ended) && ended >= started
    ? ended - started
    : span.durationMilliseconds;
}

function formatDuration(milliseconds: number): string {
  if (milliseconds < 1_000) return `${Math.round(milliseconds)}ms`;
  if (milliseconds < 60_000) return `${(milliseconds / 1_000).toFixed(milliseconds < 10_000 ? 1 : 0)}s`;
  const minutes = Math.floor(milliseconds / 60_000);
  return `${minutes}m ${Math.round((milliseconds % 60_000) / 1_000)}s`;
}

function formatInteger(value: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(value);
}

function humanize(value: string): string {
  return value.toLowerCase().replaceAll("_", " ").replace(/^./, (character) => character.toUpperCase());
}

const styles = stylex.create({
  // Keep scrolling on a separate compositor layer from the dialog and its backdrop.
  scrollContent: { transform: "translateZ(0)" },
  body: { display: "grid", gap: "var(--spacing-4)" },
  state: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13 },
  errorState: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-3)", color: "var(--destructive)", fontSize: 13 },
  summary: { display: "grid", gap: "var(--spacing-2)" },
  total: { fontFamily: "var(--noema-font-mono)", fontSize: 24, color: "var(--foreground)" },
  status: { marginInlineStart: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: 12 },
  breakdown: { display: "flex", flexWrap: "wrap", gap: "var(--spacing-1-5) var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 12 },
  // The ordered list owns the continuous timeline rail and its step markers.
  timeline: { listStyle: "none", margin: "var(--spacing-0)", padding: "var(--spacing-0)" },
  timelineItem: {
    position: "relative", marginInlineStart: "var(--spacing-2)", paddingInlineStart: "var(--spacing-4)",
    paddingBottom: "var(--spacing-2)", borderInlineStartWidth: 1, borderInlineStartStyle: "solid",
    borderInlineStartColor: "var(--border)",
    ":last-child": { borderInlineStartColor: "transparent", paddingBottom: "var(--spacing-0)" },
    "::before": {
      content: '""', position: "absolute", insetInlineStart: "calc(-1 * var(--spacing-1) - 0.5px)",
      top: "var(--spacing-4)", width: "var(--spacing-2)", height: "var(--spacing-2)",
      borderRadius: "var(--radius-full)", backgroundColor: "var(--muted-foreground)"
    }
  },
  timelineButton: {
    display: "block", height: "auto", padding: "var(--spacing-2)", textAlign: "start",
    whiteSpace: "normal", borderRadius: "var(--radius-element)",
    color: "var(--foreground)"
  },
  timelineSelected: { backgroundColor: "var(--muted)", boxShadow: "inset 0 0 0 1px var(--border)" },
  stepName: { overflowWrap: "anywhere", minWidth: 0, fontWeight: 600 },
  duration: { flexShrink: 0, fontFamily: "var(--noema-font-mono)", fontVariantNumeric: "tabular-nums" },
  stepMetadata: { color: "var(--muted-foreground)", fontWeight: 400 },
  // The desktop inspector reserves about 480px for steps and 350px for details.
  inspector: {
    display: "grid", alignItems: "start",
    gridTemplateColumns: { default: "minmax(0, 1.4fr) minmax(0, 1fr)", "@media (max-width: 760px)": "minmax(0, 1fr)" }
  },
  inspectorDetails: {
    minWidth: 0,
    borderInlineStartWidth: { default: 1, "@media (max-width: 760px)": 0 },
    borderInlineStartStyle: "solid", borderInlineStartColor: "var(--border)",
    paddingInlineStart: { default: "var(--spacing-4)", "@media (max-width: 760px)": "var(--spacing-0)" },
    borderTopWidth: { default: 0, "@media (max-width: 760px)": 1 },
    borderTopStyle: "solid", borderTopColor: "var(--border)",
    paddingTop: { default: "var(--spacing-0)", "@media (max-width: 760px)": "var(--spacing-3)" }
  },
  detailsSection: { display: "grid", gap: "var(--spacing-2)" },
  sectionTitle: { margin: "var(--spacing-0)", color: "var(--foreground)", fontSize: 12, fontWeight: 600 },
  details: { display: "grid", gap: "var(--spacing-1-5)", margin: "var(--spacing-0)" },
  detailRow: { display: "grid", gridTemplateColumns: "120px minmax(0, 1fr)", gap: "var(--spacing-2)", fontSize: 12 },
  detailLabel: { color: "var(--muted-foreground)" },
  detailValue: { margin: "var(--spacing-0)", color: "var(--foreground)", overflowWrap: "anywhere" }
});
