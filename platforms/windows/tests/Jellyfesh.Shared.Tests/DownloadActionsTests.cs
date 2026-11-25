using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Download in the More menus and the Downloaded mark (ADR-0025; docs/ui.md "Album, artist and playlist pages").</summary>
public class DownloadActionsTests
{
    [Fact]
    public void Download_is_offered_for_sources_that_allow_it()
    {
        var sources = Sources();
        var song = Song("org.jellyfin", "s1");
        Assert.Equal(new DownloadTarget.Track(song.Ref!), DownloadActions.For(sources, song));
        Assert.Null(DownloadActions.For(sources, Song("org.spotify", "s2"))); // never Spotify
        Assert.Null(DownloadActions.For(sources, Song("org.off", "s3"))); // a source that is off
        song.IsDownloaded = true;
        Assert.Null(DownloadActions.For(sources, song)); // already on this device

        var library = new TrackItem(0);
        library.Load(new TrackRow(new TrackRef.Library(7), "Lantern", "Band", null, 180_000, null, null, false, false));
        Assert.Null(DownloadActions.For(sources, library));

        Assert.Equal(new DownloadTarget.Collection("org.jellyfin", "a1", ItemKind.Album), DownloadActions.For(sources, "org.jellyfin", "a1", ItemKind.Album));
        Assert.Equal(new DownloadTarget.Collection("org.jellyfin", "p1", ItemKind.Playlist), DownloadActions.For(sources, "org.jellyfin", "p1", ItemKind.Playlist));
        Assert.Null(DownloadActions.For(sources, "org.jellyfin", "p2", ItemKind.Playlist, songsListed: false));
        Assert.Null(DownloadActions.For(sources, "org.jellyfin", "ar1", ItemKind.Artist));
        Assert.Null(DownloadActions.For(sources, "org.jellyfin", null, null));
    }

    [Fact]
    public void A_source_album_page_offers_Download_in_More()
    {
        using var h = new AppHarness(English.Instance);
        h.Core.State = CoreSnapshot.Initial with { Sources = Sources() };
        h.Started();
        var album = h.App.CreateSourceBrowse(new SourceTarget("org.jellyfin", "a1", "Night Swim", ItemKind.Album));
        Assert.True(album.CanDownload);
        Assert.True(album.HasMore);
        album.DownloadCommand.Execute(null);
        Assert.Equal(new CoreCommand.Download(new DownloadTarget.Collection("org.jellyfin", "a1", ItemKind.Album)), h.Core.Sent[^1]);

        Assert.False(h.App.CreateSourceBrowse(new SourceTarget("org.spotify", "a1", "Night Swim", ItemKind.Album)).CanDownload);
    }

    [Fact]
    public void A_download_the_core_cannot_make_yet_says_so()
    {
        using var h = new AppHarness(English.Instance);
        h.Core.State = CoreSnapshot.Initial with { Sources = Sources() };
        h.Started();
        DownloadActions.Download(h.S, new DownloadTarget.Track(Song("org.jellyfin", "s1").Ref!));
        h.Emit(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "downloads are not available in this build yet"));
        var notice = Assert.Single(h.App.Notifications.Items);
        Assert.Equal((NotificationSeverity.Informational, "Downloads", "Downloads aren't available in this build yet."), (notice.Severity, notice.Title, notice.Message));

        // Another failure is shown as it is, under Downloads.
        DownloadActions.Download(h.S, new DownloadTarget.Track(Song("org.jellyfin", "s2").Ref!));
        h.Emit(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "the disk is full"));
        Assert.Equal((NotificationSeverity.Error, "the disk is full"), (h.App.Notifications.Items[^1].Severity, h.App.Notifications.Items[^1].Message));
    }

    [Fact]
    public void Downloaded_songs_carry_the_mark_and_say_it()
    {
        var row = new TrackItem(0) { Localizer = English.Instance };
        row.Load(new TrackRow(new TrackRef.Library(7), "Lantern", "Band", null, 180_000, null, null, false, false, Downloaded: true));
        Assert.True(row.IsDownloaded);
        Assert.EndsWith(", Downloaded", row.AccessibleName, StringComparison.Ordinal);

        var source = new TrackItem(0) { Localizer = English.Instance };
        source.Load(Item("org.jellyfin", "s1") with { Downloaded = true });
        Assert.True(source.IsDownloaded);
    }

    private static SourcesState Sources()
    {
        static PluginInfo Plugin(string id, string name, bool download, bool enabled = true) =>
            new(id, name, "1.0.0", "Someone", PluginBadge.Official, false, ["search", "browse"], [AuthMethod.PasswordExchange], enabled, "ana",
                false, "{}", "{}", null, Download: download);

        return new SourcesState(1, [Plugin("org.jellyfin", "Jellyfin", true), Plugin("org.spotify", "Spotify", false), Plugin("org.off", "Off", true, enabled: false)], [], false);
    }

    private static SourceItem Item(string source, string id) =>
        new(source, ItemKind.Track, id, "Song " + id, "Band", null, 180_000, null, true, null,
            new TrackRef.Source(new SourceTrack(source, id, "Song " + id, "Band", null, 180_000, null, null)), []);

    private static TrackItem Song(string source, string id)
    {
        var item = new TrackItem(0);
        item.Load(Item(source, id));
        return item;
    }
}
