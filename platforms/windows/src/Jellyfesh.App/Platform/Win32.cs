using System.Runtime.InteropServices;
using System.Runtime.InteropServices.Marshalling;

namespace Jellyfesh.App.Platform;

/// <summary>The few Win32 calls the shell needs (source-generated, NativeAOT-friendly).</summary>
internal static unsafe partial class Win32
{
    public const uint WM_SETICON = 0x0080;
    public const uint WM_COMMAND = 0x0111;
    public const uint WM_DESTROY = 0x0002;
    public const uint WM_DPICHANGED = 0x02E0;
    public const nint ICON_SMALL = 0;
    public const nint ICON_BIG = 1;
    public const uint IMAGE_ICON = 1;
    public const uint LR_LOADFROMFILE = 0x10;
    public const int SM_CXICON = 11;
    public const int SM_CXSMICON = 49;
    public const int THBN_CLICKED = 0x1800;
    public const int SHCNE_ASSOCCHANGED = 0x08000000;
    public const uint SHCNF_IDLIST = 0x0000;
    public const uint CLSCTX_INPROC_SERVER = 1;

    [LibraryImport("user32.dll", EntryPoint = "SendMessageW")]
    public static partial nint SendMessage(nint hWnd, uint msg, nint wParam, nint lParam);

    [LibraryImport("user32.dll", EntryPoint = "LoadImageW", StringMarshalling = StringMarshalling.Utf16, SetLastError = true)]
    public static partial nint LoadImage(nint hInstance, string name, uint type, int cx, int cy, uint flags);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool DestroyIcon(nint hIcon);

    [LibraryImport("user32.dll")]
    public static partial uint GetDpiForWindow(nint hWnd);

    [LibraryImport("user32.dll")]
    public static partial int GetSystemMetricsForDpi(int index, uint dpi);

    [LibraryImport("user32.dll", EntryPoint = "RegisterWindowMessageW", StringMarshalling = StringMarshalling.Utf16)]
    public static partial uint RegisterWindowMessage(string name);

    [LibraryImport("comctl32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetWindowSubclass(nint hWnd, delegate* unmanaged<nint, uint, nint, nint, nuint, nuint, nint> proc, nuint id, nuint refData);

    [LibraryImport("comctl32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool RemoveWindowSubclass(nint hWnd, delegate* unmanaged<nint, uint, nint, nint, nuint, nuint, nint> proc, nuint id);

    [LibraryImport("comctl32.dll")]
    public static partial nint DefSubclassProc(nint hWnd, uint msg, nint wParam, nint lParam);

    [LibraryImport("shell32.dll")]
    public static partial void SHChangeNotify(int eventId, uint flags, nint item1, nint item2);

    [LibraryImport("ole32.dll")]
    public static partial int CoCreateInstance(in Guid clsid, nint outer, uint context, in Guid iid, out nint instance);

    public const int SW_RESTORE = 9;

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool AllowSetForegroundWindow(uint processId);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetForegroundWindow(nint hWnd);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool ShowWindow(nint hWnd, int command);

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool IsIconic(nint hWnd);

    [LibraryImport("kernel32.dll", EntryPoint = "CreateEventW", StringMarshalling = StringMarshalling.Utf16)]
    public static partial nint CreateEvent(nint attributes, [MarshalAs(UnmanagedType.Bool)] bool manualReset, [MarshalAs(UnmanagedType.Bool)] bool initialState, string? name);

    [LibraryImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool SetEvent(nint handle);

    [LibraryImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool CloseHandle(nint handle);

    [LibraryImport("ole32.dll")]
    public static partial int CoWaitForMultipleObjects(uint flags, uint timeout, uint count, nint* handles, out uint index);

    /// <summary>
    /// Waits up to <paramref name="timeout"/> for <paramref name="handle"/> while still dispatching
    /// COM calls (safe on an STA thread); true when it was signalled (S_OK), false when the time ran
    /// out (RPC_S_CALLPENDING) or the wait failed.
    /// </summary>
    public static bool WaitPumpingCom(nint handle, TimeSpan timeout)
    {
        var handles = stackalloc nint[1];
        handles[0] = handle;
        var ms = (uint)Math.Clamp(timeout.TotalMilliseconds, 0, uint.MaxValue - 1);
        return CoWaitForMultipleObjects(0, ms, 1, handles, out _) == 0;
    }

    public const uint MB_OK = 0x00000000;
    public const uint MB_ICONERROR = 0x00000010;
    public const uint MB_SETFOREGROUND = 0x00010000;

    /// <summary>A message box with no owner window: what start-up shows when even XAML could not start.</summary>
    [LibraryImport("user32.dll", EntryPoint = "MessageBoxW", StringMarshalling = StringMarshalling.Utf16)]
    public static partial int MessageBox(nint hWnd, string text, string caption, uint type);

    [LibraryImport("kernel32.dll")]
    public static partial nint GetCurrentProcess();

    /// <summary>Ends the process at once with <paramref name="exitCode"/> (the last resort of a watchdog; no DLL is notified).</summary>
    [LibraryImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static partial bool TerminateProcess(nint process, uint exitCode);

    /// <summary>
    /// Loads Microsoft.WindowsAppRuntime.dll, whose loading sets up the Windows App SDK's
    /// registration-free WinRT for a self-contained app (the function itself only returns S_OK).
    /// </summary>
    [LibraryImport("Microsoft.WindowsAppRuntime.dll", EntryPoint = "WindowsAppRuntime_EnsureIsLoaded")]
    public static partial int WindowsAppRuntimeEnsureIsLoaded();

    public static int LoWord(nint value) => (int)(value & 0xFFFF);

    public static int HiWord(nint value) => (int)((value >> 16) & 0xFFFF);

    /// <summary>The icon sizes Windows asks for at this window's DPI: (small, big).</summary>
    public static (int Small, int Big) IconSizes(nint hwnd)
    {
        var dpi = GetDpiForWindow(hwnd);
        if (dpi == 0)
        {
            dpi = 96;
        }

        return (GetSystemMetricsForDpi(SM_CXSMICON, dpi), GetSystemMetricsForDpi(SM_CXICON, dpi));
    }
}
