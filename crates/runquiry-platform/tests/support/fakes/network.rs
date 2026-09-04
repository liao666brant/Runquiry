use runquiry_core::{
    CapabilityStatus, Inspection, NetworkInventory, OpenPortEntry, Port, Protocol, SocketEntry,
};

use super::FakePlatform;

const FIXTURE_PORT: Port = match Port::new(8443) {
    Ok(port) => port,
    Err(_) => Port::MIN,
};

impl NetworkInventory for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn open_ports(&self) -> Inspection<Vec<OpenPortEntry>> {
        let entry = OpenPortEntry {
            pid: Some(self.baseline.pid()),
            port: FIXTURE_PORT,
            address: String::from("0.0.0.0"),
            protocol: Protocol::Tcp,
            state: String::from("LISTEN"),
        };
        self.inspect(vec![entry])
    }

    fn sockets_of(&self, pid: runquiry_core::Pid) -> Inspection<Vec<SocketEntry>> {
        let entry = SocketEntry {
            inode: None,
            port: Some(FIXTURE_PORT),
            address: String::from("0.0.0.0"),
            remote_addr: None,
            state: String::from("LISTEN"),
            protocol: Protocol::Tcp,
            owner_pid: Some(pid),
        };
        self.inspect(vec![entry])
    }
}
