#[path = "../build/meta.rs"]
mod meta;

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn check(schema: &str) -> Result<(), String> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "uniproc-protocol-meta-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let meta_source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema/meta.capnp");
    std::fs::copy(meta_source, dir.join("meta.capnp")).unwrap();
    std::fs::write(dir.join("x.capnp"), schema).unwrap();
    let output = Command::new("capnp")
        .arg("compile")
        .arg("-o-")
        .arg(format!("--src-prefix={}", dir.display()))
        .arg(dir.join("meta.capnp"))
        .arg(dir.join("x.capnp"))
        .output()
        .expect("failed to run `capnp compile`; is the `capnp` binary installed?");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let message = capnp::serialize::read_message(&mut output.stdout.as_slice(), capnp::message::ReaderOptions::new())
        .unwrap();
    meta::check_every_method_carries_meta(message.get_root().unwrap())
}

fn schema(method: &str) -> String {
    format!("@0xc4e1a2b3d4e5f607;\nusing Meta = import \"meta.capnp\";\ninterface Svc {{\n  {method}\n}}\n")
}

#[test]
fn a_method_with_meta_first_on_both_sides_passes() {
    assert_eq!(
        check(&schema("a @0 (meta :Meta.RequestMeta, x :UInt32) -> (meta :Meta.ResponseMeta, y :Text);")),
        Ok(())
    );
}

#[test]
fn a_method_without_results_needs_no_response_meta() {
    assert_eq!(check(&schema("a @0 (meta :Meta.RequestMeta) -> ();")), Ok(()));
}

#[test]
fn results_without_meta_are_refused() {
    let refused = check(&schema("a @0 (meta :Meta.RequestMeta) -> (y :Text);")).unwrap_err();
    assert!(refused.contains("Svc.a must return"), "{refused}");
}

#[test]
fn meta_that_is_not_first_is_refused() {
    let refused = check(&schema("a @0 (x :UInt32, meta :Meta.RequestMeta) -> ();")).unwrap_err();
    assert!(refused.contains("Svc.a must take"), "{refused}");
    let refused =
        check(&schema("a @0 (meta :Meta.RequestMeta) -> (y :Text, meta :Meta.ResponseMeta);")).unwrap_err();
    assert!(refused.contains("Svc.a must return"), "{refused}");
}

#[test]
fn params_without_meta_are_refused() {
    let refused = check(&schema("a @0 (x :UInt32) -> (meta :Meta.ResponseMeta);")).unwrap_err();
    assert!(refused.contains("Svc.a must take"), "{refused}");
}
