// Search by source (ADR-0024 §4), with the core's rules (jf-web's SearchResults): the library at
// once, then every source that is ready to search (ADR-0027 §3, `isReady`: on, allowed, declaring
// search, and signed in when it needs a sign-in; one that isn't set up yet is not asked), each kind
// on its own (songs, albums, artists and playlists, 20 of each), every source within the 8-second
// deadline. A source that fails shows why in plain words. A source whose terms allow it
// is merged with the library and the merging sources before it (From Your Sources on the All tab);
// the others keep their results apart, on a tab of their own, which the All tab links to. Every
// source that answered has a tab with its results kind by kind, each with Show More while it has
// more. A new query leaves the last one's answers unused.

import type { Jf } from '../core/wasm';
import type { ContentPolicy, ItemKind, ItemPage, SearchPlan, SearchTab, SearchView } from '../core/types';
import { t } from '../i18n/index.svelte';
import { SEARCH_MS, type Plugin } from '../plugins/host';
import type { Library } from './library.svelte';
import { explain, type Sources } from './sources.svelte';

const DEBOUNCE_MS = 250;
/** Answers that arrive this close together are merged in one pass (no reordering flicker). */
const SETTLE_MS = 100;

/** The tab shown: All, Library, or a source's tab by its id. */
export type SearchTabId = 'all' | 'library' | `source:${string}`;

export class Search {
  query = $state('');
  /** The All and Library tabs and the sources' tabs; null before a search. */
  view = $state.raw<SearchView | null>(null);
  tab = $state<SearchTabId>('all');
  /** Changes whenever the results do (a source's tab reads its results then). */
  revision = $state(0);
  /** Show More running, by `source|kind`. */
  loadingMore = $state.raw<ReadonlySet<string>>(new Set());
  /** Why Show More failed, by `source|kind`. */
  moreErrors = $state.raw<ReadonlyMap<string, string>>(new Map());
  private results: InstanceType<Jf['SearchResults']> | null = null;
  /** The query the results answer. */
  private searched = '';
  private generation = 0;
  private debounce: ReturnType<typeof setTimeout> | null = null;
  private readonly plan: SearchPlan;

  constructor(
    private readonly jf: Jf,
    private readonly sources: Sources,
    private readonly library: Library,
  ) {
    this.plan = JSON.parse(jf.search_plan()) as SearchPlan;
  }

  /** The query changed (as typed): search after a pause. */
  input(query: string) {
    this.query = query;
    if (this.debounce) clearTimeout(this.debounce);
    this.debounce = setTimeout(() => this.run(query), DEBOUNCE_MS);
  }

  /** Searches now (Enter, or the same query again after sources changed). */
  run(query = this.query) {
    if (this.debounce) clearTimeout(this.debounce);
    this.debounce = null;
    const generation = ++this.generation;
    const text = query.trim();
    this.results?.free();
    this.results = null;
    this.searched = text;
    this.loadingMore = new Set();
    this.moreErrors = new Map();
    if (!text) {
      this.view = null;
      this.tab = 'all';
      this.revision += 1;
      return;
    }
    const local = this.library.search(text, this.plan.library_limit);
    const results = new this.jf.SearchResults(JSON.stringify([...local.tracks, ...local.albums, ...local.artists]));
    this.results = results;
    const plugins = this.sources.usable('Search');
    for (const plugin of plugins) results.asking(plugin.id, plugin.name);
    let settle: ReturnType<typeof setTimeout> | null = null;
    const publish = () => {
      settle = null;
      if (generation !== this.generation) return;
      this.view = JSON.parse(results.view()) as SearchView;
      this.revision += 1;
    };
    const answered = () => {
      if (generation === this.generation && !settle) settle = setTimeout(publish, SETTLE_MS);
    };
    publish();
    for (const plugin of plugins) void this.ask(plugin, text, generation, answered);
  }

  /** Asks one source for each kind, within the deadline; hands what came back to the results. */
  private async ask(plugin: Plugin, text: string, generation: number, answered: () => void) {
    const pages = new Map<ItemKind, ItemPage>();
    let failure: unknown = null;
    const calls = this.plan.kinds.map((kind) =>
      plugin.search(text, this.plan.limit, SEARCH_MS, [kind]).then(
        (page) => void pages.set(kind, page),
        (error: unknown) => void (failure ??= error),
      ),
    );
    let timer: ReturnType<typeof setTimeout> | undefined;
    const late = new Promise<void>((resolve) => (timer = setTimeout(resolve, SEARCH_MS)));
    await Promise.race([Promise.all(calls), late]);
    clearTimeout(timer);
    if (generation !== this.generation || !this.results) return;
    const policy: ContentPolicy | undefined = plugin.info?.content_policy;
    if (pages.size > 0 && policy) {
      // The kinds in the plan's order; a kind it could not answer in time is left out.
      const ordered = this.plan.kinds.flatMap((kind) => (pages.has(kind) ? [[kind, pages.get(kind)]] : []));
      this.results.answer(plugin.id, JSON.stringify(policy), JSON.stringify(ordered));
    } else {
      const why = failure ? await explain(failure, plugin) : t('{service} did not answer in time.', { service: plugin.name });
      if (generation !== this.generation || !this.results) return;
      this.results.fail(plugin.id, why);
    }
    answered();
  }

  /** A source's tab, or null when it has none (it found nothing, failed, or is still searching). */
  tabOf(source: string): SearchTab | null {
    void this.revision;
    return this.results ? (JSON.parse(this.results.tab(source)) as SearchTab | null) : null;
  }

  /** Show More: the next page of a source's results of one kind. */
  async more(source: string, kind: ItemKind) {
    const results = this.results;
    const plugin = this.sources.get(source);
    const cursor = results?.cursor(source, kind);
    const key = `${source}|${kind}`;
    if (!results || !plugin || !cursor || this.loadingMore.has(key)) return;
    const generation = this.generation;
    this.loadingMore = new Set([...this.loadingMore, key]);
    const errors = new Map(this.moreErrors);
    errors.delete(key);
    this.moreErrors = errors;
    try {
      const page = await plugin.search(this.searched, this.plan.limit, SEARCH_MS, [kind], cursor);
      if (generation !== this.generation) return;
      results.more(source, kind, JSON.stringify(page));
      this.revision += 1;
    } catch (error) {
      if (generation !== this.generation) return;
      this.moreErrors = new Map(this.moreErrors).set(key, await explain(error, plugin));
    } finally {
      if (generation === this.generation) {
        const loading = new Set(this.loadingMore);
        loading.delete(key);
        this.loadingMore = loading;
      }
    }
  }
}
