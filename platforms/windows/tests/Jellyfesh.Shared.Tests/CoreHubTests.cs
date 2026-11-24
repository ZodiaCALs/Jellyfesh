using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Tests.Support;
using Xunit;

namespace Jellyfesh.Tests;

public class CoreHubTests
{
    [Fact]
    public void Start_subscribes_before_reading_the_snapshot()
    {
        var core = new FakeCore { State = CoreSnapshot.Initial with { LibraryRevision = 9 } };
        var ui = new QueueDispatcher();
        using var hub = new CoreHub(core, ui);
        var changes = new List<SliceChange>();
        hub.Changed += c => changes.Add(c);
        hub.Start();
        Assert.True(core.HasListener);
        Assert.Equal(9ul, hub.State.LibraryRevision);
        Assert.Equal(SliceChange.All, changes.Single());
    }

    [Fact]
    public void A_burst_of_events_is_applied_once_with_the_newest_slices()
    {
        var core = new FakeCore();
        var ui = new QueueDispatcher();
        using var hub = new CoreHub(core, ui);
        hub.Start();
        var changes = new List<SliceChange>();
        hub.Changed += c => changes.Add(c);

        for (var i = 0; i < 1_000; i++)
        {
            core.Emit(new CoreEvent.Scan(new ScanStatus.Progress(new ScanProgress((ulong)i, 0, $"/m/{i}.flac"))));
        }

        core.Emit(new CoreEvent.Playback(Samples.Playing(0, 0, version: 5)));
        core.Emit(new CoreEvent.Playback(Samples.Paused(10, 0, version: 4))); // older: dropped
        Assert.Equal(1, ui.Pending); // one dispatch for the whole burst

        ui.RunAll();
        Assert.Single(changes);
        Assert.Equal(SliceChange.Scan | SliceChange.Playback, changes[0]);
        Assert.True(hub.State.Scanning);
        Assert.Equal(999ul, hub.ScanProgress!.FilesSeen);
        Assert.Equal(PlaybackStatus.Playing, hub.State.Playback.Status);
        Assert.Equal(5ul, hub.State.Playback.Version);
    }

    [Fact]
    public void Notifications_keep_their_order_and_a_finished_scan_is_one()
    {
        var core = new FakeCore();
        var ui = new QueueDispatcher();
        using var hub = new CoreHub(core, ui);
        hub.Start();
        var seen = new List<CoreEvent>();
        hub.Notification += e => seen.Add(e);
        core.Emit(new CoreEvent.CommandFailed(3, "nothing to play"));
        core.Emit(new CoreEvent.Scan(new ScanStatus.Finished(new ScanReport(2, 0, 0, 5, []))));
        core.Emit(new CoreEvent.PlaylistCreated(4, 12));
        core.Emit(new CoreEvent.Error("unreadable file"));
        ui.RunAll();
        Assert.Collection(
            seen,
            e => Assert.IsType<CoreEvent.CommandFailed>(e),
            e => Assert.IsType<CoreEvent.Scan>(e),
            e => Assert.IsType<CoreEvent.PlaylistCreated>(e),
            e => Assert.IsType<CoreEvent.Error>(e));
        Assert.False(hub.State.Scanning);
        Assert.Null(hub.ScanProgress);
    }

    [Fact]
    public void The_v007_slices_apply_by_version_and_their_answers_are_notifications()
    {
        var core = new FakeCore();
        var ui = new QueueDispatcher();
        using var hub = new CoreHub(core, ui);
        hub.Start();
        var changes = new List<SliceChange>();
        var seen = new List<CoreEvent>();
        hub.Changed += c => changes.Add(c);
        hub.Notification += e => seen.Add(e);

        core.Emit(new CoreEvent.AiLyrics(AiLyricsState.Initial with { Version = 3, Available = true }));
        core.Emit(new CoreEvent.AiLyrics(AiLyricsState.Initial with { Version = 2 })); // older: dropped
        core.Emit(new CoreEvent.Look(LookState.Default with { Version = 1, Customized = true }));
        core.Emit(new CoreEvent.Pro(ProState.Initial with { Version = 4, Active = true }));
        core.Emit(new CoreEvent.UpdateStatus(UpdateState.Initial with { Version = 2, Supported = true }));
        core.Emit(new CoreEvent.Discover(new DiscoverPage(7, "library", [], null)));
        core.Emit(new CoreEvent.Artwork(new ArtworkReady("https://example.org/a.jpg", 256, "/cache/a.jpg")));
        ui.RunAll();

        Assert.Equal(SliceChange.AiLyrics | SliceChange.Look | SliceChange.Pro | SliceChange.Update, Assert.Single(changes));
        Assert.True(hub.State.AiLyrics.Available);
        Assert.True(hub.State.Look.Customized);
        Assert.True(hub.State.Pro.Active);
        Assert.True(hub.State.Update.Supported);
        Assert.Collection(seen, e => Assert.IsType<CoreEvent.Discover>(e), e => Assert.IsType<CoreEvent.Artwork>(e));

        // A slice older than the one applied changes nothing.
        core.Emit(new CoreEvent.Pro(ProState.Initial with { Version = 3 }));
        ui.RunAll();
        Assert.True(hub.State.Pro.Active);
        Assert.Single(changes);
    }

    [Fact]
    public void The_v008_slices_apply_by_version_and_their_pages_are_notifications()
    {
        var core = new FakeCore();
        var ui = new QueueDispatcher();
        using var hub = new CoreHub(core, ui);
        hub.Start();
        var changes = new List<SliceChange>();
        var seen = new List<CoreEvent>();
        hub.Changed += c => changes.Add(c);
        hub.Notification += e => seen.Add(e);

        var library = new SourceLibrary("spotify", "Spotify", null, []);
        core.Emit(new CoreEvent.SourceLibrary(new SourceLibraryState(2, [library])));
        core.Emit(new CoreEvent.SourceLibrary(new SourceLibraryState(1, []))); // older: dropped
        core.Emit(new CoreEvent.Downloads(DownloadsState.Initial with { Version = 5, Folder = "/music/Jellyfesh", Paused = DownloadsPaused.Metered }));
        core.Emit(new CoreEvent.SearchPage(new SearchMorePage(3, "blue", "spotify", ItemKind.Album, [], null, null)));
        core.Emit(new CoreEvent.SourceLibraryPage(new SourceLibraryMorePage(4, "spotify", SourceLibraryPart.Playlists, [], null, null)));
        ui.RunAll();

        Assert.Equal(SliceChange.SourceLibrary | SliceChange.Downloads, Assert.Single(changes));
        Assert.Equal(library, Assert.Single(hub.State.SourceLibrary.Sources));
        Assert.Equal(("/music/Jellyfesh", DownloadsPaused.Metered), (hub.State.Downloads.Folder, hub.State.Downloads.Paused));
        Assert.Collection(seen, e => Assert.IsType<CoreEvent.SearchPage>(e), e => Assert.IsType<CoreEvent.SourceLibraryPage>(e));

        core.Emit(new CoreEvent.Downloads(DownloadsState.Initial with { Version = 4 }));
        ui.RunAll();
        Assert.Equal(5ul, hub.State.Downloads.Version);
        Assert.Single(changes);
    }

    [Fact]
    public void Library_revisions_only_move_forward()
    {
        var core = new FakeCore();
        var ui = new QueueDispatcher();
        using var hub = new CoreHub(core, ui);
        hub.Start();
        core.Emit(new CoreEvent.LibraryChanged(7));
        core.Emit(new CoreEvent.LibraryChanged(6));
        ui.RunAll();
        Assert.Equal(7ul, hub.State.LibraryRevision);
    }

    [Fact]
    public void Muted_is_read_back_after_the_next_playback_slice()
    {
        var core = new FakeCore();
        var ui = new QueueDispatcher();
        using var hub = new CoreHub(core, ui);
        hub.Start();
        var snapshots = core.SnapshotCount;
        hub.Send(new CoreCommand.SetMuted(true));
        core.State = core.State with { Muted = true };
        core.Emit(new CoreEvent.Playback(Samples.Paused(0, 0, version: 3)));
        ui.RunUntil(() => hub.State.Muted);
        Assert.Equal(snapshots + 1, core.SnapshotCount);
    }

    [Fact]
    public void After_dispose_events_are_dropped()
    {
        var core = new FakeCore();
        var ui = new QueueDispatcher();
        var hub = new CoreHub(core, ui);
        hub.Start();
        hub.Dispose();
        Assert.False(core.HasListener);
    }

    [Fact]
    public void Core_clock_offset_comes_from_the_narrowest_sample()
    {
        // Stopwatch at 10 MHz; the core's clock is 5 s ahead. Sample widths 3, 1, 2 ticks.
        var ticks = new Queue<long>([100, 103, 200, 201, 300, 302]);
        var coreTimes = new Queue<ulong>([5_000_010_100, 5_000_020_050, 5_000_030_100]);
        var clock = CoreClock.Calibrate(() => coreTimes.Dequeue(), () => ticks.Count > 0 ? ticks.Dequeue() : 1_000, 10_000_000, samples: 3);
        Assert.Equal(5_000_000_000, clock.OffsetNs);
        Assert.Equal(50, clock.UncertaintyNs);
        Assert.Equal(5_000_100_000ul, clock.NowNs()); // 1 000 ticks = 100 µs
    }
}
