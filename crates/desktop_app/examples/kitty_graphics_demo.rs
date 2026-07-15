//! End-to-end Kitty graphics protocol smoke test for Termy.
//!
//! Run this inside Termy:
//!
//! ```text
//! cargo run -p termy --example kitty_graphics_demo
//! ```
//!
//! Pass a different PNG as the first argument, or override the cell size with
//! `--cols` and `--rows`.

use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
};

const DEFAULT_IMAGE: &[u8] = include_bytes!("assets/kitty-demo.png");
const IMAGE_ID: u32 = 424_242;
const PLACEMENT_ID: u32 = 1;
const PAYLOAD_CHUNK_SIZE: usize = 4096;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug)]
struct Options {
    image: Option<PathBuf>,
    cols: u32,
    rows: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = parse_options()? else {
        return Ok(());
    };
    let image = match options.image {
        Some(path) => fs::read(path)?,
        None => DEFAULT_IMAGE.to_vec(),
    };

    let mut stdout = io::stdout().lock();
    write_kitty_image(&mut stdout, &image, options.cols, options.rows)?;
    writeln!(
        stdout,
        "\rKitty graphics demo rendered {} bytes at {}x{} cells.",
        image.len(),
        options.cols,
        options.rows
    )?;
    stdout.flush()?;
    Ok(())
}

fn parse_options() -> Result<Option<Options>, String> {
    let mut image = None;
    let mut cols = 40;
    let mut rows = 20;
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!(
                    "Usage: kitty_graphics_demo [PNG] [--cols N] [--rows N]\n\
                     \nDisplays the bundled image, or PNG when supplied, using a chunked Kitty graphics transfer."
                );
                return Ok(None);
            }
            "--cols" => cols = parse_dimension("--cols", args.next())?,
            "--rows" => rows = parse_dimension("--rows", args.next())?,
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
            _ if image.is_none() => image = Some(PathBuf::from(arg)),
            _ => return Err("only one PNG path may be supplied".into()),
        }
    }

    Ok(Some(Options { image, cols, rows }))
}

fn parse_dimension(name: &str, value: Option<String>) -> Result<u32, String> {
    let value = value.ok_or_else(|| format!("{name} requires a value"))?;
    let dimension = value
        .parse::<u32>()
        .map_err(|_| format!("invalid {name} value: {value}"))?;
    if dimension == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(dimension)
}

fn write_kitty_image(output: &mut impl Write, png: &[u8], cols: u32, rows: u32) -> io::Result<()> {
    if !png.starts_with(PNG_SIGNATURE) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input is not a PNG image",
        ));
    }

    // Remove a previous run's image and placement before reusing their IDs.
    write_command(output, &format!("a=d,d=i,i={IMAGE_ID},q=2"), &[])?;

    let encoded = encode_base64(png);
    let mut chunks = encoded.chunks(PAYLOAD_CHUNK_SIZE).peekable();
    let mut first = true;
    while let Some(payload) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        let control = if first {
            first = false;
            format!(
                "a=T,f=100,t=d,i={IMAGE_ID},p={PLACEMENT_ID},c={cols},r={rows},C=0,q=2,m={more}"
            )
        } else {
            format!("m={more}")
        };
        write_command(output, &control, payload)?;
    }

    output.flush()
}

fn write_command(output: &mut impl Write, control: &str, payload: &[u8]) -> io::Result<()> {
    output.write_all(b"\x1b_G")?;
    output.write_all(control.as_bytes())?;
    output.write_all(b";")?;
    output.write_all(payload)?;
    output.write_all(b"\x1b\\")
}

fn encode_base64(input: &[u8]) -> Vec<u8> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(ALPHABET[((bits >> 18) & 0x3f) as usize]);
        output.push(ALPHABET[((bits >> 12) & 0x3f) as usize]);
        output.push(if chunk.len() > 1 {
            ALPHABET[((bits >> 6) & 0x3f) as usize]
        } else {
            b'='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(bits & 0x3f) as usize]
        } else {
            b'='
        });
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(encode_base64(b""), b"");
        assert_eq!(encode_base64(b"f"), b"Zg==");
        assert_eq!(encode_base64(b"fo"), b"Zm8=");
        assert_eq!(encode_base64(b"foo"), b"Zm9v");
        assert_eq!(encode_base64(b"foobar"), b"Zm9vYmFy");
    }

    #[test]
    fn emits_delete_then_chunked_png_transfer() {
        let mut png = PNG_SIGNATURE.to_vec();
        png.resize(PAYLOAD_CHUNK_SIZE, 0x5a);
        let mut output = Vec::new();

        write_kitty_image(&mut output, &png, 32, 16).unwrap();

        assert!(output.starts_with(b"\x1b_Ga=d,d=i,i=424242,q=2;\x1b\\"));
        assert!(
            output
                .windows(b"\x1b_Gm=0;".len())
                .any(|window| window == b"\x1b_Gm=0;")
        );
        assert!(output.ends_with(b"\x1b\\"));
        assert!(
            output
                .windows(b"a=T,f=100,t=d,i=424242,p=1,c=32,r=16,C=0,q=2,m=1".len())
                .any(|window| window == b"a=T,f=100,t=d,i=424242,p=1,c=32,r=16,C=0,q=2,m=1")
        );
    }

    #[test]
    fn rejects_non_png_input() {
        let error = write_kitty_image(&mut Vec::new(), b"not an image", 40, 20).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
