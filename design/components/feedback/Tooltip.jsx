import React from 'react';

/* Noema Tooltip — hover/focus label on a wrapped trigger. CSS-driven. */

const CSS = `
.noema-tip{ position:relative; display:inline-flex; }
.noema-tip__pop{
  position:absolute; z-index:1200; left:50%; transform:translateX(-50%) translateY(4px);
  bottom:calc(100% + 8px); padding:6px 9px; white-space:nowrap;
  background:var(--ink-900); color:var(--paper-50); font-family:var(--font-sans);
  font-size:var(--text-xs); font-weight:var(--weight-medium); border-radius:var(--radius-sm);
  box-shadow:var(--shadow-md); opacity:0; pointer-events:none;
  transition: opacity var(--dur-fast) var(--ease-out), transform var(--dur-fast) var(--ease-out);
}
.noema-tip__pop::after{
  content:""; position:absolute; top:100%; left:50%; transform:translateX(-50%);
  border:4px solid transparent; border-top-color:var(--ink-900);
}
.noema-tip:hover .noema-tip__pop, .noema-tip:focus-within .noema-tip__pop{
  opacity:1; transform:translateX(-50%) translateY(0);
}
.noema-tip--bottom .noema-tip__pop{ bottom:auto; top:calc(100% + 8px); transform:translateX(-50%) translateY(-4px); }
.noema-tip--bottom .noema-tip__pop::after{ top:auto; bottom:100%; border-top-color:transparent; border-bottom-color:var(--ink-900); }
.noema-tip--bottom:hover .noema-tip__pop{ transform:translateX(-50%) translateY(0); }
.noema-tip__kbd{ margin-left:6px; font-family:var(--font-mono); font-size:var(--text-2xs); opacity:0.7; }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'tooltip');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Tooltip({ label, kbd, side = 'top', children, className = '', ...rest }) {
  ensureStyles();
  return (
    <span className={`noema-tip ${side === 'bottom' ? 'noema-tip--bottom' : ''} ${className}`} {...rest}>
      {children}
      <span className="noema-tip__pop" role="tooltip">
        {label}
        {kbd && <span className="noema-tip__kbd">{kbd}</span>}
      </span>
    </span>
  );
}
