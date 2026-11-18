using System.ComponentModel;
using Jellyfesh.App.Shell;
using Jellyfesh.Formatting;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Views;

public sealed partial class SignInDialog : ContentDialog
{
    public SignInDialog(SignInViewModel flow)
    {
        Flow = flow;
        InitializeComponent();
        var l = AppHost.L;
        Title = l.Format("SignIn_Title", flow.Name);
        CloseButtonText = l.Get("Action_Cancel");
        Flow.PropertyChanged += OnFlowChanged;
        Flow.Finished += OnFinished;
        ShowStage();
    }

    public SignInViewModel Flow { get; }

    private void OnFlowChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is nameof(SignInViewModel.Stage) or nameof(SignInViewModel.Message) or nameof(SignInViewModel.CanSubmit))
        {
            ShowStage();
        }
    }

    private void ShowStage()
    {
        var l = AppHost.L;
        var stage = Flow.Stage;
        WaitingPanel.Visibility = Bind.Visible(stage is SignInStage.Starting or SignInStage.Waiting);
        if (stage == SignInStage.Starting)
        {
            WaitingText.Text = l.Get("SignIn_Starting");
        }

        QrPanel.Visibility = Bind.Visible(stage == SignInStage.ShowQr);
        CodePanel.Visibility = Bind.Visible(stage == SignInStage.ShowCode);
        UrlPanel.Visibility = Bind.Visible(stage == SignInStage.OpenUrl);
        FieldsPanel.Visibility = Bind.Visible(stage == SignInStage.AskFields);
        ResultText.Visibility = Bind.Visible(stage is SignInStage.Done or SignInStage.Failed);
        ResultText.Text = stage switch
        {
            SignInStage.Done => l.Format("SignIn_Done", Flow.Account ?? string.Empty),
            SignInStage.Failed => Flow.FailureText,
            _ => string.Empty,
        };
        PrimaryButtonText = stage == SignInStage.AskFields ? l.Get("SignIn_Submit") : string.Empty;
        IsPrimaryButtonEnabled = Flow.CanSubmit;
        CloseButtonText = l.Get(stage is SignInStage.Done or SignInStage.Failed ? "Action_Close" : "Action_Cancel");
    }

    private void OnPrimaryClick(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        // The dialog stays open: the core answers with the next step.
        args.Cancel = true;
        Flow.SubmitCommand.Execute(null);
    }

    private void OnCloseClick(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (!Flow.IsFinished)
        {
            Flow.CancelCommand.Execute(null);
        }
    }

    private void OnFinished(SignInViewModel flow)
    {
        Flow.PropertyChanged -= OnFlowChanged;
        if (flow.Stage == SignInStage.Done)
        {
            Hide();
        }
    }
}
