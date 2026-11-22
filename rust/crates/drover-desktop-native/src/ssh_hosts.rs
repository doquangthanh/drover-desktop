//! Hosts Drover can offer when adding a machine, so a host is one click and not typed: the `Host` names in
//! `~/.ssh/config` first (they carry the user, port and key), then host names in `~/.ssh/known_hosts` (machines you
//! have already connected to). Only plain text entries are read; hashed ones cannot be turned back into names.

use std::net::IpAddr;
use std::path::PathBuf;

/// How many hosts are offered at most.
const MAX_HOSTS: usize = 12;

/// The `Host` names of an ssh config. Patterns (`*`, `?`, `!`) are not hosts and are left out.
pub fn parse_config(text: &str) -> Vec<String> {
    let mut out = vec![];
    for line in text.lines().map(str::trim) {
        let mut words = line.split_whitespace();
        if !words.next().is_some_and(|w| w.eq_ignore_ascii_case("host")) {
            continue;
        }
        for name in words.map(|w| w.trim_matches('"')) {
            if !name.is_empty() && !name.contains(['*', '?', '!']) && !out.iter().any(|x| x == name) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Host names in known_hosts text: no hashed entries, no bare IP addresses, `[host]:port` as `host`.
pub fn parse_known_hosts(text: &str) -> Vec<String> {
    let mut out = vec![];
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('@')) {
        for entry in line.split_whitespace().next().unwrap_or("").split(',') {
            let host = match entry.strip_prefix('[').and_then(|e| e.split_once(']')) {
                Some((h, _)) => h,
                None => entry,
            };
            if host.is_empty() || host.starts_with('|') || host.contains(['*', '?', '!']) || host.parse::<IpAddr>().is_ok() {
                continue;
            }
            if !out.iter().any(|x| x == host) {
                out.push(host.to_string());
            }
        }
    }
    out
}

fn ssh_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".ssh"))
}

/// Hosts found on this computer, newest knowledge first: config names, then known_hosts names (at most `MAX_HOSTS`).
pub fn detect() -> Vec<String> {
    let Some(dir) = ssh_dir() else { return vec![] };
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    let mut hosts = parse_config(&read("config"));
    for h in parse_known_hosts(&read("known_hosts")) {
        if !hosts.contains(&h) {
            hosts.push(h);
        }
    }
    hosts.truncate(MAX_HOSTS);
    hosts
}

/// The name a machine gets when none is typed: the host part of `user@host`, made to pass the daemon's rule
/// (lowercase letters, digits, - and _, up to 32, starting with a letter or digit; machines_edit::alias_ok):
/// `Box.Example` -> `box-example`, `100.65.65.71` -> `100-65-65-71`. `taken` names get -2, -3…
pub fn alias_for(target: &str, taken: &[&str]) -> String {
    let host = target.rsplit('@').next().unwrap_or(target).to_ascii_lowercase();
    let mut base = String::new();
    for c in host.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '-' }) {
        if !(c == '-' && (base.is_empty() || base.ends_with('-'))) {
            base.push(c);
        }
    }
    let mut base: String = base.trim_matches(|c| c == '-' || c == '_').chars().take(28).collect();
    // Names the daemon keeps for itself, or that read like its internal ids (s1, t3, a2).
    let internal = base.len() > 1 && base.starts_with(['s', 't', 'a']) && base[1..].chars().all(|c| c.is_ascii_digit());
    if base.is_empty() || ["local", "self", "this"].contains(&base.as_str()) || internal {
        base = format!("machine-{base}").trim_end_matches('-').to_string();
    }
    let mut name = base.clone();
    for n in 2.. {
        if !taken.contains(&name.as_str()) { break }
        name = format!("{base}-{n}");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_hosts_without_patterns() {
        let text = "Host workbox lab\n  HostName 10.0.0.5\nhost *\nHost \"vps-2\" *.corp !bad\n# Host nope\n";
        assert_eq!(parse_config(text), vec!["workbox", "lab", "vps-2"]);
    }

    #[test]
    fn known_hosts_names_only() {
        let text = "github.com,140.82.1.1 ssh-ed25519 AAAA\n[box.example]:2222 ssh-rsa BBBB\n|1|abc|def ssh-rsa CCCC\n10.1.2.3 ssh-rsa DDDD\n@cert-authority *.x ssh-rsa EEEE\n";
        assert_eq!(parse_known_hosts(text), vec!["github.com", "box.example"]);
    }

    #[test]
    fn alias_comes_from_the_host_and_passes_the_daemon_rule() {
        assert_eq!(alias_for("deploy@vps-2.tail-net", &[]), "vps-2-tail-net");
        assert_eq!(alias_for("workbox", &[]), "workbox");
        assert_eq!(alias_for("100.65.65.71", &[]), "100-65-65-71");
        assert_eq!(alias_for("me@Box.Example.COM", &[]), "box-example-com");
        assert_eq!(alias_for("me@[::1]", &[]), "1");
        assert_eq!(alias_for("local", &[]), "machine-local");
        assert_eq!(alias_for("s12", &[]), "machine-s12");
        assert_eq!(alias_for("@", &[]), "machine");
        for host in ["deploy@vps-2.tail-net", "100.65.65.71", "me@[::1]", "local", "x".repeat(60).as_str()] {
            assert!(crate::machines_edit::alias_ok(&alias_for(host, &[])), "{host}");
        }
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        assert_eq!(alias_for("u@box", &["box"]), "box-2");
        assert_eq!(alias_for("u@box", &["box", "box-2"]), "box-3");
    }
}
