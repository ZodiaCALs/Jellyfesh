<script lang="ts">
  // Settings in the sections every app has (docs/ui.md), one at a time: Appearance (theme, accent
  // colour, transparency, the Now Playing backdrop, lyrics, language), Playback (crossfade, volume
  // levelling, equalizer, streaming quality, startup), Library (music folders), Sources (adding,
  // installing and managing sources) and About (keyboard shortcuts, version, notices). Every change
  // applies at once and goes through the core, which keeps the values in range.
  import Icon from '../components/Icon.svelte';
  import LineSlider from '../components/LineSlider.svelte';
  import LibrarySettings from './LibrarySettings.svelte';
  import ExtensionsSettings from './ExtensionsSettings.svelte';
  import SourcesSettings from './SourcesSettings.svelte';
  import { useApp } from '../lib/app';
  import type { EqSettings, StreamingQuality, Themes, WebSettings } from '../lib/core/types';
  import { LANGUAGES, pt, t } from '../lib/i18n/index.svelte';
  import { cssColour, type Rgba } from '../lib/state/glass';
  import type { SettingsSection } from '../lib/state/nav.svelte';

  let { installPrompt, oninstall }: { installPrompt: boolean; oninstall: () => void } = $props();

  const app = useApp();
  const settings = app.settings;
  const section = $derived(app.nav.settingsSection);
  const SECTION_LABELS: [SettingsSection, string][] = [
    ['appearance', 'Appearance'],
    ['playback', 'Playback'],
    ['library', 'Library'],
    ['sources', 'Sources'],
    ['extensions', 'Extensions'],
    ['about', 'About'],
  ];
  const s = $derived(settings.value);
  const presets = JSON.parse(app.jf.eq_presets()) as [string, EqSettings][];
  const PRESET_LABELS: Record<string, string> = { flat: 'Flat', bass_boost: 'Bass Boost', vocal: 'Vocal', treble: 'Treble' };
  const version = app.jf.version();
  const site = app.jf.website();

  const preset = $derived.by(() => {
    const gains = s.eq.bands.map((b) => b.gain_db).join(',');
    return presets.find(([, eq]) => eq.bands.map((b) => b.gain_db).join(',') === gains)?.[0] ?? 'custom';
  });

  // The accent colours of the swatches and the custom hue's gradient, from the core's themes.
  const SWATCH_HUES = [250, 210, 180, 150, 120, 60, 30, 0, 330, 290];
  const themes = new Map<number, Themes>();
  function themesFor(hue: number): Themes {
    let found = themes.get(hue);
    if (!found) {
      found = JSON.parse(app.jf.theme_for_hue(hue)) as Themes;
      themes.set(hue, found);
    }
    return found;
  }
  function accentOf(hue: number, appearance = app.theme.appearance): string {
    return cssColour(themesFor(hue)[appearance].colors['accent.accent'] as Rgba);
  }
  const swatches = $derived(
    [t('Blue'), t('Cyan'), t('Teal'), t('Green'), t('Olive'), t('Orange'), t('Red'), t('Rose'), t('Magenta'), t('Violet')].map(
      (name, index) => ({ hue: SWATCH_HUES[index], name, colour: accentOf(SWATCH_HUES[index]) }),
    ),
  );
  const hueGradient = $derived(
    `linear-gradient(to right, ${Array.from({ length: 13 }, (_, i) => `${accentOf(i * 30)} ${((i / 12) * 100).toFixed(2)}%`).join(', ')})`,
  );
  const fixedHue = $derived(s.hue_source.kind === 'fixed' ? Math.round(s.hue_source.hue) : null);

  /** Colours for the small previews, in the current hue, for either appearance. */
  function previewStyle(appearance: 'light' | 'dark'): string {
    const colors = app.theme.themes[appearance].colors;
    const css = (name: string) => cssColour(colors[name] as Rgba);
    return [
      `--pv-sky: ${css('field.sky')}`,
      `--pv-cloud: ${css('glass.cloud')}`,
      `--pv-ink: ${css('ink.ink-30')}`,
      `--pv-line: ${css('line.line')}`,
      `--pv-accent: ${css('accent.accent')}`,
      `--pv-aurora-1: ${css('field.aurora-1')}`,
      `--pv-aurora-2: ${css('field.aurora-2')}`,
    ].join('; ');
  }

  /** What a theme tile shows: the System tile both appearances, split. */
  function looksOf(choice: string): ('light' | 'dark')[] {
    return choice === 'dark' ? ['dark'] : choice === 'light' ? ['light'] : ['light', 'dark'];
  }

  function set(change: (draft: WebSettings) => void) {
    settings.update(change);
  }

  function value(event: Event): string {
    return (event.currentTarget as HTMLInputElement | HTMLSelectElement).value;
  }

  function checked(event: Event): boolean {
    return (event.currentTarget as HTMLInputElement).checked;
  }

  const SHORTCUTS: [string, string][] = [
    ['Space', 'Play or Pause'],
    ['Ctrl + →', 'Next Song'],
    ['Ctrl + ←', 'Previous Song'],
    ['Shift + →', 'Skip Forward 5 Seconds'],
    ['Shift + ←', 'Skip Back 5 Seconds'],
    ['Ctrl + ↑', 'Volume Up'],
    ['Ctrl + ↓', 'Volume Down'],
    ['Ctrl + M', 'Mute'],
    ['Ctrl + S', 'Shuffle'],
    ['Ctrl + R', 'Repeat'],
    ['Ctrl + P', 'Now Playing'],
    ['Ctrl + L', 'Lyrics'],
    ['Ctrl + U', 'Queue'],
    ['Ctrl + F', 'Search'],
    ['Esc', 'Back'],
  ];
</script>

{#snippet toggle(label: string, help: string | null, on: boolean, change: (on: boolean) => void, disabled: boolean = false)}
  <label class="row" class:off={disabled}>
    <span class="text">
      <span class="label">{label}</span>
      {#if help}<span class="help">{help}</span>{/if}
    </span>
    <span class="switch">
      <input type="checkbox" role="switch" checked={on} {disabled} onchange={(event) => change(checked(event))} />
      <span aria-hidden="true"></span>
    </span>
  </label>
{/snippet}

{#snippet segmented(name: string, label: string, options: [string, string][], current: string, change: (value: string) => void)}
  <div class="row">
    <span class="text"><span class="label" id="seg-{name}">{label}</span></span>
    <div class="segmented" role="radiogroup" aria-labelledby="seg-{name}">
      {#each options as [id, text] (id)}
        <label class:on={current === id}>
          <input type="radio" name="seg-{name}" value={id} checked={current === id} onchange={() => change(id)} />
          <span>{text}</span>
        </label>
      {/each}
    </div>
  </div>
{/snippet}

<section class="page prefs" aria-labelledby="prefs-title">
  <h1 id="prefs-title" class="page-title">{t('Settings')}</h1>

  <div class="sections" role="tablist" aria-label={t('Settings')}>
    {#each SECTION_LABELS as [id, label] (id)}
      <button
        role="tab"
        id="tab-{id}"
        aria-selected={section === id}
        aria-controls="panel-{id}"
        class:on={section === id}
        onclick={() => app.nav.showSection(id)}>{t(label)}</button
      >
    {/each}
  </div>

  <div class="panel" role="tabpanel" id="panel-{section}" aria-labelledby="tab-{section}">
  {#if section === 'appearance'}
  <section class="group" aria-labelledby="g-theme">
    <h2 id="g-theme" class="section">{t('Theme')}</h2>
    <div class="card">
      <div class="row stacked">
        <span class="text"><span class="help">{t('Light, dark, or the same as your system.')}</span></span>
        <div class="tiles" role="radiogroup" aria-labelledby="g-theme">
          {#each [['system', t('System')], ['light', t('Light')], ['dark', t('Dark')]] as [id, name] (id)}
            <label class="tile" class:on={s.appearance === id}>
              <input
                type="radio"
                name="theme"
                value={id}
                checked={s.appearance === id}
                onchange={() => set((d) => (d.appearance = id as WebSettings['appearance']))}
              />
              <span class="preview window" aria-hidden="true">
                {#each looksOf(id) as look (look)}
                  <span class="look {look}" style={previewStyle(look)}>
                    <span class="pv-side"></span>
                    <span class="pv-title"></span>
                    <span class="pv-line"></span>
                    <span class="pv-line short"></span>
                    <span class="pv-dot"></span>
                  </span>
                {/each}
              </span>
              <span class="name">{name}</span>
            </label>
          {/each}
        </div>
      </div>
    </div>

  </section>
  <section class="group" aria-labelledby="g-accent">
    <h2 id="g-accent" class="section">{t('Accent Colour')}</h2>
    <div class="card">
      {@render toggle(t('Follow the Artwork'), t('The accent and the field take the hue of what plays.'), s.hue_source.kind === 'artwork', (on) =>
        set((d) => (d.hue_source = on ? { kind: 'artwork' } : { kind: 'fixed', hue: Math.round(app.theme.themes.hue) })),
      )}
      {#if fixedHue !== null}
        <div class="row stacked">
          <span class="text"><span class="label" id="accent-label">{t('Colour')}</span></span>
          <div class="swatches" role="radiogroup" aria-labelledby="accent-label">
            {#each swatches as swatch (swatch.hue)}
              <label class="swatch" style:--swatch={swatch.colour} title={swatch.name}>
                <input
                  type="radio"
                  name="accent"
                  value={swatch.hue}
                  aria-label={swatch.name}
                  checked={fixedHue === swatch.hue}
                  onchange={() => set((d) => (d.hue_source = { kind: 'fixed', hue: swatch.hue }))}
                />
                <span class="dot" aria-hidden="true"><Icon name="check" size={16} /></span>
              </label>
            {/each}
          </div>
        </div>
        <div class="row">
          <label class="text" for="hue"><span class="label">{t('Custom')}</span></label>
          <span class="slider hue">
            <LineSlider
              id="hue"
              value={fixedHue}
              min={0}
              max={359}
              step={1}
              label={t('Custom Hue')}
              valuetext={t('{hue} degrees', { hue: fixedHue })}
              gradient={hueGradient}
              thumb="var(--jf-accent)"
              oninput={(hue) => set((d) => (d.hue_source = { kind: 'fixed', hue }))}
            />
            <span class="accent-preview" aria-hidden="true">
              <span class="pv-play"><Icon name="play" size={14} /></span>
              <span class="pv-pill">Aa</span>
            </span>
          </span>
        </div>
      {/if}
    </div>

  </section>
  <section class="group" aria-labelledby="g-transparency">
    <h2 id="g-transparency" class="section">{t('Transparency')}</h2>
    <div class="card">
      <div class="row stacked">
        <span class="text">
          <label class="sr-only" for="transparency">{t('Transparency')}</label>
          <span class="help">
            {s.reduce_effects
              ? t('Glass is solid while Reduce Motion and Transparency is on.')
              : t('How much of what lies behind shows through glass.')}
          </span>
        </span>
        <div class="transparency">
          <span class="glass-preview" aria-hidden="true" style={previewStyle(app.theme.appearance)}>
            <span class="blob one"></span>
            <span class="blob two"></span>
            <span class="pane"><span class="pv-line"></span><span class="pv-line short"></span></span>
          </span>
          <div class="ends">
            <span>{t('Solid')}</span>
            <LineSlider
              id="transparency"
              value={Math.round(s.transparency * 100)}
              min={0}
              max={100}
              step={1}
              plain
              disabled={s.reduce_effects}
              valuetext="{Math.round(s.transparency * 100)} %"
              oninput={(percent) => set((d) => (d.transparency = percent / 100))}
            />
            <span>{pt('glass', 'Clear')}</span>
            <output for="transparency">{Math.round(s.transparency * 100)} %</output>
          </div>
        </div>
      </div>
      {@render toggle(t('Reduce Motion and Transparency'), t('Solid surfaces instead of glass, and shorter motion.'), s.reduce_effects, (on) =>
        set((d) => (d.reduce_effects = on)),
      )}
    </div>

  </section>
  <section class="group" aria-labelledby="g-backdrop">
    <h2 id="g-backdrop" class="section">{t('Now Playing Backdrop')}</h2>
    <div class="card">
      <div class="row stacked">
        <span class="text">
          <span class="sr-only" id="background-label">{t('Now Playing Backdrop')}</span>
          <span class="help">{t('Artwork blurs the album cover behind Now Playing; Misty uses the Misty field in its colour.')}</span>
        </span>
        <div class="tiles" role="radiogroup" aria-labelledby="background-label">
          {#each [['artwork', t('Artwork')], ['misty', t('Misty')]] as [id, name] (id)}
            <label class="tile" class:on={s.style === id}>
              <input
                type="radio"
                name="background"
                value={id}
                checked={s.style === id}
                onchange={() => set((d) => (d.style = id as WebSettings['style']))}
              />
              <span class="preview backdrop {id}" aria-hidden="true" style={previewStyle(app.theme.appearance)}>
                {#if id === 'artwork' && app.theme.backdrop}<img src={app.theme.backdrop} alt="" />{/if}
                <span class="pv-cover"></span>
              </span>
              <span class="name">{name}</span>
            </label>
          {/each}
        </div>
      </div>
    </div>

  </section>
  <section class="group" aria-labelledby="g-lyrics">
    <h2 id="g-lyrics" class="section">{t('Lyrics')}</h2>
    <div class="card">
      {@render segmented(
        'lyrics-size',
        t('Lyrics Size'),
        [
          ['small', t('Small')],
          ['medium', t('Medium')],
          ['large', t('Large')],
        ],
        s.lyrics_size,
        (v) => set((d) => (d.lyrics_size = v as WebSettings['lyrics_size'])),
      )}
      {@render segmented(
        'lyrics-align',
        t('Lyrics Alignment'),
        [
          ['start', t('Start')],
          ['center', t('Centre')],
        ],
        s.lyrics_align,
        (v) => set((d) => (d.lyrics_align = v as WebSettings['lyrics_align'])),
      )}
      {@render toggle(
        t('Show Translations'),
        t('A line in another language under each lyric line, when the lyrics have one.'),
        s.show_translation,
        (on) => set((d) => (d.show_translation = on)),
      )}
      {@render toggle(
        t('Show Romanization'),
        t('How each line reads in Latin letters, when the lyrics have it.'),
        s.show_romanization,
        (on) => set((d) => (d.show_romanization = on)),
      )}
      {@render toggle(
        t('Show Lyrics Bar'),
        t('The line being sung, under the song’s title in the player bar.'),
        s.show_lyrics_bar,
        (on) => set((d) => (d.show_lyrics_bar = on)),
      )}
      {@render toggle(
        t('Desktop Lyrics'),
        app.desktopLyrics.supported
          ? t('The line being sung and the next one, in a window that stays above other apps.')
          : t('This browser can’t keep a window above other apps. Chrome and Edge on a computer can.'),
        s.desktop_lyrics && app.desktopLyrics.supported,
        (on) => set((d) => (d.desktop_lyrics = on)),
        !app.desktopLyrics.supported,
      )}
    </div>
  </section>
  {#if app.miniPlayer.supported}
    <!-- The mini player is a picture-in-picture window: always above other windows, closed with
         the page, so Keep on Top and Open When the Window Is Closed are not offered here. -->
    <section class="group" aria-labelledby="g-mini-player">
      <h2 id="g-mini-player" class="section">{t('Mini Player')}</h2>
      <div class="card">
        {@render toggle(
          t('Show Lyrics Line'),
          t('The line being sung, under the song in the mini player.'),
          s.mini_player.show_lyrics_line,
          (on) => set((d) => (d.mini_player.show_lyrics_line = on)),
        )}
      </div>
    </section>
  {/if}
  <section class="group" aria-labelledby="g-language">
    <h2 id="g-language" class="section">{t('Language')}</h2>
    <div class="card">
      <div class="row">
        <label class="text" for="language"><span class="help">{t('The language Jellyfesh Web is shown in.')}</span></label>
        <select id="language" value={s.language ?? ''} onchange={(event) => set((d) => (d.language = value(event) || null))}>
          <option value="">{t('System')}</option>
          {#each LANGUAGES as language (language.id)}
            <option value={language.id} lang={language.id}>{language.name}</option>
          {/each}
        </select>
      </div>
    </div>
  </section>
  {:else if section === 'playback'}
  <section class="group" aria-labelledby="g-crossfade">
    <h2 id="g-crossfade" class="section">{t('Crossfade')}</h2>
    <div class="card">
      {@render toggle(t('Crossfade'), t('Blend the end of a song into the next.'), s.crossfade.enabled, (on) =>
        set((d) => (d.crossfade.enabled = on)),
      )}
      {#if s.crossfade.enabled}
        <div class="row">
          <label class="text" for="xfade"><span class="label">{t('Length')}</span></label>
          <span class="slider">
            <LineSlider
              id="xfade"
              value={Math.round(s.crossfade.duration_ms / 1000)}
              min={1}
              max={12}
              step={1}
              plain
              onchange={(seconds) => set((d) => (d.crossfade.duration_ms = seconds * 1000))}
            />
            <output for="xfade">{Math.round(s.crossfade.duration_ms / 1000)}</output>
          </span>
        </div>
        {@render toggle(t('Album Continuity'), t('Songs of the same album join without a fade.'), s.crossfade.album_continuity, (on) =>
          set((d) => (d.crossfade.album_continuity = on)),
        )}
      {/if}
    </div>
  </section>
  <section class="group" aria-labelledby="g-levelling">
    <h2 id="g-levelling" class="section">{t('Volume Levelling')}</h2>
    <p class="note">{t('Evens out loudness using the ReplayGain tags in your files.')}</p>
    <div class="card">
      {@render segmented(
        'replaygain',
        t('ReplayGain'),
        [
          ['Off', t('Off')],
          ['Track', t('Track')],
          ['Album', t('Album')],
        ],
        s.replaygain_mode,
        (v) => set((d) => (d.replaygain_mode = v as WebSettings['replaygain_mode'])),
      )}
      <div class="row">
        <label class="text" for="preamp">
          <span class="label">{t('Preamp')}</span>
          <span class="help">{t('Decibels added to every song')}</span>
        </label>
        <span class="slider">
          <LineSlider
            id="preamp"
            value={s.preamp_db}
            min={-15}
            max={15}
            step={0.5}
            plain
            valuetext="{s.preamp_db.toFixed(1)} dB"
            onchange={(db) => set((d) => (d.preamp_db = db))}
          />
          <output for="preamp">{s.preamp_db > 0 ? '+' : ''}{s.preamp_db.toFixed(1)} dB</output>
        </span>
      </div>
    </div>
  </section>
  <section class="group" aria-labelledby="g-eq">
    <h2 id="g-eq" class="section">{t('Equalizer')}</h2>
    <div class="card">
      {@render toggle(t('Equalizer'), null, s.eq.enabled, (on) => set((d) => (d.eq.enabled = on)))}
      <div class="row">
        <label class="text" for="preset"><span class="label">{t('Preset')}</span></label>
        <select
          id="preset"
          value={preset}
          onchange={(event) => {
            const chosen = presets.find(([name]) => name === value(event));
            if (chosen) set((d) => (d.eq = { ...chosen[1], enabled: d.eq.enabled }));
          }}
        >
          {#each presets as [name] (name)}
            <option value={name}>{t(PRESET_LABELS[name] ?? name)}</option>
          {/each}
          {#if preset === 'custom'}<option value="custom">{t('Custom')}</option>{/if}
        </select>
      </div>
      <div class="row">
        <label class="text" for="eq-preamp"><span class="label">{t('Preamp')}</span></label>
        <span class="slider">
          <LineSlider
            id="eq-preamp"
            value={s.eq.preamp_db}
            min={-12}
            max={12}
            step={0.5}
            plain
            disabled={!s.eq.enabled}
            valuetext="{s.eq.preamp_db.toFixed(1)} dB"
            onchange={(db) => set((d) => (d.eq.preamp_db = db))}
          />
          <output for="eq-preamp">{s.eq.preamp_db > 0 ? '+' : ''}{s.eq.preamp_db.toFixed(1)} dB</output>
        </span>
      </div>
      <div class="bands" class:off={!s.eq.enabled}>
        {#each s.eq.bands as band, index (index)}
          <label class="band">
            <input
              type="range"
              min="-12"
              max="12"
              step="0.5"
              value={band.gain_db}
              aria-label={t('{freq} Hz', { freq: band.freq_hz >= 1000 ? `${band.freq_hz / 1000}k` : band.freq_hz })}
              aria-valuetext="{band.gain_db.toFixed(1)} dB"
              disabled={!s.eq.enabled}
              onchange={(event) => set((d) => (d.eq.bands[index].gain_db = Number(value(event))))}
            />
            <span>{band.freq_hz >= 1000 ? `${band.freq_hz / 1000}k` : band.freq_hz}</span>
          </label>
        {/each}
      </div>
      <div class="row end">
        <button
          class="button"
          disabled={!s.eq.enabled}
          onclick={() => {
            const flat = presets.find(([name]) => name === 'flat');
            if (flat) set((d) => (d.eq = { ...flat[1], enabled: d.eq.enabled }));
          }}>{t('Reset to Flat')}</button
        >
      </div>
    </div>
  </section>
  <section class="group" aria-labelledby="g-streaming">
    <h2 id="g-streaming" class="section">{t('Streaming')}</h2>
    <div class="card">
      <div class="row">
        <label class="text" for="quality">
          <span class="label">{t('Streaming Quality')}</span>
          <span class="help"
            >{t('Auto asks for lossless, or High on mobile data or with Data Saver on. Jellyfesh steps down when this browser cannot play a format.')}</span
          >
        </label>
        <select
          id="quality"
          value={s.streaming_quality}
          onchange={(event) => set((d) => (d.streaming_quality = value(event) as StreamingQuality))}
        >
          <option value="auto">{t('Auto')}</option>
          <option value="lossless">{t('Lossless')}</option>
          <option value="high">{t('High')}</option>
          <option value="normal">{t('Normal')}</option>
          <option value="low">{t('Low')}</option>
        </select>
      </div>
    </div>
  </section>
  <section class="group" aria-labelledby="g-startup">
    <h2 id="g-startup" class="section">{t('Startup')}</h2>
    <div class="card">
      {@render toggle(
        t('Resume Playback'),
        t('Start playing where you left off when Jellyfesh opens.'),
        s.resume_on_launch,
        (on) => set((d) => (d.resume_on_launch = on)),
      )}
    </div>
  </section>
  {:else if section === 'library'}
  <LibrarySettings />
  {:else if section === 'sources'}
  <SourcesSettings />
  {:else if section === 'extensions'}
  <ExtensionsSettings />
  {:else}
  <section class="group" aria-labelledby="g-keys">
    <h2 id="g-keys" class="section">{t('Keyboard Shortcuts')}</h2>
    <div class="card padded">
      <dl class="keys">
        {#each SHORTCUTS as [keys, action] (keys)}
          <dt><kbd>{keys}</kbd></dt>
          <dd>{t(action)}</dd>
        {/each}
      </dl>
    </div>
  </section>
  <section class="group about" aria-labelledby="g-about">
    <h2 id="g-about" class="section">{t('About Jellyfesh')}</h2>
    <div class="card padded">
      <p>{t('Version {version}', { version })} · {t('An elegant, lightweight music player.')}</p>
      <p>{t('Designed with the Misty design language.')}</p>
      <p><a href="THIRD_PARTY_NOTICES.txt" target="_blank" rel="noopener">{t('Third-party notices')}</a></p>
      {#if site}
        <p class="links">
          <a href={site} target="_blank" rel="noopener">{t('Website')}</a>
          <a href="{site}legal/eula.html" target="_blank" rel="noopener">{t('Licence Agreement')}</a>
          <a href="{site}legal/privacy.html" target="_blank" rel="noopener">{t('Privacy Policy')}</a>
        </p>
      {/if}
      {#if installPrompt}
        <div class="row">
          <span class="text">
            <span class="label">{t('Install Jellyfesh Web')}</span>
            <span class="help">{t('Open it from your apps, in its own window.')}</span>
          </span>
          <button class="button" onclick={oninstall}><Icon name="install" size={18} />{t('Install')}</button>
        </div>
      {/if}
    </div>
  </section>
  {/if}
  </div>
</section>

<style>
  .prefs {
    max-width: var(--jf-layout-reading);
  }
  .sections {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: var(--jf-space-s6);
    padding: 4px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
  }
  .sections button {
    flex: 1 1 auto;
    min-height: 36px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: transparent;
    color: var(--jf-ink-70);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .sections button.on {
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    box-shadow: var(--jf-elevation-shadow-1);
  }
  /* Phones: one row that scrolls sideways, rather than two rows of unequal pills. */
  @media (max-width: 760px) {
    .sections {
      flex-wrap: nowrap;
      overflow-x: auto;
      scrollbar-width: none;
      scroll-padding-inline: 4px;
    }
    .sections::-webkit-scrollbar {
      display: none;
    }
    .sections button {
      flex: none;
      padding: 0 var(--jf-space-s4);
    }
  }
  .sections button:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  .row.end {
    justify-content: flex-end;
  }
  /* Luminous (v0.0.6): each group is a card of Mist glass with its heading inside, without a
     blur layer of its own, so it shows the transparency setting at once. */
  .group {
    margin-bottom: var(--jf-space-s5);
    padding: var(--jf-space-s5) var(--jf-space-s6) var(--jf-space-s2);
    border-radius: var(--jf-radius-r-2xl);
    background: var(--jf-tint-mist);
    box-shadow:
      inset 0 0 0 1px var(--jf-glass-edge-soft),
      var(--jf-elevation-shadow-1);
  }
  .group > :global(.section) {
    margin-bottom: var(--jf-space-s1);
  }
  .card + .card {
    margin-top: var(--jf-space-s3);
  }
  .card.padded {
    padding: var(--jf-space-s3) 0 var(--jf-space-s4);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--jf-space-s3);
    min-height: 60px;
    padding: var(--jf-space-s3) 0;
  }
  .row + .row,
  .row + .bands {
    border-top: 1px solid var(--jf-line-soft);
  }
  .row.stacked {
    flex-direction: column;
    align-items: stretch;
  }
  .row.stacked .text {
    flex: none;
  }
  .text {
    flex: 1 1 220px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .label {
    font-weight: 600;
  }
  .help,
  .note {
    font-size: calc(13px * var(--jf-text-scale, 1));
    line-height: 1.45;
    color: var(--jf-ink-70);
  }
  .note {
    margin: calc(-1 * var(--jf-space-s2)) 0 var(--jf-space-s3);
  }
  /* Radio groups: native radios (arrow keys move), drawn as a segmented control, tiles or swatches;
     each invisible radio covers its whole option, so a click anywhere on it lands on the radio. */
  input[type='radio'] {
    position: absolute;
    inset: 0;
    z-index: 1;
    width: 100%;
    height: 100%;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .segmented {
    display: inline-flex;
    flex-wrap: wrap;
    padding: 3px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill);
    box-shadow: inset 0 0 0 1px var(--jf-line-soft);
  }
  .segmented label {
    position: relative;
    display: grid;
    place-items: center;
    min-height: 36px;
    padding: 0 var(--jf-space-s4);
    border-radius: var(--jf-radius-r-full);
    color: var(--jf-ink-70);
    font-size: calc(14px * var(--jf-text-scale, 1));
    font-weight: 600;
    cursor: pointer;
    transition:
      background-color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid),
      color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .segmented label.on {
    background: var(--jf-cloud);
    color: var(--jf-ink-90);
    box-shadow: var(--jf-elevation-shadow-1);
  }
  .segmented label:has(input:focus-visible),
  .tile:has(input:focus-visible) .preview,
  .swatch:has(input:focus-visible) .dot {
    box-shadow: var(--jf-elevation-focus);
  }
  .tiles {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s3);
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--jf-space-s2);
    cursor: pointer;
  }
  .tile .name {
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-70);
  }
  .tile.on .name {
    color: var(--jf-accent-text);
  }
  .preview {
    position: relative;
    display: block;
    width: 112px;
    height: 72px;
    overflow: hidden;
    border-radius: var(--jf-radius-r-lg);
    box-shadow: inset 0 0 0 1px var(--jf-line);
    transition: box-shadow var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .tile.on .preview {
    box-shadow:
      inset 0 0 0 1px var(--jf-line),
      0 0 0 2px var(--jf-sky),
      0 0 0 4px var(--jf-accent);
  }
  /* A small window in each appearance (the System tile shows both, split). */
  .look {
    position: absolute;
    inset: 0;
    background: var(--pv-sky);
  }
  .look.dark:not(:first-child) {
    clip-path: polygon(62% 0, 100% 0, 100% 100%, 38% 100%);
  }
  .pv-side {
    position: absolute;
    inset: 0 auto 0 0;
    width: 28%;
    background: var(--pv-cloud);
    box-shadow: inset -1px 0 0 var(--pv-line);
  }
  .pv-title,
  .pv-line {
    position: absolute;
    left: 36%;
    height: 6px;
    border-radius: 3px;
    background: var(--pv-ink);
  }
  .pv-title {
    top: 14px;
    width: 34%;
    height: 8px;
    border-radius: 4px;
  }
  .look .pv-line {
    top: 32px;
    width: 52%;
  }
  .look .pv-line.short {
    top: 44px;
    width: 30%;
  }
  .pv-dot {
    position: absolute;
    right: 10%;
    bottom: 10px;
    width: 14px;
    height: 14px;
    border-radius: var(--jf-radius-r-full);
    background: var(--pv-accent);
  }
  /* Now Playing backgrounds: the baked backdrop (or a stand-in), or the Misty field. */
  .backdrop {
    background:
      radial-gradient(70% 80% at 15% 10%, var(--pv-aurora-1) 0%, transparent 70%),
      radial-gradient(70% 80% at 90% 90%, var(--pv-aurora-2) 0%, transparent 70%),
      var(--pv-sky);
  }
  .backdrop.artwork {
    background: linear-gradient(135deg, var(--pv-accent), var(--pv-aurora-1));
  }
  .backdrop img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .pv-cover {
    position: absolute;
    left: 12px;
    top: 50%;
    width: 36px;
    height: 36px;
    margin-top: -18px;
    border-radius: 6px;
    background: linear-gradient(135deg, var(--pv-accent), var(--pv-aurora-2));
    box-shadow: 0 6px 12px -6px rgb(0 0 0 / 0.4);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
    margin: 0 -6px;
  }
  .swatch {
    position: relative;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    cursor: pointer;
  }
  .dot {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--jf-radius-r-full);
    background: var(--swatch);
    color: var(--jf-on-accent);
    box-shadow: inset 0 0 0 1px var(--jf-line);
    transition: box-shadow var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .dot :global(svg) {
    opacity: 0;
  }
  .swatch:has(input:checked) .dot {
    box-shadow:
      0 0 0 2px var(--jf-sky),
      0 0 0 4px var(--swatch);
  }
  .swatch:has(input:checked) .dot :global(svg) {
    opacity: 1;
  }
  .slider {
    flex: 1 1 240px;
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    min-width: 0;
  }
  .accent-preview {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pv-play {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent);
    color: var(--jf-on-accent);
  }
  .pv-pill {
    padding: 4px 10px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 700;
  }
  .transparency {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: var(--jf-space-s4);
  }
  /* Glass over a colourful field: the tints are the theme's, so it follows the slider at once. */
  .glass-preview {
    position: relative;
    flex: none;
    width: 112px;
    height: 72px;
    overflow: hidden;
    border-radius: var(--jf-radius-r-lg);
    background: var(--pv-sky);
    box-shadow: inset 0 0 0 1px var(--jf-line);
  }
  .blob {
    position: absolute;
    border-radius: var(--jf-radius-r-full);
  }
  .blob.one {
    left: 8px;
    top: 10px;
    width: 46px;
    height: 46px;
    background: var(--pv-accent);
  }
  .blob.two {
    right: 6px;
    bottom: -14px;
    width: 58px;
    height: 58px;
    background: var(--pv-aurora-2);
  }
  .pane {
    position: absolute;
    inset: 14px 14px 14px 30px;
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-tint-veil);
    backdrop-filter: blur(6px) saturate(var(--jf-glass-sat));
    box-shadow: inset 0 0 0 1px var(--jf-glass-edge-soft);
  }
  /* Phones: the tab bar and the mini-player take the two live blur layers there. */
  @media (max-width: 760px) {
    .pane {
      backdrop-filter: none;
    }
  }
  .pane .pv-line {
    top: 12px;
    left: 10px;
    width: 50%;
    background: var(--jf-ink-30);
  }
  .pane .pv-line.short {
    top: 24px;
    width: 30%;
  }
  .ends {
    display: flex;
    align-items: center;
    gap: var(--jf-space-s3);
    min-width: 0;
    font-size: calc(13px * var(--jf-text-scale, 1));
    font-weight: 600;
    color: var(--jf-ink-70);
  }
  button:focus-visible,
  select:focus-visible,
  input:focus-visible {
    outline: none;
    box-shadow: var(--jf-elevation-focus);
  }
  select {
    min-height: 40px;
    padding: 0 var(--jf-space-s3);
    border: 0;
    border-radius: var(--jf-radius-r-md);
    background: var(--jf-fill);
    color: var(--jf-ink-90);
    font: inherit;
    box-shadow: inset 0 0 0 1px var(--jf-line);
  }
  output {
    flex: none;
    min-width: 56px;
    text-align: end;
    font-variant-numeric: tabular-nums;
    color: var(--jf-ink-70);
  }
  .bands {
    display: grid;
    grid-template-columns: repeat(10, minmax(0, 1fr));
    gap: var(--jf-space-s1);
    padding: var(--jf-space-s4) 0;
  }
  .bands input {
    accent-color: var(--jf-accent);
  }
  .bands.off {
    opacity: 0.5;
  }
  .band {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--jf-space-s2);
    font-size: calc(11px * var(--jf-text-scale, 1));
    color: var(--jf-ink-70);
  }
  .band input {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 28px;
    height: 120px;
  }
  .switch {
    position: relative;
    display: inline-block;
    flex: none;
    width: 52px;
    height: 32px;
  }
  .switch input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .switch span {
    position: absolute;
    inset: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-fill-press);
    box-shadow: inset 0 0 0 1px var(--jf-line);
    pointer-events: none;
    transition: background-color var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .switch span::after {
    content: '';
    position: absolute;
    top: 4px;
    left: 4px;
    width: 24px;
    height: 24px;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-cloud);
    box-shadow: var(--jf-elevation-shadow-1);
    transition: translate var(--jf-motion-dur-1) var(--jf-motion-ease-fluid);
  }
  .switch input:checked + span {
    background: var(--jf-accent);
  }
  .switch input:checked + span::after {
    translate: 20px 0;
  }
  .switch input:focus-visible + span {
    box-shadow: var(--jf-elevation-focus);
  }
  /* A switch the browser cannot honour (Desktop Lyrics without Picture-in-Picture). */
  .row.off .label,
  .switch input:disabled + span {
    opacity: 0.45;
  }
  .switch input:disabled {
    cursor: default;
  }
  .button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 0 var(--jf-space-s4);
    border: 0;
    border-radius: var(--jf-radius-r-full);
    background: var(--jf-accent-soft);
    color: var(--jf-accent-text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .keys {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--jf-space-s2) var(--jf-space-s5);
    margin: 0;
  }
  .keys dd {
    margin: 0;
    color: var(--jf-ink-70);
  }
  kbd {
    padding: 2px 8px;
    border-radius: var(--jf-radius-r-sm);
    background: var(--jf-fill);
    box-shadow: inset 0 0 0 1px var(--jf-line);
    font-family: var(--jf-font-mono);
    font-size: calc(12px * var(--jf-text-scale, 1));
  }
  .about p {
    margin: 0 0 var(--jf-space-s2);
    color: var(--jf-ink-70);
  }
  .about p:last-child {
    margin-bottom: 0;
  }
  .about a {
    color: var(--jf-accent-text);
    font-weight: 600;
    text-decoration: none;
    border-radius: var(--jf-radius-r-sm);
  }
  .about .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--jf-space-s4);
  }
  .about a:hover {
    text-decoration: underline;
  }
</style>
