use capnp::schema_capnp::{code_generator_request, field, node, type_};

pub fn check_every_method_carries_meta(request: code_generator_request::Reader) -> Result<(), String> {
    let nodes = request.get_nodes().map_err(|e| e.to_string())?;
    let find = |id: u64| {
        nodes
            .iter()
            .find(|node| node.get_id() == id)
            .expect("the request must contain every referenced node")
    };
    let field_count = |node_id: u64| -> u32 {
        match find(node_id).which() {
            Ok(node::Struct(body)) => body.get_fields().map(|f| f.len()).unwrap_or(0),
            _ => 0,
        }
    };
    let first_field_meta_type = |node_id: u64| -> Option<u64> {
        let node::Struct(body) = find(node_id).which().ok()? else {
            return None;
        };
        let first = body.get_fields().ok()?.iter().find(|f| f.get_code_order() == 0)?;
        if first.get_name().ok()? != "meta" {
            return None;
        }
        let field::Slot(slot) = first.which().ok()? else {
            return None;
        };
        match slot.get_type().ok()?.which().ok()? {
            type_::Struct(s) => Some(s.get_type_id()),
            _ => None,
        }
    };

    let meta_ids: Vec<(String, u64)> = nodes
        .iter()
        .filter(|node| {
            matches!(node.which(), Ok(node::Struct(_)))
                && node
                    .get_display_name()
                    .map(|n| n.to_str().unwrap_or_default().starts_with("meta.capnp:"))
                    .unwrap_or(false)
        })
        .filter_map(|node| {
            let name = node.get_display_name().ok()?.to_str().ok()?.to_string();
            Some((name, node.get_id()))
        })
        .collect();
    let id_of = |name: &str| {
        let wanted = format!("meta.capnp:{name}");
        meta_ids
            .iter()
            .find(|(display, _)| *display == wanted)
            .map(|(_, id)| *id)
            .ok_or_else(|| format!("meta.capnp must define {name}"))
    };
    let request_meta = id_of("RequestMeta")?;
    let response_meta = id_of("ResponseMeta")?;

    let mut missing = Vec::new();
    for node in nodes.iter() {
        let Ok(node::Interface(interface)) = node.which() else {
            continue;
        };
        let owner = node
            .get_display_name()
            .ok()
            .and_then(|n| n.to_str().ok())
            .unwrap_or("?")
            .to_string();
        for method in interface.get_methods().map_err(|e| e.to_string())?.iter() {
            let name = method.get_name().ok().and_then(|n| n.to_str().ok()).unwrap_or("?");
            if first_field_meta_type(method.get_param_struct_type()) != Some(request_meta) {
                missing.push(format!("{owner}.{name} must take `meta :Meta.RequestMeta` as its first parameter"));
            }
            let results = method.get_result_struct_type();
            if field_count(results) != 0 && first_field_meta_type(results) != Some(response_meta) {
                missing.push(format!("{owner}.{name} must return `meta :Meta.ResponseMeta` as its first result"));
            }
        }
    }
    if missing.is_empty() { Ok(()) } else { Err(missing.join("\n")) }
}
