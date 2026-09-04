//! sudo 启动时恢复原始普通用户的身份与关键环境。

#[cfg(unix)]
use std::process::Command;

#[cfg(unix)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OriginalUser {
    pub(super) uid: u32,
    pub(super) gid: u32,
    pub(super) name: String,
    pub(super) home: String,
}

#[cfg(unix)]
impl OriginalUser {
    pub(super) fn from_process() -> Option<Self> {
        // SAFETY: [Category 8 — FFI boundary] `geteuid` has no arguments or memory
        // contract and returns the calling process's effective uid by value.
        let effective_uid = unsafe { libc::geteuid() };
        let uid = std::env::var("SUDO_UID").ok();
        let gid = std::env::var("SUDO_GID").ok();
        let name = std::env::var("SUDO_USER").ok();
        let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
        Self::parse(
            effective_uid,
            uid.as_deref(),
            gid.as_deref(),
            name.as_deref(),
            &passwd,
        )
    }

    pub(super) fn parse(
        effective_uid: u32,
        sudo_user_id: Option<&str>,
        sudo_group_id: Option<&str>,
        sudo_user: Option<&str>,
        passwd: &str,
    ) -> Option<Self> {
        if effective_uid != 0 {
            return None;
        }
        let uid = sudo_user_id?.parse::<u32>().ok()?;
        let gid = sudo_group_id?.parse::<u32>().ok()?;
        let name = sudo_user?;
        if uid == 0 || !valid_user_name(name) {
            return None;
        }
        let home = passwd
            .lines()
            .find_map(|line| parse_passwd_home(line, uid, gid, name))?;
        Some(Self {
            uid,
            gid,
            name: name.to_string(),
            home: home.to_string(),
        })
    }

    pub(super) fn apply(&self, command: &mut Command) {
        use std::os::unix::process::CommandExt;

        command.uid(self.uid).gid(self.gid).envs([
            ("HOME", self.home.clone()),
            ("USER", self.name.clone()),
            ("LOGNAME", self.name.clone()),
            ("XDG_RUNTIME_DIR", format!("/run/user/{}", self.uid)),
        ]);
    }
}

#[cfg(unix)]
fn valid_user_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains([':', '/', '\0'])
        && name.chars().all(|character| !character.is_control())
}

#[cfg(unix)]
fn parse_passwd_home<'a>(line: &'a str, uid: u32, gid: u32, name: &str) -> Option<&'a str> {
    let mut fields = line.split(':');
    let entry_name = fields.next()?;
    let _password = fields.next()?;
    let entry_user_id = fields.next()?.parse::<u32>().ok()?;
    let entry_group_id = fields.next()?.parse::<u32>().ok()?;
    let _gecos = fields.next()?;
    let home = fields.next()?;
    if entry_name == name
        && entry_user_id == uid
        && entry_group_id == gid
        && home.starts_with('/')
        && !home.contains('\0')
    {
        Some(home)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::OriginalUser;

    #[test]
    fn original_user_parses_complete_valid_sudo_identity() {
        let passwd = "alice:x:1000:1000:Alice:/home/alice:/bin/sh\n";

        let user = OriginalUser::parse(0, Some("1000"), Some("1000"), Some("alice"), passwd);

        assert_eq!(
            user,
            Some(OriginalUser {
                uid: 1000,
                gid: 1000,
                name: String::from("alice"),
                home: String::from("/home/alice"),
            })
        );
    }

    #[test]
    fn original_user_rejects_partial_invalid_or_non_root_sudo_identity() {
        let passwd = "alice:x:1000:1000:Alice:/home/alice:/bin/sh\n";
        let cases = [
            OriginalUser::parse(1000, Some("1000"), Some("1000"), Some("alice"), passwd),
            OriginalUser::parse(0, Some("0"), Some("1000"), Some("alice"), passwd),
            OriginalUser::parse(0, Some("1000"), None, Some("alice"), passwd),
            OriginalUser::parse(0, Some("bad"), Some("1000"), Some("alice"), passwd),
            OriginalUser::parse(0, Some("1000"), Some("1000"), Some("bob"), passwd),
            OriginalUser::parse(0, Some("1000"), Some("1001"), Some("alice"), passwd),
        ];

        assert!(cases.into_iter().all(|candidate| candidate.is_none()));
    }
}
