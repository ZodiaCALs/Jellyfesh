// How the sign-in sheet draws the fields a plugin asks for (`AuthStep::AskFields`) and the code it
// shows (`AuthStep::ShowCode`), as the plugin describes them (docs/parity.md N9, N11): help and
// "Optional" under a field's label, what the system may offer to fill it with, digits only in a
// number field, and Sign In only once every required field has something in it. The core and the
// plugin check what is sent.

import AppKit

enum SignInForm {
    /// Whether Sign In can go ahead: every required field has more than spaces in it.
    static func canSubmit(_ fields: [SignInField], values: [String: String]) -> Bool {
        fields.allSatisfy { field in
            !field.required || !(values[field.name] ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
    }

    /// What a field keeps of the text typed or pasted into it: a number field keeps its digits
    /// (full-width ones as their ASCII digits), the others everything.
    static func accepted(_ text: String, for field: SignInField) -> String {
        guard field.kind == .number else { return text }
        let halfwidth = text.applyingTransform(.fullwidthToHalfwidth, reverse: false) ?? text
        return String(halfwidth.filter { $0.isASCII && $0.isNumber })
    }

    /// The line under a field's label: "Optional" for a field the person may leave empty, then the
    /// plugin's help; nil when there is neither.
    static func note(for field: SignInField) -> String? {
        let optional = field.required ? nil : String(localized: "Optional")
        let help = field.help?.trimmingCharacters(in: .whitespacesAndNewlines)
        let note = CountText.joined([optional, help])
        return note.isEmpty ? nil : note
    }

    /// Whether the field takes several lines: a pasted Cookie header does.
    static func isMultiline(_ field: SignInField) -> Bool {
        field.kind == .cookieHeader
    }

    /// What the system may offer to fill a field with. Secrets are passwords, except keys (an API
    /// key is not a password to save); a text field named for the account is its user name.
    static func contentType(of field: SignInField) -> NSTextContentType? {
        let name = field.name.lowercased()
        switch field.kind {
        case .email: return .emailAddress
        case .url: return .URL
        case .number: return .oneTimeCode
        case .cookieHeader: return nil
        case .text:
            if field.secret {
                return name.contains("key") || name.contains("token") ? nil : .password
            }
            return name.contains("user") || name.contains("login") || name == "name" ? .username : nil
        }
    }

    /// A code spelled out a character at a time, for VoiceOver ("A B 1 2").
    static func spelledOut(_ code: String) -> String {
        code.filter { !$0.isWhitespace }.map(String.init).joined(separator: " ")
    }
}
