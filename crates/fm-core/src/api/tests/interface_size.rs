//! The interface size through the API (E58.5): a change is announced through
//! the typed `Event` enum with its factor; an update that leaves the size as
//! it is announces nothing.

use fm_api::settings::{InterfaceSize, InterfaceSizeChoice};

use super::*;

fn set_size(api: &CoreApi, size: InterfaceSize) -> Settings {
    api.update_settings(SettingsUpdate {
        interface_size: Some(size),
        ..SettingsUpdate::default()
    })
    .expect("saved")
}

#[test]
fn a_change_of_the_interface_size_is_an_event_with_its_factor() {
    let lib = Lib::new("api-interface-size");
    let rx = lib.api.subscribe();

    let saved = set_size(&lib.api, InterfaceSize::ExtraLarge);
    assert_eq!(saved.interface_size, InterfaceSize::ExtraLarge);
    assert_eq!(
        rx.recv_timeout(WAIT),
        Ok(Event::InterfaceSizeChanged {
            interface_size: InterfaceSizeChoice {
                size: InterfaceSize::ExtraLarge,
                factor: 1.5,
            },
        })
    );

    set_size(&lib.api, InterfaceSize::ExtraLarge);
    lib.api
        .update_settings(SettingsUpdate {
            theme: Some(fm_api::settings::Theme::Dark),
            ..SettingsUpdate::default()
        })
        .expect("saved");
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the size did not change"
    );

    set_size(&lib.api, InterfaceSize::Default);
    assert_eq!(
        rx.recv_timeout(WAIT),
        Ok(Event::InterfaceSizeChanged {
            interface_size: InterfaceSizeChoice {
                size: InterfaceSize::Default,
                factor: 1.0,
            },
        })
    );
    assert_eq!(
        lib.api.get_settings().expect("read").interface_size,
        InterfaceSize::Default
    );
}
