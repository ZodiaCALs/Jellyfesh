using Jellyfesh.App.Views;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;

namespace Jellyfesh.App.Shell;

/// <summary>The dialogs of Add a Source and of signing in, shared by the Sources page and Settings › Sources.</summary>
internal static class SourceDialogs
{
    /// <summary>
    /// A service in Add a Source: "Connect to …" asks for the Server Address and how to sign in.
    /// <paramref name="connect"/> runs only once that dialog has closed, so the sign-in dialog it
    /// opens can show: WinUI allows one ContentDialog at a time and ShellPlatform refuses a second.
    /// </summary>
    public static async Task ConnectAsync(SourcePresetViewModel preset, Action<SourcePresetViewModel, string?, AuthMethod> connect)
    {
        var dialog = new ConnectSourceViewModel(preset);
        await AppHost.Platform.ShowAsync(new ConnectSourceDialog(dialog));
        if (dialog.Chosen is { } method)
        {
            connect(preset, dialog.Address, method);
        }
    }

    /// <summary>Shows a sign-in flow in its dialog until it ends.</summary>
    public static async void ShowSignIn(SignInViewModel flow)
    {
        await AppHost.Platform.ShowAsync(new SignInDialog(flow));
        flow.Dispose();
    }
}
