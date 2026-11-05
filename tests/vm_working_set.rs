use uniproc_protocol::windows_capnp::process_columns;

#[test]
fn old_process_columns_has_no_vm_working_set() {
    let bytes: Vec<u8> = [45_u64 << 32, (41_u64 << 48) | (3_u64 << 32), 5, 7, 11]
        .into_iter()
        .chain(std::iter::repeat_n(0, 41))
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let columns = message.get_root::<process_columns::Reader>().unwrap();
    assert_eq!(columns.get_snapshot(), 5);
    assert_eq!(columns.get_sampled_at(), 7);
    assert_eq!(columns.get_passport_etag(), 11);
    assert!(!columns.has_vm_working_set());
    assert!(columns.get_vm_working_set().unwrap().is_empty());
}

#[test]
fn vm_working_set_preserves_zero_and_gap_separately_from_raw_working_set() {
    let mut message = capnp::message::Builder::new_default();
    let mut columns = message.init_root::<process_columns::Builder>();
    let mut raw = columns.reborrow().init_working_set(3);
    for (row, value) in [200, 300, 400].into_iter().enumerate() {
        raw.set(row as u32, value);
    }
    let mut corrected = columns.init_vm_working_set(3);
    for (row, value) in [0, 123, u64::MAX].into_iter().enumerate() {
        corrected.set(row as u32, value);
    }
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let columns = decoded.get_root::<process_columns::Reader>().unwrap();
    assert!(columns.has_vm_working_set());
    assert_eq!(
        columns
            .get_working_set()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [200, 300, 400]
    );
    assert_eq!(
        columns
            .get_vm_working_set()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [0, 123, u64::MAX]
    );
}
