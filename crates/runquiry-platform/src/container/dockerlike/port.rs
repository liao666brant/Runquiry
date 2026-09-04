//! Docker 发布端口筛选。

use runquiry_core::{CommandSpec, DiagnosticIssue, Port};

use super::{
    DockerLikeBin, ListedContainer, StdCommandRunner, parse_line_delimited, require_exit_zero,
    run_for_list, to_listed,
};

/// 按 Docker 发布端口筛选容器，argv 顺序固定且不经过 shell。
pub(crate) fn published_on(
    bin: &DockerLikeBin,
    runner: StdCommandRunner,
    port: Port,
) -> Result<(Vec<ListedContainer>, Vec<DiagnosticIssue>), DiagnosticIssue> {
    let filter = format!("publish={port}");
    let spec = CommandSpec::new(
        &bin.program,
        [
            "ps",
            "--filter",
            &filter,
            "--no-trunc",
            "--format",
            bin.list_format,
        ],
    );
    let output = run_for_list(bin, runner, &spec)?;
    require_exit_zero(bin.runtime, &output)?;
    let entries = parse_line_delimited(bin.runtime, &output.stdout)?;
    Ok((
        entries
            .into_iter()
            .map(|entry| to_listed(bin.runtime, entry))
            .collect(),
        Vec::new(),
    ))
}
