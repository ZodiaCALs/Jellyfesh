// Preferences → Appearance (docs/design-controls.md, "v0.0.4"), grouped like System Settings: the
// theme with small previews; the accent colour (follow the artwork, ten swatches or a custom hue
// on a hue track); transparency from Solid to Clear with a live preview; Reduce motion and
// transparency; the Now Playing background with previews; the lyrics' size, alignment,
// translation and romanization; and the app icon (ADR-0017). Every change applies at once and the
// core keeps it (SetSettings); sliders send their value when they are let go.

import SwiftUI

struct AppearanceSettings: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(ThemeStore.self) private var theme
    @Environment(\.palette) private var palette
    @State private var hueDraft: Double?
    @State private var transparencyDraft: Double?

    var body: some View {
        let current = settings.current
        Form {
            Section {
                LabeledContent {
                    ThemeChoices(selection: current.appearance) { settings.setAppearance($0) }
                } label: {
                    Text("Theme")
                    Text("Light, dark, or the same as macOS.")
                }
            }

            Section {
                Toggle(isOn: Binding(
                    get: { current.hueSource == .artwork },
                    set: { follow in
                        // Turning it off keeps the colour shown now.
                        settings.setHueSource(follow ? .artwork : .fixed(hue: AccentChoices.normalize(theme.hue)))
                    }
                )) {
                    Text("Follow the Artwork")
                    Text("The colour of buttons, selections and the Misty field.")
                }
                if case .fixed(let hue) = current.hueSource {
                    AccentSwatches(selected: AccentChoices.swatchIndex(current.hueSource)) { swatch in
                        hueDraft = nil
                        settings.setHueSource(.fixed(hue: swatch))
                    }
                    LabeledContent {
                        HStack(spacing: Tokens.Space.s3) {
                            HueSlider(hue: Binding(get: { hueDraft ?? Double(hue) }, set: { hueDraft = $0 })) { value in
                                settings.setHueSource(.fixed(hue: AccentChoices.normalize(Float(value))))
                                hueDraft = nil
                            }
                            AccentPreview(hue: Float(hueDraft ?? Double(hue)))
                        }
                    } label: {
                        Text("Custom")
                    }
                }
            } header: {
                Text("Accent Colour")
            }

            Section {
                let transparency = transparencyDraft ?? Double(current.transparency)
                VStack(alignment: .leading, spacing: Tokens.Space.s3) {
                    LabeledContent {
                        Text(transparency, format: .percent.precision(.fractionLength(0)))
                            .monospacedDigit()
                    } label: {
                        Text("Transparency")
                        Text("How much of the background shows through the glass.")
                    }
                    HStack(spacing: Tokens.Space.s3) {
                        Text("Solid")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Slider(value: Binding(get: { transparency }, set: { transparencyDraft = $0 }),
                               in: 0...1, step: 0.05,
                               onEditingChanged: { editing in
                                   if !editing, let value = transparencyDraft {
                                       settings.setTransparency(Float(value))
                                       transparencyDraft = nil
                                   }
                               })
                        .accessibilityLabel(Text("Transparency"))
                        Text(String(localized: "Appearance.Transparency.Clear", defaultValue: "Clear",
                                    comment: "The clear end of the Transparency slider (glass, not erase)"))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    TransparencyPreview(transparency: transparency, reduceEffects: current.reduceEffects)
                }
                Toggle(isOn: Binding(get: { current.reduceEffects }, set: { settings.setReduceEffects($0) })) {
                    Text("Reduce Motion and Transparency")
                    Text("Solid surfaces instead of glass, and short fades instead of movement.")
                }
            }

            Section {
                LabeledContent {
                    BackdropChoices(selection: current.style) { settings.setStyle($0) }
                } label: {
                    Text("Now Playing Backdrop")
                    Text("The artwork, blurred, or the Misty field in its colour.")
                }
            }

            Section {
                Picker("Lyrics Size", selection: Binding(get: { current.lyricsSize }, set: { settings.setLyricsSize($0) })) {
                    Text("Small").tag(LyricsSizeOption.small)
                    Text("Medium").tag(LyricsSizeOption.medium)
                    Text("Large").tag(LyricsSizeOption.large)
                }
                .pickerStyle(.segmented)
                Picker("Lyrics Alignment", selection: Binding(get: { current.lyricsAlign }, set: { settings.setLyricsAlign($0) })) {
                    Text("Start").tag(LyricsAlignOption.start)
                    Text("Centre").tag(LyricsAlignOption.center)
                }
                .pickerStyle(.segmented)
                Toggle(isOn: Binding(get: { current.showTranslation }, set: { settings.setShowTranslation($0) })) {
                    Text("Show Translations")
                    Text("A line in another language under each lyric line, when the lyrics have one.")
                }
                Toggle(isOn: Binding(get: { current.showRomanization }, set: { settings.setShowRomanization($0) })) {
                    Text("Show Romanization")
                    Text("How each line reads in Latin letters, when the lyrics have it.")
                }
            } header: {
                Text("Lyrics")
            } footer: {
                Text("Lyrics come from the song's tags or from an .lrc, .ttml or .vtt file with the same name beside it. Jellyfesh never transliterates: romanization shows only when the lyrics carry it.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }

            Section {
                AppIconPicker()
            } header: {
                Text("App Icon")
            } footer: {
                Text("Automatic uses Ice in light appearance and Steel in dark appearance. Other choices change the Dock icon while Jellyfesh is open; Finder keeps Ice.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
}

/// System, Light and Dark as small pictures of the window; the chosen one has an accent ring.
private struct ThemeChoices: View {
    var selection: AppearanceOption
    var choose: (AppearanceOption) -> Void

    @Environment(ThemeStore.self) private var theme
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            ForEach(AppearanceOption.allCases, id: \.self) { option in
                Button {
                    choose(option)
                } label: {
                    VStack(spacing: Tokens.Space.s1) {
                        ZStack {
                            switch option {
                            case .light:
                                MiniWindow(palette: theme.palette(dark: false))
                            case .dark:
                                MiniWindow(palette: theme.palette(dark: true))
                            case .system:
                                MiniWindow(palette: theme.palette(dark: false))
                                MiniWindow(palette: theme.palette(dark: true))
                                    .mask { DiagonalHalf() }
                            }
                        }
                        .frame(width: 72, height: 48)
                        .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .padding(3)
                        .overlay {
                            RoundedRectangle(cornerRadius: 10, style: .continuous)
                                .strokeBorder(selection == option ? palette[Tokens.Accent.accent] : palette[Tokens.Line.lineSoft],
                                              lineWidth: selection == option ? 2 : 1)
                        }
                        Text(Self.title(option))
                            .font(.system(size: 11, weight: selection == option ? .semibold : .regular))
                            .foregroundStyle(selection == option ? palette[Tokens.Accent.accentText] : palette[Tokens.Ink.ink70])
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(Self.title(option)))
                .accessibilityAddTraits(selection == option ? [.isButton, .isSelected] : .isButton)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Theme"))
    }

    static func title(_ option: AppearanceOption) -> LocalizedStringKey {
        switch option {
        case .system: return "System"
        case .light: return "Light"
        case .dark: return "Dark"
        }
    }
}

/// A window in one appearance, drawn small: the field, a sidebar, lines of text and the accent.
private struct MiniWindow: View {
    var palette: Palette

    var body: some View {
        HStack(spacing: 0) {
            Rectangle()
                .fill(palette[Tokens.Glass.cloud])
                .frame(width: 18)
            VStack(alignment: .leading, spacing: 4) {
                Capsule().fill(palette[Tokens.Ink.ink70]).frame(width: 26, height: 4)
                Capsule().fill(palette[Tokens.Ink.ink30]).frame(width: 34, height: 3)
                Capsule().fill(palette[Tokens.Ink.ink30]).frame(width: 30, height: 3)
                Spacer(minLength: 0)
                HStack {
                    Spacer(minLength: 0)
                    Circle().fill(palette[Tokens.Accent.accent]).frame(width: 9, height: 9)
                }
            }
            .padding(6)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .background(palette[Tokens.Field.sky])
        }
        .accessibilityHidden(true)
    }
}

/// The ten accent swatches (`AccentChoices.swatchHues`), each a 44 pt target named Blue … Violet.
/// The ring slides to the chosen one (`matchedGeometryEffect`).
private struct AccentSwatches: View {
    var selected: Int?
    var choose: (Float) -> Void

    @Environment(\.palette) private var palette
    @Environment(\.effects) private var effects
    @Namespace private var namespace

    var body: some View {
        let names = AccentChoices.swatchNames
        HStack(spacing: 0) {
            ForEach(Array(AccentChoices.swatchHues.enumerated()), id: \.offset) { index, hue in
                let chosen = selected == index
                let name = index < names.count ? names[index] : ""
                Button {
                    withAnimation(Motion.unless(!effects.animates, Motion.move)) {
                        choose(hue)
                    }
                } label: {
                    Circle()
                        .fill(Color(rgba: AccentChoices.accent(hue: hue, dark: palette.isDark)))
                        .frame(width: 26, height: 26)
                        .padding(3)
                        .overlay {
                            if chosen {
                                Circle()
                                    .strokeBorder(palette[Tokens.Ink.ink90], lineWidth: 2)
                                    .matchedGeometryEffect(id: "ring", in: namespace)
                            }
                        }
                        .frame(width: 44, height: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(PressScaleStyle(scale: 0.92))
                .accessibilityLabel(Text(name))
                .accessibilityAddTraits(chosen ? [.isButton, .isSelected] : .isButton)
                .help(Text(name))
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Accent Colour"))
    }
}

/// A hue from 0 to 360 on a track of the accent at every hue; Left and Right step 1°, Page Up and
/// Page Down 30°. The value goes to `commit` when the drag ends or a key steps it.
private struct HueSlider: View {
    @Binding var hue: Double
    var commit: (Double) -> Void

    @Environment(\.palette) private var palette
    @FocusState private var focused: Bool

    private static let inset: CGFloat = 10

    var body: some View {
        GeometryReader { geometry in
            let length = max(1, geometry.size.width - 2 * Self.inset)
            let x = Self.inset + CGFloat(min(max(hue, 0), 360) / 360) * length
            ZStack(alignment: .topLeading) {
                Capsule()
                    .fill(LinearGradient(colors: AccentChoices.hueTrack(dark: palette.isDark).map { Color(rgba: $0) },
                                         startPoint: .leading, endPoint: .trailing))
                    .frame(width: length, height: 8)
                    .position(x: geometry.size.width / 2, y: geometry.size.height / 2)
                Circle()
                    .fill(Color(rgba: AccentChoices.accent(hue: Float(hue), dark: palette.isDark)))
                    .overlay(Circle().strokeBorder(Color.white, lineWidth: 2))
                    .elevation(Tokens.Elevation.shadow1)
                    .frame(width: 20, height: 20)
                    .position(x: x, y: geometry.size.height / 2)
            }
            .contentShape(Rectangle())
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        hue = Double(min(max((value.location.x - Self.inset) / length, 0), 1)) * 360
                    }
                    .onEnded { _ in commit(hue) }
            )
        }
        .frame(height: 28)
        .frame(maxWidth: 280)
        .focusable()
        .focused($focused)
        .onKeyPress(.leftArrow) { step(-1) }
        .onKeyPress(.rightArrow) { step(1) }
        .onKeyPress(.pageDown) { step(-30) }
        .onKeyPress(.pageUp) { step(30) }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text("Hue"))
        .accessibilityValue(Text(verbatim: "\(Int(hue.rounded()))°"))
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: _ = step(1)
            case .decrement: _ = step(-1)
            @unknown default: break
            }
        }
    }

    private func step(_ degrees: Double) -> KeyPress.Result {
        hue = min(max(hue.rounded() + degrees, 0), 360)
        commit(hue)
        return .handled
    }
}

/// The accent at a hue, with its value.
private struct AccentPreview: View {
    var hue: Float
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s2) {
            Image(systemName: "play.fill")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(palette.isDark ? Color.black.opacity(0.85) : Color.white)
                .frame(width: 28, height: 28)
                .background(Circle().fill(Color(rgba: AccentChoices.accent(hue: hue, dark: palette.isDark))))
            Text(verbatim: "\(Int(AccentChoices.normalize(hue).rounded()))°")
                .monospacedDigit()
                .frame(width: 36, alignment: .trailing)
        }
        .accessibilityHidden(true)
    }
}

/// The transparency at work: a player capsule of Veil glass over the Misty field, drawn with the
/// value being chosen.
private struct TransparencyPreview: View {
    var transparency: Double
    var reduceEffects: Bool

    @Environment(\.effects) private var effects
    @Environment(\.palette) private var palette

    var body: some View {
        var preview = effects
        preview.transparency = transparency
        preview.solidGlass = effects.solidGlass || transparency <= 0.001
        return ZStack {
            MistyField()
            HStack(spacing: Tokens.Space.s2) {
                RoundedRectangle(cornerRadius: Tokens.Radius.rSm, style: .continuous)
                    .fill(palette[Tokens.Accent.accentSoft])
                    .frame(width: 30, height: 30)
                VStack(alignment: .leading, spacing: 4) {
                    Capsule().fill(palette[Tokens.Ink.ink90]).frame(width: 80, height: 5)
                    Capsule().fill(palette[Tokens.Ink.ink50]).frame(width: 56, height: 4)
                }
                Spacer(minLength: 0)
                Image(systemName: "play.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(palette[Tokens.Ink.ink90])
            }
            .padding(.horizontal, 10)
            .frame(width: 240, height: 48)
            .glass(.veil, in: RoundedRectangle(cornerRadius: SeekGeometry.miniRadius, style: .continuous))
            .elevation(Tokens.Elevation.shadow1)
            .environment(\.effects, preview)
        }
        .frame(height: 96)
        .clipShape(RoundedRectangle(cornerRadius: Tokens.Radius.rMd, style: .continuous))
        .accessibilityHidden(true)
    }
}

/// Artwork and Misty as small pictures of Now Playing; the chosen one has an accent ring.
private struct BackdropChoices: View {
    var selection: BackdropOption
    var choose: (BackdropOption) -> Void

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Tokens.Space.s3) {
            ForEach(BackdropOption.allCases, id: \.self) { option in
                Button {
                    choose(option)
                } label: {
                    VStack(spacing: Tokens.Space.s1) {
                        preview(option)
                            .frame(width: 72, height: 48)
                            .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
                            .padding(3)
                            .overlay {
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .strokeBorder(selection == option ? palette[Tokens.Accent.accent] : palette[Tokens.Line.lineSoft],
                                                  lineWidth: selection == option ? 2 : 1)
                            }
                        Text(option == .artwork ? LocalizedStringKey("Artwork") : LocalizedStringKey("Misty"))
                            .font(.system(size: 11, weight: selection == option ? .semibold : .regular))
                            .foregroundStyle(selection == option ? palette[Tokens.Accent.accentText] : palette[Tokens.Ink.ink70])
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(option == .artwork ? Text("Artwork") : Text("Misty"))
                .accessibilityAddTraits(selection == option ? [.isButton, .isSelected] : .isButton)
            }
        }
    }

    /// Artwork: the cover blurred into the background. Misty: the lit field with the cover framed.
    @ViewBuilder private func preview(_ option: BackdropOption) -> some View {
        let cover = RoundedRectangle(cornerRadius: 4, style: .continuous)
        if option == .artwork {
            ZStack {
                LinearGradient(colors: [palette[Tokens.Field.aurora1], palette[Tokens.Accent.accent],
                                        palette[Tokens.Field.aurora2]],
                               startPoint: .topLeading, endPoint: .bottomTrailing)
                    .blur(radius: 6)
                cover.fill(palette[Tokens.Accent.accent]).frame(width: 22, height: 22)
                    .shadow(color: .black.opacity(0.25), radius: 3, y: 2)
            }
        } else {
            ZStack {
                MistyField()
                cover.fill(palette[Tokens.Accent.accent]).frame(width: 20, height: 20)
                    .padding(3)
                    .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(palette[Tokens.Glass.tintClear]))
            }
        }
    }
}

/// Automatic and the ten icons as tiles; the chosen one has a ring and a check.
struct AppIconPicker: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(\.palette) private var palette

    private let columns = [GridItem(.adaptive(minimum: 84, maximum: 96), spacing: 14, alignment: .top)]

    var body: some View {
        let selected = AppIconChoice(rawValue: settings.current.appIcon) ?? .automatic
        LazyVGrid(columns: columns, alignment: .leading, spacing: 18) {
            ForEach(AppIconChoice.allCases) { choice in
                Button {
                    settings.setAppIcon(choice.rawValue)
                } label: {
                    VStack(spacing: Tokens.Space.s2) {
                        tile(choice, selected: choice == selected)
                        Text(choice.title)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(choice == selected ? palette[Tokens.Accent.accentText] : palette[Tokens.Ink.ink70])
                            .lineLimit(1)
                            .minimumScaleFactor(0.8)
                    }
                    .frame(width: 88)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(choice.title))
                .accessibilityAddTraits(choice == selected ? [.isButton, .isSelected] : .isButton)
            }
        }
        .padding(.vertical, Tokens.Space.s2)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("App Icon"))
    }

    @ViewBuilder private func tile(_ choice: AppIconChoice, selected: Bool) -> some View {
        let shape = RoundedRectangle(cornerRadius: 14, style: .continuous)
        ZStack {
            if choice == .automatic {
                // Ice and Steel split along the diagonal, as Automatic switches between them.
                Image("IconThumb-ice").resizable()
                Image("IconThumb-steel").resizable()
                    .mask { DiagonalHalf() }
            } else {
                Image(choice.thumbnailName).resizable()
            }
        }
        .frame(width: 64, height: 64)
        .clipShape(shape)
        .shadow(color: .black.opacity(0.18), radius: 1.5, y: 1)
        .padding(4)
        .overlay {
            if selected {
                RoundedRectangle(cornerRadius: 17, style: .continuous)
                    .strokeBorder(palette[Tokens.Accent.accent], lineWidth: 2)
            }
        }
        .overlay(alignment: .topTrailing) {
            if selected {
                Image(systemName: "checkmark")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(palette[Tokens.Accent.onAccent])
                    .frame(width: 20, height: 20)
                    .background(Circle().fill(palette[Tokens.Accent.accent]))
                    .offset(x: 4, y: -4)
                    .accessibilityHidden(true)
            }
        }
    }
}

/// The lower right half of a square, for the Automatic tile.
private struct DiagonalHalf: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: rect.maxX, y: rect.minY))
        path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
        path.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
        path.closeSubpath()
        return path
    }
}
