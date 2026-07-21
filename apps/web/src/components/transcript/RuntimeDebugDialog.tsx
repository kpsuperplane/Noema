import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
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
  const selected = profile?.spans.find((span) => span.id === selectedId) ?? focused ?? null;
  const hasProfile = Boolean(profile?.spans.length);

  return (
    <Dialog
      isOpen={Boolean(target)}
      onOpenChange={onOpenChange}
      purpose="info"
      width="min(760px, calc(100vw - var(--spacing-4)))"
      maxHeight="min(760px, calc(100vh - var(--spacing-4)))"
      aria-label="Runtime debug profile"
    >
      <DialogHeader
        title="Debug"
        subtitle={scope?.kind === "TASK_RUN" ? "Agent run runtime" : "Turn runtime"}
        onOpenChange={onOpenChange}
      />
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
          <FlameChart profile={profile} selectedId={selected?.id ?? null} onSelect={setSelectedId} />
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
        {selected ? <SpanDetails span={selected} /> : target?.legacyUsage ? <LegacyUsage debug={target.legacyUsage} /> : null}
      </div>
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
          const duration = profile.spans
            .filter((span) => span.category === category)
            .reduce((total, span) => total + span.durationMilliseconds, 0);
          return duration > 0 ? <span key={category}>{humanize(category)} {formatDuration(duration)}</span> : null;
        })}
        <span>Uninstrumented {formatDuration(profile.uninstrumentedMilliseconds)}</span>
      </div>
    </section>
  );
}

function FlameChart({
  profile,
  selectedId,
  onSelect
}: {
  profile: Profile;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const elapsed = Math.max(profile.elapsedMilliseconds, 1);
  return (
    <section aria-label="Chronological runtime flame chart" {...stylex.props(styles.chart)}>
      <div {...stylex.props(styles.axis)}><span>0</span><span>{formatDuration(elapsed)}</span></div>
      {categories.map((category) => {
        const spans = profile.spans.filter((span) => span.category === category);
        if (spans.length === 0) return null;
        return (
          <div key={category} {...stylex.props(styles.lane)}>
            <span {...stylex.props(styles.laneLabel)}>{humanize(category)}</span>
            <div {...stylex.props(styles.track)}>
              {spans.map((span) => {
                const left = Math.min(100, (span.startOffsetMilliseconds / elapsed) * 100);
                const width = Math.max(0.8, Math.min(100 - left, (span.durationMilliseconds / elapsed) * 100));
                return (
                  <button
                    key={span.id}
                    type="button"
                    aria-label={`${span.name}, ${formatDuration(span.durationMilliseconds)}, ${humanize(span.status)}`}
                    aria-pressed={span.id === selectedId}
                    title={`${span.name} · ${formatDuration(span.durationMilliseconds)}`}
                    style={{ left: `${left}%`, width: `${width}%` }}
                    {...stylex.props(styles.bar, categoryStyle(category), span.id === selectedId && styles.barSelected)}
                    onClick={() => onSelect(span.id)}
                  >
                    <span>{span.name}</span>
                  </button>
                );
              })}
            </div>
          </div>
        );
      })}
    </section>
  );
}

function SpanDetails({ span }: { span: Span }) {
  const rows: Array<[string, string]> = [
    ["Span", span.name],
    ["Duration", formatDuration(span.durationMilliseconds)],
    ["Status", humanize(span.status)]
  ];
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

function categoryStyle(category: RuntimeDebugSpanCategory) {
  if (category === "PROVIDER") return styles.provider;
  if (category === "TOOL") return styles.tool;
  if (category === "PERSISTENCE") return styles.persistence;
  return styles.runtime;
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
  body: { display: "grid", gap: "var(--spacing-4)", padding: "var(--spacing-4)", overflowY: "auto" },
  state: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 },
  errorState: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-3)", color: "var(--destructive)", fontSize: 13 },
  summary: { display: "grid", gap: "var(--spacing-2)" },
  total: { fontFamily: "var(--noema-font-mono)", fontSize: 24, color: "var(--foreground)" },
  status: { marginInlineStart: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: 12 },
  breakdown: { display: "flex", flexWrap: "wrap", gap: "var(--spacing-1-5) var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 11 },
  chart: { display: "grid", gap: "var(--spacing-2)" },
  axis: { display: "flex", justifyContent: "space-between", paddingInlineStart: 92, color: "var(--muted-foreground)", fontFamily: "var(--noema-font-mono)", fontSize: 10 },
  lane: { display: "grid", gridTemplateColumns: "84px minmax(0, 1fr)", alignItems: "center", gap: "var(--spacing-2)" },
  laneLabel: { color: "var(--muted-foreground)", fontSize: 11 },
  track: { position: "relative", height: 30, borderRadius: 6, backgroundColor: "var(--muted)", overflow: "hidden" },
  bar: { position: "absolute", insetBlock: 3, minWidth: 6, borderWidth: 0, borderRadius: 4, paddingInline: "var(--spacing-1)", overflow: "hidden", color: "var(--foreground)", fontFamily: "inherit", fontSize: 10, lineHeight: "24px", textAlign: "start", whiteSpace: "nowrap", cursor: "pointer", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: -2 } },
  barSelected: { boxShadow: "inset 0 0 0 2px var(--foreground)" },
  provider: { backgroundColor: "var(--accent)" },
  tool: { backgroundColor: "var(--secondary)" },
  runtime: { backgroundColor: "var(--muted-foreground)" },
  persistence: { backgroundColor: "var(--clay-100)" },
  detailsSection: { display: "grid", gap: "var(--spacing-2)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border)", paddingTop: "var(--spacing-3)" },
  sectionTitle: { margin: 0, color: "var(--foreground)", fontSize: 12, fontWeight: 600 },
  details: { display: "grid", gap: "var(--spacing-1-5)", margin: 0 },
  detailRow: { display: "grid", gridTemplateColumns: "120px minmax(0, 1fr)", gap: "var(--spacing-2)", fontSize: 12 },
  detailLabel: { color: "var(--muted-foreground)" },
  detailValue: { margin: 0, color: "var(--foreground)", overflowWrap: "anywhere" }
});
