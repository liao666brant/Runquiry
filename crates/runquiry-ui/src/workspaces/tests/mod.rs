mod common;
mod containers;
mod file_locks;
mod ports;

use runquiry_core::{Pid, Port};

fn pid(value: u32) -> Pid {
    Pid::new(value).unwrap_or(Pid::MIN)
}

fn port(value: u16) -> Port {
    Port::new(value).unwrap_or(Port::MIN)
}
