import React from 'react';

/* Noema Tabs — segmented or underline tab bar. Uncontrolled or controlled. */

const CSS = `
.noema-tabs{ font-family:var(--font-sans); }
.noema-tabs__list{ display:inline-flex; align-items:center; gap:2px; }

/* underline style */
.noema-tabs--underline .noema-tabs__list{ gap:18px; border-bottom:1px solid var(--border-subtle); }
.noema-tabs--underline .noema-tab{
  position:relative; padding:9px 1px; background:none; border:none; cursor:pointer;
  font-size:var(--text-sm); font-weight:var(--weight-medium); color:var(--text-muted);
  transition: color var(--dur-fast) var(--ease-out);
}
.noema-tabs--underline .noema-tab:hover{ color:var(--text-secondary); }
.noema-tabs--underline .noema-tab[aria-selected="true"]{ color:var(--text-primary); font-weight:var(--weight-semibold); }
.noema-tabs--underline .noema-tab[aria-selected="true"]::after{
  content:""; position:absolute; left:0; right:0; bottom:-1px; height:2px;
  background:var(--action-primary); border-radius:2px 2px 0 0;
}

/* segmented (pill) style */
.noema-tabs--segmented .noema-tabs__list{
  gap:2px; padding:3px; background:var(--surface-sunken); border-radius:var(--radius-md);
  border:1px solid var(--border-subtle);
}
.noema-tabs--segmented .noema-tab{
  padding:6px 14px; border:none; background:transparent; cursor:pointer; border-radius:var(--radius-sm);
  font-size:var(--text-sm); font-weight:var(--weight-medium); color:var(--text-secondary);
  transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-tabs--segmented .noema-tab:hover{ color:var(--text-primary); }
.noema-tabs--segmented .noema-tab[aria-selected="true"]{
  background:var(--surface-card); color:var(--text-primary); font-weight:var(--weight-semibold);
  box-shadow:var(--shadow-xs);
}
.noema-tab:focus-visible{ outline:none; box-shadow:var(--ring); border-radius:var(--radius-sm); }
.noema-tab__count{ margin-left:6px; font-size:var(--text-2xs); color:var(--text-muted); font-family:var(--font-mono); }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'tabs');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Tabs({ items = [], value, defaultValue, onChange, variant = 'underline', className = '', ...rest }) {
  ensureStyles();
  const [internal, setInternal] = React.useState(defaultValue ?? (items[0] && items[0].value));
  const active = value !== undefined ? value : internal;
  const select = (v) => { if (value === undefined) setInternal(v); onChange && onChange(v); };
  return (
    <div className={`noema-tabs noema-tabs--${variant} ${className}`} {...rest}>
      <div className="noema-tabs__list" role="tablist">
        {items.map((it) => (
          <button
            key={it.value} role="tab" aria-selected={active === it.value}
            className="noema-tab" onClick={() => select(it.value)}
          >
            {it.label}
            {it.count != null && <span className="noema-tab__count">{it.count}</span>}
          </button>
        ))}
      </div>
    </div>
  );
}
