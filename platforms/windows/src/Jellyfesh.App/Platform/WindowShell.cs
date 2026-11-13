using System.ComponentModel;
using System.Runtime.InteropServices;
using Jellyfesh.Appearance;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.ViewModels;
using Microsoft.Win32;

namespace Jellyfesh.App.Platform;

/// <summary>
/// The window's Win32 side: the subclassed window procedure (taskbar button clicks arrive as
/// WM_COMMAND, ADR-0007), the taskbar thumbnail buttons, the window and taskbar icon (WM_SETICON,
/// ADR-0017) and the icons of the shortcuts the installer made.
/// </summary>
internal sealed unsafe class WindowShell : IDisposable
{
    private const uint IdPrevious = 1;
    private const uint IdPlayPause = 2;
    private const uint IdNext = 3;

    private static readonly Dictionary<nint, WindowShell> Hooked = [];

    private readonly nint _hwnd;
    private readonly PlayerViewModel _player;
    private readonly ILocalizer _l;
    private readonly uint _taskbarCreatedMessage;
    private readonly string _assets;
    private ITaskbarList3? _taskbar;
    private bool _buttonsAdded;
    private nint _iconSmall;
    private nint _iconBig;
    private string? _iconId;
    private readonly Dictionary<string, nint> _glyphs = [];

    public WindowShell(nint hwnd, PlayerViewModel player, ILocalizer localizer)
    {
        _hwnd = hwnd;
        _player = player;
        _l = localizer;
        _assets = Path.Combine(AppContext.BaseDirectory, "Assets");
        _taskbarCreatedMessage = Win32.RegisterWindowMessage("TaskbarButtonCreated");
        Hooked[hwnd] = this;
        Win32.SetWindowSubclass(hwnd, &SubclassProc, 1, 0);
        _player.PropertyChanged += OnPlayerChanged;
    }

    /// <summary>Sets the window and taskbar icon (WM_SETICON) and, once per change, the shortcuts' icons.</summary>
    public void SetIcon(string iconId) => SetIcon(iconId, reload: false);

    private void SetIcon(string iconId, bool reload)
    {
        if (iconId == _iconId && !reload)
        {
            return;
        }

        var ico = IconFile(iconId);
        if (ico is null)
        {
            return;
        }

        var (small, big) = Win32.IconSizes(_hwnd);
        var newSmall = Win32.LoadImage(0, ico, Win32.IMAGE_ICON, small, small, Win32.LR_LOADFROMFILE);
        var newBig = Win32.LoadImage(0, ico, Win32.IMAGE_ICON, big, big, Win32.LR_LOADFROMFILE);
        if (newSmall == 0 || newBig == 0)
        {
            return;
        }

        Win32.SendMessage(_hwnd, Win32.WM_SETICON, Win32.ICON_SMALL, newSmall);
        Win32.SendMessage(_hwnd, Win32.WM_SETICON, Win32.ICON_BIG, newBig);
        if (_iconSmall != 0)
        {
            Win32.DestroyIcon(_iconSmall);
            Win32.DestroyIcon(_iconBig);
        }

        _iconSmall = newSmall;
        _iconBig = newBig;
        var changed = _iconId != iconId;
        var first = _iconId is null;
        _iconId = iconId;
        if (changed && (!first || iconId != AppIcons.DefaultLight))
        {
            _ = Task.Run(() => UpdateShortcuts(ico));
        }
    }

    public void Dispose()
    {
        _player.PropertyChanged -= OnPlayerChanged;
        Win32.RemoveWindowSubclass(_hwnd, &SubclassProc, 1);
        Hooked.Remove(_hwnd);
        foreach (var icon in _glyphs.Values)
        {
            Win32.DestroyIcon(icon);
        }

        if (_iconSmall != 0)
        {
            Win32.DestroyIcon(_iconSmall);
            Win32.DestroyIcon(_iconBig);
        }
    }

    [UnmanagedCallersOnly]
    private static nint SubclassProc(nint hwnd, uint msg, nint wParam, nint lParam, nuint id, nuint refData)
    {
        try
        {
            if (Hooked.TryGetValue(hwnd, out var shell))
            {
                if (msg == shell._taskbarCreatedMessage)
                {
                    shell.AddButtons();
                }
                else if (msg == Win32.WM_DPICHANGED && shell._iconId is { } icon)
                {
                    // Icons are loaded at the size of the window's DPI; another monitor needs them again.
                    shell.SetIcon(icon, reload: true);
                    shell.ReloadGlyphs();
                }
                else if (msg == Win32.WM_COMMAND && Win32.HiWord(wParam) == Win32.THBN_CLICKED)
                {
                    shell.OnButton((uint)Win32.LoWord(wParam));
                    return 0;
                }
            }
        }
        catch (Exception)
        {
            // Never let an exception cross into the window procedure.
        }

        return Win32.DefSubclassProc(hwnd, msg, wParam, lParam);
    }

    private void ReloadGlyphs()
    {
        foreach (var icon in _glyphs.Values)
        {
            Win32.DestroyIcon(icon);
        }

        _glyphs.Clear();
        if (_buttonsAdded)
        {
            UpdateButtons(add: false);
        }
    }

    private void OnButton(uint id)
    {
        switch (id)
        {
            case IdPrevious:
                _player.PreviousCommand.Execute(null);
                break;
            case IdPlayPause:
                _player.PlayPauseCommand.Execute(null);
                break;
            case IdNext:
                _player.NextCommand.Execute(null);
                break;
        }
    }

    private void OnPlayerChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (_buttonsAdded && e.PropertyName is nameof(PlayerViewModel.IsPlaying) or nameof(PlayerViewModel.HasTrack)
                or nameof(PlayerViewModel.CanGoNext) or nameof(PlayerViewModel.CanGoPrevious))
        {
            UpdateButtons(add: false);
        }
    }

    private void AddButtons()
    {
        try
        {
            _taskbar ??= Com.Create<ITaskbarList3>(Com.ClsidTaskbarList);
            _taskbar.HrInit();
            UpdateButtons(add: true);
            _buttonsAdded = true;
        }
        catch (COMException)
        {
            _taskbar = null; // no taskbar (Explorer restarting): try again on its next TaskbarButtonCreated
        }
    }

    private void UpdateButtons(bool add)
    {
        if (_taskbar is null)
        {
            return;
        }

        var buttons = stackalloc ThumbButton[3];
        Fill(ref buttons[0], IdPrevious, "previous", _l.Get("Player_Previous"), _player.HasTrack && _player.CanGoPrevious);
        Fill(ref buttons[1], IdPlayPause, _player.IsPlaying ? "pause" : "play", _player.PlayPauseLabel, _player.HasTrack);
        Fill(ref buttons[2], IdNext, "next", _l.Get("Player_Next"), _player.HasTrack && _player.CanGoNext);
        if (add)
        {
            _taskbar.ThumbBarAddButtons(_hwnd, 3, buttons);
        }
        else
        {
            _taskbar.ThumbBarUpdateButtons(_hwnd, 3, buttons);
        }
    }

    private void Fill(ref ThumbButton button, uint id, string glyph, string tip, bool enabled)
    {
        button.Mask = ThumbButton.MaskIcon | ThumbButton.MaskTooltip | ThumbButton.MaskFlags;
        button.Id = id;
        button.Icon = Glyph(glyph);
        button.SetTip(tip);
        button.Flags = enabled ? ThumbButton.FlagEnabled : ThumbButton.FlagDisabled;
    }

    /// <summary>The glyph icon for the taskbar's theme: light glyphs on a dark taskbar.</summary>
    private nint Glyph(string name)
    {
        var theme = SystemUsesLightTheme() ? "dark" : "light";
        var key = $"{name}-{theme}";
        if (_glyphs.TryGetValue(key, out var icon))
        {
            return icon;
        }

        var (small, _) = Win32.IconSizes(_hwnd);
        icon = Win32.LoadImage(0, Path.Combine(_assets, "Taskbar", key + ".ico"), Win32.IMAGE_ICON, small, small, Win32.LR_LOADFROMFILE);
        _glyphs[key] = icon;
        return icon;
    }

    private static bool SystemUsesLightTheme()
    {
        using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        return key?.GetValue("SystemUsesLightTheme") is int value && value != 0;
    }

    /// <summary>
    /// The .ico for an icon: Ice and Steel ship as .ico; the others are packed from their shipped
    /// PNGs on first selection (ADR-0017) into the local app data folder.
    /// </summary>
    private string? IconFile(string id)
    {
        var shipped = Path.Combine(_assets, "Icons", id + ".ico");
        if (File.Exists(shipped))
        {
            return shipped;
        }

        var folder = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Jellyfesh", "Icons");
        var built = Path.Combine(folder, id + ".ico");
        if (File.Exists(built))
        {
            return built;
        }

        try
        {
            var images = AppIcons.PngSizes
                .Select(size => (size, Path.Combine(_assets, "Icons", id, $"{size}.png")))
                .Where(p => File.Exists(p.Item2))
                .Select(p => (p.size, File.ReadAllBytes(p.Item2)))
                .ToList();
            if (images.Count == 0)
            {
                return null;
            }

            Directory.CreateDirectory(folder);
            File.WriteAllBytes(built, IcoFile.Write(images));
            return built;
        }
        catch (IOException)
        {
            return null;
        }
    }

    /// <summary>
    /// Points the Start menu and desktop shortcuts that start this executable at the new icon.
    /// A pinned taskbar shortcut may keep the old icon until Explorer refreshes (ADR-0017).
    /// </summary>
    private static void UpdateShortcuts(string ico)
    {
        var exe = Environment.ProcessPath;
        if (exe is null)
        {
            return;
        }

        var folders = new[]
        {
            Environment.GetFolderPath(Environment.SpecialFolder.Programs),
            Environment.GetFolderPath(Environment.SpecialFolder.CommonPrograms),
            Environment.GetFolderPath(Environment.SpecialFolder.DesktopDirectory),
            Environment.GetFolderPath(Environment.SpecialFolder.CommonDesktopDirectory),
        };
        var changed = false;
        foreach (var folder in folders.Where(Directory.Exists))
        {
            IEnumerable<string> links;
            try
            {
                links = Directory.EnumerateFiles(folder, "Jellyfesh*.lnk", SearchOption.AllDirectories).ToList();
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                continue;
            }

            foreach (var link in links)
            {
                changed |= Retarget(link, exe, ico);
            }
        }

        if (changed)
        {
            Win32.SHChangeNotify(Win32.SHCNE_ASSOCCHANGED, Win32.SHCNF_IDLIST, 0, 0);
        }
    }

    private static bool Retarget(string link, string exe, string ico)
    {
        try
        {
            var shellLink = Com.Create<IShellLinkW>(Com.ClsidShellLink);
            var file = (IPersistFile)shellLink;
            fixed (char* path = link)
            {
                file.Load(path, 2 /* STGM_READWRITE */);
            }

            var target = stackalloc char[1024];
            shellLink.GetPath(target, 1024, 0, 0);
            if (!string.Equals(new string(target), exe, StringComparison.OrdinalIgnoreCase))
            {
                return false; // not ours
            }

            var current = stackalloc char[1024];
            shellLink.GetIconLocation(current, 1024, out var index);
            if (index == 0 && string.Equals(new string(current), ico, StringComparison.OrdinalIgnoreCase))
            {
                return false; // already this icon
            }

            fixed (char* icon = ico)
            {
                shellLink.SetIconLocation(icon, 0);
            }

            fixed (char* path = link)
            {
                file.Save(path, 1);
            }

            return true;
        }
        catch (Exception e) when (e is COMException or UnauthorizedAccessException or InvalidCastException)
        {
            return false; // a read-only or foreign shortcut
        }
    }
}
