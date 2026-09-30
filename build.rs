use std::path::Path;
use std::process::Command;

#[path = "build/meta.rs"]
mod meta;

const META: &str = "schema/meta.capnp";

const LINKS: &[(&str, &str)] = &[
    ("WINDOWS_PROTOCOL", "schema/windows.capnp"),
    ("LINUX_PROTOCOL", "schema/linux.capnp"),
];

fn main() {
    println!("cargo:rerun-if-changed={META}");

    let mut command = capnpc::CompilerCommand::new();
    command.src_prefix("schema").file(META);
    for (_, path) in LINKS {
        command.file(path);
        println!("cargo:rerun-if-changed={path}");
    }
    command
        .run()
        .expect("capnp schema compilation failed; is the `capnp` binary installed?");

    let request = code_generator_request();
    let message = capnp::serialize::read_message_from_flat_slice(
        &mut request.as_slice(),
        capnp::message::ReaderOptions::new(),
    )
    .expect("failed to read the code generator request");
    let request = message
        .get_root::<capnp::schema_capnp::code_generator_request::Reader>()
        .expect("code generator request has no root");

    if let Err(missing) = meta::check_every_method_carries_meta(request) {
        panic!("{missing}");
    }

    let mut protocols = String::new();
    for (name, path) in LINKS {
        let (id, [major, minor, patch]) = protocol_of(request, path);
        protocols.push_str(&format!(
            "/// `{path}`: its file id and the `version` it declares.\n\
             pub const {name}: ProtocolInfo = ProtocolInfo {{ id: 0x{id:016x}, major: {major}, minor: {minor}, patch: {patch} }};\n"
        ));
    }

    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    std::fs::write(Path::new(&out_dir).join("protocols.rs"), protocols)
        .expect("failed to write protocol constants");
}

fn code_generator_request() -> Vec<u8> {
    let mut command = Command::new("capnp");
    command.arg("compile").arg("-o-").arg("--src-prefix=schema");
    command.arg(META);
    for (_, path) in LINKS {
        command.arg(path);
    }

    let output = command
        .output()
        .expect("failed to run `capnp compile`; is the `capnp` binary installed?");
    assert!(
        output.status.success(),
        "`capnp compile` failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn protocol_of(
    request: capnp::schema_capnp::code_generator_request::Reader,
    path: &str,
) -> (u64, [u32; 3]) {
    let file = path.strip_prefix("schema/").expect("links live under schema/");
    let file_id = request
        .get_requested_files()
        .expect("request lists its files")
        .iter()
        .find(|f| f.get_filename().map(|n| n == file).unwrap_or(false))
        .unwrap_or_else(|| panic!("{file} is not in the request"))
        .get_id();

    let const_name = format!("{file}:version");
    let version = request
        .get_nodes()
        .expect("request carries nodes")
        .iter()
        .find(|node| node.get_display_name().map(|n| n == const_name.as_str()).unwrap_or(false))
        .unwrap_or_else(|| panic!("{path} must declare `const version :Text = \"MAJOR.MINOR.PATCH\";`"));
    let capnp::schema_capnp::node::Const(constant) = version.which().expect("known node kind") else {
        panic!("{const_name} must be a const");
    };
    let capnp::schema_capnp::value::Text(text) = constant
        .get_value()
        .expect("const has a value")
        .which()
        .expect("known value kind")
    else {
        panic!("{const_name} must be Text");
    };
    let text = text.expect("version text").to_str().expect("version is utf-8");

    let parts: Vec<u32> = text
        .split('.')
        .map(|part| part.parse().unwrap_or_else(|_| panic!("{const_name} = {text:?} is not MAJOR.MINOR.PATCH")))
        .collect();
    let [major, minor, patch] = parts[..] else {
        panic!("{const_name} = {text:?} is not MAJOR.MINOR.PATCH");
    };
    (file_id, [major, minor, patch])
}
