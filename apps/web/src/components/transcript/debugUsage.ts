export type ProviderUsageDebug = {
  provider: string;
  model: string;
  phase: "initial" | "continuation" | string;
  responseIndex: number;
  outputIndex?: number;
  inputTokens: number;
  cachedInputTokens?: number;
  cacheHitRatio?: number;
  outputTokens: number;
  totalTokens: number;
};

export type ProviderUsageDebugRow = {
  label: string;
  value: string;
};

export function parseProviderUsageDebug(metadata: unknown): ProviderUsageDebug | null {
  if (!isRecord(metadata)) {
    return null;
  }
  const usage = metadata.provider_usage;
  if (!isRecord(usage)) {
    return null;
  }

  const provider = stringValue(usage.provider);
  const model = stringValue(usage.model);
  const phase = stringValue(usage.phase);
  const responseIndex = numberValue(usage.response_index);
  const outputIndex = optionalNumberValue(usage.output_index);
  const inputTokens = numberValue(usage.input_tokens);
  const outputTokens = numberValue(usage.output_tokens);
  const totalTokens = numberValue(usage.total_tokens);
  const cachedInputTokens = optionalNumberValue(usage.cached_input_tokens);
  const cacheHitRatio = optionalNumberValue(usage.cache_hit_ratio);

  if (
    provider === null ||
    model === null ||
    phase === null ||
    responseIndex === null ||
    outputIndex === false ||
    inputTokens === null ||
    outputTokens === null ||
    totalTokens === null ||
    cachedInputTokens === false ||
    cacheHitRatio === false
  ) {
    return null;
  }

  return {
    provider,
    model,
    phase,
    responseIndex,
    ...(outputIndex === undefined ? {} : { outputIndex }),
    inputTokens,
    ...(cachedInputTokens === undefined ? {} : { cachedInputTokens }),
    ...(cacheHitRatio === undefined ? {} : { cacheHitRatio }),
    outputTokens,
    totalTokens
  };
}

export function providerUsageDebugRows(debug: ProviderUsageDebug): ProviderUsageDebugRow[] {
  const rows: ProviderUsageDebugRow[] = [
    { label: "Provider", value: debug.provider },
    { label: "Model", value: debug.model },
    { label: "Phase", value: debug.phase },
    { label: "Response index", value: String(debug.responseIndex) }
  ];
  if (debug.outputIndex !== undefined) {
    rows.push({ label: "Output index", value: String(debug.outputIndex) });
  }
  rows.push(
    { label: "Input tokens", value: formatInteger(debug.inputTokens) },
    {
      label: "Cached input tokens",
      value: debug.cachedInputTokens === undefined ? "Unavailable" : formatInteger(debug.cachedInputTokens)
    }
  );
  if (debug.cacheHitRatio !== undefined) {
    rows.push({ label: "Cache hit ratio", value: formatCacheHitRatio(debug.cacheHitRatio) });
  }
  rows.push(
    { label: "Output tokens", value: formatInteger(debug.outputTokens) },
    { label: "Total tokens", value: formatInteger(debug.totalTokens) }
  );
  return rows;
}

export function formatCacheHitRatio(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "Unavailable";
  }
  return `${Math.round(value * 100)}%`;
}

function formatInteger(value: number): string {
  return new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 }).format(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() !== "" ? value : null;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function optionalNumberValue(value: unknown): number | undefined | false {
  if (value === undefined || value === null) {
    return undefined;
  }
  return typeof value === "number" && Number.isFinite(value) ? value : false;
}
