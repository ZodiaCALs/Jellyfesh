using System.ComponentModel;
using System.Numerics;
using Jellyfesh.App.Theming;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Foundation;

namespace Jellyfesh.App.Controls;

/// <summary>
/// One lyric line (after Apple Music, docs/design-controls.md "v0.0.4"; Luminous: 40 epx bold at
/// line height 1.18). Lines still to come are lyrics.ink-inactive (ink-50), sung ones ink-30, both
/// at lyrics.scale-inactive (92 %) scaled from their start edge; the active line is ink-90 at full
/// size, and when it has word timing its words
/// are laid out separately so the word being sung can be swept: a clip slides across a sung-colour
/// copy of the word with a linear, compositor-run animation over the word's remaining duration
/// (Misty MOT-06: progress is linear). Scale changes run dur-3 Fluid on the compositor. No app
/// code runs per frame; the view is told again only when the word, the line or the anchor
/// changes. Reduced motion lights the whole line at once. Scaling never reflows (lyrics tokens).
/// </summary>
public sealed partial class LyricLineView : StackPanel
{
    public static readonly DependencyProperty LineProperty =
        DependencyProperty.Register(nameof(Line), typeof(LyricLineViewModel), typeof(LyricLineView), new PropertyMetadata(null, (d, e) => ((LyricLineView)d).OnLineChanged((LyricLineViewModel?)e.OldValue)));

    public static readonly DependencyProperty LineFontSizeProperty =
        DependencyProperty.Register(nameof(LineFontSize), typeof(double), typeof(LyricLineView), new PropertyMetadata(30.0, (d, _) => ((LyricLineView)d).ApplyTypography()));

    private readonly TextBlock _plain = new() { TextWrapping = TextWrapping.WrapWholeWords, FontWeight = FontWeights.Bold, LineStackingStrategy = LineStackingStrategy.BlockLineHeight };
    private readonly WrapPanel _words = new() { Visibility = Visibility.Collapsed };
    private readonly TextBlock _background = new() { TextWrapping = TextWrapping.Wrap, FontWeight = FontWeights.SemiBold, Visibility = Visibility.Collapsed };
    private readonly TextBlock _translation = new() { TextWrapping = TextWrapping.Wrap, FontWeight = FontWeights.Medium };
    private readonly TextBlock _romanization = new() { TextWrapping = TextWrapping.Wrap, FontWeight = FontWeights.Medium };
    private readonly List<SweptWord> _parts = [];
    private readonly Grid _plainLayer;
    private readonly TextGlow _plainGlow;
    private Storyboard? _sweep;
    private LyricsAlign _align;
    private LyricsLook? _look;

    public LyricLineView()
    {
        Spacing = 4;

        // The look's glow sits behind the line's text (a host the text's shadow is drawn in).
        var glowHost = new Canvas();
        _plainLayer = new Grid { Children = { glowHost, _plain } };
        _plainGlow = new TextGlow(_plain, glowHost);
        Children.Add(_plainLayer);
        Children.Add(_words);
        Children.Add(_background);
        Children.Add(_romanization);
        Children.Add(_translation);
        AutomationProperties.SetAccessibilityView(_words, AccessibilityView.Raw);
        Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent); // the whole line takes taps
        Tapped += OnTapped;
        SizeChanged += (_, _) => ApplyScale(animate: false);
        ApplyTypography();
    }

    /// <summary>The person chose this line: seek to its start (ms).</summary>
    public event EventHandler<uint>? SeekRequested;

    public LyricLineViewModel? Line
    {
        get => (LyricLineViewModel?)GetValue(LineProperty);
        set => SetValue(LineProperty, value);
    }

    public double LineFontSize
    {
        get => (double)GetValue(LineFontSizeProperty);
        set => SetValue(LineFontSizeProperty, value);
    }

    /// <summary>Lyrics settings the panel passes down.</summary>
    public bool ReduceMotion { get; set; }

    /// <summary>The Lyrics alignment setting: the start edge or centred.</summary>
    public LyricsAlign Align
    {
        get => _align;
        set
        {
            if (_align != value)
            {
                _align = value;
                ApplyAlignment();
            }
        }
    }

    /// <summary>The look's lyrics (ADR-0020): the lines' weight, the current line's colour and glow; null is Luminous.</summary>
    internal LyricsLook? Look
    {
        get => _look;
        set
        {
            if (_look != value)
            {
                _look = value;
                ApplyLook();
            }
        }
    }

    /// <summary>Shows the sweep for this line, or ends it.</summary>
    public void ApplySweep(SweepState? sweep)
    {
        StopSweep();
        var line = Line;
        if (line is not { IsActive: true } || _parts.Count == 0 || sweep is not { } s || s.Line != line.Index)
        {
            return; // what is sung stays as last shown
        }

        for (var i = 0; i < _parts.Count; i++)
        {
            _parts[i].Show(i < s.Word ? 1 : i > s.Word ? 0 : s.Progress);
        }

        if (s.Running && s.Word < _parts.Count && s.Remaining > TimeSpan.Zero && !ReduceMotion)
        {
            _sweep = _parts[s.Word].Animate(s.Remaining);
            _sweep?.Begin();
        }
    }

    protected override AutomationPeer OnCreateAutomationPeer() => new LyricLinePeer(this);

    private void OnTapped(object sender, TappedRoutedEventArgs e) => RequestSeek();

    private void RequestSeek()
    {
        if (Line?.StartMs is { } start)
        {
            SeekRequested?.Invoke(this, start);
        }
    }

    private void OnLineChanged(LyricLineViewModel? old)
    {
        if (old is not null)
        {
            old.PropertyChanged -= OnLinePropertyChanged;
        }

        StopSweep();
        _parts.Clear();
        _words.Children.Clear();
        var line = Line;
        if (line is null)
        {
            return;
        }

        line.PropertyChanged += OnLinePropertyChanged;
        _plain.Text = line.IsInstrumental ? "• • •" : line.Text;
        _background.Text = line.BackgroundText ?? string.Empty;
        _background.Visibility = line.BackgroundText is null ? Visibility.Collapsed : Visibility.Visible;
        _translation.Text = line.Translation ?? string.Empty;
        _romanization.Text = line.Romanization ?? string.Empty;
        FlowDirection = line.Direction == TextDirection.Rtl ? FlowDirection.RightToLeft : FlowDirection.LeftToRight;
        AutomationProperties.SetName(this, line.Text);
        ApplyAlignment();
        ApplyTypography();
        UpdateAuxVisibility();
        UpdateActive();
        ApplyScale(animate: false);
    }

    private void OnLinePropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(LyricLineViewModel.IsActive):
                UpdateActive();
                break;
            case nameof(LyricLineViewModel.Role):
                UpdateInk();
                ApplyScale(animate: !ReduceMotion);
                break;
            case nameof(LyricLineViewModel.TranslationVisible) or nameof(LyricLineViewModel.RomanizationVisible):
                UpdateAuxVisibility();
                break;
        }
    }

    private void UpdateAuxVisibility()
    {
        _translation.Visibility = Line is { TranslationVisible: true } ? Visibility.Visible : Visibility.Collapsed;
        _romanization.Visibility = Line is { RomanizationVisible: true } ? Visibility.Visible : Visibility.Collapsed;
    }

    private void UpdateActive()
    {
        var line = Line;
        var active = line?.IsActive == true;
        if (FrameworkElementAutomationPeer.FromElement(this) is { } peer)
        {
            peer.RaisePropertyChangedEvent(SelectionItemPatternIdentifiers.IsSelectedProperty, !active, active);
        }

        if (active && line is { HasWords: true } && !ReduceMotion)
        {
            if (_parts.Count == 0)
            {
                BuildWords(line);
            }

            foreach (var part in _parts)
            {
                part.Show(0);
            }

            _plainLayer.Visibility = Visibility.Collapsed;
            _words.Visibility = Visibility.Visible;
        }
        else
        {
            StopSweep();
            _plainLayer.Visibility = Visibility.Visible;
            _words.Visibility = Visibility.Collapsed;
        }

        UpdateInk();
    }

    /// <summary>The look's weight for the line's text and its lit colour for the words; then the inks and the glow.</summary>
    private void ApplyLook()
    {
        var weight = _look?.Weight ?? FontWeights.Bold;
        _plain.FontWeight = weight;
        var sung = _look?.Highlight ?? (Brush)Application.Current.Resources["JfInk90"];
        foreach (var part in _parts)
        {
            part.SetWeight(weight);
            part.SetSung(sung);
        }

        UpdateInk();
    }

    /// <summary>ink-90 for the active line, ink-30 once sung, lyrics.ink-inactive (ink-50) still to come.</summary>
    private void UpdateInk()
    {
        var res = Application.Current.Resources;
        var role = Line?.IsActive == true ? LyricLineRole.Active : Line?.Role ?? LyricLineRole.Upcoming;
        var ink = (Brush)res[role switch
        {
            LyricLineRole.Active => "JfInk90",
            LyricLineRole.Sung => "JfInk30",
            _ => "JfInk50",
        }];
        // The current line in the look's highlight (ink-90 by default, as before).
        _plain.Foreground = role == LyricLineRole.Active && _look is { } look ? look.Highlight : ink;
        _translation.Foreground = ink;
        _romanization.Foreground = ink;
        _background.Foreground = (Brush)res[role == LyricLineRole.Sung ? "JfInk30" : "JfInk50"];

        // Only the current line glows, and only in a look that asks for it.
        var glow = role == LyricLineRole.Active ? _look?.Glow ?? 0 : 0;
        var color = _look?.GlowColor ?? default;
        _plainGlow.Set(color, glow);
        foreach (var part in _parts)
        {
            part.SetGlow(color, glow);
        }
    }

    /// <summary>Full size for the active line, 92 % for the others, from the start edge (the centre when centred).</summary>
    private void ApplyScale(bool animate)
    {
        var role = Line?.IsActive == true ? LyricLineRole.Active : Line?.Role ?? LyricLineRole.Upcoming;
        var x = _align == LyricsAlign.Center ? ActualWidth / 2 : 0; // FlowDirection mirrors the start edge
        CenterPoint = new Vector3((float)x, (float)(ActualHeight / 2), 0);
        var duration = animate ? TokenValues.Duration(3) : TimeSpan.Zero;
        Motion.Scale(this, (float)NowPlayingMotion.LineScale(role), duration, TokenValues.Fluid);
    }

    /// <summary>The Lyrics alignment setting; a second singer in a duet keeps to the other side when lines start at the edge.</summary>
    private void ApplyAlignment()
    {
        var centre = _align == LyricsAlign.Center;
        var text = centre ? TextAlignment.Center : TextAlignment.Start;
        _plain.TextAlignment = _background.TextAlignment = _translation.TextAlignment = _romanization.TextAlignment = text;
        _words.HorizontalAlignment = centre ? HorizontalAlignment.Center : HorizontalAlignment.Left;
        HorizontalAlignment = !centre && Line?.IsAlternateVoice == true ? HorizontalAlignment.Right : HorizontalAlignment.Stretch;
        ApplyScale(animate: false);
    }

    private void BuildWords(LyricLineViewModel line)
    {
        var res = Application.Current.Resources;
        var unsung = (Brush)res["JfInk70"];
        var sung = _look?.Highlight ?? (Brush)res["JfInk90"];
        var weight = _look?.Weight ?? FontWeights.Bold;
        for (var i = 0; i < line.Words.Count; i++)
        {
            var w = line.Words[i];
            var text = w.JoinsNext || i == line.Words.Count - 1 ? w.Text : w.Text + " ";
            var part = new SweptWord(text, unsung, sung, line.Direction == TextDirection.Rtl);
            part.SetWeight(weight);
            part.Apply(LineFontSize, LineHeight(line));
            _parts.Add(part);
            _words.Children.Add(part.Root);
        }
    }

    private void ApplyTypography()
    {
        var size = LineFontSize;
        var height = LineHeight(Line);
        _plain.FontSize = size;
        _plain.LineHeight = size * height;
        foreach (var part in _parts)
        {
            part.Apply(size, height);
        }

        // Translation and romanization keep their token sizes' ratio to the line (18 and 15 at 40).
        var line = TokenValues.Number("Lyrics.LineSize");
        _translation.FontSize = Math.Round(size * TokenValues.Number("Lyrics.TranslationSize") / line);
        _romanization.FontSize = Math.Round(size * TokenValues.Number("Lyrics.RomanizationSize") / line);
        _background.FontSize = _translation.FontSize;
    }

    /// <summary>lyrics.line's line height (1.18), or the taller one Han, Kana and Hangul need.</summary>
    private static double LineHeight(LyricLineViewModel? line) =>
        TokenValues.Number(line?.IsCjk == true ? "Lyrics.CjkLineHeight" : "Lyrics.LineHeight");

    private void StopSweep()
    {
        _sweep?.Stop();
        _sweep = null;
    }

    /// <summary>
    /// Lyric lines read as a list whose current line is selected (architecture §12); selecting a
    /// line with Narrator seeks to it.
    /// </summary>
    private sealed partial class LyricLinePeer(LyricLineView owner) : FrameworkElementAutomationPeer(owner), ISelectionItemProvider
    {
        public bool IsSelected => owner.Line?.IsActive == true;

        public IRawElementProviderSimple? SelectionContainer => null;

        public void AddToSelection()
        {
        }

        public void RemoveFromSelection()
        {
        }

        public void Select() => owner.RequestSeek();

        protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.ListItem;

        protected override string GetClassNameCore() => nameof(LyricLineView);

        protected override object? GetPatternCore(PatternInterface patternInterface) =>
            patternInterface == PatternInterface.SelectionItem ? this : base.GetPatternCore(patternInterface);
    }
}
