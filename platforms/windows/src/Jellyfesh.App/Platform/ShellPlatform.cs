using System.Diagnostics;
using Jellyfesh.Formatting;
using Jellyfesh.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Windows.Storage.Pickers;
using Windows.ApplicationModel.DataTransfer;

namespace Jellyfesh.App.Platform;

/// <summary>
/// Pickers (the Windows App SDK pickers, which take the window id and work unpackaged), dialogs,
/// the browser and the clipboard for the view models.
/// </summary>
internal sealed class ShellPlatform(Window window, ILocalizer l) : IShellPlatform
{
    private bool _dialogOpen;

    public async Task<string?> PickFolderAsync()
    {
        var picker = new FolderPicker(window.AppWindow.Id) { SuggestedStartLocation = PickerLocationId.MusicLibrary };
        var result = await picker.PickSingleFolderAsync();
        return result?.Path;
    }

    public async Task<string?> PickFileAsync(IReadOnlyList<string> extensions)
    {
        var picker = new FileOpenPicker(window.AppWindow.Id) { SuggestedStartLocation = PickerLocationId.Downloads };
        foreach (var ext in extensions)
        {
            picker.FileTypeFilter.Add(ext);
        }

        var result = await picker.PickSingleFileAsync();
        return result?.Path;
    }

    public async Task<string?> PickSaveFileAsync(string suggestedName, string extension)
    {
        var picker = new FileSavePicker(window.AppWindow.Id)
        {
            SuggestedFileName = suggestedName,
            DefaultFileExtension = extension,
            SuggestedStartLocation = PickerLocationId.MusicLibrary,
        };
        picker.FileTypeChoices.Add(l.Get("Picker_Playlist"), new List<string> { extension });
        var result = await picker.PickSaveFileAsync();
        return result?.Path;
    }

    /// <summary>Opens web addresses only: sign-in steps come from plugins, so other schemes are refused.</summary>
    public void OpenUrl(string url)
    {
        if (Uri.TryCreate(url, UriKind.Absolute, out var uri) && (uri.Scheme == Uri.UriSchemeHttps || uri.Scheme == Uri.UriSchemeHttp))
        {
            Process.Start(new ProcessStartInfo(uri.AbsoluteUri) { UseShellExecute = true });
        }
    }

    public void CopyText(string text)
    {
        var package = new DataPackage();
        package.SetText(text);
        Clipboard.SetContent(package);
    }

    public async Task<bool> ConfirmAsync(string title, string body, string primary, string secondary)
    {
        var dialog = new ContentDialog
        {
            Title = title,
            Content = new TextBlock { Text = body, TextWrapping = TextWrapping.Wrap },
            PrimaryButtonText = primary,
            CloseButtonText = secondary,
            DefaultButton = ContentDialogButton.Close,
        };
        return await ShowAsync(dialog) == ContentDialogResult.Primary;
    }

    public async Task<string?> PromptTextAsync(string title, string initial, string primary)
    {
        var box = new TextBox { Text = initial, SelectionStart = 0, SelectionLength = initial.Length };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, title);
        var dialog = new ContentDialog
        {
            Title = title,
            Content = box,
            PrimaryButtonText = primary,
            CloseButtonText = l.Get("Action_Cancel"),
            DefaultButton = ContentDialogButton.Primary,
        };
        return await ShowAsync(dialog) == ContentDialogResult.Primary ? box.Text : null;
    }

    /// <summary>Shows a dialog over the window; only one ContentDialog may be open at a time.</summary>
    public async Task<ContentDialogResult> ShowAsync(ContentDialog dialog)
    {
        if (_dialogOpen || window.Content?.XamlRoot is null)
        {
            return ContentDialogResult.None;
        }

        dialog.XamlRoot = window.Content.XamlRoot;
        if (Application.Current.Resources.TryGetValue("DefaultContentDialogStyle", out var style) && style is Style s)
        {
            dialog.Style = s;
        }

        _dialogOpen = true;
        try
        {
            return await dialog.ShowAsync();
        }
        finally
        {
            _dialogOpen = false;
        }
    }
}
