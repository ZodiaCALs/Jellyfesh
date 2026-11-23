// The v0.0.4 design pass's numbers (docs/design-controls.md, "v0.0.4: the controls as built"):
// the straight seek bar's geometry, the player bar's breakpoint, Now Playing's motion, the accent
// swatches and how the transparency setting picks a material; and Luminous (v0.0.6, docs/ui.md):
// the field's four glows and the floating player's geometry. Pure functions, so views only draw
// them and Tests/DesignTests.swift checks them (the same rules as the Windows and Android apps).

import Foundation

/// What the person is doing with the seek bar (or the volume line in its style).
enum SeekBarState: Equatable {
    /// Nothing: a thin line, no thumb.
    case rest
    /// Hovered or focused: the line thickens and the thumb shows.
    case hover
    /// Being dragged: the line thickens further and the times follow the thumb.
    case drag
}

/// The seek bar's keyboard steps.
enum SeekKey: Equatable {
    case back, forward, pageBack, pageForward, start, end
}

/// The straight seek bar: a line with round caps that thickens on hover and while dragging, a
/// 14 pt thumb on hover, focus and drag (Media Player), the track inset so the thumb stays inside;
/// the mini player's 2 pt progress line; the width from which the player bar replaces the capsule.
enum SeekGeometry {
    /// `player.scrubber-track`, `-hover`, `-drag` and `player.scrubber-thumb`.
    static let track: CGFloat = 4
    static let trackHover: CGFloat = 6
    static let trackDrag: CGFloat = 8
    static let thumb: CGFloat = 14
    /// `player.mini-progress` and `player.mini-radius`.
    static let miniProgress: CGFloat = 2
    static let miniRadius: CGFloat = 16
    /// `player.bar-height`.
    static let barHeight: CGFloat = 80
    /// Windows this wide and wider show the player bar instead of the capsule (as the web's viewport).
    static let playerBarMinWidth: CGFloat = 900
    /// Left and Right seek 5 s; Page Up and Page Down 30 s.
    static let smallStepUs: Int64 = 5_000_000
    static let largeStepUs: Int64 = 30_000_000

    static func thickness(_ state: SeekBarState) -> CGFloat {
        switch state {
        case .rest: return track
        case .hover: return trackHover
        case .drag: return trackDrag
        }
    }

    static func showsThumb(_ state: SeekBarState) -> Bool { state != .rest }

    /// Room at each end for the thumb.
    static var inset: CGFloat { thumb / 2 }

    static func trackLength(width: CGFloat) -> CGFloat { max(0, width - 2 * inset) }

    /// The fraction of the track at `x`, clamped to 0...1 and mirrored right to left.
    static func fraction(atX x: CGFloat, width: CGFloat, rightToLeft: Bool = false) -> Double {
        let length = trackLength(width: width)
        guard length > 0, !x.isNaN else { return 0 }
        let f = Double(min(max((x - inset) / length, 0), 1))
        return rightToLeft ? 1 - f : f
    }

    static func playedLength(fraction: Double, width: CGFloat) -> CGFloat {
        CGFloat(clamp01(fraction)) * trackLength(width: width)
    }

    /// The thumb's centre.
    static func thumbCentre(fraction: Double, width: CGFloat, rightToLeft: Bool = false) -> CGFloat {
        let played = playedLength(fraction: fraction, width: width)
        return rightToLeft ? width - inset - played : inset + played
    }

    /// Where a key moves the position (µs), within the track.
    static func keyTarget(_ key: SeekKey, positionUs: Int64, durationUs: Int64) -> Int64 {
        let target: Int64
        switch key {
        case .back: target = positionUs - smallStepUs
        case .forward: target = positionUs + smallStepUs
        case .pageBack: target = positionUs - largeStepUs
        case .pageForward: target = positionUs + largeStepUs
        case .start: target = 0
        case .end: target = durationUs
        }
        return min(max(target, 0), max(durationUs, 0))
    }

    /// The mini player's line runs along the capsule's bottom edge between its corners.
    static func miniTrackLength(width: CGFloat) -> CGFloat { max(0, width - 2 * miniRadius) }

    static func usesPlayerBar(width: CGFloat) -> Bool { width >= playerBarMinWidth }

    private static func clamp01(_ value: Double) -> Double { value.isNaN ? 0 : min(max(value, 0), 1) }
}

/// Where a lyric line is relative to the line being sung.
enum LyricLineRole: Equatable {
    /// Not yet sung: `lyrics.ink-inactive`.
    case upcoming
    /// Being sung: ink-90 at full size.
    case active
    /// Sung: ink-30.
    case sung
}

/// Now Playing's motion after Apple Music, as numbers SwiftUI animates.
enum NowPlayingMotion {
    /// `player.artwork-paused-scale`, `lyrics.scale-inactive` (Luminous: 92 %), `-small`, `-large`.
    static let artworkPausedScale: CGFloat = 0.88
    static let inactiveLineScale: CGFloat = 0.92
    static let smallLyrics: CGFloat = 0.8
    static let largeLyrics: CGFloat = 1.2
    /// `lyrics.stagger` (Luminous: 40 ms): each following line starts this much after the one above.
    static let staggerMs = 40
    /// Lines further below the active one start together, so the cascade never drags.
    static let maxStaggeredLines = 8

    /// 1 while playing, 0.88 paused; full size under reduced motion.
    static func artworkScale(playing: Bool, reduceMotion: Bool) -> CGFloat {
        playing || reduceMotion ? 1 : artworkPausedScale
    }

    static func lyricsScale(_ size: LyricsSizeOption) -> CGFloat {
        switch size {
        case .small: return smallLyrics
        case .medium: return 1
        case .large: return largeLyrics
        }
    }

    static func role(index: Int, active: Int?) -> LyricLineRole {
        guard let active else { return .upcoming }
        if index == active { return .active }
        return index < active ? .sung : .upcoming
    }

    static func lineScale(_ role: LyricLineRole) -> CGFloat { role == .active ? 1 : inactiveLineScale }

    /// How much later than the scroll line `index` starts moving (ms).
    static func cascadeDelayMs(index: Int, active: Int?) -> Int {
        guard let active, index > active else { return 0 }
        return staggerMs * min(index - active, maxStaggeredLines)
    }

    /// The extra offset that makes a line start `delayMs` after a scroll of `distance` running
    /// `durationMs` on `curve`: distance·(E(t/D) − E((t−delay)/D)), at the fractions `at` of the
    /// delay plus the duration (0 at both ends).
    static func cascadeLag(distance: CGFloat, delayMs: Int, durationMs: Int, curve: CubicBezier,
                           at fractions: [Double]) -> [CGFloat] {
        let d = Double(max(durationMs, 1))
        let lag = Double(max(delayMs, 0))
        let total = d + lag
        return fractions.map { p in
            guard p < 1 else { return 0 }
            let t = total * p
            return distance * CGFloat(ease(curve, t / d) - ease(curve, (t - lag) / d))
        }
    }

    /// The cascade as five linear keyframes (points, seconds) for a line starting `delayMs` after
    /// the scroll: samples at half the delay, the delay, and 15 % and 40 % of the scroll after it,
    /// then 0 when it ends. Empty without a delay or a distance.
    static func cascadeKeyframes(distance: CGFloat, delayMs: Int, durationMs: Int,
                                 curve: CubicBezier) -> [(value: CGFloat, seconds: Double)] {
        guard delayMs > 0, distance > 0, durationMs > 0 else { return [] }
        let d = Double(durationMs)
        let lag = Double(delayMs)
        let total = d + lag
        let times = [lag * 0.5, lag, lag + 0.15 * d, lag + 0.4 * d, total]
        let values = cascadeLag(distance: distance, delayMs: delayMs, durationMs: durationMs, curve: curve,
                                at: times.map { $0 / total })
        var frames: [(value: CGFloat, seconds: Double)] = []
        var previous = 0.0
        for (value, time) in zip(values, times) {
            frames.append((value, (time - previous) / 1000))
            previous = time
        }
        return frames
    }

    /// The eased progress of a CSS cubic-bezier curve at time fraction `x`.
    static func ease(_ c: CubicBezier, _ x: Double) -> Double {
        if x <= 0 { return 0 }
        if x >= 1 { return 1 }
        func sample(_ p1: Double, _ p2: Double, _ t: Double) -> Double {
            let u = 1 - t
            return 3 * u * u * t * p1 + 3 * u * t * t * p2 + t * t * t
        }
        var lo = 0.0, hi = 1.0, t = x
        for _ in 0..<24 {
            t = (lo + hi) / 2
            if sample(Double(c.x1), Double(c.x2), t) < x { lo = t } else { hi = t }
        }
        return sample(Double(c.y1), Double(c.y2), t)
    }
}

/// Luminous's field (docs/ui.md, "Luminous"): four large aurora glows at the theme hue, as the
/// owner's mock-up draws them at `field.aurora-strength` 0.75 (Misty's field is about 0.45).
enum LuminousField {
    /// One glow: an ellipse at (`x`, `y`) in the field's unit square, `radius` across in the same
    /// units (so it stretches with the field), fading from its colour at `baseAlpha` to nothing
    /// at `fadeEnd` of the radius.
    struct Glow: Equatable {
        var color: ThemeColorKey
        var x: CGFloat
        var y: CGFloat
        var radius: CGFloat
        var baseAlpha: Double
        var fadeEnd: CGFloat

        /// The glow's alpha at an aurora strength: `baseAlpha` at the reference strength, within 0...1.
        func alpha(strength: Double) -> Double {
            guard strength.isFinite else { return baseAlpha }
            return min(max(baseAlpha * strength / LuminousField.referenceStrength, 0), 1)
        }
    }

    /// The strength the base alphas are drawn at (the token's value when the owner chose it).
    static let referenceStrength = 0.75

    /// Top start, bottom end, bottom start and a smaller one top end.
    static let glows: [Glow] = [
        Glow(color: Tokens.Field.aurora1, x: 0.04, y: 0, radius: 0.60, baseAlpha: 0.80, fadeEnd: 0.72),
        Glow(color: Tokens.Field.aurora2, x: 1, y: 0.96, radius: 0.61, baseAlpha: 0.72, fadeEnd: 0.72),
        Glow(color: Tokens.Field.aurora3, x: 0, y: 1, radius: 0.48, baseAlpha: 0.66, fadeEnd: 0.70),
        Glow(color: Tokens.Field.aurora1, x: 0.92, y: 0.04, radius: 0.39, baseAlpha: 0.46, fadeEnd: 0.70),
    ]
}

/// Luminous's floating player and Now Playing stage (docs/ui.md, "Luminous").
enum LuminousGeometry {
    /// The player bar: a floating Veil capsule (`r-2xl` corners) `player.bar-height` high.
    static let barRadius: CGFloat = 24
    /// The gap between the floating player and the window's edges.
    static let playerMargin: CGFloat = 16
    /// Content fades out over this height just above the floating player.
    static let fadeHeight: CGFloat = 48
    /// Now Playing's dock: one floating Veil capsule with every control.
    static let dockHeight: CGFloat = 80
    static let dockRadius: CGFloat = 40
    /// Now Playing's artwork, the stage's hero.
    static let stageArtwork: CGFloat = 380
    /// Grid covers lift this much on hover or focus.
    static let coverLift: CGFloat = 4

    /// The player at the bottom of the window for the look's style (ADR-0020).
    enum BottomPlayer: Equatable {
        /// The player bar (art, transport, seek bar, utilities).
        case bar
        /// The mini player: art, two lines and Play.
        case mini
        /// The mini player as a smaller capsule at the trailing corner.
        case floating
    }

    /// Bar is v0.0.6: the player bar in wide columns, the mini player in narrow ones. Compact is
    /// the mini player at every width; Floating, the smaller capsule.
    static func bottomPlayer(style: LookMiniPlayerStyle, wide: Bool) -> BottomPlayer {
        switch style {
        case .bar: return wide ? .bar : .mini
        case .compact: return .mini
        case .floating: return .floating
        }
    }

    /// How Now Playing arranges itself for the look's layout and the chosen pane (ADR-0020).
    enum NowPlayingArrangement: Equatable {
        /// The v0.0.6 stage: the artwork beside the pane.
        case stage
        /// A large cover with the line being sung below it.
        case cover
        /// The pane across the window, a small cover in the corner.
        case widePane
    }

    /// Stage is v0.0.6. Artwork shows the cover and the sung line in place of the Lyrics pane, and
    /// Queue and Info beside the cover as on the stage. Lyrics puts any pane across the window. In a
    /// narrow window (the phone layout, ADR-0023) the stage would leave the pane too little room:
    /// the pane goes across it with the small cover, except the Artwork layout's cover.
    static func nowPlayingArrangement(_ layout: LookNowPlayingLayout, panel: NowPlayingPanel,
                                      narrow: Bool = false) -> NowPlayingArrangement {
        let arrangement: NowPlayingArrangement
        switch layout {
        case .stage: arrangement = .stage
        case .artwork: arrangement = panel == .lyrics ? .cover : .stage
        case .lyrics: arrangement = .widePane
        }
        return narrow && arrangement == .stage ? .widePane : arrangement
    }

    /// The cover in the Artwork layout: up to 520 pt, as the window allows (the top bar, the dock,
    /// the title and the sung line take about 420 pt of the height), never under 180.
    static func coverArtworkSide(width: CGFloat, height: CGFloat) -> CGFloat {
        let side = min(520, width * 0.5, height - 420)
        guard side.isFinite else { return 380 }
        return max(side, 180).rounded()
    }

    /// The height the floating player takes at the bottom of the content, its margin included;
    /// 0 without a player.
    static func playerZone(hasPlayer: Bool, wide: Bool) -> CGFloat {
        guard hasPlayer else { return 0 }
        return (wide ? SeekGeometry.barHeight : Tokens.Player.miniHeight) + playerMargin
    }

    /// The artwork on the stage: 380 pt, smaller when the window leaves less room (the top bar, the
    /// dock and the title under the artwork take about 370 pt of the height).
    static func stageArtworkSide(width: CGFloat, height: CGFloat) -> CGFloat {
        let byWidth = width * 0.34
        let byHeight = height - 370
        let side = min(stageArtwork, byWidth, byHeight)
        guard side.isFinite else { return stageArtwork }
        return max(side, 180).rounded()
    }
}

/// OKLCH to 8-bit sRGB as jf_palette converts (CSS Color 4 matrices, channels clipped, rounded
/// as floor(v·255 + 0.5)), so a swatch shows the accent the core resolves for its hue.
enum Oklch {
    private static let oklabToLms: [[Double]] = [
        [1.0, 0.3963377773761749, 0.2158037573099136],
        [1.0, -0.1055613458156586, -0.0638541728258133],
        [1.0, -0.0894841775298119, -1.2914855480194092],
    ]
    private static let lmsToXyz: [[Double]] = [
        [1.2268798758459243, -0.5578149944602171, 0.2813910456659647],
        [-0.0405757452148008, 1.112286803280317, -0.0717110580655164],
        [-0.0763729366746601, -0.4214933324022432, 1.5869240198367816],
    ]
    private static let xyzToLinearSrgb: [[Double]] = [
        [12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0],
        [-851781.0 / 878810.0, 1648619.0 / 878810.0, 36519.0 / 878810.0],
        [705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0],
    ]

    /// oklch(l c hue), opaque, as 0xRRGGBBAA (the core's colour format).
    static func rgba(l: Double, c: Double, hue: Double) -> UInt32 {
        let h = hue * .pi / 180
        let lms = multiply(oklabToLms, [l, c * cos(h), c * sin(h)]).map { $0 * $0 * $0 }
        let rgb = multiply(xyzToLinearSrgb, multiply(lmsToXyz, lms)).map { quantize(encode($0)) }
        return (rgb[0] << 24) | (rgb[1] << 16) | (rgb[2] << 8) | 0xFF
    }

    private static func multiply(_ m: [[Double]], _ v: [Double]) -> [Double] {
        m.map { row in row[0] * v[0] + row[1] * v[1] + row[2] * v[2] }
    }

    private static func encode(_ v: Double) -> Double {
        let a = abs(v)
        let encoded = a > 0.0031308 ? 1.055 * pow(a, 1 / 2.4) - 0.055 : 12.92 * a
        return min(max(v < 0 ? -encoded : encoded, 0), 1)
    }

    private static func quantize(_ v: Double) -> UInt32 { UInt32((min(max(v, 0), 1) * 255 + 0.5).rounded(.down)) }
}

/// Preferences → Appearance → Accent colour: follow the artwork, ten swatches or a custom hue.
enum AccentChoices {
    /// The swatches' hues in the order shown: Misty blue first, then round the wheel.
    static let swatchHues: [Float] = [250, 210, 180, 150, 120, 60, 30, 0, 330, 290]

    /// The swatches' names in `swatchHues` order, as every app names them (docs/ui.md: the last
    /// one is Violet).
    static var swatchNames: [String] {
        [
            String(localized: "Blue"), String(localized: "Cyan"), String(localized: "Teal"),
            String(localized: "Green"), String(localized: "Olive"), String(localized: "Orange"),
            String(localized: "Red"), String(localized: "Rose"), String(localized: "Magenta"),
            String(localized: "Violet"),
        ]
    }

    /// A hue in degrees, normalised to 0..<360 (NaN counts as Misty's 250).
    static func normalize(_ hue: Float) -> Float {
        guard hue.isFinite else { return 250 }
        let h = hue.truncatingRemainder(dividingBy: 360)
        let positive = h < 0 ? h + 360 : h
        return positive >= 360 ? 0 : positive
    }

    /// The chosen swatch, or nil for the artwork's hue or a custom one.
    static func swatchIndex(_ source: HueSourceOption) -> Int? {
        guard case .fixed(let fixed) = source else { return nil }
        let hue = normalize(fixed)
        return swatchHues.firstIndex { swatch in
            let d = abs(hue - swatch)
            return min(d, 360 - d) < 0.5
        }
    }

    /// The accent at `hue`: oklch(0.46 0.15 h) light, oklch(0.78 0.12 h) dark.
    static func accent(hue: Float, dark: Bool) -> UInt32 {
        let h = Double(normalize(hue))
        return dark ? Oklch.rgba(l: 0.78, c: 0.12, hue: h) : Oklch.rgba(l: 0.46, c: 0.15, hue: h)
    }

    /// The custom hue slider's track: the accent every 30° (13 stops, red at both ends).
    static func hueTrack(dark: Bool) -> [UInt32] {
        (0...12).map { accent(hue: Float($0 * 30 % 360), dark: dark) }
    }
}

/// How the transparency setting (0 solid … 1 clearest) dresses a glass grade on macOS, whose
/// materials have no alpha: the nearest material thickness, plus the tint's alpha from the shared
/// glass-alpha port (`GlassTransparency`). At 0 % the glass is solid Cloud.
enum GlassLook {
    /// The system materials from thinnest to thickest.
    enum Thickness: Int, CaseIterable, Equatable {
        case ultraThin, thin, regular, thick, ultraThick
    }

    /// The material for a grade whose usual one is `base`, or nil for solid Cloud.
    static func thickness(base: Thickness, transparency: Double) -> Thickness? {
        let t = transparency.isFinite ? min(max(transparency, 0), 1) : 0.5
        if t <= 0.001 { return nil }
        let shift = t < 0.25 ? 1 : (t >= 0.75 ? -1 : 0)
        let index = min(max(base.rawValue + shift, 0), Thickness.allCases.count - 1)
        return Thickness(rawValue: index)
    }
}
