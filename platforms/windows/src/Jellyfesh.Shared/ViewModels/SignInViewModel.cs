using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Qr;

namespace Jellyfesh.ViewModels;

/// <summary>A way to sign in as a button: the method and its title ("Sign in with a QR code").</summary>
public sealed record SignInMethodChoice(AuthMethod Method, string Title)
{
    public static SignInMethodChoice For(AuthMethod method, ILocalizer l) => new(method, l.Get(TitleKey(method)));

    /// <summary>The string key of the button that signs in with <paramref name="method"/>.</summary>
    public static string TitleKey(AuthMethod method) => method switch
    {
        AuthMethod.Qr => "SignIn_MethodQr",
        AuthMethod.Oauth => "SignIn_MethodOauth",
        AuthMethod.CookieImport => "SignIn_MethodCookies",
        AuthMethod.PasswordExchange => "SignIn_MethodPassword",
        _ => "SignIn_MethodApiKey",
    };

    public override string ToString() => Title;
}

public enum SignInStage
{
    Starting,
    ShowQr,

    /// <summary>A code to enter on another device, shown large (Jellyfin's Quick Connect).</summary>
    ShowCode,
    OpenUrl,
    AskFields,
    Waiting,
    Done,
    Failed,
}

/// <summary>
/// A field the source asks for, as the plugin describes it (docs/parity.md N11): its label, the
/// line under it ("Optional" for a field that may stay empty, then the plugin's help), the field its
/// kind needs (several lines for a pasted Cookie header, digits only for a number), and its
/// placeholder. Secrets go to the core once and are never stored (ADR-0015).
/// </summary>
public sealed partial class SignInField : ObservableObject
{
    [ObservableProperty]
    private string _value = string.Empty;

    public SignInField(AuthField field, ILocalizer l)
    {
        Name = field.Name;
        Label = field.Label;
        IsSecret = field.Secret;
        Placeholder = field.Placeholder ?? string.Empty;
        Kind = field.Kind;
        IsRequired = field.Required;
        var note = string.Join(" · ", new[] { field.Required ? null : l.Get("SignIn_Optional"), field.Help?.Trim() }.Where(p => !string.IsNullOrEmpty(p)));
        Note = note.Length == 0 ? null : note;
    }

    public string Name { get; }

    public string Label { get; }

    public bool IsSecret { get; }

    public string Placeholder { get; }

    public AuthFieldKind Kind { get; }

    public bool IsRequired { get; }

    /// <summary>The line under the field: "Optional", the plugin's help, both, or none (null, so no empty line shows).</summary>
    public string? Note { get; }

    /// <summary>A pasted Cookie header takes several lines.</summary>
    public bool IsMultiline => Kind == AuthFieldKind.CookieHeader && !IsSecret;

    /// <summary>Whether Sign In may go ahead as far as this field goes: optional, or more than spaces in it.</summary>
    public bool IsFilled => !IsRequired || !string.IsNullOrWhiteSpace(Value);

    /// <summary>What a field keeps of what is typed or pasted: a number field its digits (full-width ones as ASCII), the others everything.</summary>
    public static string Accepted(string text, AuthFieldKind kind)
    {
        if (kind != AuthFieldKind.Number)
        {
            return text;
        }

        var digits = new System.Text.StringBuilder(text.Length);
        foreach (var c in text)
        {
            if (c is >= '0' and <= '9')
            {
                digits.Append(c);
            }
            else if (c is >= '\uFF10' and <= '\uFF19')
            {
                digits.Append((char)('0' + (c - '\uFF10')));
            }
        }

        return digits.ToString();
    }

    partial void OnValueChanged(string value)
    {
        var accepted = Accepted(value, Kind);
        if (accepted != value)
        {
            Value = accepted;
        }
    }
}

/// <summary>
/// One sign-in flow, rendered natively from the core's steps (architecture §10): a QR code to
/// scan, a code to enter on another device, a page to open in the browser, fields to fill, or a
/// wait.
/// </summary>
public sealed partial class SignInViewModel : ObservableObject, IDisposable
{
    private readonly ShellServices _s;
    private readonly CoreCommand _start;
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
    private string _qrHint = string.Empty;

    [ObservableProperty]
    private string _code = string.Empty;

    [ObservableProperty]
    private string _codeHint = string.Empty;

    [ObservableProperty]
    private string _message = string.Empty;

    [ObservableProperty]
    private string? _account;

    [ObservableProperty]
    private DateTimeOffset? _expiresAt;

    public SignInViewModel(ShellServices services, PluginInfo plugin, AuthMethod method)
        : this(services, plugin, method, plugin.Name, new CoreCommand.BeginLogin(plugin.Id, method))
    {
    }

    /// <summary>
    /// A flow that <paramref name="start"/> begins instead of BeginLogin (ConnectSource, from Add a
    /// Source), titled with <paramref name="name"/>; its steps still arrive as the plugin's Auth events.
    /// </summary>
    public SignInViewModel(ShellServices services, PluginInfo plugin, AuthMethod method, string name, CoreCommand start)
    {
        _s = services;
        Plugin = plugin;
        Method = method;
        Name = name;
        _start = start;
        _s.Hub.Notification += OnNotification;
    }

    public PluginInfo Plugin { get; }

    /// <summary>What the dialog's title names: the plugin, or the service chosen in Add a Source.</summary>
    public string Name { get; }

    public AuthMethod Method { get; }

    public ObservableCollection<SignInField> Fields { get; } = [];

    public bool IsFinished => Stage is SignInStage.Done or SignInStage.Failed;

    /// <summary>Sign In waits until every required field has something in it.</summary>
    public bool CanSubmit => Stage == SignInStage.AskFields && Fields.All(f => f.IsFilled);

    /// <summary>The code as a screen reader should say it, one character at a time ("Code 4 8 2 9 1 3").</summary>
    public string CodeSpoken => _s.L.Format("SignIn_CodeSpoken", Spelled(Code));

    /// <summary>What a failed sign-in says: the plugin's or the core's message alone, as in the other apps.</summary>
    public string FailureText => string.IsNullOrWhiteSpace(Message) ? _s.L.Get("SignIn_FailedNoReason") : Message;

    /// <summary>A code spelled out a character at a time, without its spaces.</summary>
    public static string Spelled(string code) => string.Join(' ', code.Where(c => !char.IsWhiteSpace(c)));

    /// <summary>Raised when the flow ends (signed in, failed or cancelled).</summary>
    public event Action<SignInViewModel>? Finished;

    public void Begin()
    {
        Stage = SignInStage.Starting;
        _request = _s.SendHandled(_start);
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

    [RelayCommand(CanExecute = nameof(CanSubmit))]
    private void Submit()
    {
        if (_flow is { } flow && CanSubmit)
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
                OnPropertyChanged(nameof(IsFinished));
                OnPropertyChanged(nameof(FailureText));
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
                QrHint = string.IsNullOrWhiteSpace(q.Hint) ? _s.L.Format("SignIn_ScanQr", Plugin.Name) : q.Hint.Trim();
                Stage = SignInStage.ShowQr;
                break;
            case AuthStep.ShowCode c:
                Code = c.Code.Trim();
                CodeHint = c.Hint.Trim();
                ExpiresAt = c.ExpiresAtMs is { } codeMs ? DateTimeOffset.FromUnixTimeMilliseconds(codeMs) : null;
                OnPropertyChanged(nameof(CodeSpoken));
                Stage = SignInStage.ShowCode;
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
                foreach (var old in Fields)
                {
                    old.PropertyChanged -= OnFieldChanged;
                }

                Fields.Clear();
                foreach (var f in a.Fields)
                {
                    var field = new SignInField(f, _s.L);
                    field.PropertyChanged += OnFieldChanged;
                    Fields.Add(field);
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
        OnPropertyChanged(nameof(FailureText));
        RefreshCanSubmit();
    }

    private void OnFieldChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(SignInField.Value))
        {
            RefreshCanSubmit();
        }
    }

    private void RefreshCanSubmit()
    {
        OnPropertyChanged(nameof(CanSubmit));
        SubmitCommand.NotifyCanExecuteChanged();
    }

    partial void OnStageChanged(SignInStage value) => RefreshCanSubmit();

    private void Finish()
    {
        _s.Hub.Notification -= OnNotification;
        Finished?.Invoke(this);
    }

    public void Dispose() => _s.Hub.Notification -= OnNotification;
}
