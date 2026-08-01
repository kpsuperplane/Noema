import { Button } from "@astryxdesign/core/Button";
import { useQuery } from "@apollo/client/react";
import { Bell } from "lucide-react";
import { PendingHumanInterventionsDocument } from "@/generated/graphql";
import { useWebPush } from "@/pwa/WebPushContext";
import { HumanInterventionCard } from "./HumanInterventionCard";

export function WebPushChatPrompt({
  activeTurn,
  conversationId
}: {
  activeTurn: boolean;
  conversationId: string | null;
}) {
  const webPush = useWebPush();
  const interventions = useQuery(PendingHumanInterventionsDocument, {
    variables: { conversationId: conversationId ?? undefined, first: 50 },
    skip: conversationId === null,
    fetchPolicy: "cache-and-network"
  });
  if (
    activeTurn ||
    !webPush.promptEligible ||
    !interventions.data ||
    interventions.loading ||
    interventions.data.pendingHumanInterventions.length > 0
  ) return null;

  return (
    <HumanInterventionCard
      label="Notifications"
      title="Know when Noema replies"
      description="Get alerts for primary chat replies and items that need you, even when this app is closed."
      error={webPush.error}
      dismissLabel="Dismiss notification prompt"
      onDismiss={webPush.dismissPrompt}
      actions={
        <Button
          type="button"
          variant="primary"
          size="sm"
          label="Enable notifications"
          icon={<Bell aria-hidden="true" size={14} />}
          clickAction={webPush.enable}
        />
      }
    />
  );
}
