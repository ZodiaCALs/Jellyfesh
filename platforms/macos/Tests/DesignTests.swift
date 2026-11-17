// The v0.0.4 design pass (docs/design-controls.md, "v0.0.4: the controls as built"): the straight
// seek bar's geometry, the player bar's breakpoint, Now Playing's motion numbers, the accent
// swatches and how the transparency setting picks a material. The same cases as the Windows and
// Android apps' design tests.

import XCTest
@testable import Jellyfesh

final class DesignTests: XCTestCase {
    /// design/tokens.json, found from this file's location in the repository.
    private func tokens() throws -> [String: Any] {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { root.deleteLastPathComponent() } // DesignTests.swift, Tests, macos, platforms
        let data = try Data(contentsOf: root.appendingPathComponent("design/tokens.json"))
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    func testGeometryConstantsAreTheTokens() {
        XCTAssertEqual(SeekGeometry.track, Tokens.Player.scrubberTrack)
        XCTAssertEqual(SeekGeometry.trackHover, Tokens.Player.scrubberTrackHover)
        XCTAssertEqual(SeekGeometry.trackDrag, Tokens.Player.scrubberTrackDrag)
        XCTAssertEqual(SeekGeometry.thumb, Tokens.Player.scrubberThumb)
        XCTAssertEqual(SeekGeometry.miniProgress, Tokens.Player.miniProgress)
        XCTAssertEqual(SeekGeometry.miniRadius, Tokens.Player.miniRadius)
        XCTAssertEqual(SeekGeometry.barHeight, Tokens.Player.barHeight)
        XCTAssertEqual(Double(NowPlayingMotion.artworkPausedScale), Tokens.Player.artworkPausedScale, accuracy: 1e-9)
        XCTAssertEqual(Double(NowPlayingMotion.inactiveLineScale), Tokens.Lyrics.scaleInactive, accuracy: 1e-9)
        XCTAssertEqual(Double(NowPlayingMotion.smallLyrics), Tokens.Lyrics.scaleSmall, accuracy: 1e-9)
        XCTAssertEqual(Double(NowPlayingMotion.largeLyrics), Tokens.Lyrics.scaleLarge, accuracy: 1e-9)
        XCTAssertEqual(NowPlayingMotion.staggerMs, Tokens.Lyrics.staggerMs)
    }

    func testHoverAndDragThickenTheLineAndShowTheThumb() {
        XCTAssertEqual(SeekGeometry.thickness(.rest), 4)
        XCTAssertEqual(SeekGeometry.thickness(.hover), 6)
        XCTAssertEqual(SeekGeometry.thickness(.drag), 8)
        XCTAssertFalse(SeekGeometry.showsThumb(.rest))
        XCTAssertTrue(SeekGeometry.showsThumb(.hover))
        XCTAssertTrue(SeekGeometry.showsThumb(.drag))
    }

    func testTheTrackIsInsetAndMirroredRightToLeft() {
        // 214 pt: 7 pt at each end for the 14 pt thumb, a 200 pt line.
        XCTAssertEqual(SeekGeometry.trackLength(width: 214), 200)
        XCTAssertEqual(SeekGeometry.fraction(atX: 57, width: 214), 0.25, accuracy: 1e-9)
        XCTAssertEqual(SeekGeometry.fraction(atX: 57, width: 214, rightToLeft: true), 0.75, accuracy: 1e-9)
        XCTAssertEqual(SeekGeometry.fraction(atX: 999, width: 214), 1)
        XCTAssertEqual(SeekGeometry.fraction(atX: .nan, width: 214), 0)
        XCTAssertEqual(SeekGeometry.fraction(atX: 50, width: 10), 0, "no room for a track")
        XCTAssertEqual(SeekGeometry.playedLength(fraction: 0.25, width: 214), 50)
        XCTAssertEqual(SeekGeometry.playedLength(fraction: .nan, width: 214), 0)
        XCTAssertEqual(SeekGeometry.thumbCentre(fraction: 0.25, width: 214), 57)
        XCTAssertEqual(SeekGeometry.thumbCentre(fraction: 0.25, width: 214, rightToLeft: true), 157)
    }

    func testKeysStepFiveAndThirtySecondsWithinTheTrack() {
        let s: Int64 = 1_000_000
        XCTAssertEqual(SeekGeometry.keyTarget(.back, positionUs: 60 * s, durationUs: 225 * s), 55 * s)
        XCTAssertEqual(SeekGeometry.keyTarget(.forward, positionUs: 223 * s, durationUs: 225 * s), 225 * s)
        XCTAssertEqual(SeekGeometry.keyTarget(.pageForward, positionUs: 60 * s, durationUs: 225 * s), 90 * s)
        XCTAssertEqual(SeekGeometry.keyTarget(.pageBack, positionUs: 10 * s, durationUs: 225 * s), 0)
        XCTAssertEqual(SeekGeometry.keyTarget(.start, positionUs: 60 * s, durationUs: 225 * s), 0)
        XCTAssertEqual(SeekGeometry.keyTarget(.end, positionUs: 60 * s, durationUs: 225 * s), 225 * s)
    }

    func testTheMiniLineRunsBetweenTheCornersAndTheBarTakesOverAt900() {
        XCTAssertEqual(SeekGeometry.miniTrackLength(width: 360), 328)
        XCTAssertEqual(SeekGeometry.miniTrackLength(width: 20), 0)
        XCTAssertFalse(SeekGeometry.usesPlayerBar(width: 899))
        XCTAssertTrue(SeekGeometry.usesPlayerBar(width: 900))
    }

    func testNowPlayingMotionFollowsTheTokens() {
        XCTAssertEqual(NowPlayingMotion.artworkScale(playing: false, reduceMotion: false), 0.88)
        XCTAssertEqual(NowPlayingMotion.artworkScale(playing: false, reduceMotion: true), 1)
        XCTAssertEqual(NowPlayingMotion.artworkScale(playing: true, reduceMotion: false), 1)
        XCTAssertEqual(NowPlayingMotion.lyricsScale(.small), 0.8)
        XCTAssertEqual(NowPlayingMotion.lyricsScale(.medium), 1)
        XCTAssertEqual(NowPlayingMotion.lyricsScale(.large), 1.2)
        XCTAssertEqual(NowPlayingMotion.role(index: 1, active: 3), .sung)
        XCTAssertEqual(NowPlayingMotion.role(index: 3, active: 3), .active)
        XCTAssertEqual(NowPlayingMotion.role(index: 4, active: 3), .upcoming)
        XCTAssertEqual(NowPlayingMotion.role(index: 0, active: nil), .upcoming)
        // Luminous: inactive lines at 92 %, the cascade 40 ms apart.
        XCTAssertEqual(NowPlayingMotion.lineScale(.upcoming), 0.92)
        XCTAssertEqual(NowPlayingMotion.lineScale(.sung), 0.92)
        XCTAssertEqual(NowPlayingMotion.lineScale(.active), 1)
        XCTAssertEqual(NowPlayingMotion.cascadeDelayMs(index: 3, active: 3), 0)
        XCTAssertEqual(NowPlayingMotion.cascadeDelayMs(index: 2, active: 3), 0)
        XCTAssertEqual(NowPlayingMotion.cascadeDelayMs(index: 5, active: 3), 80)
        XCTAssertEqual(NowPlayingMotion.cascadeDelayMs(index: 50, active: 3), 320)
        XCTAssertEqual(NowPlayingMotion.cascadeDelayMs(index: 5, active: nil), 0)
    }

    func testTheCascadeLagTurnsOneScrollIntoALaterStart() {
        let fluid = Tokens.Motion.easeFluid
        let fractions = (0...12).map { Double($0) / 12 }
        let lag = NowPlayingMotion.cascadeLag(distance: 60, delayMs: 90, durationMs: 380, curve: fluid, at: fractions)
        XCTAssertEqual(lag.count, 13)
        XCTAssertEqual(lag.first, 0)
        XCTAssertEqual(lag.last, 0)
        XCTAssertTrue(lag.allSatisfy { $0 >= 0 && $0 <= 60 })
        let total = 380.0 + 90.0
        for (p, offset) in zip(fractions, lag).dropLast() {
            let t = p * total
            // The scroll moves every line by 60·E(t/D); the lag holds this one back to E((t−90)/D).
            let drawn = 60 * NowPlayingMotion.ease(fluid, t / 380) - Double(offset)
            XCTAssertEqual(drawn, 60 * NowPlayingMotion.ease(fluid, (t - 90) / 380), accuracy: 1e-4)
        }
    }

    func testTheCascadeAsFiveKeyframes() {
        let fluid = Tokens.Motion.easeFluid
        let frames = NowPlayingMotion.cascadeKeyframes(distance: 60, delayMs: 90, durationMs: 380, curve: fluid)
        XCTAssertEqual(frames.count, 5)
        XCTAssertEqual(frames.map { $0.seconds }.reduce(0, +), 0.47, accuracy: 1e-9)
        XCTAssertEqual(frames.last?.value, 0)
        XCTAssertTrue(frames.allSatisfy { $0.value >= 0 && $0.seconds > 0 })
        XCTAssertTrue(NowPlayingMotion.cascadeKeyframes(distance: 60, delayMs: 0, durationMs: 380, curve: fluid).isEmpty)
        XCTAssertTrue(NowPlayingMotion.cascadeKeyframes(distance: 0, delayMs: 90, durationMs: 380, curve: fluid).isEmpty)
    }

    func testEaseFollowsTheCurve() {
        let linear = CubicBezier(0, 0, 1, 1)
        XCTAssertEqual(NowPlayingMotion.ease(linear, 0.3), 0.3, accuracy: 1e-5)
        XCTAssertEqual(NowPlayingMotion.ease(Tokens.Motion.easeFluid, -1), 0)
        XCTAssertEqual(NowPlayingMotion.ease(Tokens.Motion.easeFluid, 2), 1)
        XCTAssertGreaterThan(NowPlayingMotion.ease(Tokens.Motion.easeFluid, 0.2), 0.5, "Fluid is fast at the start")
    }

    func testTheOklchPortReproducesTheTokenColours() throws {
        let tokens = try tokens()
        var checked = 0
        for name in ["accent", "semantic", "field"] {
            let group = try XCTUnwrap(tokens[name] as? [String: Any])
            for entry in group.values {
                guard let token = entry as? [String: Any],
                      let value = token["$value"] as? [String: Any],
                      let components = value["components"] as? [NSNumber],
                      let hex = value["hex"] as? String
                else { continue }
                let rgba = Oklch.rgba(l: components[0].doubleValue, c: components[1].doubleValue,
                                      hue: components[2].doubleValue)
                XCTAssertEqual(String(format: "#%06x", rgba >> 8), hex)
                checked += 1
            }
        }
        XCTAssertGreaterThanOrEqual(checked, 10)
        XCTAssertEqual(AccentChoices.accent(hue: 250, dark: false), 0x0059_A7FF)
    }

    func testSwatchesMatchFixedHuesOnly() {
        XCTAssertEqual(AccentChoices.swatchHues, [250, 210, 180, 150, 120, 60, 30, 0, 330, 290])
        XCTAssertEqual(AccentChoices.swatchIndex(.fixed(hue: 250)), 0)
        XCTAssertEqual(AccentChoices.swatchIndex(.fixed(hue: 359.8)), 7)
        XCTAssertEqual(AccentChoices.swatchIndex(.fixed(hue: -30)), 8)
        XCTAssertNil(AccentChoices.swatchIndex(.fixed(hue: 212.5)))
        XCTAssertNil(AccentChoices.swatchIndex(.artwork))
        let track = AccentChoices.hueTrack(dark: true)
        XCTAssertEqual(track.count, 13)
        XCTAssertEqual(track.first, track.last, "the hue track is red at both ends")
    }

    func testHuesAreNormalised() {
        XCTAssertEqual(AccentChoices.normalize(.nan), 250)
        XCTAssertEqual(AccentChoices.normalize(-30), 330)
        XCTAssertEqual(AccentChoices.normalize(720), 0)
        XCTAssertEqual(AccentChoices.normalize(360), 0)
        XCTAssertEqual(AccentChoices.normalize(42.5), 42.5)
    }

    func testTransparencyPicksTheNearestMaterial() {
        XCTAssertEqual(GlassLook.thickness(base: .regular, transparency: 0.5), .regular)
        XCTAssertNil(GlassLook.thickness(base: .regular, transparency: 0), "0 % is solid Cloud")
        XCTAssertEqual(GlassLook.thickness(base: .regular, transparency: 0.1), .thick)
        XCTAssertEqual(GlassLook.thickness(base: .regular, transparency: 0.9), .thin)
        XCTAssertEqual(GlassLook.thickness(base: .ultraThin, transparency: 1), .ultraThin)
        XCTAssertEqual(GlassLook.thickness(base: .ultraThick, transparency: 0.1), .ultraThick)
        XCTAssertEqual(GlassLook.thickness(base: .thin, transparency: .nan), .thin)
    }

    func testTheTintAlphaComesFromTheSharedPort() {
        XCTAssertEqual(GlassTransparency.alpha(0.62, transparency: 0.5), 0.62, accuracy: 1e-6)
        XCTAssertEqual(GlassTransparency.alpha(0.62, transparency: 0), 1, accuracy: 1e-6)
        XCTAssertEqual(GlassTransparency.alpha(0.62, transparency: 1), 0.31, accuracy: 1e-6)
    }

    func testLuminousFieldFollowsTheMockUpAtTheTokensStrength() {
        XCTAssertEqual(Tokens.Field.auroraStrength, 0.75, accuracy: 1e-9)
        XCTAssertEqual(LuminousField.referenceStrength, Tokens.Field.auroraStrength, accuracy: 1e-9)
        let glows = LuminousField.glows
        XCTAssertEqual(glows.count, 4, "four glows")
        for (glow, expected) in zip(glows, [0.80, 0.72, 0.66, 0.46]) {
            XCTAssertEqual(glow.alpha(strength: Tokens.Field.auroraStrength), expected, accuracy: 1e-9)
        }
        XCTAssertEqual(glows.map(\.color), [Tokens.Field.aurora1, Tokens.Field.aurora2, Tokens.Field.aurora3,
                                             Tokens.Field.aurora1])
        XCTAssertTrue(glows.allSatisfy { (0...1).contains($0.x) && (0...1).contains($0.y) && $0.fadeEnd <= 1 })
        // Misty's field was about 0.45: weaker glows, and never past opaque.
        XCTAssertEqual(glows[0].alpha(strength: 0.45), 0.48, accuracy: 1e-9)
        XCTAssertEqual(glows[0].alpha(strength: 2), 1)
        XCTAssertEqual(glows[0].alpha(strength: .nan), 0.80)
    }

    func testLuminousTokensReachTheApp() {
        XCTAssertEqual(Tokens.TypeScale.textH1, 48, "page titles")
        XCTAssertEqual(Tokens.TypeRoles.nowPlayingTitle.size, 32)
        XCTAssertEqual(Tokens.Artwork.radiusGrid, 16)
        XCTAssertEqual(Tokens.Lyrics.line.size, 40)
        XCTAssertEqual(Tokens.Lyrics.line.lineHeight, 1.18, accuracy: 1e-9)
    }

    func testTheFloatingPlayerAndTheStage() {
        XCTAssertEqual(LuminousGeometry.playerZone(hasPlayer: false, wide: true), 0, "nothing fades without a player")
        XCTAssertEqual(LuminousGeometry.playerZone(hasPlayer: true, wide: true), Tokens.Player.barHeight + 16)
        XCTAssertEqual(LuminousGeometry.playerZone(hasPlayer: true, wide: false), Tokens.Player.miniHeight + 16)
        XCTAssertEqual(LuminousGeometry.stageArtworkSide(width: 1280, height: 800), 380)
        XCTAssertEqual(LuminousGeometry.stageArtworkSide(width: 900, height: 800), 306)
        XCTAssertEqual(LuminousGeometry.stageArtworkSide(width: 1180, height: 600), 230, "the smallest window")
        XCTAssertEqual(LuminousGeometry.stageArtworkSide(width: 300, height: 300), 180, "never smaller than 180")
        XCTAssertEqual(LuminousGeometry.dockHeight, 80)
        XCTAssertEqual(LuminousGeometry.dockRadius, 40)
        XCTAssertEqual(LuminousGeometry.coverLift, 4)
    }

    func testRemainingTimeHasATrueMinus() {
        XCTAssertEqual(TimeText.remaining(positionUs: 83_000_000, durationUs: 225_000_000), "\u{2212}2:22")
        XCTAssertEqual(TimeText.remaining(positionUs: 300_000_000, durationUs: 225_000_000), "\u{2212}0:00")
    }
}

@MainActor
final class AppearanceSettingsTests: XCTestCase {
    func testTransparencyIsClampedToTheCoresRange() {
        let client = FakeCoreClient()
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(.defaults)
        store.setTransparency(1.4)
        XCTAssertEqual(store.current.transparency, 1)
        store.setTransparency(-0.2)
        XCTAssertEqual(store.current.transparency, 0)
        store.setTransparency(.nan)
        XCTAssertEqual(store.current.transparency, 0.5)
    }

    func testLyricsSizeAndAlignmentGoToTheCore() {
        let client = FakeCoreClient()
        let store = SettingsStore(client: client, tracker: CommandTracker(), toasts: ToastCenter())
        store.apply(.defaults)
        store.setLyricsSize(.large)
        store.setLyricsAlign(.center)
        guard case .setSettings(let sent)? = client.sent.last else {
            return XCTFail("expected SetSettings")
        }
        XCTAssertEqual(sent.lyricsSize, .large)
        XCTAssertEqual(sent.lyricsAlign, .center)
    }
}
