// Sign-in flows (docs/core-api.md: BeginLogin, SubmitLogin, CancelLogin, Auth updates): the core
// drives the steps and the sheet renders each one, a QR code to scan, a code to enter on another
// device (Jellyfin's Quick Connect), the browser to continue in, or fields to fill as the plugin
// describes them. Closing the sheet before the end cancels the flow. And the disclosure for
// community plugins (ADR-0012).

import AppKit
import SwiftUI

struct SignInSheet: View {
    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette

    var body: some View {
        if let flow = sources.signIn {
            VStack(spacing: Tokens.Space.s5) {
                Text("Sign In to \(flow.pluginName)")
                    .font(.system(size: 20, weight: .bold))
                    .accessibilityAddTraits(.isHeader)
                SignInStepView(flow: flow)
                    .frame(maxWidth: .infinity)
                HStack {
                    Spacer()
                    Button(flow.isOver ? LocalizedStringKey("Done") : LocalizedStringKey("Cancel")) {
                        sources.endSignIn()
                    }
                    .keyboardShortcut(flow.isOver ? .defaultAction : .cancelAction)
                }
            }
            .padding(Tokens.Space.s6)
            .frame(width: 440)
        }
    }
}

private struct SignInStepView: View {
    var flow: SignInFlow

    @Environment(SourcesStore.self) private var sources
    @Environment(\.palette) private var palette

    var body: some View {
        if flow.unavailable {
            SourcesUnavailableNotice()
        } else if let error = flow.error {
            message(error, symbol: "exclamationmark.triangle", tint: palette[Tokens.Semantic.danger])
        } else {
            switch flow.step {
            case nil:
                ProgressView()
                    .controlSize(.large)
                    .padding()
            case .showQR(let data, let hint, _)?:
                VStack(spacing: Tokens.Space.s3) {
                    if let image = QRCode.image(for: data) {
                        Image(nsImage: image)
                            .interpolation(.none)
                            .resizable()
                            .scaledToFit()
                            .frame(width: 220, height: 220)
                            .padding(Tokens.Space.s3)
                            .background(RoundedRectangle(cornerRadius: Tokens.Radius.rLg).fill(Color.white))
                            .accessibilityLabel(Text("Sign-in QR code"))
                    }
                    Text(scanSentence(hint))
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.secondary)
                }
            case .showCode(let code, let hint, _)?:
                SignInCode(code: code, hint: hint)
            case .openURL(let address)?:
                VStack(spacing: Tokens.Space.s3) {
                    Text("Continue in your browser. Come back here when you are done.")
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.secondary)
                    Button("Open Browser Again") {
                        Self.open(address)
                    }
                }
                .task(id: address) {
                    Self.open(address)
                }
            case .askFields(let fields)?:
                SignInFieldsForm(fields: fields)
            case .waiting(let text)?:
                VStack(spacing: Tokens.Space.s3) {
                    ProgressView()
                    Text(text)
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.secondary)
                }
            case .done(let account)?:
                message(String(localized: "Signed in as \(account)."), symbol: "checkmark.circle",
                        tint: palette[Tokens.Semantic.success])
            case .failed(let text)?:
                message(text, symbol: "exclamationmark.triangle", tint: palette[Tokens.Semantic.danger])
            }
        }
    }

    /// The plugin's sentence under its QR code, or the shared one when it gives none.
    private func scanSentence(_ hint: String?) -> String {
        if let hint, !hint.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return hint }
        return String(localized: "Scan the code with the \(flow.pluginName) app on your phone.")
    }

    private func message(_ text: String, symbol: String, tint: Color) -> some View {
        VStack(spacing: Tokens.Space.s3) {
            Image(systemName: symbol)
                .font(.system(size: 28))
                .foregroundStyle(tint)
            Text(text)
                .multilineTextAlignment(.center)
        }
        .accessibilityElement(children: .combine)
    }

    /// Opens a sign-in page. Only web pages: a plugin cannot make the app launch other schemes.
    static func open(_ address: String) {
        guard let url = URL(string: address), let scheme = url.scheme?.lowercased(),
              scheme == "https" || scheme == "http"
        else { return }
        NSWorkspace.shared.open(url)
    }
}

/// A code to enter on another device (`AuthStep::ShowCode`, Jellyfin's Quick Connect): large and
/// selectable, with the plugin's words for where to enter it, while the core waits for the
/// approval.
private struct SignInCode: View {
    var code: String
    var hint: String

    var body: some View {
        VStack(spacing: Tokens.Space.s3) {
            Text(verbatim: code)
                .font(.system(size: 40, weight: .bold))
                .monospacedDigit()
                .tracking(6)
                .textSelection(.enabled)
                .accessibilityLabel(Text("Code \(SignInForm.spelledOut(code))"))
            if !hint.isEmpty {
                Text(hint)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.secondary)
            }
            HStack(spacing: Tokens.Space.s2) {
                ProgressView()
                    .controlSize(.small)
                Text("Waiting for approval…")
                    .foregroundStyle(.secondary)
            }
        }
    }
}

/// The fields a plugin asks for (`AuthStep::AskFields`), as it describes them: "Optional" and its
/// help under a field's label, the field its kind needs (several lines for a pasted Cookie header)
/// and what the system may fill in. Sign In waits for the required fields. Secrets are never kept
/// here.
private struct SignInFieldsForm: View {
    var fields: [SignInField]

    @Environment(SourcesStore.self) private var sources
    @State private var values: [String: String] = [:]

    var body: some View {
        Form {
            ForEach(fields, id: \.name) { field in
                SignInFieldRow(field: field, text: binding(for: field))
            }
            HStack {
                Spacer()
                Button("Sign In") {
                    sources.submitSignIn(fields: values)
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!SignInForm.canSubmit(fields, values: values))
            }
        }
        .formStyle(.grouped)
        .onDisappear {
            values = [:]
        }
    }

    private func binding(for field: SignInField) -> Binding<String> {
        Binding(get: { values[field.name] ?? "" },
                set: { values[field.name] = SignInForm.accepted($0, for: field) })
    }
}

/// One field: its label with the line under it, and the plugin's placeholder.
private struct SignInFieldRow: View {
    var field: SignInField
    @Binding var text: String

    var body: some View {
        if SignInForm.isMultiline(field) {
            VStack(alignment: .leading, spacing: Tokens.Space.s1) {
                Text(field.label)
                if let note = SignInForm.note(for: field) {
                    Text(note)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                TextEditor(text: $text)
                    .font(.system(size: 12).monospaced())
                    .autocorrectionDisabled()
                    .frame(minHeight: 72, maxHeight: 120)
                    .accessibilityLabel(Text(field.label))
            }
        } else if field.secret {
            SecureField(text: $text, prompt: prompt) { label }
                .textContentType(SignInForm.contentType(of: field))
        } else {
            TextField(text: $text, prompt: prompt) { label }
                .textContentType(SignInForm.contentType(of: field))
                .autocorrectionDisabled()
        }
    }

    private var prompt: Text? {
        field.placeholder.map { Text($0) }
    }

    @ViewBuilder private var label: some View {
        Text(field.label)
        if let note = SignInForm.note(for: field) {
            Text(note)
        }
    }
}

/// What turning on community plugins means (ADR-0012). `pluginName` nil is the global switch.
struct CommunityDisclosure: Identifiable, Equatable {
    var pluginID: String?
    var pluginName: String?
    var id: String { pluginID ?? "" }
}

extension View {
    /// Asks before allowing community plugins or turning one on; `accept` runs on "I Understand".
    func communityDisclosure(_ request: Binding<CommunityDisclosure?>,
                             accept: @escaping (CommunityDisclosure) -> Void) -> some View {
        alert(
            request.wrappedValue?.pluginName.map { Text("Turn On “\($0)”?") } ?? Text("Allow Community Plugins?"),
            isPresented: Binding(get: { request.wrappedValue != nil }, set: { if !$0 { request.wrappedValue = nil } }),
            presenting: request.wrappedValue
        ) { disclosure in
            Button("I Understand, Allow", role: .destructive) {
                accept(disclosure)
            }
            Button("Cancel", role: .cancel) {}
        } message: { _ in
            Text("Community plugins are not made or checked by Jellyfesh.\n• They may break the terms of the service they connect to.\n• They may stop working at any time.\n• Use them for personal use only.")
        }
    }
}
