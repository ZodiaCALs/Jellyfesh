# ADR-0024: Sources' likes and playlists, and search by source (API 1.3)

* Status: **Accepted**
* Date: 2026-11-22
* Amends: ADR-0022 (API 1.2), [`../ui.md`](../ui.md) (Favourites, Playlists,
  Search)

## Context

For v0.0.8 the owner asked (2026-11-22) for three things:
* to import favourites, likes and playlists from the sources. Asked how, the
  owner chose **live from the source**: the Library shows each signed-in
  source's likes and playlists, refreshed when opened; nothing is copied,
  and any of them can be saved as a Jellyfesh playlist;
* working source playlists, including Play and Shuffle;
* search results grouped by source.

**What the survey found:**
* Plugin API 1.2 has no library interface. Likes and playlists are reachable
  only as browse folders:
  * Jellyfin: Favourites and Playlists;
  * Subsonic: Starred and Playlists;
  * NetEase: Liked Songs and playlists;
  * the community NetEase and QQ Music plugins: Liked Songs and My
    Playlists.
* The apps' Playlists and Favourites pages show local data only, and
  Jellyfesh Web has neither page.
* Search merges the library with the sources that allow merging
  (`content_policy.unified_search`) and groups the results by source. Spotify
  and QQ Music (official) do not allow merging, and no app ever searches
  them.
* Jellyfesh Web runs a search of its own that leaves the library out and
  ignores that flag.

**What the research found (2026-11-22):**
* **Jellyfin 12.4:**
  * favourites: `/Items?isFavorite=true` with `includeItemTypes`;
  * playlists: `/Items?includeItemTypes=Playlist`, with their songs at
    `/Playlists/{id}/Items`;
  * the `/Users/{userId}/…` routes are obsolete.
* **Subsonic:** `getStarred2`, `getPlaylists`, `getPlaylist`.
* **Spotify:**
  * the person's saved tracks (`/me/tracks`) and playlists (`/me/playlists`);
  * a playlist's songs come from `/playlists/{id}/items`, which replaced
    `/tracks`, and only for playlists the person owns or collaborates on.
    Other playlists answer 403, though they can still be played by context;
  * the Developer Policy (§III) and Design Guidelines forbid mixing Spotify
    content with other services': it gets a full row of its own.
* **NetEase:** its official API offers liked songs and collected playlists.
* **QQ Music:** its official API offers neither.

## Decision

1. **API 1.3 adds an optional `library` export** (capability `library`):
   * `likes(kind, page)`: liked tracks, albums or artists. A kind the
     service doesn't keep returns `unsupported`.
   * `playlists(page)`: the person's own and followed playlists, as
     `playlist-entry { item, owned, songs-listed }`.
     * These are new 1.3 records. The 1.0–1.2 `item` stays as it was,
       because component types are checked structurally.
   * A playlist's songs still come from `catalog.browse`, page by page, so
     Play, Shuffle and the playlist page work as for any source container.
   * With `songs-listed` false (a Spotify playlist the person neither owns
     nor collaborates on), the page has no song list. It says "{Service}
     lists this playlist's songs only in {Service}." Play and Shuffle start
     it by context on the person's device, and **Open in {Service}** opens
     it.
   * Read-only in v0.0.8; the owner chose live, not two-way. The heart on a
     source song marks it in Jellyfesh only.
2. **The core:**
   * **A `SourceLibrary` slice.** It covers each enabled, signed-in source
     with the capability: its liked songs, albums and artists, and its
     playlists. Each part holds the first page (up to 100), `total`, a
     cursor, `refreshed_at`, `loading` and `error`.
   * **`Command::RefreshSourceLibrary { plugin }`.** Sent when Favourites or
     Playlists opens, or is returned to after ten minutes. There is no
     background polling.
   * **`Command::SourceLibraryMore { plugin, part, cursor }`** loads more.
   * **A cache on disk,** so the pages show the last lists at once (offline
     too) with their age.
   * **`Command::SavePlaylistCopy { plugin, id, name }`** makes a Jellyfesh
     playlist of the source playlist's songs (as plugin items). It is not
     updated afterwards.
   * Plugins without the export are absent from these pages. Every official
     and community plugin moves to 1.3.
3. **The pages**, web first (ADR-0023):
   * **Favourites:** the songs marked with the heart, then one section per
     source in its own words ("Liked Songs", "Favourites", "Starred"), with
     Songs, Albums and Artists where it keeps them.
   * **Playlists:** Jellyfesh playlists (New Playlist…, Import Playlist…),
     then one section per source. A source playlist opens on its page:
     Play, Shuffle, and More with **Save as Playlist…** and Open in
     {Service}.
   * Spotify's sections are full sections of their own, attributed "Content
     from Spotify", as every Spotify view already is.
   * Both pages refresh when opened. Showing the cache, a quiet line says
     when it was updated ("Updated 5 minutes ago").
4. **Search by source:**
   * The core searches every enabled, signed-in source, including those
     that forbid merging (Spotify, QQ Music). Their results come back in a
     section of their own (`merged: false`), never de-duplicated against
     other sources and never shown beside their rows.
   * The Search page has tabs: **All**, **Library**, and one per source that
     answered.
     * **All:** the library's results, then From Your Sources with one group
       per merging source (as today), then links to the separate sources'
       tabs ("Results from Spotify").
     * **A source's tab:** its Songs, Albums, Artists and Playlists, each
       with **Show More** (`Command::SearchMore { plugin, kind, cursor }`).
   * Each kind is asked for separately (`search-filters.kinds`), 20 per kind
     on the first page. A new query cancels the last one. Results arrive
     source by source, and the 8-second deadline stays.
   * Jellyfesh Web uses the core's rules (through `jf-web`): the library is
     included and merging is respected.

## Consequences

* **Plugins that implement `library`:** Jellyfin, Subsonic, Spotify,
  NetEase, and the community NetEase and QQ Music plugins. QQ Music's
  official plugin doesn't: its API has no likes or playlists.
* **Spotify asks for `user-library-read`, `playlist-read-private` and
  `playlist-read-collaborative`.** Existing sign-ins are asked to sign in
  again. Spotify now also ends a sign-in after six months.
* **Privacy:** the person's likes and playlists come from their own accounts
  into the device's cache and go nowhere else. The privacy page says so.

## Tests

* **WIT and the host:**
  * a golden test for WIT 1.3;
  * host tests for the export's page, item and string limits (the sandbox
    and limit tests);
  * a test plugin with `library`.
* **Core:**
  * the slice's refresh, cache and age;
  * Save as Playlist…;
  * search sections that keep non-merging sources apart, never
    de-duplicated;
  * Show More paging.
* **Plugins:** recorded responses for each service, including Spotify's 403
  for a followed playlist (`songs-listed` false).
* **FFI and web:** FFI contract tests for the new types; web end-to-end tests
  for Favourites, Playlists and the search tabs against the mock servers.
