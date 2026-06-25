import React from 'react';

/* Noema Checkbox — accessible custom checkbox with label. */

const CSS = `
.noema-check{ display:inline-flex; align-items:flex-start; gap:10px; cursor:pointer; font-family:var(--font-sans); }
.noema-check--disabled{ opacity:0.5; cursor:not-allowed; }
.noema-check__input{ position:absolute; opacity:0; width:0; height:0; }
.noema-check__box{
  flex:none; width:18px; height:18px; margin-top:1px; border-radius:5px;
  border:1.5px solid var(--border-strong); background:var(--surface-card);
  display:flex; align-items:center; justify-content:center; color:transparent;
  transition: background var(--dur-fast) var(--ease-out), border-color var(--dur-fast) var(--ease-out);
}
.noema-check__input:checked + .noema-check__box{ background:var(--action-primary); border-color:var(--action-primary); color:var(--paper-50); }
.noema-check__input:focus-visible + .noema-check__box{ box-shadow:var(--ring); }
.noema-check__box svg{ width:12px; height:12px; }
.noema-check__label{ font-size:var(--text-sm); color:var(--text-primary); line-height:1.4; }
.noema-check__desc{ display:block; font-size:var(--text-xs); color:var(--text-muted); margin-top:1px; }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'checkbox');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Checkbox({ label, description, disabled, id, className = '', ...rest }) {
  ensureStyles();
  const fieldId = id || rest.name;
  return (
    <label className={`noema-check ${disabled ? 'noema-check--disabled' : ''} ${className}`} htmlFor={fieldId}>
      <input id={fieldId} type="checkbox" className="noema-check__input" disabled={disabled} {...rest} />
      <span className="noema-check__box" aria-hidden="true">
        <svg viewBox="0 0 12 12" fill="none"><path d="M2 6.2l2.6 2.6L10 3.2" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>
      </span>
      {label && (
        <span className="noema-check__label">
          {label}
          {description && <span className="noema-check__desc">{description}</span>}
        </span>
      )}
    </label>
  );
}
