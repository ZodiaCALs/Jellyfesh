using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Qr;

namespace Jellyfesh.ViewModels;

public enum SignInStage
{
    Starting,
    ShowQr,
    OpenUrl,
    AskFields,
    Waiting,
    Done,
    Failed,
}

/// <summary>A field the source asks for; secrets go to the core once and are never stored (ADR-0015).</summary>
public sealed partial class SignInField(AuthField field) : ObservableObject
{
    [ObservableProperty]
    private string _value = string.Empty;

    public string Name { get; } = field.Name;

    public string Label { get; } = field.Label;

    public bool IsSecret { get; } = field.Secret;

    public string Placeholder { get; } = field.Placeholder ?? string.Empty;
}

/// <summary>
/// One sign-in flow, rendered natively from the core's steps (architecture §10): a QR code to
/// scan, a page to open in the browser, fields to fill, or a wait.
/// </summary>
public sealed partial class SignInViewModel : ObservableObject, IDisposable
{
    private readonly ShellServices _s;
    private ulong _request;
    private ulong? _flow;
    private bool _openedUrl;

    [ObservableProperty]
    private SignInStage _stage = SignInStage.Starting;

    [ObservableProperty]
    private QrCode? _qr;

    [ObservableProperty]
    private string? _qrData;

    [ObservableProperty]
    private string? _url;

    [ObservableProperty]
    private string _message = string.Empty;

    [ObservableProperty]
    private string? _account;

    [ObservableProperty]
    private DateTimeOffset? _expiresAt;

    public SignInViewModel(ShellServices services, PluginInfo plugin, AuthMethod method)
    {
        _s = services;
        Plugin = plugin;
        Method = method;
        _s.Hub.Notification += OnNotification;
    }

    public PluginInfo Plugin { get; }

    public AuthMethod Method { get; }

    public ObservableCollection<SignInField> Fields { get; } = [];

    public bool IsFinished => Stage is SignInStage.Done or SignInStage.Failed;

    /// <summary>Raised when the flow ends (signed in, failed or cancelled).</summary>
    public event Action<SignInViewModel>? Finished;

    public void Begin()
    {
        Stage = SignInStage.Starting;
        _request = _s.SendHandled(new CoreCommand.BeginLogin(Plugin.Id, Method));
    }

    [RelayCommand]
    private void OpenBrowser()
    {
        if (Url is not null)
        {
            _s.Platform.OpenUrl(Url);
        }
    }

    [RelayCommand]
    private void CopyLink()
    {
        if ((Url ?? QrData) is { } text)
        {
            _s.Platform.CopyText(text);
        }
    }

    [RelayCommand]
    private void Submit()
    {
        if (_flow is { } flow && Stage == SignInStage.AskFields)
        {
            var fields = Fields.Select(f => new KeyValuePair<string, string>(f.Name, f.Value)).ToArray();
            _s.SendHandled(new CoreCommand.SubmitLogin(Plugin.Id, flow, fields));
            foreach (var f in Fields.Where(f => f.IsSecret))
            {
                f.Value = string.Empty; // do not keep secrets in memory longer than needed
            }

            Stage = SignInStage.Waiting;
        }
    }

    [RelayCommand]
    private void Cancel()
    {
        if (_flow is { } flow && !IsFinished)
        {
            _s.SendHandled(new CoreCommand.CancelLogin(Plugin.Id, flow));
        }

        Finish();
    }

    internal void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.Auth { Update: var u } when u.Plugin == Plugin.Id && (_flow is null || _flow == u.Flow):
                _flow = u.Flow;
                Apply(u.Step);
                break;
            case CoreEvent.CommandFailed f when f.Id == _request:
                Message = f.Message;
                Stage = SignInStage.Failed;
                break;
        }
    }

    private void Apply(AuthStep step)
    {
        switch (step)
        {
            case AuthStep.ShowQr q:
                QrData = q.Data;
                Qr = QrCode.Encode(q.Data);
                ExpiresAt = q.ExpiresAtMs is { } ms ? DateTimeOffset.FromUnixTimeMilliseconds(ms) : null;
                Stage = SignInStage.ShowQr;
                break;
            case AuthStep.OpenUrl o:
                Url = o.Url;
                Stage = SignInStage.OpenUrl;
                if (!_openedUrl)
                {
                    _openedUrl = true;
                    _s.Platform.OpenUrl(o.Url);
                }

                break;
            case AuthStep.AskFields a:
                Fields.Clear();
                foreach (var f in a.Fields)
                {
                    Fields.Add(new SignInField(f));
                }

                Stage = SignInStage.AskFields;
                break;
            case AuthStep.Waiting w:
                Message = w.Message;
                Stage = SignInStage.Waiting;
                break;
            case AuthStep.Done d:
                Account = d.Account;
                Stage = SignInStage.Done;
                Finish();
                break;
            case AuthStep.Failed f:
                Message = f.Message;
                Stage = SignInStage.Failed;
                break;
        }

        OnPropertyChanged(nameof(IsFinished));
    }

    private void Finish()
    {
        _s.Hub.Notification -= OnNotification;
        Finished?.Invoke(this);
    }

    public void Dispose() => _s.Hub.Notification -= OnNotification;
}
