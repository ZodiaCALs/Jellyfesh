using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Jellyfesh.App.Views;

/// <summary>The sources' sections of Favourites or Playlists, under the page's own list.</summary>
public sealed partial class SourceSections : UserControl
{
    /// <summary>The song menu of the sources' songs.</summary>
    private readonly FlyoutBase _trackMenu;

    /// <summary>The menu of the sources' albums, artists and playlists (Play, Open in {Service}).</summary>
    private readonly FlyoutBase _entryMenu;

    private SourceLibrarySections? _model;

    public SourceSections()
    {
        InitializeComponent();
        _trackMenu = new TrackMenu(AppHost.Services).Flyout;
        _entryMenu = new EntryMenu(AppHost.Services).Flyout;
    }

    /// <summary>The page's sections; set once, by the page.</summary>
    public SourceLibrarySections? Model
    {
        get => _model;
        set
        {
            if (_model is not null)
            {
                _model.OpenRequested -= OnOpenRequested;
            }

            _model = value;
            if (_model is not null)
            {
                _model.OpenRequested += OnOpenRequested;
            }

            Repeater.ItemsSource = value?.Sections;
        }
    }

    private static void OnOpenRequested(SourceTarget target) => AppHost.Window.Navigate(typeof(SourceBrowsePage), target);

    private void OnTrackClick(object sender, ItemClickEventArgs e) => _model?.PlayTrack(e.ClickedItem as TrackItem);

    private void OnEntryClick(object sender, ItemClickEventArgs e) => _model?.Open(e.ClickedItem as SourceEntryItem);

    /// <summary>A list of songs: thumbnails and the song menu, once per list.</summary>
    private void OnTracksLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is ListView { Tag: null } list)
        {
            list.Tag = this;
            Lists.Attach(list, 40, menu: _trackMenu);
        }
    }

    /// <summary>A list of albums, artists or playlists: thumbnails and their menu, once per list.</summary>
    private void OnEntriesLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is ListView { Tag: null } list)
        {
            list.Tag = this;
            Lists.Attach(list, 40, menu: _entryMenu);
        }
    }
}
