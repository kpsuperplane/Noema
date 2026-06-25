import React from 'react';

/* Noema Select — styled native <select> with chevron + label/hint. */

const CSS = `
.noema-select__box{ position:relative; display:flex; align-items:center; }
.noema-select__el{
  appearance:none; -webkit-appearance:none; width:100%; height:40px;
  padding:0 38px 0 12px; background:var(--surface-card); color:var(--text-primary);
  font-family:var(--font-sans); font-size:var(--text-sm); cursor:pointer;
  border:1px solid var(--border-default); border-radius:var(--radius-md); box-shadow:var(--shadow-xs);
  transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-select__el:hover{ border-color:var(--border-strong); }
.noema-select__el:focus-visible{ outline:none; border-color:var(--border-focus); box-shadow:var(--ring); }
.noema-select__el:disabled{ background:var(--surface-sunken); opacity:0.7; cursor:not-allowed; }
.noema-select__el--sm{ height:32px; border-radius:var(--radius-sm); font-size:var(--text-xs); }
.noema-select__chev{
  position:absolute; right:12px; pointer-events:none; color:var(--text-muted);
  width:16px; height:16px;
}
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'select');
  el.textContent = CSS;
  document.head.appendChild(el);
}

const Chevron = () => (
  <svg className="noema-select__chev" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export function Select({
  label, hint, error, required, size = 'md', options, children, id, className = '', ...rest
}) {
  ensureStyles();
  const fieldId = id || (label ? `noema-sel-${String(label).replace(/\s+/g, '-').toLowerCase()}` : undefined);
  return (
    <div className={`noema-field ${className}`}>
      {label && (
        <label className="noema-field__label" htmlFor={fieldId}>
          {label}{required && <span className="noema-field__req">*</span>}
        </label>
      )}
      <div className="noema-select__box">
        <select id={fieldId} className={`noema-select__el ${size === 'sm' ? 'noema-select__el--sm' : ''}`} {...rest}>
          {options
            ? options.map((o) => {
                const opt = typeof o === 'string' ? { value: o, label: o } : o;
                return <option key={opt.value} value={opt.value}>{opt.label}</option>;
              })
            : children}
        </select>
        <Chevron />
      </div>
      {(error || hint) && (
        <span className={`noema-field__hint ${error ? 'noema-field__hint--error' : ''}`}>{error || hint}</span>
      )}
    </div>
  );
}
