import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

export function Composer({
  value,
  disabled,
  pending,
  placeholder,
  onChange,
  onSubmit
}: {
  value: string;
  disabled: boolean;
  pending: boolean;
  placeholder: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
}) {
  return (
    <form
      className="mx-auto grid w-[var(--chat-column-width)] grid-cols-[minmax(0,1fr)_auto] items-end gap-2.5 border-t border-[var(--border-subtle)] bg-background pt-3.5 max-[760px]:grid-cols-1"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <Textarea
        value={value}
        disabled={disabled}
        placeholder={placeholder}
        rows={3}
        onChange={(event) => onChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            onSubmit();
          }
        }}
      />
      <Button type="submit" disabled={disabled || !value.trim()}>
        {pending ? "Sending" : "Send"}
      </Button>
    </form>
  );
}
