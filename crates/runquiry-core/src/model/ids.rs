//! 正整数型标识新类型：`Pid`、`Port` 与容器去重键 `ContainerKey`。
//!
//! 语义来自 [witr 行为契约](../../../../docs/witr-parity.md) §2：
//! PID 必须是正整数，端口必须在 1-65535 区间。

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 标识取值非法错误（如 PID 或端口为 0）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidId {
    /// 非法标识所属的类别名，如 `"pid"`、`"port"`。
    kind: &'static str,
    /// 被拒绝的原始数值。
    value: i64,
}

impl InvalidId {
    pub(crate) const fn new(kind: &'static str, value: i64) -> Self {
        Self { kind, value }
    }

    /// 非法标识的类别名。
    pub const fn kind(&self) -> &'static str {
        self.kind
    }

    /// 被拒绝的原始数值。
    pub const fn value(&self) -> i64 {
        self.value
    }
}

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} 必须是正整数，收到 {}", self.kind, self.value)
    }
}

impl std::error::Error for InvalidId {}

/// 进程标识（正整数，不含 0）。
///
/// 字段私有，因此非法值只能经 [`Pid::new`] 被拒绝，不存在非法实例。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pid(u32);

impl Pid {
    /// 最小合法 PID（`1`，通常是 init/systemd）。
    pub const MIN: Self = Self(1);

    /// 构造进程标识；0 表示「无进程」，被拒绝。
    pub const fn new(value: u32) -> Result<Self, InvalidId> {
        if value == 0 {
            Err(InvalidId::new("pid", 0))
        } else {
            Ok(Self(value))
        }
    }

    /// 读取原始数值。
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Pid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Pid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for Pid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// 端口号（1-65535，不含 0）。
///
/// 字段私有，因此非法值只能经 [`Port::new`] 被拒绝，不存在非法实例。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Port(u16);

impl Port {
    /// 最小合法端口（`1`）。
    pub const MIN: Self = Self(1);

    /// 最大合法端口（`65535`）。
    pub const MAX: Self = Self(u16::MAX);

    /// 构造端口号；0 被拒绝（parity：端口必须介于 1 与 65535 之间）。
    pub const fn new(value: u16) -> Result<Self, InvalidId> {
        if value == 0 {
            Err(InvalidId::new("port", 0))
        } else {
            Ok(Self(value))
        }
    }

    /// 读取原始数值。
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for Port {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> Deserialize<'de> for Port {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u16::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// 容器去重键：`runtime + id` 唯一标识一个容器（parity：容器按 runtime + id 去重）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContainerKey {
    /// 容器运行时名称，如 `docker`、`podman`、`incus`。
    pub runtime: String,
    /// 该运行时内的容器 ID（长 ID 优先）。
    pub id: String,
}

impl ContainerKey {
    /// 生成跨运行时的去重键字符串，形如 `docker|abc123`。
    pub fn dedup_key(&self) -> String {
        format!("{}|{}", self.runtime, self.id)
    }
}
