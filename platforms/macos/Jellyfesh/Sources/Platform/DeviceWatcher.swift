// Output devices coming and going (docs/core-api.md: the core never polls; shells send
// `RefreshDevices` on the OS's device notifications, which also lets playback follow a new system
// default). CoreAudio reports the device list and the default output; a burst of changes (a
// Bluetooth headset connecting) becomes one refresh.

import CoreAudio
import Foundation

@MainActor
final class DeviceWatcher {
    private let onChange: () -> Void
    private var listener: AudioObjectPropertyListenerBlock?
    private var pending: Task<Void, Never>?
    private var addresses = [
        AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        ),
        AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        ),
    ]

    init(onChange: @escaping () -> Void) {
        self.onChange = onChange
    }

    func start() {
        guard listener == nil else { return }
        let block: AudioObjectPropertyListenerBlock = { [weak self] _, _ in
            Task { @MainActor [weak self] in
                self?.changed()
            }
        }
        listener = block
        let system = AudioObjectID(kAudioObjectSystemObject)
        for index in addresses.indices {
            _ = AudioObjectAddPropertyListenerBlock(system, &addresses[index], DispatchQueue.main, block)
        }
    }

    func stop() {
        guard let block = listener else { return }
        let system = AudioObjectID(kAudioObjectSystemObject)
        for index in addresses.indices {
            _ = AudioObjectRemovePropertyListenerBlock(system, &addresses[index], DispatchQueue.main, block)
        }
        listener = nil
        pending?.cancel()
    }

    private func changed() {
        pending?.cancel()
        pending = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(300))
            guard !Task.isCancelled else { return }
            self?.onChange()
        }
    }
}
