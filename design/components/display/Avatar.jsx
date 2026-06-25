import React from 'react';

/* Noema Avatar — user/agent identity. Supports image, initials, or agent mark.
   Agent avatars use a pine tint to distinguish them from people. */

const CSS = `
.noema-avatar{
  position:relative; display:inline-flex; align-items:center; justify-content:center;
  flex:none; font-family:var(--font-sans); font-weight:var(--weight-semibold);
  color:var(--paper-50); overflow:hidden; user-select:none;
  border-radius:var(--radius-round);
}
.noema-avatar--square{ border-radius:var(--radius-md); }
.noema-avatar--xs{ width:24px; height:24px; font-size:10px; }
.noema-avatar--sm{ width:32px; height:32px; font-size:12px; }
.noema-avatar--md{ width:40px; height:40px; font-size:15px; }
.noema-avatar--lg{ width:52px; height:52px; font-size:19px; }
.noema-avatar__img{ width:100%; height:100%; object-fit:cover; }
.noema-avatar--agent{ background:linear-gradient(150deg, var(--pine-500), var(--pine-700)); }
.noema-avatar--person{ background:var(--clay-500); }
.noema-avatar--ring{ box-shadow:0 0 0 2px var(--surface-card), 0 0 0 4px var(--pine-300); }
.noema-avatar__status{
  position:absolute; right:-1px; bottom:-1px; width:30%; height:30%; min-width:8px; min-height:8px;
  border-radius:50%; border:2px solid var(--surface-card); background:var(--status-success);
}
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'avatar');
  el.textContent = CSS;
  document.head.appendChild(el);
}

function initials(name = '') {
  return name.trim().split(/\s+/).slice(0, 2).map((w) => w[0]).join('').toUpperCase() || '?';
}

export function Avatar({
  name, src, kind = 'person', size = 'md', square = false, ring = false, status = false, className = '', ...rest
}) {
  ensureStyles();
  const cls = [
    'noema-avatar',
    `noema-avatar--${size}`,
    `noema-avatar--${kind}`,
    square && 'noema-avatar--square',
    ring && 'noema-avatar--ring',
    className,
  ].filter(Boolean).join(' ');
  return (
    <span className={cls} title={name} {...rest}>
      {src ? <img className="noema-avatar__img" src={src} alt={name || ''} /> : initials(name)}
      {status && <span className="noema-avatar__status" />}
    </span>
  );
}
