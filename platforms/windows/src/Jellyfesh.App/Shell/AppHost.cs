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

    public static void Set(AppViewModel app, ShellPlatform platform, MainWindow window)
    {
        _app = app;
        Platform = platform;
        Window = window;
    }
}

/// <summary>A small log in the local app data folder, for problems the window cannot show.</summary>
internal static class AppLog
{
    private static readonly object Gate = new();

    public static string Folder { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Jellyfesh", "logs");

    public static void Write(string what, Exception? e = null)
    {
        try
        {
            lock (Gate)
            {
                Directory.CreateDirectory(Folder);
                var file = Path.Combine(Folder, "app.log");
                if (File.Exists(file) && new FileInfo(file).Length > 1_000_000)
                {
                    File.Move(file, Path.Combine(Folder, "app.old.log"), overwrite: true);
                }

                File.AppendAllText(file, $"{DateTimeOffset.Now:O} {what}{(e is null ? string.Empty : ": " + e)}{Environment.NewLine}");
            }
        }
        catch (Exception)
        {
            // Logging must never take the app down.
        }
    }
}
