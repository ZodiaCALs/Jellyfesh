using Jellyfesh.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Shell;

/// <summary>
/// <c>Jellyfesh.exe --smoke-test</c> (docs/windows.md, "When Jellyfesh does not start"): once the
/// main window is open, wait for its first frames, open every section in turn (each page loaded and
/// drawn), open and close the mini player (ADR-0026 §4), narrow the window to the phone layout and
/// open the tabs' places again, then close the
/// window, which shuts the core down, and exit with
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
                await OpenAsync(window, section);
            }

            // The mini player opens and closes (nothing plays, so it is empty).
            Startup.EnterStage("smoke test: opening and closing the mini player");
            window.EnterCompact(evenWithoutTrack: true);
            await FramesAsync(Frames);
            window.ExitCompact();
            await FramesAsync(Frames);

            // The phone layout (ADR-0023): narrowed below 761 epx, the window shows the tab bar and,
            // on the library's pages, the Library switcher; their styles first load here.
            Startup.EnterStage("smoke test: narrowing the window to the phone layout");
            window.ResizeForSmokeTest(600, 860);
            for (var frame = 0; frame < 300 && !window.UsesPhoneLayout; frame++)
            {
                await FramesAsync(1);
            }

            if (window.UsesPhoneLayout)
            {
                foreach (var section in MainWindow.SmokePhoneSections)
                {
                    await OpenAsync(window, section);
                }
            }
            else
            {
                AppLog.Write("smoke test: the window did not narrow, so the phone layout was not opened");
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

    /// <summary>Opens a place and waits until its page has loaded and been drawn.</summary>
    private static async Task OpenAsync(MainWindow window, string section)
    {
        Startup.EnterStage($"smoke test: opening {section}");
        var page = window.ShowSection(section)
            ?? throw new InvalidOperationException($"no page opened for {section}");
        await LoadedAsync(page);
        await FramesAsync(Frames);
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
