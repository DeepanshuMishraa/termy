fn brace_delta(line: &str) -> isize {
    line.chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

fn exported_fn_name(signature: &str) -> &str {
    signature
        .split_once("fn ")
        .and_then(|(_, rest)| {
            rest.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .next()
        })
        .unwrap_or("<unknown>")
}

#[test]
fn status_returning_exports_use_panic_guard() {
    let source = include_str!("../src/lib.rs");
    let mut lines = source.lines().enumerate().peekable();
    let mut missing = Vec::new();
    let mut checked = 0usize;

    while let Some((line_index, line)) = lines.next() {
        if line != "#[unsafe(no_mangle)]" {
            continue;
        }

        let mut signature = String::new();
        for (_, next_line) in lines.by_ref() {
            if next_line.starts_with("#[") {
                continue;
            }
            signature.push_str(next_line);
            signature.push('\n');
            if next_line.contains('{') {
                break;
            }
        }

        let returns_status = signature.contains("TermyFfiStatus");
        let mut body = signature.clone();
        let mut depth = brace_delta(&signature);
        while depth > 0 {
            let Some((_, next_line)) = lines.next() else {
                break;
            };
            body.push_str(next_line);
            body.push('\n');
            depth += brace_delta(next_line);
        }

        if returns_status {
            checked += 1;
            if !body.contains("ffi_status_guard") {
                missing.push(format!(
                    "{}:{}",
                    line_index + 1,
                    exported_fn_name(&signature)
                ));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "status-returning FFI exports must use ffi_status_guard: {missing:?}"
    );
    assert!(checked > 0, "expected at least one status-returning export");
}
