//! PEB 布局常量与有界读取计划（纯逻辑：只产出「读什么、读多长」）。

/// 64 位 PEB 内 ProcessParameters 指针的字节偏移。
pub const PEB64_PARAMS_PTR_OFFSET: usize = 0x20;
/// 32 位（WOW64）PEB 内 ProcessParameters32 指针的字节偏移。
pub const PEB32_PARAMS_PTR_OFFSET: usize = 0x10;

/// UNICODE_STRING64：`Length/MaximumLength` u16 + 填充 + Buffer 指针 @8。
const US64_BYTES: usize = 16;
/// UNICODE_STRING32：`Length/MaximumLength` u16 + Buffer u32 @4。
const US32_BYTES: usize = 8;

/// 平台布局：一次远程读取的偏移、宽度与步长全部由此驱动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PebLayout {
    /// 是否为 WOW64（64 位 Runquiry 读 32 位进程）。
    pub is_wow64: bool,
    /// PEB → ProcessParameters 指针的字节偏移。
    pub params_ptr_offset: usize,
    /// 指针宽度（8 / 4 字节）。
    pub pointer_bytes: usize,
    /// ProcessParameters 结构的读取字节数（覆盖到 Environment 指针末尾）。
    pub params_bytes: usize,
    /// CurrentDirectory.DosPath 的 UNICODE_STRING 偏移。
    pub cwd_offset: usize,
    /// ImagePathName 偏移。
    pub image_offset: usize,
    /// CommandLine 偏移。
    pub cmdline_offset: usize,
    /// Environment 指针偏移。
    pub env_offset: usize,
    /// UNICODE_STRING 结构字节数。
    pub unicode_string_bytes: usize,
    /// Buffer 指针在 UNICODE_STRING 内的偏移。
    pub string_buffer_offset: usize,
}

impl PebLayout {
    /// 64 位目标进程的布局（witr x64 分支偏移）。
    #[must_use]
    pub const fn win64() -> Self {
        Self {
            is_wow64: false,
            params_ptr_offset: PEB64_PARAMS_PTR_OFFSET,
            pointer_bytes: 8,
            params_bytes: 136, // 0x88：Environment 指针 @0x80 + 8 字节
            cwd_offset: 0x38,
            image_offset: 0x60,
            cmdline_offset: 0x70,
            env_offset: 0x80,
            unicode_string_bytes: US64_BYTES,
            string_buffer_offset: 8,
        }
    }

    /// 32 位（WOW64）目标进程的布局（witr 32 位分支偏移）。
    #[must_use]
    pub const fn wow64() -> Self {
        Self {
            is_wow64: true,
            params_ptr_offset: PEB32_PARAMS_PTR_OFFSET,
            pointer_bytes: 4,
            params_bytes: 80, // Environment u32 @72 + 4
            cwd_offset: 36,
            image_offset: 56,
            cmdline_offset: 64,
            env_offset: 72,
            unicode_string_bytes: US32_BYTES,
            string_buffer_offset: 4,
        }
    }
}

/// 按 WOW64 判定选取布局。
#[must_use]
pub const fn layout_for(is_wow64: bool) -> PebLayout {
    if is_wow64 {
        PebLayout::wow64()
    } else {
        PebLayout::win64()
    }
}

/// 远程读取计划：按序执行的 (远端地址, 本地读取字节数) 步骤。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadPlan {
    /// 第一步：读 ProcessParameters 指针（PEB 内）。
    pub params_ptr: (u64, usize),
    /// 第二步：读 ProcessParameters 结构（覆盖到 Environment 指针）。
    pub params_struct: (u64, usize),
}

/// 以 PEB 基址构造有界读取计划（地址来自 NtQueryInformationProcess）。
#[must_use]
pub const fn build_plan(layout: &PebLayout, peb_address: u64) -> ReadPlan {
    let (offset, bytes) = (layout.params_ptr_offset as u64, layout.pointer_bytes as u64);
    ReadPlan {
        params_ptr: (peb_address + offset, bytes as usize),
        params_struct: (0, layout.params_bytes),
    }
}

/// 从本地已读缓冲区按宽度提取指针（LE；32 位零扩展）。
#[must_use]
pub fn extract_pointer(buf: &[u8], width: usize) -> Option<u64> {
    if width != 4 && width != 8 {
        return None;
    }
    if buf.len() < width {
        return None;
    }
    let mut value: u64 = 0;
    let mut index = 0;
    while index < width {
        value |= u64::from(buf[index]) << (index * 8);
        index += 1;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::{
        PEB32_PARAMS_PTR_OFFSET, PEB64_PARAMS_PTR_OFFSET, PebLayout, build_plan, extract_pointer,
        layout_for,
    };

    #[test]
    fn win64_layout_matches_documented_abi() {
        let layout = PebLayout::win64();
        assert_eq!(layout.params_ptr_offset, PEB64_PARAMS_PTR_OFFSET);
        assert_eq!(layout.params_ptr_offset, 0x20);
        assert_eq!(layout.cwd_offset, 0x38);
        assert_eq!(layout.image_offset, 0x60);
        assert_eq!(layout.cmdline_offset, 0x70);
        assert_eq!(layout.env_offset, 0x80);
        assert_eq!(layout.pointer_bytes, 8);
        assert_eq!(layout.unicode_string_bytes, 16);
        assert_eq!(layout.string_buffer_offset, 8);
    }

    #[test]
    fn wow64_layout_uses_32_bit_offsets() {
        let layout = PebLayout::wow64();
        assert_eq!(layout.params_ptr_offset, PEB32_PARAMS_PTR_OFFSET);
        assert_eq!(layout.params_ptr_offset, 0x10);
        assert_eq!(layout.cwd_offset, 36);
        assert_eq!(layout.image_offset, 56);
        assert_eq!(layout.cmdline_offset, 64);
        assert_eq!(layout.env_offset, 72);
        assert_eq!(layout.pointer_bytes, 4);
        assert_eq!(layout.unicode_string_bytes, 8);
        assert_eq!(layout.string_buffer_offset, 4);
    }

    #[test]
    fn layout_selection_follows_wow64_flag() {
        assert_eq!(layout_for(false), PebLayout::win64());
        assert_eq!(layout_for(true), PebLayout::wow64());
    }

    #[test]
    fn plan_is_bounded_and_address_derived() {
        let layout = PebLayout::win64();
        let plan = build_plan(&layout, 0x0000_7FF6_1234_5000);
        assert_eq!(plan.params_ptr, (0x0000_7FF6_1234_5000 + 0x20, 8));
        assert_eq!(plan.params_struct.1, 136);
    }

    #[test]
    fn pointer_extraction_zero_extends_32_bit() {
        let bytes = [0x34, 0x12, 0x00, 0x00];
        assert_eq!(extract_pointer(&bytes, 4), Some(0x1234));
        let wide = [0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(extract_pointer(&wide, 8), Some(0x1234_5678));
        assert_eq!(extract_pointer(&bytes, 8), None);
    }
}