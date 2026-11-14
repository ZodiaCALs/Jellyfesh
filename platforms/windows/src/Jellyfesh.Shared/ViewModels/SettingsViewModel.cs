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

/// <summary>One band of the graphic EQ.</summary>
public sealed partial class EqBandViewModel(int index, float frequencyHz, Action<EqBandViewModel> changed) : ObservableObject
{
    [ObservableProperty]
    private double _gainDb;

    public int Index { get; } = index;

    public float FrequencyHz { get; } = frequencyHz;

    public string Label => FrequencyHz >= 1000 ? $"{FrequencyHz / 1000:0.#}k" : $"{FrequencyHz:0}";

    internal bool Syncing { get; set; }

    partial void OnGainDbChanged(double value)
    {
        if (!Syncing)
        {
            changed(this);
        }
    }
}

/// <summary>An EQ preset: the gains jf-engine's presets use.</summary>
public sealed record EqPreset(string NameKey, float[] Gains)
{
    public static IReadOnlyList<EqPreset> All { get; } =
    [
        new("Eq_Flat", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        new("Eq_BassBoost", [6, 5, 4, 2, 0.5f, 0, 0, 0, 0, 0]),
        new("Eq_Vocal", [-2, -2, -1, 0, 1.5f, 3, 3, 2, 0, -1]),
        new("Eq_Treble", [0, 0, 0, 0, 0, 0, 1.5f, 3, 4.5f, 6]),
    ];
}

/// <summary>
/// Settings. Every change sends the core a modified copy of its Settings slice (or the dedicated
/// command for EQ, ReplayGain, crossfade and output); the slice coming back is the truth. While a
/// slider moves, echoes of older values are ignored for a moment so the thumb does not jump.
/// </summary>
public sealed partial class SettingsViewModel : PageViewModel
{
    private static readonly TimeSpan EchoGuard = TimeSpan.FromMilliseconds(700);

    private bool _syncing;
    private long _lastAudioEditMs = long.MinValue / 2;
    private long _lastLookEditMs = long.MinValue / 2;

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

    [ObservableProperty]
    private int _lyricsSizeIndex = 1;

    [ObservableProperty]
    private int _lyricsAlignIndex;

    [ObservableProperty]
    private AppIconChoice? _selectedIcon;

    // ---- lyrics ----
    [ObservableProperty]
    private bool _showTranslation;

    [ObservableProperty]
    private bool _showRomanization;

    // ---- playback ----
    [ObservableProperty]
    private bool _eqEnabled;

    [ObservableProperty]
    private double _eqPreampDb;

    [ObservableProperty]
    private int _replayGainIndex;

    [ObservableProperty]
    private double _replayGainPreampDb;

    [ObservableProperty]
    private bool _crossfadeEnabled;

    [ObservableProperty]
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

    // ---- sources ----
    [ObservableProperty]
    private bool _allowCommunity;

    [ObservableProperty]
    private bool _sourcesNotAvailable;

    public SettingsViewModel(ShellServices services, PlayerViewModel player, Func<string, string?> iconPreview)
        : base(services)
    {
        Player = player;
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

        for (var i = 0; i < EqSettings.PresetFrequencies.Length; i++)
        {
            Bands.Add(new EqBandViewModel(i, EqSettings.PresetFrequencies[i], _ => SendEq()));
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

    public ObservableCollection<EqBandViewModel> Bands { get; } = [];

    public IReadOnlyList<EqPreset> Presets => EqPreset.All;

    public ObservableCollection<string> Folders { get; } = [];

    public ObservableCollection<PluginViewModel> Plugins { get; } = [];

    /// <summary>The services Add a Source offers (SourcesState.Presets), in the core's order; empty hides the section.</summary>
    public ObservableCollection<SourcePresetViewModel> SourcePresets { get; } = [];

    /// <summary>Raised when a source's sign-in should open (the app shows it in a dialog).</summary>
    public event Action<SignInViewModel>? SignInRequested;

    protected override async Task LoadCoreAsync()
    {
        var roots = await S.QueryAsync(c => c.Roots());
        Folders.Clear();
        foreach (var r in roots.Value ?? [])
        {
            Folders.Add(r);
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

    /// <summary>The scan's progress line (from the hub's latest progress).</summary>
    public void UpdateScan(ScanProgress? progress)
    {
        IsScanning = progress is not null;
        ScanText = progress is null ? string.Empty : S.L.Format("Scan_Progress", progress.FilesSeen, progress.FilesChanged);
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

    partial void OnSelectedIconChanged(AppIconChoice? value)
    {
        if (value is not null)
        {
            Change(s => s with { AppIcon = value.Id });
        }
    }

    // ---- lyrics ----
    partial void OnShowTranslationChanged(bool value) => Change(s => s with { ShowTranslation = value });

    partial void OnShowRomanizationChanged(bool value) => Change(s => s with { ShowRomanization = value });

    // ---- playback ----
    partial void OnEqEnabledChanged(bool value) => SendEq();

    partial void OnEqPreampDbChanged(double value) => SendEq();

    [RelayCommand]
    private void ApplyPreset(EqPreset? preset)
    {
        if (preset is null)
        {
            return;
        }

        foreach (var band in Bands)
        {
            band.Syncing = true;
            band.GainDb = preset.Gains[band.Index];
            band.Syncing = false;
        }

        EqEnabled = true;
        SendEq();
    }

    partial void OnReplayGainIndexChanged(int value) => SendReplayGain();

    partial void OnReplayGainPreampDbChanged(double value) => SendReplayGain();

    partial void OnCrossfadeEnabledChanged(bool value) => SendCrossfade();

    partial void OnCrossfadeSecondsChanged(double value) => SendCrossfade();

    partial void OnAlbumContinuityChanged(bool value) => SendCrossfade();

    partial void OnResumeOnLaunchChanged(bool value) => Change(s => s with { ResumeOnLaunch = value });

    // ---- library ----
    partial void OnWatchFoldersChanged(bool value) => Change(s => s with { WatchFolders = value });

    [RelayCommand]
    private async Task AddFolder()
    {
        var folder = await S.Platform.PickFolderAsync();
        if (folder is not null)
        {
            S.Send(new CoreCommand.AddRoot(folder));
        }
    }

    [RelayCommand]
    private void RemoveFolder(string? folder)
    {
        if (folder is not null)
        {
            S.Send(new CoreCommand.RemoveRoot(folder));
            Folders.Remove(folder);
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

    [RelayCommand]
    private async Task InstallPlugin()
    {
        var path = await S.Platform.PickFileAsync([".jfpkg"]);
        if (path is not null)
        {
            S.Send(new CoreCommand.InstallPlugin(path));
        }
    }

    internal void RequestSignIn(PluginInfo plugin, AuthMethod method) => StartSignIn(new SignInViewModel(S, plugin, method));

    /// <summary>
    /// Connects to a service from Add a Source (Command.ConnectSource): the core sets the address,
    /// turns the plugin on and signs in as BeginLogin does, so the steps arrive as the plugin's Auth
    /// events and the sign-in dialog shows them as for the plugin's own Sign in. Nothing is sent
    /// while the service needs an address and none was typed.
    /// </summary>
    public void Connect(SourcePresetViewModel preset, string? address, AuthMethod method)
    {
        var typed = address?.Trim() ?? string.Empty;
        if (preset.NeedsAddress && typed.Length == 0)
        {
            return;
        }

        // A service is listed only while its plugin is installed.
        if (S.Hub.State.Sources.Plugins.FirstOrDefault(p => p.Id == preset.PluginId) is not { } plugin)
        {
            return;
        }

        var start = new CoreCommand.ConnectSource(preset.Id, preset.NeedsAddress ? typed : null, method);
        StartSignIn(new SignInViewModel(S, plugin, method, preset.Name, start));
    }

    private void StartSignIn(SignInViewModel flow)
    {
        SignInRequested?.Invoke(flow);
        flow.Begin();
    }

    private void Change(Func<AppSettings, AppSettings> edit)
    {
        if (_syncing)
        {
            return;
        }

        S.Send(new CoreCommand.SetSettings(edit(S.Hub.State.Settings)));
    }

    private void SendEq()
    {
        if (_syncing)
        {
            return;
        }

        _lastAudioEditMs = Environment.TickCount64;
        var bands = Bands.Select(b => new EqBand(EqBandKind.Peaking, b.FrequencyHz, (float)Math.Round(b.GainDb, 1), EqSettings.PresetQ, true)).ToArray();
        S.Send(new CoreCommand.SetEq(new EqSettings(EqEnabled, (float)Math.Round(EqPreampDb, 1), bands)));
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
                foreach (var band in Bands)
                {
                    band.Syncing = true;
                    band.GainDb = s.Eq.Bands.ElementAtOrDefault(band.Index)?.GainDb ?? 0;
                    band.Syncing = false;
                }

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

            SyncPlugins(state.Sources);
            SyncPresets(state.Sources);
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

    private void SyncPresets(SourcesState sources)
    {
        SourcePresets.Clear();
        foreach (var preset in sources.Presets)
        {
            SourcePresets.Add(new SourcePresetViewModel(preset, sources.Plugins.FirstOrDefault(p => p.Id == preset.Plugin), S.L));
        }
    }
}

/// <summary>One installed source in Settings → Sources.</summary>
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

    public string Details => Info is null ? string.Empty : s.L.Format("Source_Details", Info.Version, Info.Author);

    public bool IsOfficial => Info?.Badge == PluginBadge.Official;

    public string BadgeLabel => s.L.Get(IsOfficial ? "Source_Official" : "Source_Community");

    public string? Account => Info?.Account;

    public bool IsSignedIn => Info?.Account is not null;

    public bool CanSignIn => Info is { AuthMethods.Count: > 0 } && !IsSignedIn;

    public bool VendorPlayback => Info?.VendorPlayback ?? false;

    public IReadOnlyList<AuthMethod> AuthMethods => Info?.AuthMethods ?? [];

    public bool HasSettings => Form is { IsEmpty: false };

    internal void Update(PluginInfo info)
    {
        _syncing = true;
        Info = info;
        IsEnabled = info.Enabled;
        Form = PluginSettingsForm.Create(info.SettingsSchemaJson, info.SettingsJson);
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

    [RelayCommand]
    private void SignIn(AuthMethod? method)
    {
        if (Info is { } info && (method ?? info.AuthMethods.FirstOrDefault()) is var m && info.AuthMethods.Count > 0)
        {
            owner.RequestSignIn(info, m);
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

/// <summary>
/// A service in Settings → Sources → Add a Source (SourcesState.Presets), offered while its official
/// plugin is installed: its name, what it is (or the account signed in), the plugin's icon and the
/// ways to sign in.
/// </summary>
public sealed class SourcePresetViewModel
{
    public SourcePresetViewModel(SourcePreset preset, PluginInfo? plugin, ILocalizer l)
    {
        Preset = preset;
        Description = DescriptionKey(preset.Id) is { } key ? l.Get(key) : string.Empty;
        AccountLine = preset.Account is { } account ? l.Format("SignIn_Done", account) : null;
        IconPath = plugin?.Icon;
        IconUri = IconPath is null ? null : FileUris.From(IconPath);
        Methods = preset.AuthMethods.Select(m => SignInMethodChoice.For(m, l)).ToArray();
        ConnectTitle = l.Format("Connect_Title", preset.Name);
    }

    public SourcePreset Preset { get; }

    public string Id => Preset.Id;

    public string Name => Preset.Name;

    /// <summary>The plugin that connects to the service; the sign-in steps arrive as its Auth events.</summary>
    public string PluginId => Preset.Plugin;

    /// <summary>What the service is: "Your Jellyfin server".</summary>
    public string Description { get; }

    /// <summary>"Signed in as …" while the plugin has an account, otherwise null.</summary>
    public string? AccountLine { get; }

    /// <summary>The line under the name: the account when signed in, otherwise the description.</summary>
    public string Subtitle => AccountLine ?? Description;

    /// <summary>The icon of the plugin that connects to it (PluginInfo.Icon), when it has one.</summary>
    public string? IconPath { get; }

    /// <summary><see cref="IconPath"/> as a URI an image control loads.</summary>
    public string? IconUri { get; }

    /// <summary>False shows a generic glyph instead of the plugin's icon.</summary>
    public bool HasIcon => IconUri is not null;

    public IReadOnlyList<AuthMethod> AuthMethods => Preset.AuthMethods;

    /// <summary>The ways to sign in as buttons, in the plugin's order.</summary>
    public IReadOnlyList<SignInMethodChoice> Methods { get; }

    public bool NeedsAddress => Preset.NeedsAddress;

    public string? Address => Preset.Address;

    public string? AddressExample => Preset.AddressExample;

    /// <summary>"Connect to Navidrome".</summary>
    public string ConnectTitle { get; }

    public override string ToString() => Name;

    /// <summary>The string key of what a service is, by its stable id.</summary>
    private static string? DescriptionKey(string id) => id switch
    {
        "jellyfin" => "SourcePreset_Jellyfin",
        "navidrome" => "SourcePreset_Navidrome",
        "subsonic" => "SourcePreset_Subsonic",
        _ => null,
    };
}

/// <summary>
/// "Connect to …" for a service in Add a Source: the server's address (filled with the one set now)
/// when the service needs one, and a button per way to sign in, password exchange as the primary
/// one. Choosing a way only records it: the page calls <see cref="SettingsViewModel.Connect"/> once
/// this dialog has closed, so the sign-in dialog never opens over it.
/// </summary>
public sealed partial class ConnectSourceViewModel : ObservableObject
{
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanConnect))]
    private string _address = string.Empty;

    public ConnectSourceViewModel(SourcePresetViewModel preset)
    {
        Preset = preset;
        Address = preset.Address ?? string.Empty;
        Primary = preset.Methods.FirstOrDefault(m => m.Method == AuthMethod.PasswordExchange) ?? preset.Methods.FirstOrDefault();
        Others = preset.Methods.Where(m => m != Primary).ToArray();
    }

    public SourcePresetViewModel Preset { get; }

    public string Title => Preset.ConnectTitle;

    public bool NeedsAddress => Preset.NeedsAddress;

    public string AddressExample => Preset.AddressExample ?? string.Empty;

    /// <summary>Signing in waits for an address while the service needs one.</summary>
    public bool CanConnect => !NeedsAddress || !string.IsNullOrWhiteSpace(Address);

    /// <summary>The dialog's primary button: password exchange when offered, otherwise the first way.</summary>
    public SignInMethodChoice? Primary { get; }

    /// <summary>The other ways to sign in, as buttons in the dialog.</summary>
    public IReadOnlyList<SignInMethodChoice> Others { get; }

    public bool HasOthers => Others.Count > 0;

    /// <summary>The way the person chose; null while none was (or the dialog was cancelled).</summary>
    public AuthMethod? Chosen { get; private set; }

    /// <summary>Records <paramref name="method"/>; false, choosing nothing, while a needed address is empty.</summary>
    public bool Choose(AuthMethod method)
    {
        if (!CanConnect)
        {
            return false;
        }

        Chosen = method;
        return true;
    }
}
