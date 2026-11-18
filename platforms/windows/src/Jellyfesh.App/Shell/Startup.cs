using System.Globalization;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using Jellyfesh.App.Platform;
using Jellyfesh.Services;

namespace Jellyfesh.App.Shell;

/// <summary>
/// Start-up's safety net (docs/windows.md, "When Jellyfesh does not start"). It writes the log's
/// start record and each stage, logs first-chance exceptions until the window is open, and decides
/// what a failure does: before the main window is open, the failure is logged and shown (the error
/// window, or a message box when XAML itself could not start) and Jellyfesh then exits. It never
/// stays alive without a window, which would also swallow every later start through the
/// single-instance redirect. With --smoke-test nothing is shown: a failure exits with its code.
/// </summary>
internal static class Startup
{
    /// <summary>How long a smoke test may take, start to exit, before the watchdog ends it.</summary>
    private static readonly TimeSpan SmokeBudget = TimeSpan.FromMinutes(3);

    /// <summary>How long the process may take to end after Exit before it is ended outright.</summary>
    private static readonly TimeSpan ExitGrace = TimeSpan.FromSeconds(10);

    private const int MaxFirstChance = 50;

    private static int _firstChanceCount;
    private static int _failed;
    private static Timer? _watchdog;

    [ThreadStatic]
    private static bool _inFirstChance;

    public static StartupOptions Options { get; private set; } = new(false);

    public static bool SmokeTest => Options.SmokeTest;

    /// <summary>What Main returns once the app has ended normally.</summary>
    public static int ExitCode { get; set; } = ExitCodes.Ok;

    /// <summary>The step being run, for the log and for the error message ("While opening the main window: …").</summary>
    public static string Stage { get; private set; } = "starting";

    /// <summary>True once the main window is open: from then on a failure is logged and the app goes on.</summary>
    public static bool WindowOpen { get; private set; }

    /// <summary>The first thing Main does: the log's start record and the crash handlers.</summary>
    public static void Begin(string[] args)
    {
        Options = StartupOptions.Parse(args);
        AppDomain.CurrentDomain.FirstChanceException += OnFirstChance;
        AppDomain.CurrentDomain.UnhandledException += OnUnhandled;
        TaskScheduler.UnobservedTaskException += OnUnobserved;
        WriteStartRecord();
        if (SmokeTest)
        {
            ArmWatchdog(SmokeBudget, ExitCodes.TimedOut, "the smoke test");
        }
    }

    public static void EnterStage(string stage)
    {
        Stage = stage;
        AppLog.Write($"start-up: {stage}");
    }

    /// <summary>The main window is open: first-chance exceptions are no longer logged (except in a smoke test).</summary>
    public static void MarkWindowOpen()
    {
        WindowOpen = true;
        Stage = "running";
        AppLog.Write("start-up: the main window is open");
    }

    /// <summary>
    /// A failure that stops Jellyfesh from starting (or, in a smoke test, any failure): logged in
    /// full, shown with the log's path, then the process exits. <paramref name="xamlWorks"/> says
    /// whether a WinUI window can still be made; when not, a message box says it instead. Only the
    /// first failure is shown; later ones are logged.
    /// </summary>
    public static void Fail(Exception error, bool xamlWorks)
    {
        AppLog.Write($"start-up failed while {Stage}", error);
        if (Interlocked.Exchange(ref _failed, 1) == 1)
        {
            return;
        }

        if (SmokeTest)
        {
            Exit(ExitCodes.StartupFailed);
            return;
        }

        var summary = FailureText.Describe(Stage, error);
        if (xamlWorks && StartupErrorWindow.TryShow(summary))
        {
            return; // it exits when closed
        }

        StartupErrorWindow.ShowMessageBox(summary);
        Exit(ExitCodes.StartupFailed);
    }

    /// <summary>
    /// Ends the process with <paramref name="code"/>. Should anything hold up the runtime's shutdown
    /// (a native thread, a DLL's detach), the watchdog ends it outright after a few seconds.
    /// </summary>
    public static void Exit(int code)
    {
        ExitCode = code;
        AppLog.Write($"exiting with code {code}");
        ArmWatchdog(ExitGrace, code, "exiting");
        Environment.Exit(code);
    }

    /// <summary>
    /// Ends the process with <paramref name="code"/> if it is still running after
    /// <paramref name="after"/> (one watchdog at a time; arming another replaces it).
    /// </summary>
    public static void ArmWatchdog(TimeSpan after, int code, string what)
    {
        var timer = new Timer(_ =>
        {
            AppLog.Write($"{what} did not end within {after.TotalSeconds:0} s (stage: {Stage}); ending the process with code {code}");
            Win32.TerminateProcess(Win32.GetCurrentProcess(), (uint)code);
        });
        Interlocked.Exchange(ref _watchdog, timer)?.Dispose();
        timer.Change(after, Timeout.InfiniteTimeSpan);
    }

    private static void WriteStartRecord()
    {
        var assembly = typeof(Startup).Assembly;
        var version = assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "unknown";
        string Metadata(string key) =>
            assembly.GetCustomAttributes<AssemblyMetadataAttribute>().FirstOrDefault(a => a.Key == key)?.Value ?? "unknown";
        AppLog.Write($"Jellyfesh {version} ({Metadata("JfEdition")} edition, {Metadata("JfDistribution")} distribution) starting{(SmokeTest ? " a smoke test" : string.Empty)}");
        AppLog.Write($"  app: {Environment.ProcessPath ?? AppContext.BaseDirectory}");
        AppLog.Write(string.Create(
            CultureInfo.InvariantCulture,
            $"  Windows {Environment.OSVersion.Version}, {RuntimeInformation.OSArchitecture} (process {RuntimeInformation.ProcessArchitecture}), .NET {Environment.Version}, language {CultureInfo.CurrentUICulture.Name}"));
        AppLog.Write($"  log: {AppLog.FilePath}");
    }

    private static void OnFirstChance(object? sender, FirstChanceExceptionEventArgs e)
    {
        if (_inFirstChance || AppLog.Busy || (WindowOpen && !SmokeTest))
        {
            return;
        }

        _inFirstChance = true;
        try
        {
            if (Interlocked.Increment(ref _firstChanceCount) <= MaxFirstChance)
            {
                AppLog.Write($"  first-chance {e.Exception.GetType().FullName} while {Stage}: {e.Exception.Message}");
            }
        }
        catch (Exception)
        {
            // Never from here.
        }
        finally
        {
            _inFirstChance = false;
        }
    }

    /// <summary>An exception no code caught, on any thread: the runtime ends the process after this.</summary>
    private static void OnUnhandled(object sender, System.UnhandledExceptionEventArgs e)
    {
        AppLog.Write($"fatal: an unhandled exception ends Jellyfesh (stage: {Stage})", e.ExceptionObject as Exception);
        if (SmokeTest)
        {
            Exit(ExitCodes.StartupFailed);
        }
    }

    private static void OnUnobserved(object? sender, UnobservedTaskExceptionEventArgs e)
    {
        AppLog.Write("a background task failed and nobody looked", e.Exception);
        e.SetObserved();
    }
}
