using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>One of the shared keys in Settings › About › Keyboard Shortcuts: its keys and what they do.</summary>
public sealed record KeyboardShortcut(string Keys, string Action);

/// <summary>
/// Keyboard Shortcuts (docs/ui.md, "Keyboard" and Settings › About): the keys every desktop app and
/// the web share, with Windows' modifier, in the order ui.md lists them. The window handles them
/// (MainWindow's accelerators); this is what Settings shows.
/// </summary>
public static class KeyboardShortcuts
{
    /// <summary>The keys and the string key of what they do.</summary>
    public static IReadOnlyList<(string Keys, string Action)> Shared { get; } =
    [
        ("Space", "Shortcut_PlayPause"),
        ("Ctrl + →", "Shortcut_NextSong"),
        ("Ctrl + ←", "Shortcut_PreviousSong"),
        ("Shift + →", "Shortcut_SkipForward"),
        ("Shift + ←", "Shortcut_SkipBack"),
        ("Ctrl + ↑", "Shortcut_VolumeUp"),
        ("Ctrl + ↓", "Shortcut_VolumeDown"),
        ("Ctrl + F", "Shortcut_Search"),
        ("Ctrl + M", "Shortcut_MiniPlayer"),
        ("Ctrl + ,", "Shortcut_Settings"),
        ("Esc", "Shortcut_Back"),
    ];

    /// <summary>The shared keys in the app's language.</summary>
    public static IReadOnlyList<KeyboardShortcut> All(ILocalizer l) =>
        [.. Shared.Select(s => new KeyboardShortcut(s.Keys, l.Get(s.Action)))];
}
