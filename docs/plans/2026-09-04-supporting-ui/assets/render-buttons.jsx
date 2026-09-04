// Run with the installed Noema frontend dependencies. Output is static preview markup.
// Saving uses disabled text here; the production Button loading spinner requires its canvas runtime.
import React from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {Button} from '@astryxdesign/core/Button';

const templates = Object.fromEntries(['primary', 'secondary', 'outline', 'saving', 'disabled-secondary'].map(kind => [kind,
  renderToStaticMarkup(<Button label="__LABEL__" variant={kind === 'primary' || kind === 'saving' ? 'primary' : 'secondary'} size="md" isDisabled={kind === 'saving' || kind === 'disabled-secondary'} className={`preview-action ${kind === 'saving' ? 'primary' : kind === 'disabled-secondary' ? 'secondary' : kind}`}/>),
]));
console.log(`/* Generated from Astryx 0.1.9 Button. Do not style a parallel button. */\nwindow.noemaButtonMarkup = ${JSON.stringify(templates)};`);
