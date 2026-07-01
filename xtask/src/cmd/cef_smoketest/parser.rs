use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SocketViolation {
    pub sample: String,
    pub reason: SocketViolationReason,
    pub pid: String,
    pub cmd: String,
    pub proto: String,
    pub state: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocketViolationReason {
    NonLoopbackRemote,
    NonLoopbackBind,
}

impl fmt::Display for SocketViolationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonLoopbackRemote => f.write_str("non-loopback remote"),
            Self::NonLoopbackBind => f.write_str("non-loopback bind"),
        }
    }
}

impl fmt::Display for SocketViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} sample={} pid={} cmd={} proto={} state={} name={}",
            self.reason, self.sample, self.pid, self.cmd, self.proto, self.state, self.name
        )
    }
}

pub fn endpoint_host(endpoint: &str) -> String {
    let endpoint = endpoint.split_whitespace().next().unwrap_or_default();

    if endpoint == "::1" {
        return endpoint.to_string();
    }

    if let Some(rest) = endpoint.strip_prefix('[') {
        if let Some((host, _)) = rest.split_once("]:") {
            return host.to_string();
        }
        if let Some((host, _)) = rest.split_once(']') {
            return host.to_string();
        }
    }

    endpoint
        .rsplit_once(':')
        .map_or_else(|| endpoint.to_string(), |(host, _)| host.to_string())
}

pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.starts_with("127.") || host == "::1"
}

pub fn classify_socket(
    sample: &str,
    pid: &str,
    cmd: &str,
    proto: &str,
    state: &str,
    name: &str,
) -> Option<SocketViolation> {
    if let Some((_, remote)) = name.rsplit_once("->") {
        let remote_host = endpoint_host(remote);
        if is_loopback_host(&remote_host) {
            return None;
        }

        return Some(SocketViolation {
            sample: sample.to_string(),
            reason: SocketViolationReason::NonLoopbackRemote,
            pid: pid.to_string(),
            cmd: cmd.to_string(),
            proto: proto.to_string(),
            state: state.to_string(),
            name: name.to_string(),
        });
    }

    let bind_host = endpoint_host(name);
    if is_loopback_host(&bind_host) {
        return None;
    }

    Some(SocketViolation {
        sample: sample.to_string(),
        reason: SocketViolationReason::NonLoopbackBind,
        pid: pid.to_string(),
        cmd: cmd.to_string(),
        proto: proto.to_string(),
        state: state.to_string(),
        name: name.to_string(),
    })
}

pub fn classify_lsof_fields(sample: &str, raw_fields: &[u8]) -> Vec<SocketViolation> {
    let fields = String::from_utf8_lossy(raw_fields).replace('\0', "\n");
    let mut pid = String::new();
    let mut cmd = String::new();
    let mut proto = String::new();
    let mut state = String::new();
    let mut violations = Vec::new();

    for field in fields.lines().filter(|field| !field.is_empty()) {
        if let Some(value) = field.strip_prefix('p') {
            pid = value.to_string();
        } else if let Some(value) = field.strip_prefix('c') {
            cmd = value.to_string();
        } else if let Some(value) = field.strip_prefix('P') {
            proto = value.to_string();
        } else if let Some(value) = field.strip_prefix("TST=") {
            state = value.to_string();
        } else if field.starts_with('T') {
            continue;
        } else if let Some(name) = field.strip_prefix('n') {
            if let Some(violation) = classify_socket(sample, &pid, &cmd, &proto, &state, name) {
                violations.push(violation);
            }
            proto.clear();
            state.clear();
        }
    }

    violations
}

pub fn fixture_stream(name: &str) -> Option<Vec<u8>> {
    let stream = match name {
        "loopback-listener" => b"p1234\0czbag\0PTCP\0TST=LISTEN\0n127.0.0.1:7777\0".as_slice(),
        "wildcard-listener" => b"p1234\0czbag\0PTCP\0TST=LISTEN\0n*:7777\0".as_slice(),
        "zero-listener" => b"p1234\0czbag\0PTCP\0TST=LISTEN\0n0.0.0.0:7777\0".as_slice(),
        "external-connected" => {
            b"p1234\0czbag\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->142.250.190.78:443\0"
                .as_slice()
        }
        "loopback-connected" => {
            b"p1234\0czbag\0PTCP\0TST=ESTABLISHED\0n127.0.0.1:54321->127.0.0.1:7777\0".as_slice()
        }
        _ => return None,
    };

    Some(stream.to_vec())
}
