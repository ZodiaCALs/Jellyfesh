using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>An output device choice; a null <see cref="Id"/> follows the system default.</summary>
public sealed record DeviceOption(string? Id, string Name, bool IsDefault)
{
    public override string ToString() => Name;
}

/// <summary>
/// A device in Devices (a remote session's picker): its name, the glyph of its kind, and whether
/// it is the one playing (checked). Choosing another sends Remote { Transfer }.
/// </summary>
public sealed record RemoteDeviceOption(string Id, string Name, RemoteDeviceKind Kind, bool IsPlaying)
{
    /// <summary>The glyph resource for the device's kind.</summary>
    public string Glyph => GlyphFor(Kind);

    public static string GlyphFor(RemoteDeviceKind kind) => kind switch
    {
        RemoteDeviceKind.Computer => "IconDeviceComputer",
        RemoteDeviceKind.Smartphone => "IconDevicePhone",
        RemoteDeviceKind.Speaker => "IconSpeaker",
        RemoteDeviceKind.Tv => "IconDeviceTv",
        RemoteDeviceKind.Car => "IconDeviceCar",
        _ => "IconDeviceOther",
    };

    /// <summary>
    /// Devices in the service's order, the playing one checked (the slice's device, else the one the
    /// service marks active). The playing device comes first when the list does not have it yet.
    /// </summary>
    public static IReadOnlyList<RemoteDeviceOption> From(RemoteState remote)
    {
        var playing = remote.Device;
        bool IsPlaying(RemoteDevice d) => playing is not null ? d.Id == playing.Id : d.Active;
        var options = remote.Devices
            .Where(d => !string.IsNullOrWhiteSpace(d.Name))
            .Select(d => new RemoteDeviceOption(d.Id, d.Name, d.Kind, IsPlaying(d)))
            .ToList();
        if (playing is not null && !string.IsNullOrWhiteSpace(playing.Name) && options.All(o => o.Id != playing.Id))
        {
            options.Insert(0, new RemoteDeviceOption(playing.Id, playing.Name, playing.Kind, true));
        }

        return options;
    }

    public override string ToString() => Name;
}

/// <summary>
/// What is playing and the transport: Now Playing, the mini player, media keys (SMTC) and the
/// taskbar buttons all bind to this. It mirrors the core's slices and only sends commands.
/// While the Remote slice has a source (ADR-0018), it shows the remote session's track instead of
/// the engine's, with Playing on {Device}; the transport commands stay the same (the core routes
/// them), and the volume sets the playing device's volume.
/// </summary>
public sealed partial class PlayerViewModel : ObservableObject
{
    /// <summary>After the person moves the volume, slices that still carry the old value are ignored for this long.</summary>
    private static readonly TimeSpan VolumeEchoGuard = TimeSpan.FromMilliseconds(600);

    private readonly ShellServices _s;
    private long _lastVolumeInputMs = long.MinValue / 2;
    private bool _syncingDevice;
    private RemoteState _remote = RemoteState.Initial;
    private string? _trackKey;

    [ObservableProperty]
    private string _title = string.Empty;

    [ObservableProperty]
    private string _artist = string.Empty;

    [ObservableProperty]
    private string _album = string.Empty;

    [ObservableProperty]
    private string _subtitle = string.Empty;

    [ObservableProperty]
    private string? _artworkUri;

    /// <summary>The size source pictures of what plays are asked for: Now Playing's large artwork.</summary>
    private const uint PlayerArtworkPixels = 1024;

    private ArtworkRef? _artworkShown;
    private CancellationTokenSource? _artworkRequest;

    [ObservableProperty]
    private string? _artworkPath;

    [ObservableProperty]
    private bool _hasTrack;

    [ObservableProperty]
    private bool _isPlaying;

    [ObservableProperty]
    private bool _isBuffering;

    [ObservableProperty]
    private bool _shuffle;

    [ObservableProperty]
    private RepeatMode _repeat;

    [ObservableProperty]
    private double _volume = 100;

    [ObservableProperty]
    private bool _isMuted;

    [ObservableProperty]
    private bool _isFavorite;

    [ObservableProperty]
    private bool _canFavorite;

    [ObservableProperty]
    private string _formatText = string.Empty;

    /// <summary>The core's quality label for what is decoded ("Hi-Res Lossless"), for the Info pane.</summary>
    [ObservableProperty]
    private string _qualityText = string.Empty;

    /// <summary>The quality badge and its menu (ADR-0026 §3); null while nothing plays.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasQualityBadge))]
    [NotifyPropertyChangedFor(nameof(QualityBadge))]
    [NotifyPropertyChangedFor(nameof(QualityBadgeLabel))]
    private QualityMenu? _quality;

    // Now Playing › Info, "About This Song": Title, Artist, Album, Year, Length, Format, Lyrics and File.

    /// <summary>The song's year, for library songs that have one.</summary>
    [ObservableProperty]
    private string _year = string.Empty;

    /// <summary>"3:52".</summary>
    [ObservableProperty]
    private string _lengthText = string.Empty;

    /// <summary>How the lyrics are timed and where they come from ("Word by word · song.lrc"), or that none were found.</summary>
    [ObservableProperty]
    private string _lyricsText = string.Empty;

    /// <summary>The file playing; empty for a song from a source.</summary>
    [ObservableProperty]
    private string _filePath = string.Empty;

    [ObservableProperty]
    private bool _canGoNext;

    [ObservableProperty]
    private bool _canGoPrevious;

    [ObservableProperty]
    private DeviceOption? _selectedDevice;

    // ---- remote sessions and links (docs/ui.md, "Links and remote playback") ----

    /// <summary>A remote session shows instead of the engine's track.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanMute))]
    [NotifyPropertyChangedFor(nameof(HasMore))]
    private bool _isRemote;

    /// <summary>"Playing on Kitchen" under the title while a remote session shows; empty otherwise.</summary>
    [ObservableProperty]
    private string _remoteDeviceText = string.Empty;

    /// <summary>What went wrong last talking to the service: a quieter line below Playing on {Device}.</summary>
    [ObservableProperty]
    private string _remoteError = string.Empty;

    /// <summary>The service: the remote session's source, or the source of the song playing.</summary>
    [ObservableProperty]
    private string _serviceName = string.Empty;

    /// <summary>Opens the song in its service (Open in {Service}); null when it has no link.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasLink))]
    [NotifyPropertyChangedFor(nameof(HasMore))]
    private string? _link;

    /// <summary>"Open in Spotify".</summary>
    [ObservableProperty]
    private string _openLinkLabel = string.Empty;

    /// <summary>The volume control: hidden while the remote device has no volume Jellyfesh may change.</summary>
    [ObservableProperty]
    private bool _showsVolume = true;

    /// <summary>The Lyrics pane while a remote session shows: "Lyrics aren't shown while Spotify plays."</summary>
    [ObservableProperty]
    private string _remoteLyricsText = string.Empty;

    /// <summary>The Queue pane while a remote session shows: "Up Next is in Spotify."</summary>
    [ObservableProperty]
    private string _remoteQueueText = string.Empty;

    /// <summary>The device button: the output device, or the remote device playing.</summary>
    [ObservableProperty]
    private string _outputName = string.Empty;

    /// <summary>The device button's glyph resource: a speaker, or the remote device's kind.</summary>
    [ObservableProperty]
    private string _outputGlyph = "IconSpeaker";

    /// <summary>The device button's name for Narrator: Output Device, or Devices in a remote session.</summary>
    [ObservableProperty]
    private string _outputLabel = string.Empty;

    public PlayerViewModel(ShellServices services)
    {
        _s = services;
        Progress = new ProgressPresenter(services.L, services.Time, showsTime: true);
        BarProgress = new ProgressPresenter(services.L, services.Time, showsTime: true);
        MiniProgress = new ProgressPresenter(services.L, services.Time, showsTime: false);
        services.Ticker.Attach(Progress);
        services.Ticker.Attach(BarProgress);
        services.Ticker.Attach(MiniProgress);
        UpdateOutputButton();
    }

    /// <summary>The seek bar and its times on Now Playing.</summary>
    public ProgressPresenter Progress { get; }

    /// <summary>The seek bar and its times on the player bar (windows 900 px and wider).</summary>
    public ProgressPresenter BarProgress { get; }

    /// <summary>The mini player's progress line (and the compact player's ring).</summary>
    public ProgressPresenter MiniProgress { get; }

    public ObservableCollection<DeviceOption> Devices { get; } = [];

    /// <summary>Devices: every device the remote service lists, the playing one checked.</summary>
    public ObservableCollection<RemoteDeviceOption> RemoteDevices { get; } = [];

    public NowPlayingInfo? NowPlaying { get; private set; }

    /// <summary>The remote session as last applied (RemoteState.Initial without one).</summary>
    public RemoteState Remote => _remote;

    /// <summary>
    /// What is shown, as a key that changes when the song does: the engine's queue entry, or the
    /// remote session's track. Null when nothing is shown.
    /// </summary>
    public string? TrackKey => _trackKey;

    public PlaybackStatus Status { get; private set; }

    /// <summary>The engine's mute; a remote device has only its volume.</summary>
    public bool CanMute => !IsRemote;

    public bool HasLink => Link is not null;

    /// <summary>The More menu has something: Open in {Service}, or Stop Controlling.</summary>
    public bool HasMore => HasLink || IsRemote;

    /// <summary>The bar and Now Playing show the quality badge while something plays.</summary>
    public bool HasQualityBadge => Quality is not null;

    /// <summary>The badge's words: the playing quality's label, else the chosen level.</summary>
    public string QualityBadge => Quality?.Badge ?? string.Empty;

    /// <summary>"Quality: Lossless", the badge's name for screen readers.</summary>
    public string QualityBadgeLabel => Quality is { } q ? _s.L.Format("Quality_BadgeLabel", q.Badge) : string.Empty;

    /// <summary>The current track's position in the queue, as "3 of 12" for Narrator; empty when unknown.</summary>
    public string QueuePositionText { get; private set; } = string.Empty;

    public string PlayPauseLabel => _s.L.Get(IsPlaying ? "Player_Pause" : "Player_Play");

    public string ShuffleLabel => _s.L.Get(Shuffle ? "Player_ShuffleOn" : "Player_ShuffleOff");

    public string RepeatLabel => _s.L.Get(Repeat switch
    {
        RepeatMode.All => "Player_RepeatAll",
        RepeatMode.One => "Player_RepeatOne",
        _ => "Player_RepeatOff",
    });

    public string MuteLabel => _s.L.Get(IsMuted ? "Player_Unmute" : "Player_Mute");

    public string FavoriteLabel => _s.L.Get(IsFavorite ? "Player_Unfavorite" : "Player_Favorite");

    /// <summary>Takes the slices that changed (UI thread).</summary>
    public void OnSlices(SliceChange changes, CoreSnapshot state)
    {
        var switched = false;
        if (changes.HasFlag(SliceChange.Remote))
        {
            NoticeEndedSession(_remote, state.Remote);
            switched = _remote.HasSession != state.Remote.HasSession;
            _remote = state.Remote;
            if (switched)
            {
                // Between the engine and a remote session, everything shown changes.
                changes |= SliceChange.Playback | SliceChange.NowPlaying | SliceChange.Queue | SliceChange.Lyrics;
            }
        }

        if (state.Remote.HasSession)
        {
            if (changes.HasFlag(SliceChange.Remote) || changes.HasFlag(SliceChange.Sources))
            {
                ApplyRemote(state.Remote);
            }
        }
        else
        {
            ApplyEngine(changes, state);
        }

        if (changes.HasFlag(SliceChange.Muted))
        {
            IsMuted = state.Muted;
        }

        if (changes.HasFlag(SliceChange.Devices))
        {
            ApplyDevices(state.Devices);
        }

        if (switched || changes.HasFlag(SliceChange.Devices))
        {
            UpdateOutputButton();
        }

        if ((changes & (SliceChange.NowPlaying | SliceChange.Settings | SliceChange.Remote)) != 0)
        {
            UpdateQuality(state);
        }
    }

    /// <summary>
    /// The quality badge and its menu (ADR-0026 §3): for the engine's song its quality, the levels
    /// its source offers and why there are none; in a remote session, the service's note.
    /// </summary>
    private void UpdateQuality(CoreSnapshot state)
    {
        var chosen = state.Settings.StreamingQuality;
        Quality = state.Remote.HasSession ? QualityMenu.Of(null, null, chosen, state.Remote.QualityNote, _s.L)
            : state.NowPlaying is { } np ? QualityMenu.Of(np.Quality, np.Qualities, chosen, np.QualityNote, _s.L)
            : null;
    }

    /// <summary>The engine's track and transport (no remote session).</summary>
    private void ApplyEngine(SliceChange changes, CoreSnapshot state)
    {
        if (IsRemote)
        {
            IsRemote = false;
            RemoteDeviceText = string.Empty;
            RemoteError = string.Empty;
            RemoteLyricsText = string.Empty;
            RemoteQueueText = string.Empty;
            ShowsVolume = true;
            RemoteDevices.Clear();
        }

        if (changes.HasFlag(SliceChange.Playback))
        {
            var p = state.Playback;
            Status = p.Status;
            IsPlaying = p.Status == PlaybackStatus.Playing;
            if (Environment.TickCount64 - _lastVolumeInputMs > VolumeEchoGuard.TotalMilliseconds)
            {
                Volume = Math.Round(p.Volume * 100);
            }

            UpdateDuration(p.Anchor.DurationUs);
            _s.Ticker.OnAnchor(p.Anchor);
        }

        if (changes.HasFlag(SliceChange.NowPlaying))
        {
            ApplyNowPlaying(state.NowPlaying);
        }

        if (changes.HasFlag(SliceChange.NowPlaying) || changes.HasFlag(SliceChange.Sources))
        {
            // Open in {Service}: the song's own source names the service.
            var np = NowPlaying;
            ServiceName = np?.Track is TrackRef.Source source ? ItemLinks.ServiceName(state.Sources, source.Track.Plugin) : string.Empty;
            Link = ItemLinks.Usable(np?.Link);
            OpenLinkLabel = Link is null ? string.Empty : ItemLinks.OpenInLabel(_s.L, ServiceName);
        }

        if (changes.HasFlag(SliceChange.NowPlaying) || changes.HasFlag(SliceChange.Lyrics))
        {
            LyricsText = DescribeLyrics(state.Lyrics);
        }

        if (changes.HasFlag(SliceChange.Queue))
        {
            var q = state.Queue;
            Shuffle = q.Shuffle;
            Repeat = q.Repeat;
            CanGoNext = q.Length > 0 && (q.Repeat != RepeatMode.Off || q.UpNext.Count > 0);
            CanGoPrevious = q.Length > 0;
            QueuePositionText = q.Position is { } pos ? _s.L.Format("Queue_PositionOf", pos + 1, q.Length) : string.Empty;
            OnPropertyChanged(nameof(QueuePositionText));
        }
    }

    /// <summary>
    /// The remote session's track (docs/ui.md, "Links and remote playback"): its title, artist and
    /// artwork, Playing on {Device}, the error quietly, progress from its anchor, its shuffle and
    /// repeat, and the playing device's volume.
    /// </summary>
    private void ApplyRemote(RemoteState r)
    {
        var item = r.Item;
        var key = $"remote:{r.Source}:{item?.Id}";
        var changedTrack = key != _trackKey;
        IsRemote = true;
        Status = r.Status;
        IsPlaying = r.Status == PlaybackStatus.Playing;
        IsBuffering = false;
        ServiceName = string.IsNullOrWhiteSpace(r.SourceName) ? r.Source ?? string.Empty : r.SourceName;

        // With no track reported yet, the service's name stands in for the title.
        Title = item?.Title ?? ServiceName;
        Artist = item is null ? string.Empty : item.Subtitle ?? _s.L.Get("Library_UnknownArtist");
        Album = item?.Album ?? string.Empty;
        Subtitle = item is null ? string.Empty
            : item.Album is null ? Artist
            : _s.L.Format("Player_ArtistAlbum", Artist, item.Album);
        ShowArtwork(item?.Artwork);
        FormatText = string.Empty;
        QualityText = string.Empty;
        FilePath = string.Empty;
        Year = string.Empty;
        LyricsText = string.Empty;
        HasTrack = true;

        RemoteDeviceText = r.Device is { Name: { Length: > 0 } name } ? _s.L.Format("Remote_PlayingOn", name) : string.Empty;
        RemoteError = r.Error ?? string.Empty;
        RemoteLyricsText = _s.L.Format("Remote_Lyrics", ServiceName);
        RemoteQueueText = _s.L.Format("Remote_Queue", ServiceName);
        Link = ItemLinks.Usable(item?.Link);
        OpenLinkLabel = Link is null ? string.Empty : ItemLinks.OpenInLabel(_s.L, ServiceName);

        // The core routes the transport to the service, which keeps its own queue.
        Shuffle = r.Shuffle;
        Repeat = r.Repeat;
        CanGoNext = true;
        CanGoPrevious = true;
        QueuePositionText = string.Empty;
        OnPropertyChanged(nameof(QueuePositionText));

        ShowsVolume = r.Device?.VolumePercent is not null;
        if (r.Device?.VolumePercent is { } percent && Environment.TickCount64 - _lastVolumeInputMs > VolumeEchoGuard.TotalMilliseconds)
        {
            Volume = Math.Min(percent, 100u);
        }

        var durationUs = r.Anchor.DurationUs ?? (item?.DurationMs is { } ms ? (long)ms * 1_000 : null);
        LengthText = durationUs is > 0 ? TimeFormat.Clock(durationUs.Value) : string.Empty;
        UpdateDuration(durationUs ?? 0);
        _s.Ticker.OnAnchor(r.Anchor);

        SyncRemoteDevices(r);
        UpdateOutputButton();
        CanFavorite = item?.Track is TrackRef.Library or TrackRef.Source;
        if (changedTrack)
        {
            // Last, so whoever follows TrackKey (the media controls) reads the new track whole.
            _trackKey = key;
            OnPropertyChanged(nameof(TrackKey));
            _ = RefreshFavoriteAsync(key, item?.Track);
        }
    }

    /// <summary>
    /// A session that ended with an error ("Sign in to Spotify again.") says so once: the slice had a
    /// session and has none now. Errors while a session runs show in its quieter line instead.
    /// </summary>
    private void NoticeEndedSession(RemoteState before, RemoteState after)
    {
        if (before.HasSession && !after.HasSession && !string.IsNullOrWhiteSpace(after.Error))
        {
            var service = string.IsNullOrWhiteSpace(before.SourceName) ? before.Source! : before.SourceName;
            _s.Notifications.Warning(_s.L.Format("Remote_Ended", service), after.Error);
        }
    }

    private void SyncRemoteDevices(RemoteState r)
    {
        var options = RemoteDeviceOption.From(r);
        if (options.SequenceEqual(RemoteDevices))
        {
            return;
        }

        RemoteDevices.Clear();
        foreach (var option in options)
        {
            RemoteDevices.Add(option);
        }
    }

    private void UpdateOutputButton()
    {
        if (IsRemote)
        {
            OutputName = _remote.Device?.Name ?? ServiceName;
            OutputGlyph = RemoteDeviceOption.GlyphFor(_remote.Device?.Kind ?? RemoteDeviceKind.Other);
            OutputLabel = _s.L.Get("Remote_Devices");
        }
        else
        {
            OutputName = SelectedDevice?.Name ?? string.Empty;
            OutputGlyph = "IconSpeaker";
            OutputLabel = _s.L.Get("Player_OutputDevice");
        }
    }

    /// <summary>Devices opened: ask the service for its devices now (it is never polled for them).</summary>
    public void RefreshRemoteDevices()
    {
        if (IsRemote)
        {
            _s.Send(new CoreCommand.Remote(new RemoteAction.Refresh()));
        }
    }

    /// <summary>Moves playback to another of the person's devices.</summary>
    [RelayCommand]
    private void TransferTo(RemoteDeviceOption? device)
    {
        if (IsRemote && device is { IsPlaying: false })
        {
            _s.Send(new CoreCommand.Remote(new RemoteAction.Transfer(device.Id)));
        }
    }

    /// <summary>Stop Controlling: the service's player carries on as it was.</summary>
    [RelayCommand]
    private void StopControlling()
    {
        if (IsRemote)
        {
            _s.Send(new CoreCommand.Remote(new RemoteAction.End()));
        }
    }

    /// <summary>Open in {Service}.</summary>
    [RelayCommand]
    private void OpenLink() => ItemLinks.Open(_s, Link);

    public void OnBuffering(CoreEvent.Buffering b)
    {
        if (NowPlaying?.Uid == b.Uid)
        {
            IsBuffering = b.IsBuffering;
        }
    }

    [RelayCommand]
    private void PlayPause() => _s.Send(new CoreCommand.TogglePlayPause());

    [RelayCommand]
    private void Play() => _s.Send(new CoreCommand.Play());

    [RelayCommand]
    private void Pause() => _s.Send(new CoreCommand.Pause());

    [RelayCommand]
    private void Stop() => _s.Send(new CoreCommand.Stop());

    [RelayCommand]
    private void Next() => _s.Send(new CoreCommand.Next());

    [RelayCommand]
    private void Previous() => _s.Send(new CoreCommand.Previous());

    [RelayCommand]
    private void ToggleShuffle() => _s.Send(new CoreCommand.SetShuffle(!Shuffle));

    /// <summary>A level from the quality menu: the same setting as Settings › Playback › Streaming.</summary>
    [RelayCommand]
    private void ChooseQuality(StreamingQuality level)
    {
        var settings = _s.Hub.State.Settings;
        if (settings.StreamingQuality != level)
        {
            _s.Send(new CoreCommand.SetSettings(settings with { StreamingQuality = level }));
        }
    }

    /// <summary>Off → All → One → Off.</summary>
    [RelayCommand]
    private void CycleRepeat() => _s.Send(new CoreCommand.SetRepeat(Repeat switch
    {
        RepeatMode.Off => RepeatMode.All,
        RepeatMode.All => RepeatMode.One,
        _ => RepeatMode.Off,
    }));

    [RelayCommand]
    private void ToggleMute()
    {
        if (CanMute)
        {
            _s.Send(new CoreCommand.SetMuted(!IsMuted));
        }
    }

    /// <summary>The song shown: the remote session's track, or the engine's.</summary>
    private TrackRef? CurrentTrack => IsRemote ? _remote.Item?.Track : NowPlaying?.Track;

    [RelayCommand]
    private void ToggleFavorite()
    {
        if (CanFavorite && CurrentTrack is { } track)
        {
            _s.Send(new CoreCommand.SetFavorite(track, !IsFavorite));
            IsFavorite = !IsFavorite; // the next library revision confirms it
        }
    }

    /// <summary>
    /// The person set the volume (slider, wheel, keys); 0 to 100. In a remote session it is the
    /// playing device's volume, and nothing is sent while that device has none.
    /// </summary>
    public void SetVolumeFromUser(double percent)
    {
        percent = Math.Clamp(Math.Round(percent), 0, 100);
        if (IsRemote)
        {
            if (ShowsVolume)
            {
                _lastVolumeInputMs = Environment.TickCount64;
                Volume = percent;
                _s.Send(new CoreCommand.Remote(new RemoteAction.SetVolume((uint)percent)));
            }

            return;
        }

        _lastVolumeInputMs = Environment.TickCount64;
        Volume = percent;
        _s.Send(new CoreCommand.SetVolume((float)(percent / 100.0)));
        if (IsMuted && percent > 0)
        {
            _s.Send(new CoreCommand.SetMuted(false));
        }
    }

    [RelayCommand]
    private void VolumeUp() => SetVolumeFromUser(Volume + 5);

    [RelayCommand]
    private void VolumeDown() => SetVolumeFromUser(Volume - 5);

    /// <summary>Seeks to a fraction of the track (the seek bar).</summary>
    public void SeekToFraction(double fraction)
    {
        var duration = Progress.DurationUs;
        if (duration <= 0)
        {
            return;
        }

        SeekToUs((long)(Math.Clamp(fraction, 0, 1) * duration));
    }

    /// <summary>Jumps by <paramref name="seconds"/> from the drawn position (arrow keys).</summary>
    public void SeekBy(double seconds) => SeekToUs(_s.Ticker.PositionUsNow() + (long)(seconds * 1_000_000));

    public void SeekToUs(long positionUs)
    {
        var duration = Progress.DurationUs;
        var clamped = Math.Max(0, duration > 0 ? Math.Min(positionUs, duration) : positionUs);
        _s.Send(new CoreCommand.Seek((ulong)(clamped / 1_000)));
    }

    partial void OnSelectedDeviceChanged(DeviceOption? value)
    {
        if (!_syncingDevice && value is not null)
        {
            _s.Send(new CoreCommand.SetOutputDevice(value.Id));
        }

        UpdateOutputButton();
    }

    partial void OnIsPlayingChanged(bool value) => OnPropertyChanged(nameof(PlayPauseLabel));

    partial void OnShuffleChanged(bool value) => OnPropertyChanged(nameof(ShuffleLabel));

    partial void OnRepeatChanged(RepeatMode value) => OnPropertyChanged(nameof(RepeatLabel));

    partial void OnIsMutedChanged(bool value) => OnPropertyChanged(nameof(MuteLabel));

    partial void OnIsFavoriteChanged(bool value) => OnPropertyChanged(nameof(FavoriteLabel));

    private void ApplyNowPlaying(NowPlayingInfo? np)
    {
        var key = np is null ? null : $"engine:{np.Uid}";
        var changedTrack = key != _trackKey;
        NowPlaying = np;
        HasTrack = np is not null;
        Title = np?.Title ?? string.Empty;
        Artist = np?.Artist ?? (np is null ? string.Empty : _s.L.Get("Library_UnknownArtist"));
        Album = np?.Album ?? string.Empty;
        Subtitle = np is null ? string.Empty
            : np.Album is null ? Artist
            : _s.L.Format("Player_ArtistAlbum", Artist, np.Album);
        ShowArtwork(np?.Artwork);
        // What is decoded (the core's AudioFormat) once the engine opened the track, else the stream's facts.
        FormatText = np?.Quality is { } decoded && DetailsRows.FormatLine(decoded, _s.L) is { } line
            ? decoded.BitrateKbps is > 0 and { } kbps ? $"{line} · {_s.L.Format("Info_Kbps", kbps)}" : line
            : np?.Format is { } fmt ? Describe(fmt) : string.Empty;
        QualityText = np?.Quality?.Quality is { } quality ? DetailsRows.Quality(quality, _s.L) : string.Empty;
        FilePath = np?.Path ?? string.Empty;
        LengthText = np is null ? string.Empty
            : np.DurationMs is { } length ? TimeFormat.ClockMs(length)
            : !changedTrack && Progress.DurationUs > 0 ? TimeFormat.Clock(Progress.DurationUs)
            : string.Empty;
        CanFavorite = np?.Track is TrackRef.Library or TrackRef.Source;
        if (np?.DurationMs is { } ms && Progress.DurationUs == 0)
        {
            UpdateDuration((long)ms * 1_000);
        }

        if (changedTrack)
        {
            // Last, so whoever follows TrackKey (the media controls) reads the new track whole.
            _trackKey = key;
            IsBuffering = false;
            Year = string.Empty;
            OnPropertyChanged(nameof(TrackKey));
            _ = RefreshFavoriteAsync(key, np?.Track);
            _ = RefreshYearAsync(np);
        }
    }

    /// <summary>
    /// The picture of what plays: a file as it is; a source picture once the core has it in its
    /// cache (docs/core-api.md, "Covers"), at the size Now Playing draws, with no picture meanwhile.
    /// The same picture again (a NowPlaying event for the same song) changes nothing.
    /// </summary>
    private void ShowArtwork(ArtworkRef? artwork)
    {
        if (artwork == _artworkShown)
        {
            return;
        }

        _artworkShown = artwork;
        _artworkRequest?.Cancel();
        _artworkRequest = null;
        ArtworkPath = artwork is ArtworkRef.File f ? f.Path : null;
        ArtworkUri = ArtworkPath is { } path ? FileUris.From(path) : null;
        if (artwork is ArtworkRef.Url url)
        {
            _ = LoadSourceArtworkAsync(artwork, url.Address);
        }
    }

    private async Task LoadSourceArtworkAsync(ArtworkRef artwork, string url)
    {
        var cts = new CancellationTokenSource();
        _artworkRequest = cts;
        try
        {
            var path = await _s.Thumbnails.GetSourceAsync(url, PlayerArtworkPixels, cts.Token);
            if (!cts.IsCancellationRequested && path is not null && artwork == _artworkShown)
            {
                ArtworkPath = path;
                ArtworkUri = FileUris.From(path);
            }
        }
        catch (OperationCanceledException)
        {
            // Another song started.
        }
    }

    /// <summary>The year comes from the library's copy of the song; songs from sources and files have none here.</summary>
    private async Task RefreshYearAsync(NowPlayingInfo? np)
    {
        if (np?.Track is not TrackRef.Library song)
        {
            return;
        }

        var track = await _s.QueryAsync(c => c.Track(song.Id));
        if (NowPlaying?.Uid == np.Uid && !IsRemote)
        {
            Year = track.Value?.Year?.ToString(_s.L.Culture) ?? string.Empty;
        }
    }

    /// <summary>Lyrics in About This Song: their timing and their file (or the file's tags), or "None found".</summary>
    private string DescribeLyrics(LyricsState? lyrics)
    {
        if (NowPlaying is not { } np || lyrics is null || lyrics.Uid != np.Uid)
        {
            return string.Empty; // not looked up yet
        }

        if (lyrics.Doc is not { } doc)
        {
            return _s.L.Get("Lyrics_InfoNone");
        }

        var timing = _s.L.Get(doc.Timing switch
        {
            LyricTiming.Word => "Lyrics_InfoWord",
            LyricTiming.Line => "Lyrics_InfoLine",
            _ => "Lyrics_InfoUnsynced",
        });
        if (lyrics.Source is LyricsOrigin.Made made)
        {
            return _s.L.Format("AiLyrics_MadeBy", made.Engine);
        }

        var source = lyrics.Source switch
        {
            LyricsOrigin.Sidecar sidecar => Path.GetFileName(sidecar.Path),
            LyricsOrigin.Embedded => _s.L.Get("Lyrics_InfoTags"),
            LyricsOrigin.Plugin plugin => _s.Hub.State.Sources.Find(plugin.Id)?.Name,
            _ => null,
        };
        return string.IsNullOrEmpty(source) ? timing : _s.L.Format("Lyrics_InfoFrom", timing, source);
    }

    private async Task RefreshFavoriteAsync(string? key, TrackRef? track)
    {
        if (track is null || !CanFavorite)
        {
            IsFavorite = false;
            return;
        }

        var result = await _s.QueryAsync(c => c.IsFavorite(track));
        if (_trackKey == key)
        {
            IsFavorite = result.IsOk && result.Value;
        }
    }

    private void UpdateDuration(long? durationUs)
    {
        var duration = durationUs ?? (NowPlaying?.DurationMs is { } ms ? (long)ms * 1_000 : 0);
        Progress.SetDuration(duration);
        BarProgress.SetDuration(duration);
        MiniProgress.SetDuration(duration);
        if (duration > 0 && HasTrack)
        {
            LengthText = TimeFormat.Clock(duration);
        }
    }

    private string Describe(TrackFormat f)
    {
        var rate = (f.SampleRate / 1000.0).ToString(f.SampleRate % 1000 == 0 ? "0" : "0.#", _s.L.Culture);
        var text = _s.L.Format("Format_CodecRate", f.Codec.ToUpperInvariant(), rate);
        if (f.Bitrate is { } bps)
        {
            text = _s.L.Format("Format_WithBitrate", text, bps / 1000);
        }

        return f.Gapless ? _s.L.Format("Format_Gapless", text) : text;
    }

    private void ApplyDevices(DevicesState devices)
    {
        _syncingDevice = true;
        try
        {
            Devices.Clear();
            Devices.Add(new DeviceOption(null, _s.L.Get("Device_SystemDefault"), true));
            foreach (var d in devices.Devices)
            {
                Devices.Add(new DeviceOption(d.Id, d.Name, d.IsDefault));
            }

            SelectedDevice = devices.Selected is null
                ? Devices[0]
                : Devices.FirstOrDefault(d => d.Id == devices.Selected || d.Name == devices.Selected) ?? Devices[0];
        }
        finally
        {
            _syncingDevice = false;
        }
    }
}
