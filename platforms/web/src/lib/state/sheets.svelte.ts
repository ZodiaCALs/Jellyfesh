// The sheets the library's menus open over any page (App.svelte draws them): a playlist's name
// (New Playlist…, Rename…, Save as Playlist…), the playlist to add songs to (Add to Playlist…) and
// Delete Playlist…, which asks first (docs/ui.md, "Steps that ask first").

import type { Saved } from '../core/types';

/** What asks for a name, and what happens with it (it throws the core's refusal). */
export interface Naming {
  title: string;
  action: string;
  initial: string;
  /** Runs with the name typed; a refusal (an empty name) is shown and the sheet stays. */
  submit: (name: string) => void;
}

/** Add to Playlist…: these songs (or, for an album or playlist, how to get them). */
export interface Picking {
  songs: () => Promise<Saved[]>;
}

export interface Deleting {
  id: number;
  name: string;
  /** After deleting (a playlist's own page goes back). */
  then?: () => void;
}

class Sheets {
  naming = $state.raw<Naming | null>(null);
  picking = $state.raw<Picking | null>(null);
  deleting = $state.raw<Deleting | null>(null);
}

export const sheets = new Sheets();
