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

/// Identity and version of one link's protocol, taken from its schema file:
/// `id` is the file id, the version is its `const version`. Pass the fields to
/// `ogurpchik::auth::handshake::Protocol::new` on both ends of the link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolInfo {
    pub id: u64,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

include!(concat!(env!("OUT_DIR"), "/protocols.rs"));

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
    fn each_link_is_its_own_protocol() {
        assert_ne!(WINDOWS_PROTOCOL.id, LINUX_PROTOCOL.id);
    }

    #[test]
    fn the_protocol_id_is_the_schema_file_id() {
        assert_eq!(WINDOWS_PROTOCOL.id, 0xfd5a_69cd_5615_08f1);
        assert_eq!(LINUX_PROTOCOL.id, 0xd0b0_0dd2_6d1a_5151);
    }
}
