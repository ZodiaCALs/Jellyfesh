using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.Drawing;

/// <summary>
/// The word being sung on the active line. The view sweeps it with a compositor animation from
/// <see cref="Progress"/> to the end over <see cref="Remaining"/>, so no app code runs per frame;
/// it is told again when the word, the line or the anchor changes.
/// </summary>
public readonly record struct SweepState(int Line, int Word, float Progress, TimeSpan Remaining, bool Running);

public sealed class LyricWordViewModel(string text, bool joinsNext, uint startMs, uint endMs)
{
    public string Text { get; } = text;

    public bool JoinsNext { get; } = joinsNext;

    public uint StartMs { get; } = startMs;

    public uint EndMs { get; } = endMs;
}

/// <summary>One lyric line with its translation and romanization.</summary>
public sealed partial class LyricLineViewModel : ObservableObject
{
    [ObservableProperty]
    private bool _isActive;

    /// <summary>Sung, active or still to come: its ink and scale (NowPlayingMotion).</summary>
    [ObservableProperty]
    private LyricLineRole _role;

    [ObservableProperty]
    private bool _showTranslation;

    [ObservableProperty]
    private bool _showRomanization;

    public LyricLineViewModel(int index, LyricLine line, string? translation, string? romanization, bool cjk, bool alternateVoice)
    {
        Index = index;
        Text = line.Text;
        StartMs = line.StartMs;
        Translation = string.IsNullOrWhiteSpace(translation) ? null : translation;
        Romanization = string.IsNullOrWhiteSpace(romanization) ? null : romanization;
        Words = line.Words.Select(w => new LyricWordViewModel(w.Text, w.JoinsNext, w.StartMs, w.EndMs)).ToArray();
        BackgroundText = line.Background.Count == 0
            ? null
            : string.Concat(line.Background.Select(w => w.JoinsNext ? w.Text : w.Text + " ")).TrimEnd();
        Direction = line.Direction;
        IsCjk = cjk;
        IsAlternateVoice = alternateVoice;
    }

    public int Index { get; }

    public string Text { get; }

    public uint? StartMs { get; }

    public string? Translation { get; }

    public string? Romanization { get; }

    public IReadOnlyList<LyricWordViewModel> Words { get; }

    public string? BackgroundText { get; }

    public TextDirection Direction { get; }

    /// <summary>Han, Kana or Hangul: the view uses the taller CJK line height.</summary>
    public bool IsCjk { get; }

    /// <summary>A second singer in a duet: the view aligns the line to the other side.</summary>
    public bool IsAlternateVoice { get; }

    /// <summary>An empty timed line marks an instrumental gap.</summary>
    public bool IsInstrumental => Text.Length == 0;

    public bool HasWords => Words.Count > 0;

    public bool TranslationVisible => ShowTranslation && Translation is not null;

    public bool RomanizationVisible => ShowRomanization && Romanization is not null;

    partial void OnShowTranslationChanged(bool value) => OnPropertyChanged(nameof(TranslationVisible));

    partial void OnShowRomanizationChanged(bool value) => OnPropertyChanged(nameof(RomanizationVisible));
}

/// <summary>
/// The lyrics view's state. It draws when the active line or word changes (next_change), never
/// per frame: the sweep inside a word is the view's compositor animation (<see cref="SweepState"/>).
/// </summary>
public sealed partial class LyricsPresenter : ObservableObject, ITimedView
{
    private readonly IDrawingTime _time;
    private ILyricsCursor? _timeline;
    private LyricDocument? _doc;
    private (int Line, int Word, bool Running, long Serial) _lastSweep = (-1, -1, false, -1);
    private bool _showTranslation = true;
    private bool _showRomanization = true;

    [ObservableProperty]
    private int _activeIndex = -1;

    [ObservableProperty]
    private int _scrollIndex = -1;

    [ObservableProperty]
    private bool _hasLyrics;

    [ObservableProperty]
    private bool _isSynced;

    [ObservableProperty]
    private bool _hasWordTiming;

    [ObservableProperty]
    private bool _hasTranslation;

    [ObservableProperty]
    private bool _hasRomanization;

    [ObservableProperty]
    private string? _language;

    public LyricsPresenter(IDrawingTime time) => _time = time;

    public ObservableCollection<LyricLineViewModel> Lines { get; } = [];

    /// <summary>The track these lyrics belong to.</summary>
    public ulong? Uid { get; private set; }

    /// <summary>Set by the view while the lyrics are visible.</summary>
    public bool IsActive { get; set; }

    /// <summary>Reduced motion: no sweep, the active line lights as a whole and the view jumps.</summary>
    public bool ReduceMotion { get; set; }

    /// <summary>The word sweep to show, or null for none (line timing, a gap, reduced motion).</summary>
    public SweepState? Sweep { get; private set; }

    public event Action<SweepState?>? SweepChanged;

    public bool ShowTranslation
    {
        get => _showTranslation;
        set
        {
            _showTranslation = value;
            foreach (var line in Lines)
            {
                line.ShowTranslation = value;
            }
        }
    }

    public bool ShowRomanization
    {
        get => _showRomanization;
        set
        {
            _showRomanization = value;
            foreach (var line in Lines)
            {
                line.ShowRomanization = value;
            }
        }
    }

    /// <summary>Takes a new Lyrics slice. Lyrics for another track than <paramref name="currentUid"/> are ignored.</summary>
    public void Load(LyricsState? state, ulong? currentUid)
    {
        if (state is not null && currentUid is not null && state.Uid != currentUid)
        {
            return;
        }

        if (state is not null && state.Uid == Uid && ReferenceEquals(state.Doc, _doc))
        {
            return;
        }

        Uid = state?.Uid;
        _doc = state?.Doc;
        _timeline = state?.Timeline is { } t && _doc is { IsSynced: true } ? _time.CreateCursor(t) : null;
        Lines.Clear();
        ActiveIndex = -1;
        ScrollIndex = -1;
        _lastSweep = (-1, -1, false, -1);
        SetSweep(null);

        var doc = _doc;
        HasLyrics = doc is { Lines.Count: > 0 };
        IsSynced = _timeline is { IsEmpty: false };
        HasWordTiming = doc?.Timing == LyricTiming.Word;
        Language = doc?.Language;
        if (doc is null)
        {
            HasTranslation = HasRomanization = false;
            return;
        }

        var translation = doc.Track(AuxKind.Translation);
        var romanization = doc.Track(AuxKind.Romanization);
        HasTranslation = translation is { Lines.Count: > 0 };
        HasRomanization = romanization is { Lines.Count: > 0 };
        var cjkLanguage = Script.IsCjkLanguage(doc.Language);
        var firstAgent = doc.Lines.Select(l => l.Agent).FirstOrDefault(a => a is not null);
        for (var i = 0; i < doc.Lines.Count; i++)
        {
            var line = doc.Lines[i];
            var vm = new LyricLineViewModel(
                i,
                line,
                translation?.Lines.GetValueOrDefault(line.Id)?.Text,
                romanization?.Lines.GetValueOrDefault(line.Id)?.Text,
                cjkLanguage || Script.HasCjk(line.Text),
                alternateVoice: firstAgent is not null && line.Agent is not null && line.Agent != firstAgent)
            {
                ShowTranslation = _showTranslation,
                ShowRomanization = _showRomanization,
            };
            Lines.Add(vm);
        }
    }

    public void Render(in TimedFrame frame)
    {
        if (_timeline is null || _doc is null)
        {
            return;
        }

        var tMs = ToMs(frame.PositionUs);
        var cursor = _timeline.Cursor(tMs);
        var active = cursor.Line ?? -1;
        if (active != ActiveIndex)
        {
            if (ActiveIndex >= 0 && ActiveIndex < Lines.Count)
            {
                Lines[ActiveIndex].IsActive = false;
            }

            if (active >= 0 && active < Lines.Count)
            {
                Lines[active].IsActive = true;
                ScrollIndex = active;
            }

            UpdateRoles(ActiveIndex, active);
            ActiveIndex = active;
        }

        UpdateSweep(cursor, tMs, frame);
    }

    /// <summary>
    /// The lines whose role changed when the active line moved from <paramref name="from"/> to
    /// <paramref name="to"/>: those between the two (every line when either is none).
    /// </summary>
    private void UpdateRoles(int from, int to)
    {
        if (Lines.Count == 0)
        {
            return;
        }

        var all = from < 0 || to < 0;
        var lo = all ? 0 : Math.Max(0, Math.Min(from, to));
        var hi = all ? Lines.Count - 1 : Math.Min(Lines.Count - 1, Math.Max(from, to));
        for (var i = lo; i <= hi; i++)
        {
            Lines[i].Role = NowPlayingMotion.Role(i, to);
        }
    }

    public long? NextChangeUs(long positionUs) =>
        _timeline?.NextChange(ToMs(positionUs)) is { } next ? next * 1_000L : null;

    private void UpdateSweep(LyricsCursor cursor, uint tMs, in TimedFrame frame)
    {
        if (ReduceMotion || cursor is not { Line: { } line, Word: { } word }
            || line >= Lines.Count || word >= Lines[line].Words.Count)
        {
            _lastSweep = (-1, -1, false, frame.AnchorSerial);
            SetSweep(null);
            return;
        }

        var key = (line, word, frame.IsRunning, frame.AnchorSerial);
        if (key == _lastSweep)
        {
            return;
        }

        _lastSweep = key;
        var w = Lines[line].Words[word];
        var remainingMs = w.EndMs > tMs ? w.EndMs - tMs : 0;
        SetSweep(new SweepState(line, word, cursor.WordProgress, TimeSpan.FromMilliseconds(remainingMs), frame.IsRunning));
    }

    private void SetSweep(SweepState? sweep)
    {
        if (Sweep == sweep)
        {
            return;
        }

        Sweep = sweep;
        SweepChanged?.Invoke(sweep);
    }

    private static uint ToMs(long positionUs) =>
        positionUs <= 0 ? 0 : (uint)Math.Min(positionUs / 1_000, uint.MaxValue);
}
