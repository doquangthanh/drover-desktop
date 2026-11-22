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

/// The name a machine gets when none is typed: the host part of `user@host`, safe as a file key.
pub fn alias_for(target: &str) -> String {
    let host = target.rsplit('@').next().unwrap_or(target);
    host.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' }).collect()
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
    fn alias_comes_from_the_host() {
        assert_eq!(alias_for("deploy@vps-2.tail-net"), "vps-2.tail-net");
        assert_eq!(alias_for("workbox"), "workbox");
        assert_eq!(alias_for("me@[::1]"), "---1-");
    }
}
