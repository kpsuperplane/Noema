import React from 'react';

/* Noema Textarea — multi-line field, shares the field shell with Input. */

const CSS = `
.noema-ta__box{
  display:block; padding:10px 12px; background:var(--surface-card);
  border:1px solid var(--border-default); border-radius:var(--radius-md);
  box-shadow:var(--shadow-xs);
  transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-ta__box:hover{ border-color:var(--border-strong); }
.noema-ta__box:focus-within{ border-color:var(--border-focus); box-shadow:var(--ring); }
.noema-ta__box--error{ border-color:var(--status-danger); }
.noema-ta__el{
  display:block; width:100%; border:none; outline:none; background:transparent; resize:vertical;
  font-family:var(--font-sans); font-size:var(--text-sm); line-height:var(--leading-normal);
  color:var(--text-primary); min-height:84px;
}
.noema-ta__el::placeholder{ color:var(--text-faint); }
.noema-ta__el--mono{ font-family:var(--font-mono); font-size:var(--text-xs); }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'textarea');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Textarea({
  label, hint, error, required, mono = false, id, className = '', rows = 4, ...rest
}) {
  ensureStyles();
  const fieldId = id || (label ? `noema-ta-${String(label).replace(/\s+/g, '-').toLowerCase()}` : undefined);
  return (
    <div className={`noema-field ${className}`}>
      {label && (
        <label className="noema-field__label" htmlFor={fieldId}>
          {label}{required && <span className="noema-field__req">*</span>}
        </label>
      )}
      <div className={`noema-ta__box ${error ? 'noema-ta__box--error' : ''}`}>
        <textarea
          id={fieldId} rows={rows} aria-invalid={!!error}
          className={`noema-ta__el ${mono ? 'noema-ta__el--mono' : ''}`} {...rest}
        />
      </div>
      {(error || hint) && (
        <span className={`noema-field__hint ${error ? 'noema-field__hint--error' : ''}`}>{error || hint}</span>
      )}
    </div>
  );
}
