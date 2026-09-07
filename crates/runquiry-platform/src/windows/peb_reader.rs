//! 远程 PEB 字符串读取编排（仅 Windows 编译）：执行 [`peb`] 的有界读取
//! 计划，产出命令行 / 可执行路径 / 工作目录 / 环境块的部分成功结果。
//!
//! 边界（契约）：UNICODE_STRING 长度校验、缓冲区总量受限（witr 语义：
//! 字符串 ≤ 32 KiB、环境块 ≤ 128 KiB 分块读取）、实际读取字节数必须等于
//! 请求量；读取中进程退出 / PEB 变化 → 已得字段保留、缺失字段进诊断，
//! 不返回伪造数据。环境变量值不写入日志（调用方同样不得打印）。

use runquiry_core::{DiagnosticCode, DiagnosticIssue};

use super::ffi::{self, HandleGuard};
use super::peb::{self, PebFieldError, RemoteString};
use super::utf16;
use super::winerror::{Win32Error, diagnostic_for};

/// 一次远程 PEB 读取的结果：字段按可得性填充，单字段失败只加诊断。
#[derive(Debug, Default)]
pub struct PebRead {
    /// 完整命令行。
    pub command_line: Option<String>,
    /// PEB 内的镜像路径（注意：可能与 QueryFullProcessImageNameW 的
    /// 大小写 / 短路径形式不同）。
    pub image_path: Option<String>,
    /// 工作目录。
    pub working_dir: Option<String>,
    /// 环境变量键值（`environment` 键值，不含敏感日志）。
    pub environment: Option<Vec<(String, String)>>,
    /// 单字段失败诊断（部分成功语义）。
    pub issues: Vec<DiagnosticIssue>,
}

impl PebRead {
    /// 是否所有字段都不可得（调用方据此判断 PEB 通道整体不可用）。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.command_line.is_none()
            && self.image_path.is_none()
            && self.working_dir.is_none()
            && self.environment.is_none()
    }
}

/// 以已打开的进程句柄读取 PEB 字符串（64 位与 WOW64 目标进程）。
#[must_use]
pub fn read_remote_strings(handle: &HandleGuard) -> PebRead {
    let mut read = PebRead::default();
    // WOW64 判定失败（罕见）按 64 位目标处理并记诊断。
    let is_wow64 = match ffi::is_wow64(handle) {
        Some(wow64) => wow64,
        None => {
            read.issues.push(DiagnosticIssue::new(
                DiagnosticCode::Unknown,
                String::from("IsWow64Process 失败，按 64 位 PEB 布局读取"),
            ));
            false
        }
    };
    let Some(peb_address) = if is_wow64 {
        ffi::nt_wow64_peb_address(handle)
    } else {
        ffi::nt_peb_address(handle)
    } else {
        read.issues.push(DiagnosticIssue::new(
            DiagnosticCode::Unknown,
            String::from("NtQueryInformationProcess 未返回 PEB 地址（受保护进程或读取失败）"),
        ));
        return read;
    };
    let layout = peb::layout_for(is_wow64);
    let plan = peb::build_plan(&layout, peb_address);
    let mut pointer_buffer = [0u8; 8];
    let pointer_bytes = layout.pointer_bytes;
    if let Err(error) = ffi::read_remote_memory(
        handle,
        plan.params_ptr.0,
        &mut pointer_buffer[..pointer_bytes],
    ) {
        read.issues.push(diagnostic_for(error, "PEB ProcessParameters 指针"));
        return read;
    }
    let Some(params_address) = peb::extract_pointer(&pointer_buffer, pointer_bytes) else {
        read.issues.push(DiagnosticIssue::new(
            DiagnosticCode::ParseFailed,
            String::from("PEB ProcessParameters 指针宽度与布局不符"),
        ));
        return read;
    };
    let mut params = vec![0u8; layout.params_bytes];
    if let Err(error) = ffi::read_remote_memory(handle, params_address, &mut params) {
        // 进程在读取中退出：已读部分不足以解释任何字段 → 整体不可得。
        read.issues.push(diagnostic_for(error, "PEB ProcessParameters 结构"));
        return read;
    }
    for (field, target) in [
        ("工作目录", Field::WorkingDir),
        ("镜像路径", Field::ImagePath),
        ("命令行", Field::CommandLine),
    ] {
        let offset = match target {
            Field::WorkingDir => layout.cwd_offset,
            Field::ImagePath => layout.image_offset,
            Field::CommandLine => layout.cmdline_offset,
        };
        match peb::remote_string_field(&params, &layout, offset) {
            Ok(Some(remote)) => {
                if let Err(error) = read_remote_string(handle, &remote, &mut read, target) {
                    read.issues.push(diagnostic_for(error, field));
                }
            }
            Ok(None) => {} // 空串 / Buffer 为 0：字段不可得，不算故障。
            Err(PebFieldError::ParamsTooShort { .. }) => {
                read.issues.push(DiagnosticIssue::new(
                    DiagnosticCode::ParseFailed,
                    format!("进程读取中退出，{field} 的 PEB 结构不完整"),
                ));
            }
            Err(PebFieldError::PointerExtract | PebFieldError::InvalidUnicodeString) => {
                read.issues.push(DiagnosticIssue::new(
                    DiagnosticCode::ParseFailed,
                    format!("{field} 的 UNICODE_STRING 校验失败（读取中 PEB 变化）"),
                ));
            }
        }
    }
    match peb::environment_pointer(&params, &layout) {
        Some(0) | None => {}
        Some(address) => {
            let (environment, error) = read_environment_block(handle, address);
            if let Some(pairs) = environment {
                read.environment = Some(pairs);
            }
            if let Some(error) = error {
                read.issues.push(diagnostic_for(error, "环境块"));
            }
        }
    }
    read
}

#[derive(Clone, Copy)]
enum Field {
    WorkingDir,
    ImagePath,
    CommandLine,
}

/// 读取单个远程 UNICODE_STRING 指向的缓冲区并按目标字段写入。
fn read_remote_string(
    handle: &HandleGuard,
    remote: &RemoteString,
    read: &mut PebRead,
    target: Field,
) -> Result<(), Win32Error> {
    // 奇数字节长度截断为完整单元（witr readUnicodeString 语义）。
    let byte_count = usize::try_from(remote.length_bytes).unwrap_or(0) & !1;
    let mut buffer = vec![0u8; byte_count];
    ffi::read_remote_memory(handle, remote.buffer, &mut buffer)?;
    let text = utf16::decode_lossy(&buffer);
    if text.is_empty() {
        return Ok(());
    }
    match target {
        Field::WorkingDir => read.working_dir = Some(text),
        Field::ImagePath => read.image_path = Some(text),
        Field::CommandLine => read.command_line = Some(text),
    }
    Ok(())
}

/// 分块读取环境块（4 KiB 步进，总量 ≤ 128 KiB），返回解析键值与可选错误。
fn read_environment_block(
    handle: &HandleGuard,
    address: u64,
) -> (Option<Vec<(String, String)>>, Option<Win32Error>) {
    let (max_bytes, chunk_bytes) = peb::env_block_limits();
    let mut block: Vec<u16> = Vec::new();
    loop {
        let mut chunk = vec![0u8; chunk_bytes];
        let chunk_address = address + u64::try_from(block.len() * 2).unwrap_or(u64::MAX);
        if let Err(error) = ffi::read_remote_memory(handle, chunk_address, &mut chunk) {
            // 读取失败：按已读长度截断（契约），不丢弃已有内容；部分数据与
            // 错误同时返回——调用方把二者分别表达为部分结果与诊断，不得无
            // 诊断静默截断。
            if block.is_empty() {
                return (None, Some(error));
            }
            let parsed = utf16::parse_env_block(&block);
            return (Some(parsed.pairs), Some(error));
        }
        block.extend(utf16_units(&chunk));
        if utf16::scan_env_block_end(&block).is_some() {
            break;
        }
        if block.len() * 2 >= max_bytes {
            // 无终止符：按已读长度截断（契约行为，非失败）。
            break;
        }
    }
    let parsed = utf16::parse_env_block(&block);
    (Some(parsed.pairs), None)
}

/// LE u16 单元视图（奇尾字节丢弃）。
fn utf16_units(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}
