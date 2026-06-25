Labelled single-line text field. Pass `label`, `hint`, and `error`; supports inline icons.

```jsx
<Input label="Workspace name" placeholder="e.g. research-agent" />
<Input label="API key" leftIcon={<KeyIcon/>} error="Required" required />
<Input label="Search" size="sm" leftIcon={<SearchIcon/>} />
```

`error` overrides `hint` and turns the field red. Sizes `sm`/`md`/`lg`. Spreads native input attributes (`type`, `value`, `onChange`, …).
