export type ChatDetailTarget = {
  type: "artifact";
  version: string;
};

export function artifactDetailTarget(version: string | null | undefined): ChatDetailTarget | null {
  const trimmed = version?.trim();
  return trimmed ? { type: "artifact", version: trimmed } : null;
}
