import * as React from "react";
import {
  ArrowRight,
  Check,
  MessageSquareText,
  ShieldCheck,
  UserRoundCheck,
  Zap
} from "lucide-react";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

export type CapabilityDataSharingPolicy = "allow_automatically" | "review_every_call";
export type CapabilityUnsafeActionPolicy = "always_ask" | "reviewer_may_approve" | "never_ask";

type CapabilityPolicy = {
  dataSharingPolicy: CapabilityDataSharingPolicy;
  unsafeActionPolicy: CapabilityUnsafeActionPolicy;
};

export function CapabilityPolicyChoices({
  serviceName,
  step = "all",
  hasAutoFocus = false,
  dataSharingPolicy,
  unsafeActionPolicy,
  onChange
}: CapabilityPolicy & {
  serviceName: string;
  step?: "all" | "sharing" | "unsafe_actions";
  hasAutoFocus?: boolean;
  onChange: (policy: CapabilityPolicy) => void;
}) {
  const showSharing = step === "all" || step === "sharing";
  const showUnsafeActions = step === "all" || step === "unsafe_actions";
  const setSharing = (next: CapabilityDataSharingPolicy) => onChange({
    dataSharingPolicy: next,
    unsafeActionPolicy: next === "review_every_call" && unsafeActionPolicy === "never_ask"
      ? "reviewer_may_approve"
      : unsafeActionPolicy
  });

  return (
    <VStack gap={4}>
      {showSharing ? (
        <VStack gap={2}>
          <VStack gap={1}>
            <h2 {...stylex.props(styles.heading)}>Share personal information with {serviceName}?</h2>
            <p {...stylex.props(styles.muted)}>Choose how Noema shares relevant conversation details.</p>
          </VStack>
          <VStack gap={2}>
            <PolicyChoiceCard
              hasAutoFocus={hasAutoFocus}
              selected={dataSharingPolicy === "allow_automatically"}
              title="Share when needed"
              icon={<MessageSquareText aria-hidden="true" size={18} />}
              steps={["Relevant details", "Tool runs"]}
              onClick={() => setSharing("allow_automatically")}
            />
            <PolicyChoiceCard
              selected={dataSharingPolicy === "review_every_call"}
              title="Review every time"
              icon={<ShieldCheck aria-hidden="true" size={18} />}
              steps={["Relevant details", "Approval check", "Tool runs"]}
              onClick={() => setSharing("review_every_call")}
            />
          </VStack>
        </VStack>
      ) : null}
      {showUnsafeActions ? (
        <VStack
          gap={2}
          {...stylex.props(step === "all" ? styles.dividedPolicyGroup : undefined)}
        >
          <VStack gap={1}>
            <h2 {...stylex.props(styles.heading)}>Who approves risky calls?</h2>
            <p {...stylex.props(styles.muted)}>
              {dataSharingPolicy === "review_every_call"
                ? "This applies to every call because sharing always requires review."
                : "Risky calls can change, delete, or send information."}
            </p>
          </VStack>
          <VStack gap={2}>
            <PolicyChoiceCard
              hasAutoFocus={hasAutoFocus && !showSharing}
              selected={unsafeActionPolicy === "always_ask"}
              title="Always me"
              icon={<UserRoundCheck aria-hidden="true" size={18} />}
              steps={["Risky call", "You approve", "Runs"]}
              onClick={() => onChange({ dataSharingPolicy, unsafeActionPolicy: "always_ask" })}
            />
            <PolicyChoiceCard
              selected={unsafeActionPolicy === "reviewer_may_approve"}
              title="Noema first"
              icon={<ShieldCheck aria-hidden="true" size={18} />}
              steps={["Risky call", "Noema checks", "You if needed"]}
              onClick={() => onChange({ dataSharingPolicy, unsafeActionPolicy: "reviewer_may_approve" })}
            />
            <PolicyChoiceCard
              selected={unsafeActionPolicy === "never_ask"}
              title="Run automatically"
              note="Not recommended"
              icon={<Zap aria-hidden="true" size={18} />}
              steps={["Risky call", "Runs"]}
              disabled={dataSharingPolicy === "review_every_call"}
              disabledReason="Choose “Share when needed” first."
              onClick={() => onChange({ dataSharingPolicy, unsafeActionPolicy: "never_ask" })}
            />
          </VStack>
        </VStack>
      ) : null}
    </VStack>
  );
}

function PolicyChoiceCard({
  hasAutoFocus = false,
  selected,
  title,
  icon,
  steps,
  note,
  disabled = false,
  disabledReason,
  onClick
}: {
  hasAutoFocus?: boolean;
  selected: boolean;
  title: string;
  icon: React.ReactNode;
  steps: readonly string[];
  note?: string;
  disabled?: boolean;
  disabledReason?: string;
  onClick: () => void;
}) {
  return (
    <div>
      <button
        type="button"
        data-autofocus={hasAutoFocus || undefined}
        disabled={disabled}
        aria-pressed={selected}
        {...stylex.props(styles.choice, selected && styles.choiceSelected)}
        onClick={onClick}
      >
        <HStack
          as="span"
          gap={2}
          hAlign="between"
          vAlign="center"
        >
          <HStack as="span" gap={2} vAlign="center" {...stylex.props(styles.choiceHeading)}>
            <span {...stylex.props(styles.choiceIcon, selected && styles.choiceIconSelected)}>{icon}</span>
            <span {...stylex.props(styles.choiceTitle, selected && styles.choiceTitleSelected)}>{title}</span>
          </HStack>
          <HStack as="span" gap={2} vAlign="center" {...stylex.props(styles.choiceMeta)}>
            {note ? <span {...stylex.props(styles.choiceNote)}>{note}</span> : null}
            {selected ? <Check aria-hidden="true" {...stylex.props(styles.choiceCheck, styles.selectionMarker)} /> : null}
          </HStack>
        </HStack>
        <HStack as="span" gap={1} wrap="wrap" vAlign="center">
          {steps.map((pathStep, index) => (
            <React.Fragment key={pathStep}>
              {index > 0 ? <ArrowRight aria-hidden="true" {...stylex.props(styles.choiceArrow)} /> : null}
              <span {...stylex.props(styles.choiceStep)}>{pathStep}</span>
            </React.Fragment>
          ))}
        </HStack>
      </button>
      {disabled && disabledReason ? <p {...stylex.props(styles.disabledReason)}>{disabledReason}</p> : null}
    </div>
  );
}

const styles = stylex.create({
  dividedPolicyGroup: { paddingTop: "var(--spacing-4)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)" },
  heading: { margin: "var(--spacing-0)", fontFamily: "var(--font-heading)", fontSize: 16, fontWeight: 600 },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 12 },
  choice: { display: "grid", width: "100%", gap: "var(--spacing-2)", padding: "var(--spacing-3)", textAlign: "left", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 8, color: "var(--foreground)", backgroundColor: "var(--surface-raised)", cursor: "pointer", transitionProperty: "border-color, box-shadow", transitionDuration: "var(--motion-spring-micro-duration)", transitionTimingFunction: "var(--motion-spring-critical-easing)", ":disabled": { cursor: "not-allowed", opacity: 0.5 } },
  choiceSelected: { borderColor: "var(--primary)", boxShadow: "inset 0 0 0 1px var(--primary)" },
  choiceHeading: { minWidth: 0 },
  choiceIcon: { display: "inline-flex", width: 18, height: 18, flexShrink: 0, color: "var(--muted-foreground)" },
  choiceIconSelected: { color: "var(--primary)" },
  choiceTitle: { fontSize: 14, fontWeight: 600, lineHeight: 1.4 },
  choiceTitleSelected: { color: "var(--primary)" },
  choiceMeta: { flexShrink: 0 },
  choiceNote: { fontSize: 11, fontWeight: 600, lineHeight: 1.3, color: "var(--destructive)" },
  choiceCheck: { width: 16, height: 16, color: "var(--primary)" },
  selectionMarker: { animationName: stylex.keyframes({ from: { opacity: 0, transform: "scale(0.6)" }, to: { opacity: 1, transform: "scale(1)" } }), animationDuration: "var(--motion-spring-micro-duration)", animationTimingFunction: "var(--motion-spring-critical-easing)" },
  choiceStep: { fontSize: 12, fontWeight: 500, lineHeight: 1.4, color: "var(--muted-foreground)" },
  choiceArrow: { width: 12, height: 12, flexShrink: 0, color: "var(--noema-text-faint)" },
  disabledReason: { margin: "var(--spacing-1-5) var(--spacing-1) 0", fontSize: 12, lineHeight: 1.4, color: "var(--muted-foreground)" }
});
