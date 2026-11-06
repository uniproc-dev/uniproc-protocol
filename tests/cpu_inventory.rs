use uniproc_protocol::windows_capnp::{Toggle, machine_sample};

#[test]
fn old_machine_sample_has_no_system_stats_or_inventory() {
    let bytes: Vec<u8> = [12_u64 << 32, (9_u64 << 48) | (2_u64 << 32), 17, 23]
        .into_iter()
        .chain(std::iter::repeat_n(0, 9))
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = message.get_root::<machine_sample::Reader>().unwrap();
    assert_eq!(sample.get_snapshot(), 17);
    assert_eq!(sample.get_sampled_at(), 23);
    assert!(!sample.has_system_stats());
    assert!(!sample.has_cpu_inventory());
}

#[test]
fn missing_inventory_values_are_unknown_instead_of_zero_or_false() {
    let mut message = capnp::message::Builder::new_default();
    message
        .init_root::<machine_sample::Builder>()
        .init_cpu_inventory();
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = decoded.get_root::<machine_sample::Reader>().unwrap();
    assert!(sample.has_cpu_inventory());
    let inventory = sample.get_cpu_inventory().unwrap();
    assert_eq!(inventory.get_active_package_count(), u32::MAX);
    assert_eq!(inventory.get_physical_core_count(), u32::MAX);
    assert_eq!(inventory.get_logical_processor_count(), u32::MAX);
    assert_eq!(inventory.get_l1_cache_bytes(), u64::MAX);
    assert_eq!(inventory.get_l2_cache_bytes(), u64::MAX);
    assert_eq!(inventory.get_l3_cache_bytes(), u64::MAX);
    assert_eq!(inventory.get_base_mhz(), u32::MAX);
    assert_eq!(
        inventory.get_virtualization_hardware_supported().unwrap(),
        Toggle::Unknown
    );
    assert_eq!(
        inventory.get_virtualization_firmware_enabled().unwrap(),
        Toggle::Unknown
    );
    assert_eq!(inventory.get_hypervisor_present().unwrap(), Toggle::Unknown);
    assert!(!sample.has_system_stats());
}

#[test]
fn present_zero_system_and_inventory_values_survive() {
    let mut message = capnp::message::Builder::new_default();
    let mut sample = message.init_root::<machine_sample::Builder>();
    let mut stats = sample.reborrow().init_system_stats();
    stats.set_process_count(0);
    stats.set_thread_count(0);
    stats.set_handle_count(0);
    stats.set_uptime_ms(0);
    let mut inventory = sample.init_cpu_inventory();
    inventory.set_active_package_count(1);
    inventory.set_physical_core_count(6);
    inventory.set_logical_processor_count(12);
    inventory.set_l1_cache_bytes(0);
    inventory.set_l2_cache_bytes(0);
    inventory.set_l3_cache_bytes(0);
    inventory.set_base_mhz(2900);
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = decoded.get_root::<machine_sample::Reader>().unwrap();
    assert!(sample.has_system_stats());
    let stats = sample.get_system_stats().unwrap();
    assert_eq!(stats.get_process_count(), 0);
    assert_eq!(stats.get_thread_count(), 0);
    assert_eq!(stats.get_handle_count(), 0);
    assert_eq!(stats.get_uptime_ms(), 0);
    let inventory = sample.get_cpu_inventory().unwrap();
    assert_eq!(inventory.get_active_package_count(), 1);
    assert_eq!(inventory.get_physical_core_count(), 6);
    assert_eq!(inventory.get_logical_processor_count(), 12);
    assert_eq!(inventory.get_l1_cache_bytes(), 0);
    assert_eq!(inventory.get_l2_cache_bytes(), 0);
    assert_eq!(inventory.get_l3_cache_bytes(), 0);
    assert_eq!(inventory.get_base_mhz(), 2900);
    assert!(!sample.has_temperatures());
}

#[test]
fn virtualization_signals_remain_independent_and_preserve_false() {
    let mut message = capnp::message::Builder::new_default();
    let mut inventory = message
        .init_root::<machine_sample::Builder>()
        .init_cpu_inventory();
    inventory.set_virtualization_hardware_supported(Toggle::Unknown);
    inventory.set_virtualization_firmware_enabled(Toggle::Unknown);
    inventory.set_hypervisor_present(Toggle::On);
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = decoded.get_root::<machine_sample::Reader>().unwrap();
    let inventory = sample.get_cpu_inventory().unwrap();
    assert_eq!(
        inventory.get_virtualization_hardware_supported().unwrap(),
        Toggle::Unknown
    );
    assert_eq!(
        inventory.get_virtualization_firmware_enabled().unwrap(),
        Toggle::Unknown
    );
    assert_eq!(inventory.get_hypervisor_present().unwrap(), Toggle::On);
    let mut message = capnp::message::Builder::new_default();
    message
        .init_root::<machine_sample::Builder>()
        .init_cpu_inventory()
        .set_hypervisor_present(Toggle::Off);
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    assert_eq!(
        decoded
            .get_root::<machine_sample::Reader>()
            .unwrap()
            .get_cpu_inventory()
            .unwrap()
            .get_hypervisor_present()
            .unwrap(),
        Toggle::Off
    );
}
