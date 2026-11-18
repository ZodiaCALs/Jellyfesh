using System.Globalization;
using System.Text;
using Jellyfesh.Services;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The app's log, %LOCALAPPDATA%\Jellyfesh\logs\app.log (JF_LOG_DIR moves it; the temporary folder
/// serves when neither can be written, LogLocation): how each start went, stage by stage, and every
/// failure the window cannot show, so a person can send it with a report (the start-up error window
/// shows where it is). Lines carry the time and the process id. Writing never fails the app.
/// </summary>
internal static class AppLog
{
    private const long MaxBytes = 1_000_000;
    private static readonly object Gate = new();
    private static string? _folder;

    [ThreadStatic]
    private static bool _busy;

    /// <summary>True while this thread is writing the log (its own exceptions are not logged again).</summary>
    public static bool Busy => _busy;

    /// <summary>The folder the log is in (chosen on first use).</summary>
    public static string Folder
    {
        get
        {
            var outer = _busy;
            _busy = true;
            try
            {
                lock (Gate)
                {
                    return _folder ??= ChooseFolder();
                }
            }
            finally
            {
                _busy = outer;
            }
        }
    }

    /// <summary>The log file.</summary>
    public static string FilePath => Path.Combine(Folder, LogLocation.FileName);

    /// <summary>Appends a line, and the whole exception (type, message, HRESULT, stack, inner exceptions) when there is one.</summary>
    public static void Write(string what, Exception? e = null)
    {
        var outer = _busy;
        _busy = true;
        try
        {
            var line = new StringBuilder()
                .Append(DateTimeOffset.Now.ToString("O", CultureInfo.InvariantCulture))
                .Append(" [").Append(Environment.ProcessId.ToString(CultureInfo.InvariantCulture)).Append("] ")
                .Append(what);
            if (e is not null)
            {
                line.Append(": ").Append(e).Append(" (HRESULT 0x").Append(e.HResult.ToString("X8", CultureInfo.InvariantCulture)).Append(')');
            }

            line.AppendLine();
            Append(line.ToString());
        }
        catch (Exception)
        {
            // Logging must never take the app down.
        }
        finally
        {
            _busy = outer;
        }
    }

    private static void Append(string text)
    {
        lock (Gate)
        {
            var folder = _folder ??= ChooseFolder();
            var file = Path.Combine(folder, LogLocation.FileName);
            if (File.Exists(file) && new FileInfo(file).Length > MaxBytes)
            {
                File.Move(file, Path.Combine(folder, LogLocation.OldFileName), overwrite: true);
            }

            // Shared, so a second start (or someone reading the log) never finds it locked.
            using var stream = new FileStream(file, FileMode.Append, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete);
            stream.Write(Encoding.UTF8.GetBytes(text));
        }
    }

    /// <summary>The first folder of LogLocation's list that can be written; the last one when none can (writes then fail quietly).</summary>
    private static string ChooseFolder()
    {
        string? local = null;
        string? temp = null;
        try
        {
            local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            temp = Path.GetTempPath();
        }
        catch (Exception)
        {
            // Left out below.
        }

        var candidates = LogLocation.Candidates(Environment.GetEnvironmentVariable(LogLocation.Variable), local, temp);
        foreach (var folder in candidates)
        {
            try
            {
                Directory.CreateDirectory(folder);
                using (new FileStream(Path.Combine(folder, LogLocation.FileName), FileMode.Append, FileAccess.Write, FileShare.ReadWrite | FileShare.Delete))
                {
                }

                return folder;
            }
            catch (Exception)
            {
                // Try the next one.
            }
        }

        return candidates.Count > 0 ? candidates[^1] : AppContext.BaseDirectory;
    }
}
