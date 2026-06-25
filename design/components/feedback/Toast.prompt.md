Transient notification. Stack multiple inside a fixed `.noema-toast__stack` container at bottom-right.

```jsx
<div className="noema-toast__stack">
  <Toast tone="success" title="Agent deployed" message="research-agent is now live." onClose={dismiss} />
  <Toast tone="warning" title="Tool needs approval" />
</div>
```

Tones: `success` `warning` `danger` `info`.
