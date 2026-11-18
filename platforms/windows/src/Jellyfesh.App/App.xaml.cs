using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Core;
using Jellyfesh.Interop;
using Jellyfesh.Services;
using Microsoft.UI.Xaml;

namespace Jellyfesh.App;

/// <summary>
/// The application: installs the theme brushes and the control style dictionary, starts the core
/// (with the bundled official plugins) and opens the main window. Shells hold no business logic
/// (ADR-0001): everything after start-up is the view models in Jellyfesh.Shared. Each step is a
/// stage of Startup: a failure is logged and shown, and Jellyfesh then exits rather than lingering
/// without a window (docs/windows.md, "When Jellyfesh does not start").
/// </summary>
public partial class App : Application
{
    private static MainWindow? _main;

    public App()
    {
        // First, so that XAML's own failures from here on reach the log.
        UnhandledException += OnUnhandledException;
        try
        {
            Startup.EnterStage("loading App.xaml (WinUI's resources)");
            InitializeComponent();

            // The Jf* brushes first: the style dictionaries below refer to them.
            Startup.EnterStage("loading the colours and styles");
            ThemeBrushes.Install(Resources);
            Resources.MergedDictionaries.Add(new ResourceDictionary { Source = new Uri("ms-appx:///Styles/Controls.xaml") });
            Resources.MergedDictionaries.Add(new Styles.Templates());
        }
        catch (Exception e)
        {
            // Without its styles no window of the app can be drawn: a message box says why.
            Startup.Fail(e, xamlWorks: false);
        }
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        ICoreClient? core = null;
        try
        {
            Startup.EnterStage("starting the core");
            core = StartCore();

            Startup.EnterStage("opening the main window");
            _main = new MainWindow(core);

            Startup.EnterStage("showing the main window");
            _main.Activate();
            Startup.MarkWindowOpen();
            if (Startup.SmokeTest)
            {
                SmokeTest.Run(_main);
            }
        }
        catch (Exception e)
        {
            var window = _main;
            _main = null;
            if (window is not null)
            {
                Quietly(window.Close); // its Closed handler shuts the core down
            }
            else if (core is not null)
            {
                Quietly(core.Dispose);
            }

            Startup.Fail(e, xamlWorks: true);
        }
    }

    /// <summary>Another start was redirected here: show the window (called on a background thread).</summary>
    internal static void BringToFront() => _main?.DispatcherQueue.TryEnqueue(() => _main?.BringToFront());

    /// <summary>The core in its folders (logged, so a report says where the library is), with the plugins shipped in the app.</summary>
    private static FfiCoreClient StartCore()
    {
        var (data, cache) = FfiCoreClient.DefaultFolders();
        var plugins = BundledPlugins.Find(AppContext.BaseDirectory);
        AppLog.Write($"  data: {data}");
        AppLog.Write($"  cache: {cache}");
        AppLog.Write($"  bundled plugins: {plugins.Count}");
        return FfiCoreClient.Start(data, cache, plugins);
    }

    private static void OnUnhandledException(object sender, Microsoft.UI.Xaml.UnhandledExceptionEventArgs e)
    {
        // Message keeps the original exception's message when Exception does not (WinUI's remarks).
        AppLog.Write($"unhandled exception while {Startup.Stage}: {e.Message}", e.Exception);
        if (!Startup.WindowOpen || Startup.SmokeTest)
        {
            e.Handled = true;
            Startup.Fail(e.Exception, xamlWorks: true);
            return;
        }

        // A view that failed to draw should not end playback; anything the runtime cannot recover from still ends the process.
        e.Handled = e.Exception is not (OutOfMemoryException or StackOverflowException or AccessViolationException);
    }

    private static void Quietly(Action action)
    {
        try
        {
            action();
        }
        catch (Exception e)
        {
            AppLog.Write("cleaning up after a failed start failed too", e);
        }
    }
}
