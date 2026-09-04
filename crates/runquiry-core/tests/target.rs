//! 目标解析集成测试入口；职责测试位于同名目录。

mod support;

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

#[path = "target/containers.rs"]
mod containers;
#[path = "target/files.rs"]
mod files;
#[path = "target/matchers.rs"]
mod matchers;
#[path = "target/name.rs"]
mod name;
#[path = "target/parsing.rs"]
mod parsing;
#[path = "target/ports.rs"]
mod ports;
#[path = "target/socket_validation.rs"]
mod socket_validation;
