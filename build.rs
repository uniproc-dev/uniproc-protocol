use std::path::Path;
use std::process::Command;

const META: &str = "schema/meta.capnp";

const SCHEMAS: &[(&str, &str)] = &[
    ("WINDOWS_SCHEMA_ID", "schema/windows.capnp"),
    ("LINUX_SCHEMA_ID", "schema/linux.capnp"),
];

fn main() {
    println!("cargo:rerun-if-changed={META}");

    let mut command = capnpc::CompilerCommand::new();
    command.src_prefix("schema").file(META);

    let mut ids = String::new();
    for (name, path) in SCHEMAS {
        command.file(path);
        println!("cargo:rerun-if-changed={path}");
        ids.push_str(&format!(
            "/// Identity of `{path}` together with the shared `{META}` it imports;\n\
             /// pass it to the handshake on both ends of the link.\n\
             pub const {name}: u64 = 0x{:016x};\n",
            schema_hash(&[META, path])
        ));
    }

    command
        .run()
        .expect("capnp schema compilation failed; is the `capnp` binary installed?");

    check_every_method_carries_meta();

    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    std::fs::write(Path::new(&out_dir).join("schema_ids.rs"), ids)
        .expect("failed to write schema ids");
}

fn schema_hash(paths: &[&str]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for path in paths {
        let text = std::fs::read_to_string(path).expect("failed to read schema file");
        for byte in text.replace("\r\n", "\n").bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn check_every_method_carries_meta() {
    let mut command = Command::new("capnp");
    command.arg("compile").arg("-o-").arg("--src-prefix=schema");
    command.arg(META);
    for (_, path) in SCHEMAS {
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

    let message = capnp::serialize::read_message_from_flat_slice(
        &mut output.stdout.as_slice(),
        capnp::message::ReaderOptions::new(),
    )
    .expect("failed to read the code generator request");
    let request = message
        .get_root::<capnp::schema_capnp::code_generator_request::Reader>()
        .expect("code generator request has no root");

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
