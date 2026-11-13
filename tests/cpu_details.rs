use uniproc_protocol::windows_capnp::{CpuInstructionSetSemantics, cpu_details, cpu_inventory};

#[test]
fn old_inventory_has_no_details() {
    let bytes: Vec<u8> = [4_u64 << 32, 3_u64 << 32, 0, 0, 0]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let inventory = message.get_root::<cpu_inventory::Reader>().unwrap();
    assert!(!inventory.has_details());
}

#[test]
fn absent_detail_values_are_unavailable_instead_of_zero() {
    let mut message = capnp::message::Builder::new_default();
    message.init_root::<cpu_inventory::Builder>().init_details();
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let inventory = decoded.get_root::<cpu_inventory::Reader>().unwrap();
    assert!(inventory.has_details());
    let details = inventory.get_details().unwrap();
    assert!(details.get_vendor().unwrap().is_empty());
    assert_eq!(details.get_family(), u32::MAX);
    assert_eq!(details.get_model(), u32::MAX);
    assert_eq!(details.get_stepping(), u32::MAX);
    assert_eq!(details.get_microcode_revision(), u64::MAX);
    assert!(!details.has_instruction_sets());
    assert_eq!(
        details.get_instruction_sets_semantics().unwrap(),
        CpuInstructionSetSemantics::Unknown
    );
    assert!(details.get_instruction_sets_scope().unwrap().is_empty());
}

#[test]
fn zero_identity_values_and_full_width_microcode_round_trip() {
    for microcode in [0, 0x100_0000_0001_u64] {
        let mut message = capnp::message::Builder::new_default();
        let mut details = message.init_root::<cpu_details::Builder>();
        details.set_vendor("GenuineIntel");
        details.set_family(0);
        details.set_model(0);
        details.set_stepping(0);
        details.set_microcode_revision(microcode);
        details.set_instruction_sets_semantics(CpuInstructionSetSemantics::OsUsable);
        details.set_instruction_sets_scope("selected_x86_features");
        let bytes = capnp::serialize::write_message_to_words(&message);
        let decoded = capnp::serialize::read_message(
            &mut bytes.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .unwrap();
        let details = decoded.get_root::<cpu_details::Reader>().unwrap();
        assert_eq!(details.get_vendor().unwrap(), "GenuineIntel");
        assert_eq!(details.get_family(), 0);
        assert_eq!(details.get_model(), 0);
        assert_eq!(details.get_stepping(), 0);
        assert_eq!(details.get_microcode_revision(), microcode);
        assert_eq!(
            details.get_instruction_sets_semantics().unwrap(),
            CpuInstructionSetSemantics::OsUsable
        );
        assert_eq!(
            details.get_instruction_sets_scope().unwrap(),
            "selected_x86_features"
        );
    }
}

#[test]
fn unknown_empty_and_nonempty_instruction_sets_remain_distinct() {
    let cases: [Option<&[&str]>; 3] = [None, Some(&[]), Some(&["AVX2", "SSE2"])];
    for expected in cases {
        let mut message = capnp::message::Builder::new_default();
        let mut details = message.init_root::<cpu_details::Builder>();
        if let Some(values) = expected {
            let mut list = details
                .reborrow()
                .init_instruction_sets(values.len() as u32);
            for (index, value) in values.iter().enumerate() {
                list.set(index as u32, *value);
            }
        }
        let bytes = capnp::serialize::write_message_to_words(&message);
        let decoded = capnp::serialize::read_message(
            &mut bytes.as_slice(),
            capnp::message::ReaderOptions::new(),
        )
        .unwrap();
        let details = decoded.get_root::<cpu_details::Reader>().unwrap();
        let actual = details.has_instruction_sets().then(|| {
            details
                .get_instruction_sets()
                .unwrap()
                .iter()
                .map(|s| s.unwrap().to_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            actual,
            expected.map(|values| values.iter().map(|s| s.to_string()).collect())
        );
    }
}
