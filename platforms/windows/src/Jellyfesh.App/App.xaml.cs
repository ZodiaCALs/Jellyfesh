using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Core;
using Jellyfesh.Interop;
using Jellyfesh.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App;

/// <summary>
/// The application: installs the theme brushes and the control style dictionary, starts the core
/// (with the bundled official plugins) and opens the main window. Shells hold no business logic
/// (ADR-0001): everything after start-up is the view models in Jellyfesh.Shared.
/// </summary>
public partial class App : Application
{
    private static MainWindow? _main;

    public App()
    {
        InitializeComponent();

        // The Jf* brushes first: the style dictionaries below refer to them.
        ThemeBrushes.Install(Resources);
        Resources.MergedDictionaries.Add(new ResourceDictionary { Source = new Uri("ms-appx:///Styles/Controls.xaml") });
        Resources.MergedDictionaries.Add(new Styles.Templates());
        UnhandledException += OnUnhandledException;
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        ICoreClient core;
        try
        {
            core = FfiCoreClient.Start(bundledPlugins: BundledPlugins.Find(AppContext.BaseDirectory));
        }
        catch (Exception e) when (e is CoreQueryException or DllNotFoundException or EntryPointNotFoundException or BadImageFormatException or TypeInitializationException)
        {
            AppLog.Write("core did not start", e);
            StartupErrorWindow.Show(e.Message);
            return;
        }

        _main = new MainWindow(core);
        _main.Activate();
    }

    /// <summary>Another start was redirected here: show the window (called on a background thread).</summary>
    internal static void BringToFront() => _main?.DispatcherQueue.TryEnqueue(() => _main?.BringToFront());

    private static void OnUnhandledException(object sender, Microsoft.UI.Xaml.UnhandledExceptionEventArgs e)
    {
        AppLog.Write("unhandled exception", e.Exception);

        // A view that failed to draw should not end playback; anything the runtime cannot recover from still ends the process.
        e.Handled = e.Exception is not (OutOfMemoryException or StackOverflowException or AccessViolationException);
    }
}

/// <summary>A minimal window that says why Jellyfesh could not start (no core, no main window).</summary>
internal static class StartupErrorWindow
{
    public static void Show(string message)
    {
        var l = new Platform.ResourceLocalizer();
        var window = new Window { Title = "Jellyfesh" };
        var panel = new StackPanel { Spacing = 12, Margin = new Thickness(32), MaxWidth = 560, HorizontalAlignment = HorizontalAlignment.Left };
        panel.Children.Add(new TextBlock { Text = l.Get("Startup_FailedTitle"), FontSize = 20, FontWeight = Microsoft.UI.Text.FontWeights.Bold, TextWrapping = TextWrapping.Wrap });
        panel.Children.Add(new TextBlock { Text = l.Get("Startup_FailedBody"), TextWrapping = TextWrapping.Wrap });
        panel.Children.Add(new TextBox { Text = message, IsReadOnly = true, TextWrapping = TextWrapping.Wrap });
        window.Content = panel;
        window.Activate();
    }
}
