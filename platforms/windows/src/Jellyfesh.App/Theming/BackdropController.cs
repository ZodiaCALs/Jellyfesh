using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Microsoft.Windows.System.Power;
using Windows.UI.ViewManagement;

namespace Jellyfesh.App.Theming;

/// <summary>
/// The window material and the glass fallbacks (ADR-0007): Mica on Windows 11; the solid field on
/// Windows 10 (Mica falls back to a solid colour there anyway); and solid Cloud for every glass
/// surface when transparency effects are off, in Battery Saver, in high contrast or with Reduce
/// effects (Misty MAT-08). Blur and tint never animate (MAT-10): a change swaps them.
/// </summary>
internal sealed class BackdropController
{
    private readonly Window _window;
    private readonly DispatcherQueue _queue;
    private readonly UISettings _ui = new();
    private readonly AccessibilitySettings _accessibility = new();
    private MicaBackdrop? _mica;
    private bool _reduceEffects;

    public BackdropController(Window window)
    {
        _window = window;
        _queue = window.DispatcherQueue;
        // These fire on background threads. Each subscription is guarded: some appeared after
        // Windows 10 1809, the minimum (ADR-0007), and a missing one only loses live updates.
        Try(() => _ui.AdvancedEffectsEnabledChanged += (_, _) => _queue.TryEnqueue(Refresh));
        Try(() => _accessibility.HighContrastChanged += (_, _) => _queue.TryEnqueue(Refresh));
        Try(() => PowerManager.EnergySaverStatusChanged += (_, _) => _queue.TryEnqueue(Refresh));
    }

    /// <summary>True when Mica shows through the window (the content then leaves the pane clear).</summary>
    public bool HasMica { get; private set; }

    public bool IsHighContrast => _accessibility.HighContrast;

    /// <summary>Raised after the material changed (UI thread).</summary>
    public event Action? Changed;

    public void SetReduceEffects(bool reduce)
    {
        _reduceEffects = reduce;
        Refresh();
    }

    public void Refresh()
    {
        var batterySaver = false;
        Try(() => batterySaver = PowerManager.EnergySaverStatus == EnergySaverStatus.On);
        var solid = _reduceEffects || !_ui.AdvancedEffectsEnabled || batterySaver || _accessibility.HighContrast;
        var mica = !solid && MicaController.IsSupported();
        if (mica)
        {
            _mica ??= new MicaBackdrop { Kind = MicaKind.Base };
            if (!ReferenceEquals(_window.SystemBackdrop, _mica))
            {
                _window.SystemBackdrop = _mica;
            }
        }
        else if (_window.SystemBackdrop is not null)
        {
            _window.SystemBackdrop = null;
        }

        HasMica = mica;
        ThemeBrushes.SetSolidGlass(solid);
        Changed?.Invoke();
    }

    private static void Try(Action action)
    {
        try
        {
            action();
        }
        catch (Exception)
        {
            // Not available on this Windows version.
        }
    }
}
