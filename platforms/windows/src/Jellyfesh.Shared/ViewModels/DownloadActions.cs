using Jellyfesh.Core;

namespace Jellyfesh.ViewModels;

/// <summary>
/// Download (ADR-0025): in the More menus of songs, albums and playlists from a source that allows
/// downloads (its plugin's download capability). The core queues it and the Downloads page shows its
/// progress; while the core cannot download yet, the person is told so in the app's words.
/// </summary>
public static class DownloadActions
{
    /// <summary>The source is on and lets this app download from it.</summary>
    public static bool Offered(SourcesState sources, string plugin) => sources.Find(plugin) is { Download: true, Enabled: true };

    /// <summary>What Download downloads for a song: a source's song not yet on this device.</summary>
    public static DownloadTarget? For(SourcesState sources, TrackItem? song) =>
        song is { IsDownloaded: false, Ref: TrackRef.Source source } && Offered(sources, source.Track.Plugin)
            ? new DownloadTarget.Track(source)
            : null;

    /// <summary>What Download downloads for an album or a playlist (one whose songs the service lists).</summary>
    public static DownloadTarget? For(SourcesState sources, string plugin, string? id, ItemKind? kind, bool songsListed = true) =>
        id is not null && kind is ItemKind.Album or ItemKind.Playlist && songsListed && Offered(sources, plugin)
            ? new DownloadTarget.Collection(plugin, id, kind.Value)
            : null;

    /// <summary>For a source's album or playlist listed in search or browse.</summary>
    public static DownloadTarget? For(SourcesState sources, SourceEntryItem? entry) =>
        entry is null ? null : For(sources, entry.Item.Source, entry.Item.Id, entry.Item.Kind, entry.SongsListed);

    public static void Download(ShellServices s, DownloadTarget target) =>
        s.Send(new CoreCommand.Download(target), message =>
        {
            if (SearchViewModel.IsNotAvailableMessage(message))
            {
                s.Notifications.Info(s.L.Get("Place_Downloads"), s.L.Get("Downloads_NotAvailable"));
            }
            else
            {
                s.Notifications.Error(s.L.Get("Place_Downloads"), message);
            }
        });
}
