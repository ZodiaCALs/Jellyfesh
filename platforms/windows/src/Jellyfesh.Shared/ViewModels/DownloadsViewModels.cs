using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// A download in progress (docs/ui.md, "Downloads"): the song, then "Waiting", its percentage, or,
/// after a failure, the reason with Try Again; Cancel on each.
/// </summary>
public sealed partial class DownloadRow : ObservableObject
{
    private readonly ILocalizer _l;

    [ObservableProperty]
    private string _status = string.Empty;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasProgress))]
    private double? _percent;

    [ObservableProperty]
    private bool _isFailed;

    public DownloadRow(DownloadItem item, ILocalizer l)
    {
        _l = l;
        Id = item.Id;
        Title = item.Track.Title;
        Subtitle = Downloads.Byline(item.Track, l);
        Update(item);
    }

    public ulong Id { get; }

    public string Title { get; }

    /// <summary>"Artist · Album".</summary>
    public string Subtitle { get; }

    /// <summary>The bar shows while the size is known.</summary>
    public bool HasProgress => Percent is not null;

    internal void Update(DownloadItem item)
    {
        IsFailed = item.State is DownloadState.Failed;
        Percent = item.State is DownloadState.Downloading && item.BytesTotal is > 0 and { } total
            ? Math.Min(100, Math.Floor(item.BytesDone * 100.0 / total))
            : null;
        Status = item.State switch
        {
            DownloadState.Failed failed => Downloads.Reason(failed.Error, _l),
            DownloadState.Downloading when Percent is { } percent => _l.Format("Downloads_Percent", percent.ToString("0", _l.Culture)),
            DownloadState.Downloading => _l.Get("Downloads_Downloading"),
            _ => _l.Get("Downloads_Waiting"),
        };
    }
}

/// <summary>A downloaded song: it plays from the file, and Remove Download… deletes it (asks first).</summary>
public sealed class DownloadedSong(DownloadItem item, ILocalizer l)
{
    public ulong Id { get; } = item.Id;

    public string Title { get; } = item.Track.Title;

    public string Artist { get; } = item.Track.Artist ?? l.Get("Library_UnknownArtist");

    public string DurationText { get; } = item.Track.DurationMs is { } ms ? TimeFormat.ClockMs(ms) : string.Empty;

    /// <summary>Its library track, which plays the file.</summary>
    public long? LibraryId { get; } = item.LibraryId;

    public override string ToString() => Title;
}

/// <summary>The downloaded songs of one album, with Remove Download… for all of them.</summary>
public sealed class DownloadedAlbum(string title, string? artist, IReadOnlyList<DownloadedSong> songs)
{
    public string Title { get; } = title;

    public string Artist { get; } = artist ?? string.Empty;

    public bool HasArtist => Artist.Length > 0;

    public IReadOnlyList<DownloadedSong> Songs { get; } = songs;

    public override string ToString() => Title;
}

/// <summary>The words of the Downloads page and Settings › Downloads.</summary>
public static class Downloads
{
    /// <summary>"Artist · Album", with what is known of it.</summary>
    public static string Byline(SourceTrack track, ILocalizer l) =>
        string.Join(" · ", new[] { track.Artist, track.Album }.Where(s => !string.IsNullOrWhiteSpace(s)));

    /// <summary>Why a download failed, in the app's words.</summary>
    public static string Reason(DownloadError error, ILocalizer l) => error switch
    {
        DownloadError.NotAllowed n => l.Format("Downloads_NotAllowed", n.Service),
        DownloadError.NotOffered n => l.Format("Downloads_NotOffered", n.Service),
        DownloadError.Network n => n.Message,
        DownloadError.Storage n => n.Message,
        DownloadError.Other o => o.Message,
        _ => string.Empty,
    };

    /// <summary>Why downloads wait, while the core pauses them.</summary>
    public static string? PausedText(DownloadsPaused? paused, ILocalizer l) => paused switch
    {
        DownloadsPaused.Offline => l.Get("Downloads_PausedOffline"),
        DownloadsPaused.Metered => l.Get("Downloads_PausedMetered"),
        _ => null,
    };
}

/// <summary>
/// Downloads (ADR-0025; docs/ui.md "Downloads"): Play and Shuffle with "{n} songs · {size} used";
/// In Progress, each with "Waiting", its percentage, or the failure and Try Again, and Cancel;
/// while the core pauses downloads, why; then the downloaded songs by album, with Remove
/// Download… on a song or a whole album (asks first). Everything comes from the Downloads slice.
/// </summary>
public sealed partial class DownloadsViewModel(ShellServices services) : PageViewModel(services)
{
    [ObservableProperty]
    private string _summary = string.Empty;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsPaused))]
    private string? _pausedText;

    [ObservableProperty]
    private bool _hasInProgress;

    [ObservableProperty]
    private bool _hasDownloaded;

    /// <summary>In Progress, in the queue's order.</summary>
    public ObservableCollection<DownloadRow> InProgress { get; } = [];

    /// <summary>The downloaded songs by album.</summary>
    public ObservableCollection<DownloadedAlbum> Albums { get; } = [];

    public bool IsPaused => PausedText is not null;

    protected override Task LoadCoreAsync()
    {
        Apply(S.Hub.State.Downloads);
        return Task.CompletedTask;
    }

    protected override void OnActivated() => Apply(S.Hub.State.Downloads);

    protected override void OnSlices(SliceChange changes, CoreSnapshot state)
    {
        if (changes.HasFlag(SliceChange.Downloads))
        {
            Apply(state.Downloads);
        }
    }

    /// <summary>The downloaded songs from the first, on their files.</summary>
    [RelayCommand]
    private void Play() => PlayDownloaded(shuffle: false);

    /// <summary>The downloaded songs in a random order (the core chooses where to start).</summary>
    [RelayCommand]
    private void Shuffle() => PlayDownloaded(shuffle: true);

    /// <summary>A downloaded song plays with the ones after it.</summary>
    [RelayCommand]
    private void PlaySong(DownloadedSong? song)
    {
        var refs = Songs().ToList();
        var start = song?.LibraryId is { } id ? refs.FindIndex(r => r is TrackRef.Library l && l.Id == id) : -1;
        if (start >= 0)
        {
            S.Send(new CoreCommand.PlayTracks(refs, (uint)start));
        }
    }

    [RelayCommand]
    private void Cancel(DownloadRow? row)
    {
        if (row is not null)
        {
            S.Send(new CoreCommand.CancelDownload(row.Id));
        }
    }

    [RelayCommand]
    private void Retry(DownloadRow? row)
    {
        if (row is { IsFailed: true })
        {
            S.Send(new CoreCommand.RetryDownload(row.Id));
        }
    }

    /// <summary>Remove Download… on a song: asks first, then deletes its file.</summary>
    [RelayCommand]
    private async Task RemoveSong(DownloadedSong? song)
    {
        if (song is not null && await ConfirmRemoveAsync(song.Title, "Downloads_RemoveBody"))
        {
            S.Send(new CoreCommand.RemoveDownload(song.Id));
        }
    }

    /// <summary>Remove Download… on an album: asks first, then deletes every downloaded song of it.</summary>
    [RelayCommand]
    private async Task RemoveAlbum(DownloadedAlbum? album)
    {
        if (album is not null && await ConfirmRemoveAsync(album.Title, "Downloads_RemoveAlbumBody"))
        {
            foreach (var song in album.Songs)
            {
                S.Send(new CoreCommand.RemoveDownload(song.Id));
            }
        }
    }

    private Task<bool> ConfirmRemoveAsync(string name, string body) => S.Platform.ConfirmAsync(
        S.L.Format("Downloads_RemoveTitle", name), S.L.Get(body), S.L.Get("Downloads_RemoveConfirm"), S.L.Get("Action_Cancel"));

    private IEnumerable<TrackRef> Songs() =>
        Albums.SelectMany(a => a.Songs).Where(s => s.LibraryId is not null).Select(s => (TrackRef)new TrackRef.Library(s.LibraryId!.Value));

    private void PlayDownloaded(bool shuffle)
    {
        var refs = Songs().ToList();
        if (refs.Count > 0)
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Tracks(refs), 0, shuffle));
        }
    }

    private void Apply(DownloadsState state)
    {
        // In Progress: rows keep their place while only their progress moves.
        var going = state.Items.Where(i => i.State is not DownloadState.Done).ToList();
        if (InProgress.Select(r => r.Id).SequenceEqual(going.Select(i => i.Id)))
        {
            for (var i = 0; i < going.Count; i++)
            {
                InProgress[i].Update(going[i]);
            }
        }
        else
        {
            InProgress.Clear();
            foreach (var item in going)
            {
                InProgress.Add(new DownloadRow(item, S.L));
            }
        }

        // The finished ones come by album from the core; a run of one album is one group.
        var done = state.Items.Where(i => i.State is DownloadState.Done).ToList();
        var shownIds = Albums.SelectMany(a => a.Songs).Select(s => s.Id);
        if (!shownIds.SequenceEqual(done.Select(i => i.Id)))
        {
            Albums.Clear();
            foreach (var run in Runs(done))
            {
                var first = run[0].Track;
                Albums.Add(new DownloadedAlbum(
                    string.IsNullOrWhiteSpace(first.Album) ? S.L.Get("Downloads_UnknownAlbum") : first.Album,
                    first.Artist,
                    run.Select(i => new DownloadedSong(i, S.L)).ToList()));
            }
        }

        HasInProgress = InProgress.Count > 0;
        HasDownloaded = Albums.Count > 0;
        IsEmpty = !HasInProgress && !HasDownloaded;
        PausedText = HasInProgress ? Downloads.PausedText(state.Paused, S.L) : null;
        Summary = done.Count == 0 ? string.Empty
            : S.L.Format("Downloads_Summary", S.L.Plural("Count_Songs", done.Count), DetailsRows.Size(state.BytesUsed, S.L));
    }

    private static IEnumerable<List<DownloadItem>> Runs(IEnumerable<DownloadItem> items)
    {
        List<DownloadItem>? run = null;
        foreach (var item in items)
        {
            if (run is not null && run[0].Track.Album == item.Track.Album)
            {
                run.Add(item);
                continue;
            }

            if (run is not null)
            {
                yield return run;
            }

            run = [item];
        }

        if (run is not null)
        {
            yield return run;
        }
    }
}
