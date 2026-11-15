using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Appearance;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Settings;

namespace Jellyfesh.ViewModels;

/// <summary>An app icon in the picker; the app supplies the preview image's URI.</summary>
public sealed class AppIconChoice(string id, string name, string? previewUri, bool isAutomatic)
{
    public string Id { get; } = id;

    public string Name { get; } = name;

    public string? PreviewUri { get; } = previewUri;

    public bool IsAutomatic { get; } = isAutomatic;
}

/// <summary>An accent swatch: its hue and the accent it gives in each appearance.</summary>
public sealed class AccentSwatch(int index, float hue, string name)
{
    public int Index { get; } = index;

    public float Hue { get; } = hue;

    /// <summary>For Narrator: "Hue 250".</summary>
    public string Name { get; } = name;

    public Rgba Light { get; } = AccentChoices.Accent(hue, dark: false);

    public Rgba Dark { get; } = AccentChoices.Accent(hue, dark: true);
}

/// <summary>One band of the equalizer, as the core's settings have it (kind, frequency, Q); the slider moves its gain.</summary>
public sealed partial class EqBandViewModel(int index, EqBand band, Action<EqBandViewModel> changed) : ObservableObject
{
    [ObservableProperty]
    private double _gainDb = band.GainDb;

    public int Index { get; } = index;

    /// <summary>The core's band; a change sends it back with the slider's gain.</summary>
    public EqBand Band { get; } = band;

    public float FrequencyHz => Band.FreqHz;

    public string Label => FrequencyHz >= 1000 ? $"{FrequencyHz / 1000:0.#}k" : $"{FrequencyHz:0}";

    internal bool Syncing { get; set; }

    /// <summary>Whether <paramref name="other"/> is this band apart from its gain.</summary>
    internal bool SameShape(EqBand other) =>
        Band.Kind == other.Kind && Band.FreqHz.Equals(other.FreqHz) && Band.Q.Equals(other.Q) && Band.Enabled == other.Enabled;

    partial void OnGainDbChanged(double value)
    {
        if (!Syncing)
        {
            changed(this);
        }
    }
}

/// <summary>
/// A choice in the equalizer's Preset menu: one of the core's presets (<see cref="ICoreClient.EqPresets"/>),
/// or Custom, which the menu lists only once a band no longer matches a preset.
/// </summary>
public sealed class EqPresetChoice
{
    private EqPresetChoice(string id, string name, EqSettings? settings)
    {
        Id = id;
        Name = name;
        Settings = settings;
    }

    public string Id { get; }

    /// <summary>"Flat", "Bass Boost", "Vocal", "Treble" or "Custom", translated.</summary>
    public string Name { get; }

    /// <summary>The core's settings for the preset; null for Custom.</summary>
    public EqSettings? Settings { get; }

    public bool IsCustom => Settings is null;

    public static EqPresetChoice For(EqPreset preset, ILocalizer l) =>
        new(preset.Id, NameKey(preset.Id) is { } key ? l.Get(key) : preset.Id, preset.Settings);

    public static EqPresetChoice Custom(ILocalizer l) => new("custom", l.Get("Eq_Custom"), null);

    public override string ToString() => Name;

    /// <summary>The string key of a preset's name, by the core's stable id.</summary>
    private static string? NameKey(string id) => id switch
    {
        "flat" => "Eq_Flat",
        "bass_boost" => "Eq_BassBoost",
        "vocal" => "Eq_Vocal",
        "treble" => "Eq_Treble",
        _ => null,
    };
}

/// <summary>A music folder in Settings › Library: where it is and the name its row and Remove Folder… show.</summary>
public sealed record FolderItem(string Path)
{
    /// <summary>The folder's own name ("Music"), or the whole path for a drive.</summary>
    public string Name { get; } = NameOf(Path);

    private static string NameOf(string path)
    {
        var name = System.IO.Path.GetFileName(path.TrimEnd('/', '\\'));
        return string.IsNullOrEmpty(name) ? path : name;
    }
}

/// <summary>
/// Settings: Appearance, Playback, Library, Sources and About (docs/ui.md, "Settings"). Every change
/// sends the core a modified copy of its Settings slice (or the dedicated command for EQ,
/// ReplayGain, crossfade and output); the slice coming back is the truth. While a slider moves,
/// echoes of older values are ignored for a moment so the thumb does not jump.
/// </summary>
public sealed partial class SettingsViewModel : PageViewModel
{
    private static readonly TimeSpan EchoGuard = TimeSpan.FromMilliseconds(700);

    private bool _syncing;
    private long _lastAudioEditMs = long.MinValue / 2;
    private long _lastLookEditMs = long.MinValue / 2;
    private IReadOnlyList<EqPreset> _eqPresets = [];
    private EqPresetChoice? _custom;

    // ---- appearance ----
    [ObservableProperty]
    private int _appearanceIndex;

    [ObservableProperty]
    private int _styleIndex;

    [ObservableProperty]
    private bool _hueFromArtwork = true;

    [ObservableProperty]
    private double _fixedHue = FallbackTheme.Hue;

    /// <summary>The chosen swatch, or -1 for the artwork's hue or a custom one.</summary>
    [ObservableProperty]
    private int _swatchIndex = -1;

    /// <summary>How see-through glass is, 0 (solid) to 100 (clearest); 50 keeps the tokens' alphas.</summary>
    [ObservableProperty]
    private double _transparency = AppSettings.DefaultTransparency * 100;

    [ObservableProperty]
    private bool _reduceEffects;

    // Appearance › Lyrics: the only place the lyrics settings live.
    [ObservableProperty]
    private int _lyricsSizeIndex = 1;

    [ObservableProperty]
    private int _lyricsAlignIndex;

    [ObservableProperty]
    private bool _showTranslation;

    [ObservableProperty]
    private bool _showRomanization;

    [ObservableProperty]
    private AppIconChoice? _selectedIcon;

    // ---- playback ----
    [ObservableProperty]
    private bool _eqEnabled;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(EqPreampText))]
    private double _eqPreampDb;

    /// <summary>The Preset menu's choice: the core's preset the bands match, or Custom.</summary>
    [ObservableProperty]
    private EqPresetChoice? _selectedPreset;

    [ObservableProperty]
    private int _replayGainIndex;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ReplayGainPreampText))]
    private double _replayGainPreampDb;

    [ObservableProperty]
    private bool _crossfadeEnabled;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CrossfadeText))]
    private double _crossfadeSeconds = 6;

    [ObservableProperty]
    private bool _albumContinuity = true;

    [ObservableProperty]
    private bool _resumeOnLaunch;

    // ---- library ----
    [ObservableProperty]
    private bool _watchFolders;

    [ObservableProperty]
    private string _libraryStats = string.Empty;

    [ObservableProperty]
    private bool _isScanning;

    [ObservableProperty]
    private string _scanText = string.Empty;

    /// <summary>How this session's last scan ended ("12 added, 3 updated, 0 removed, 0 problems"); empty before one.</summary>
    [ObservableProperty]
    private string _lastScanText = string.Empty;

    // ---- sources ----
    [ObservableProperty]
    private bool _allowCommunity;

    public SettingsViewModel(ShellServices services, PlayerViewModel player, Func<string, string?> iconPreview)
        : base(services)
    {
        Player = player;
        SignIns = new SourceSignIns(services);
        SignIns.Requested += flow => SignInRequested?.Invoke(flow);
        Icons.Add(new AppIconChoice(AppIcons.Automatic, services.L.Get("Icon_Automatic"), iconPreview(AppIcons.Automatic), true));
        foreach (var option in AppIcons.All)
        {
            Icons.Add(new AppIconChoice(option.Id, services.L.Get(option.NameKey), iconPreview(option.Id), false));
        }

        for (var i = 0; i < AccentChoices.SwatchHues.Count; i++)
        {
            var hue = AccentChoices.SwatchHues[i];
            Swatches.Add(new AccentSwatch(i, hue, services.L.Format("Appearance_SwatchName", hue)));
        }

        Apply(services.Hub.State);
    }

    /// <summary>Output devices live on the player (the same list as Now Playing's picker).</summary>
    public PlayerViewModel Player { get; }

    public ObservableCollection<AppIconChoice> Icons { get; } = [];

    /// <summary>The ten accent swatches (AccentChoices.SwatchHues).</summary>
    public ObservableCollection<AccentSwatch> Swatches { get; } = [];

    /// <summary>"50 %": the transparency slider's value for Narrator and the label beside it.</summary>
    public string TransparencyText => S.L.Format("Appearance_TransparencyValue", (int)Math.Round(Transparency));

    /// <summary>"Hue 212": the custom hue slider's value.</summary>
    public string HueText => S.L.Format("Appearance_SwatchName", (int)Math.Round(FixedHue));

    /// <summary>The bands of the core's equalizer settings, in its order.</summary>
    public ObservableCollection<EqBandViewModel> Bands { get; } = [];

    /// <summary>The Preset menu: the core's presets (eq_presets) in its order, then Custom while the bands match none.</summary>
    public ObservableCollection<EqPresetChoice> Presets { get; } = [];

    /// <summary>"6 s": the crossfade length beside its slider.</summary>
    public string CrossfadeText => S.L.Format("Playback_SecondsValue", (int)Math.Round(CrossfadeSeconds));

    /// <summary>"+1.5 dB": Volume Levelling's preamp beside its slider.</summary>
    public string ReplayGainPreampText => Decibels(ReplayGainPreampDb);

    /// <summary>"−2.0 dB": the equalizer's preamp beside its slider.</summary>
    public string EqPreampText => Decibels(EqPreampDb);

    public ObservableCollection<FolderItem> Folders { get; } = [];

    public ObservableCollection<PluginViewModel> Plugins { get; } = [];

    /// <summary>The services Add a Source offers (SourcesState.Presets), in the core's order; empty hides the section.</summary>
    public ObservableCollection<SourcePresetViewModel> SourcePresets { get; } = [];

    public SourceSignIns SignIns { get; }

    /// <summary>Raised when a source's sign-in should open (the app shows it in a dialog).</summary>
    public event Action<SignInViewModel>? SignInRequested;

    protected override async Task LoadCoreAsync()
    {
        if (_eqPresets.Count == 0)
        {
            var presets = await S.QueryAsync(c => c.EqPresets());
            if (presets.IsOk && presets.Value!.Count > 0)
            {
                _eqPresets = presets.Value!;
                Presets.Clear();
                foreach (var preset in _eqPresets)
                {
                    Presets.Add(EqPresetChoice.For(preset, S.L));
                }

                UpdatePresetChoice();
            }
        }

        var roots = await S.QueryAsync(c => c.Roots());
        Folders.Clear();
        foreach (var r in roots.Value ?? [])
        {
            Folders.Add(new FolderItem(r));
        }

        var stats = await S.QueryAsync(c => c.Stats());
        if (stats.IsOk)
        {
            var st = stats.Value!;
            LibraryStats = S.L.Format(
                "Library_StatsLine",
                S.L.Plural("Count_Songs", (long)st.Tracks),
                S.L.Plural("Count_Albums", (long)st.Albums),
                S.L.Plural("Count_Artists", (long)st.Artists),
                TimeFormat.Total(st.TotalDurationMs, S.L));
        }
    }

    protected override void OnSlices(SliceChange changes, CoreSnapshot state)
    {
        if (changes.HasFlag(SliceChange.Settings) || changes.HasFlag(SliceChange.Sources) || changes.HasFlag(SliceChange.Scan))
        {
            Apply(state);
        }

        if (changes.HasFlag(SliceChange.Scan) && !state.Scanning)
        {
            _ = LoadAsync(); // a finished scan changes folders and counts
        }
    }

    /// <summary>The scan's progress line (from the hub's latest progress) and how the last scan ended.</summary>
    public void UpdateScan(ScanProgress? progress)
    {
        IsScanning = progress is not null;
        ScanText = progress is null ? string.Empty : S.L.Format("Scan_Progress", progress.FilesSeen, progress.FilesChanged);
        LastScanText = S.Hub.LastScan switch
        {
            ScanStatus.Finished f => S.L.Format("Scan_Report", f.Report.Added, f.Report.Updated, f.Report.Removed, f.Report.Errors.Count),
            ScanStatus.Failed x => x.Message,
            _ => string.Empty,
        };
    }

    // ---- appearance ----
    partial void OnAppearanceIndexChanged(int value) => Change(s => s with
    {
        Appearance = value switch
        {
            1 => AppearancePreference.Light,
            2 => AppearancePreference.Dark,
            _ => AppearancePreference.System,
        },
    });

    partial void OnStyleIndexChanged(int value) => Change(s => s with { Style = value == 1 ? BackdropStyle.Misty : BackdropStyle.Artwork });

    partial void OnHueFromArtworkChanged(bool value) => Change(s => s with
    {
        HueSource = value ? new HueSource.Artwork() : new HueSource.Fixed((float)FixedHue),
    });

    partial void OnFixedHueChanged(double value)
    {
        OnPropertyChanged(nameof(HueText));
        if (!HueFromArtwork && !_syncing)
        {
            var hue = AccentChoices.Normalize((float)value);
            _lastLookEditMs = Environment.TickCount64;
            SwatchIndex = AccentChoices.SwatchIndex(new HueSource.Fixed(hue));
            Change(s => s with { HueSource = new HueSource.Fixed(hue) });
        }
    }

    /// <summary>A swatch was chosen: its hue becomes the fixed accent hue.</summary>
    [RelayCommand]
    private void ChooseSwatch(AccentSwatch? swatch)
    {
        if (swatch is null)
        {
            return;
        }

        _syncing = true;
        try
        {
            HueFromArtwork = false;
            FixedHue = swatch.Hue;
            SwatchIndex = swatch.Index;
        }
        finally
        {
            _syncing = false;
        }

        Change(s => s with { HueSource = new HueSource.Fixed(swatch.Hue) });
    }

    partial void OnTransparencyChanged(double value)
    {
        OnPropertyChanged(nameof(TransparencyText));
        if (!_syncing)
        {
            _lastLookEditMs = Environment.TickCount64;
            Change(s => s with { Transparency = (float)Math.Clamp(value / 100, 0, 1) });
        }
    }

    partial void OnReduceEffectsChanged(bool value) => Change(s => s with { ReduceEffects = value });

    partial void OnLyricsSizeIndexChanged(int value) => Change(s => s with
    {
        LyricsSize = value switch
        {
            0 => LyricsSize.Small,
            2 => LyricsSize.Large,
            _ => LyricsSize.Medium,
        },
    });

    partial void OnLyricsAlignIndexChanged(int value) => Change(s => s with { LyricsAlign = value == 1 ? LyricsAlign.Center : LyricsAlign.Start });

    partial void OnShowTranslationChanged(bool value) => Change(s => s with { ShowTranslation = value });

    partial void OnShowRomanizationChanged(bool value) => Change(s => s with { ShowRomanization = value });

    partial void OnSelectedIconChanged(AppIconChoice? value)
    {
        if (value is not null)
        {
            Change(s => s with { AppIcon = value.Id });
        }
    }

    // ---- playback ----
    partial void OnEqEnabledChanged(bool value) => SendEq();

    partial void OnEqPreampDbChanged(double value) => SendEq();

    /// <summary>
    /// A preset chosen in the menu: its bands and preamp (the core's settings) with the switch as it
    /// is. Custom is only a label for bands that match no preset, so choosing it changes nothing.
    /// </summary>
    partial void OnSelectedPresetChanged(EqPresetChoice? value)
    {
        if (!_syncing && value?.Settings is { } preset)
        {
            ApplyEq(preset);
        }
    }

    /// <summary>Reset to Flat: the core's flat preset (every gain and the preamp at 0 dB), the switch as it is.</summary>
    [RelayCommand]
    private void ResetToFlat()
    {
        var flat = _eqPresets.FirstOrDefault(p => p.Id == "flat")?.Settings
            ?? CurrentEq() with { PreampDb = 0f, Bands = Bands.Select(b => b.Band with { GainDb = 0f }).ToArray() };
        ApplyEq(flat);
        UpdatePresetChoice();
    }

    partial void OnReplayGainIndexChanged(int value) => SendReplayGain();

    partial void OnReplayGainPreampDbChanged(double value) => SendReplayGain();

    partial void OnCrossfadeEnabledChanged(bool value) => SendCrossfade();

    partial void OnCrossfadeSecondsChanged(double value) => SendCrossfade();

    partial void OnAlbumContinuityChanged(bool value) => SendCrossfade();

    partial void OnResumeOnLaunchChanged(bool value) => Change(s => s with { ResumeOnLaunch = value });

    // ---- library ----
    partial void OnWatchFoldersChanged(bool value) => Change(s => s with { WatchFolders = value });

    /// <summary>Add Folder…: a folder the person picks joins the library.</summary>
    [RelayCommand]
    private Task AddFolder() => LibraryActions.AddFolderAsync(S);

    /// <summary>Remove Folder… asks first: its songs leave the library and its playlists; the files stay.</summary>
    [RelayCommand]
    private async Task RemoveFolder(FolderItem? folder)
    {
        if (folder is not null && await S.Platform.ConfirmAsync(
                S.L.Format("Library_RemoveFolderTitle", folder.Name), S.L.Get("Library_RemoveFolderBody"), S.L.Get("Action_RemoveFolder"), S.L.Get("Action_Cancel")))
        {
            S.Send(new CoreCommand.RemoveRoot(folder.Path));
            Folders.Remove(folder);
        }
    }

    /// <summary>Show in Explorer for a music folder.</summary>
    [RelayCommand]
    private void ShowFolder(FolderItem? folder)
    {
        if (folder is not null)
        {
            S.Platform.ShowInExplorer(folder.Path);
        }
    }

    [RelayCommand]
    private void Rescan() => S.Send(new CoreCommand.Rescan());

    // ---- sources ----
    partial void OnAllowCommunityChanged(bool value)
    {
        if (_syncing)
        {
            return;
        }

        if (!value)
        {
            Change(s => s with { AllowCommunityPlugins = false });
            return;
        }

        _ = ConfirmCommunityAsync();
    }

    private async Task ConfirmCommunityAsync()
    {
        if (await S.Platform.ConfirmAsync(S.L.Get("Community_Title"), S.L.Get("Community_Disclosure"), S.L.Get("Community_Accept"), S.L.Get("Action_Cancel")))
        {
            Change(s => s with { AllowCommunityPlugins = true });
        }
        else
        {
            _syncing = true;
            AllowCommunity = false;
            _syncing = false;
        }
    }

    /// <summary>Install from File…: a .jfpkg the person picks.</summary>
    [RelayCommand]
    private async Task InstallPlugin()
    {
        var path = await S.Platform.PickFileAsync([".jfpkg"]);
        if (path is not null)
        {
            S.Send(new CoreCommand.InstallPlugin(path));
        }
    }

    internal void RequestSignIn(PluginInfo plugin, AuthMethod method) => SignIns.SignIn(plugin, method);

    /// <summary>Connects to a service from Add a Source; see <see cref="SourceSignIns.Connect"/>.</summary>
    public void Connect(SourcePresetViewModel preset, string? address, AuthMethod method) => SignIns.Connect(preset, address, method);

    private void Change(Func<AppSettings, AppSettings> edit)
    {
        if (_syncing)
        {
            return;
        }

        S.Send(new CoreCommand.SetSettings(edit(S.Hub.State.Settings)));
    }

    private string Decibels(double db) =>
        S.L.Format("Playback_DecibelValue", Math.Round(db, 1).ToString("+0.0;−0.0;0.0", S.L.Culture));

    /// <summary>The equalizer as the page shows it: the switch, the preamp and the core's bands with the sliders' gains.</summary>
    private EqSettings CurrentEq() => new(
        EqEnabled,
        (float)Math.Round(EqPreampDb, 1),
        Bands.Select(b => b.Band with { GainDb = (float)Math.Round(b.GainDb, 1) }).ToArray());

    private void OnBandChanged(EqBandViewModel band)
    {
        SendEq();
        UpdatePresetChoice();
    }

    private void SendEq()
    {
        if (_syncing)
        {
            return;
        }

        _lastAudioEditMs = Environment.TickCount64;
        S.Send(new CoreCommand.SetEq(CurrentEq()));
    }

    /// <summary>Shows <paramref name="eq"/>'s gains and preamp and sends it with the switch as it is.</summary>
    private void ApplyEq(EqSettings eq)
    {
        _syncing = true;
        try
        {
            EqPreampDb = eq.PreampDb;
            if (Bands.Count == eq.Bands.Count)
            {
                foreach (var band in Bands)
                {
                    band.Syncing = true;
                    band.GainDb = eq.Bands[band.Index].GainDb;
                    band.Syncing = false;
                }
            }
            else
            {
                SyncBands(eq);
            }
        }
        finally
        {
            _syncing = false;
        }

        _lastAudioEditMs = Environment.TickCount64;
        S.Send(new CoreCommand.SetEq(eq with { Enabled = EqEnabled }));
    }

    /// <summary>
    /// The Preset menu follows the bands: the core's preset whose gains they have, else Custom, which
    /// is listed only then. Never called while the menu changes its own selection.
    /// </summary>
    private void UpdatePresetChoice()
    {
        if (_eqPresets.Count == 0)
        {
            return;
        }

        var current = CurrentEq();
        var match = Presets.FirstOrDefault(p => p.Settings is { } s && current.SameGains(s));
        var wasSyncing = _syncing;
        _syncing = true;
        try
        {
            if (match is not null)
            {
                SelectedPreset = match;
                if (_custom is not null)
                {
                    Presets.Remove(_custom);
                }
            }
            else
            {
                _custom ??= EqPresetChoice.Custom(S.L);
                if (!Presets.Contains(_custom))
                {
                    Presets.Add(_custom);
                }

                SelectedPreset = _custom;
            }
        }
        finally
        {
            _syncing = wasSyncing;
        }
    }

    /// <summary>The sliders take the core's bands: new ones when the layout changed, else only their gains.</summary>
    private void SyncBands(EqSettings eq)
    {
        if (Bands.Count != eq.Bands.Count || Bands.Zip(eq.Bands).Any(p => !p.First.SameShape(p.Second)))
        {
            Bands.Clear();
            for (var i = 0; i < eq.Bands.Count; i++)
            {
                Bands.Add(new EqBandViewModel(i, eq.Bands[i], OnBandChanged));
            }

            return;
        }

        foreach (var band in Bands)
        {
            band.Syncing = true;
            band.GainDb = eq.Bands[band.Index].GainDb;
            band.Syncing = false;
        }
    }

    private void SendReplayGain()
    {
        if (_syncing)
        {
            return;
        }

        _lastAudioEditMs = Environment.TickCount64;
        var mode = ReplayGainIndex switch
        {
            1 => ReplayGainMode.Track,
            2 => ReplayGainMode.Album,
            _ => ReplayGainMode.Off,
        };
        S.Send(new CoreCommand.SetReplayGain(mode, (float)Math.Round(ReplayGainPreampDb, 1)));
    }

    private void SendCrossfade()
    {
        if (_syncing)
        {
            return;
        }

        _lastAudioEditMs = Environment.TickCount64;
        var ms = (uint)Math.Clamp(Math.Round(CrossfadeSeconds) * 1_000, CrossfadeSettings.MinDurationMs, CrossfadeSettings.MaxDurationMs);
        S.Send(new CoreCommand.SetCrossfade(new CrossfadeSettings(CrossfadeEnabled, ms, AlbumContinuity)));
    }

    private void Apply(CoreSnapshot state)
    {
        var s = state.Settings;
        _syncing = true;
        try
        {
            AppearanceIndex = s.Appearance switch
            {
                AppearancePreference.Light => 1,
                AppearancePreference.Dark => 2,
                _ => 0,
            };
            StyleIndex = s.Style == BackdropStyle.Misty ? 1 : 0;
            HueFromArtwork = s.HueSource is HueSource.Artwork;
            var lookEchoes = Environment.TickCount64 - _lastLookEditMs <= EchoGuard.TotalMilliseconds;
            if (s.HueSource is HueSource.Fixed f && !lookEchoes)
            {
                FixedHue = f.Hue;
            }

            SwatchIndex = AccentChoices.SwatchIndex(s.HueSource);
            if (!lookEchoes)
            {
                Transparency = Math.Round(Math.Clamp(s.Transparency, 0, 1) * 100);
            }

            ReduceEffects = s.ReduceEffects;
            LyricsSizeIndex = s.LyricsSize switch
            {
                LyricsSize.Small => 0,
                LyricsSize.Large => 2,
                _ => 1,
            };
            LyricsAlignIndex = s.LyricsAlign == LyricsAlign.Center ? 1 : 0;
            SelectedIcon = Icons.FirstOrDefault(i => i.Id == s.AppIcon) ?? Icons[0];
            ShowTranslation = s.ShowTranslation;
            ShowRomanization = s.ShowRomanization;
            ResumeOnLaunch = s.ResumeOnLaunch;
            WatchFolders = s.WatchFolders;
            AllowCommunity = s.AllowCommunityPlugins;
            if (Environment.TickCount64 - _lastAudioEditMs > EchoGuard.TotalMilliseconds)
            {
                EqEnabled = s.Eq.Enabled;
                EqPreampDb = s.Eq.PreampDb;
                SyncBands(s.Eq);
                ReplayGainIndex = s.ReplayGainMode switch
                {
                    ReplayGainMode.Track => 1,
                    ReplayGainMode.Album => 2,
                    _ => 0,
                };
                ReplayGainPreampDb = s.PreampDb;
                CrossfadeEnabled = s.Crossfade.Enabled;
                CrossfadeSeconds = s.Crossfade.DurationMs / 1000.0;
                AlbumContinuity = s.Crossfade.AlbumContinuity;
            }

            UpdatePresetChoice();
            SyncPlugins(state.Sources);
            SourceSignIns.Sync(SourcePresets, state.Sources, S.L);
        }
        finally
        {
            _syncing = false;
        }

        UpdateScan(S.Hub.ScanProgress);
    }

    private void SyncPlugins(SourcesState sources)
    {
        var byId = Plugins.ToDictionary(p => p.Id);
        Plugins.Clear();
        foreach (var info in sources.Plugins)
        {
            var vm = byId.TryGetValue(info.Id, out var existing) ? existing : new PluginViewModel(S, this);
            vm.Update(info);
            Plugins.Add(vm);
        }
    }
}

/// <summary>A sign-in button of an installed source: the method's title ("Sign In with QR Code") and the command that starts it.</summary>
public sealed class SignInAction(SignInMethodChoice choice, Action start)
{
    public string Title => choice.Title;

    public AuthMethod Method => choice.Method;

    public IRelayCommand Command { get; } = new RelayCommand(start);

    public override string ToString() => Title;
}

/// <summary>
/// One installed source in Settings › Sources › Installed: its name, badge and status; Enabled, a
/// sign-in button for each method it offers (or Sign Out), its own settings, "Version · Author" and
/// Uninstall…, which asks first.
/// </summary>
public sealed partial class PluginViewModel(ShellServices s, SettingsViewModel owner) : ObservableObject
{
    private bool _syncing;

    [ObservableProperty]
    private bool _isEnabled;

    [ObservableProperty]
    private PluginInfo? _info;

    [ObservableProperty]
    private PluginSettingsForm? _form;

    public string Id => Info?.Id ?? string.Empty;

    public string Name => Info?.Name ?? string.Empty;

    /// <summary>"Version 1.2.0 · Author".</summary>
    public string Details => Info is null ? string.Empty : s.L.Format("Source_Details", Info.Version, Info.Author);

    public bool IsOfficial => Info?.Badge == PluginBadge.Official;

    /// <summary>"Official" or "Community · unofficial".</summary>
    public string BadgeLabel => s.L.Get(IsOfficial ? "Source_Official" : "Source_Community");

    public string? Account => Info?.Account;

    /// <summary>"Signed in as …" or "Not signed in"; empty for a source that never signs in.</summary>
    public string Status => Info switch
    {
        { Account: { } account } => s.L.Format("Source_SignedInAs", account),
        { AuthMethods.Count: > 0 } => s.L.Get("Source_NotSignedIn"),
        _ => string.Empty,
    };

    public bool IsSignedIn => Info?.Account is not null;

    public bool CanSignIn => Info is { AuthMethods.Count: > 0 } && !IsSignedIn;

    public bool VendorPlayback => Info?.VendorPlayback ?? false;

    public IReadOnlyList<AuthMethod> AuthMethods => Info?.AuthMethods ?? [];

    /// <summary>A button for each way the source signs in, in its order, with the shared labels.</summary>
    public IReadOnlyList<SignInAction> SignInActions { get; private set; } = [];

    public bool HasSettings => Form is { IsEmpty: false };

    internal void Update(PluginInfo info)
    {
        _syncing = true;
        Info = info;
        IsEnabled = info.Enabled;
        Form = PluginSettingsForm.Create(info.SettingsSchemaJson, info.SettingsJson);
        SignInActions = info.AuthMethods.Select(m => new SignInAction(SignInMethodChoice.For(m, s.L), () => SignIn(m))).ToArray();
        _syncing = false;
        OnPropertyChanged(string.Empty);
    }

    partial void OnIsEnabledChanged(bool value)
    {
        if (_syncing || Info is null)
        {
            return;
        }

        if (value && Info.Badge == PluginBadge.Community && Info.NeedsDisclosure)
        {
            _ = EnableCommunityAsync(Info);
            return;
        }

        s.Send(new CoreCommand.SetPluginEnabled(Info.Id, value));
    }

    private async Task EnableCommunityAsync(PluginInfo info)
    {
        if (await s.Platform.ConfirmAsync(s.L.Format("Community_PluginTitle", info.Name), s.L.Get("Community_Disclosure"), s.L.Get("Community_Accept"), s.L.Get("Action_Cancel")))
        {
            s.Send(new CoreCommand.AcknowledgeUnofficial(info.Id));
            s.Send(new CoreCommand.SetPluginEnabled(info.Id, true));
        }
        else
        {
            _syncing = true;
            IsEnabled = false;
            _syncing = false;
        }
    }

    /// <summary>Signs in with <paramref name="method"/>, one of the methods the source offers.</summary>
    [RelayCommand]
    private void SignIn(AuthMethod method)
    {
        if (Info is { } info && info.AuthMethods.Contains(method))
        {
            owner.RequestSignIn(info, method);
        }
    }

    [RelayCommand]
    private void SignOut()
    {
        if (Info is { } info)
        {
            s.Send(new CoreCommand.Logout(info.Id));
        }
    }

    [RelayCommand]
    private void SaveSettings()
    {
        if (Info is { } info && Form is { } form && form.Validate())
        {
            s.Send(new CoreCommand.SetPluginSettings(info.Id, form.ToJson()));
        }
    }

    /// <summary>Uninstall… asks "Uninstall Name?" first: its settings and sign-in are removed.</summary>
    [RelayCommand]
    private async Task Uninstall()
    {
        if (Info is { } info && await s.Platform.ConfirmAsync(
                s.L.Format("Source_UninstallTitle", info.Name), s.L.Get("Source_UninstallBody"), s.L.Get("Action_Uninstall"), s.L.Get("Action_Cancel")))
        {
            s.Send(new CoreCommand.UninstallPlugin(info.Id));
        }
    }
}
