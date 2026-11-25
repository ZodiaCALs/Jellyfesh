using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// Settings › Downloads (ADR-0025 §2; docs/ui.md "Settings"): the folder (Change… and Show in
/// Explorer), Quality, File Names with the core's live preview, Tags, Network (Pause on Metered
/// Connections and Downloads at Once) and Space Used with Remove All Downloads… (asks first). The settings are the
/// core's; the slice coming back is the truth.
/// </summary>
public sealed partial class DownloadSettingsViewModel(ShellServices s) : ObservableObject
{
    /// <summary>The qualities, in the order shown.</summary>
    public static IReadOnlyList<DownloadQuality> Qualities { get; } =
        [DownloadQuality.Original, DownloadQuality.High, DownloadQuality.Normal, DownloadQuality.Low];

    private bool _syncing;
    private string _savedTemplate = string.Empty;

    /// <summary>Where downloads go: the person's folder, or the default one (the Downloads slice).</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasFolder))]
    private string _folder = string.Empty;

    [ObservableProperty]
    private int _qualityIndex;

    /// <summary>File Names: the template as typed; sent when the person is done with it.</summary>
    [ObservableProperty]
    private string _nameTemplate = string.Empty;

    /// <summary>"Preview: Sample Artist/Night Swim/01 Lantern.flac".</summary>
    [ObservableProperty]
    private string _namePreview = string.Empty;

    [ObservableProperty]
    private bool _addMissingCoverAndLyrics;

    [ObservableProperty]
    private bool _lyricsInTags = true;

    [ObservableProperty]
    private bool _saveLrcFiles;

    /// <summary>Pause on Metered Connections: downloads wait while the connection is metered (the core's wifi_only).</summary>
    [ObservableProperty]
    private bool _pauseOnMetered = true;

    /// <summary>Downloads at Once: 1 to 4.</summary>
    [ObservableProperty]
    private double _atOnce = 2;

    /// <summary>The space the finished downloads take ("1.2 GB").</summary>
    [ObservableProperty]
    private string _spaceUsed = string.Empty;

    public bool HasFolder => Folder.Length > 0;

    /// <summary>The settings and the Downloads slice, as they are now.</summary>
    public void Apply(AppSettings settings, DownloadsState downloads)
    {
        var d = settings.Downloads ?? DownloadSettings.Default;
        _syncing = true;
        try
        {
            Folder = downloads.Folder.Length > 0 ? downloads.Folder : d.Folder ?? string.Empty;
            QualityIndex = Math.Max(0, Qualities.ToList().IndexOf(d.Quality));
            if (NameTemplate == _savedTemplate)
            {
                // Not while the person types a new one.
                NameTemplate = d.NameTemplate;
            }

            _savedTemplate = d.NameTemplate;
            AddMissingCoverAndLyrics = d.AddMissingCoverAndLyrics;
            LyricsInTags = d.LyricsInTags;
            SaveLrcFiles = d.SaveLrcFiles;
            PauseOnMetered = d.WifiOnly;
            AtOnce = Math.Clamp(d.AtOnce, DownloadSettings.MinAtOnce, DownloadSettings.MaxAtOnce);
            SpaceUsed = DetailsRows.Size(downloads.BytesUsed, s.L);
        }
        finally
        {
            _syncing = false;
        }

        UpdatePreview();
    }

    /// <summary>Change…: asks for a folder, then downloads go there.</summary>
    [RelayCommand]
    private async Task ChangeFolder()
    {
        var folder = await s.Platform.PickFolderAsync();
        if (folder is not null)
        {
            Change(d => d with { Folder = folder });
        }
    }

    [RelayCommand]
    private void ShowFolder()
    {
        if (HasFolder)
        {
            s.Platform.ShowInExplorer(Folder);
        }
    }

    /// <summary>
    /// The person is done with File Names (Enter, or the field lost focus): the template is sent,
    /// once (Enter and then leaving the field send nothing more). The slice coming back has the final word.
    /// </summary>
    [RelayCommand]
    private void CommitNameTemplate()
    {
        var template = NameTemplate.Trim();
        if (template != _savedTemplate)
        {
            Change(d => d with { NameTemplate = template });
            _savedTemplate = template;
            NameTemplate = template;
        }
    }

    /// <summary>Remove All Downloads…: asks first, then every downloaded file is deleted.</summary>
    [RelayCommand]
    private async Task RemoveAll()
    {
        if (await s.Platform.ConfirmAsync(
                s.L.Get("Downloads_RemoveAllTitle"), s.L.Get("Downloads_RemoveAllBody"), s.L.Get("Downloads_RemoveAllConfirm"), s.L.Get("Action_Cancel")))
        {
            s.Send(new CoreCommand.RemoveAllDownloads());
        }
    }

    partial void OnNameTemplateChanged(string value) => UpdatePreview();

    partial void OnQualityIndexChanged(int value)
    {
        if (value >= 0 && value < Qualities.Count)
        {
            Change(d => d with { Quality = Qualities[value] });
        }
    }

    partial void OnAddMissingCoverAndLyricsChanged(bool value) => Change(d => d with { AddMissingCoverAndLyrics = value });

    partial void OnLyricsInTagsChanged(bool value) => Change(d => d with { LyricsInTags = value });

    partial void OnSaveLrcFilesChanged(bool value) => Change(d => d with { SaveLrcFiles = value });

    partial void OnPauseOnMeteredChanged(bool value) => Change(d => d with { WifiOnly = value });

    partial void OnAtOnceChanged(double value)
    {
        if (!double.IsNaN(value))
        {
            Change(d => d with { AtOnce = (uint)Math.Clamp(Math.Round(value), DownloadSettings.MinAtOnce, DownloadSettings.MaxAtOnce) });
        }
    }

    private void Change(Func<DownloadSettings, DownloadSettings> edit)
    {
        if (_syncing)
        {
            return;
        }

        var settings = s.Hub.State.Settings;
        var next = edit(settings.Downloads ?? DownloadSettings.Default);
        if (next != settings.Downloads)
        {
            s.Send(new CoreCommand.SetSettings(settings with { Downloads = next }));
        }
    }

    private void UpdatePreview()
    {
        try
        {
            NamePreview = s.L.Format("Downloads_Preview", s.Core.DownloadPreview(NameTemplate.Trim()));
        }
        catch (CoreQueryException)
        {
            NamePreview = string.Empty;
        }
    }
}
