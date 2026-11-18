using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// Starts sign-ins for Settings › Sources and the Sources page: a plugin's own way to sign in
/// (BeginLogin) or a service from Add a Source (ConnectSource). The page shows each flow in the
/// sign-in dialog when <see cref="Requested"/> is raised.
/// </summary>
public sealed class SourceSignIns(ShellServices s)
{
    /// <summary>Raised when a sign-in should open (the app shows it in a dialog).</summary>
    public event Action<SignInViewModel>? Requested;

    /// <summary>Signs in to an installed source with one of its methods (BeginLogin).</summary>
    public void SignIn(PluginInfo plugin, AuthMethod method) => Start(new SignInViewModel(s, plugin, method));

    /// <summary>
    /// Connects to a service from Add a Source (Command.ConnectSource): the core sets the address,
    /// turns the plugin on and signs in as BeginLogin does, so the steps arrive as the plugin's Auth
    /// events and the sign-in dialog shows them as for the plugin's own Sign In. Nothing is sent
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
        if (s.Hub.State.Sources.Plugins.FirstOrDefault(p => p.Id == preset.PluginId) is not { } plugin)
        {
            return;
        }

        var start = new CoreCommand.ConnectSource(preset.Id, preset.NeedsAddress ? typed : null, method);
        Start(new SignInViewModel(s, plugin, method, preset.Name, start));
    }

    /// <summary>Fills <paramref name="target"/> with the services Add a Source offers, in the core's order.</summary>
    public static void Sync(ObservableCollection<SourcePresetViewModel> target, SourcesState sources, ILocalizer l)
    {
        target.Clear();
        foreach (var preset in sources.Presets)
        {
            target.Add(new SourcePresetViewModel(preset, sources.Plugins.FirstOrDefault(p => p.Id == preset.Plugin), l));
        }
    }

    private void Start(SignInViewModel flow)
    {
        Requested?.Invoke(flow);
        flow.Begin();
    }
}

/// <summary>
/// A service in Add a Source (SourcesState.Presets), on the Sources page and in Settings › Sources,
/// offered while its official plugin is installed: its name, what it is (or the account signed in),
/// the plugin's icon and the ways to sign in.
/// </summary>
public sealed class SourcePresetViewModel
{
    public SourcePresetViewModel(SourcePreset preset, PluginInfo? plugin, ILocalizer l)
    {
        Preset = preset;
        Description = DescriptionKey(preset.Id) is { } key ? l.Get(key) : string.Empty;
        AccountLine = preset.Account is { } account ? l.Format("Source_SignedInAs", account) : null;
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
        "qqmusic" => "SourcePreset_QqMusic",
        "spotify" => "SourcePreset_Spotify",
        _ => null,
    };
}

/// <summary>
/// "Connect to …" for a service in Add a Source: the Server Address (filled with the one set now,
/// the core's example as placeholder) when the service needs one, and a button per way to sign in,
/// password exchange as the primary one. Choosing a way only records it: the page connects once
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

/// <summary>An installed source in Your Sources: its name, badge and status; choosing it browses it.</summary>
public sealed class SourceChoice
{
    public SourceChoice(PluginInfo plugin, ILocalizer l)
    {
        Plugin = plugin;
        BadgeLabel = l.Get(plugin.Badge == PluginBadge.Official ? "Source_Official" : "Source_Community");
        Status = StatusOf(plugin, l);
        IconUri = plugin.Icon is { } icon ? FileUris.From(icon) : null;
    }

    public PluginInfo Plugin { get; }

    public string Id => Plugin.Id;

    public string Name => Plugin.Name;

    public bool IsOfficial => Plugin.Badge == PluginBadge.Official;

    /// <summary>"Official" or "Community · unofficial".</summary>
    public string BadgeLabel { get; }

    public string? Account => Plugin.Account;

    /// <summary>"Signed in as …", "Not signed in", or "Off" while the source is turned off.</summary>
    public string Status { get; }

    /// <summary>The plugin's icon as a URI, when it has one.</summary>
    public string? IconUri { get; }

    public bool HasIcon => IconUri is not null;

    /// <summary>Only a source that is on and has a catalogue can be browsed.</summary>
    public bool CanBrowse => Plugin.Enabled && Plugin.Capabilities.Contains("browse");

    /// <summary>"Navidrome, Official, Signed in as ana" for Narrator.</summary>
    public string AccessibleName => AlbumItem.Join(Name, BadgeLabel, Status);

    public override string ToString() => Name;

    /// <summary>A source's status line: off, the account it is signed in with, or not signed in (when it signs in at all).</summary>
    internal static string StatusOf(PluginInfo plugin, ILocalizer l) =>
        !plugin.Enabled ? l.Get("Source_StatusOff")
        : plugin.Account is { } account ? l.Format("Source_SignedInAs", account)
        : plugin.AuthMethods.Count > 0 ? l.Get("Source_NotSignedIn")
        : string.Empty;
}

/// <summary>
/// The Sources page: the intro, Add a Source (the core's presets), Your Sources (each installed
/// source with its status) and Sources Settings…. Choosing a source opens its catalogue on a page
/// of its own (<see cref="SourceBrowseViewModel"/>), as in the other apps.
/// </summary>
public sealed partial class SourcesViewModel : PageViewModel
{
    public SourcesViewModel(ShellServices services)
        : base(services)
    {
        SignIns = new SourceSignIns(services);
        SignIns.Requested += flow => SignInRequested?.Invoke(flow);
    }

    public SourceSignIns SignIns { get; }

    /// <summary>Raised when a sign-in from Add a Source should open (the app shows it in a dialog).</summary>
    public event Action<SignInViewModel>? SignInRequested;

    /// <summary>Raised when a source should open its catalogue (the app opens its page).</summary>
    public event Action<SourceTarget>? BrowseRequested;

    /// <summary>Add a Source: the services the core offers, in its order; empty hides the section.</summary>
    public ObservableCollection<SourcePresetViewModel> Presets { get; } = [];

    /// <summary>Your Sources: every installed source, on or off.</summary>
    public ObservableCollection<SourceChoice> Choices { get; } = [];

    /// <summary>Connects to a service from Add a Source; see <see cref="SourceSignIns.Connect"/>.</summary>
    public void Connect(SourcePresetViewModel preset, string? address, AuthMethod method) => SignIns.Connect(preset, address, method);

    protected override Task LoadCoreAsync()
    {
        Refresh(S.Hub.State.Sources);
        return Task.CompletedTask;
    }

    protected override void OnSlices(SliceChange changes, CoreSnapshot state)
    {
        if (changes.HasFlag(SliceChange.Sources))
        {
            Refresh(state.Sources);
        }
    }

    /// <summary>Choosing a source in Your Sources opens its catalogue (when it is on and has one).</summary>
    [RelayCommand]
    private void Open(SourceChoice? choice)
    {
        if (choice is { CanBrowse: true })
        {
            BrowseRequested?.Invoke(SourceTarget.Root(choice.Plugin));
        }
    }

    private void Refresh(SourcesState sources)
    {
        SourceSignIns.Sync(Presets, sources, S.L);
        Choices.Clear();
        foreach (var plugin in sources.Plugins)
        {
            Choices.Add(new SourceChoice(plugin, S.L));
        }

        IsEmpty = Choices.Count == 0;
    }
}
