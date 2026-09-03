//! gallery 的 CLI 参数解析与用法输出。
//!
//! 从 `main.rs` 原样搬入（A4 拆分），行为不变。
//!
//! 本模块是 example 内部的私有模块：条目用 `pub(crate)` 暴露给 `main.rs`，
//! 这里显式豁免 `redundant_pub_crate`（否则与 `unreachable_pub` 互相冲突）。

#![allow(clippy::redundant_pub_crate)]

use gpui_component::ThemeMode;

use crate::overlays::Overlay;
use runquiry_ui::{DataState, Lang};

/// 默认窗口尺寸。
pub(crate) const DEFAULT_SIZE: (f32, f32) = (1280., 800.);
/// 最小窗口尺寸（DESIGN.md §7）。
pub(crate) const MIN_SIZE: (f32, f32) = (960., 640.);

/// gallery 的 CLI 参数。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Args {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) mode: ThemeMode,
    pub(crate) lang: Lang,
    pub(crate) state: DataState,
    pub(crate) open: Option<Overlay>,
}

pub(crate) fn parse_args<I: Iterator<Item = String>>(mut args: I) -> Result<Args, String> {
    let mut parsed = Args {
        width: DEFAULT_SIZE.0,
        height: DEFAULT_SIZE.1,
        mode: ThemeMode::Light,
        lang: Lang::En,
        state: DataState::Ready,
        open: None,
    };

    while let Some(arg) = args.next() {
        let mut take_value = |name: &str| args.next().ok_or_else(|| format!("{name} 缺少取值"));
        match arg.as_str() {
            "--size" => {
                let value = take_value("--size")?;
                let (width, height) = value
                    .split_once('x')
                    .ok_or_else(|| format!("--size 需要形如 1280x800，得到 {value}"))?;
                parsed.width = width
                    .parse()
                    .map_err(|_| format!("--size 宽度非法：{width}"))?;
                parsed.height = height
                    .parse()
                    .map_err(|_| format!("--size 高度非法：{height}"))?;
            }
            "--theme" => {
                let value = take_value("--theme")?;
                parsed.mode = match value.as_str() {
                    "light" => ThemeMode::Light,
                    "dark" => ThemeMode::Dark,
                    other => return Err(format!("--theme 只接受 light|dark，得到 {other}")),
                };
            }
            "--lang" => {
                let value = take_value("--lang")?;
                parsed.lang = Lang::parse(&value)
                    .ok_or_else(|| format!("--lang 只接受 en|zh-CN，得到 {value}"))?;
            }
            "--state" => {
                let value = take_value("--state")?;
                parsed.state =
                    DataState::parse(&value).ok_or_else(|| format!("--state 取值非法：{value}"))?;
            }
            "--open" => {
                let value = take_value("--open")?;
                parsed.open = Some(match value.as_str() {
                    "sheet" => Overlay::Sheet,
                    "alert" => Overlay::Alert,
                    "notification" => Overlay::Notification,
                    other => {
                        return Err(format!(
                            "--open 只接受 sheet|alert|notification，得到 {other}"
                        ));
                    }
                });
            }
            other => return Err(format!("未知参数 {other}")),
        }
    }

    // NaN 与任何数比较都为 false，会绕过下面的最小尺寸检查，须先排除；
    // 无穷大虽无比较问题，但同样不是合法窗口尺寸。
    if !parsed.width.is_finite() || !parsed.height.is_finite() {
        return Err(String::from("--size 需要有限的宽高数值"));
    }
    if parsed.width < MIN_SIZE.0 || parsed.height < MIN_SIZE.1 {
        return Err(format!(
            "--size 不得小于最小窗口 {}x{}",
            MIN_SIZE.0, MIN_SIZE.1
        ));
    }

    Ok(parsed)
}

/// 开发用示例的 CLI 错误输出：gallery 不是产品入口，stderr 是唯一可用通道。
#[allow(clippy::print_stderr)]
pub(crate) fn print_cli_error(message: &str) {
    eprintln!("gallery: {message}");
    eprintln!(
        "用法: gallery [--size 1280x800] [--theme light|dark] [--lang en|zh-CN] \
         [--state ready|loading|empty|error|unsupported|permission-denied] \
         [--open sheet|alert|notification]"
    );
}
