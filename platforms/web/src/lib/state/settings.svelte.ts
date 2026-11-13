// The person's preferences: kept in this browser (localStorage, nothing secret), normalised by the
// core (jf_web::settings) every time they change, as the apps' Settings does.

import type { Jf } from '../core/wasm';
import type { WebSettings } from '../core/types';

const KEY = 'jf.settings';

export class Settings {
  value = $state<WebSettings>() as WebSettings;
  private listeners = new Set<(settings: WebSettings) => void>();

  constructor(private readonly core: InstanceType<Jf['Core']>) {
    let stored: string | null = null;
    try {
      stored = localStorage.getItem(KEY);
    } catch {
      // Storage blocked: defaults for this visit.
    }
    let value: WebSettings = JSON.parse(core.settings());
    if (stored) {
      try {
        value = JSON.parse(core.set_settings(stored));
      } catch {
        // Unreadable: defaults.
      }
    }
    this.value = value;
  }

  /** Changes the settings through `change`, normalised by the core, then saves them. */
  update(change: (draft: WebSettings) => void) {
    const draft = $state.snapshot(this.value) as WebSettings;
    change(draft);
    this.value = JSON.parse(this.core.set_settings(JSON.stringify(draft)));
    try {
      localStorage.setItem(KEY, JSON.stringify(this.value));
    } catch {
      // Kept for this visit only.
    }
    for (const listener of this.listeners) listener(this.value);
  }

  onChange(listener: (settings: WebSettings) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}
