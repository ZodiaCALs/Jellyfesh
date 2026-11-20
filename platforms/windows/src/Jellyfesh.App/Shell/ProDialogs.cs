using Jellyfesh.App.Platform;
using Jellyfesh.Formatting;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The Pro sheet and Enter Licence Key (docs/ui.md, "AI Lyrics, Customize and Pro"), as content
/// dialogs over the window. What they offer and what the core answered come from ProViewModel and
/// LicenceKeyViewModel; these only lay them out.
/// </summary>
internal static class ProDialogs
{
    /// <summary>
    /// "Jellyfesh Pro": what Pro is, Buy and Restore Purchases (store builds), Enter Licence Key…,
    /// Not Now, and the Licence Agreement and Privacy Policy.
    /// </summary>
    public static async Task ShowSheetAsync(ShellPlatform platform, ProViewModel pro, ILocalizer l)
    {
        var enterKey = false;
        var panel = new StackPanel { Spacing = 12, MaxWidth = 440 };
        panel.Children.Add(new TextBlock { Text = l.Get("Pro_Pitch"), TextWrapping = TextWrapping.Wrap });
        var dialog = new ContentDialog
        {
            Title = l.Get("Pro_Title"),
            Content = panel,
            CloseButtonText = l.Get("Pro_NotNow"),
            DefaultButton = ContentDialogButton.Close,
        };

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        if (pro.CanBuy)
        {
            actions.Children.Add(Button(pro.BuyLabel, accent: true, async () => await pro.BuyCommand.ExecuteAsync(null)));
            actions.Children.Add(Button(l.Get("Pro_RestoreLabel"), accent: false, async () => await pro.RestoreCommand.ExecuteAsync(null)));
        }

        if (pro.CanEnterKey)
        {
            // One dialog at a time: the sheet closes, then Enter Licence Key opens.
            actions.Children.Add(Button(l.Get("Pro_EnterKeyLabel"), accent: !pro.CanBuy, () =>
            {
                enterKey = true;
                dialog.Hide();
                return Task.CompletedTask;
            }));
        }

        if (actions.Children.Count > 0)
        {
            panel.Children.Add(actions);
        }

        if (pro.Links is { } links)
        {
            var legal = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
            legal.Children.Add(Link(l.Get("Legal_LicenceAgreementLabel"), () => pro.OpenLicenceAgreementCommand.Execute(null)));
            legal.Children.Add(Link(l.Get("Legal_PrivacyPolicyLabel"), () => pro.OpenPrivacyPolicyCommand.Execute(null)));
            panel.Children.Add(legal);
        }

        // Pro bought or restored meanwhile: the sheet has done its work.
        void OnPro(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
        {
            if (pro.IsActive)
            {
                dialog.Hide();
            }
        }

        pro.PropertyChanged += OnPro;
        try
        {
            await platform.ShowAsync(dialog);
        }
        finally
        {
            pro.PropertyChanged -= OnPro;
        }

        if (enterKey)
        {
            pro.EnterLicenceKeyCommand.Execute(null);
        }
    }

    /// <summary>
    /// Enter Licence Key: the key ("JFP1-…"), Cancel and Activate. The dialog stays open until the
    /// core accepted the key, and says why it did not in the app's words.
    /// </summary>
    public static async Task ShowLicenceKeyAsync(ShellPlatform platform, LicenceKeyViewModel key, ILocalizer l)
    {
        var box = new TextBox { PlaceholderText = l.Get("Licence_Placeholder"), MinWidth = 320 };
        AutomationProperties.SetName(box, l.Get("Licence_Title"));
        var error = new TextBlock { TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
        if (Application.Current.Resources.TryGetValue("JfDanger", out var danger) && danger is Brush brush)
        {
            error.Foreground = brush;
        }

        var working = new ProgressRing { Width = 20, Height = 20, IsActive = false, HorizontalAlignment = HorizontalAlignment.Left };
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(box);
        panel.Children.Add(error);
        panel.Children.Add(working);
        var dialog = new ContentDialog
        {
            Title = l.Get("Licence_Title"),
            Content = panel,
            PrimaryButtonText = l.Get("Licence_Activate"),
            CloseButtonText = l.Get("Action_Cancel"),
            DefaultButton = ContentDialogButton.Primary,
            IsPrimaryButtonEnabled = false,
        };

        void Refresh()
        {
            dialog.IsPrimaryButtonEnabled = key.CanActivate;
            working.IsActive = key.IsWorking;
            error.Text = key.Error ?? string.Empty;
            error.Visibility = string.IsNullOrEmpty(key.Error) ? Visibility.Collapsed : Visibility.Visible;
        }

        box.TextChanged += (_, _) =>
        {
            key.Key = box.Text;
            Refresh();
        };
        dialog.PrimaryButtonClick += (_, e) =>
        {
            e.Cancel = true; // open until the core answers
            key.ActivateCommand.Execute(null);
            Refresh();
        };
        key.PropertyChanged += (_, _) => Refresh();
        key.Activated += dialog.Hide;
        try
        {
            await platform.ShowAsync(dialog);
        }
        finally
        {
            key.Close();
        }
    }

    private static Button Button(string text, bool accent, Func<Task> click)
    {
        var button = new Button { Content = text };
        if (Application.Current.Resources.TryGetValue(accent ? "JfAccentPillStyle" : "JfChipButtonStyle", out var style) && style is Style s)
        {
            button.Style = s;
        }

        button.Click += async (_, _) =>
        {
            try
            {
                await click();
            }
            catch (Exception e)
            {
                // async void: an exception would end the app.
                AppLog.Write($"{text} failed", e);
            }
        };
        return button;
    }

    private static HyperlinkButton Link(string text, Action open)
    {
        var link = new HyperlinkButton { Content = text };
        link.Click += (_, _) => open();
        return link;
    }
}
