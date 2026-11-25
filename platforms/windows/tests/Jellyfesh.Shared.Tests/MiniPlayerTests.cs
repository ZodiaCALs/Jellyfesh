using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Settings › Appearance › Mini Player (ADR-0026 §4); the window follows them in the app.</summary>
public class MiniPlayerTests
{
    [Fact]
    public void The_app_reads_the_mini_players_settings_with_the_cores_defaults()
    {
        using var h = new AppHarness().Started();
        Assert.Equal(MiniPlayerSettings.Default, h.App.MiniPlayer);
        Assert.Equal((true, false, true, false), (h.App.MiniPlayer.KeepOnTop, h.App.MiniPlayer.OpenWhenWindowClosed, h.App.MiniPlayer.ShowLyricsLine, h.App.MiniPlayer.AndroidPip));

        var changed = new List<string?>();
        h.App.PropertyChanged += (_, e) => changed.Add(e.PropertyName);
        h.Emit(new CoreEvent.Settings(AppSettings.Default with { MiniPlayer = new MiniPlayerSettings(false, true, false, false) }));
        Assert.Equal(new MiniPlayerSettings(false, true, false, false), h.App.MiniPlayer);
        Assert.Contains(nameof(h.App.MiniPlayer), changed);

        // Settings without them (a record built without) read as the defaults.
        h.Emit(new CoreEvent.Settings(AppSettings.Default with { MiniPlayer = null }));
        Assert.Equal(MiniPlayerSettings.Default, h.App.MiniPlayer);
    }

    [Fact]
    public void Settings_switch_each_and_follow_the_slice()
    {
        using var h = new AppHarness().Started();
        var settings = h.App.CreateSettings(_ => null);
        settings.Activate();
        h.Ui.RunAll();
        Assert.Equal((true, false, true), (settings.MiniKeepOnTop, settings.MiniOpenWhenClosed, settings.MiniShowLyricsLine));

        h.Emit(new CoreEvent.Settings(AppSettings.Default with { MiniPlayer = new MiniPlayerSettings(false, true, false, true) }));
        Assert.Equal((false, true, false), (settings.MiniKeepOnTop, settings.MiniOpenWhenClosed, settings.MiniShowLyricsLine));
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.SetSettings); // no echo

        // Each switch changes only its own field (Android's stays as the core has it).
        settings.MiniKeepOnTop = true;
        Assert.Equal(new MiniPlayerSettings(true, true, false, true), Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]).Settings.MiniPlayer);
        settings.MiniOpenWhenClosed = false;
        Assert.Equal(new MiniPlayerSettings(false, false, false, true), Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]).Settings.MiniPlayer);
        settings.MiniShowLyricsLine = true;
        Assert.Equal(new MiniPlayerSettings(false, true, true, true), Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]).Settings.MiniPlayer);
    }
}
