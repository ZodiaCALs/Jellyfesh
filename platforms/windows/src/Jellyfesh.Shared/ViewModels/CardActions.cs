using Jellyfesh.Core;

namespace Jellyfesh.ViewModels;

/// <summary>
/// Album and playlist cards and their menus (docs/ui.md, "Rows and cards", v0.0.9): what the play
/// button on the cover and the menu's Play, Shuffle, Play Next, Play Last and Add to Playlist… act
/// on, which of them are offered, and where Go to Artist leads. The views only ask.
/// </summary>
public static class CardActions
{
    /// <summary>
    /// What it plays: a library album, a Jellyfesh playlist, or a source's album or playlist that
    /// plays here; null for anything else (an artist, a folder, a placeholder, or one that can only
    /// open its link).
    /// </summary>
    public static Collection? CollectionOf(object? card) => card switch
    {
        AlbumItem { IsLoaded: true } album => new Collection.Album(album.Id),
        PlaylistItem playlist => new Collection.Playlist(playlist.Id),
        SourceEntryItem { CanPlay: true } entry => new Collection.Source(entry.Item.Source, entry.Item.Id, entry.Item.Kind),
        _ => null,
    };

    /// <summary>
    /// Play Next, Play Last and Add to Playlist… are offered: it is not from a source that plays on
    /// the person's own devices, whose songs are never queued (ADR-0018).
    /// </summary>
    public static bool OffersQueue(SourcesState sources, object? card) =>
        card is not SourceEntryItem entry || !ItemLinks.IsRemoteSource(sources, entry.Item.Source);

    /// <summary>Play Next and Play Last can be chosen: it plays here, and is offered them.</summary>
    public static bool CanQueue(SourcesState sources, object? card) => CollectionOf(card) is not null && OffersQueue(sources, card);

    /// <summary>
    /// Add to Playlist… can be chosen: it can be queued and its songs are listed (a source's playlist
    /// whose songs the service lists only in its own app has none to add).
    /// </summary>
    public static bool CanAddToPlaylist(SourcesState sources, object? card) =>
        CanQueue(sources, card) && card is not SourceEntryItem { SongsListed: false };

    /// <summary>A source's album or playlist: its artist's page, when the source names the artist.</summary>
    public static SourceTarget? SourceArtist(SourceEntryItem? entry) =>
        entry?.Item is { Kind: not (ItemKind.Artist or ItemKind.Folder), ArtistId: { Length: > 0 } artist } item
            ? new SourceTarget(item.Source, artist, string.IsNullOrEmpty(item.Subtitle) ? item.Title : item.Subtitle, ItemKind.Artist)
            : null;

    /// <summary>A library album has an album artist to go to.</summary>
    public static bool HasArtist(AlbumItem? album) => album is { IsLoaded: true, Model.AlbumArtist.Length: > 0 };

    /// <summary>
    /// A library album's artist: the library's artist of its album artist's name, found through the
    /// core's search (as the album page's artist link); null when there is none.
    /// </summary>
    public static async Task<Artist?> ArtistAsync(ShellServices s, AlbumItem? album)
    {
        if (!HasArtist(album))
        {
            return null;
        }

        var name = album!.Model!.AlbumArtist;
        return (await s.QueryAsync(c => TrackPlaces.FindArtist(c, name))).Value;
    }
}
