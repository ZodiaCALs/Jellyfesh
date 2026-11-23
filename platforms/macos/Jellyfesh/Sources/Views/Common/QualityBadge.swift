// The quality badge (ADR-0026 §3): a quiet capsule with the playing quality's label that opens a
// native menu: the format line, then Streaming Quality's levels with the chosen one checked, or
// why there are none. What it says is QualityMenuContent's; choosing a level is the caller's.

import SwiftUI

struct QualityBadge: View {
    var content: QualityMenuContent
    var choose: (StreamingQualityOption) -> Void

    @Environment(\.palette) private var palette
    @State private var hovering = false

    var body: some View {
        Menu {
            if let line = content.formatLine {
                Text(line)
            }
            if content.levels.isEmpty {
                if let note = content.note {
                    Text(note)
                }
            } else {
                Section("Streaming Quality") {
                    Picker("Streaming Quality", selection: Binding(get: { content.chosen }, set: { choose($0) })) {
                        ForEach(content.levels, id: \.self) { level in
                            Text(level.title).tag(level)
                        }
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                }
            }
        } label: {
            Text(content.badge)
                .lookFont(size: 12, weight: .semibold)
                .lineLimit(1)
                .foregroundStyle(palette[Tokens.Ink.ink90])
                .padding(.horizontal, 10)
                .frame(height: 28)
                .frame(maxWidth: 132)
                .background(Capsule().fill(palette[hovering ? Tokens.Line.fillHover : Tokens.Line.fill]))
                .contentShape(Capsule())
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .onHover { hovering = $0 }
        .accessibilityLabel(Text("Quality: \(content.badge)"))
        .help(Text("Streaming Quality"))
    }
}
