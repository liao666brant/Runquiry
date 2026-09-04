//! 公共领域类型集成测试入口；按值对象、诊断和 socket 契约拆分。

#[path = "domain_types/inspection.rs"]
mod inspection;
#[path = "domain_types/serialization.rs"]
mod serialization;
#[path = "domain_types/socket.rs"]
mod socket;
#[path = "domain_types/value_objects.rs"]
mod value_objects;
