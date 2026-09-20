pub mod meta_capnp {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/meta_capnp.rs"));
}

pub mod windows_capnp {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/windows_capnp.rs"));
}

pub mod linux_capnp {
    #![allow(clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/linux_capnp.rs"));
}

include!(concat!(env!("OUT_DIR"), "/schema_ids.rs"));

/// Application name used to derive local endpoints (`\\.\pipe\<app>.<service>`).
pub const APP_NAME: &str = "uniproc";

/// Service name the Windows agent listens under.
pub const WINDOWS_AGENT_SERVICE: &str = "windows-agent";

/// vsock port the agent inside the WSL guest listens on.
pub const WSL_AGENT_VSOCK_PORT: u32 = 5000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_ids_are_distinct_per_link() {
        assert_ne!(WINDOWS_SCHEMA_ID, LINUX_SCHEMA_ID);
    }
}
