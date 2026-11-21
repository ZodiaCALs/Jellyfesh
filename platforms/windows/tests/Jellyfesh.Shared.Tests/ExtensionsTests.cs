using System.Globalization;
using System.Text.Json;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Services;
using Jellyfesh.Settings;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Settings › Extensions, Jellyfesh Pro, updates and the settings form's profile 1.2 (docs/ui.md, "Settings").</summary>
public class ExtensionsTests
{
    private static readonly AboutInfo.Links Site = AboutInfo.LinksFor("https://jellyfesh.example/")!;

    private static PluginInfo Extension(string id, string name, string capability, string? account = null, bool requiresPro = true, string schema = "{}") =>
        new(id, name, "0.0.7", "Jellyfesh", PluginBadge.Official, false, [capability], capability == "make_lyrics" ? [AuthMethod.ApiKey] : [],
            true, account, false, schema, "{}", null, Kind: PluginKind.Extension, RequiresPro: requiresPro);

    private static PluginInfo Source(string id) =>
        new(id, "Navidrome", "1.0.0", "Someone", PluginBadge.Official, false, ["search"], [AuthMethod.PasswordExchange], true, null, false, "{}", "{}", null);

    private static async Task Poll(Func<bool> done)
    {
        for (var i = 0; i < 300 && !done(); i++)
        {
            await Task.Delay(10, TestContext.Current.CancellationToken);
        }

        Assert.True(done());
    }

    [Fact]
    public void Pro_says_where_it_comes_from_and_offers_what_this_build_allows()
    {
        using var h = new AppHarness().Started();
        var pro = h.App.Pro;
        pro.Update(ProState.Initial);
        Assert.Equal("Pro_Inactive", pro.Status);
        Assert.True(pro.CanEnterKey);
        Assert.False(pro.CanBuy); // no store glue in this build
        Assert.False(pro.CanRemoveLicence);

        pro.Update(ProState.Initial with { Active = true, Source = new ProSource.Licence("ABCD", 0) });
        Assert.Equal("Pro_Licence", pro.Status);
        Assert.True(pro.CanRemoveLicence);
        Assert.False(pro.CanEnterKey);

        pro.Update(ProState.Initial with { Active = true, Source = new ProSource.Purchased(ProStore.MicrosoftStore) });
        Assert.Equal("Pro_Purchased", pro.Status);
        pro.Update(ProState.Initial with { Active = true, Preview = true, Source = new ProSource.Preview() });
        Assert.Equal("Pro_Preview", pro.Status);

        // App Store and Google Play builds take no keys.
        pro.Update(ProState.Initial with { LicenceKeys = false });
        Assert.False(pro.CanEnterKey);
    }

    [Fact]
    public async Task A_store_build_sells_pro_at_the_stores_price()
    {
        using var h = new AppHarness();
        var store = new FakeStore { Price = "4,99 €" };
        var app = new AppViewModel(h.Core, h.Ui, KeyLocalizer.Instance, h.Platform, Interop.PortDrawingTime.Instance, h.Clock, h.Timer, store, Site);
        app.Pro.Update(ProState.Initial with { Stores = [ProStore.MicrosoftStore], LicenceKeys = true });
        Assert.True(app.Pro.CanBuy);
        await Poll(() => app.Pro.Price is not null);
        Assert.Equal("Pro_Buy", app.Pro.BuyLabel);
        await app.Pro.BuyCommand.ExecuteAsync(null);
        await app.Pro.RestoreCommand.ExecuteAsync(null);
        Assert.Equal((1, 1), (store.Bought, store.Restored));

        app.Pro.OpenPrivacyPolicyCommand.Execute(null);
        app.Pro.OpenLicenceAgreementCommand.Execute(null);
        app.Pro.OpenTermsOfSaleCommand.Execute(null);
        Assert.Equal(
            ["https://jellyfesh.example/legal/privacy.html", "https://jellyfesh.example/legal/eula.html", "https://jellyfesh.example/legal/terms-of-sale.html"],
            h.Platform.Opened);
    }

    [Fact]
    public async Task Removing_the_licence_asks_first_and_pro_actions_open_the_sheet_without_pro()
    {
        using var h = new AppHarness().Started();
        var pro = h.App.Pro;
        pro.Update(ProState.Initial with { Active = true, Source = new ProSource.Licence("ABCD", 0) });
        h.Platform.Confirm = false;
        await pro.RemoveLicenceCommand.ExecuteAsync(null);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.RemoveLicence);
        h.Platform.Confirm = true;
        await pro.RemoveLicenceCommand.ExecuteAsync(null);
        Assert.Equal(("Pro_RemoveTitle", "Pro_RemoveBody", "Pro_RemoveAction"), h.Platform.Asked[^1]);
        Assert.IsType<CoreCommand.RemoveLicence>(h.Core.Sent[^1]);

        var sheets = 0;
        pro.SheetRequested += () => sheets++;
        var ran = 0;
        Assert.True(pro.Require(() => ran++));
        pro.Update(ProState.Initial);
        Assert.False(pro.Require(() => ran++));
        Assert.Equal((1, 1), (ran, sheets));
    }

    [Fact]
    public void A_licence_key_is_activated_by_the_core_and_its_refusal_worded()
    {
        using var h = new AppHarness().Started();
        LicenceKeyViewModel? dialog = null;
        h.App.Pro.LicenceKeyRequested += d => dialog = d;
        h.App.Pro.EnterLicenceKeyCommand.Execute(null);
        Assert.NotNull(dialog);
        Assert.False(dialog.ActivateCommand.CanExecute(null));
        dialog.Key = "  JFP1-AAAA-BBBB  ";
        dialog.ActivateCommand.Execute(null);
        Assert.Equal(new CoreCommand.ActivateLicence("JFP1-AAAA-BBBB"), h.Core.Sent[^1]);
        Assert.True(dialog.IsWorking);

        h.Emit(new CoreEvent.Pro(ProState.Initial with { Version = 1, LicenceError = LicenceError.BadSignature }));
        Assert.Equal("Licence_Invalid", dialog.Error);
        Assert.False(dialog.IsWorking);

        var closed = false;
        dialog.Activated += () => closed = true;
        dialog.ActivateCommand.Execute(null);
        h.Emit(new CoreEvent.Pro(ProState.Initial with { Version = 2, Active = true, Source = new ProSource.Licence("BBBB", 0) }));
        Assert.True(closed);

        // A core that cannot take keys yet says so in the dialog.
        dialog.ActivateCommand.Execute(null);
        var id = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.CommandFailed(id, "not yet"));
        Assert.Equal("not yet", dialog.Error);
        Assert.Empty(h.App.Notifications.Items); // shown in the dialog, not as a notice
        dialog.Close();

        Assert.Equal("Licence_Malformed", ProViewModel.LicenceErrorText(LicenceError.Malformed, KeyLocalizer.Instance));
        Assert.Equal("Licence_Invalid", ProViewModel.LicenceErrorText(LicenceError.UnknownKey, KeyLocalizer.Instance));
        Assert.Equal("Licence_WrongProduct", ProViewModel.LicenceErrorText(LicenceError.WrongProduct, KeyLocalizer.Instance));
        Assert.Equal("Licence_Revoked", ProViewModel.LicenceErrorText(LicenceError.Revoked, KeyLocalizer.Instance));
    }

    [Fact]
    public void Extensions_are_listed_apart_from_sources_with_what_their_kind_adds()
    {
        using var h = new AppHarness();
        var models = new[]
        {
            new SpeechModel("tiny", "Tiny", 77_691_713, new SpeechModelStatus.NotDownloaded()),
            new SpeechModel("base", "Base", 147_951_465, new SpeechModelStatus.Downloading(0.35f)),
            new SpeechModel("small", "Small", 487_601_967, new SpeechModelStatus.Ready()),
        };
        h.Core.State = CoreSnapshot.Initial with
        {
            Sources = new SourcesState(1, [Source("org.sub"), Extension("org.jellyfesh.ai-lyrics", "AI Lyrics", "make_lyrics", account: "key"), Extension("org.jellyfesh.customize", "Customize", "appearance")], [], false, CanInstallPlugins: false),
            AiLyrics = AiLyricsState.Initial with { Version = 1, Models = models, Consents = [new AudioConsent("org.jellyfesh.ai-lyrics", "https://api.example.com"), new AudioConsent("other", "https://x.example")] },
        };
        h.Started();
        var settings = h.App.CreateSettings(_ => null);
        Assert.Equal(["org.sub"], settings.Plugins.Select(p => p.Id));
        Assert.Equal(["org.jellyfesh.ai-lyrics", "org.jellyfesh.customize"], settings.Extensions.Select(e => e.Plugin.Id));
        Assert.False(settings.CanInstallPlugins);

        var ai = settings.Extensions[0];
        Assert.True(ai.IsAiLyrics);
        Assert.True(ai.ShowsProBadge);
        Assert.Equal("AiLyrics_KeyAdded", ai.KeyStatus);
        Assert.Equal(["Models_NameSize", "Models_NameSize", "Models_NameSize"], ai.Models.Select(m => m.Title));
        Assert.Equal((true, false, false), (ai.Models[0].CanDownload, ai.Models[0].IsDownloading, ai.Models[0].CanDelete));
        Assert.Equal(("Models_Downloading", 35d), (ai.Models[1].Status, Math.Round(ai.Models[1].Progress)));
        Assert.Equal(("Models_Ready", true), (ai.Models[2].Status, ai.Models[2].CanDelete));
        Assert.Equal(["https://api.example.com"], ai.Consents.Select(c => c.Origin)); // only its own

        ai.Models[0].DownloadCommand.Execute(null);
        ai.Models[1].CancelCommand.Execute(null);
        ai.Consents[0].StopAllowingCommand.Execute(null);
        Assert.Equal(
            [new CoreCommand.DownloadSpeechModel("tiny"), new CoreCommand.CancelSpeechModelDownload("base"), new CoreCommand.RevokeAudioUpload("org.jellyfesh.ai-lyrics", "https://api.example.com")],
            h.Core.Sent.TakeLast(3));

        var customize = settings.Extensions[1];
        Assert.True(customize.IsCustomize);
        Assert.False(customize.HasModels);
        customize.ResetToDefaultsCommand.Execute(null);
        Assert.Equal(new CoreCommand.ResetPluginSettings("org.jellyfesh.customize"), h.Core.Sent[^1]);

        // With Pro, no badge.
        h.Emit(new CoreEvent.Pro(ProState.Initial with { Version = 1, Active = true, Source = new ProSource.Preview() }));
        settings.Activate();
        h.Emit(new CoreEvent.Pro(ProState.Initial with { Version = 2, Active = true, Source = new ProSource.Preview() }));
        Assert.False(settings.Extensions[0].ShowsProBadge);
        Assert.False(settings.CustomizeNeedsPro);
    }

    [Fact]
    public async Task Deleting_a_model_asks_first()
    {
        using var h = new AppHarness().Started();
        var item = new SpeechModelItem(h.S);
        item.Update(new SpeechModel("base", "Base", 147_951_465, new SpeechModelStatus.Ready()));
        await item.DeleteCommand.ExecuteAsync(null);
        Assert.Equal(("Models_DeleteTitle", "Models_DeleteBody", "Action_Delete"), h.Platform.Asked[^1]);
        Assert.Equal(new CoreCommand.DeleteSpeechModel("base"), h.Core.Sent[^1]);
        Assert.Equal("Size_MB", Sizes.Bytes(147_951_465, KeyLocalizer.Instance));
        Assert.Equal("Size_GB", Sizes.Bytes(1_600_000_000, KeyLocalizer.Instance));
    }

    [Fact]
    public void Lyrics_bar_and_desktop_lyrics_are_appearance_settings_the_core_keeps()
    {
        using var h = new AppHarness().Started();
        var settings = h.App.CreateSettings(_ => null);
        Assert.True(settings.ShowLyricsBar);
        Assert.False(settings.DesktopLyrics);
        settings.ShowLyricsBar = false;
        Assert.False(Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]).Settings.ShowLyricsBar);
        settings.DesktopLyrics = true;
        Assert.True(Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]).Settings.DesktopLyrics);

        var opened = 0;
        settings.ExtensionsRequested += () => opened++;
        settings.OpenCustomizeCommand.Execute(null);
        Assert.Equal(1, opened);
        Assert.True(settings.CustomizeNeedsPro);
    }

    [Fact]
    public void Updates_show_only_where_supported_and_keep_their_setting_in_the_core()
    {
        using var h = new AppHarness().Started();
        var settings = h.App.CreateSettings(_ => null);
        Assert.False(settings.Updates.IsSupported);
        settings.Activate();
        h.Emit(new CoreEvent.UpdateStatus(UpdateState.Initial with { Version = 1, Supported = true, Auto = true, Available = new UpdateInfo("0.0.8", "https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.8") }));
        var updates = settings.Updates;
        Assert.True(updates.IsSupported);
        Assert.Equal("Update_Available", updates.AvailableText);
        updates.CheckCommand.Execute(null);
        Assert.IsType<CoreCommand.CheckForUpdates>(h.Core.Sent[^1]);
        updates.ViewReleaseCommand.Execute(null);
        Assert.Equal("https://github.com/ZodiaCALs/Jellyfesh/releases/tag/v0.0.8", h.Platform.Opened[^1]);
        updates.CheckAutomatically = false;
        Assert.False(Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]).Settings.CheckForUpdates);
    }

    [Fact]
    public async Task Plugin_settings_follow_the_apps_language()
    {
        using var h = new AppHarness(new Table(new() { ["Language_Tag"] = "zh-Hans" }));
        h.Core.LocalizedSchemas["org.jellyfesh.customize"] = """{"type":"object","properties":{"glass":{"type":"number","title":"玻璃","minimum":0,"maximum":1,"multipleOf":0.05}}}""";
        h.Core.State = CoreSnapshot.Initial with { Sources = new SourcesState(1, [Extension("org.jellyfesh.customize", "Customize", "appearance", schema: """{"type":"object","properties":{"glass":{"type":"number","title":"Glass","minimum":0,"maximum":1}}}""")], [], false) };
        h.Started();
        var settings = h.App.CreateSettings(_ => null);
        var plugin = settings.Extensions[0].Plugin;
        await Poll(() => plugin.Form?.Fields.FirstOrDefault()?.Label == "玻璃");
        Assert.Equal(["zh-Hans"], h.Core.SchemaLanguages);
        Assert.Equal("en", KeyLocalizer.Instance.LanguageTag());
    }

    [Fact]
    public void The_settings_form_reads_profile_one_two()
    {
        const string schema = """
            {"type":"object","properties":{
              "corners":{"type":"number","title":"Corners","minimum":0,"maximum":2,"multipleOf":0.05,"default":1,"x-jf-group":"Look"},
              "typeface":{"type":"string","title":"Typeface","oneOf":[{"const":"system","title":"System"},{"const":"mono","title":"Monospaced"}],"default":"system","x-jf-group":"Look"},
              "highlight_colour":{"type":"string","format":"color","title":"Highlight Colour","default":"#FF8A3D","x-jf-group":"Lyrics"},
              "sidebar":{"type":"array","title":"Sidebar","items":{"type":"string","oneOf":[{"const":"home","title":"Home"},{"const":"albums","title":"Albums"},{"const":"songs","title":"Songs"}]},"uniqueItems":true,"default":["songs","home"],"x-jf-group":"Layout"},
              "count":{"type":"integer","title":"Count","minimum":1,"maximum":10,"default":3},
              "legacy":{"type":"string","enum":["a","b"],"default":"b"}
            }}
            """;
        var form = PluginSettingsForm.Create(schema, """{"corners":1.37,"sidebar":["albums","home"]}""");
        var f = form.Fields.ToDictionary(x => x.Name);
        Assert.Equal(SettingFieldKind.Slider, f["corners"].Kind);
        Assert.Equal((0d, 2d, 0.05), (f["corners"].Minimum!.Value, f["corners"].Maximum!.Value, f["corners"].Step!.Value));
        Assert.Equal(1.37, f["corners"].Value);
        Assert.Equal(SettingFieldKind.Choice, f["typeface"].Kind);
        Assert.Equal(["System", "Monospaced"], f["typeface"].Options.Select(o => o.Title));
        Assert.Equal("system", f["typeface"].SelectedOption!.Value);
        Assert.Equal(SettingFieldKind.Color, f["highlight_colour"].Kind);
        Assert.Equal("#FF8A3D", f["highlight_colour"].Text);
        Assert.Equal(SettingFieldKind.Checklist, f["sidebar"].Kind);
        Assert.Equal([("Albums", true), ("Home", true), ("Songs", false)], f["sidebar"].Items.Select(i => (i.Title, i.IsChecked)));
        Assert.Equal(SettingFieldKind.Slider, f["count"].Kind); // an integer between bounds too (ADR-0020 §3)
        Assert.Equal(["a", "b"], f["legacy"].Choices);
        Assert.Equal("b", f["legacy"].Choice);

        // Group headings: once per run of a group.
        Assert.Equal([true, false, true, true, false, false], form.Fields.Select(x => x.StartsGroup));
        Assert.Equal(["Look", "Look", "Lyrics", "Layout", null, null], form.Fields.Select(x => x.Group));

        // Editing: the slider snaps to its step, the checklist keeps its order, the colour is checked.
        f["typeface"].SelectedOption = f["typeface"].Options[1];
        f["sidebar"].MoveDownCommand.Execute(f["sidebar"].Items[0]);
        f["sidebar"].Items[2].IsChecked = true;
        f["count"].Value = 6.6;
        Assert.True(form.Validate());
        using var json = JsonDocument.Parse(form.ToJson());
        var root = json.RootElement;
        Assert.Equal(1.35, root.GetProperty("corners").GetDouble(), 6);
        Assert.Equal("mono", root.GetProperty("typeface").GetString());
        Assert.Equal("#FF8A3D", root.GetProperty("highlight_colour").GetString());
        Assert.Equal(["home", "albums", "songs"], root.GetProperty("sidebar").EnumerateArray().Select(e => e.GetString()));
        Assert.Equal(JsonValueKind.Number, root.GetProperty("count").ValueKind);
        Assert.Equal(7, root.GetProperty("count").GetInt32());

        f["highlight_colour"].Text = "orange";
        Assert.False(form.Validate());
        Assert.Equal("Settings_NotAColour", f["highlight_colour"].Error);
        f["corners"].Value = 0.75;
        Assert.Equal(0.75.ToString("0.00", CultureInfo.CurrentCulture), f["corners"].ValueText);
        Assert.Equal(1d, PluginSettingsForm.Create(schema, "{}").Fields[0].Value); // the default
    }

    private sealed class FakeStore : IProStore
    {
        public string? Price { get; init; }

        public int Bought { get; private set; }

        public int Restored { get; private set; }

        public Task<string?> PriceAsync() => Task.FromResult(Price);

        public Task<ProPurchase> BuyAsync()
        {
            Bought++;
            return Task.FromResult(ProPurchase.Bought);
        }

        public Task RestoreAsync()
        {
            Restored++;
            return Task.CompletedTask;
        }
    }

    private sealed class Table(Dictionary<string, string> table) : ILocalizer
    {
        public CultureInfo Culture => CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}
