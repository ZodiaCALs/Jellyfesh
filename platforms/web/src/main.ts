import '../generated/tokens.css';
import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { createApp } from './lib/app';

const target = document.getElementById('app')!;

function fail(error: unknown) {
  console.error(error);
  const wasm = typeof WebAssembly === 'object';
  target.innerHTML = '';
  const box = document.createElement('div');
  box.className = 'boot-error';
  const title = document.createElement('h1');
  title.textContent = 'Jellyfesh Web could not start';
  const text = document.createElement('p');
  text.textContent = wasm
    ? `Reload the page to try again. (${error instanceof Error ? error.message : String(error)})`
    : 'This browser does not run WebAssembly. Jellyfesh Web needs a current Chrome, Edge, Firefox or Safari.';
  box.append(title, text);
  target.append(box);
}

createApp()
  .then((app) => {
    target.textContent = '';
    mount(App, { target, props: { app } });
  })
  .catch(fail);

// The service worker caches the app shell for offline starts and carries stream requests that need
// headers (src/lib/audio/streams.ts). Only in builds: the dev server serves modules it rewrites.
if (import.meta.env.PROD && 'serviceWorker' in navigator && window.isSecureContext) {
  addEventListener('load', () => {
    navigator.serviceWorker.register('./sw.js', { scope: './' }).catch((error) => console.warn('service worker:', error));
  });
}
