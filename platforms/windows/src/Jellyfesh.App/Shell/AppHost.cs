using Jellyfesh.Services;
using Jellyfesh.App.Platform;
using Jellyfesh.Formatting;
using Jellyfesh.ViewModels;

namespace Jellyfesh.App.Shell;

/// <summary>What pages reach for: the window's view model, platform services and strings. Set once by MainWindow.</summary>
internal static class AppHost
{
    private static AppViewModel? _app;

    public static AppViewModel App => _app ?? throw new InvalidOperationException("the main window has not started yet");

    public static ShellServices Services => App.Services;

    public static ILocalizer L => App.Services.L;

    public static ShellPlatform Platform { get; private set; } = null!;

    public static MainWindow Window { get; private set; } = null!;

    /// <summary>The one layout's measures (ADR-0023) that pages bind their padding and titles to.</summary>
    public static ShellMetrics Metrics { get; } = new();

    /// <summary>The website and its pages (About, the legal links), when the build knows the website (JfWebsite).</summary>
    public static AboutInfo.Links? Website { get; } = AboutInfo.LinksFor(
        System.Reflection.CustomAttributeExtensions.GetCustomAttributes<System.Reflection.AssemblyMetadataAttribute>(typeof(AppHost).Assembly)
            .FirstOrDefault(a => a.Key == "JfWebsite")?.Value);

    public static void Set(AppViewModel app, ShellPlatform platform, MainWindow window)
    {
        _app = app;
        Platform = platform;
        Window = window;
    }
}
