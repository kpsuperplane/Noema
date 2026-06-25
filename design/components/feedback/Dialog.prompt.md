Centered modal for confirmations, forms, and detail views. Clicking the scrim or × closes it.

```jsx
<Dialog
  open={open}
  onClose={() => setOpen(false)}
  title="Grant filesystem access?"
  description="Research Agent is requesting read access to ~/Documents."
  footer={<>
    <Button variant="ghost" onClick={cancel}>Not now</Button>
    <Button onClick={allow}>Allow</Button>
  </>}
>
  Tools can be revoked anytime from Settings → Permissions.
</Dialog>
```

Sizes `sm`/`md`/`lg`. Provide actions via `footer`.
