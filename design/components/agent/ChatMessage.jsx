import React from 'react';

/* Noema ChatMessage — a turn in an agent conversation.
   User turns sit in a soft bubble; agent turns are full-width prose with a mark. */

const CSS = `
.noema-msg{ display:flex; gap:12px; font-family:var(--font-sans); padding:var(--space-3) 0; }
.noema-msg--user{ flex-direction:row-reverse; }
.noema-msg__avatar{ flex:none; }
.noema-msg__col{ min-width:0; max-width:760px; }
.noema-msg--user .noema-msg__col{ display:flex; flex-direction:column; align-items:flex-end; }
.noema-msg__meta{ display:flex; align-items:baseline; gap:8px; margin-bottom:4px; }
.noema-msg__name{ font-size:var(--text-sm); font-weight:var(--weight-semibold); color:var(--text-primary); }
.noema-msg__time{ font-family:var(--font-mono); font-size:var(--text-2xs); color:var(--text-faint); }
.noema-msg__bubble{
  font-size:var(--text-base); line-height:var(--leading-relaxed); color:var(--text-primary);
}
.noema-msg--user .noema-msg__bubble{
  background:var(--surface-brand-soft); border:1px solid var(--pine-100);
  padding:11px 15px; border-radius:var(--radius-lg) var(--radius-lg) var(--radius-xs) var(--radius-lg);
  font-size:var(--text-sm); text-align:left;
}
.noema-msg--agent .noema-msg__bubble > :first-child{ margin-top:0; }
.noema-msg--agent .noema-msg__bubble > :last-child{ margin-bottom:0; }
.noema-msg__typing{ display:inline-flex; gap:4px; padding:6px 0; }
.noema-msg__typing span{ width:6px; height:6px; border-radius:50%; background:var(--pine-400); animation:noema-typing 1.2s var(--ease-in-out) infinite; }
.noema-msg__typing span:nth-child(2){ animation-delay:0.15s; }
.noema-msg__typing span:nth-child(3){ animation-delay:0.3s; }
@keyframes noema-typing{ 0%,60%,100%{ opacity:0.3; transform:translateY(0); } 30%{ opacity:1; transform:translateY(-3px); } }
`;

let injected = false;
function ensureStyles() {
  if (injected || typeof document === 'undefined') return;
  injected = true;
  const el = document.createElement('style');
  el.setAttribute('data-noema', 'chatmessage');
  el.textContent = CSS;
  document.head.appendChild(el);
}

export function ChatMessage({
  role = 'agent', name, time, avatar, typing = false, children, className = '', ...rest
}) {
  ensureStyles();
  const isUser = role === 'user';
  return (
    <div className={`noema-msg noema-msg--${isUser ? 'user' : 'agent'} ${className}`} {...rest}>
      {avatar && <div className="noema-msg__avatar">{avatar}</div>}
      <div className="noema-msg__col">
        {(name || time) && (
          <div className="noema-msg__meta">
            {name && <span className="noema-msg__name">{name}</span>}
            {time && <span className="noema-msg__time">{time}</span>}
          </div>
        )}
        <div className="noema-msg__bubble">
          {typing ? (
            <span className="noema-msg__typing" aria-label="Agent is typing"><span /><span /><span /></span>
          ) : children}
        </div>
      </div>
    </div>
  );
}
