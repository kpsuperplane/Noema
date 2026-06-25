A single conversation turn. Agent turns render as full-width prose; user turns sit in a soft pine bubble.

```jsx
<ChatMessage role="user" name="You" time="2:14 PM" avatar={<Avatar name="You" size="sm"/>}>
  Find the three most-cited papers on retrieval and summarize them.
</ChatMessage>

<ChatMessage role="agent" name="Research Agent" time="2:14 PM"
  avatar={<Avatar kind="agent" square size="sm" name="RA"/>}>
  <p>Here's what I found…</p>
</ChatMessage>

<ChatMessage role="agent" typing avatar={<Avatar kind="agent" square size="sm"/>} />
```

Set `typing` for the in-progress indicator. Compose ToolCall between agent messages to show tool use.
