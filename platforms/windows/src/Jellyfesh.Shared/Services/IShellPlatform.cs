namespace Jellyfesh.Services;

/// <summary>Platform actions view models ask for; the app implements them with WinUI and Win32.</summary>
public interface IShellPlatform
{
    /// <summary>A folder the person picked, or null.</summary>
    Task<string?> PickFolderAsync();

    /// <summary>A file to open with one of <paramref name="extensions"/> (".m3u8"), or null.</summary>
    Task<string?> PickFileAsync(IReadOnlyList<string> extensions);

    /// <summary>A file to save, or null.</summary>
    Task<string?> PickSaveFileAsync(string suggestedName, string extension);

    /// <summary>Opens a web address in the default browser (sign-in flows, links).</summary>
    void OpenUrl(string url);

    void CopyText(string text);

    /// <summary>Asks a yes/no question with a localised title and body; true for the primary answer.</summary>
    Task<bool> ConfirmAsync(string title, string body, string primary, string secondary);

    /// <summary>Asks for one line of text (a playlist name), or null when cancelled.</summary>
    Task<string?> PromptTextAsync(string title, string initial, string primary);
}
