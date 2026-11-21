<script lang="ts">
  // Now Playing's Comments (docs/ui.md "Comments"; ADR-0022 §4), for sources whose plugin has a
  // `comments` export: "Comments from {Service}", then Popular (the hot comments) and Latest, each
  // with the author, picture, time, likes and text; more load at the end of the list. Read-only.
  // Empty: "No Comments Yet".
  import Art from '../components/Art.svelte';
  import Icon from '../components/Icon.svelte';
  import { useApp } from '../lib/app';
  import type { Comment, Item } from '../lib/core/types';
  import { language, t, tn } from '../lib/i18n/index.svelte';
  import type { Plugin } from '../lib/plugins/host';
  import { explain } from '../lib/state/sources.svelte';

  let { plugin, item }: { plugin: Plugin; item: Item } = $props();
  const app = useApp();
  let hot = $state.raw<Comment[]>([]);
  let latest = $state.raw<Comment[]>([]);
  let next = $state<string | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let attempt = $state(0);
  let sentinel: HTMLElement | undefined = $state();
  let generation = 0;

  async function load(cursor: string | null) {
    const mine = generation;
    loading = true;
    try {
      const page = await plugin.comments(item, cursor, 50);
      if (mine !== generation) return;
      if (!cursor) hot = page.hot;
      latest = cursor ? [...latest, ...page.comments] : page.comments;
      next = page.next;
      error = null;
    } catch (e) {
      if (mine === generation) error = await explain(e, plugin);
    } finally {
      if (mine === generation) loading = false;
    }
  }

  $effect(() => {
    void item;
    void attempt;
    generation += 1;
    hot = [];
    latest = [];
    next = null;
    void load(null);
  });

  $effect(() => {
    if (!sentinel || !next) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting) && !loading && next) void load(next);
    }, { rootMargin: '300px' });
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

  function when(ms: number | null): string {
    if (ms === null) return '';
    return new Intl.DateTimeFormat(language(), { dateStyle: 'medium' }).format(new Date(ms));
  }

  /** An avatar as a stand-in item, so it loads as any picture of the source does. */
  function avatarOf(comment: Comment): Item | null {
    if (!comment.avatar) return null;
    return { ...item, id: `avatar:${comment.id}`, kind: 'Artist', title: comment.author, artwork: comment.avatar };
  }
</script>

<div class="comments">
  <p class="from">{t('Comments from {service}', { service: plugin.name })}</p>
  {#if error && hot.length === 0 && latest.length === 0}
    <div class="problem" role="alert">
      <p><Icon name="alert" size={18} />{error}</p>
      <button class="again" onclick={() => (attempt += 1)}>{t('Try Again')}</button>
    </div>
  {:else if loading && hot.length === 0 && latest.length === 0}
    <p class="muted" role="status">{t('Loading…')}</p>
  {:else if hot.length === 0 && latest.length === 0}
    <p class="empty">{t('No Comments Yet')}</p>
  {:else}
    {#if hot.length}
      <h3 class="section">{t('Popular')}</h3>
      {@render list(hot)}
    {/if}
    {#if latest.length}
      <h3 class="section">{t('Latest')}</h3>
      {@render list(latest)}
    {/if}
    {#if next}
      <div class="sentinel" bind:this={sentinel}>{#if loading}<span class="muted">{t('Loading…')}</span>{/if}</div>
    {/if}
  {/if}
</div>

{#snippet list(comments: Comment[])}
  <ul>
    {#each comments as comment, index (comment.id + ':' + index)}
      {@const avatar = avatarOf(comment)}
      <li>
        <span class="avatar">
          {#if avatar}<Art plugin={plugin.id} item={avatar} size={40} radius="list" />{:else}<Icon name="artist" size={20} />{/if}
        </span>
        <div class="body">
          <p class="meta">
            <span class="author">{comment.author}</span>
            {#if comment.time_ms !== null}<span class="time">{when(comment.time_ms)}</span>{/if}
            {#if comment.likes !== null}
              <span class="likes" aria-label={tn('{n} like', '{n} likes', comment.likes)}><Icon name="heart" size={14} />{comment.likes.toLocaleString(language())}</span>
            {/if}
          </p>
          <p class="text">{comment.text}</p>
        </div>
      </li>
    {/each}
  </ul>
{/snippet}

<style>
  .comments {
    padding: 0 var(--jf-space-s2) var(--jf-space-s6);
  }
  .from {
    margin: 0 0 var(--jf-space-s3);
    color: var(--jf-ink-70);
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
  }
  .section {
    margin: var(--jf-space-s4) 0 var(--jf-space-s2);
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: var(--jf-space-s4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: var(--jf-space-s3);
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    overflow: hidden;
    border-radius: 50%;
    background: var(--jf-fill);
    color: var(--jf-ink-70);
  }
  .body {
    min-width: 0;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px var(--jf-space-s3);
    margin: 0 0 2px;
    font-size: calc(13px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .author {
    font-weight: 600;
    color: var(--jf-ink-90);
  }
  .likes {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .text {
    margin: 0;
    white-space: pre-line;
    overflow-wrap: anywhere;
    color: var(--jf-ink-90);
  }
  .empty,
  .muted {
    color: var(--jf-ink-70);
  }
  .problem p {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s2);
  }
  .again {
    min-height: 36px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .sentinel {
    min-height: 24px;
    padding-top: var(--jf-space-s3);
  }
</style>
