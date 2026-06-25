Tab bar for switching views. `underline` for page-level sections, `segmented` for compact in-card switches.

```jsx
<Tabs
  variant="underline"
  items={[
    { value: 'chat', label: 'Chat' },
    { value: 'tools', label: 'Tools', count: 4 },
    { value: 'memory', label: 'Memory' },
  ]}
  defaultValue="chat"
  onChange={setTab}
/>
```

Controlled (`value`) or uncontrolled (`defaultValue`).
