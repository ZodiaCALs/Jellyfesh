using System.Globalization;
using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Microsoft.UI.Dispatching;
using Microsoft.Windows.ApplicationModel.Resources;

namespace Jellyfesh.App.Platform;

/// <summary>The UI thread's DispatcherQueue as the hub's dispatcher.</summary>
internal sealed class WinUiDispatcher(DispatcherQueue queue) : IUiDispatcher
{
    public bool HasThreadAccess => queue.HasThreadAccess;

    public void Post(Action action) => queue.TryEnqueue(DispatcherQueuePriority.Normal, () => action());
}

/// <summary>
/// The playback ticker's one-shot timer: a DispatcherQueueTimer that fires once. Nothing runs
/// between ticks, and nothing at all while the ticker has stopped it (paused, Still mode).
/// </summary>
internal sealed class DispatcherOneShotTimer : IOneShotTimer
{
    private readonly DispatcherQueueTimer _timer;

    public DispatcherOneShotTimer(DispatcherQueue queue)
    {
        _timer = queue.CreateTimer();
        _timer.IsRepeating = false;
        _timer.Tick += (_, _) =>
        {
            _timer.Stop();
            Tick?.Invoke(this, EventArgs.Empty);
        };
    }

    public event EventHandler? Tick;

    public void Start(TimeSpan delay)
    {
        _timer.Stop();
        _timer.Interval = delay;
        _timer.Start();
    }

    public void Stop() => _timer.Stop();
}

/// <summary>Strings from the .resw files through MRT Core (unpackaged: Jellyfesh.pri next to the executable).</summary>
internal sealed class ResourceLocalizer : ILocalizer
{
    /// <summary>How many missing strings the log names per run.</summary>
    private const int MissingLogged = 20;

    private readonly ResourceLoader? _loader;
    private int _missing;

    public ResourceLocalizer()
    {
        try
        {
            _loader = new ResourceLoader();
        }
        catch (Exception e)
        {
            // A missing resource index shows keys rather than failing to start; the log says so.
            AppLog.Write("the app's strings could not be loaded (MRT Core found no resource index); keys show instead", e);
            _loader = null;
        }
    }

    public CultureInfo Culture => CultureInfo.CurrentUICulture;

    public string Get(string key)
    {
        try
        {
            var value = _loader?.GetString(key);
            return string.IsNullOrEmpty(value) ? key : value;
        }
        catch (Exception e)
        {
            // MRT Core throws for a string it does not have: the key shows, and the log names it.
            if (Interlocked.Increment(ref _missing) <= MissingLogged)
            {
                AppLog.Write($"no string {key}", e);
            }

            return key;
        }
    }
}
