export function MemoryDetailRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-0.5">
      <dt className="font-mono text-[10px] tracking-[0.08em] text-[var(--text-faint)] uppercase">{label}</dt>
      <dd className="m-0 max-h-40 overflow-auto whitespace-pre-wrap break-words text-[13px] text-muted-foreground">
        {value}
      </dd>
    </div>
  );
}
