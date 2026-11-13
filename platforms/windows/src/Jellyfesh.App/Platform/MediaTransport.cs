using System.ComponentModel;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Dispatching;
using Windows.Media;
using Windows.Storage;
using Windows.Storage.Streams;

namespace Jellyfesh.App.Platform;

/// <summary>
/// System Media Transport Controls for the window (ISystemMediaTransportControlsInterop
/// GetForWindow, what Chromium uses with its own audio engine; architecture §11): media keys, the
/// volume flyout and the lock screen show and control what the core plays. Updated from the
/// player's state and from playback anchors, never on a timer.
/// </summary>
internal sealed class MediaTransport : IDisposable
{
    private readonly SystemMediaTransportControls _smtc;
    private readonly PlayerViewModel _player;
    private readonly ShellServices _s;
    private readonly DispatcherQueue _queue;
    private ulong? _shownUid;

    public MediaTransport(nint hwnd, PlayerViewModel player, ShellServices services, DispatcherQueue queue)
    {
        _player = player;
        _s = services;
        _queue = queue;
        _smtc = SystemMediaTransportControlsInterop.GetForWindow(hwnd);
        _smtc.IsEnabled = true;
        _smtc.IsPlayEnabled = true;
        _smtc.IsPauseEnabled = true;
        _smtc.IsStopEnabled = true;
        _smtc.IsNextEnabled = true;
        _smtc.IsPreviousEnabled = true;
        _smtc.ButtonPressed += OnButtonPressed;
        _smtc.PlaybackPositionChangeRequested += OnPositionRequested;
        _player.PropertyChanged += OnPlayerChanged;
        _s.Hub.Changed += OnSlices;
        Update();
    }

    public void Dispose()
    {
        _smtc.ButtonPressed -= OnButtonPressed;
        _smtc.PlaybackPositionChangeRequested -= OnPositionRequested;
        _player.PropertyChanged -= OnPlayerChanged;
        _s.Hub.Changed -= OnSlices;
        _smtc.IsEnabled = false;
    }

    // Raised on a background thread.
    private void OnButtonPressed(SystemMediaTransportControls sender, SystemMediaTransportControlsButtonPressedEventArgs args)
    {
        var button = args.Button;
        _queue.TryEnqueue(() =>
        {
            switch (button)
            {
                case SystemMediaTransportControlsButton.Play:
                    _s.Send(new CoreCommand.Play());
                    break;
                case SystemMediaTransportControlsButton.Pause:
                    _s.Send(new CoreCommand.Pause());
                    break;
                case SystemMediaTransportControlsButton.Stop:
                    _s.Send(new CoreCommand.Stop());
                    break;
                case SystemMediaTransportControlsButton.Next:
                    _s.Send(new CoreCommand.Next());
                    break;
                case SystemMediaTransportControlsButton.Previous:
                    _s.Send(new CoreCommand.Previous());
                    break;
            }
        });
    }

    private void OnPositionRequested(SystemMediaTransportControls sender, PlaybackPositionChangeRequestedEventArgs args)
    {
        var position = args.RequestedPlaybackPosition;
        _queue.TryEnqueue(() => _player.SeekToUs((long)position.TotalMicroseconds));
    }

    private void OnPlayerChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(PlayerViewModel.Title) or nameof(PlayerViewModel.Artist) or nameof(PlayerViewModel.Album)
            or nameof(PlayerViewModel.ArtworkPath) or nameof(PlayerViewModel.IsPlaying) or nameof(PlayerViewModel.HasTrack)
            or nameof(PlayerViewModel.CanGoNext) or nameof(PlayerViewModel.CanGoPrevious))
        {
            Update();
        }
    }

    private void OnSlices(SliceChange changes)
    {
        if (changes.HasFlag(SliceChange.Playback))
        {
            var p = _s.Hub.State.Playback;
            var duration = TimeSpan.FromMicroseconds(Math.Max(0, p.Anchor.DurationUs ?? 0));
            _smtc.UpdateTimelineProperties(new SystemMediaTransportControlsTimelineProperties
            {
                StartTime = TimeSpan.Zero,
                MinSeekTime = TimeSpan.Zero,
                EndTime = duration,
                MaxSeekTime = duration,
                Position = TimeSpan.FromMicroseconds(Math.Clamp(p.Anchor.PositionUs, 0, Math.Max(0, p.Anchor.DurationUs ?? long.MaxValue))),
            });
        }
    }

    private async void Update()
    {
        _smtc.PlaybackStatus = !_player.HasTrack ? MediaPlaybackStatus.Closed
            : _player.IsPlaying ? MediaPlaybackStatus.Playing
            : _player.Status == PlaybackStatus.Stopped ? MediaPlaybackStatus.Stopped
            : MediaPlaybackStatus.Paused;
        _smtc.IsNextEnabled = _player.CanGoNext;
        _smtc.IsPreviousEnabled = _player.CanGoPrevious;
        var uid = _player.NowPlaying?.Uid;
        if (uid == _shownUid)
        {
            return;
        }

        _shownUid = uid;
        var updater = _smtc.DisplayUpdater;
        updater.ClearAll();
        if (_player.NowPlaying is null)
        {
            updater.Update();
            return;
        }

        updater.Type = MediaPlaybackType.Music;
        updater.MusicProperties.Title = _player.Title;
        updater.MusicProperties.Artist = _player.Artist;
        updater.MusicProperties.AlbumTitle = _player.Album;
        try
        {
            if (_player.ArtworkPath is { } path)
            {
                var file = await StorageFile.GetFileFromPathAsync(path);
                if (_shownUid != uid)
                {
                    return; // the track changed while the file opened
                }

                updater.Thumbnail = RandomAccessStreamReference.CreateFromFile(file);
            }
            else if (_player.ArtworkUri is { } uri && uri.StartsWith("http", StringComparison.OrdinalIgnoreCase))
            {
                updater.Thumbnail = RandomAccessStreamReference.CreateFromUri(new Uri(uri));
            }
        }
        catch (Exception)
        {
            // No thumbnail: the flyout shows the app icon instead.
        }

        updater.Update();
    }
}
