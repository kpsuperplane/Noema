import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { CheckboxList, CheckboxListItem } from "@astryxdesign/core/CheckboxList";
import { Divider } from "@astryxdesign/core/Divider";
import { Heading } from "@astryxdesign/core/Heading";
import { RadioList, RadioListItem } from "@astryxdesign/core/RadioList";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { TextArea } from "@astryxdesign/core/TextArea";
import { TextInput } from "@astryxdesign/core/TextInput";
import type { A2UIActionSubmission, TurnTranscriptItem } from "@/shared/types";

type A2UISurfaceItem = Extract<TurnTranscriptItem, { kind: "a2ui_surface" }>;
type JSONObject = Record<string, unknown>;
type DynamicValue = unknown;

type A2UISnapshot = {
  surface_id: string;
  send_data_model: boolean;
  components: Record<string, JSONObject>;
  data_model: unknown;
  actions: Array<{
    source_component_id: string;
    name: string;
    context?: unknown;
  }>;
};

export function A2UISurface({
  item,
  disabled,
  onSubmit
}: {
  item: A2UISurfaceItem;
  disabled: boolean;
  onSubmit?: (action: A2UIActionSubmission) => void;
}) {
  const snapshot = parseSnapshot(item.snapshot);
  if (!snapshot) {
    return <Text type="supporting" color="secondary">This A2UI surface could not be displayed.</Text>;
  }
  return (
    <A2UISurfaceContent
      key={`${item.interaction_id ?? item.id}:${item.surface_id}:${item.revision}`}
      item={item}
      snapshot={snapshot}
      disabled={disabled}
      onSubmit={onSubmit}
    />
  );
}

function A2UISurfaceContent({
  item,
  snapshot,
  disabled,
  onSubmit
}: {
  item: A2UISurfaceItem;
  snapshot: A2UISnapshot;
  disabled: boolean;
  onSubmit?: (action: A2UIActionSubmission) => void;
}) {
  const [dataModel, setDataModel] = React.useState<unknown>(() => snapshot.data_model);
  const [localValues, setLocalValues] = React.useState<Record<string, unknown>>({});

  if (!snapshot.components.root) {
    return <Text type="supporting" color="secondary">This A2UI surface has no displayable root.</Text>;
  }

  const interactive =
    !disabled &&
    item.lifecycle === "pending" &&
    item.has_actions &&
    item.interaction_id !== null &&
    item.interaction_revision !== null &&
    Boolean(onSubmit);

  const updateDynamic = (componentId: string, dynamic: DynamicValue, value: unknown) => {
    const path = bindingPath(dynamic);
    if (path !== null) {
      setDataModel((current: unknown) => setPointer(current, path, value));
      return;
    }
    setLocalValues((current) => ({ ...current, [componentId]: value }));
  };

  const valueFor = (componentId: string, dynamic: DynamicValue) =>
    Object.prototype.hasOwnProperty.call(localValues, componentId)
      ? localValues[componentId]
      : resolveDynamic(dynamic, dataModel);

  const submit = (componentId: string, actionName: string) => {
    if (!interactive || !onSubmit || item.interaction_id === null || item.interaction_revision === null) {
      return;
    }
    const action = snapshot.actions.find(
      (candidate) =>
        candidate.source_component_id === componentId && candidate.name === actionName
    );
    if (!action) {
      return;
    }
    const context = resolveActionContext(action.context, dataModel);
    onSubmit({
      interaction_id: item.interaction_id,
      expected_revision: item.interaction_revision,
      surface_id: item.surface_id,
      source_component_id: componentId,
      action_name: actionName,
      context,
      data_model: snapshot.send_data_model ? dataModel : undefined
    });
  };

  const renderComponent = (componentId: string, ancestors: Set<string>): React.ReactNode => {
    if (ancestors.has(componentId)) {
      return null;
    }
    const component = snapshot.components[componentId];
    if (!component || typeof component.component !== "string") {
      return null;
    }
    const nextAncestors = new Set(ancestors).add(componentId);
    const children = stringArray(component.children);
    const renderChildren = () => children.map((childId) => (
      <React.Fragment key={childId}>{renderComponent(childId, nextAncestors)}</React.Fragment>
    ));

    switch (component.component) {
      case "Text": {
        const content = stringValue(resolveDynamic(component.text, dataModel));
        const headingLevel = headingLevelFor(component.variant);
        return headingLevel ? <Heading level={headingLevel}>{content}</Heading> : (
          <Text type={component.variant === "caption" ? "supporting" : "body"}>{content}</Text>
        );
      }
      case "Row":
        return (
          <HStack gap={2} wrap="wrap" justify={stackJustify(component.justify)} align={stackAlign(component.align)}>
            {renderChildren()}
          </HStack>
        );
      case "Column":
        return (
          <VStack gap={2} justify={stackJustify(component.justify)} align={stackAlign(component.align)}>
            {renderChildren()}
          </VStack>
        );
      case "Card": {
        const childId = stringValue(component.child);
        return <Card padding={3}>{renderComponent(childId, nextAncestors)}</Card>;
      }
      case "Divider":
        return <Divider orientation={component.axis === "vertical" ? "vertical" : "horizontal"} />;
      case "Button": {
        const childId = stringValue(component.child);
        const label = textContent(snapshot.components, childId, dataModel) || "Continue";
        const event = isObject(component.action) && isObject(component.action.event)
          ? component.action.event
          : null;
        const actionName = event && typeof event.name === "string" ? event.name : "";
        return (
          <Button
            label={label}
            size="sm"
            variant={buttonVariant(component.variant)}
            isDisabled={!interactive || !actionName}
            onClick={() => submit(componentId, actionName)}
          />
        );
      }
      case "TextField": {
        const label = stringValue(resolveDynamic(component.label, dataModel));
        const value = stringValue(valueFor(componentId, component.value ?? ""));
        const common = {
          label,
          value,
          size: "sm" as const,
          isDisabled: !interactive || !snapshot.send_data_model || bindingPath(component.value) === null,
          onChange: (next: string) => updateDynamic(componentId, component.value ?? "", next)
        };
        return component.variant === "longText" ? (
          <TextArea {...common} rows={3} />
        ) : (
          <TextInput
            {...common}
            type={component.variant === "obscured" ? "password" : "text"}
          />
        );
      }
      case "CheckBox": {
        const label = stringValue(resolveDynamic(component.label, dataModel));
        return (
          <CheckboxInput
            label={label}
            value={Boolean(valueFor(componentId, component.value))}
            isDisabled={!interactive || !snapshot.send_data_model || bindingPath(component.value) === null}
            onChange={(next) => updateDynamic(componentId, component.value, next)}
          />
        );
      }
      case "ChoicePicker": {
        const label = stringValue(resolveDynamic(component.label ?? "Choose an option", dataModel));
        const options = choiceOptions(component.options, dataModel);
        const selected = stringArray(valueFor(componentId, component.value));
        if (component.variant === "multipleSelection") {
          return (
            <CheckboxList
              label={label}
              value={selected}
              isDisabled={!interactive || !snapshot.send_data_model || bindingPath(component.value) === null}
              onChange={(next) => updateDynamic(componentId, component.value, next)}
            >
              {options.map((option) => (
                <CheckboxListItem key={option.value} label={option.label} value={option.value} />
              ))}
            </CheckboxList>
          );
        }
        return (
          <RadioList
            label={label}
            value={selected[0] ?? ""}
            isDisabled={!interactive || !snapshot.send_data_model || bindingPath(component.value) === null}
            onChange={(next) => updateDynamic(componentId, component.value, [next])}
          >
            {options.map((option) => (
              <RadioListItem key={option.value} label={option.label} value={option.value} />
            ))}
          </RadioList>
        );
      }
      default:
        return null;
    }
  };

  return (
    <VStack gap={2}>
      {renderComponent("root", new Set())}
      {item.lifecycle !== "pending" ? (
        <Text type="supporting" color="secondary">{lifecycleLabel(item.lifecycle)}</Text>
      ) : null}
    </VStack>
  );
}

function parseSnapshot(value: unknown): A2UISnapshot | null {
  if (
    !isObject(value) ||
    !isObject(value.components) ||
    !Array.isArray(value.actions) ||
    !Object.prototype.hasOwnProperty.call(value, "data_model")
  ) {
    return null;
  }
  return {
    surface_id: stringValue(value.surface_id),
    send_data_model: value.send_data_model === true,
    components: Object.fromEntries(
      Object.entries(value.components).filter((entry): entry is [string, JSONObject] => isObject(entry[1]))
    ),
    data_model: value.data_model,
    actions: value.actions.filter(isObject).flatMap((action) =>
      typeof action.source_component_id === "string" && typeof action.name === "string"
        ? [{
            source_component_id: action.source_component_id,
            name: action.name,
            context: action.context
          }]
        : []
    )
  };
}

function textContent(components: Record<string, JSONObject>, id: string, model: unknown) {
  const component = components[id];
  return component?.component === "Text" ? stringValue(resolveDynamic(component.text, model)) : "";
}

function choiceOptions(value: unknown, model: unknown) {
  return Array.isArray(value) ? value.filter(isObject).flatMap((option) =>
    typeof option.value === "string"
      ? [{ value: option.value, label: stringValue(resolveDynamic(option.label, model)) }]
      : []
  ) : [];
}

function resolveDynamic(value: DynamicValue, model: unknown) {
  const path = bindingPath(value);
  return path === null ? value : getPointer(model, path);
}

function resolveBindings(value: unknown, model: unknown): unknown {
  const path = bindingPath(value);
  if (path !== null) {
    return getPointer(model, path) ?? null;
  }
  if (Array.isArray(value)) {
    return value.map((entry) => resolveBindings(entry, model));
  }
  if (isObject(value)) {
    return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, resolveBindings(entry, model)]));
  }
  return value;
}

function resolveActionContext(value: unknown, model: unknown) {
  return isObject(value)
    ? Object.fromEntries(
        Object.entries(value).map(([key, entry]) => [key, resolveBindings(entry, model)])
      )
    : undefined;
}

function bindingPath(value: unknown) {
  return isObject(value) && Object.keys(value).length === 1 && typeof value.path === "string"
    ? value.path
    : null;
}

function getPointer(value: unknown, pointer: string): unknown {
  if (pointer === "" || pointer === "/") {
    return value;
  }
  let current = value;
  for (const segment of pointer.split("/").slice(1)) {
    const key = segment.replaceAll("~1", "/").replaceAll("~0", "~");
    if (Array.isArray(current) && /^(0|[1-9]\d*)$/.test(key)) {
      current = current[Number(key)];
    } else if (isObject(current) && Object.prototype.hasOwnProperty.call(current, key)) {
      current = current[key];
    } else {
      return undefined;
    }
  }
  return current;
}

function setPointer(value: unknown, pointer: string, next: unknown): unknown {
  if (pointer === "" || pointer === "/") {
    return next;
  }
  const segments = pointer.split("/").slice(1).map((segment) =>
    segment.replaceAll("~1", "/").replaceAll("~0", "~")
  );
  return updatePointer(value, segments, next);
}

function updatePointer(current: unknown, segments: string[], next: unknown): unknown {
  if (segments.length === 0) {
    return next;
  }
  const [segment, ...rest] = segments;
  if (Array.isArray(current) && /^(0|[1-9]\d*)$/.test(segment)) {
    const copy = [...current];
    const index = Number(segment);
    copy[index] = updatePointer(copy[index], rest, next);
    return copy;
  }
  const entries = isObject(current) ? Object.entries(current) : [];
  const previous = entries.find(([key]) => key === segment)?.[1];
  return Object.fromEntries([
    ...entries.filter(([key]) => key !== segment),
    [segment, updatePointer(previous, rest, next)]
  ]);
}

function headingLevelFor(value: unknown): 1 | 2 | 3 | 4 | 5 | null {
  return value === "h1" ? 1 : value === "h2" ? 2 : value === "h3" ? 3 : value === "h4" ? 4 : value === "h5" ? 5 : null;
}

function stackJustify(value: unknown): "start" | "center" | "end" | "between" | "around" | "evenly" | undefined {
  return value === "spaceBetween" ? "between" : value === "spaceAround" ? "around" : value === "spaceEvenly" ? "evenly" : value === "start" || value === "center" || value === "end" ? value : undefined;
}

function stackAlign(value: unknown): "start" | "center" | "end" | "stretch" | undefined {
  return value === "start" || value === "center" || value === "end" || value === "stretch" ? value : undefined;
}

function buttonVariant(value: unknown): "primary" | "secondary" | "ghost" {
  return value === "primary" ? "primary" : value === "borderless" ? "ghost" : "secondary";
}

function lifecycleLabel(value: string) {
  return value === "answered" || value === "completed" ? "Submitted" : value === "failed" ? "Submission failed" : "No longer available";
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === "string") : [];
}

function stringValue(value: unknown) {
  return typeof value === "string" ? value : value === null || value === undefined ? "" : String(value);
}

function isObject(value: unknown): value is JSONObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
