#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedLink {
    pub start_col: usize,
    pub end_col: usize,
    pub target: String,
}

pub fn find_link_in_line(line: &[char], col: usize) -> Option<DetectedLink> {
    if col >= line.len() || line[col].is_whitespace() {
        return None;
    }

    let mut start = col;
    while start > 0 && !line[start - 1].is_whitespace() {
        start -= 1;
    }

    let mut end = col;
    while end + 1 < line.len() && !line[end + 1].is_whitespace() {
        end += 1;
    }

    while start <= end && edge_trim_char(line[start]) {
        start += 1;
    }
    while end >= start && edge_trim_char(line[end]) {
        if end == 0 {
            break;
        }
        end -= 1;
    }

    if start > end {
        return None;
    }

    let token: String = line[start..=end].iter().collect();
    let target = classify_link_token(token.trim_end_matches(':'))?;

    Some(DetectedLink {
        start_col: start,
        end_col: end,
        target,
    })
}

pub fn classify_link_token(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }

    let lower = token.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(token.to_string());
    }

    if lower.starts_with("www.") {
        return Some(format!("https://{}", token));
    }

    if is_ipv4_with_optional_port_and_path(token) || looks_like_domain(token) {
        return Some(format!("http://{}", token));
    }

    None
}

fn edge_trim_char(c: char) -> bool {
    matches!(
        c,
        '\'' | '"' | '`' | ',' | '.' | ';' | '!' | '?' | '(' | ')' | '[' | ']' | '{' | '}'
            | '<' | '>'
    )
}

fn is_ipv4_with_optional_port_and_path(input: &str) -> bool {
    let host_port = input.split('/').next().unwrap_or(input);
    let (host, port) = if let Some((host, port)) = host_port.rsplit_once(':') {
        (host, Some(port))
    } else {
        (host_port, None)
    };

    let octets: Vec<&str> = host.split('.').collect();
    if octets.len() != 4 {
        return false;
    }
    if octets
        .iter()
        .any(|octet| octet.is_empty() || octet.parse::<u8>().is_err())
    {
        return false;
    }

    if let Some(port) = port {
        if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if port.parse::<u16>().is_err() {
            return false;
        }
    }

    true
}

fn looks_like_domain(input: &str) -> bool {
    let host_port = input.split('/').next().unwrap_or(input);
    let (host, port) = if let Some((host, port)) = host_port.rsplit_once(':') {
        (host, Some(port))
    } else {
        (host_port, None)
    };

    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    if !host.contains('.') {
        return false;
    }

    for label in host.split('.') {
        if label.is_empty() {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }

    if let Some(port) = port {
        if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if port.parse::<u16>().is_err() {
            return false;
        }
    }

    true
}
