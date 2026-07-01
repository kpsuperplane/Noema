import { ArrowLeft, Loader2, Save, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";

export function ToolPermissionsFooter({
  canSave,
  saving,
  loading,
  autofilling,
  onAutofill,
  onSave,
  onBack
}: {
  canSave: boolean;
  saving: boolean;
  loading: boolean;
  autofilling: boolean;
  onAutofill: () => void;
  onSave: () => void;
  onBack?: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 border-t border-[var(--border-subtle)] px-6 py-4">
      {onBack ? (
        <Button type="button" variant="ghost" className="w-fit" onClick={onBack}>
          <ArrowLeft className="size-4" aria-hidden="true" />
          Back
        </Button>
      ) : (
        <span aria-hidden="true" />
      )}
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          type="button"
          variant="outline"
          className="w-fit"
          disabled={saving || loading || autofilling || !canSave}
          onClick={onAutofill}
        >
          {autofilling ? (
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          ) : (
            <Sparkles className="size-4" aria-hidden="true" />
          )}
          Autofill
        </Button>
        <Button
          type="button"
          className="w-fit"
          disabled={saving || loading || autofilling || !canSave}
          onClick={onSave}
        >
          {saving ? (
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          ) : (
            <Save className="size-4" aria-hidden="true" />
          )}
          Save
        </Button>
      </div>
    </div>
  );
}
