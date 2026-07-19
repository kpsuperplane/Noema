import React from "react";
import { Button } from "@astryxdesign/core/Button";
import { X } from "lucide-react";

export function ChatDetailCloseButton({ closeButtonRef, onClose }: { closeButtonRef: React.RefObject<HTMLButtonElement | null>; onClose: () => void }) {
  return <Button ref={closeButtonRef} type="button" variant="ghost" size="sm" label="Close detail" tooltip="Close detail" icon={<X aria-hidden="true" size={16} />} isIconOnly onClick={onClose} />;
}
