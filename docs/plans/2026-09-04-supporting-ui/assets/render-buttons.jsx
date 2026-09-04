// Run with the installed Noema frontend dependencies. Output is static preview markup.
import React from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {Button} from '@astryxdesign/core/Button';

const templates = Object.fromEntries(['primary', 'secondary', 'outline'].map(kind => [kind,
  renderToStaticMarkup(<Button label="__LABEL__" variant={kind === 'primary' ? 'primary' : 'secondary'} size="md" className={`preview-action ${kind}`}/>),
]));
console.log(`/* Generated from Astryx 0.1.9 Button. Do not style a parallel button. */\nwindow.noemaButtonMarkup = ${JSON.stringify(templates)};`);
