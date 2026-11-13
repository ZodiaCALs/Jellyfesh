// One short message at the top of the window (ToastCenter announces it to VoiceOver), and the
// page the window shows when the core could not start.

import SwiftUI

struct ToastOverlay: View {
    @Environment(ToastCenter.self) private var toasts
    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects

    var body: some View {
        ZStack {
            if let message = toasts.message {
                HStack(spacing: Tokens.Space.s2) {
                    Image(systemName: "exclamationmark.circle")
                        .foregroundStyle(palette[Tokens.Semantic.warning])
                    Text(message)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(palette[Tokens.Ink.ink90])
                        .lineLimit(3)
                        .multilineTextAlignment(.leading)
                    Button {
                        toasts.dismiss()
                    } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .bold))
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(palette[Tokens.Ink.ink50])
                    .accessibilityLabel(Text("Dismiss"))
                }
                .padding(.horizontal, Tokens.Space.s4)
                .padding(.vertical, Tokens.Space.s2)
                .glass(.veil, in: Capsule())
                .elevation(Tokens.Elevation.shadow2)
                .frame(maxWidth: 520)
                .padding(.top, Tokens.Space.s3)
                .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .animation(Motion.unless(effects.reduceMotion, Motion.move), value: toasts.message)
    }
}

struct StartupErrorView: View {
    var message: String

    var body: some View {
        ContentUnavailableView {
            Label("Jellyfesh Could Not Start", systemImage: "exclamationmark.triangle")
        } description: {
            Text(message)
        } actions: {
            Button("Quit") {
                NSApplication.shared.terminate(nil)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
