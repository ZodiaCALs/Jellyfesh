using System.Globalization;
using System.Text;

namespace Jellyfesh.Services;

/// <summary>
/// How Jellyfesh.exe was started (docs/windows.md, "When Jellyfesh does not start"). With
/// <c>--smoke-test</c> it starts the core and the main window, opens every page, closes and exits
/// with <see cref="ExitCodes.Ok"/>; a failure on the way is logged and exits with another of
/// <see cref="ExitCodes"/>. CI runs it on the published app (scripts/smoke-test.ps1).
/// </summary>
public sealed record StartupOptions(bool SmokeTest)
{
    public const string SmokeTestArgument = "--smoke-test";

    public static StartupOptions Parse(IEnumerable<string> args) =>
        new(args.Any(a => string.Equals(a.Trim(), SmokeTestArgument, StringComparison.OrdinalIgnoreCase)));
}

/// <summary>What Jellyfesh.exe returns (docs/windows.md, "When Jellyfesh does not start").</summary>
public static class ExitCodes
{
    /// <summary>It ran and closed normally; with --smoke-test, every page opened.</summary>
    public const int Ok = 0;

    /// <summary>Start-up failed (or, with --smoke-test, anything failed): the log says where and why.</summary>
    public const int StartupFailed = 1;

    /// <summary>--smoke-test: the window did not show, or a page did not open, in time.</summary>
    public const int TimedOut = 2;

    /// <summary>A Jellyfesh that is already running took the start, or did not answer it.</summary>
    public const int AlreadyRunning = 3;

    /// <summary>The window closed, but shutting down did not end in time.</summary>
    public const int ShutdownHung = 4;
}

/// <summary>
/// Where the app log goes: the folder in <see cref="Variable"/> when it is set (the smoke test uses
/// it), else <c>%LOCALAPPDATA%\Jellyfesh\logs</c>, else the temporary folder, in that order of
/// preference: the first that can be written wins.
/// </summary>
public static class LogLocation
{
    /// <summary>The environment variable that moves the log, like JF_DATA_DIR and JF_CACHE_DIR move the core's folders.</summary>
    public const string Variable = "JF_LOG_DIR";

    public const string FileName = "app.log";

    public const string OldFileName = "app.old.log";

    /// <summary>The folders to try, best first, without empty or repeated ones.</summary>
    public static IReadOnlyList<string> Candidates(string? configured, string? localAppData, string? temp)
    {
        var folders = new List<string>();
        Add(configured);
        Add(string.IsNullOrWhiteSpace(localAppData) ? null : Path.Combine(localAppData, "Jellyfesh", "logs"));
        Add(string.IsNullOrWhiteSpace(temp) ? null : Path.Combine(temp, "Jellyfesh", "logs"));
        return folders;

        void Add(string? folder)
        {
            if (!string.IsNullOrWhiteSpace(folder) && !folders.Contains(folder.Trim(), StringComparer.OrdinalIgnoreCase))
            {
                folders.Add(folder.Trim());
            }
        }
    }
}

/// <summary>
/// A start-up failure in a few readable lines for the error window and the message box (the log
/// keeps the whole exception with its stack): the stage, then each exception in the chain with its
/// type, message and HRESULT.
/// </summary>
public static class FailureText
{
    /// <summary>At most this many exceptions of a chain are described (the log has them all).</summary>
    public const int MaxDepth = 4;

    /// <summary>At most this many frames of where a first-chance exception was thrown are logged.</summary>
    public const int MaxFrames = 12;

    /// <summary>
    /// A first-chance exception's log line: its type, HRESULT and message (a COMException's is often
    /// empty, so the HRESULT names it), then where it was thrown, innermost frame first, without
    /// the runtime's own frames.
    /// </summary>
    public static string FirstChance(Exception error, string stage, IEnumerable<string> frames)
    {
        var text = new StringBuilder();
        text.Append("  first-chance ").Append(error.GetType().FullName)
            .Append(" (HRESULT 0x").Append(error.HResult.ToString("X8", CultureInfo.InvariantCulture)).Append(')')
            .Append(" while ").Append(stage).Append(": ").Append(error.Message.Trim());
        foreach (var frame in frames.Where(f => !IsRuntimeFrame(f)).Take(MaxFrames))
        {
            text.AppendLine().Append("      at ").Append(frame);
        }

        return text.ToString();
    }

    /// <summary>Frames of the runtime's exception machinery and of the logging itself, which say nothing about the cause.</summary>
    private static bool IsRuntimeFrame(string frame) =>
        frame.StartsWith("System.Runtime.ExceptionServices.", StringComparison.Ordinal)
        || frame.StartsWith("System.AppDomain.", StringComparison.Ordinal)
        || frame.StartsWith("System.Diagnostics.", StringComparison.Ordinal)
        || frame.StartsWith("Jellyfesh.App.Shell.Startup.", StringComparison.Ordinal)
        || frame.StartsWith("Jellyfesh.App.Shell.AppLog.", StringComparison.Ordinal);

    public static string Describe(string stage, Exception error)
    {
        var text = new StringBuilder();
        text.Append("While ").Append(stage).Append(':');
        var depth = 0;
        for (Exception? e = error; e is not null && depth < MaxDepth; e = e.InnerException, depth++)
        {
            text.AppendLine();
            if (depth > 0)
            {
                text.Append("Caused by ");
            }

            text.Append(e.GetType().Name).Append(": ").Append(e.Message.Trim());
            if (e.HResult != 0)
            {
                text.Append(" (0x").Append(e.HResult.ToString("X8", CultureInfo.InvariantCulture)).Append(')');
            }
        }

        return text.ToString();
    }
}
