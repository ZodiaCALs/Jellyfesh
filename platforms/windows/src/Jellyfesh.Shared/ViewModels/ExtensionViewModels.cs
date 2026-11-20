using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// An on-device speech model in Settings › Extensions › AI Lyrics: "Name · Size", then Download,
/// "Downloading… 35%" with Cancel, or "Ready" with Delete… (which asks first).
/// </summary>
public sealed partial class SpeechModelItem(ShellServices s) : ObservableObject
{
    [ObservableProperty]
    private SpeechModel? _model;

    public string Id => Model?.Id ?? string.Empty;

    /// <summary>"Base · 142 MB".</summary>
    public string Title => Model is { } m ? s.L.Format("Models_NameSize", m.Name, Sizes.Bytes(m.Bytes, s.L)) : string.Empty;

    /// <summary>"Downloading… 35%", "Ready", the failure, or nothing before a download.</summary>
    public string Status => Model?.State switch
    {
        SpeechModelStatus.Downloading d => s.L.Format("Models_Downloading", Percent(d.Fraction)),
        SpeechModelStatus.Ready => s.L.Get("Models_Ready"),
        SpeechModelStatus.Failed f => f.Message,
        _ => string.Empty,
    };

    public bool IsDownloading => Model?.State is SpeechModelStatus.Downloading;

    public double Progress => Model?.State is SpeechModelStatus.Downloading d ? Math.Clamp(d.Fraction, 0, 1) * 100 : 0;

    public bool CanDownload => Model?.State is SpeechModelStatus.NotDownloaded or SpeechModelStatus.Failed;

    public bool CanDelete => Model?.State is SpeechModelStatus.Ready;

    internal void Update(SpeechModel model)
    {
        Model = model;
        OnPropertyChanged(string.Empty);
    }

    [RelayCommand]
    private void Download()
    {
        if (Model is { } m)
        {
            s.Send(new CoreCommand.DownloadSpeechModel(m.Id));
        }
    }

    [RelayCommand]
    private void Cancel()
    {
        if (Model is { } m)
        {
            s.Send(new CoreCommand.CancelSpeechModelDownload(m.Id));
        }
    }

    /// <summary>Delete… asks "Delete the {Model} Model?" first.</summary>
    [RelayCommand]
    private async Task Delete()
    {
        if (Model is { } m && await s.Platform.ConfirmAsync(
                s.L.Format("Models_DeleteTitle", m.Name), s.L.Get("Models_DeleteBody"), s.L.Get("Action_Delete"), s.L.Get("Action_Cancel")))
        {
            s.Send(new CoreCommand.DeleteSpeechModel(m.Id));
        }
    }

    internal static string Percent(float fraction) => $"{Math.Round(Math.Clamp(fraction, 0f, 1f) * 100):0}%";
}

/// <summary>An origin AI Lyrics may send audio to (Sending Audio), with Stop Allowing.</summary>
public sealed partial class AudioConsentItem(ShellServices s, AudioConsent consent) : ObservableObject
{
    public AudioConsent Consent { get; } = consent;

    public string Origin => Consent.Origin;

    [RelayCommand]
    private void StopAllowing() => s.Send(new CoreCommand.RevokeAudioUpload(Consent.Plugin, Consent.Origin));
}

/// <summary>
/// An extension in Settings › Extensions (ADR-0019, ADR-0020): the plugin's own card (Enabled, its
/// settings by group, sign-in, "Version · Author", Uninstall…) and what its kind adds. AI Lyrics
/// (the plugin that makes lyrics) adds its sign-in state ("API Key Added"), On-Device Models and
/// Sending Audio; Customize (the appearance plugin) adds Reset to Defaults. A Pro plugin shows the
/// Pro badge while Pro is inactive.
/// </summary>
public sealed partial class ExtensionViewModel(ShellServices s, PluginViewModel plugin) : ObservableObject
{
    public PluginViewModel Plugin { get; } = plugin;

    public PluginInfo? Info => Plugin.Info;

    /// <summary>The plugin that makes lyrics (capability make_lyrics).</summary>
    public bool IsAiLyrics => Info?.Capabilities.Contains("make_lyrics") ?? false;

    /// <summary>The plugin that changes the look (capability appearance).</summary>
    public bool IsCustomize => Info?.Capabilities.Contains("appearance") ?? false;

    /// <summary>The Pro badge: a Pro plugin while Pro is inactive.</summary>
    public bool ShowsProBadge { get; private set; }

    /// <summary>AI Lyrics' sign-in: "API Key Added" while its online key is kept.</summary>
    public string KeyStatus => IsAiLyrics && Info?.Account is not null ? s.L.Get("AiLyrics_KeyAdded") : string.Empty;

    public ObservableCollection<SpeechModelItem> Models { get; } = [];

    public ObservableCollection<AudioConsentItem> Consents { get; } = [];

    public bool HasModels => Models.Count > 0;

    public bool HasConsents => Consents.Count > 0;

    internal void Update(PluginInfo info, CoreSnapshot state)
    {
        Plugin.Update(info);
        ShowsProBadge = info.RequiresPro && !state.Pro.Active;
        if (IsAiLyrics)
        {
            SyncModels(state.AiLyrics.Models);
            SyncConsents(state.AiLyrics.Consents.Where(c => c.Plugin == info.Id));
        }
        else
        {
            Models.Clear();
            Consents.Clear();
        }

        OnPropertyChanged(string.Empty);
    }

    /// <summary>Reset to Defaults: the schema's defaults (Customize).</summary>
    [RelayCommand]
    private void ResetToDefaults()
    {
        if (Info is { } info)
        {
            s.Send(new CoreCommand.ResetPluginSettings(info.Id));
        }
    }

    private void SyncModels(IReadOnlyList<SpeechModel> models)
    {
        var byId = Models.ToDictionary(m => m.Id);
        Models.Clear();
        foreach (var model in models)
        {
            var item = byId.TryGetValue(model.Id, out var existing) ? existing : new SpeechModelItem(s);
            item.Update(model);
            Models.Add(item);
        }
    }

    private void SyncConsents(IEnumerable<AudioConsent> consents)
    {
        Consents.Clear();
        foreach (var consent in consents)
        {
            Consents.Add(new AudioConsentItem(s, consent));
        }
    }
}

/// <summary>Sizes in the person's units: "142 MB", "1.6 GB".</summary>
public static class Sizes
{
    public static string Bytes(ulong bytes, ILocalizer l)
    {
        const double Mb = 1_000_000, Gb = 1_000_000_000;
        return bytes >= Gb
            ? l.Format("Size_GB", (bytes / Gb).ToString("0.#", l.Culture))
            : l.Format("Size_MB", Math.Max(1, Math.Round(bytes / Mb)).ToString("0", l.Culture));
    }
}
