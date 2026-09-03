//! gallery 的合成数据。
//!
//! 全部为常量：不读取本机进程、端口、文件、容器或环境变量。条目刻意包含长中文
//! 名称与无空格长路径，用于检验截断、省略与中英文混排。
//!
//! 本模块是 example 内部的私有模块：条目用 `pub(crate)` 暴露给 `main.rs`，
//! 这里显式豁免 `redundant_pub_crate`（否则与 `unreachable_pub` 互相冲突）。

#![allow(clippy::redundant_pub_crate)]

/// 合成的进程行。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Row {
    /// 稳定领域 ID（用于元素身份与选择保持）。
    pub(crate) id: &'static str,
    /// 进程名（含长中文条目）。
    pub(crate) name: &'static str,
    /// 可执行路径（含无空格长路径条目）。
    pub(crate) path: &'static str,
    /// 进程 ID。
    pub(crate) pid: u32,
    /// 监听端口（无监听为 0）。
    pub(crate) port: u16,
}

/// 合成的进程清单。
pub(crate) const ROWS: [Row; 8] = [
    Row {
        id: "row-database-keeper",
        name: "数据库连接池维护守护进程（长名称示例）",
        path: "/usr/lib/postgresql/16/bin/postgres-checkpoint-worker",
        pid: 1834,
        port: 5432,
    },
    Row {
        id: "row-container-shim",
        name: "containerd-shim-runc-v2",
        path: "/var/lib/containerd/io.containerd.runtime.v2.task/k8s.io/demo-backend-7f9c6d8b4-x2qtl/rootfs/usr/local/bin/entrypoint-controller",
        pid: 2210,
        port: 8080,
    },
    Row {
        id: "row-snapshot-sync",
        name: "容器运行时快照同步与清理调度器",
        path: "/opt/runquiry/fixtures/snapshot-sync-worker",
        pid: 2487,
        port: 0,
    },
    Row {
        id: "row-shell-render",
        name: "gnome-shell-render-thread-worker",
        path: "/usr/bin/gnome-shell",
        pid: 3120,
        port: 0,
    },
    Row {
        id: "row-long-path-agent",
        name: "metrics-agent",
        path: "/run/user/1000/app/org.example.LongRunningServiceName@instance.service/workspace/build/debug/incremental-cache-collector",
        pid: 4102,
        port: 9100,
    },
    Row {
        id: "row-ssh-session",
        name: "sshd: session worker",
        path: "/usr/sbin/sshd",
        pid: 4377,
        port: 22,
    },
    Row {
        id: "row-cache-compactor",
        name: "本地缓存压实与索引重建服务（夜间批处理）",
        path: "/var/lib/runquiry/fixtures/cache-compactor",
        pid: 5021,
        port: 0,
    },
    Row {
        id: "row-gpu-helper",
        name: "gpu-helper",
        path: "/usr/lib/x86_64-linux-gnu/libcuda-helper/registered-process",
        pid: 5988,
        port: 0,
    },
];

/// 合成的来源树：进程 → 来源（容器/服务/会话）。
pub(crate) fn tree_items() -> Vec<gpui_component::tree::TreeItem> {
    use gpui_component::tree::TreeItem;

    vec![
        TreeItem::new("tree-container", "Container")
            .expanded(true)
            .children([
                TreeItem::new("tree-container-docker", "docker")
                    .expanded(true)
                    .children([TreeItem::new(
                        "tree-container-docker-entrypoint",
                        "entrypoint-controller（容器内主进程）",
                    )]),
                TreeItem::new("tree-container-podman", "podman").children([TreeItem::new(
                    "tree-container-podman-worker",
                    "snapshot-sync-worker",
                )]),
            ]),
        TreeItem::new("tree-systemd", "systemd")
            .expanded(true)
            .children([
                TreeItem::new(
                    "tree-systemd-unit",
                    "org.example.LongRunningServiceName@instance.service",
                ),
                TreeItem::new("tree-systemd-timer", "数据库备份定时任务（每日 02:30）"),
            ]),
        TreeItem::new("tree-ssh", "SSH").children([TreeItem::new(
            "tree-ssh-session",
            "sshd: session worker (ops@10.0.0.42)",
        )]),
        TreeItem::new("tree-shell", "Shell").children([TreeItem::new(
            "tree-shell-job",
            "cache-compactor（后台作业）",
        )]),
    ]
}
