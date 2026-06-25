import React from 'react';

/* Noema Switch — on/off toggle for settings. */

const CSS = `
.noema-switch{ display:inline-flex; align-items:center; gap:10px; cursor:pointer; font-family:var(--font-sans); }
.noema-switch--disabled{ opacity:0.5; cursor:not-allowed; }
.noema-switch__input{ position:absolute; opacity:0; width:0; height:0; }
.noema-switch__track{
  position:relative; width:38px; height:22px; border-radius:var(--radius-pill);
  background:var(--border-strong); flex:none;
  transition: background var(--dur-base) var(--ease-out);
}
.noema-switch__thumb{
  position:absolute; top:2px; left:2px; width:18px; height:18px; border-radius:50%;
  background:var(--paper-50); box-shadow:var(--shadow-sm);
  transition: transform var(--dur-base) var(--ease-spring);
}
.noema-switch__input:checked + .noema-switch__track{ background:var(--action-primary); }
.noema-switch__input:checked + .noema-switch__track .noema-switch__thumb{ transform: translateX(16px); }
.noema-switch__input:focus-visible + .noema-switch__track{ box-shadow:var(--ring); }
.noema-switch__label{ font-size:var(--text-sm); color:var(--text-primary); }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'switch');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Switch({ label, disabled, id, className = '', ...rest }) {
  ensureStyles();
  const fieldId = id || rest.name;
  return (
    <label className={`noema-switch ${disabled ? 'noema-switch--disabled' : ''} ${className}`} htmlFor={fieldId}>
      <input id={fieldId} type="checkbox" role="switch" className="noema-switch__input" disabled={disabled} {...rest} />
      <span className="noema-switch__track" aria-hidden="true"><span className="noema-switch__thumb" /></span>
      {label && <span className="noema-switch__label">{label}</span>}
    </label>
  );
}
