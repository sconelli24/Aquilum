use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::release::{ENDPOINT, PUBLIC_KEY, newer, verify};
use super::{Status, Updater};

#[test]
fn versions_compare_numerically() {
    assert!(newer("0.2.10", "0.2.9"));
    assert!(newer("v0.3.0", "0.2.6"));
    assert!(!newer("0.2.6", "0.2.6"));
    assert!(!newer("0.2.5", "0.2.6"));
    assert!(!newer("0.3.16", "0.3.16"));
    assert!(newer("0.3.17", "0.3.16"));
    assert!(!newer("0.3.16-beta.1", "0.3.16"));
    assert!(newer("1.0.0", "0.9.9"));
}

#[test]
fn circuit_breaker_stops_loop_and_requires_manual_download() {
    let dir = tempfile::tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    let woke = Arc::new(AtomicBool::new(false));
    let wake: Arc<dyn Fn() + Send + Sync> = {
        let woke = Arc::clone(&woke);
        Arc::new(move || woke.store(true, Ordering::SeqCst))
    };

    // 1. First launch: Idle status
    let u1 = Updater::new(data_dir.clone(), Arc::clone(&wake));
    assert_eq!(u1.status(), Status::Idle);

    // 2. Simulate an update attempt (attempt 1)
    u1.record_attempt("99.0.0", "https://github.com/Freaction/Aquilum/releases/download/v99.0.0/setup.exe");

    // 3. Restart still on the old version (attempt 1 < MAX_ATTEMPTS)
    let u2 = Updater::new(data_dir.clone(), Arc::clone(&wake));
    assert_eq!(u2.status(), Status::Idle);

    // 4. Second update attempt (attempt 2)
    u2.record_attempt("99.0.0", "https://github.com/Freaction/Aquilum/releases/download/v99.0.0/setup.exe");

    // 5. Restart on the old version again: the loop circuit breaker trips!
    let u3 = Updater::new(data_dir.clone(), Arc::clone(&wake));
    let status = u3.status();
    assert_eq!(
        status,
        Status::ManualRequired {
            version: "99.0.0".into(),
            url: "https://github.com/Freaction/Aquilum/releases/download/v99.0.0/setup.exe".into(),
        }
    );

    // 6. In the ManualRequired status install() does not start another download or restart loop
    u3.install();
    assert_eq!(
        u3.status(),
        Status::ManualRequired {
            version: "99.0.0".into(),
            url: "https://github.com/Freaction/Aquilum/releases/download/v99.0.0/setup.exe".into(),
        }
    );
    assert_eq!(u3.take_ready(), None);

    // 7. When the version matches (the update was installed), the marker is cleared
    super::Attempt::record(&data_dir, env!("CARGO_PKG_VERSION"), "https://example.com");
    let u4 = Updater::new(data_dir.clone(), Arc::clone(&wake));
    assert_eq!(u4.status(), Status::Idle);
    assert!(super::Attempt::load(&data_dir).is_none());
}

#[test]
fn android_asks_and_unsupported_platforms_have_no_auto_update() {
    #[cfg(target_os = "android")]
    {
        assert!(super::apply::supported() && super::available() && super::ASKS);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "android")))]
    {
        assert!(!super::apply::supported());
        assert!(!super::available());
        assert_eq!(super::apply::apply(&[], || {}), Err(crate::i18n::t("native.updater.platformUnsupported")));
    }
}

#[test]
fn manual_required_settings_row_shows_download_button() {
    use crate::ui::settings::schema::{Action, Control, update_row};

    let status = Status::ManualRequired {
        version: "0.3.17".into(),
        url: "https://example.com/download".into(),
    };
    let row = update_row(&status);
    assert!(row.description.is_some());
    if let Control::Buttons(buttons) = row.control {
        assert_eq!(buttons.len(), 1);
        assert_eq!(buttons[0].action, Action::OpenUrl("https://example.com/download".into()));
    } else {
        panic!("expected a manual download button");
    }
}

#[test]
#[ignore]
fn verifies_real_release_signature() {
    let manifest: serde_json::Value = serde_json::from_reader(aquilum_core::web_agent::web_agent("updates").get(ENDPOINT).call().unwrap().into_reader()).unwrap();
    let platform = &manifest["platforms"]["windows-x86_64"];
    let mut data = Vec::new();
    std::io::Read::read_to_end(&mut aquilum_core::web_agent::web_agent("updates").get(platform["url"].as_str().unwrap()).call().unwrap().into_reader(), &mut data).unwrap();
    verify(&data, platform["signature"].as_str().unwrap(), PUBLIC_KEY).unwrap();
    data[100] ^= 1;
    assert!(verify(&data, platform["signature"].as_str().unwrap(), PUBLIC_KEY).is_err());
}
