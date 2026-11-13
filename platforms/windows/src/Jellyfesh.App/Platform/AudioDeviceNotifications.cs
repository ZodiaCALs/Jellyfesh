using Jellyfesh.Core;
using Microsoft.UI.Dispatching;
using Windows.Devices.Enumeration;
using Windows.Media.Devices;

namespace Jellyfesh.App.Platform;

/// <summary>
/// Tells the core when output devices come, go or the default changes (Command.RefreshDevices):
/// the core never polls (ADR-0009), so the shell forwards the OS notifications, coalesced.
/// </summary>
internal sealed class AudioDeviceNotifications : IDisposable
{
    private readonly Action<CoreCommand> _send;
    private readonly DispatcherQueueTimer _debounce;
    private readonly DeviceWatcher? _watcher;
    private readonly Windows.Foundation.TypedEventHandler<object, DefaultAudioRenderDeviceChangedEventArgs> _defaultChanged;
    private bool _enumerated;

    public AudioDeviceNotifications(DispatcherQueue queue, Action<CoreCommand> send)
    {
        _send = send;
        _debounce = queue.CreateTimer();
        _debounce.IsRepeating = false;
        _debounce.Interval = TimeSpan.FromMilliseconds(300);
        _debounce.Tick += (_, _) =>
        {
            _debounce.Stop();
            _send(new CoreCommand.RefreshDevices());
        };
        _defaultChanged = (_, _) => queue.TryEnqueue(Kick);
        MediaDevice.DefaultAudioRenderDeviceChanged += _defaultChanged;
        try
        {
            _watcher = DeviceInformation.CreateWatcher(DeviceClass.AudioRender);
            _watcher.Added += (_, _) => Changed(queue);
            _watcher.Removed += (_, _) => Changed(queue);
            _watcher.EnumerationCompleted += (_, _) => _enumerated = true;
            _watcher.Start();
        }
        catch (Exception)
        {
            _watcher = null; // default-device changes still arrive
        }

        void Changed(DispatcherQueue q)
        {
            if (_enumerated)
            {
                q.TryEnqueue(Kick);
            }
        }
    }

    public void Dispose()
    {
        MediaDevice.DefaultAudioRenderDeviceChanged -= _defaultChanged;
        try
        {
            _watcher?.Stop();
        }
        catch (Exception)
        {
            // Already stopped.
        }

        _debounce.Stop();
    }

    private void Kick()
    {
        _debounce.Stop();
        _debounce.Start();
    }
}
