using Jellyfesh.Drawing;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Jellyfesh.App.Controls;

/// <summary>
/// One line of the lyrics bar (the player bar, Desktop Lyrics): on one row, cut at the edge, with
/// its translation under it when Show Translations is on. A word-timed line fills word by word in
/// the highlight colour (<see cref="ApplySweep"/>, the same sweep as Now Playing's); a line-timed
/// one shows whole. Unlike Now Playing's lines it never dims: the bar shows the line until the next
/// one starts. It keeps to its column (docs/ui.md, v0.0.9): a whole line and the translation end
/// with "…"; a word-filled line too long for the row fades out at its end, word by word over the
/// last <see cref="FadeWidth"/>, and is cut at the edge.
/// </summary>
public sealed partial class LyricsBarLine : StackPanel
{
    public static readonly DependencyProperty LineProperty =
        DependencyProperty.Register(nameof(Line), typeof(LyricLineViewModel), typeof(LyricsBarLine), new PropertyMetadata(null, (d, e) => ((LyricsBarLine)d).OnLineChanged((LyricLineViewModel?)e.OldValue)));

    public static readonly DependencyProperty LineFontSizeProperty =
        DependencyProperty.Register(nameof(LineFontSize), typeof(double), typeof(LyricsBarLine), new PropertyMetadata(13.0, (d, _) => ((LyricsBarLine)d).Rebuild()));

    public static readonly DependencyProperty DimProperty =
        DependencyProperty.Register(nameof(Dim), typeof(bool), typeof(LyricsBarLine), new PropertyMetadata(false, (d, _) => ((LyricsBarLine)d).Rebuild()));

    /// <summary>The width over which a word-filled line too long for its row fades out at the row's end.</summary>
    private const double FadeWidth = 32;

    private readonly Grid _row = new();
    private readonly TextBlock _plain = new() { TextWrapping = TextWrapping.NoWrap, TextTrimming = TextTrimming.CharacterEllipsis, FontWeight = FontWeights.SemiBold };
    private readonly StackPanel _words = new() { Orientation = Orientation.Horizontal, Visibility = Visibility.Collapsed };
    private readonly TextBlock _translation = new() { TextWrapping = TextWrapping.NoWrap, TextTrimming = TextTrimming.CharacterEllipsis };
    private readonly List<SweptWord> _parts = [];
    private Brush? _highlight;
    private Microsoft.UI.Xaml.Media.Animation.Storyboard? _sweep;

    public LyricsBarLine()
    {
        Spacing = 0;
        _row.Children.Add(_plain);
        _row.Children.Add(_words);
        Children.Add(_row);
        Children.Add(_translation);
        AutomationProperties.SetAccessibilityView(_words, AccessibilityView.Raw);
        SizeChanged += (_, e) =>
        {
            _row.Clip = new RectangleGeometry { Rect = new Windows.Foundation.Rect(0, 0, e.NewSize.Width, Math.Max(e.NewSize.Height, 1)) };
            FadeEdge();
        };
    }

    /// <summary>The bar's line (LyricsBarPresenter.Line or Next), or null for none.</summary>
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

    /// <summary>Desktop Lyrics' next line: quieter, and not swept.</summary>
    public bool Dim
    {
        get => (bool)GetValue(DimProperty);
        set => SetValue(DimProperty, value);
    }

    /// <summary>The colour sung words light up in: the look's own colour (ADR-0020), or the accent when null.</summary>
    public Brush? Highlight
    {
        get => _highlight;
        set
        {
            if (!ReferenceEquals(_highlight, value))
            {
                _highlight = value;
                var sung = value ?? (Brush)Application.Current.Resources["JfAccentText"];
                foreach (var part in _parts)
                {
                    part.SetSung(sung);
                }
            }
        }
    }

    private void OnLineChanged(LyricLineViewModel? old)
    {
        if (old is not null)
        {
            old.PropertyChanged -= OnLinePropertyChanged;
        }

        if (Line is { } line)
        {
            line.PropertyChanged += OnLinePropertyChanged;
        }

        Rebuild();
    }

    /// <summary>Show Translations changed while the line shows.</summary>
    private void OnLinePropertyChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(LyricLineViewModel.TranslationVisible))
        {
            _translation.Visibility = Line is { TranslationVisible: true } ? Visibility.Visible : Visibility.Collapsed;
        }
    }

    /// <summary>Shows the sweep (LyricsBarPresenter.Sweep) when it is this line's.</summary>
    public void ApplySweep(SweepState? sweep)
    {
        _sweep?.Stop();
        _sweep = null;
        if (Line is not { } line || _parts.Count == 0 || sweep is not { } s || s.Line != line.Index)
        {
            return;
        }

        for (var i = 0; i < _parts.Count; i++)
        {
            _parts[i].Show(i < s.Word ? 1 : i > s.Word ? 0 : s.Progress);
        }

        if (s.Running && s.Word < _parts.Count && s.Remaining > TimeSpan.Zero)
        {
            _sweep = _parts[s.Word].Animate(s.Remaining);
            _sweep?.Begin();
        }
    }

    private void Rebuild()
    {
        _sweep?.Stop();
        _sweep = null;
        _parts.Clear();
        _words.Children.Clear();
        var line = Line;
        var res = Application.Current.Resources;
        var ink = (Brush)res[Dim ? "JfInk50" : "JfInk90"];
        _plain.FontSize = LineFontSize;
        _plain.Foreground = ink;
        _translation.FontSize = Math.Max(10, Math.Round(LineFontSize * 0.8));
        _translation.Foreground = (Brush)res["JfInk70"];
        if (line is null)
        {
            _plain.Text = string.Empty;
            _translation.Visibility = Visibility.Collapsed;
            AutomationProperties.SetName(this, string.Empty);
            return;
        }

        _plain.Text = line.IsInstrumental ? "• • •" : line.Text;
        _translation.Text = line.Translation ?? string.Empty;
        _translation.Visibility = line.TranslationVisible ? Visibility.Visible : Visibility.Collapsed;
        FlowDirection = line.Direction == Core.TextDirection.Rtl ? FlowDirection.RightToLeft : FlowDirection.LeftToRight;
        AutomationProperties.SetName(this, line.Text);
        var swept = line.HasWords && !Dim;
        if (swept)
        {
            // The sung part in the highlight colour, the rest quieter.
            var unsung = (Brush)res["JfInk70"];
            var sung = _highlight ?? (Brush)res["JfAccentText"];
            for (var i = 0; i < line.Words.Count; i++)
            {
                var w = line.Words[i];
                var text = w.JoinsNext || i == line.Words.Count - 1 ? w.Text : w.Text + " ";
                var part = new SweptWord(text, unsung, sung, line.Direction == Core.TextDirection.Rtl);
                part.Apply(LineFontSize, 1.3);
                part.Root.SizeChanged += (_, _) => FadeEdge();
                _parts.Add(part);
                _words.Children.Add(part.Root);
            }
        }

        _plain.Visibility = swept ? Visibility.Collapsed : Visibility.Visible;
        _words.Visibility = swept ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>
    /// A word-filled line wider than its row fades out at the row's end: each word that reaches into
    /// the last <see cref="FadeWidth"/> dims with how far it reaches, and the clip cuts what is past
    /// the edge. A line that fits shows whole. Words run in the line's direction, so the end is the
    /// left edge of a right-to-left line.
    /// </summary>
    private void FadeEdge()
    {
        var width = ActualWidth;
        var total = 0.0;
        foreach (var part in _parts)
        {
            total += part.Root.ActualWidth;
        }

        var x = 0.0;
        foreach (var part in _parts)
        {
            x += part.Root.ActualWidth;
            part.Root.Opacity = width <= 0 || total <= width ? 1 : Math.Clamp(0.3 + (0.7 * (width - x) / FadeWidth), 0.3, 1);
        }
    }
}
