import * as React from "react";
import { cn } from "@/lib/utils";

function Item({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="item"
      className={cn(
        "flex min-w-0 items-center gap-3 rounded-md border border-[var(--border-subtle)] bg-background p-3 text-sm",
        className
      )}
      {...props}
    />
  );
}

function ItemContent({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="item-content" className={cn("grid min-w-0 flex-1 gap-1", className)} {...props} />;
}

function ItemTitle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="item-title"
      className={cn("min-w-0 font-medium text-foreground [overflow-wrap:anywhere]", className)}
      {...props}
    />
  );
}

function ItemDescription({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="item-description"
      className={cn("min-w-0 text-muted-foreground", className)}
      {...props}
    />
  );
}

function ItemActions({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="item-actions" className={cn("flex shrink-0 items-center gap-2", className)} {...props} />;
}

export { Item, ItemActions, ItemContent, ItemDescription, ItemTitle };
