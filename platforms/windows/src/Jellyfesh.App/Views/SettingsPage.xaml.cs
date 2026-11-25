using System.ComponentModel;
using System.Diagnostics;
using System.Reflection;
using Jellyfesh.App.Controls;
using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Appearance;
using Jellyfesh.Formatting;
using Jellyfesh.Interop;
using Jellyfesh.Services;
using Jellyfesh.Settings;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Navigation;

namespace Jellyfesh.App.Views;

/// <summary>A section of Settings, for links that open one directly (Sources Settings…).</summary>
public enum SettingsSection
{
    Appearance,
    Playback,
    Library,
    Downloads,
    Sources,
    Extensions,
    About,
}

public sealed partial class SettingsPage : JfPage
{
    /// <summary>The sections in every app's order (docs/ui.md, "Settings") and their names' string keys.</summary>
    private static readonly (SettingsSection Section, string Label)[] SectionLabels =
    [
        (SettingsSection.Appearance, "Section_Appearance"),
        (SettingsSection.Playback, "Section_Playback"),
        (SettingsSection.Library, "Section_Library"),
        (SettingsSection.Downloads, "Section_Downloads"),
        (SettingsSection.Sources, "Section_Sources"),
        (SettingsSection.Extensions, "Section_Extensions"),
        (SettingsSection.About, "Section_About"),
    ];

    private readonly Dictionary<SettingsSection, SidebarItem> _sections = [];
    private bool _syncing;

    public SettingsPage()
    {
        ViewModel = AppHost.App.CreateSettings(AppAssets.IconPreview);
        PageModel = ViewModel;
        InitializeComponent();
        ViewModel.PropertyChanged += OnViewModelChanged;
        ViewModel.Player.PropertyChanged += OnPlayerChanged;
        ViewModel.SignInRequested += OnSignInRequested;
        ViewModel.ExtensionsRequested += () => ShowSection(SettingsSection.Extensions); // Customize…
        BuildSections();
        ShowSection(SettingsSection.Appearance);
        Loaded += (_, _) =>
        {
            AppHost.App.ThemeChanged += OnThemeChanged;
            BuildSwatches();
            OnThemeChanged();
            SyncChoices();
        };
        Unloaded += (_, _) => AppHost.App.ThemeChanged -= OnThemeChanged;
        LicencesButton.Visibility = Bind.Visible(File.Exists(NoticesPath) || Links is not null);
        WebsiteButton.Visibility = Bind.Visible(Links is not null);
        VersionText.Text = Version();
        AboutIcon.Source = new BitmapImage(AppAssets.IconPng(AppHost.App.AppIcon, 64));
    }

    public SettingsViewModel ViewModel { get; }

    /// <summary>The third-party notices the build puts next to the app (scripts/build.ps1).</summary>
    private static string NoticesPath => Path.Combine(AppContext.BaseDirectory, "THIRD_PARTY_NOTICES.md");

    /// <summary>The website and its licences page, when the build knows the website (JfWebsite).</summary>
    private static AboutInfo.Links? Links => AppHost.Website;

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        if (e.Parameter is SettingsSection section)
        {
            ShowSection(section);
        }

        base.OnNavigatedTo(e);
    }

    private static string Version()
    {
        var app = Assembly.GetExecutingAssembly().GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
        string? core;
        try
        {
            core = FfiCoreClient.LibraryVersion();
        }
        catch (Exception)
        {
            core = null;
        }

        return AboutInfo.Version(app, core, AppHost.L);
    }

    /// <summary>The section pills (Controls/SidebarItem in JfLibraryTabStyle), sharing the track's width.</summary>
    private void BuildSections()
    {
        var style = (Style)Application.Current.Resources["JfLibraryTabStyle"];
        foreach (var (section, label) in SectionLabels)
        {
            var item = new SidebarItem
            {
                Label = AppHost.L.Get(label),
                GroupName = "SettingsSections",
                Style = style,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                Padding = new Thickness(12, 0, 12, 0),
            };
            var chosen = section;
            item.Click += (_, _) => ShowSection(chosen);
            Sections.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Grid.SetColumn(item, Sections.Children.Count);
            Sections.Children.Add(item);
            _sections[section] = item;
        }
    }

    /// <summary>File Names: Enter sends the template (the preview follows each key).</summary>
    private void OnNameTemplateKeyDown(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        if (e.Key == Windows.System.VirtualKey.Enter)
        {
            ViewModel.Downloads.CommitNameTemplateCommand.Execute(null);
            e.Handled = true;
        }
    }

    /// <summary>File Names: leaving the field sends the template.</summary>
    private void OnNameTemplateLostFocus(object sender, RoutedEventArgs e) => ViewModel.Downloads.CommitNameTemplateCommand.Execute(null);

    /// <summary>Shows one section, from its pill, a link (Sources Settings…, Customize…) or Ctrl+,.</summary>
    public void ShowSection(SettingsSection section)
    {
        foreach (var (each, item) in _sections)
        {
            item.IsChecked = each == section;
        }

        AppearancePanel.Visibility = Bind.Visible(section == SettingsSection.Appearance);
        PlaybackPanel.Visibility = Bind.Visible(section == SettingsSection.Playback);
        LibraryPanel.Visibility = Bind.Visible(section == SettingsSection.Library);
        DownloadsPanel.Visibility = Bind.Visible(section == SettingsSection.Downloads);
        SourcesPanel.Visibility = Bind.Visible(section == SettingsSection.Sources);
        ExtensionsPanel.Visibility = Bind.Visible(section == SettingsSection.Extensions);
        AboutPanel.Visibility = Bind.Visible(section == SettingsSection.About);
        Scroller.ChangeView(null, 0, null, disableAnimation: true);
    }

    /// <summary>The pills fill the track as wide as the reading measure, and scroll sideways when they do not fit.</summary>
    private void OnSectionScrollerSizeChanged(object sender, SizeChangedEventArgs e) => SectionTrack.MinWidth = e.NewSize.Width;

    // ---- choices that have no two-way binding of their own (typed selections) ----

    private void SyncChoices()
    {
        _syncing = true;
        try
        {
            ArtworkStyle.IsChecked = ViewModel.StyleIndex == 0;
            MistyStyle.IsChecked = ViewModel.StyleIndex == 1;
            SystemMode.IsChecked = ViewModel.AppearanceIndex == 0;
            LightMode.IsChecked = ViewModel.AppearanceIndex == 1;
            DarkMode.IsChecked = ViewModel.AppearanceIndex == 2;
            LyricsSmall.IsChecked = ViewModel.LyricsSizeIndex == 0;
            LyricsMedium.IsChecked = ViewModel.LyricsSizeIndex == 1;
            LyricsLarge.IsChecked = ViewModel.LyricsSizeIndex == 2;
            LyricsStart.IsChecked = ViewModel.LyricsAlignIndex == 0;
            LyricsCentre.IsChecked = ViewModel.LyricsAlignIndex == 1;
            foreach (var child in SwatchPanel.Children)
            {
                if (child is RadioButton { Tag: AccentSwatch swatch } button)
                {
                    button.IsChecked = swatch.Index == ViewModel.SwatchIndex;
                }
            }
            IconGrid.SelectedItem = ViewModel.SelectedIcon;
            DeviceBox.SelectedItem = ViewModel.Player.SelectedDevice;
            PresetBox.SelectedItem = ViewModel.SelectedPreset;
        }
        finally
        {
            _syncing = false;
        }
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(SettingsViewModel.StyleIndex) or nameof(SettingsViewModel.SelectedIcon)
            or nameof(SettingsViewModel.AppearanceIndex) or nameof(SettingsViewModel.SwatchIndex)
            or nameof(SettingsViewModel.LyricsSizeIndex) or nameof(SettingsViewModel.LyricsAlignIndex)
            or nameof(SettingsViewModel.SelectedPreset))
        {
            SyncChoices();
        }

        if (e.PropertyName == nameof(SettingsViewModel.FixedHue))
        {
            ShowHuePreview();
        }
    }

    private void OnPlayerChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(PlayerViewModel.SelectedDevice))
        {
            SyncChoices();
        }
    }

    private void OnStyleChecked(object sender, RoutedEventArgs e)
    {
        if (!_syncing)
        {
            ViewModel.StyleIndex = ReferenceEquals(sender, MistyStyle) ? 1 : 0;
        }
    }

    private void OnModeChecked(object sender, RoutedEventArgs e)
    {
        if (!_syncing)
        {
            ViewModel.AppearanceIndex = ReferenceEquals(sender, DarkMode) ? 2 : ReferenceEquals(sender, LightMode) ? 1 : 0;
        }
    }

    private void OnLyricsSizeChecked(object sender, RoutedEventArgs e)
    {
        if (!_syncing)
        {
            ViewModel.LyricsSizeIndex = ReferenceEquals(sender, LyricsSmall) ? 0 : ReferenceEquals(sender, LyricsLarge) ? 2 : 1;
        }
    }

    private void OnLyricsAlignChecked(object sender, RoutedEventArgs e)
    {
        if (!_syncing)
        {
            ViewModel.LyricsAlignIndex = ReferenceEquals(sender, LyricsCentre) ? 1 : 0;
        }
    }

    // ---- accent swatches, the hue track and the previews ----

    /// <summary>Ten round swatches, a radio group Narrator reads as "Hue 250, 1 of 10".</summary>
    private void BuildSwatches()
    {
        if (SwatchPanel.Children.Count > 0)
        {
            return;
        }

        foreach (var swatch in ViewModel.Swatches)
        {
            var button = new RadioButton
            {
                Tag = swatch,
                GroupName = "AccentSwatch",
                Style = (Style)Resources["SwatchStyle"],
                IsChecked = swatch.Index == ViewModel.SwatchIndex,
            };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, swatch.Name);
            ToolTipService.SetToolTip(button, swatch.Name);
            button.Checked += (_, _) =>
            {
                if (!_syncing)
                {
                    ViewModel.ChooseSwatchCommand.Execute(swatch);
                }
            };
            SwatchPanel.Children.Add(button);
        }
    }

    /// <summary>Swatch colours, the hue track and the previews in the current appearance.</summary>
    private void OnThemeChanged()
    {
        var dark = AppHost.App.Appearance == Core.Appearance.Dark;
        foreach (var child in SwatchPanel.Children)
        {
            if (child is RadioButton { Tag: AccentSwatch swatch } button)
            {
                button.Background = new SolidColorBrush(ThemeBrushes.ToColor(dark ? swatch.Dark : swatch.Light));
            }
        }

        var track = new LinearGradientBrush { StartPoint = new Windows.Foundation.Point(0, 0.5), EndPoint = new Windows.Foundation.Point(1, 0.5) };
        foreach (var (offset, color) in AccentChoices.HueTrack(dark))
        {
            track.GradientStops.Add(new GradientStop { Offset = offset, Color = ThemeBrushes.ToColor(color) });
        }

        HueSlider.Background = track;
        ShowHuePreview();
        var backdrop = AppHost.App.BackdropPath;
        ArtworkStylePreview.Source = backdrop is null ? null : new BitmapImage(new Uri(backdrop)) { DecodePixelWidth = 128 };
    }

    private void ShowHuePreview()
    {
        var dark = AppHost.App.Appearance == Core.Appearance.Dark;
        HuePreview.Fill = new SolidColorBrush(ThemeBrushes.ToColor(AccentChoices.Accent((float)ViewModel.FixedHue, dark)));
    }

    private void OnIconChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_syncing && IconGrid.SelectedItem is AppIconChoice choice)
        {
            ViewModel.SelectedIcon = choice;
        }
    }

    private void OnDeviceChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_syncing && DeviceBox.SelectedItem is DeviceOption device)
        {
            ViewModel.Player.SelectedDevice = device;
        }
    }

    /// <summary>A preset chosen in the Preset menu (Custom only labels bands that match none).</summary>
    private void OnPresetChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_syncing && PresetBox.SelectedItem is EqPresetChoice choice && !ReferenceEquals(choice, ViewModel.SelectedPreset))
        {
            ViewModel.SelectedPreset = choice;
        }
    }

    private void OnShowFolderClick(object sender, RoutedEventArgs e) => ViewModel.ShowFolderCommand.Execute((sender as FrameworkElement)?.Tag as FolderItem);

    private void OnRemoveFolderClick(object sender, RoutedEventArgs e) => ViewModel.RemoveFolderCommand.Execute((sender as FrameworkElement)?.Tag as FolderItem);

    /// <summary>A settings choice; the combo box shows each choice by its title (SettingChoice.ToString), as the device list does.</summary>
    private void OnFieldChoiceChanged(object sender, SelectionChangedEventArgs e)
    {
        if (sender is ComboBox { DataContext: SettingField field, SelectedItem: SettingChoice choice })
        {
            field.SelectedOption = choice;
        }
    }

    /// <summary>An ordered checklist's item shown or hidden (CheckBox.IsChecked is a bool?, the item's a bool).</summary>
    private void OnChecklistItemClick(object sender, RoutedEventArgs e)
    {
        if (sender is CheckBox { DataContext: ChecklistItem item } box)
        {
            item.IsChecked = box.IsChecked == true;
        }
    }

    /// <summary>A colour well opens on the field's colour.</summary>
    private void OnColorFlyoutOpening(object sender, object e)
    {
        if (sender is Flyout { Content: ColorPicker { Tag: SettingField field } picker } && Bind.ColorOf(field.Text) is { } color)
        {
            picker.Color = color;
        }
    }

    private void OnFieldColorChanged(ColorPicker sender, ColorChangedEventArgs args)
    {
        if (sender.Tag is SettingField field)
        {
            field.Text = $"#{args.NewColor.R:X2}{args.NewColor.G:X2}{args.NewColor.B:X2}";
        }
    }

    private void OnSignInRequested(SignInViewModel flow) => SourceDialogs.ShowSignIn(flow);

    /// <summary>A service in Add a Source: "Connect to …", then the sign-in dialog.</summary>
    private async void OnSourcePresetClick(object sender, RoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.Tag is SourcePresetViewModel preset)
        {
            await SourceDialogs.ConnectAsync(preset, ViewModel.Connect);
        }
    }

    /// <summary>The notices file next to the app, else the website's licences page.</summary>
    private void OnLicencesClick(object sender, RoutedEventArgs e)
    {
        if (File.Exists(NoticesPath))
        {
            Process.Start(new ProcessStartInfo(NoticesPath) { UseShellExecute = true });
        }
        else if (Links is { } links)
        {
            AppHost.Platform.OpenUrl(links.Licences.AbsoluteUri);
        }
    }

    private void OnWebsiteClick(object sender, RoutedEventArgs e)
    {
        if (Links is { } links)
        {
            AppHost.Platform.OpenUrl(links.Website.AbsoluteUri);
        }
    }

    private void OnOpenLogsClick(object sender, RoutedEventArgs e)
    {
        Directory.CreateDirectory(AppLog.Folder);
        Process.Start(new ProcessStartInfo("explorer.exe", $"\"{AppLog.Folder}\"") { UseShellExecute = false });
    }
}
