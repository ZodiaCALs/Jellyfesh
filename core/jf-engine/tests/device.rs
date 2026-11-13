//! Device output must fail cleanly without a sound card, and must not break CI where one exists.

use jf_engine::{Engine, EngineConfig, EngineError, OutputKind};

#[cfg(feature = "device")]
#[test]
fn listing_devices_returns_a_list_or_a_clean_error() {
    match jf_engine::list_devices() {
        Ok(devices) => {
            println!("{} output device(s)", devices.len());
            for device in &devices {
                println!(
                    "  {} (default: {}) {:?}",
                    device.name, device.is_default, device.sample_rates
                );
                assert!(!device.name.is_empty());
                assert!(device.sample_rates.windows(2).all(|w| w[0] < w[1]));
            }
            assert!(devices.iter().filter(|d| d.is_default).count() <= 1);
        }
        Err(err) => println!("no output devices: {err}"),
    }
}

#[cfg(feature = "device")]
#[test]
fn device_engine_starts_or_fails_cleanly() {
    match Engine::new(EngineConfig::default()) {
        Ok(engine) => {
            // A device exists: the engine must be controllable and shut down cleanly.
            engine.set_volume(0.0);
            engine.stop();
            assert!(engine.wait_until_idle(std::time::Duration::from_secs(5)));
        }
        Err(err) => {
            println!("device output unavailable: {err}");
            assert!(
                matches!(err, EngineError::Device(_) | EngineError::DeviceNotFound(_)),
                "{err:?}"
            );
        }
    }
}

#[cfg(feature = "device")]
#[test]
fn an_unknown_device_name_is_reported() {
    let config = EngineConfig {
        output: OutputKind::Device {
            name: Some("no such device, jf-engine test".into()),
        },
        ..EngineConfig::default()
    };
    match Engine::new(config) {
        Ok(_) => panic!("opened a device that does not exist"),
        Err(err) => assert!(
            matches!(err, EngineError::DeviceNotFound(_) | EngineError::Device(_)),
            "{err:?}"
        ),
    }
}

#[cfg(not(feature = "device"))]
#[test]
fn device_output_needs_the_device_feature() {
    let config = EngineConfig {
        output: OutputKind::Device { name: None },
        buffer_ms: 200,
        ..EngineConfig::default()
    };
    assert!(matches!(
        Engine::new(config),
        Err(EngineError::DeviceSupportDisabled)
    ));
}

#[test]
fn an_unwritable_wav_path_fails_at_construction() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing-dir").join("out.wav");
    let config = EngineConfig {
        output: OutputKind::WavFile {
            path,
            realtime: false,
        },
        buffer_ms: 200,
        ..EngineConfig::default()
    };
    assert!(matches!(Engine::new(config), Err(EngineError::Io(_))));
}
