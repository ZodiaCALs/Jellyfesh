using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Settings › Downloads (ADR-0025 §2; docs/ui.md "Settings").</summary>
public class DownloadSettingsTests
{
    private const string Folder = @"C:\Users\ana\Music\Jellyfesh";

    [Fact]
    public void The_section_shows_the_settings_and_the_folder_in_use()
    {
        using var h = new AppHarness(English.Instance).Started();
        var downloads = new DownloadSettingsViewModel(h.S);
        downloads.Apply(
            AppSettings.Default with { Downloads = DownloadSettings.Default with { Quality = DownloadQuality.Normal, WifiOnly = false, AtOnce = 3 } },
            new DownloadsState(1, Folder, [], 1_500_000_000, null));

        Assert.Equal(Folder, downloads.Folder);
        Assert.Equal(2, downloads.QualityIndex);
        Assert.Equal("{album_artist}/{album}/{track} {title}", downloads.NameTemplate);
        Assert.Equal("Preview: Sample Artist/Night Swim/01 Lantern.flac", downloads.NamePreview);
        Assert.Equal((false, true, false, false, 3d), (downloads.AddMissingCoverAndLyrics, downloads.LyricsInTags, downloads.SaveLrcFiles, downloads.PauseOnMetered, downloads.AtOnce));
        Assert.Equal("1.5 GB", downloads.SpaceUsed);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.SetSettings); // no echo
    }

    [Fact]
    public void Each_setting_changes_only_its_own_field()
    {
        using var h = new AppHarness(English.Instance).Started();
        var downloads = new DownloadSettingsViewModel(h.S);
        downloads.Apply(AppSettings.Default, new DownloadsState(1, Folder, [], 0, null));

        downloads.QualityIndex = 1;
        Assert.Equal(DownloadSettings.Default with { Quality = DownloadQuality.High }, Sent(h));
        downloads.AddMissingCoverAndLyrics = true;
        Assert.Equal(DownloadSettings.Default with { AddMissingCoverAndLyrics = true }, Sent(h));
        downloads.SaveLrcFiles = true;
        Assert.Equal(DownloadSettings.Default with { SaveLrcFiles = true }, Sent(h));
        downloads.PauseOnMetered = false;
        Assert.Equal(DownloadSettings.Default with { WifiOnly = false }, Sent(h));
        downloads.AtOnce = 9; // kept within 1 to 4
        Assert.Equal(DownloadSettings.Default with { AtOnce = 4 }, Sent(h));
    }

    [Fact]
    public void File_Names_preview_as_you_type_and_are_sent_when_you_are_done()
    {
        using var h = new AppHarness(English.Instance).Started();
        var downloads = new DownloadSettingsViewModel(h.S);
        downloads.Apply(AppSettings.Default, new DownloadsState(1, Folder, [], 0, null));

        downloads.NameTemplate = "{artist}/{title}";
        Assert.Equal("Preview: Sample Artist/Lantern.flac", downloads.NamePreview);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.SetSettings);

        // A slice for another reason does not undo what is being typed.
        downloads.Apply(AppSettings.Default with { ShowLyricsBar = false }, new DownloadsState(2, Folder, [], 0, null));
        Assert.Equal("{artist}/{title}", downloads.NameTemplate);

        downloads.CommitNameTemplateCommand.Execute(null);
        Assert.Equal(DownloadSettings.Default with { NameTemplate = "{artist}/{title}" }, Sent(h));
        var count = h.Core.Sent.Count;
        downloads.CommitNameTemplateCommand.Execute(null); // unchanged since: nothing more
        downloads.Apply(AppSettings.Default with { Downloads = DownloadSettings.Default with { NameTemplate = "{artist}/{title}" } }, new DownloadsState(3, Folder, [], 0, null));
        downloads.CommitNameTemplateCommand.Execute(null);
        Assert.Equal(count, h.Core.Sent.Count);
    }

    [Fact]
    public async Task The_folder_changes_and_shows_and_Remove_All_asks_first()
    {
        using var h = new AppHarness(English.Instance).Started();
        var downloads = new DownloadSettingsViewModel(h.S);
        downloads.Apply(AppSettings.Default, new DownloadsState(1, Folder, [], 0, null));

        h.Platform.Folder = null; // cancelled
        await downloads.ChangeFolderCommand.ExecuteAsync(null);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.SetSettings);
        h.Platform.Folder = @"D:\Music";
        await downloads.ChangeFolderCommand.ExecuteAsync(null);
        Assert.Equal(DownloadSettings.Default with { Folder = @"D:\Music" }, Sent(h));

        downloads.ShowFolderCommand.Execute(null);
        Assert.Equal([Folder], h.Platform.Shown);

        h.Platform.Confirm = false;
        await downloads.RemoveAllCommand.ExecuteAsync(null);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.RemoveAllDownloads);
        h.Platform.Confirm = true;
        await downloads.RemoveAllCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.RemoveAllDownloads(), h.Core.Sent[^1]);
        Assert.Equal(("Remove All Downloads?", "Every downloaded file is deleted from this device.", "Remove All"), h.Platform.Asked[^1]);
    }

    [Fact]
    public void Settings_follow_the_downloads_slice()
    {
        using var h = new AppHarness(English.Instance).Started();
        var settings = h.App.CreateSettings(_ => null);
        settings.Activate();
        h.Ui.RunAll();
        h.Emit(new CoreEvent.Downloads(new DownloadsState(1, Folder, [], 42_000_000, null)));
        Assert.Equal((Folder, "42 MB"), (settings.Downloads.Folder, settings.Downloads.SpaceUsed));
        settings.Deactivate();
    }

    private static DownloadSettings? Sent(AppHarness h) => Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]).Settings.Downloads;
}
