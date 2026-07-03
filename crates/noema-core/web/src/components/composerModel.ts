export function isComposerTextareaDisabled({ ready }: { ready: boolean }) {
  return !ready;
}

export function composerSubmitState({
  ready,
  pending,
  value
}: {
  ready: boolean;
  pending: boolean;
  value: string;
}) {
  return {
    disabled: !canSend({ ready, value }),
    label: pending ? "Sending message" : "Send message"
  };
}

export function canSend({ ready, value }: { ready: boolean; value: string }) {
  return ready && value.trim().length > 0;
}

export function shouldSubmitFromPointerDown({
  ready,
  value,
  button,
  isPrimary,
  pointerType
}: {
  ready: boolean;
  value: string;
  button: number;
  isPrimary: boolean;
  pointerType: string;
}) {
  return pointerType !== "mouse" && button === 0 && isPrimary && canSend({ ready, value });
}

export function composerTextareaProps() {
  return {
    rows: 1
  };
}
