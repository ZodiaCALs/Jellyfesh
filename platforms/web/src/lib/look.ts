// The look's layout rules for the page (ADR-0020), as the apps arrange themselves: which bottom
// player shows for the mini-player style, how Now Playing arranges itself for its layout and the
// chosen pane, and the sidebar's places in the look's order. Pure, so they are unit tested.

export type Place = 'Home' | 'Albums' | 'Artists' | 'Songs' | 'Playlists' | 'Favourites' | 'Downloads' | 'Search' | 'Sources';
export type HomeSection = 'Summary' | 'RecentlyAdded' | 'RecentlyPlayed' | 'MostPlayed' | 'Favourites';
export type MiniPlayerStyle = 'Bar' | 'Compact' | 'Floating';
export type NowPlayingLayout = 'Stage' | 'Artwork' | 'Lyrics';
export type BottomPlayer = 'bar' | 'mini' | 'floating';
export type Arrangement = 'stage' | 'cover' | 'wide-pane';

/** Bar: the player bar where the window is wide, else the mini-player; Compact: the mini-player;
 *  Floating: the mini-player floating at the end corner. */
export function bottomPlayer(style: MiniPlayerStyle, wide: boolean): BottomPlayer {
  switch (style) {
    case 'Bar':
      return wide ? 'bar' : 'mini';
    case 'Compact':
      return 'mini';
    case 'Floating':
      return 'floating';
  }
}

/** Stage is v0.0.6. Artwork shows the cover and the sung line in place of the Lyrics pane (Queue
 *  and Info beside the cover, as on the stage). Lyrics puts any pane across the window. */
export function arrangement(layout: NowPlayingLayout, pane: string): Arrangement {
  switch (layout) {
    case 'Stage':
      return 'stage';
    case 'Artwork':
      return pane === 'lyrics' ? 'cover' : 'stage';
    case 'Lyrics':
      return 'wide-pane';
  }
}

/** The page's places in the look's order: those the web has (no Downloads: ADR-0025 §5), the
 *  library ones once when the library is one entry. */
export function placesInOrder<T>(order: Place[], have: Map<Place, T>): T[] {
  const out: T[] = [];
  for (const place of order) {
    const found = have.get(place);
    if (found !== undefined && !out.includes(found)) out.push(found);
  }
  return out;
}

/** Home's sections in the look's order (the web has them all since v0.0.8: hearts and plays are
 *  kept in the browser). */
export function homeSections(order: HomeSection[]): HomeSection[] {
  const known: HomeSection[] = ['Summary', 'RecentlyAdded', 'RecentlyPlayed', 'MostPlayed', 'Favourites'];
  return order.filter((s, i) => known.includes(s) && order.indexOf(s) === i);
}
