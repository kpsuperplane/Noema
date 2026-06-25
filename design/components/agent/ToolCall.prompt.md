Shows an agent invoking a tool, with status and expandable args/result. Embodies Noema's transparency principle — the user can always see what the agent did.

```jsx
<ToolCall tool="web_search" status="success"
  args={{ query: "retrieval augmented generation" }}
  result="3 results found" />

<ToolCall tool="filesystem.read" status="running" />
<ToolCall tool="shell.exec" status="error" args="rm -rf /tmp/cache" result="Permission denied" defaultOpen />
```

Status: `pending` `running` `success` `error`. Render between `ChatMessage` turns. Click the header to expand.
