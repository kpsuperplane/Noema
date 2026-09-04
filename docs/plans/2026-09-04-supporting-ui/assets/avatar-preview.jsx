// Offline preview of IdentityAvatar's local agent. Reuse IdentityAvatar in the app.
import React from 'react';
import {createRoot} from 'react-dom/client';
import Avatar from '@kpsuperplane/boring-avatars';

const actorId = 'agent:local';
let hash = 0x811c9dc5;
for (let index = 0; index < actorId.length; index++) {
  hash = Math.imul(hash ^ actorId.charCodeAt(index), 0x01000162);
}
export const avatarProps = {
  name: `actor-${(hash >>> 0).toString(16).padStart(8, '0')}`,
  colors: ['#3b4a6b', '#7d6a91', '#b9786d', '#d6ad6b', '#e6d8c4', '#2f3440'],
  variant: 'beam', activity: 'idle', animated: true, size: '100%',
  title: false, 'aria-hidden': true, focusable: 'false'
};
if (typeof document !== 'undefined') {
  createRoot(document.getElementById('noema-avatar')).render(<Avatar {...avatarProps}/>);
}
