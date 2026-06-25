Identity chip for people and agents. Agents use `kind="agent"` (pine, rounded-square) to read differently from people (clay circle).

```jsx
<Avatar name="Ada Lovelace" status />
<Avatar kind="agent" square name="Research Agent" size="lg" />
<Avatar src="/me.jpg" name="You" ring />
```

Sizes `xs`/`sm`/`md`/`lg`. Falls back to initials when no `src`.
