//! Remote URL validation and sanitization.
//!
//! RepoDNA only clones URLs it can reason about. Transport-helper syntax such as
//! `ext::<command>` (which asks Git to run a program), `file://` and other local transports,
//! option-like values, and control characters are rejected. Plain `http://` and `git://`
//! require an explicit opt-in, and private or loopback hosts are refused unless allowed,
//! which prevents a hosted deployment from being used to probe internal networks (SSRF).
//! Host names are checked literally; DNS names that resolve to private addresses are a
//! documented limitation.
//!
//! Credentials embedded in HTTP(S) URLs are never stored or displayed: every URL that
//! leaves this module for artifacts or logs goes through [`sanitize_url`].

use std::net::IpAddr;

use crate::error::GitError;

/// Which otherwise-rejected URLs are acceptable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UrlPolicy {
    /// Accept unencrypted `http://` and `git://` URLs.
    pub allow_insecure: bool,
    /// Accept loopback, private, link-local, and `.local`/`.internal` hosts.
    pub allow_private_hosts: bool,
}

/// A remote URL broken into its parts, with credentials removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRemote {
    /// The URL without credentials.
    pub url: String,
    /// Host name, lowercase.
    pub host: Option<String>,
    /// `github`, `gitlab`, `bitbucket`, `azure`, `codeberg`, `sourcehut`, or `other`.
    pub provider: String,
    /// Owner or namespace (may contain `/` for nested groups).
    pub owner: Option<String>,
    /// Repository name without a `.git` suffix.
    pub name: Option<String>,
}

struct Parts<'a> {
    scheme: Option<String>,
    userinfo: Option<&'a str>,
    host: &'a str,
    path: &'a str,
}

fn split(url: &str) -> Option<Parts<'_>> {
    if let Some((scheme, rest)) = url.split_once("://") {
        let (authority, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
        let (userinfo, host_port) = match authority.rsplit_once('@') {
            Some((userinfo, host)) => (Some(userinfo), host),
            None => (None, authority),
        };
        let host = if host_port.starts_with('[') {
            host_port
                .split_once(']')
                .map_or(host_port, |(h, _)| &h[1..])
        } else {
            host_port.split(':').next().unwrap_or(host_port)
        };
        return Some(Parts {
            scheme: Some(scheme.to_ascii_lowercase()),
            userinfo,
            host,
            path,
        });
    }
    // scp-like syntax: [user@]host:path
    let (before, path) = url.split_once(':')?;
    if before.contains('/') || path.starts_with("//") {
        return None;
    }
    let (userinfo, host) = match before.rsplit_once('@') {
        Some((user, host)) => (Some(user), host),
        None => (None, before),
    };
    // A single letter before ':' is a Windows drive, not a host.
    if host.len() <= 1 || host.is_empty() {
        return None;
    }
    Some(Parts {
        scheme: None,
        userinfo,
        host,
        path,
    })
}

/// Returns `true` when `input` looks like a remote URL rather than a local path.
pub fn looks_like_url(input: &str) -> bool {
    let input = input.trim();
    if input.contains("://") {
        return true;
    }
    split(input).is_some_and(|parts| parts.userinfo.is_some() && parts.host.contains('.'))
}

/// Removes credentials from an HTTP(S) URL. Other URL forms are returned unchanged
/// (their user names, such as `git@`, are not secrets).
pub fn sanitize_url(url: &str) -> String {
    let trimmed = url.trim();
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return trimmed.to_owned();
    };
    let scheme_lower = scheme.to_ascii_lowercase();
    let (authority, path) = rest
        .split_once('/')
        .map_or((rest, None), |(a, p)| (a, Some(p)));
    let keep_user = !matches!(scheme_lower.as_str(), "http" | "https");
    let authority = match authority.rsplit_once('@') {
        Some((userinfo, host)) => {
            if keep_user && !userinfo.contains(':') {
                format!("{userinfo}@{host}")
            } else {
                host.to_owned()
            }
        }
        None => authority.to_owned(),
    };
    match path {
        Some(path) => format!("{scheme}://{authority}/{path}"),
        None => format!("{scheme}://{authority}"),
    }
}

fn provider_for(host: &str) -> &'static str {
    match host {
        "github.com" | "www.github.com" => "github",
        "bitbucket.org" => "bitbucket",
        "dev.azure.com" | "ssh.dev.azure.com" => "azure",
        "codeberg.org" => "codeberg",
        "git.sr.ht" => "sourcehut",
        host if host.contains("gitlab") => "gitlab",
        host if host.ends_with("visualstudio.com") => "azure",
        _ => "other",
    }
}

/// Parses a remote URL into its parts, removing credentials.
pub fn parse_remote(url: &str) -> ParsedRemote {
    let sanitized = sanitize_url(url);
    let Some(parts) = split(&sanitized) else {
        return ParsedRemote {
            url: sanitized,
            host: None,
            provider: "other".to_owned(),
            owner: None,
            name: None,
        };
    };
    let host = parts.host.to_ascii_lowercase();
    let path = parts
        .path
        .trim_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/');
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let (owner, name) = match segments.split_last() {
        Some((name, owners)) if !owners.is_empty() => {
            (Some(owners.join("/")), Some((*name).to_owned()))
        }
        Some((name, _)) => (None, Some((*name).to_owned())),
        None => (None, None),
    };
    ParsedRemote {
        provider: provider_for(&host).to_owned(),
        url: sanitized,
        host: (!host.is_empty()).then_some(host),
        owner,
        name,
    }
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || (v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_private_ip(IpAddr::V4(v4)))
        }
    }
}

fn is_private_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
        || host.ends_with(".home.arpa")
    {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_private_ip(ip);
    }
    // Integer, octal, or hexadecimal IPv4 spellings such as 2130706433 or 0x7f000001 are
    // only used to disguise addresses, so they are treated as private.
    host.bytes().all(|b| b.is_ascii_digit() || b == b'.') || host.starts_with("0x")
}

/// Validates a URL for cloning and returns it unchanged (including any credentials the user
/// deliberately supplied, which Git needs; they are never stored or printed).
pub fn validate_clone_url(input: &str, policy: UrlPolicy) -> Result<String, GitError> {
    let url = input.trim();
    let reject = |reason: &str| {
        Err(GitError::InvalidUrl(format!(
            "{} ({reason})",
            sanitize_url(url)
        )))
    };
    if url.is_empty() {
        return reject("empty");
    }
    if url.starts_with('-') {
        return reject("looks like a command-line option");
    }
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return reject("contains whitespace or control characters");
    }
    let transport_helper = url
        .find("::")
        .is_some_and(|position| url.find("://").is_none_or(|scheme| position < scheme));
    if transport_helper {
        return reject("transport-helper syntax is not allowed");
    }
    let Some(parts) = split(url) else {
        return reject("not an https://, ssh://, or git@host:path URL");
    };
    match parts.scheme.as_deref() {
        None | Some("https" | "ssh" | "git+ssh" | "ssh+git") => {}
        Some("http" | "git") if policy.allow_insecure => {}
        Some("http" | "git") => {
            return reject("unencrypted protocol; pass --allow-insecure-url to permit it");
        }
        Some(_) => return reject("unsupported protocol"),
    }
    if parts.host.is_empty() {
        return reject("missing host");
    }
    if parts.host.starts_with('-') {
        return reject("host looks like a command-line option");
    }
    if !policy.allow_private_hosts && is_private_host(parts.host) {
        return reject("private, loopback, or local host");
    }
    if parts.path.trim_matches('/').is_empty() {
        return reject("missing repository path");
    }
    Ok(url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC: UrlPolicy = UrlPolicy {
        allow_insecure: false,
        allow_private_hosts: false,
    };

    #[test]
    fn accepts_common_remote_forms() {
        for url in [
            "https://github.com/sanskarIN/RepoDNA",
            "https://github.com/sanskarIN/RepoDNA.git",
            "git@github.com:sanskarIN/RepoDNA.git",
            "ssh://git@gitlab.com/group/sub/project.git",
            "https://gitlab.example.com/a/b",
        ] {
            assert!(validate_clone_url(url, PUBLIC).is_ok(), "{url}");
        }
    }

    #[test]
    fn rejects_dangerous_or_unsupported_urls() {
        for url in [
            "ext::sh -c touch% /tmp/pwned",
            "ext::sh",
            "file:///etc",
            "--upload-pack=touch /tmp/x",
            "fd::17",
            "ftp://example.com/repo",
            "https://",
            "https://github.com",
            "C:\\repos\\project",
            "/home/user/project",
            "https://exa mple.com/repo",
        ] {
            assert!(validate_clone_url(url, PUBLIC).is_err(), "{url}");
        }
    }

    #[test]
    fn insecure_and_private_hosts_require_opt_in() {
        assert!(validate_clone_url("http://example.com/r.git", PUBLIC).is_err());
        let insecure = UrlPolicy {
            allow_insecure: true,
            ..PUBLIC
        };
        assert!(validate_clone_url("http://example.com/r.git", insecure).is_ok());
        for url in [
            "https://localhost/r.git",
            "https://127.0.0.1/r.git",
            "https://10.0.0.5/r.git",
            "https://192.168.1.10/r.git",
            "https://169.254.169.254/latest",
            "https://[::1]/r.git",
            "https://2130706433/r.git",
            "https://git.corp.internal/r.git",
            "git@printer.local:r.git",
        ] {
            assert!(validate_clone_url(url, PUBLIC).is_err(), "{url}");
        }
        let private = UrlPolicy {
            allow_private_hosts: true,
            ..PUBLIC
        };
        assert!(validate_clone_url("https://10.0.0.5/r.git", private).is_ok());
    }

    #[test]
    fn sanitizes_credentials() {
        assert_eq!(
            sanitize_url("https://user:placeholder@github.com/o/r.git"),
            "https://github.com/o/r.git"
        );
        assert_eq!(
            sanitize_url("https://token@github.com/o/r"),
            "https://github.com/o/r"
        );
        assert_eq!(sanitize_url("ssh://git@host/o/r"), "ssh://git@host/o/r");
        assert_eq!(sanitize_url("ssh://git:pw@host/o/r"), "ssh://host/o/r");
        assert_eq!(
            sanitize_url("git@github.com:o/r.git"),
            "git@github.com:o/r.git"
        );
    }

    #[test]
    fn parses_owners_names_and_providers() {
        let remote = parse_remote("https://token@github.com/sanskarIN/RepoDNA.git");
        assert_eq!(remote.url, "https://github.com/sanskarIN/RepoDNA.git");
        assert_eq!(remote.provider, "github");
        assert_eq!(remote.owner.as_deref(), Some("sanskarIN"));
        assert_eq!(remote.name.as_deref(), Some("RepoDNA"));
        let nested = parse_remote("git@gitlab.com:group/sub/project.git");
        assert_eq!(nested.provider, "gitlab");
        assert_eq!(nested.owner.as_deref(), Some("group/sub"));
        assert_eq!(nested.name.as_deref(), Some("project"));
        assert_eq!(parse_remote("not a url").provider, "other");
    }

    #[test]
    fn distinguishes_urls_from_paths() {
        assert!(looks_like_url("https://github.com/o/r"));
        assert!(looks_like_url("git@github.com:o/r.git"));
        assert!(!looks_like_url("./project"));
        assert!(!looks_like_url("C:\\project"));
        assert!(!looks_like_url("project:backup"));
    }
}
