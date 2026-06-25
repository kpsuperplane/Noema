The primary content container — white surface, hairline border, soft warm shadow.

```jsx
<Card>Default card</Card>
<Card variant="brand" padding="lg">Highlighted</Card>
<Card variant="sunken">Recessed panel</Card>
<Card interactive onClick={open}>Clickable card with hover lift</Card>
```

Variants: `default`, `flat` (no shadow), `raised` (more shadow), `sunken` (recessed), `brand` (pine tint). Set `interactive` for hover-lift. Compose freely — Card has no internal layout opinions.
