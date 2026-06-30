export type KeyValueParseResult =
  | { value: Record<string, string>; error: null }
  | { value: null; error: string };

export function parseKeyValueLines(input: string): KeyValueParseResult {
  const value: Record<string, string> = {};
  const lines = input.split(/\r?\n/);
  for (const [index, rawLine] of lines.entries()) {
    const line = rawLine.trim();
    if (!line) continue;
    const equalsIndex = line.indexOf("=");
    if (equalsIndex <= 0) {
      return { value: null, error: `Line ${index + 1} must use KEY=value.` };
    }
    const key = line.slice(0, equalsIndex).trim();
    const parsedValue = line.slice(equalsIndex + 1).trim();
    if (!key) {
      return { value: null, error: `Line ${index + 1} must include a key.` };
    }
    if (Object.prototype.hasOwnProperty.call(value, key)) {
      return { value: null, error: `Duplicate key: ${key}` };
    }
    value[key] = parsedValue;
  }
  return { value, error: null };
}
