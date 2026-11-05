use uniproc_protocol::windows_capnp::{
    TemperatureCategory, TemperatureStatus, machine_sample, temperature_device, temperature_sample,
    temperature_sensor,
};

#[test]
fn old_machine_sample_has_unknown_temperatures() {
    let bytes: Vec<u8> = [10_u64 << 32, (7_u64 << 48) | (2_u64 << 32), 17, 23]
        .into_iter()
        .chain(std::iter::repeat_n(0, 7))
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = message.get_root::<machine_sample::Reader>().unwrap();
    assert_eq!(sample.get_snapshot(), 17);
    assert_eq!(sample.get_sampled_at(), 23);
    assert!(!sample.has_temperatures());
    let temperatures = sample.get_temperatures().unwrap();
    assert_eq!(
        temperatures.get_status().unwrap(),
        TemperatureStatus::Unknown
    );
    assert!(temperatures.get_error().unwrap().is_empty());
    assert!(temperatures.get_devices().unwrap().is_empty());
}

#[test]
fn old_temperature_device_has_no_diagnostic() {
    let bytes: Vec<u8> = [5_u64 << 32, (3_u64 << 48) | (1_u64 << 32), 0, 0, 0, 0]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect();
    let message =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let device = message.get_root::<temperature_device::Reader>().unwrap();
    assert!(device.get_error().unwrap().is_empty());
}

#[test]
fn device_error_survives_serialization() {
    let mut message = capnp::message::Builder::new_default();
    let mut device = message.init_root::<temperature_device::Builder>();
    device.set_status(TemperatureStatus::Error);
    device.set_error("temperature read failed");
    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let device = decoded.get_root::<temperature_device::Reader>().unwrap();
    assert_eq!(device.get_status().unwrap(), TemperatureStatus::Error);
    assert_eq!(device.get_error().unwrap(), "temperature read failed");
}

#[test]
fn missing_reading_and_zero_celsius_remain_distinct() {
    let mut message = capnp::message::Builder::new_default();
    let mut sample = message.init_root::<temperature_sample::Builder>();
    sample.set_status(TemperatureStatus::Available);
    let mut devices = sample.init_devices(1);
    let mut cpu = devices.reborrow().get(0);
    cpu.set_id("/cpu/0");
    cpu.set_name("CPU");
    cpu.set_category(TemperatureCategory::Cpu);
    cpu.set_status(TemperatureStatus::Available);
    let mut sensors = cpu.init_sensors(2);
    sensors.reborrow().get(0).set_id("/cpu/0/temperature/0");
    let mut zero = sensors.get(1);
    zero.set_id("/cpu/0/temperature/1");
    zero.set_name("Package");
    zero.init_reading().set_celsius(0.0);

    let bytes = capnp::serialize::write_message_to_words(&message);
    let decoded =
        capnp::serialize::read_message(&mut bytes.as_slice(), capnp::message::ReaderOptions::new())
            .unwrap();
    let sample = decoded.get_root::<temperature_sample::Reader>().unwrap();
    assert_eq!(sample.get_status().unwrap(), TemperatureStatus::Available);
    let devices = sample.get_devices().unwrap();
    assert_eq!(devices.get(0).get_id().unwrap(), "/cpu/0");
    let sensors = devices.get(0).get_sensors().unwrap();
    assert!(matches!(
        sensors.get(0).get_reading().which().unwrap(),
        temperature_sensor::reading::Unavailable(())
    ));
    assert!(matches!(
        sensors.get(1).get_reading().which().unwrap(),
        temperature_sensor::reading::Celsius(value) if value == 0.0
    ));
    assert_eq!(sensors.get(1).get_id().unwrap(), "/cpu/0/temperature/1");
}
