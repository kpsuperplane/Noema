Icon-only button for toolbars, headers, and dense controls — always pass an `aria-label`.

```jsx
<IconButton aria-label="Settings" icon={<SettingsIcon/>} />
<IconButton aria-label="Send" variant="primary" icon={<ArrowUpIcon/>} />
<IconButton aria-label="More" variant="soft" round />
```

Variants: `ghost` (default, toolbars), `soft` (filled neutral), `outline`, `primary` (pine). Sizes `sm`/`md`/`lg`; `round` for circular. Pairs with the Lucide icon set.
