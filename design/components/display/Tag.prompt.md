Chip for tags, capabilities, and filter tokens. Squarer than Badge; can be removable or selectable.

```jsx
<Tag icon={<HashIcon/>}>web-search</Tag>
<Tag tone="brand" mono>filesystem</Tag>
<Tag selectable selected>Active</Tag>
<Tag onRemove={() => drop(id)}>Removable</Tag>
```

Use Tag for user-managed tokens (capabilities, filters); use Badge for system status.
