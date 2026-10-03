//! `/etc/fstab` edits that leave every untouched line byte-for-byte.

use std::fmt::Write as _;

const NETWORK_TYPES: &[&str] = &[
    "cifs", "smb", "nfs", "nfs4", "fuse.sshfs", "sshfs", "davfs", "fuse.rclone",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fields {
    pub source: String,
    pub target: String,
    pub fstype: String,
    pub options: String,
    pub dump: String,
    pub pass: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Share {
    pub id: String,
    pub source: String,
    pub target: String,
    pub fstype: String,
    pub options: String,
    pub mounted: bool,
}

#[derive(Clone, Debug)]
pub struct ShareDraft {
    pub id: String,
    pub protocol: String,
    pub source: String,
    pub mount_point: String,
    pub guest: bool,
    pub username: String,
    pub password: String,
    pub credentials_file: String,
    pub identity_file: String,
    pub extra: String,
    pub netdev: bool,
    pub nofail: bool,
    pub automount: bool,
    pub idle_timeout: String,
    pub vers: String,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredWrite {
    pub path: String,
    pub contents: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharePlan {
    pub id: String,
    pub source: String,
    pub target: String,
    pub entry_line: String,
    pub credentials: Option<CredWrite>,
}

#[derive(Clone, Debug)]
pub enum Change {
    Upsert {
        id: String,
        source: String,
        target: String,
        line: String,
    },
    Delete {
        id: String,
        source: String,
        target: String,
    },
}

pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            ' ' => out.push_str("\\040"),
            '\t' => out.push_str("\\011"),
            '\n' => out.push_str("\\012"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

pub fn unescape(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'\\' {
                out.push('\\');
                i += 2;
                continue;
            }
            if i + 3 < bytes.len() && bytes[i + 1..i + 4].iter().all(|b| b.is_ascii_digit()) {
                let oct = &value[i + 1..i + 4];
                if let Ok(code) = u8::from_str_radix(oct, 8) {
                    out.push(code as char);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn parse_entry(line: &str) -> Option<Fields> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let parts = split_fields(trimmed);
    if parts.len() < 4 {
        return None;
    }
    Some(Fields {
        source: unescape(&parts[0]),
        target: unescape(&parts[1]),
        fstype: parts[2].clone(),
        options: unescape(&parts[3]),
        dump: parts.get(4).cloned().unwrap_or_else(|| "0".into()),
        pass: parts.get(5).cloned().unwrap_or_else(|| "0".into()),
    })
}

fn split_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            current.push('\\');
            if let Some(next) = chars.next() {
                current.push(next);
                // octal escapes are three digits; keep them inside the field
                if next.is_ascii_digit() {
                    for _ in 0..2 {
                        if chars.peek().is_some_and(|d| d.is_ascii_digit()) {
                            current.push(chars.next().unwrap());
                        }
                    }
                }
            }
            continue;
        }
        if c.is_whitespace() {
            if !current.is_empty() {
                fields.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        fields.push(current);
    }
    fields
}

pub fn is_network(fields: &Fields) -> bool {
    let kind = fields.fstype.to_ascii_lowercase();
    NETWORK_TYPES.iter().any(|t| *t == kind)
        || fields.options.split(',').any(|opt| {
            let key = opt.split('=').next().unwrap_or("").trim();
            key == "_netdev" || key == "x-systemd.automount"
        })
}

pub fn is_target_mounted(target: &str) -> bool {
    let Ok(mounts) = std::fs::read_to_string("/proc/mounts") else {
        return false;
    };
    let target = target.trim_end_matches('/');
    mounts.lines().any(|line| {
        let mut parts = line.split_whitespace();
        let _src = parts.next();
        let tgt = parts.next().map(unescape);
        tgt.as_deref().map(|t| t.trim_end_matches('/')) == Some(target)
    })
}

pub fn network_shares(text: &str) -> Vec<Share> {
    let lines = lines_of(text);
    let mut shares = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(fields) = parse_entry(line) else {
            continue;
        };
        if !is_network(&fields) {
            continue;
        }
        let id = previous_id(&lines, index).unwrap_or_else(|| slug(&fields.target));
        let mounted = is_target_mounted(&fields.target);
        shares.push(Share {
            id,
            source: fields.source,
            target: fields.target,
            fstype: fields.fstype,
            options: fields.options,
            mounted,
        });
    }
    shares
}

fn previous_id(lines: &[String], index: usize) -> Option<String> {
    if index == 0 {
        return None;
    }
    let prev = lines[index - 1].trim();
    prev.strip_prefix("# disktui:")
        .map(|rest| rest.trim().to_string())
        .filter(|id| !id.is_empty())
}

pub fn slug(target: &str) -> String {
    let mut out = String::new();
    for c in target.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "share".into()
    } else {
        out
    }
}

/// Apply one insert, replace, or delete. Every line that is not the edited
/// entry is copied through unchanged.
pub fn transform(original: &str, change: &Change) -> String {
    let newline_at_eof = original.is_empty() || original.ends_with('\n');
    let mut lines = lines_of(original);
    match change {
        Change::Delete { id, source, target } => {
            if let Some(idx) = find_entry(&lines, id, source, target) {
                let start = comment_start(&lines, idx);
                lines.drain(start..=idx);
            }
        }
        Change::Upsert {
            id,
            source,
            target,
            line,
        } => {
            let comment = format!("# disktui: {id}");
            if let Some(idx) = find_entry(&lines, id, source, target) {
                let start = comment_start(&lines, idx);
                lines.splice(start..=idx, [comment, line.clone()]);
            } else {
                if lines.last().is_some_and(|l| l.is_empty()) {
                    lines.pop();
                }
                if lines.last().is_some_and(|l| !l.is_empty()) {
                    // keep a blank line between the existing file and our entry
                }
                lines.push(comment);
                lines.push(line.clone());
            }
        }
    }
    let mut out = lines.join("\n");
    if (newline_at_eof || matches!(change, Change::Upsert { .. })) && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if original.is_empty() && matches!(change, Change::Delete { .. }) {
        return String::new();
    }
    out
}

fn lines_of(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    trimmed.split('\n').map(|s| s.to_string()).collect()
}

fn comment_start(lines: &[String], idx: usize) -> usize {
    if idx > 0 && is_disktui_comment(&lines[idx - 1]) {
        idx - 1
    } else {
        idx
    }
}

fn is_disktui_comment(line: &str) -> bool {
    line.trim().starts_with("# disktui:")
}

fn find_entry(lines: &[String], id: &str, source: &str, target: &str) -> Option<usize> {
    let marker = format!("# disktui: {id}");
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == marker {
            if let Some(j) = ((i + 1)..lines.len()).find(|j| parse_entry(&lines[*j]).is_some()) {
                return Some(j);
            }
        }
    }
    lines.iter().position(|line| match parse_entry(line) {
        Some(fields) => fields.source == source && fields.target == target && is_network(&fields),
        None => false,
    })
}

/// Normalize user-entered share source URLs into standard Linux fstab format.
/// Also suggests protocol and share name.
pub fn normalize_source(input: &str, current_proto: &str) -> (String, String, Option<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), current_proto.to_string(), None);
    }
    // Convert backslashes (Windows-style) to slashes
    let converted = trimmed.replace('\\', "/");
    let mut s = converted.as_str();
    let mut proto = current_proto.to_string();

    if let Some(rest) = s.strip_prefix("smb://") {
        proto = "cifs".into();
        s = rest;
    } else if let Some(rest) = s.strip_prefix("cifs://") {
        proto = "cifs".into();
        s = rest;
    } else if let Some(rest) = s.strip_prefix("nfs://") {
        proto = "nfs".into();
        s = rest;
    } else if let Some(rest) = s.strip_prefix("ssh://") {
        proto = "fuse.sshfs".into();
        s = rest;
    } else if let Some(rest) = s.strip_prefix("sftp://") {
        proto = "fuse.sshfs".into();
        s = rest;
    }

    let normalized = if proto == "nfs" || proto == "nfs4" {
        if let Some((host, path)) = s.split_once(":/") {
            format!("{}:/{}", host.trim_matches('/'), path)
        } else if let Some((host, path)) = s.split_once(':') {
            format!("{}:/{}", host.trim_matches('/'), path.trim_start_matches('/'))
        } else if let Some((host, path)) = s.trim_start_matches('/').split_once('/') {
            format!("{}:/{}", host, path)
        } else {
            s.to_string()
        }
    } else if proto == "fuse.sshfs" || proto == "sshfs" {
        if s.contains(':') {
            s.to_string()
        } else if let Some((host, path)) = s.trim_start_matches('/').split_once('/') {
            format!("{}:/{}", host, path)
        } else {
            s.to_string()
        }
    } else if proto == "cifs" || proto == "smb" {
        let stripped = s.trim_start_matches('/');
        format!("//{}", stripped)
    } else {
        s.to_string()
    };

    let share_name = extract_share_name(&normalized);
    (normalized, proto, share_name)
}

pub fn extract_share_name(source: &str) -> Option<String> {
    let converted = source.replace('\\', "/");
    let clean = converted.trim_end_matches('/');
    let last_seg = if let Some((_, path)) = clean.split_once(':') {
        path.rsplit('/').next()
    } else {
        clean.rsplit('/').next()
    };
    last_seg
        .map(|s| s.trim_matches('/').trim_matches('\\').to_string())
        .filter(|s| !s.is_empty() && !s.contains(':') && !s.starts_with('/') && !s.starts_with('\\'))
}

pub fn plan_share(draft: &ShareDraft) -> Result<SharePlan, String> {
    let (source, protocol, suggested_name) = normalize_source(&draft.source, &draft.protocol);
    let target = {
        let t = draft.mount_point.trim();
        if t.is_empty() || t == "/mnt" || t == "/mnt/" {
            if let Some(name) = &suggested_name {
                format!("/mnt/{name}")
            } else {
                "/mnt/share".into()
            }
        } else {
            t.to_string()
        }
    };
    if !target.starts_with('/') {
        return Err("Mount point must be an absolute path (e.g. /mnt/photos).".into());
    }
    if target.contains('\n') || source.contains('\n') || draft.extra.contains('\n') {
        return Err("Fields cannot contain a newline.".into());
    }
    if source.is_empty() {
        return Err("Please enter the server address (e.g. //192.168.1.50/photos).".into());
    }
    if protocol.is_empty() {
        return Err("Filesystem type is empty.".into());
    }
    let id = {
        let raw = draft.id.trim();
        let id = if raw.is_empty() { slug(&target) } else { slug(raw) };
        if id.is_empty() {
            return Err("Share id is empty.".into());
        }
        id
    };

    let mut credentials = None;
    let cred_path = if protocol == "cifs" && !draft.guest {
        if !draft.credentials_file.trim().is_empty() {
            draft.credentials_file.trim().to_string()
        } else if !draft.username.trim().is_empty() {
            let path = format!("/etc/samba/credentials-{id}");
            if draft.password.contains('\n') || draft.username.contains('\n') {
                return Err("Username and password cannot contain a newline.".into());
            }
            credentials = Some(CredWrite {
                path: path.clone(),
                contents: format!(
                    "username={}\npassword={}\n",
                    draft.username.trim(),
                    draft.password
                ),
            });
            path
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut draft_with_proto = draft.clone();
    draft_with_proto.protocol = protocol.clone();
    let options = build_options(&draft_with_proto, &cred_path);
    let line = format!(
        "{source} {target} {protocol} {options} 0 0",
        source = escape(&source),
        target = escape(&target),
        protocol = protocol,
        options = escape(&options),
    );
    if line.contains(draft.password.as_str()) && !draft.password.is_empty() && protocol == "cifs" {
        return Err("Refusing to write the password into fstab. Use a credentials file.".into());
    }
    Ok(SharePlan {
        id,
        source,
        target,
        entry_line: line,
        credentials,
    })
}

fn build_options(draft: &ShareDraft, cred_path: &str) -> String {
    let managed = [
        "_netdev",
        "nofail",
        "x-systemd.automount",
        "x-systemd.idle-timeout",
        "guest",
        "credentials",
        "username",
        "password",
        "uid",
        "gid",
        "file_mode",
        "dir_mode",
        "iocharset",
        "vers",
        "IdentityFile",
        "identityfile",
        "reconnect",
        "ServerAliveInterval",
    ];
    let mut parts: Vec<String> = Vec::new();
    for opt in draft.extra.split(',') {
        let opt = opt.trim();
        if opt.is_empty() {
            continue;
        }
        let key = opt.split('=').next().unwrap_or("").trim();
        if managed.contains(&key) {
            continue;
        }
        parts.push(opt.to_string());
    }
    let cifs = draft.protocol == "cifs";
    let ssh = draft.protocol == "fuse.sshfs" || draft.protocol == "sshfs";
    if cifs {
        if draft.guest {
            parts.push("guest".into());
        } else if !cred_path.is_empty() {
            parts.push(format!("credentials={cred_path}"));
        }
        parts.push(format!("uid={}", draft.uid));
        parts.push(format!("gid={}", draft.gid));
        parts.push("file_mode=0644".into());
        parts.push("dir_mode=0755".into());
        parts.push("iocharset=utf8".into());
        if !draft.vers.trim().is_empty() && draft.vers != "default" {
            parts.push(format!("vers={}", draft.vers.trim()));
        }
    }
    if ssh {
        if !draft.identity_file.trim().is_empty() {
            parts.push(format!("IdentityFile={}", draft.identity_file.trim()));
        }
        parts.push("reconnect".into());
        parts.push("ServerAliveInterval=15".into());
    }
    if draft.netdev {
        parts.push("_netdev".into());
    }
    if draft.nofail {
        parts.push("nofail".into());
    }
    if draft.automount {
        parts.push("x-systemd.automount".into());
        if let Ok(seconds) = draft.idle_timeout.trim().parse::<u64>() {
            if seconds > 0 {
                parts.push(format!("x-systemd.idle-timeout={seconds}"));
            }
        }
    }
    if parts.is_empty() {
        "defaults".into()
    } else {
        parts.join(",")
    }
}

pub fn helper_for(fstype: &str) -> Option<&'static str> {
    match fstype {
        "cifs" | "smb" => Some("mount.cifs"),
        "nfs" | "nfs4" => Some("mount.nfs"),
        "fuse.sshfs" | "sshfs" => Some("sshfs"),
        "davfs" => Some("mount.davfs"),
        _ => None,
    }
}

pub fn missing_helper(fstype: &str) -> Option<String> {
    let helper = helper_for(fstype)?;
    let candidates = [
        format!("/usr/bin/{helper}"),
        format!("/usr/sbin/{helper}"),
        format!("/bin/{helper}"),
        format!("/sbin/{helper}"),
    ];
    if candidates.iter().any(|p| std::path::Path::new(p).exists()) {
        None
    } else {
        let package = match fstype {
            "cifs" | "smb" => "cifs-utils",
            "nfs" | "nfs4" => "nfs-utils",
            "fuse.sshfs" | "sshfs" => "sshfs",
            "davfs" => "davfs2",
            _ => helper,
        };
        Some(format!("{helper} is not installed (package {package})"))
    }
}

/// Shell-quote a single argument. Used when the sudo script embeds a path.
pub fn sh_quote(value: &str) -> String {
    let mut out = String::from("'");
    for c in value.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

pub fn render_script(backup: &str, plan_contents: &str, creds: Option<&CredWrite>, mount: Option<&str>) -> String {
    let mut script = String::from("set -eu\n");
    let _ = writeln!(script, "cp -a /etc/fstab {}", sh_quote(backup));
    if let Some(cred) = creds {
        let dir = std::path::Path::new(&cred.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/etc/samba".into());
        let _ = writeln!(script, "install -d -m 755 {}", sh_quote(&dir));
        let _ = writeln!(script, "cat > {} << 'DISKTUI_CRED'", sh_quote(&cred.path));
        script.push_str(&cred.contents);
        if !cred.contents.ends_with('\n') {
            script.push('\n');
        }
        script.push_str("DISKTUI_CRED\n");
        let _ = writeln!(script, "chmod 600 {}", sh_quote(&cred.path));
    }
    script.push_str("cat > /etc/fstab << 'DISKTUI_FSTAB'\n");
    script.push_str(plan_contents);
    if !plan_contents.ends_with('\n') {
        script.push('\n');
    }
    script.push_str("DISKTUI_FSTAB\n");
    script.push_str("chmod 644 /etc/fstab\n");
    if let Some(target) = mount {
        let _ = writeln!(script, "install -d -m 755 {}", sh_quote(target));
        let _ = writeln!(script, "mount -- {}", sh_quote(target));
    }
    script
}

pub fn explain_mount_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("no route to host") || lower.contains("network is unreachable") || lower.contains("error(113)") {
        "Could not reach server. Check that your NAS is powered on and IP/address is correct.".into()
    } else if lower.contains("permission denied") || lower.contains("error(13)") || lower.contains("logon failure") {
        "Access denied. Check your username, password, or server share permissions.".into()
    } else if lower.contains("no such file or directory") || lower.contains("error(2)") {
        "Share not found. Check that the share folder name on the server is correct.".into()
    } else if lower.contains("connection refused") || lower.contains("error(111)") {
        "Connection refused. The server is not accepting connections for this protocol.".into()
    } else if lower.contains("mount.cifs: command not found") || lower.contains("cannot find mount.cifs") {
        "cifs-utils is not installed. Install with: sudo pacman -S cifs-utils".into()
    } else if lower.contains("mount.nfs: command not found") {
        "nfs-utils is not installed. Install with: sudo pacman -S nfs-utils".into()
    } else if lower.contains("sshfs: command not found") {
        "sshfs is not installed. Install with: sudo pacman -S sshfs".into()
    } else if lower.contains("mount point does not exist") {
        "Mount folder does not exist on this machine.".into()
    } else {
        crate::util::brief(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Static information about the filesystems.\n\
UUID=9574fe74-248e-45d8-bdbf-269b1ddcfd9f\t/\tbtrfs\trw,relatime,subvol=/@\t0\t0\n\
UUID=0356-B9FA  /boot  vfat  rw,relatime  0  2\n\
/swap/swapfile none swap defaults 0 0\n";

    fn draft() -> ShareDraft {
        ShareDraft {
            id: String::new(),
            protocol: "cifs".into(),
            source: "//nas/media".into(),
            mount_point: "/mnt/media".into(),
            guest: false,
            username: "ken".into(),
            password: "s3cret".into(),
            credentials_file: String::new(),
            identity_file: String::new(),
            extra: String::new(),
            netdev: true,
            nofail: true,
            automount: false,
            idle_timeout: String::new(),
            vers: "3.1.1".into(),
            uid: 1000,
            gid: 1000,
        }
    }

    #[test]
    fn upsert_preserves_unrelated_lines() {
        let plan = plan_share(&draft()).unwrap();
        assert!(!plan.entry_line.contains("s3cret"));
        assert!(plan.entry_line.contains("credentials=/etc/samba/credentials-mnt-media"));
        let next = transform(
            SAMPLE,
            &Change::Upsert {
                id: plan.id.clone(),
                source: plan.source.clone(),
                target: plan.target.clone(),
                line: plan.entry_line.clone(),
            },
        );
        assert!(next.contains("UUID=9574fe74-248e-45d8-bdbf-269b1ddcfd9f\t/\tbtrfs\trw,relatime,subvol=/@\t0\t0\n"));
        assert!(next.contains("UUID=0356-B9FA  /boot  vfat  rw,relatime  0  2\n"));
        assert!(next.contains("# disktui: mnt-media\n"));
        assert!(next.contains(&plan.entry_line));
        let again = transform(
            &next,
            &Change::Upsert {
                id: "mnt-media".into(),
                source: "//nas/media".into(),
                target: "/mnt/media".into(),
                line: "//nas/media /mnt/photos cifs credentials=/etc/samba/credentials-mnt-media,_netdev 0 0".into(),
            },
        );
        assert_eq!(again.matches("# disktui:").count(), 1);
        assert!(again.contains("/mnt/photos"));
        assert!(!again.contains("/mnt/media cifs"));
        assert!(again.contains("subvol=/@"));
    }

    #[test]
    fn delete_removes_only_the_marked_entry() {
        let plan = plan_share(&draft()).unwrap();
        let with = transform(
            SAMPLE,
            &Change::Upsert {
                id: plan.id.clone(),
                source: plan.source,
                target: plan.target.clone(),
                line: plan.entry_line,
            },
        );
        let cleared = transform(
            &with,
            &Change::Delete {
                id: "mnt-media".into(),
                source: "//nas/media".into(),
                target: "/mnt/media".into(),
            },
        );
        assert_eq!(cleared, SAMPLE);
    }

    #[test]
    fn escapes_spaces_in_the_mount_point() {
        let mut d = draft();
        d.mount_point = "/mnt/my media".into();
        d.guest = true;
        d.password.clear();
        let plan = plan_share(&d).unwrap();
        assert!(plan.entry_line.contains("/mnt/my\\040media"));
        let fields = parse_entry(&plan.entry_line).unwrap();
        assert_eq!(fields.target, "/mnt/my media");
        assert!(fields.options.contains("guest"));
        assert!(!fields.options.contains("credentials="));
    }

    #[test]
    fn nfs_and_existing_manual_entry() {
        let mut d = draft();
        d.protocol = "nfs".into();
        d.source = "nas:/export".into();
        d.mount_point = "/mnt/export".into();
        let plan = plan_share(&d).unwrap();
        assert!(plan.credentials.is_none());
        assert!(plan.entry_line.contains("nfs"));
        let manual = format!("{SAMPLE}nas:/export /mnt/export nfs _netdev 0 0\n");
        let next = transform(
            &manual,
            &Change::Upsert {
                id: plan.id,
                source: plan.source,
                target: plan.target,
                line: plan.entry_line,
            },
        );
        assert_eq!(next.matches("nas:/export").count(), 1);
        assert!(next.contains("# disktui:"));
        let shares = network_shares(&next);
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].fstype, "nfs");
    }

    #[test]
    fn password_never_lands_in_the_fstab_line() {
        let plan = plan_share(&draft()).unwrap();
        let creds = plan.credentials.unwrap();
        assert!(creds.contents.contains("password=s3cret"));
        assert!(!plan.entry_line.contains("s3cret"));
    }

    #[test]
    fn normalize_source_handles_windows_and_schemes() {
        let (src, proto, name) = normalize_source(r"\\192.168.1.50\photos", "cifs");
        assert_eq!(src, "//192.168.1.50/photos");
        assert_eq!(proto, "cifs");
        assert_eq!(name.as_deref(), Some("photos"));

        let (src, proto, name) = normalize_source("smb://nas.local/music", "custom");
        assert_eq!(src, "//nas.local/music");
        assert_eq!(proto, "cifs");
        assert_eq!(name.as_deref(), Some("music"));

        let (src, proto, name) = normalize_source("nfs://nas.local/exports/data", "cifs");
        assert_eq!(src, "nas.local:/exports/data");
        assert_eq!(proto, "nfs");
        assert_eq!(name.as_deref(), Some("data"));

        let (src, proto, name) = normalize_source("user@server.com:/var/www", "fuse.sshfs");
        assert_eq!(src, "user@server.com:/var/www");
        assert_eq!(proto, "fuse.sshfs");
        assert_eq!(name.as_deref(), Some("www"));
    }

    #[test]
    fn render_script_creates_mount_directory() {
        let script = render_script("/etc/fstab.bak", "sample", None, Some("/mnt/photos"));
        assert!(script.contains("install -d -m 755 '/mnt/photos'"));
        assert!(script.contains("mount -- '/mnt/photos'"));
    }

    #[test]
    fn explain_mount_error_gives_friendly_messages() {
        let msg = explain_mount_error("mount error(113): No route to host");
        assert!(msg.contains("Could not reach server"));

        let msg2 = explain_mount_error("mount error(13): Permission denied");
        assert!(msg2.contains("Access denied"));

        let msg3 = explain_mount_error("mount error(2): No such file or directory");
        assert!(msg3.contains("Share not found"));

        let msg4 = explain_mount_error("/sbin/mount.cifs: command not found");
        assert!(msg4.contains("cifs-utils is not installed"));
    }
}
