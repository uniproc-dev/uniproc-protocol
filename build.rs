use std::path::Path;
use std::process::Command;

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

    check_every_method_carries_meta(request);

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

fn check_every_method_carries_meta(request: capnp::schema_capnp::code_generator_request::Reader) {
    let nodes = request.get_nodes().expect("request carries no nodes");
    let find = |id: u64| {
        nodes
            .iter()
            .find(|node| node.get_id() == id)
            .expect("the request must contain every referenced node")
    };
    let struct_field_type_id = |node_id: u64, field: &str| -> Option<u64> {
        let node = find(node_id);
        let capnp::schema_capnp::node::Struct(body) = node.which().ok()? else {
            return None;
        };
        let named = body
            .get_fields()
            .ok()?
            .iter()
            .find(|f| f.get_name().map(|n| n == field).unwrap_or(false))?;
        let capnp::schema_capnp::field::Slot(slot) = named.which().ok()? else {
            return None;
        };
        match slot.get_type().ok()?.which().ok()? {
            capnp::schema_capnp::type_::Struct(s) => Some(s.get_type_id()),
            _ => None,
        }
    };

    let meta_ids: Vec<(String, u64)> = nodes
        .iter()
        .filter(|node| {
            matches!(node.which(), Ok(capnp::schema_capnp::node::Struct(_)))
                && node
                    .get_display_name()
                    .map(|n| n.to_str().unwrap_or_default().contains("meta.capnp"))
                    .unwrap_or(false)
        })
        .filter_map(|node| {
            let name = node.get_display_name().ok()?.to_str().ok()?.to_string();
            Some((name, node.get_id()))
        })
        .collect();
    let id_of = |suffix: &str| {
        meta_ids
            .iter()
            .find(|(name, _)| name.ends_with(suffix))
            .unwrap_or_else(|| panic!("{META} must define {suffix}"))
            .1
    };
    let request_meta = id_of("RequestMeta");
    let response_meta = id_of("ResponseMeta");

    for node in nodes.iter() {
        let Ok(capnp::schema_capnp::node::Interface(interface)) = node.which() else {
            continue;
        };
        let owner = node
            .get_display_name()
            .expect("node has a display name")
            .to_str()
            .expect("display name is utf-8")
            .to_string();

        for method in interface
            .get_methods()
            .expect("interface carries methods")
            .iter()
        {
            let name = method
                .get_name()
                .expect("method has a name")
                .to_str()
                .expect("method name is utf-8")
                .to_string();

            assert_eq!(
                struct_field_type_id(method.get_param_struct_type(), "meta"),
                Some(request_meta),
                "{owner}.{name} must take `meta :Meta.RequestMeta`"
            );

            let results = method.get_result_struct_type();
            if find(results).get_scope_id() == 0 {
                continue;
            }
            assert_eq!(
                struct_field_type_id(results, "meta"),
                Some(response_meta),
                "{owner}.{name} must return `meta :Meta.ResponseMeta`"
            );
        }
    }
}
