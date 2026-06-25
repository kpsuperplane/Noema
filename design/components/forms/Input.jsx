import React from 'react';

/* Noema Input — labelled text field with hint/error and optional icons. */

const CSS = `
.noema-field{ display:flex; flex-direction:column; gap:6px; font-family:var(--font-sans); }
.noema-field__label{ font-size:var(--text-sm); font-weight:var(--weight-semibold); color:var(--text-primary); }
.noema-field__req{ color:var(--status-danger); margin-left:2px; }
.noema-field__hint{ font-size:var(--text-xs); color:var(--text-muted); }
.noema-field__hint--error{ color:var(--status-danger); }

.noema-input{
  display:flex; align-items:center; gap:8px; height:40px; padding:0 12px;
  background:var(--surface-card); border:1px solid var(--border-default);
  border-radius:var(--radius-md); box-shadow:var(--shadow-xs);
  transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
}
.noema-input:hover{ border-color:var(--border-strong); }
.noema-input:focus-within{ border-color:var(--border-focus); box-shadow:var(--ring); }
.noema-input--sm{ height:32px; border-radius:var(--radius-sm); }
.noema-input--lg{ height:48px; border-radius:var(--radius-lg); }
.noema-input--error{ border-color:var(--status-danger); }
.noema-input--error:focus-within{ box-shadow:0 0 0 3px var(--status-danger-soft); }
.noema-input--disabled{ background:var(--surface-sunken); opacity:0.7; pointer-events:none; }
.noema-input__icon{ display:flex; color:var(--text-muted); flex:none; }
.noema-input__el{
  flex:1; min-width:0; border:none; outline:none; background:transparent;
  font-family:var(--font-sans); font-size:var(--text-sm); color:var(--text-primary);
}
.noema-input__el::placeholder{ color:var(--text-faint); }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'input');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Input({
  label,
  hint,
  error,
  required,
  size = 'md',
  leftIcon,
  rightIcon,
  disabled,
  id,
  className = '',
  ...rest
}) {
  ensureStyles();
  const fieldId = id || (label ? `noema-${String(label).replace(/\s+/g, '-').toLowerCase()}` : undefined);
  const boxCls = [
    'noema-input',
    size !== 'md' && `noema-input--${size}`,
    error && 'noema-input--error',
    disabled && 'noema-input--disabled',
  ].filter(Boolean).join(' ');

  return (
    <div className={`noema-field ${className}`}>
      {label && (
        <label className="noema-field__label" htmlFor={fieldId}>
          {label}{required && <span className="noema-field__req">*</span>}
        </label>
      )}
      <div className={boxCls}>
        {leftIcon && <span className="noema-input__icon">{leftIcon}</span>}
        <input id={fieldId} className="noema-input__el" disabled={disabled} aria-invalid={!!error} {...rest} />
        {rightIcon && <span className="noema-input__icon">{rightIcon}</span>}
      </div>
      {(error || hint) && (
        <span className={`noema-field__hint ${error ? 'noema-field__hint--error' : ''}`}>{error || hint}</span>
      )}
    </div>
  );
}
