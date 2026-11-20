using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Core;

namespace Jellyfesh.Drawing;

/// <summary>
/// The lyrics bar (docs/ui.md, "Discover, details, comments and the lyrics bar"): one line, and its
/// translation when Show Translations is on, in the player bar; Desktop Lyrics shows the line and
/// the next one. Which line and how much of it is sung come from the core's lyrics_bar rule
/// (<see cref="LyricsBars"/>): the active line, or in a gap the last one that started. Its words
/// fill like Now Playing's: the view sweeps the word being sung with a compositor animation
/// (<see cref="Sweep"/>), so no app code runs per frame. It draws only while a view shows it.
/// </summary>
public sealed partial class LyricsBarPresenter : ObservableObject, ITimedView
{
    private readonly IDrawingTime _time;
    private ILyricsCursor? _cursor;
    private LyricDocument? _doc;
    private ulong? _uid;
    private bool _showTranslation = true;
    private int _shown;
    private (int Line, int Word, bool Running, long Serial) _lastSweep = (-1, -1, false, -1);

    /// <summary>The line to show (its own view model: always lit), or null before the first line and without synced lyrics.</summary>
    [ObservableProperty]
    private LyricLineViewModel? _line;

    /// <summary>The line after it (Desktop Lyrics' second row).</summary>
    [ObservableProperty]
    private LyricLineViewModel? _next;

    [ObservableProperty]
    private bool _hasLine;

    public LyricsBarPresenter(IDrawingTime time) => _time = time;

    /// <summary>Raised when the sweep of <see cref="Line"/> changes: the word being sung, or all sung in a gap.</summary>
    public event Action<SweepState?>? SweepChanged;

    /// <summary>The sweep of <see cref="Line"/>: the word being sung, its progress and the time it has left.</summary>
    public SweepState? Sweep { get; private set; }

    /// <summary>How many views show the bar (the player bar, Desktop Lyrics); it draws while any does.</summary>
    public bool IsActive => _shown > 0 && _cursor is not null;

    /// <summary>A view starts or stops showing the bar.</summary>
    public void Show(bool shown)
    {
        _shown = Math.Max(0, _shown + (shown ? 1 : -1));
    }

    /// <summary>Reduced motion: the line lights as a whole, no sweep.</summary>
    public bool ReduceMotion { get; set; }

    public bool ShowTranslation
    {
        get => _showTranslation;
        set
        {
            _showTranslation = value;
            if (Line is { } line)
            {
                line.ShowTranslation = value;
            }

            if (Next is { } next)
            {
                next.ShowTranslation = value;
            }
        }
    }

    /// <summary>Takes a new Lyrics slice (the same as Now Playing's); others than <paramref name="currentUid"/>'s are ignored.</summary>
    public void Load(LyricsState? state, ulong? currentUid)
    {
        if (state is not null && currentUid is not null && state.Uid != currentUid)
        {
            return;
        }

        if (state is not null && state.Uid == _uid && ReferenceEquals(state.Doc, _doc))
        {
            return;
        }

        _uid = state?.Uid;
        _doc = state?.Doc;
        _cursor = state?.Timeline is { } t && _doc is { IsSynced: true } ? _time.CreateCursor(t) : null;
        if (_cursor is { IsEmpty: true })
        {
            _cursor = null;
        }

        Line = null;
        Next = null;
        HasLine = false;
        _lastSweep = (-1, -1, false, -1);
        SetSweep(null);
    }

    public void Render(in TimedFrame frame)
    {
        if (_cursor is null || _doc is null)
        {
            return;
        }

        var tMs = frame.PositionUs <= 0 ? 0 : (uint)Math.Min(frame.PositionUs / 1_000, uint.MaxValue);
        var bar = _cursor.Bar(tMs);
        if (bar.Line != Line?.Index)
        {
            Line = bar.Line is { } l ? Make(l) : null;
            HasLine = Line is not null;
        }

        if (bar.Next != Next?.Index)
        {
            Next = bar.Next is { } n ? Make(n) : null;
        }

        // Told again only when the word, the line, running or the anchor changes (not every wake).
        var sweep = SweepOf(bar, tMs, frame.IsRunning);
        var key = sweep is { } s ? (s.Line, s.Word, s.Running, frame.AnchorSerial) : (-1, -1, false, frame.AnchorSerial);
        if (key != _lastSweep)
        {
            _lastSweep = key;
            SetSweep(sweep);
        }
    }

    public long? NextChangeUs(long positionUs) =>
        _cursor?.NextChange(positionUs <= 0 ? 0 : (uint)Math.Min(positionUs / 1_000, uint.MaxValue)) is { } next ? next * 1_000L : null;

    /// <summary>
    /// The sweep the bar's words show: words before the one being sung are sung, it is swept over
    /// the time it has left, and in a gap every word is sung.
    /// </summary>
    private SweepState? SweepOf(LyricsBarState bar, uint tMs, bool running)
    {
        if (bar.Line is not { } line || Line is not { HasWords: true } shown)
        {
            return null;
        }

        var words = bar.Words;
        var word = 0;
        while (word < words.Count && words[word] >= 1f)
        {
            word++;
        }

        if (word >= words.Count || word >= shown.Words.Count || ReduceMotion)
        {
            // Every word sung (a gap, or reduced motion lights the whole line).
            return new SweepState(line, ReduceMotion && word < words.Count ? word : shown.Words.Count, 0f, TimeSpan.Zero, false);
        }

        var end = shown.Words[word].EndMs;
        return new SweepState(line, word, words[word], TimeSpan.FromMilliseconds(end > tMs ? end - tMs : 0), running);
    }

    private LyricLineViewModel Make(int index)
    {
        var doc = _doc!;
        var line = doc.Lines[index];
        var translation = doc.Track(AuxKind.Translation)?.Lines.GetValueOrDefault(line.Id)?.Text;
        return new LyricLineViewModel(index, line, translation, null, Jellyfesh.Formatting.Script.IsCjkLanguage(doc.Language) || Jellyfesh.Formatting.Script.HasCjk(line.Text), alternateVoice: false)
        {
            // The bar's own copy: always lit, whatever Now Playing's line does.
            IsActive = true,
            Role = LyricLineRole.Active,
            ShowTranslation = _showTranslation,
            ShowRomanization = false,
        };
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
}
