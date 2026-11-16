use uniproc_protocol::windows_capnp::machine_sample;

#[test]
fn old_machine_sample_has_no_cpu_power_reading() {
    let bytes: Vec<u8> = [14_u64 << 32, (11_u64 << 48) | (2_u64 << 32), 17, 23]
        .into_iter()
        .chain(std::iter::repeat_n(0, 11))
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = message.get_root::<machine_sample::Reader>().unwrap();
    assert_eq!(sample.get_snapshot(), 17);
    assert_eq!(sample.get_sampled_at(), 23);
    assert!(!sample.has_cpu_package_power_watts());
}

#[test]
fn missing_zero_and_nonzero_watts_remain_distinct() {
    for expected in [None, Some(0.0), Some(42.125)] {
        let mut message = capnp::message::Builder::new_default();
        let mut sample = message.init_root::<machine_sample::Builder>();
        sample.set_snapshot(17);
        sample.set_sampled_at(23);
        if let Some(watts) = expected {
            sample.init_cpu_package_power_watts().set_watts(watts);
        }
        let bytes = capnp::serialize::write_message_to_words(&message);
        let decoded = capnp::serialize::read_message(
            &mut bytes.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .unwrap();
        let sample = decoded.get_root::<machine_sample::Reader>().unwrap();
        let actual = sample
            .has_cpu_package_power_watts()
            .then(|| sample.get_cpu_package_power_watts().unwrap().get_watts());
        assert_eq!(actual, expected);
        assert_eq!(sample.get_snapshot(), 17);
        assert_eq!(sample.get_sampled_at(), 23);
    }
}
