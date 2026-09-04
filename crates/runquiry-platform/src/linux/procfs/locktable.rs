//! `/proc/locks` 的文本解析。

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::linux) struct LockRow {
    /// 锁类型原词（POSIX / FLOCK / OFDLCK / …）。
    pub(in crate::linux) lock_type: String,
    /// 模式原词（READ / WRITE / RW）。
    pub(in crate::linux) mode: String,
    /// 持有者 PID。
    pub(in crate::linux) pid: u32,
    /// `maj:min:ino` 原文（路径解析失败时的兜底展示值）。
    pub(in crate::linux) dev_inode: String,
    /// 文件 inode（十进制，匹配 FD 的依据）。
    pub(in crate::linux) inode: u64,
}

/// 解析 `/proc/locks`（`<id>: <TYPE> <KIND> <ACCESS> <PID> <MAJ:MIN:INODE>
/// <START> <END>`；maj/min 为十六进制，inode 为十进制）。列不足 8 或 PID/inode
/// 非法的行跳过。
pub(in crate::linux) fn parse_locks(raw: &str) -> Vec<LockRow> {
    let mut rows = Vec::new();
    for line in raw.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 8 {
            continue;
        }
        let Ok(pid) = fields[4].parse::<u32>() else {
            continue;
        };
        let segments: Vec<&str> = fields[5].split(':').collect();
        let Ok(inode) = segments.last().copied().unwrap_or("").parse::<u64>() else {
            continue;
        };
        rows.push(LockRow {
            lock_type: String::from(fields[1]),
            mode: String::from(fields[3]),
            pid,
            dev_inode: String::from(fields[5]),
            inode,
        });
    }
    rows
}
