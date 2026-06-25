Primary action button — use for any clickable commit/submit/navigate action across Noema.

```jsx
<Button variant="primary" onClick={save}>Save changes</Button>
<Button variant="secondary" leftIcon={<PlusIcon/>}>New agent</Button>
<Button variant="accent" size="lg">Get started</Button>
<Button variant="ghost" size="sm">Cancel</Button>
<Button variant="danger" loading>Deleting…</Button>
```

Variants: `primary` (pine, default CTA), `accent` (clay, marketing/highlight CTAs), `secondary` (outline), `ghost` (low-emphasis), `danger` (destructive). Sizes: `sm` `md` `lg`. Supports `leftIcon`/`rightIcon`, `loading`, `fullWidth`, and `as`/`href` for links. One primary button per view; pair with `ghost`/`secondary` for secondary actions.
