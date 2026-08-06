fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/windows.capnp")
        .file("schema/linux.capnp")
        .run()
        .expect("capnp schema compilation failed; is the `capnp` binary installed?");
    println!("cargo:rerun-if-changed=schema/windows.capnp");
    println!("cargo:rerun-if-changed=schema/linux.capnp");
}
