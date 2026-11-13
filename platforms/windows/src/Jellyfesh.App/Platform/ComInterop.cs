using System.Runtime.InteropServices;
using System.Runtime.InteropServices.Marshalling;

namespace Jellyfesh.App.Platform;

/// <summary>
/// The shell COM interfaces the app uses, as source-generated COM (.NET 8): taskbar thumbnail
/// buttons (ITaskbarList3) and shortcut icons (IShellLinkW, IPersistFile). Methods are listed in
/// vtable order, base interfaces flattened.
/// </summary>
internal static class Com
{
    public static readonly Guid ClsidTaskbarList = new("56FDF344-FD6D-11d0-958A-006097C9A090");
    public static readonly Guid ClsidShellLink = new("00021401-0000-0000-C000-000000000046");

    private static readonly StrategyBasedComWrappers Wrappers = new();

    /// <summary>Creates an in-process COM object and returns it as <typeparamref name="T"/>.</summary>
    public static T Create<T>(Guid clsid)
        where T : class
    {
        var iid = typeof(T).GUID;
        Marshal.ThrowExceptionForHR(Win32.CoCreateInstance(clsid, 0, Win32.CLSCTX_INPROC_SERVER, iid, out var ptr));
        try
        {
            return (T)Wrappers.GetOrCreateObjectForComInstance(ptr, CreateObjectFlags.None);
        }
        finally
        {
            Marshal.Release(ptr);
        }
    }
}

[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
internal unsafe struct ThumbButton
{
    public const uint MaskBitmap = 0x1;
    public const uint MaskIcon = 0x2;
    public const uint MaskTooltip = 0x4;
    public const uint MaskFlags = 0x8;
    public const uint FlagEnabled = 0x0;
    public const uint FlagDisabled = 0x1;

    public uint Mask;
    public uint Id;
    public uint Bitmap;
    public nint Icon;
    public fixed char Tip[260];
    public uint Flags;

    public void SetTip(string text)
    {
        fixed (char* tip = Tip)
        {
            var span = new Span<char>(tip, 260);
            span.Clear();
            text.AsSpan(0, Math.Min(text.Length, 259)).CopyTo(span);
        }
    }
}

[GeneratedComInterface]
[Guid("ea1afb91-9e28-4b86-90e9-9e9f8a5eee6f")]
internal unsafe partial interface ITaskbarList3
{
    // ITaskbarList
    void HrInit();

    void AddTab(nint hwnd);

    void DeleteTab(nint hwnd);

    void ActivateTab(nint hwnd);

    void SetActiveAlt(nint hwnd);

    // ITaskbarList2
    void MarkFullscreenWindow(nint hwnd, int fullscreen);

    // ITaskbarList3
    void SetProgressValue(nint hwnd, ulong completed, ulong total);

    void SetProgressState(nint hwnd, int flags);

    void RegisterTab(nint tab, nint mdi);

    void UnregisterTab(nint tab);

    void SetTabOrder(nint tab, nint insertBefore);

    void SetTabActive(nint tab, nint mdi, uint reserved);

    void ThumbBarAddButtons(nint hwnd, uint count, ThumbButton* buttons);

    void ThumbBarUpdateButtons(nint hwnd, uint count, ThumbButton* buttons);

    void ThumbBarSetImageList(nint hwnd, nint imageList);

    void SetOverlayIcon(nint hwnd, nint icon, char* description);

    void SetThumbnailTooltip(nint hwnd, char* tip);

    void SetThumbnailClip(nint hwnd, nint clip);
}

[GeneratedComInterface]
[Guid("000214F9-0000-0000-C000-000000000046")]
internal unsafe partial interface IShellLinkW
{
    void GetPath(char* file, int capacity, nint findData, uint flags);

    void GetIDList(out nint idList);

    void SetIDList(nint idList);

    void GetDescription(char* name, int capacity);

    void SetDescription(char* name);

    void GetWorkingDirectory(char* dir, int capacity);

    void SetWorkingDirectory(char* dir);

    void GetArguments(char* args, int capacity);

    void SetArguments(char* args);

    void GetHotkey(out ushort hotkey);

    void SetHotkey(ushort hotkey);

    void GetShowCmd(out int showCmd);

    void SetShowCmd(int showCmd);

    void GetIconLocation(char* iconPath, int capacity, out int icon);

    void SetIconLocation(char* iconPath, int icon);

    void SetRelativePath(char* path, uint reserved);

    void Resolve(nint hwnd, uint flags);

    void SetPath(char* file);
}

[GeneratedComInterface]
[Guid("0000010b-0000-0000-C000-000000000046")]
internal unsafe partial interface IPersistFile
{
    // IPersist
    void GetClassID(out Guid classId);

    // IPersistFile
    [PreserveSig]
    int IsDirty();

    void Load(char* fileName, uint mode);

    void Save(char* fileName, int remember);

    void SaveCompleted(char* fileName);

    void GetCurFile(out nint fileName);
}
