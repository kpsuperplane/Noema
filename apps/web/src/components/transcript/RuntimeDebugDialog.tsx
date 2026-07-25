import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
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
const categoryPalette: Record<RuntimeDebugSpanCategory, readonly string[]> = {
  PROVIDER: ["#fbbf24", "#f59e0b", "#fde047", "#fb923c"],
  TOOL: ["#22d3ee", "#06b6d4", "#38bdf8", "#2dd4bf"],
  RUNTIME: ["#a78bfa", "#8b5cf6", "#c084fc", "#818cf8"],
  PERSISTENCE: ["#4ade80", "#22c55e", "#a3e635", "#34d399"]
};
const categoryCode: Record<RuntimeDebugSpanCategory, string> = {
  PROVIDER: "P",
  TOOL: "T",
  RUNTIME: "R",
  PERSISTENCE: "S"
};
const chartWidth = 1_000;
const plotX = 116;
const plotWidth = chartWidth - plotX;
const firstLaneY = 34;
const laneGap = 16;
const trackGap = 4;
const barHeight = 32;
const shortSpanWidth = 12;

type PackedSpan = {
  span: Span;
  spanIndex: number;
  trackIndex: number;
};

type FlameLane = {
  category: RuntimeDebugSpanCategory;
  spans: PackedSpan[];
  trackCount: number;
  y: number;
  height: number;
};

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
          <LayoutContent>
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
  const layout = categories.reduce<{ lanes: FlameLane[]; nextY: number }>((result, category) => {
    const spans = packLaneSpans(
      profile.spans.filter((span) => span.category === category),
      elapsed
    );
    if (spans.length === 0) return result;
    const trackCount = Math.max(...spans.map((span) => span.trackIndex)) + 1;
    const height = trackCount * barHeight + (trackCount - 1) * trackGap;
    return {
      lanes: [...result.lanes, { category, spans, trackCount, y: result.nextY, height }],
      nextY: result.nextY + height + laneGap
    };
  }, { lanes: [], nextY: firstLaneY });
  const chartHeight = layout.nextY - laneGap + 12;
  const lanes = layout.lanes;
  const clipPrefix = React.useId().replaceAll(":", "");
  const positioned = lanes.flatMap(({ category, spans, y }, laneIndex) => spans.map(({ span, spanIndex, trackIndex }) => {
    const anchorX = plotX
      + Math.min(1, Math.max(0, span.startOffsetMilliseconds / elapsed)) * plotWidth;
    const timelineDuration = timelineDurationMilliseconds(span);
    const naturalWidth = Math.max(0, (timelineDuration / elapsed) * plotWidth);
    const marker = naturalWidth < shortSpanWidth;
    const width = marker ? shortSpanWidth : Math.min(plotWidth, naturalWidth);
    const x = marker
      ? Math.min(plotX + plotWidth - width, Math.max(plotX, anchorX - width / 2))
      : Math.min(plotX + plotWidth - width, Math.max(plotX, anchorX));
    const code = `${categoryCode[category]}${spanIndex + 1}`;
    const duration = formatDuration(timelineDuration);
    const fullLabel = `${code} ${span.name} · ${duration}`;
    const compactLabel = `${code} · ${duration}`;
    const label = marker ? null : width >= estimatedTextWidth(fullLabel) ? fullLabel
      : width >= estimatedTextWidth(compactLabel) ? compactLabel
        : width >= estimatedTextWidth(code) ? code : null;
    return {
      span,
      category,
      code,
      color: categoryPalette[category][spanIndex % categoryPalette[category].length],
      anchorX,
      x,
      y: y + trackIndex * (barHeight + trackGap),
      width,
      marker,
      timelineDuration,
      label,
      clipId: `${clipPrefix}-${laneIndex}-${spanIndex}`
    };
  }));
  const keyedSpans = positioned.filter((item) => !item.label?.includes(item.span.name));
  const selectFromKeyboard = (event: React.KeyboardEvent<SVGGElement>, id: string) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onSelect(id);
  };
  return (
    <section aria-label="Chronological runtime flame chart" {...stylex.props(styles.chart)}>
      <div {...stylex.props(styles.legend)} aria-label="Span categories">
        {lanes.some((lane) => lane.trackCount > 1) ? (
          <span {...stylex.props(styles.overlapNote)}>Stacked traces overlap in time</span>
        ) : null}
        {lanes.map(({ category }) => (
          <span key={category} {...stylex.props(styles.legendItem)}>
            <i aria-hidden="true" style={{ backgroundColor: categoryPalette[category][0] }} {...stylex.props(styles.legendSwatch)} />
            {humanize(category)}
          </span>
        ))}
      </div>
      <div {...stylex.props(styles.chartViewport)}>
        <svg
          viewBox={`0 0 ${chartWidth} ${chartHeight}`}
          role="group"
          aria-label={`Runtime spans from zero to ${formatDuration(elapsed)}`}
          preserveAspectRatio="xMinYMin meet"
          {...stylex.props(styles.svg)}
        >
          <defs>
            {positioned.map((item) => (
              <clipPath key={item.clipId} id={item.clipId}>
                <rect x={item.x + 5} y={item.y} width={Math.max(0, item.width - 10)} height={barHeight} />
              </clipPath>
            ))}
          </defs>
          {lanes.map(({ category, spans, trackCount, y, height }) => (
            <g key={category}>
              <text x={0} y={y + height / 2} dominantBaseline="middle" {...stylex.props(styles.svgLaneLabel)}>
                  {humanize(category)} · {spans.length}
              </text>
              {Array.from({ length: trackCount }, (_, trackIndex) => (
                <rect
                  key={trackIndex}
                  x={plotX}
                  y={y + trackIndex * (barHeight + trackGap)}
                  width={plotWidth}
                  height={barHeight}
                  rx={4}
                  {...stylex.props(styles.svgTrack)}
                />
              ))}
            </g>
          ))}
          {[0, 0.25, 0.5, 0.75, 1].map((ratio) => {
            const x = plotX + plotWidth * ratio;
            return (
              <g key={ratio} aria-hidden="true">
                <line x1={x} x2={x} y1={22} y2={chartHeight - 10} {...stylex.props(styles.gridLine)} />
                <text x={x} y={14} textAnchor={ratio === 0 ? "start" : ratio === 1 ? "end" : "middle"} {...stylex.props(styles.tickLabel)}>
                  {formatDuration(elapsed * ratio)}
                </text>
              </g>
            );
          })}
          {positioned.map((item) => {
            const selected = item.span.id === selectedId;
            const statusLabel = humanize(item.span.status);
            return (
              <g
                key={item.span.id}
                role="button"
                tabIndex={0}
                aria-label={`${item.code}, ${item.span.name}, ${formatDuration(item.span.durationMilliseconds)}, ${statusLabel}`}
                aria-pressed={selected}
                onClick={() => onSelect(item.span.id)}
                onKeyDown={(event) => selectFromKeyboard(event, item.span.id)}
                {...stylex.props(styles.svgSpan)}
              >
                <title>{item.code} · {item.span.name} · {formatDuration(item.timelineDuration)} · {statusLabel}</title>
                {item.marker ? (
                  <>
                    <rect
                      x={item.x - 2}
                      y={item.y}
                      width={item.width + 4}
                      height={barHeight}
                      fill="transparent"
                    />
                    <line
                      x1={item.anchorX}
                      x2={item.anchorX}
                      y1={item.y + 3}
                      y2={item.y + barHeight - 3}
                      stroke={selected ? "#111827" : item.color}
                      strokeWidth={selected ? 6 : 4}
                      strokeLinecap="round"
                      vectorEffect="non-scaling-stroke"
                    />
                    <circle
                      cx={item.anchorX}
                      cy={item.y + 6}
                      r={selected ? 5 : 4}
                      fill={item.color}
                      stroke="#ffffff"
                      strokeWidth={1.5}
                      vectorEffect="non-scaling-stroke"
                    />
                  </>
                ) : (
                  <rect
                    x={item.x}
                    y={item.y}
                    width={item.width}
                    height={barHeight}
                    rx={3}
                    fill={item.color}
                    stroke={selected ? "#111827" : "#ffffff"}
                    strokeWidth={selected ? 3 : 1.5}
                    strokeDasharray={item.span.status === "COMPLETED" ? undefined : "5 3"}
                    vectorEffect="non-scaling-stroke"
                  />
                )}
                {item.label ? (
                  <text
                    x={item.x + 6}
                    y={item.y + barHeight / 2}
                    dominantBaseline="middle"
                    clipPath={`url(#${item.clipId})`}
                    {...stylex.props(styles.spanLabel)}
                  >
                    {item.label}
                  </text>
                ) : null}
              </g>
            );
          })}
        </svg>
      </div>
      {keyedSpans.length > 0 ? (
        <div {...stylex.props(styles.traceKey)}>
          <span {...stylex.props(styles.traceKeyLabel)}>Trace key</span>
          <div {...stylex.props(styles.traceItems)}>
            {keyedSpans.map((item) => (
              <button
                key={item.span.id}
                type="button"
                aria-pressed={item.span.id === selectedId}
                title={`${humanize(item.category)} · ${humanize(item.span.status)}`}
                {...stylex.props(styles.traceItem, item.span.id === selectedId && styles.traceItemSelected)}
                onClick={() => onSelect(item.span.id)}
              >
                <i aria-hidden="true" style={{ backgroundColor: item.color }} {...stylex.props(styles.traceSwatch)} />
                <strong>{item.code}</strong>
                <span>{item.span.name}</span>
                <small>
                  {formatDuration(item.timelineDuration)}
                  {item.span.status === "COMPLETED" ? "" : ` · ${humanize(item.span.status)}`}
                </small>
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </section>
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

function packLaneSpans(spans: Span[], elapsed: number): PackedSpan[] {
  const markerDuration = elapsed * (shortSpanWidth / plotWidth);
  const trackEnds: number[] = [];
  return spans
    .map((span, originalIndex) => ({ span, originalIndex }))
    .sort((left, right) => left.span.startOffsetMilliseconds - right.span.startOffsetMilliseconds
      || left.originalIndex - right.originalIndex)
    .map(({ span }, spanIndex) => {
      const start = span.startOffsetMilliseconds;
      const timelineDuration = timelineDurationMilliseconds(span);
      const marker = timelineDuration < markerDuration;
      const collisionStart = marker ? start - markerDuration / 2 : start;
      const collisionEnd = marker
        ? start + markerDuration / 2
        : start + timelineDuration;
      let trackIndex = trackEnds.findIndex((end) => end <= collisionStart);
      if (trackIndex === -1) {
        trackIndex = trackEnds.length;
        trackEnds.push(collisionEnd);
      } else {
        trackEnds[trackIndex] = collisionEnd;
      }
      return { span, spanIndex, trackIndex };
    });
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

function estimatedTextWidth(value: string): number {
  return value.length * 6.2 + 12;
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
  body: { display: "grid", gap: "var(--spacing-4)" },
  state: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 },
  errorState: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-3)", color: "var(--destructive)", fontSize: 13 },
  summary: { display: "grid", gap: "var(--spacing-2)" },
  total: { fontFamily: "var(--noema-font-mono)", fontSize: 24, color: "var(--foreground)" },
  status: { marginInlineStart: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: 12 },
  breakdown: { display: "flex", flexWrap: "wrap", gap: "var(--spacing-1-5) var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 11 },
  chart: { display: "grid", gap: "var(--spacing-2)" },
  legend: { display: "flex", flexWrap: "wrap", justifyContent: "flex-end", gap: "var(--spacing-1-5) var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 10 },
  overlapNote: { marginInlineEnd: "auto", color: "var(--muted-foreground)" },
  legendItem: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)" },
  legendSwatch: { width: 10, height: 10, borderRadius: 2, boxShadow: "inset 0 0 0 1px color-mix(in srgb, #111827 20%, transparent)" },
  chartViewport: { overflowX: "auto", paddingBottom: "var(--spacing-1)" },
  svg: { display: "block", width: "100%", minWidth: 680, height: "auto", overflow: "visible" },
  gridLine: { stroke: "var(--border)", strokeWidth: 1, strokeDasharray: "2 4", vectorEffect: "non-scaling-stroke" },
  tickLabel: { fill: "var(--muted-foreground)", fontFamily: "var(--noema-font-mono)", fontSize: 10 },
  svgLaneLabel: { fill: "var(--muted-foreground)", fontSize: 11 },
  svgTrack: { fill: "color-mix(in srgb, var(--muted) 72%, var(--background))", stroke: "var(--border)", strokeWidth: 1, vectorEffect: "non-scaling-stroke" },
  svgSpan: { cursor: "pointer", outline: "none", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  spanLabel: { fill: "#111827", fontFamily: "var(--noema-font-mono)", fontSize: 10, fontWeight: 600, pointerEvents: "none" },
  traceKey: { display: "grid", gridTemplateColumns: "80px minmax(0, 1fr)", alignItems: "start", gap: "var(--spacing-2)", paddingTop: "var(--spacing-1)" },
  traceKeyLabel: { color: "var(--muted-foreground)", fontSize: 10, lineHeight: "24px" },
  traceItems: { display: "flex", flexWrap: "wrap", gap: "var(--spacing-1)" },
  traceItem: { display: "inline-flex", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 4, paddingBlock: "var(--spacing-1)", paddingInline: "var(--spacing-1-5)", backgroundColor: "transparent", color: "var(--foreground)", fontFamily: "inherit", fontSize: 10, lineHeight: 1.3, cursor: "pointer", ":hover": { backgroundColor: "var(--muted)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 1 } },
  traceItemSelected: { borderColor: "var(--foreground)", backgroundColor: "var(--muted)" },
  traceSwatch: { flex: "0 0 auto", width: 8, height: 14, borderRadius: 2, boxShadow: "inset 0 0 0 1px color-mix(in srgb, #111827 20%, transparent)" },
  detailsSection: { display: "grid", gap: "var(--spacing-2)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border)", paddingTop: "var(--spacing-3)" },
  sectionTitle: { margin: 0, color: "var(--foreground)", fontSize: 12, fontWeight: 600 },
  details: { display: "grid", gap: "var(--spacing-1-5)", margin: 0 },
  detailRow: { display: "grid", gridTemplateColumns: "120px minmax(0, 1fr)", gap: "var(--spacing-2)", fontSize: 12 },
  detailLabel: { color: "var(--muted-foreground)" },
  detailValue: { margin: 0, color: "var(--foreground)", overflowWrap: "anywhere" }
});
