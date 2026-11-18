using Jellyfesh.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Shell;

/// <summary>
/// <c>Jellyfesh.exe --smoke-test</c> (docs/windows.md, "When Jellyfesh does not start"): once the
/// main window is open, wait for its first frames, open every section in turn (each page loaded and
/// drawn), then close the window, which shuts the core down, and exit with
/// <see cref="ExitCodes.Ok"/>. A failure anywhere, including one WinUI only reports through
/// UnhandledException or a failed navigation, exits with <see cref="ExitCodes.StartupFailed"/>, and
/// the watchdog ends a test that hangs (Startup). CI runs it on the published app.
/// </summary>
internal static class SmokeTest
{
    /// <summary>Frames to wait for after something has loaded, so that it has really been drawn.</summary>
    private const int Frames = 3;

    public static async void Run(MainWindow window)
    {
        try
        {
            Startup.EnterStage("smoke test: drawing the main window");
            await LoadedAsync(window.ContentRoot);
            await FramesAsync(Frames);

            foreach (var section in MainWindow.SmokeSections)
            {
                Startup.EnterStage($"smoke test: opening {section}");
                var page = window.ShowSection(section)
                    ?? throw new InvalidOperationException($"no page opened for {section}");
                await LoadedAsync(page);
                await FramesAsync(Frames);
            }

            Startup.EnterStage("smoke test: closing");
            AppLog.Write("smoke test: every page opened and was drawn");
            Startup.ExitCode = ExitCodes.Ok;
            window.Close(); // shuts the core down; Main returns the exit code once the last window has closed
        }
        catch (Exception e)
        {
            Startup.Fail(e, xamlWorks: true);
        }
    }

    private static Task LoadedAsync(FrameworkElement element)
    {
        if (element.IsLoaded)
        {
            return Task.CompletedTask;
        }

        var loaded = new TaskCompletionSource();
        void OnLoaded(object sender, RoutedEventArgs e)
        {
            element.Loaded -= OnLoaded;
            loaded.TrySetResult();
        }

        element.Loaded += OnLoaded;
        return loaded.Task;
    }

    /// <summary>Completes after <paramref name="count"/> frames have been rendered.</summary>
    private static Task FramesAsync(int count)
    {
        var done = new TaskCompletionSource();
        var seen = 0;
        void OnRendering(object? sender, object e)
        {
            if (++seen >= count)
            {
                CompositionTarget.Rendering -= OnRendering;
                done.TrySetResult();
            }
        }

        CompositionTarget.Rendering += OnRendering;
        return done.Task;
    }
}
