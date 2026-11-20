using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Services;

namespace Jellyfesh.ViewModels;

/// <summary>How a purchase in the Store ended.</summary>
public enum ProPurchase
{
    /// <summary>Bought now (the Store's licence follows to the core).</summary>
    Bought,

    /// <summary>The person owned it already.</summary>
    AlreadyOwned,

    /// <summary>The person closed the Store's dialog.</summary>
    Cancelled,

    /// <summary>The Store could not be reached or failed.</summary>
    Failed,
}

/// <summary>
/// The Microsoft Store glue of a Store build (ADR-0021 §4): it reads the Pro add-on's price and
/// licence and reports the licence to the core with SetStoreEntitlement. Other builds have none.
/// </summary>
public interface IProStore
{
    /// <summary>The price to show ("4,99 €"), or null when the Store did not say.</summary>
    Task<string?> PriceAsync();

    /// <summary>Opens the Store's purchase dialog for Jellyfesh Pro.</summary>
    Task<ProPurchase> BuyAsync();

    /// <summary>Reads the Store's licences again and reports them to the core (Restore Purchases).</summary>
    Task RestoreAsync();
}

/// <summary>
/// Jellyfesh Pro as the Pro slice says (docs/ui.md, "AI Lyrics, Customize and Pro"): the status
/// line, Remove Licence… (asks first), Buy and Restore Purchases where this build's store sells it,
/// Enter Licence Key… where keys are taken, and the legal links. One per window: Settings ›
/// Extensions and the Pro sheet show the same.
/// </summary>
public sealed partial class ProViewModel : ObservableObject
{
    private readonly ShellServices _s;
    private readonly IProStore? _store;
    private bool _priceAsked;

    [ObservableProperty]
    private ProState _state = ProState.Initial;

    [ObservableProperty]
    private string? _price;

    [ObservableProperty]
    private bool _isBuying;

    public ProViewModel(ShellServices services, IProStore? store, AboutInfo.Links? links)
    {
        _s = services;
        _store = store;
        Links = links;
    }

    /// <summary>The website's legal pages, when the build knows the website.</summary>
    public AboutInfo.Links? Links { get; }

    public bool HasLinks => Links is not null;

    /// <summary>Raised when the Pro sheet should open (the app shows it as a dialog).</summary>
    public event Action? SheetRequested;

    /// <summary>Raised when Enter Licence Key should open, with its dialog's model.</summary>
    public event Action<LicenceKeyViewModel>? LicenceKeyRequested;

    public bool IsActive => State.Active;

    /// <summary>
    /// The status line: "Pro features are free during the public beta.", "Jellyfesh Pro ·
    /// Purchased", "Jellyfesh Pro · Licence …ABCD", or without Pro "AI Lyrics and Customize are
    /// part of Jellyfesh Pro."
    /// </summary>
    public string Status => State switch
    {
        { Source: ProSource.Preview } or { Preview: true } => _s.L.Get("Pro_Preview"),
        { Active: true, Source: ProSource.Purchased } => _s.L.Get("Pro_Purchased"),
        { Active: true, Source: ProSource.Licence l } => _s.L.Format("Pro_Licence", l.IdSuffix),
        _ => _s.L.Get("Pro_Inactive"),
    };

    /// <summary>Remove Licence…, for Pro from a licence key.</summary>
    public bool CanRemoveLicence => State.Source is ProSource.Licence;

    /// <summary>Buy and Restore Purchases: this build's store sells Pro and Pro is not active.</summary>
    public bool CanBuy => !State.Active && _store is not null && State.Stores.Contains(ProStore.MicrosoftStore);

    /// <summary>Enter Licence Key…, where this build takes keys and Pro is not active.</summary>
    public bool CanEnterKey => !State.Active && State.LicenceKeys;

    /// <summary>"Buy Jellyfesh Pro – 4,99 €", or without a price from the Store "Buy Jellyfesh Pro".</summary>
    public string BuyLabel => Price is { Length: > 0 } price ? _s.L.Format("Pro_Buy", price) : _s.L.Get("Pro_BuyNoPrice");

    /// <summary>The words for why the last licence key was refused (docs/ui.md).</summary>
    public static string? LicenceErrorText(LicenceError? error, ILocalizer l) => error switch
    {
        LicenceError.Malformed => l.Get("Licence_Malformed"),
        LicenceError.UnknownKey or LicenceError.BadSignature => l.Get("Licence_Invalid"),
        LicenceError.WrongProduct => l.Get("Licence_WrongProduct"),
        LicenceError.Revoked => l.Get("Licence_Revoked"),
        _ => null,
    };

    public void Update(ProState state)
    {
        State = state;
        OnPropertyChanged(string.Empty);
        if (CanBuy && !_priceAsked)
        {
            _priceAsked = true;
            _ = LoadPriceAsync();
        }
    }

    /// <summary>
    /// Runs <paramref name="action"/> when Pro is active, else opens the Pro sheet: what every Pro
    /// action does while Pro is inactive. True when the action ran.
    /// </summary>
    public bool Require(Action action)
    {
        if (State.Active)
        {
            action();
            return true;
        }

        SheetRequested?.Invoke();
        return false;
    }

    [RelayCommand]
    private void OpenSheet() => SheetRequested?.Invoke();

    /// <summary>Remove Licence… asks "Remove the Licence Key?" first.</summary>
    [RelayCommand]
    private async Task RemoveLicence()
    {
        if (await _s.Platform.ConfirmAsync(_s.L.Get("Pro_RemoveTitle"), _s.L.Get("Pro_RemoveBody"), _s.L.Get("Pro_RemoveAction"), _s.L.Get("Action_Cancel")))
        {
            _s.Send(new CoreCommand.RemoveLicence());
        }
    }

    [RelayCommand]
    private void EnterLicenceKey() => LicenceKeyRequested?.Invoke(new LicenceKeyViewModel(_s));

    [RelayCommand]
    private async Task Buy()
    {
        if (_store is null || IsBuying)
        {
            return;
        }

        IsBuying = true;
        try
        {
            var result = await _store.BuyAsync();
            if (result == ProPurchase.Failed)
            {
                _s.Notifications.Error(_s.L.Get("Pro_Title"), _s.L.Get("Pro_StoreFailed"));
            }
        }
        finally
        {
            IsBuying = false;
        }
    }

    [RelayCommand]
    private async Task Restore()
    {
        if (_store is not null)
        {
            await _store.RestoreAsync();
        }
    }

    [RelayCommand]
    private void OpenLicenceAgreement() => Open(Links?.LicenceAgreement);

    [RelayCommand]
    private void OpenPrivacyPolicy() => Open(Links?.PrivacyPolicy);

    [RelayCommand]
    private void OpenTermsOfSale() => Open(Links?.TermsOfSale);

    private void Open(Uri? uri)
    {
        if (uri is not null)
        {
            _s.Platform.OpenUrl(uri.AbsoluteUri);
        }
    }

    private async Task LoadPriceAsync()
    {
        try
        {
            Price = await _store!.PriceAsync();
            OnPropertyChanged(nameof(BuyLabel));
        }
        catch (Exception e) when (e is not OutOfMemoryException)
        {
            // No price: Buy Jellyfesh Pro without one.
            System.Diagnostics.Debug.WriteLine($"store price: {e.Message}");
        }
    }
}

/// <summary>
/// Enter Licence Key: the key ("JFP1-…"), Activate, and why the core refused it in the app's words.
/// The dialog closes once the Pro slice says a licence is active; the core decides everything.
/// </summary>
public sealed partial class LicenceKeyViewModel : ObservableObject
{
    private readonly ShellServices _s;
    private ulong? _sentAt;
    private ulong _request;

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(ActivateCommand))]
    private string _key = string.Empty;

    [ObservableProperty]
    private string? _error;

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(ActivateCommand))]
    private bool _isWorking;

    public LicenceKeyViewModel(ShellServices services)
    {
        _s = services;
        _s.Hub.Changed += OnChanged;
        _s.Hub.Notification += OnNotification;
    }

    /// <summary>Raised when the key was accepted: the dialog closes.</summary>
    public event Action? Activated;

    public bool CanActivate => Key.Trim().Length > 0 && !IsWorking;

    /// <summary>Stops listening (the dialog closed).</summary>
    public void Close()
    {
        _s.Hub.Changed -= OnChanged;
        _s.Hub.Notification -= OnNotification;
    }

    [RelayCommand(CanExecute = nameof(CanActivate))]
    private void Activate()
    {
        Error = null;
        IsWorking = true;
        _sentAt = _s.Hub.State.Pro.Version;
        _request = _s.SendHandled(new CoreCommand.ActivateLicence(Key.Trim()));
    }

    private void OnChanged(SliceChange changes)
    {
        if (!changes.HasFlag(SliceChange.Pro) || _sentAt is not { } before)
        {
            return;
        }

        var pro = _s.Hub.State.Pro;
        if (pro.Version <= before)
        {
            return;
        }

        if (pro.Active && pro.Source is ProSource.Licence)
        {
            IsWorking = false;
            _sentAt = null;
            Activated?.Invoke();
        }
        else if (ProViewModel.LicenceErrorText(pro.LicenceError, _s.L) is { } text)
        {
            IsWorking = false;
            _sentAt = null;
            Error = text;
        }
    }

    private void OnNotification(CoreEvent e)
    {
        if (e is CoreEvent.CommandFailed f && f.Id == _request && _sentAt is not null)
        {
            // The core could not take the key at all (in this release, maybe "not yet"): say so.
            IsWorking = false;
            _sentAt = null;
            Error = f.Message;
        }
    }
}

/// <summary>
/// Settings › About's updates (direct downloads; docs/ui.md, "About"): Check for Updates, Check for
/// Updates Automatically, and "Jellyfesh {version} Is Available" with View Release. Hidden where the
/// Update slice is not supported (store builds, builds from source).
/// </summary>
public sealed partial class UpdatesViewModel(ShellServices s) : ObservableObject
{
    private bool _syncing;

    [ObservableProperty]
    private UpdateState _state = UpdateState.Initial;

    [ObservableProperty]
    private bool _checkAutomatically = true;

    public bool IsSupported => State.Supported;

    public bool IsChecking => State.Checking;

    public bool HasUpdate => State.Available is not null;

    /// <summary>"Jellyfesh 0.0.8 Is Available".</summary>
    public string AvailableText => State.Available is { } a ? s.L.Format("Update_Available", a.Version) : string.Empty;

    /// <summary>The last check's problem, in the core's words.</summary>
    public string ErrorText => State.Error ?? string.Empty;

    public void Update(UpdateState state, AppSettings settings)
    {
        State = state;
        _syncing = true;
        CheckAutomatically = settings.CheckForUpdates;
        _syncing = false;
        OnPropertyChanged(string.Empty);
    }

    partial void OnCheckAutomaticallyChanged(bool value)
    {
        if (!_syncing && s.Hub.State.Settings.CheckForUpdates != value)
        {
            s.Send(new CoreCommand.SetSettings(s.Hub.State.Settings with { CheckForUpdates = value }));
        }
    }

    [RelayCommand]
    private void Check() => s.Send(new CoreCommand.CheckForUpdates());

    [RelayCommand]
    private void ViewRelease()
    {
        if (State.Available is { } a && ItemLinks.Usable(a.Url) is { } url)
        {
            s.Platform.OpenUrl(url);
        }
    }
}
