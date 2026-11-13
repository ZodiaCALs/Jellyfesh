// Translations, gettext style: the English text is the key (as in the Linux app's .po files, whose
// terms the catalogues reuse), placeholders are `{name}`. English needs no catalogue; the others
// load on demand, so a person reading English downloads none of them.

export type Language = 'en' | 'zh-Hans' | 'zh-Hant' | 'ja';

export const LANGUAGES: { id: Language; name: string }[] = [
  { id: 'en', name: 'English' },
  { id: 'zh-Hans', name: '简体中文' },
  { id: 'zh-Hant', name: '繁體中文' },
  { id: 'ja', name: '日本語' },
];

const catalogues: Record<Exclude<Language, 'en'>, () => Promise<{ default: Record<string, string> }>> = {
  'zh-Hans': () => import('./zh-Hans.json'),
  'zh-Hant': () => import('./zh-Hant.json'),
  ja: () => import('./ja.json'),
};

const i18n = $state<{ lang: Language; dict: Record<string, string> }>({ lang: 'en', dict: {} });

/** The language in use. */
export function language(): Language {
  return i18n.lang;
}

/** The best of our languages for the browser's list (`navigator.languages`). */
export function pickLanguage(preferred: readonly string[]): Language {
  for (const tag of preferred) {
    const lower = tag.toLowerCase();
    if (lower.startsWith('ja')) return 'ja';
    if (lower.startsWith('zh')) {
      if (/^zh-(hant|tw|hk|mo)/.test(lower)) return 'zh-Hant';
      return 'zh-Hans';
    }
    if (lower.startsWith('en')) return 'en';
  }
  return 'en';
}

/** Switches the language (`null`: the browser's). */
export async function setLanguage(choice: string | null): Promise<void> {
  const lang: Language =
    choice && LANGUAGES.some((l) => l.id === choice)
      ? (choice as Language)
      : pickLanguage(typeof navigator === 'undefined' ? [] : navigator.languages);
  const dict = lang === 'en' ? {} : (await catalogues[lang]()).default;
  i18n.lang = lang;
  i18n.dict = dict;
  if (typeof document !== 'undefined') document.documentElement.lang = lang;
}

function fill(text: string, args?: Record<string, string | number>): string {
  if (!args) return text;
  return text.replace(/\{(\w+)\}/g, (match, key: string) => (key in args ? String(args[key]) : match));
}

/** `msg` in the current language, with `{placeholders}` filled from `args`. */
export function t(msg: string, args?: Record<string, string | number>): string {
  return fill(i18n.dict[msg] ?? msg, args);
}

/**
 * `msg` in a `context`, gettext's msgctxt, for an English word with two meanings ("Clear" glass,
 * "Clear" the queue): the catalogues key it `context|msg`; English shows `msg`.
 */
export function pt(context: string, msg: string, args?: Record<string, string | number>): string {
  return fill(i18n.dict[`${context}|${msg}`] ?? msg, args);
}

/** A count: `one` or `other` in English (the catalogues key on `one`; CJK has one form). */
export function tn(one: string, other: string, n: number, args?: Record<string, string | number>): string {
  const translated = i18n.dict[one];
  const text = translated ?? (n === 1 ? one : other);
  return fill(text, { n: n.toLocaleString(i18n.lang), ...args });
}

/** A length as m:ss or h:mm:ss. */
export function formatTime(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms) || ms < 0) return '–:––';
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}
