using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// AI Lyrics in the Lyrics pane (docs/ui.md, "AI Lyrics, Customize and Pro"; docs/core-api.md,
/// "AI Lyrics"): Create Synced Lyrics for a song without lyrics, Time These Lyrics under plain ones,
/// the current song's job (its stage, a progress bar, Cancel), what is needed first (a model to
/// download, an API key), a failed job (Try Again), and for lyrics AI Lyrics made the line "Made by
/// AI Lyrics · {engine}" with Make Again and Delete Made Lyrics. The AiLyrics slice decides all of
/// it; Create asks what the slice needs first: the Pro sheet, or "Send This Song to {origin}?".
/// </summary>
public sealed partial class AiLyricsViewModel : ObservableObject
{
    private readonly ShellServices _s;
    private readonly ProViewModel _pro;
    private CoreSnapshot _state = CoreSnapshot.Initial;
    private LyricsJob? _job;

    public AiLyricsViewModel(ShellServices services, ProViewModel pro)
    {
        _s = services;
        _pro = pro;
    }

    /// <summary>Raised by Open AI Lyrics Settings (the app opens Settings › Extensions).</summary>
    public event Action? SettingsRequested;

    /// <summary>Create Synced Lyrics (no lyrics) or Time These Lyrics (plain lyrics), while nothing runs for the song.</summary>
    public bool CanCreate { get; private set; }

    /// <summary>"Create Synced Lyrics", or under plain lyrics "Time These Lyrics".</summary>
    public string CreateLabel { get; private set; } = string.Empty;

    /// <summary>A job for the song playing is waiting or running: its stage, progress and Cancel.</summary>
    public bool HasJob { get; private set; }

    /// <summary>"Reading Audio…", "Sending to api.example.com…", "Recognising… 42%", "Timing Words…".</summary>
    public string JobText { get; private set; } = string.Empty;

    /// <summary>0–100 while recognising; the bar runs without a value otherwise.</summary>
    public double JobProgress { get; private set; }

    public bool JobIsIndeterminate { get; private set; }

    /// <summary>The last job for the song failed: "Couldn't make lyrics: {message}" and Try Again.</summary>
    public bool HasFailure { get; private set; }

    public string FailureText { get; private set; } = string.Empty;

    /// <summary>What is needed first, in words (a model, an API key, why it cannot work).</summary>
    public string NeedText { get; private set; } = string.Empty;

    /// <summary>"Download" or "Open AI Lyrics Settings", or empty when the need has no action.</summary>
    public string NeedAction { get; private set; } = string.Empty;

    /// <summary>The needed model is downloading: "Downloading Base… 35%" with Cancel.</summary>
    public bool IsModelDownloading { get; private set; }

    public double ModelProgress { get; private set; }

    /// <summary>"Made by AI Lyrics · Whisper Base on this device", for lyrics AI Lyrics made.</summary>
    public string MadeBy { get; private set; } = string.Empty;

    /// <summary>Make Again and Delete Made Lyrics (the More menu), for lyrics AI Lyrics made.</summary>
    public bool HasMadeLyrics => MadeBy.Length > 0;

    /// <summary>Create Synced Lyrics in a library song's menu: AI Lyrics is offered (docs/ui.md).</summary>
    public bool OffersForSongs => _state.AiLyrics.Available || !_state.Pro.Active;

    public void Update(CoreSnapshot state)
    {
        _state = state;
        var ai = state.AiLyrics;
        var np = state.NowPlaying;
        var remote = state.Remote.HasSession;
        var lyrics = state.Lyrics is { } l && np is not null && l.Uid == np.Uid ? l : null;
        _job = np is null ? null : ai.JobFor(np.Uid);

        var offered = (ai.Available || !state.Pro.Active) && np is not null && !remote;
        var running = _job is { Stage.IsActive: true };
        HasJob = running;
        JobText = _job is { } job ? StageText(job.Stage) : string.Empty;
        JobIsIndeterminate = _job?.Stage is not LyricsJobStage.Recognising;
        JobProgress = _job?.Stage is LyricsJobStage.Recognising r ? Math.Clamp(r.Fraction, 0f, 1f) * 100 : 0;
        HasFailure = !running && _job?.Stage is LyricsJobStage.Failed;
        FailureText = _job?.Stage is LyricsJobStage.Failed f ? _s.L.Format("AiLyrics_Failed", f.Message) : string.Empty;

        var made = lyrics?.Source as LyricsOrigin.Made;
        MadeBy = made is null ? string.Empty : _s.L.Format("AiLyrics_MadeBy", made.Engine);
        var synced = lyrics?.Doc is { IsSynced: true, Lines.Count: > 0 };
        var plain = lyrics?.Doc is { IsSynced: false, Lines.Count: > 0 };
        CanCreate = offered && !running && !synced;
        CreateLabel = _s.L.Get(plain ? "AiLyrics_Time" : "AiLyrics_Create");

        // What is needed first, while nothing runs and synced lyrics are still wanted.
        NeedText = string.Empty;
        NeedAction = string.Empty;
        IsModelDownloading = false;
        ModelProgress = 0;
        if (offered && !running && !synced && state.Pro.Active && ai.Available)
        {
            switch (ai.Need)
            {
                case AiLyricsNeed.Model m:
                    var model = ai.Models.FirstOrDefault(x => x.Id == m.Id);
                    if (model?.State is SpeechModelStatus.Downloading d)
                    {
                        IsModelDownloading = true;
                        ModelProgress = Math.Clamp(d.Fraction, 0f, 1f) * 100;
                        NeedText = _s.L.Format("AiLyrics_Downloading", m.Name, SpeechModelItem.Percent(d.Fraction));
                    }
                    else
                    {
                        NeedText = _s.L.Format("AiLyrics_NeedModel", m.Name, Sizes.Bytes(m.Bytes, _s.L));
                        NeedAction = _s.L.Get("AiLyrics_Download");
                    }

                    break;
                case AiLyricsNeed.SignIn:
                    NeedText = _s.L.Get("AiLyrics_NeedKey");
                    NeedAction = _s.L.Get("AiLyrics_OpenSettings");
                    break;
                case AiLyricsNeed.Unavailable u when u.Reason.Length > 0:
                    NeedText = u.Reason;
                    break;
            }
        }

        OnPropertyChanged(string.Empty);
    }

    /// <summary>Create Synced Lyrics or Time These Lyrics for the song playing (the Lyrics pane).</summary>
    [RelayCommand]
    private Task Create() => CreateFor(new SongRef.Current());

    /// <summary>Try Again after a failed job, and Make Again for made lyrics: the same as Create.</summary>
    [RelayCommand]
    private Task MakeAgain() => CreateFor(new SongRef.Current());

    /// <summary>
    /// Makes lyrics for <paramref name="song"/> after what the slice needs first: without Pro the Pro
    /// sheet; for the online mode's first upload to an origin, "Send This Song to {origin}?" (Allow
    /// sends AllowAudioUpload); a model or a key the pane offers instead.
    /// </summary>
    public async Task CreateFor(SongRef song)
    {
        var ai = _state.AiLyrics;
        if (!_state.Pro.Active || ai.Need is AiLyricsNeed.Pro)
        {
            _pro.OpenSheetCommand.Execute(null);
            return;
        }

        switch (ai.Need)
        {
            case AiLyricsNeed.Consent c when ai.Plugin is { } plugin:
                if (!await _s.Platform.ConfirmAsync(
                        _s.L.Format("AiLyrics_ConsentTitle", c.Origin),
                        _s.L.Format("AiLyrics_ConsentBody", c.Origin),
                        _s.L.Get("AiLyrics_Allow"),
                        _s.L.Get("Action_Cancel")))
                {
                    return;
                }

                _s.Send(new CoreCommand.AllowAudioUpload(plugin, c.Origin));
                break;
            case AiLyricsNeed.Model m:
                _s.Send(new CoreCommand.DownloadSpeechModel(m.Id));
                return;
            case AiLyricsNeed.SignIn:
                SettingsRequested?.Invoke();
                return;
        }

        _s.Send(new CoreCommand.MakeLyrics(song));
    }

    /// <summary>Cancel: stops the song's job.</summary>
    [RelayCommand]
    private void CancelJob()
    {
        if (_job is { Stage.IsActive: true } job)
        {
            _s.Send(new CoreCommand.CancelLyrics(job.Id));
        }
    }

    /// <summary>The need's action: Download the model, or Open AI Lyrics Settings.</summary>
    [RelayCommand]
    private void ResolveNeed()
    {
        switch (_state.AiLyrics.Need)
        {
            case AiLyricsNeed.Model m:
                _s.Send(new CoreCommand.DownloadSpeechModel(m.Id));
                break;
            case AiLyricsNeed.SignIn:
                SettingsRequested?.Invoke();
                break;
        }
    }

    /// <summary>Cancel while the needed model downloads (what came is kept).</summary>
    [RelayCommand]
    private void CancelModelDownload()
    {
        if (_state.AiLyrics.Need is AiLyricsNeed.Model m)
        {
            _s.Send(new CoreCommand.CancelSpeechModelDownload(m.Id));
        }
    }

    /// <summary>Delete Made Lyrics asks "Delete These Lyrics?" first.</summary>
    [RelayCommand]
    private async Task DeleteMade()
    {
        if (HasMadeLyrics && await _s.Platform.ConfirmAsync(
                _s.L.Get("AiLyrics_DeleteTitle"), _s.L.Get("AiLyrics_DeleteBody"), _s.L.Get("Action_Delete"), _s.L.Get("Action_Cancel")))
        {
            _s.Send(new CoreCommand.DeleteMadeLyrics(new SongRef.Current()));
        }
    }

    private string StageText(LyricsJobStage stage) => stage switch
    {
        LyricsJobStage.Waiting => _s.L.Get("AiLyrics_Waiting"),
        LyricsJobStage.ReadingAudio => _s.L.Get("AiLyrics_Reading"),
        LyricsJobStage.Sending s => _s.L.Format("AiLyrics_Sending", HostOf(s.Origin)),
        LyricsJobStage.Recognising r => _s.L.Format("AiLyrics_Recognising", SpeechModelItem.Percent(r.Fraction)),
        LyricsJobStage.TimingWords => _s.L.Get("AiLyrics_Timing"),
        _ => string.Empty,
    };

    /// <summary>An origin's host ("api.example.com") for "Sending to …".</summary>
    private static string HostOf(string origin) =>
        Uri.TryCreate(origin, UriKind.Absolute, out var uri) && uri.Host.Length > 0 ? uri.Host : origin;
}
