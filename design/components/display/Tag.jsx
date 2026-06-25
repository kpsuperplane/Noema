import React from 'react';

/* Noema Tag — input chip / filter token. Optional leading icon and remove button. */

const CSS = `
.noema-tag{
  display:inline-flex; align-items:center; gap:6px; height:26px; padding:0 10px;
  font-family:var(--font-sans); font-size:var(--text-xs); font-weight:var(--weight-medium);
  color:var(--text-primary); background:var(--surface-sunken);
  border:1px solid var(--border-subtle); border-radius:var(--radius-sm); white-space:nowrap;
}
.noema-tag--brand{ background:var(--surface-brand-soft); border-color:var(--pine-100); color:var(--pine-700); }
.noema-tag--mono{ font-family:var(--font-mono); font-size:var(--text-2xs); }
.noema-tag--selectable{ cursor:pointer; transition: background var(--dur-fast) var(--ease-out), border-color var(--dur-fast) var(--ease-out); }
.noema-tag--selectable:hover{ border-color:var(--border-default); background:var(--surface-active); }
.noema-tag--selected{ background:var(--action-primary); border-color:var(--action-primary); color:var(--paper-50); }
.noema-tag__x{
  display:inline-flex; align-items:center; justify-content:center; width:15px; height:15px;
  margin-right:-3px; border-radius:50%; border:none; background:transparent; color:inherit;
  cursor:pointer; opacity:0.6;
}
.noema-tag__x:hover{ opacity:1; background:rgba(0,0,0,0.08); }
.noema-tag__x svg{ width:9px; height:9px; }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'tag');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Tag({
  children, icon, tone = 'neutral', mono = false, selected, selectable = false, onRemove, className = '', ...rest
}) {
  ensureStyles();
  const cls = [
    'noema-tag',
    tone === 'brand' && 'noema-tag--brand',
    mono && 'noema-tag--mono',
    (selectable || selected) && 'noema-tag--selectable',
    selected && 'noema-tag--selected',
    className,
  ].filter(Boolean).join(' ');
  return (
    <span className={cls} {...rest}>
      {icon}
      {children}
      {onRemove && (
        <button type="button" className="noema-tag__x" aria-label="Remove" onClick={onRemove}>
          <svg viewBox="0 0 10 10" fill="none"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
        </button>
      )}
    </span>
  );
}
