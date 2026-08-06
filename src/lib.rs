pub mod windows_capnp {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/windows_capnp.rs"));
}

pub mod linux_capnp {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/linux_capnp.rs"));
}
