using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>AI Lyrics in the Lyrics pane (docs/ui.md, "AI Lyrics, Customize and Pro"; docs/core-api.md, "AI Lyrics").</summary>
public class AiLyricsTests
{
    private static readonly NowPlayingInfo Song = new(5, new TrackRef.Library(42), "/m/a.flac", "Lantern", "Artist", null, 184_000, null, null);

    private static readonly ProState WithPro = ProState.Initial with { Version = 1, Active = true, Source = new ProSource.Preview(), Preview = true };

    private static AiLyricsState Ready(AiLyricsNeed? need = null) => AiLyricsState.Initial with
    {
        Version = 1,
        Available = true,
        Plugin = "org.jellyfesh.ai-lyrics",
        Need = need ?? new AiLyricsNeed.Nothing(),
    };

    private static LyricsState Plain(ulong uid = 5) => new(uid, new LyricDocument("en", LyricTiming.Unsynced, 0,
        [new LyricLine(0, null, null, null, "Words", [], [], TextDirection.Ltr)], [], "Plain", null), null, null);

    private static AppHarness Start(AiLyricsState ai, ProState? pro = null, LyricsState? lyrics = null)
    {
        var h = new AppHarness();
        h.Core.State = CoreSnapshot.Initial with { NowPlaying = Song, AiLyrics = ai, Pro = pro ?? WithPro, Lyrics = lyrics };
        return h.Started();
    }

    [Fact]
    public async Task A_song_without_lyrics_offers_create_and_makes_them_for_the_song_playing()
    {
        using var h = Start(Ready());
        var ai = h.App.AiLyrics;
        Assert.True(ai.CanCreate);
        Assert.Equal("AiLyrics_Create", ai.CreateLabel);
        Assert.Empty(ai.NeedText);
        await ai.CreateCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.MakeLyrics(new SongRef.Current()), h.Core.Sent[^1]);
        Assert.True(ai.OffersForSongs);
    }

    [Fact]
    public void Plain_lyrics_offer_time_these_lyrics_and_synced_ones_nothing()
    {
        using var h = Start(Ready(), lyrics: Plain());
        Assert.Equal("AiLyrics_Time", h.App.AiLyrics.CreateLabel);
        Assert.True(h.App.AiLyrics.CanCreate);

        var synced = new LyricsState(5, new LyricDocument("en", LyricTiming.Line, 0,
            [new LyricLine(0, 1_000, 2_000, null, "Words", [], [], TextDirection.Ltr)], [], "Lrc", null),
            new LyricTimelineData([1_000], [2_000], [1_000, 2_000], [0, uint.MaxValue], [0, 0], [], []), new LyricsOrigin.Made("org.jellyfesh.ai-lyrics", "Whisper Base on this device"));
        h.Emit(new CoreEvent.Lyrics(synced));
        Assert.False(h.App.AiLyrics.CanCreate);
        Assert.Equal("AiLyrics_MadeBy", h.App.AiLyrics.MadeBy);
        Assert.True(h.App.AiLyrics.HasMadeLyrics);
    }

    [Fact]
    public async Task Without_pro_create_opens_the_pro_sheet()
    {
        using var h = Start(Ready(new AiLyricsNeed.Pro()), pro: ProState.Initial);
        var sheets = 0;
        h.App.Pro.SheetRequested += () => sheets++;
        Assert.True(h.App.AiLyrics.CanCreate); // offered, so Pro can be had
        await h.App.AiLyrics.CreateCommand.ExecuteAsync(null);
        Assert.Equal(1, sheets);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.MakeLyrics);
    }

    [Fact]
    public async Task The_first_upload_to_an_origin_asks_and_allowing_sends_the_consent_first()
    {
        using var h = Start(Ready(new AiLyricsNeed.Consent("https://api.example.com")));
        h.Platform.Confirm = false;
        await h.App.AiLyrics.CreateCommand.ExecuteAsync(null);
        Assert.Equal(("AiLyrics_ConsentTitle", "AiLyrics_ConsentBody", "AiLyrics_Allow"), h.Platform.Asked[^1]);
        Assert.Empty(h.Core.Sent);

        h.Platform.Confirm = true;
        await h.App.AiLyrics.CreateCommand.ExecuteAsync(null);
        Assert.Equal(
            [new CoreCommand.AllowAudioUpload("org.jellyfesh.ai-lyrics", "https://api.example.com"), new CoreCommand.MakeLyrics(new SongRef.Current())],
            h.Core.Sent);
    }

    [Fact]
    public void A_model_to_download_and_an_api_key_are_asked_for_in_the_pane()
    {
        using var h = Start(Ready(new AiLyricsNeed.Model("base", "Base", 147_951_465)));
        var ai = h.App.AiLyrics;
        Assert.Equal(("AiLyrics_NeedModel", "AiLyrics_Download"), (ai.NeedText, ai.NeedAction));
        ai.ResolveNeedCommand.Execute(null);
        Assert.Equal(new CoreCommand.DownloadSpeechModel("base"), h.Core.Sent[^1]);

        h.Emit(new CoreEvent.AiLyrics(Ready(new AiLyricsNeed.Model("base", "Base", 147_951_465)) with
        {
            Version = 2,
            Models = [new SpeechModel("base", "Base", 147_951_465, new SpeechModelStatus.Downloading(0.35f))],
        }));
        Assert.True(ai.IsModelDownloading);
        Assert.Equal(("AiLyrics_Downloading", 35d), (ai.NeedText, Math.Round(ai.ModelProgress)));
        ai.CancelModelDownloadCommand.Execute(null);
        Assert.Equal(new CoreCommand.CancelSpeechModelDownload("base"), h.Core.Sent[^1]);

        var opened = 0;
        ai.SettingsRequested += () => opened++;
        h.Emit(new CoreEvent.AiLyrics(Ready(new AiLyricsNeed.SignIn()) with { Version = 3 }));
        Assert.Equal(("AiLyrics_NeedKey", "AiLyrics_OpenSettings"), (ai.NeedText, ai.NeedAction));
        ai.ResolveNeedCommand.Execute(null);
        Assert.Equal(1, opened);
    }

    [Fact]
    public async Task The_songs_job_shows_its_stage_and_cancels_and_a_failure_tries_again()
    {
        var running = new LyricsJob(9, "Lantern", "Artist", 5, 42, new LyricsJobStage.Recognising(0.42f));
        using var h = Start(Ready() with { Jobs = [running] });
        var ai = h.App.AiLyrics;
        Assert.True(ai.HasJob);
        Assert.False(ai.CanCreate);
        Assert.Equal(("AiLyrics_Recognising", 42d, false), (ai.JobText, Math.Round(ai.JobProgress), ai.JobIsIndeterminate));
        ai.CancelJobCommand.Execute(null);
        Assert.Equal(new CoreCommand.CancelLyrics(9), h.Core.Sent[^1]);

        h.Emit(new CoreEvent.AiLyrics(Ready() with { Version = 2, Jobs = [running with { Stage = new LyricsJobStage.Sending("https://api.example.com") }] }));
        Assert.Equal(("AiLyrics_Sending", true), (ai.JobText, ai.JobIsIndeterminate));

        h.Emit(new CoreEvent.AiLyrics(Ready() with { Version = 3, Jobs = [running with { Stage = new LyricsJobStage.Failed("the provider refused the key") }] }));
        Assert.False(ai.HasJob);
        Assert.True(ai.HasFailure);
        Assert.Equal("AiLyrics_Failed", ai.FailureText);
        await ai.MakeAgainCommand.ExecuteAsync(null);
        Assert.Equal(new CoreCommand.MakeLyrics(new SongRef.Current()), h.Core.Sent[^1]);
    }

    [Fact]
    public async Task Deleting_made_lyrics_asks_first()
    {
        var made = new LyricsState(5, new LyricDocument("en", LyricTiming.Word, 0, [], [], "Elrc", null), null, new LyricsOrigin.Made("org.jellyfesh.ai-lyrics", "whisper-1"));
        using var h = Start(Ready(), lyrics: made);
        await h.App.AiLyrics.DeleteMadeCommand.ExecuteAsync(null);
        Assert.Equal(("AiLyrics_DeleteTitle", "AiLyrics_DeleteBody", "Action_Delete"), h.Platform.Asked[^1]);
        Assert.Equal(new CoreCommand.DeleteMadeLyrics(new SongRef.Current()), h.Core.Sent[^1]);
    }

    [Fact]
    public async Task A_library_song_from_its_menu_and_nothing_during_a_remote_session()
    {
        using var h = Start(Ready());
        await h.App.AiLyrics.CreateFor(new SongRef.Library(42));
        Assert.Equal(new CoreCommand.MakeLyrics(new SongRef.Library(42)), h.Core.Sent[^1]);

        h.Emit(new CoreEvent.Remote(RemoteState.Initial with { Version = 1, Source = "org.spotify" }));
        Assert.False(h.App.AiLyrics.CanCreate);
    }
}
