use uniproc_protocol::windows_capnp::{ServiceStartupType, ServiceState, service_stats};

#[test]
fn old_service_stats_preserves_status_and_defaults_new_fields() {
    let bytes: Vec<u8> = [
        7_u64 << 32,
        (5_u64 << 48) | (1_u64 << 32),
        42_u64 | (4_u64 << 32),
        0,
        0,
        0,
        0,
        0,
    ]
    .into_iter()
    .flat_map(u64::to_le_bytes)
    .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let service = message.get_root::<service_stats::Reader>().unwrap();

    assert_eq!(service.get_pid(), 42);
    assert_eq!(service.get_state().unwrap(), ServiceState::Running);
    assert_eq!(
        service.get_startup_type().unwrap(),
        ServiceStartupType::Unknown
    );
    assert!(!service.get_delayed_auto());
    assert!(service.get_account().unwrap().is_empty());
    assert_eq!(service.get_service_type(), 0);
    assert_eq!(service.get_accepted_controls(), 0);
}
