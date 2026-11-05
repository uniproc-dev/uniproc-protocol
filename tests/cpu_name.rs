use uniproc_protocol::windows_capnp::machine_sample;

#[test]
fn old_machine_sample_has_no_cpu_name() {
    let bytes: Vec<u8> = [11_u64 << 32, (8_u64 << 48) | (2_u64 << 32), 17, 23]
        .into_iter()
        .chain(std::iter::repeat_n(0, 8))
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = message.get_root::<machine_sample::Reader>().unwrap();
    assert_eq!(sample.get_snapshot(), 17);
    assert_eq!(sample.get_sampled_at(), 23);
    assert!(!sample.has_cpu_name());
    assert!(sample.get_cpu_name().unwrap().is_empty());
}

#[test]
fn cpu_name_and_times_survive_without_temperatures() {
    let mut message = capnp::message::Builder::new_default();
    let mut sample = message.init_root::<machine_sample::Builder>();
    sample.set_snapshot(17);
    sample.set_sampled_at(23);
    sample.set_cpu_name("Intel(R) Core(TM) i5-10400F CPU @ 2.90GHz");
    let mut cpu = sample.init_cpu();
    cpu.set_idle_time(31);
    cpu.set_kernel_time(47);
    cpu.set_user_time(59);
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = decoded.get_root::<machine_sample::Reader>().unwrap();
    assert_eq!(sample.get_snapshot(), 17);
    assert_eq!(sample.get_sampled_at(), 23);
    assert_eq!(
        sample.get_cpu_name().unwrap(),
        "Intel(R) Core(TM) i5-10400F CPU @ 2.90GHz"
    );
    assert!(!sample.has_temperatures());
    let cpu = sample.get_cpu().unwrap();
    assert_eq!(cpu.get_idle_time(), 31);
    assert_eq!(cpu.get_kernel_time(), 47);
    assert_eq!(cpu.get_user_time(), 59);
}
