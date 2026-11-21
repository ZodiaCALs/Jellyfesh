// Jellyfesh Pro in Jellyfesh Web (ADR-0021; docs/ui.md "The Pro sheet"): jf-web's Pro rules
// (core/jf-web/src/pro.rs, after the core's) decide; this keeps the licence key in the browser
// (it is checked again at every visit) and opens the Pro sheet from any Pro action while Pro is
// inactive.

import type { Jf } from '../core/wasm';

const KEY = 'jellyfesh.licence';

export type LicenceError = 'Malformed' | 'UnknownKey' | 'BadSignature' | 'WrongProduct' | 'Revoked';

export type ProSource = 'None' | 'Preview' | { Licence: { id_suffix: string; issued_at_ms: number } };

export interface ProSnapshot {
  active: boolean;
  source: ProSource;
  preview: boolean;
  licence_error: LicenceError | null;
  licence_keys: boolean;
}

export class Pro {
  snapshot = $state.raw<ProSnapshot>({ active: false, source: 'None', preview: false, licence_error: null, licence_keys: true });
  /** The Pro sheet is open. */
  sheet = $state(false);
  /** Enter Licence Key is open. */
  entering = $state(false);
  private readonly inner: InstanceType<Jf['Pro']>;
  private listeners = new Set<(active: boolean) => void>();

  constructor(jf: Jf) {
    let stored: string | undefined;
    try {
      stored = localStorage.getItem(KEY) ?? undefined;
    } catch {
      // Storage blocked: no key this visit.
    }
    this.inner = new jf.Pro(stored);
    this.snapshot = JSON.parse(this.inner.snapshot());
  }

  get active(): boolean {
    return this.snapshot.active;
  }

  /** Runs `action` with Pro, or opens the Pro sheet. */
  require(action: () => void) {
    if (this.active) action();
    else this.sheet = true;
  }

  /** Checks a licence key; null when it was accepted, else why not. */
  activate(key: string): LicenceError | null {
    const answer = JSON.parse(this.inner.activate(key)) as { ok: string } | { err: LicenceError };
    this.refresh();
    if ('err' in answer) return answer.err;
    try {
      localStorage.setItem(KEY, answer.ok);
    } catch {
      // Kept for this visit only.
    }
    return null;
  }

  removeLicence() {
    this.inner.remove_licence();
    try {
      localStorage.removeItem(KEY);
    } catch {
      // Nothing kept.
    }
    this.refresh();
  }

  onChange(listener: (active: boolean) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private refresh() {
    const was = this.snapshot.active;
    this.snapshot = JSON.parse(this.inner.snapshot());
    if (was !== this.snapshot.active) for (const listener of this.listeners) listener(this.snapshot.active);
  }
}

/** A licence error in the apps' words. */
export function licenceErrorText(error: LicenceError, t: (text: string) => string): string {
  switch (error) {
    case 'Malformed':
      return t('That doesn’t look like a Jellyfesh licence key.');
    case 'UnknownKey':
    case 'BadSignature':
      return t('This licence key isn’t valid.');
    case 'WrongProduct':
      return t('This key is for another product.');
    case 'Revoked':
      return t('This licence key has been withdrawn.');
  }
}
