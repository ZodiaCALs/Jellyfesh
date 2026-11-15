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
/// What is playing and the transport: Now Playing, the mini player, media keys (SMTC) and the
/// taskbar buttons all bind to this. It mirrors the core's slices and only sends commands.
/// </summary>
public sealed partial class PlayerViewModel : ObservableObject
{
    /// <summary>After the person moves the volume, slices that still carry the old value are ignored for this long.</summary>
    private static readonly TimeSpan VolumeEchoGuard = TimeSpan.FromMilliseconds(600);

    private readonly ShellServices _s;
    private long _lastVolumeInputMs = long.MinValue / 2;
    private bool _syncingDevice;

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

    public PlayerViewModel(ShellServices services)
    {
        _s = services;
        Progress = new ProgressPresenter(services.L, services.Time, showsTime: true);
        BarProgress = new ProgressPresenter(services.L, services.Time, showsTime: true);
        MiniProgress = new ProgressPresenter(services.L, services.Time, showsTime: false);
        services.Ticker.Attach(Progress);
        services.Ticker.Attach(BarProgress);
        services.Ticker.Attach(MiniProgress);
    }

    /// <summary>The seek bar and its times on Now Playing.</summary>
    public ProgressPresenter Progress { get; }

    /// <summary>The seek bar and its times on the player bar (windows 900 px and wider).</summary>
    public ProgressPresenter BarProgress { get; }

    /// <summary>The mini player's progress line (and the compact player's ring).</summary>
    public ProgressPresenter MiniProgress { get; }

    public ObservableCollection<DeviceOption> Devices { get; } = [];

    public NowPlayingInfo? NowPlaying { get; private set; }

    public PlaybackStatus Status { get; private set; }

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

        if (changes.HasFlag(SliceChange.Muted))
        {
            IsMuted = state.Muted;
        }

        if (changes.HasFlag(SliceChange.Devices))
        {
            ApplyDevices(state.Devices);
        }
    }

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

    /// <summary>Off → All → One → Off.</summary>
    [RelayCommand]
    private void CycleRepeat() => _s.Send(new CoreCommand.SetRepeat(Repeat switch
    {
        RepeatMode.Off => RepeatMode.All,
        RepeatMode.All => RepeatMode.One,
        _ => RepeatMode.Off,
    }));

    [RelayCommand]
    private void ToggleMute() => _s.Send(new CoreCommand.SetMuted(!IsMuted));

    [RelayCommand]
    private void ToggleFavorite()
    {
        if (NowPlaying is { } np)
        {
            _s.Send(new CoreCommand.SetFavorite(np.Track, !IsFavorite));
            IsFavorite = !IsFavorite; // the next library revision confirms it
        }
    }

    /// <summary>The person set the volume (slider, wheel, keys); 0 to 100.</summary>
    public void SetVolumeFromUser(double percent)
    {
        percent = Math.Clamp(Math.Round(percent), 0, 100);
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
    }

    partial void OnIsPlayingChanged(bool value) => OnPropertyChanged(nameof(PlayPauseLabel));

    partial void OnShuffleChanged(bool value) => OnPropertyChanged(nameof(ShuffleLabel));

    partial void OnRepeatChanged(RepeatMode value) => OnPropertyChanged(nameof(RepeatLabel));

    partial void OnIsMutedChanged(bool value) => OnPropertyChanged(nameof(MuteLabel));

    partial void OnIsFavoriteChanged(bool value) => OnPropertyChanged(nameof(FavoriteLabel));

    private void ApplyNowPlaying(NowPlayingInfo? np)
    {
        var changedTrack = NowPlaying?.Uid != np?.Uid;
        NowPlaying = np;
        HasTrack = np is not null;
        Title = np?.Title ?? string.Empty;
        Artist = np?.Artist ?? (np is null ? string.Empty : _s.L.Get("Library_UnknownArtist"));
        Album = np?.Album ?? string.Empty;
        Subtitle = np is null ? string.Empty
            : np.Album is null ? Artist
            : _s.L.Format("Player_ArtistAlbum", Artist, np.Album);
        ArtworkUri = np?.Artwork?.ToUriString();
        ArtworkPath = np?.Artwork is ArtworkRef.File f ? f.Path : null;
        FormatText = np?.Format is { } fmt ? Describe(fmt) : string.Empty;
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
            IsBuffering = false;
            Year = string.Empty;
            _ = RefreshFavoriteAsync(np);
            _ = RefreshYearAsync(np);
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
        if (NowPlaying?.Uid == np.Uid)
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
        var source = lyrics.Source switch
        {
            LyricsOrigin.Sidecar sidecar => Path.GetFileName(sidecar.Path),
            LyricsOrigin.Embedded => _s.L.Get("Lyrics_InfoTags"),
            _ => null,
        };
        return string.IsNullOrEmpty(source) ? timing : _s.L.Format("Lyrics_InfoFrom", timing, source);
    }

    private async Task RefreshFavoriteAsync(NowPlayingInfo? np)
    {
        if (np is null || !CanFavorite)
        {
            IsFavorite = false;
            return;
        }

        var result = await _s.QueryAsync(c => c.IsFavorite(np.Track));
        if (NowPlaying?.Uid == np.Uid)
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
