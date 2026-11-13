// Unified search, as in the apps: one query to every source that can search, answers merged by the
// core (jf_plugin_model::merge through jf-web's search_sections) into one section per source in the
// order they answered, each entry where its first playable copy is, with "also in" for the others.

import type { Jf } from '../core/wasm';
import type { Item } from '../core/types';
import { SEARCH_MS } from '../plugins/host';
import { explain, type Sources } from './sources.svelte';

export interface SectionRow {
  item: Item;
  also_in: string[];
}

export interface Section {
  source: string;
  name: string;
  status: 'loading' | 'done' | 'error';
  error: string | null;
  rows: SectionRow[];
}

const DEBOUNCE_MS = 250;
/** Answers that arrive this close together are merged in one pass (no reordering flicker). */
const SETTLE_MS = 100;

export class Search {
  query = $state('');
  sections = $state.raw<Section[]>([]);
  private generation = 0;
  private debounce: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly jf: Jf,
    private readonly sources: Sources,
  ) {}

  /** The query changed (as typed): search after a pause. */
  input(query: string) {
    this.query = query;
    if (this.debounce) clearTimeout(this.debounce);
    this.debounce = setTimeout(() => this.run(query), DEBOUNCE_MS);
  }

  /** Searches now (Enter, or a repeated query after sources changed). */
  run(query = this.query) {
    if (this.debounce) clearTimeout(this.debounce);
    this.debounce = null;
    const generation = ++this.generation;
    const text = query.trim();
    if (!text) {
      this.sections = [];
      return;
    }
    const plugins = this.sources.usable('Search');
    const pending = new Map(plugins.map((p) => [p.id, p.name]));
    const answers: [string, Item[]][] = [];
    const failures = new Map<string, string>();
    let settle: ReturnType<typeof setTimeout> | null = null;
    const publish = () => {
      settle = null;
      if (generation !== this.generation) return;
      const merged = JSON.parse(this.jf.search_sections(JSON.stringify(answers))) as SectionRow[][];
      const sections: Section[] = answers.map(([source], i) => ({
        source,
        name: this.sources.get(source)?.name ?? source,
        status: 'done',
        error: null,
        rows: merged[i] ?? [],
      }));
      for (const [source, error] of failures) {
        sections.push({ source, name: this.sources.get(source)?.name ?? source, status: 'error', error, rows: [] });
      }
      for (const [source, name] of pending) sections.push({ source, name, status: 'loading', error: null, rows: [] });
      this.sections = sections;
    };
    const schedule = () => {
      if (!settle) settle = setTimeout(publish, pending.size === 0 ? 0 : SETTLE_MS);
    };
    publish();
    for (const plugin of plugins) {
      plugin
        .search(text, 50, SEARCH_MS)
        .then((page) => {
          answers.push([plugin.id, page.items]);
        })
        .catch(async (error) => {
          failures.set(plugin.id, await explain(error, plugin));
        })
        .finally(() => {
          pending.delete(plugin.id);
          if (generation === this.generation) schedule();
        });
    }
  }
}
