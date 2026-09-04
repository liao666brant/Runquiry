//! 子进程启动与进程组回收。

#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use runquiry_core::CommandSpec;

use super::CommandFailure;
#[cfg(unix)]
use super::original_user::OriginalUser;

const ETXTBSY: i32 = 26;

pub(super) fn spawn(spec: &CommandSpec, original_user: bool) -> Result<Child, CommandFailure> {
    let mut last_err: Option<std::io::Error> = None;
    let mut builder = Command::new(&spec.program);
    builder
        .args(&spec.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    builder.process_group(0);
    #[cfg(unix)]
    if original_user && let Some(user) = OriginalUser::from_process() {
        user.apply(&mut builder);
    }
    #[cfg(not(unix))]
    let _ = original_user;

    for attempt in 0..=5_u32 {
        match builder.spawn() {
            Ok(child) => return Ok(child),
            Err(err) if err.raw_os_error() == Some(ETXTBSY) && attempt < 5 => {
                thread::sleep(Duration::from_millis(5 * u64::from(attempt + 1)));
                last_err = Some(err);
            }
            Err(err) => return Err(spawn_failure(spec, &err)),
        }
    }
    last_err.map_or_else(
        || {
            Err(CommandFailure::Spawn {
                program: spec.program.clone(),
                detail: String::from("子进程启动重试耗尽"),
            })
        },
        |error| Err(spawn_failure(spec, &error)),
    )
}

fn spawn_failure(spec: &CommandSpec, error: &std::io::Error) -> CommandFailure {
    CommandFailure::Spawn {
        program: spec.program.clone(),
        detail: error.to_string(),
    }
}

pub(super) fn reap(child: &mut Child) {
    kill_process_group(child.id());
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
fn kill_process_group(process_id: u32) {
    let Ok(pgid) = i32::try_from(process_id) else {
        return;
    };
    // SAFETY: [Category 8 — FFI boundary] kill(2) receives no pointers. The
    // checked child pid is the process-group id assigned by `process_group(0)`.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process_group(_process_id: u32) {}
