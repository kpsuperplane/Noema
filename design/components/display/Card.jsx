import React from 'react';

/* Noema Card — the primary content container. White surface, hairline border,
   soft warm shadow. Optional `interactive` lift on hover. */

const CSS = `
.noema-card{
  display:block; background:var(--surface-card); color:var(--text-primary);
  border:1px solid var(--border-subtle); border-radius:var(--radius-lg);
  box-shadow:var(--shadow-sm); overflow:clip;
}
.noema-card--pad-sm{ padding:var(--space-4); }
.noema-card--pad-md{ padding:var(--space-6); }
.noema-card--pad-lg{ padding:var(--space-8); }
.noema-card--flat{ box-shadow:none; }
.noema-card--raised{ box-shadow:var(--shadow-md); }
.noema-card--sunken{ background:var(--surface-sunken); box-shadow:none; border-color:var(--border-subtle); }
.noema-card--brand{ background:var(--surface-brand-soft); border-color:var(--pine-100); }
.noema-card--interactive{ cursor:pointer; transition: transform var(--dur-base) var(--ease-out), box-shadow var(--dur-base) var(--ease-out), border-color var(--dur-base) var(--ease-out); }
.noema-card--interactive:hover{ transform: translateY(-2px); box-shadow:var(--shadow-lg); border-color:var(--border-default); }
.noema-card--interactive:active{ transform: translateY(0); box-shadow:var(--shadow-sm); }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'card');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function Card({
  children,
  padding = 'md',
  variant = 'default',
  interactive = false,
  as = 'div',
  className = '',
  ...rest
}) {
  ensureStyles();
  const Tag = as;
  const cls = [
    'noema-card',
    padding && `noema-card--pad-${padding}`,
    variant !== 'default' && `noema-card--${variant}`,
    interactive && 'noema-card--interactive',
    className,
  ].filter(Boolean).join(' ');
  return <Tag className={cls} {...rest}>{children}</Tag>;
}
