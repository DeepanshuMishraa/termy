use std::collections::{BTreeMap, BTreeSet};

struct ExportedFn {
    line_number: usize,
    name: String,
    signature: String,
    body: String,
}

#[derive(Debug, Eq, PartialEq)]
struct AbiSignature {
    return_type: String,
    args: Vec<String>,
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

fn canonical_rust_type(raw_type: &str) -> String {
    let raw_type = raw_type.trim().trim_end_matches(',').trim();
    if let Some(rest) = raw_type.strip_prefix("*const ") {
        return format!("*const {}", canonical_rust_type(rest));
    }
    if let Some(rest) = raw_type.strip_prefix("*mut ") {
        return format!("*mut {}", canonical_rust_type(rest));
    }

    match raw_type {
        "bool" => "bool".to_string(),
        "i32" => "i32".to_string(),
        "u8" => "u8".to_string(),
        "u16" => "u16".to_string(),
        "u32" => "u32".to_string(),
        "u64" => "u64".to_string(),
        "usize" => "usize".to_string(),
        "tmux_control_core::session::ControlSession" => "TermyFfiTmuxControl".to_string(),
        other => other.to_string(),
    }
}

fn rust_export_signature(export: &ExportedFn) -> AbiSignature {
    let signature = export.signature.replace('\n', " ");
    let open_paren = signature
        .find('(')
        .unwrap_or_else(|| panic!("{} has no argument list", export.name));
    let close_paren = signature
        .rfind(')')
        .unwrap_or_else(|| panic!("{} has no argument list terminator", export.name));
    let args = signature[open_paren + 1..close_paren]
        .split(',')
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(|arg| {
            let (_, ty) = arg
                .split_once(": ")
                .unwrap_or_else(|| panic!("{} has an unparsable arg: {arg}", export.name));
            canonical_rust_type(ty)
        })
        .collect();

    let return_tail = signature[close_paren + 1..]
        .split_once('{')
        .map_or("", |(tail, _)| tail)
        .trim();
    let return_type = return_tail
        .strip_prefix("->")
        .map_or_else(|| "void".to_string(), canonical_rust_type);

    AbiSignature { return_type, args }
}

fn canonical_c_base_type(raw_type: &str) -> &str {
    match raw_type {
        "bool" => "bool",
        "float" => "f32",
        "int32_t" => "i32",
        "uint8_t" => "u8",
        "uint16_t" => "u16",
        "uint32_t" => "u32",
        "uint64_t" => "u64",
        "size_t" => "usize",
        other => other,
    }
}

fn trim_c_param_name(param: &str) -> &str {
    let trimmed = param.trim();
    let Some(name_start) = trimmed
        .char_indices()
        .rev()
        .find_map(|(index, ch)| (!ch.is_ascii_alphanumeric() && ch != '_').then_some(index + 1))
    else {
        return trimmed;
    };
    trimmed[..name_start].trim()
}

fn canonical_c_type(raw_type: &str) -> String {
    let pointer_count = raw_type.chars().filter(|ch| *ch == '*').count();
    let no_pointer_type = raw_type.replace('*', " ");
    let mut parts = no_pointer_type.split_whitespace().collect::<Vec<_>>();
    let is_const = parts.first() == Some(&"const");
    parts.retain(|part| *part != "const");

    let base_type = canonical_c_base_type(&parts.join(" ")).to_string();
    (0..pointer_count).fold(base_type, |ty, depth| {
        let pointer_kind = if depth == 0 && is_const {
            "const"
        } else {
            "mut"
        };
        format!("*{pointer_kind} {ty}")
    })
}

fn header_function_signatures(header: &str) -> BTreeMap<String, AbiSignature> {
    let mut signatures = BTreeMap::new();
    let mut declaration = String::new();

    for line in header.lines().map(str::trim) {
        if declaration.is_empty() && !line.contains("termy_") {
            continue;
        }

        declaration.push_str(line);
        declaration.push(' ');
        if line.ends_with(';') {
            if let Some((name, signature)) = parse_header_declaration(&declaration) {
                signatures.insert(name, signature);
            }
            declaration.clear();
        }
    }

    signatures
}

fn parse_header_declaration(declaration: &str) -> Option<(String, AbiSignature)> {
    let declaration = declaration.trim().trim_end_matches(';').trim();
    let open_paren = declaration.find('(')?;
    let close_paren = declaration.rfind(')')?;
    let before_args = declaration[..open_paren].trim();
    let name_start = before_args.rfind(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')? + 1;
    let name = before_args[name_start..].to_string();
    let return_type = canonical_c_type(before_args[..name_start].trim());
    let args_text = declaration[open_paren + 1..close_paren].trim();
    let args = if args_text.is_empty() || args_text == "void" {
        Vec::new()
    } else {
        args_text
            .split(',')
            .map(trim_c_param_name)
            .map(canonical_c_type)
            .collect()
    };

    Some((name, AbiSignature { return_type, args }))
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

#[test]
fn c_header_signatures_match_rust_exports() {
    let rust_signatures = exported_fns(include_str!("../src/lib.rs"))
        .into_iter()
        .map(|export| (export.name.clone(), rust_export_signature(&export)))
        .collect::<BTreeMap<_, _>>();
    let header_signatures = header_function_signatures(include_str!("../include/termy.h"));

    let mismatches = rust_signatures
        .iter()
        .filter_map(|(name, rust_signature)| {
            header_signatures.get(name).and_then(|header_signature| {
                (header_signature != rust_signature).then(|| {
                    format!("{name}: rust={rust_signature:?}, header={header_signature:?}")
                })
            })
        })
        .collect::<Vec<_>>();

    assert!(
        mismatches.is_empty(),
        "termy.h signatures drifted from Rust exports: {mismatches:#?}"
    );
    assert!(
        !rust_signatures.is_empty(),
        "expected exported Rust functions"
    );
}
