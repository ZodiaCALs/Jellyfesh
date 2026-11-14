using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

/// <summary>
/// "Connect to …": a way to sign in is chosen here and the dialog closes; the page reads
/// <see cref="ConnectSourceViewModel.Chosen"/> afterwards and starts the sign-in.
/// </summary>
public sealed partial class ConnectSourceDialog : ContentDialog
{
    public ConnectSourceDialog(ConnectSourceViewModel connect)
    {
        ViewModel = connect;
        InitializeComponent();
        Title = connect.Title;
        PrimaryButtonText = connect.Primary?.Title ?? string.Empty;
        CloseButtonText = AppHost.L.Get("Action_Cancel");
        IsPrimaryButtonEnabled = connect.CanConnect;
        connect.PropertyChanged += OnConnectChanged;
        Closed += (_, _) => connect.PropertyChanged -= OnConnectChanged;
    }

    // Not "Connect": the XAML compiler gives every page a Connect method (IComponentConnector).
    public ConnectSourceViewModel ViewModel { get; }

    private void OnConnectChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(ConnectSourceViewModel.CanConnect))
        {
            IsPrimaryButtonEnabled = ViewModel.CanConnect;
        }
    }

    private void OnPrimaryClick(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        // The dialog closes with the choice recorded; it stays open when nothing could be chosen.
        args.Cancel = ViewModel.Primary is not { } primary || !ViewModel.Choose(primary.Method);
    }

    private void OnMethodClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is SignInMethodChoice choice && ViewModel.Choose(choice.Method))
        {
            Hide();
        }
    }
}
