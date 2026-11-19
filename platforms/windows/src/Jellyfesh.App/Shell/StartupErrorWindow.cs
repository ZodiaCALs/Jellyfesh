using System.Diagnostics;
using Jellyfesh.App.Platform;
using Jellyfesh.Formatting;
using Jellyfesh.Services;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// What a failed start shows (Startup.Fail): why Jellyfesh could not start, the details, and the
/// log file to send with a report, with Open Log Folder. A plain window built in code, so it needs
/// none of the app's styles; a message box when even that cannot open. The words fall back to
/// English when the string tables cannot be read (no Jellyfesh.pri).
/// </summary>
internal static class StartupErrorWindow
{
    private static readonly Dictionary<string, string> English = new(StringComparer.Ordinal)
    {
        ["Startup_FailedTitle"] = "Jellyfesh Could Not Start",
        ["Startup_FailedBody"] = "Something went wrong while Jellyfesh was starting. The details are below; please send the log file with your report.",
        ["Startup_LogFile"] = "Log File",
        ["Startup_OpenLogFolder"] = "Open Log Folder",
        ["Action_Close"] = "Close",
        ["Startup_NotAnswering"] = "Jellyfesh is already running but isn't answering. End Jellyfesh in Task Manager, then start it again.",
    };

    /// <summary>Shows the window; Jellyfesh exits when it closes. False when the window could not be made.</summary>
    public static bool TryShow(string details)
    {
        try
        {
            var l = Strings();
            var window = new Window { Title = "Jellyfesh" };
            var panel = new StackPanel { Spacing = 12, Margin = new Thickness(32), MaxWidth = 640, HorizontalAlignment = HorizontalAlignment.Left };
            panel.Children.Add(new TextBlock { Text = Text(l, "Startup_FailedTitle"), FontSize = 20, FontWeight = FontWeights.Bold, TextWrapping = TextWrapping.Wrap });
            panel.Children.Add(new TextBlock { Text = Text(l, "Startup_FailedBody"), TextWrapping = TextWrapping.Wrap });
            panel.Children.Add(new TextBox { Text = details, IsReadOnly = true, AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MaxHeight = 240 });
            panel.Children.Add(new TextBlock { Text = Text(l, "Startup_LogFile"), FontWeight = FontWeights.SemiBold });
            panel.Children.Add(new TextBox { Text = AppLog.FilePath, IsReadOnly = true });

            var open = new Button { Content = Text(l, "Startup_OpenLogFolder") };
            open.Click += (_, _) => OpenLogFolder();
            var close = new Button { Content = Text(l, "Action_Close") };
            close.Click += (_, _) => window.Close();
            var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            buttons.Children.Add(open);
            buttons.Children.Add(close);
            panel.Children.Add(buttons);

            window.Content = new ScrollViewer { Content = panel };
            window.Closed += (_, _) => Startup.Exit(ExitCodes.StartupFailed);
            window.Activate();
            return true;
        }
        catch (Exception e)
        {
            AppLog.Write("the start-up error window could not open", e);
            return false;
        }
    }

    /// <summary>The same in a message box, which needs nothing but Windows (for failures before or inside XAML's start).</summary>
    public static void ShowMessageBox(string details)
    {
        var l = Strings();
        var text = $"{Text(l, "Startup_FailedBody")}\n\n{details}\n\n{Text(l, "Startup_LogFile")}: {AppLog.FilePath}";
        Show(Text(l, "Startup_FailedTitle"), text);
    }

    /// <summary>A second start found a Jellyfesh that does not take it (hung or shutting down).</summary>
    public static void ShowNotAnswering() => Show("Jellyfesh", Text(Strings(), "Startup_NotAnswering"));

    private static void Show(string title, string text)
    {
        try
        {
            _ = Win32.MessageBox(0, text, title, Win32.MB_OK | Win32.MB_ICONERROR | Win32.MB_SETFOREGROUND);
        }
        catch (Exception e)
        {
            AppLog.Write("the message box could not open", e);
        }
    }

    private static ILocalizer? Strings()
    {
        try
        {
            return new ResourceLocalizer();
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// <summary>The string from the tables, or English when they cannot be read (the localizer then returns the key).</summary>
    private static string Text(ILocalizer? l, string key)
    {
        var value = l?.Get(key);
        return string.IsNullOrEmpty(value) || value == key ? English[key] : value;
    }

    private static void OpenLogFolder()
    {
        try
        {
            var file = AppLog.FilePath;
            var arguments = File.Exists(file) ? $"/select,\"{file}\"" : $"\"{AppLog.Folder}\"";
            Process.Start(new ProcessStartInfo("explorer.exe", arguments) { UseShellExecute = false });
        }
        catch (Exception e)
        {
            AppLog.Write("the log folder could not open", e);
        }
    }
}
