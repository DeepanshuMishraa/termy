use std::collections::BTreeSet;

struct ExportedFn {
    line_number: usize,
    name: String,
    signature: String,
    body: String,
}

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

fn exported_fns(source: &str) -> Vec<ExportedFn> {
    let mut lines = source.lines().enumerate().peekable();
    let mut exports = Vec::new();

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

        exports.push(ExportedFn {
            line_number: line_index + 1,
            name: exported_fn_name(&signature).to_string(),
            signature,
            body,
        });
    }

    exports
}

fn header_function_names(header: &str) -> BTreeSet<String> {
    let bytes = header.as_bytes();
    let mut names = BTreeSet::new();
    let mut cursor = 0usize;

    while let Some(offset) = header[cursor..].find("termy_") {
        let start = cursor + offset;
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }

        let mut next = end;
        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
            next += 1;
        }
        if bytes.get(next) == Some(&b'(') {
            names.insert(header[start..end].to_string());
        }

        cursor = end;
    }

    names
}

#[test]
fn status_returning_exports_use_panic_guard() {
    let exports = exported_fns(include_str!("../src/lib.rs"));
    let mut missing = Vec::new();
    let mut checked = 0usize;

    for export in exports {
        if export.signature.contains("TermyFfiStatus") {
            checked += 1;
            if !export.body.contains("ffi_status_guard") {
                missing.push(format!("{}:{}", export.line_number, export.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "status-returning FFI exports must use ffi_status_guard: {missing:?}"
    );
    assert!(checked > 0, "expected at least one status-returning export");
}

#[test]
fn c_header_declares_all_rust_exports() {
    let rust_exports = exported_fns(include_str!("../src/lib.rs"))
        .into_iter()
        .map(|export| export.name)
        .collect::<BTreeSet<_>>();
    let header_exports = header_function_names(include_str!("../include/termy.h"));

    let missing_from_header = rust_exports
        .difference(&header_exports)
        .cloned()
        .collect::<Vec<_>>();
    let missing_from_rust = header_exports
        .difference(&rust_exports)
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        missing_from_header.is_empty(),
        "Rust exports missing from termy.h: {missing_from_header:?}"
    );
    assert!(
        missing_from_rust.is_empty(),
        "termy.h declarations missing Rust exports: {missing_from_rust:?}"
    );
    assert!(!rust_exports.is_empty(), "expected exported Rust functions");
}
