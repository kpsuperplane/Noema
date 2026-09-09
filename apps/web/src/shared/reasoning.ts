// Remove emphasis only when it wraps a whole reasoning section.
export function readableReasoningText(value: string): string {
  const bold = value.match(/^(\s*)(\*\*|__)(\S(?:[\s\S]*\S)?)\2(\s*)$/);
  return bold && !bold[3].includes(bold[2])
    ? `${bold[1]}${bold[3]}${bold[4]}`
    : value;
}
