using Jellyfesh.App.Platform;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Shell;

/// <summary>
/// Get Info (docs/ui.md, "Details"): a dialog with About This Song and Audio for a song, or an
/// album's or artist's facts and notes, from DetailsViewModel. It opens at once with what the row
/// knows and fills in when the core's details come.
/// </summary>
internal static class InfoDialog
{
    /// <summary>Get Info for a song row of any list.</summary>
    public static void ShowForTrack(ShellServices s, TrackItem item)
    {
        if (item.Ref is not { } track || ItemRef.Of(track) is not { } reference)
        {
            return;
        }

        var source = track is TrackRef.Source ? ItemLinks.ServiceName(s.Hub.State.Sources, item.SourceId) : null;
        var known = new KnownDetails(Title: item.Title, DurationMs: item.DurationMs, Path: (track as TrackRef.File)?.Path, Source: source);
        _ = ShowAsync(AppHost.Platform, new DetailsViewModel(s, reference, InfoKind.Song, known), s.L);
    }

    /// <summary>Get Info for an album or an artist (their pages' More menus).</summary>
    public static void ShowFor(ShellServices s, ItemRef item, InfoKind kind, string title) =>
        _ = ShowAsync(AppHost.Platform, new DetailsViewModel(s, item, kind, new KnownDetails(Title: title)), s.L);

    public static async Task ShowAsync(ShellPlatform platform, DetailsViewModel details, ILocalizer l)
    {
        var panel = new StackPanel { Spacing = 16, MinWidth = 380, MaxWidth = 520 };
        var dialog = new ContentDialog
        {
            Title = details.Title,
            Content = new ScrollViewer { Content = panel, MaxHeight = 560 },
            CloseButtonText = l.Get("Action_Close"),
            DefaultButton = ContentDialogButton.Close,
        };

        void Render()
        {
            panel.Children.Clear();
            dialog.Title = details.Title;
            if (details.IsLoading)
            {
                panel.Children.Add(new ProgressRing { IsActive = true, Width = 20, Height = 20, HorizontalAlignment = HorizontalAlignment.Left });
            }

            if (details.Description.Length > 0)
            {
                panel.Children.Add(new TextBlock { Text = details.Description, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true, MaxLines = 12, TextTrimming = TextTrimming.WordEllipsis });
            }

            Section(panel, l.Get(details.Kind == InfoKind.Song ? "Info_AboutSong" : "Info_About"), details.About);
            Section(panel, l.Get("Info_Audio"), details.Audio);
            if (details.Attribution.Length > 0)
            {
                panel.Children.Add(Caption(details.Attribution));
            }

            if (details.Error.Length > 0)
            {
                panel.Children.Add(Caption(details.Error));
            }
        }

        details.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(DetailsViewModel.IsLoading))
            {
                Render();
            }
        };
        Render();
        var load = details.LoadAsync();
        await platform.ShowAsync(dialog);
        await load;
    }

    private static void Section(StackPanel panel, string heading, IReadOnlyCollection<InfoRow> rows)
    {
        if (rows.Count == 0)
        {
            return;
        }

        var title = new TextBlock { Text = heading, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold };
        AutomationProperties.SetHeadingLevel(title, Microsoft.UI.Xaml.Automation.Peers.AutomationHeadingLevel.Level2);
        panel.Children.Add(title);
        var grid = new Grid { ColumnSpacing = 16, RowSpacing = 6 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var row = 0;
        foreach (var info in rows)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var label = Caption(info.Label);
            var value = new TextBlock { Text = info.Value, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
            AutomationProperties.SetName(value, $"{info.Label}: {info.Value}");
            Grid.SetRow(label, row);
            Grid.SetRow(value, row);
            Grid.SetColumn(value, 1);
            grid.Children.Add(label);
            grid.Children.Add(value);
            row++;
        }

        panel.Children.Add(grid);
    }

    private static TextBlock Caption(string text)
    {
        var block = new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap };
        if (Application.Current.Resources.TryGetValue("JfInk70", out var ink) && ink is Brush brush)
        {
            block.Foreground = brush;
        }

        return block;
    }
}
