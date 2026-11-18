use uniproc_protocol::windows_capnp::machine_memory;

#[test]
fn old_memory_keeps_counters_and_has_no_details() {
    let bytes: Vec<u8> = [5_u64 << 32, 4_u64 << 32, 100, 40, 200, 70]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let memory = message.get_root::<machine_memory::Reader>().unwrap();
    assert_eq!(memory.get_total_physical(), 100);
    assert_eq!(memory.get_available_physical(), 40);
    assert_eq!(memory.get_commit_limit(), 200);
    assert_eq!(memory.get_committed(), 70);
    assert!(!memory.has_details());
    assert!(!memory.has_inventory());
}

#[test]
fn inventory_unknown_zero_and_raw_codes_survive() {
    for value in [u32::MAX, 0, 254] {
        let mut message = capnp::message::Builder::new_default();
        let memory = message.init_root::<machine_memory::Builder>();
        let mut inventory = memory.init_inventory();
        assert_eq!(inventory.reborrow().get_memory_type(), u32::MAX);
        assert_eq!(inventory.reborrow().get_hardware_reserved(), u64::MAX);
        inventory.set_memory_type(value);
        inventory.set_speed_mt_s(value);
        inventory.set_slots_used(value);
        inventory.set_slots_total(value);
        inventory.set_form_factor(value);
        inventory.set_hardware_reserved(u64::from(value));
        let bytes = capnp::serialize::write_message_to_words(&message);
        let decoded = capnp::serialize::read_message(
            &mut bytes.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .unwrap();
        let memory = decoded.get_root::<machine_memory::Reader>().unwrap();
        assert!(memory.has_inventory());
        let inventory = memory.get_inventory().unwrap();
        assert_eq!(
            [
                inventory.get_memory_type(),
                inventory.get_speed_mt_s(),
                inventory.get_slots_used(),
                inventory.get_slots_total(),
                inventory.get_form_factor()
            ],
            [value; 5]
        );
        assert_eq!(inventory.get_hardware_reserved(), u64::from(value));
    }
}

#[test]
fn missing_memory_readings_and_zero_remain_distinct() {
    for value in [u64::MAX, 0, 123456789] {
        let mut message = capnp::message::Builder::new_default();
        let mut memory = message.init_root::<machine_memory::Builder>();
        assert!(!memory.reborrow().has_details());
        let mut details = memory.init_details();
        assert_eq!(details.reborrow().get_compressed(), u64::MAX);
        details.set_compressed(value);
        details.set_cached(value);
        details.set_paged_pool(value);
        details.set_non_paged_pool(value);
        details.set_standby(value);
        details.set_modified(value);
        details.set_free(value);
        details.set_installed(value);
        let bytes = capnp::serialize::write_message_to_words(&message);
        let decoded = capnp::serialize::read_message(
            &mut bytes.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .unwrap();
        let memory = decoded.get_root::<machine_memory::Reader>().unwrap();
        assert!(memory.has_details());
        let details = memory.get_details().unwrap();
        assert_eq!(
            [
                details.get_compressed(),
                details.get_cached(),
                details.get_paged_pool(),
                details.get_non_paged_pool(),
                details.get_standby(),
                details.get_modified(),
                details.get_free(),
                details.get_installed(),
            ],
            [value; 8]
        );
    }
}
