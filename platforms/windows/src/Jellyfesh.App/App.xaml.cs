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
/// <remarks>
/// WinUI makes this object in Application.Start's callback, before XAML runs on the thread:
/// InitializeComponent only notes where App.xaml is, and Application.Resources does not exist yet
/// (it fails with E_UNEXPECTED). WinUI then creates the thread's application, loads App.xaml and
/// calls <see cref="OnLaunched"/>, where the resources are installed (microsoft-ui-xaml:
/// FrameworkApplication::StartDesktop, LoadComponent, DXamlCore::EnsureCoreApplicationInitialized).
/// </remarks>
public partial class App : Application
{
    private static MainWindow? _main;

    /// <summary>OnLaunched ran: XAML started on the thread, so a window can show a failure.</summary>
    private static bool _launched;

    /// <summary>How many of XAML's own warnings the log keeps per run.</summary>
    private const int XamlWarningsLogged = 20;

    private static int _xamlWarnings;

    public App()
    {
        // First, so that XAML's own failures reach the log: WinUI reports a failure to load
        // App.xaml, which happens after this constructor, through this event.
        UnhandledException += OnUnhandledException;
        try
        {
            Startup.EnterStage("creating the application");
            InitializeComponent();
        }
        catch (Exception e)
        {
            Startup.Fail(e, xamlWorks: false);
            return;
        }

        Startup.EnterStage("loading App.xaml (WinUI's resources)");
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        _launched = true;
        ICoreClient? core = null;
        try
        {
            // A resource a view refers to that does not resolve, and (in a smoke test) a failed
            // binding: XAML reports them here rather than as exceptions.
            DebugSettings.XamlResourceReferenceFailed += (_, e) => LogXamlWarning("resource reference failed", e.Message);
            if (Startup.SmokeTest)
            {
                DebugSettings.BindingFailed += (_, e) => LogXamlWarning("binding failed", e.Message);
            }

            // The Jf* brushes first: the style dictionaries below refer to them.
            Startup.EnterStage("loading the colours and styles");
            ThemeBrushes.Install(Resources);
            Resources.MergedDictionaries.Add(new ResourceDictionary { Source = new Uri("ms-appx:///Styles/Controls.xaml") });
            Resources.MergedDictionaries.Add(new Styles.Templates());

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
            // Before OnLaunched (App.xaml did not load) WinUI ends the process once this returns: a
            // message box, which waits, says why.
            e.Handled = true;
            Startup.Fail(e.Exception, xamlWorks: _launched);
            return;
        }

        // A view that failed to draw should not end playback; anything the runtime cannot recover from still ends the process.
        e.Handled = e.Exception is not (OutOfMemoryException or StackOverflowException or AccessViolationException);
    }

    private static void LogXamlWarning(string what, string message)
    {
        if (Interlocked.Increment(ref _xamlWarnings) <= XamlWarningsLogged)
        {
            AppLog.Write($"XAML {what} while {Startup.Stage}: {message}");
        }
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
