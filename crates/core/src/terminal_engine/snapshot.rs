//! Full-state snapshots of an [`Engine`].
//!
//! A snapshot restores everything that decides how the engine responds to
//! later output: both screens and the scrollback (cells, combining text,
//! styles, OSC 8 links, underline colors, wrap flags), cursors and saved
//! cursors, margins, tab stops, charsets, every mode, the kitty keyboard
//! stacks, palette overrides, the title and its stack, the OSC 7 working
//! directory, program status records, the parser's in-progress sequence and
//! a pending synchronized update. Feeding the same bytes to an engine and to
//! one decoded from its snapshot leaves both in the same state.
//!
//! Not included: queued events and protocol replies (drain them first),
//! render damage (a decoded engine starts with full damage), kitty graphics
//! images and placements, and memory caches that do not affect behavior.
//!
//! # Format
//!
//! All integers are little-endian; variable-length integers are unsigned
//! LEB128. The layout is:
//!
//! ```text
//! magic    8 bytes  "TRMYSNAP"
//! version  u32      SNAPSHOT_VERSION
//! flags    u32      reserved, zero
//! tables   u64      byte offset of the tables section
//! body              parser, synchronized update, terminal state, grid
//! tables            hyperlinks, then cell metadata, referenced by index
//! ```
//!
//! Decoders reject other magic values and versions they do not support with
//! a [`SnapshotError`] instead of guessing.

use std::{collections::HashMap, sync::Arc};

use super::{CellExtra, Color, Cursor, CursorShape, Engine, Hyperlink, Style, UnderlineStyle};

const MAGIC: &[u8; 8] = b"TRMYSNAP";
/// The snapshot format this build writes and reads.
pub const SNAPSHOT_VERSION: u32 = 1;
const HEADER_BYTES: usize = 8 + 4 + 4 + 8;

/// Why a snapshot could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// The bytes do not start with the snapshot magic.
    NotASnapshot,
    /// The snapshot was written by an incompatible format version.
    UnsupportedVersion(u32),
    /// The snapshot ended early.
    Truncated,
    /// A field holds a value no encoder writes.
    Corrupt(&'static str),
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotASnapshot => formatter.write_str("not a termy snapshot"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "unsupported snapshot version {version} (this build reads {SNAPSHOT_VERSION})"
            ),
            Self::Truncated => formatter.write_str("snapshot is truncated"),
            Self::Corrupt(what) => write!(formatter, "corrupt snapshot: {what}"),
        }
    }
}

impl std::error::Error for SnapshotError {}

pub(super) type Result<T> = std::result::Result<T, SnapshotError>;

/// Read the version of a snapshot without decoding it.
pub fn snapshot_version(bytes: &[u8]) -> Result<u32> {
    if bytes.len() >= MAGIC.len() && &bytes[..MAGIC.len()] != MAGIC {
        return Err(SnapshotError::NotASnapshot);
    }
    let header = bytes.get(..HEADER_BYTES).ok_or(SnapshotError::Truncated)?;
    Ok(u32::from_le_bytes([
        header[8], header[9], header[10], header[11],
    ]))
}

pub(super) struct Writer {
    out: Vec<u8>,
    /// Canonical output omits storage details (packing, metadata charges)
    /// and shares metadata by value, so equal states encode equally. Tests
    /// compare engines with it.
    canonical: bool,
    links: Vec<Arc<Hyperlink>>,
    link_indices: HashMap<(String, String), u32>,
    extras: Vec<Arc<CellExtra>>,
    extra_by_pointer: HashMap<usize, u32>,
    extra_by_value: HashMap<(String, u32, [u8; 4]), u32>,
}

impl Writer {
    fn new(canonical: bool) -> Self {
        Self {
            out: Vec::new(),
            canonical,
            links: Vec::new(),
            link_indices: HashMap::new(),
            extras: Vec::new(),
            extra_by_pointer: HashMap::new(),
            extra_by_value: HashMap::new(),
        }
    }

    pub(super) fn canonical(&self) -> bool {
        self.canonical
    }

    pub(super) fn reserve(&mut self, bytes: usize) {
        self.out.reserve(bytes);
    }

    #[inline]
    pub(super) fn u8(&mut self, value: u8) {
        self.out.push(value);
    }

    #[inline]
    pub(super) fn bool(&mut self, value: bool) {
        self.out.push(u8::from(value));
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.out.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.out.extend_from_slice(&value.to_le_bytes());
    }

    #[inline]
    pub(super) fn varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.out.push(value as u8 | 0x80);
            value >>= 7;
        }
        self.out.push(value as u8);
    }

    #[inline]
    pub(super) fn usize(&mut self, value: usize) {
        self.varint(value as u64);
    }

    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        self.usize(bytes.len());
        self.out.extend_from_slice(bytes);
    }

    pub(super) fn str(&mut self, text: &str) {
        self.bytes(text.as_bytes());
    }

    pub(super) fn option_bool(&mut self, value: Option<bool>) {
        self.u8(match value {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
    }

    pub(super) fn color(&mut self, color: Color) {
        self.out.extend_from_slice(&color.to_bytes());
    }

    pub(super) fn option_color(&mut self, color: Option<Color>) {
        self.bool(color.is_some());
        if let Some(color) = color {
            self.color(color);
        }
    }

    #[inline]
    pub(super) fn style(&mut self, style: Style) {
        self.color(style.foreground);
        self.color(style.background);
        self.u8(style.attributes);
        self.u8(style.underline as u8);
    }

    pub(super) fn cursor_shape(&mut self, shape: CursorShape) {
        self.u8(match shape {
            CursorShape::Block => 0,
            CursorShape::Beam => 1,
            CursorShape::Underline => 2,
        });
    }

    pub(super) fn cursor(&mut self, cursor: Cursor) {
        self.usize(cursor.row);
        self.usize(cursor.col);
        self.bool(cursor.visible);
        self.cursor_shape(cursor.shape);
        self.bool(cursor.blinking);
    }

    pub(super) fn link(&mut self, link: &Arc<Hyperlink>) -> u32 {
        let key = (link.id.clone(), link.uri.clone());
        if let Some(&index) = self.link_indices.get(&key) {
            return index;
        }
        let index = self.links.len() as u32;
        self.links.push(Arc::clone(link));
        self.link_indices.insert(key, index);
        index
    }

    /// The table index of `extra`. Shared metadata stays shared on decode.
    pub(super) fn extra(&mut self, extra: &Arc<CellExtra>) -> u32 {
        if !self.canonical
            && let Some(&index) = self.extra_by_pointer.get(&Arc::as_ptr(extra).addr())
        {
            return index;
        }
        let link = extra
            .hyperlink
            .as_ref()
            .map_or(0, |link| self.link(link) + 1);
        let index = if self.canonical {
            let key = (
                extra.combining.clone(),
                link,
                extra.underline_color.to_bytes(),
            );
            if let Some(&index) = self.extra_by_value.get(&key) {
                return index;
            }
            let index = self.extras.len() as u32;
            self.extra_by_value.insert(key, index);
            index
        } else {
            let index = self.extras.len() as u32;
            self.extra_by_pointer
                .insert(Arc::as_ptr(extra).addr(), index);
            index
        };
        self.extras.push(Arc::clone(extra));
        index
    }

    pub(super) fn option_extra(&mut self, extra: Option<&Arc<CellExtra>>) {
        let index = extra.map_or(0, |extra| u64::from(self.extra(extra)) + 1);
        self.varint(index);
    }

    fn finish(mut self) -> Vec<u8> {
        let tables = self.out.len() as u64;
        self.out[16..24].copy_from_slice(&tables.to_le_bytes());
        let links = std::mem::take(&mut self.links);
        self.usize(links.len());
        for link in &links {
            self.str(&link.id);
            self.str(&link.uri);
        }
        let extras = std::mem::take(&mut self.extras);
        self.usize(extras.len());
        for extra in &extras {
            self.str(&extra.combining);
            let link = extra.hyperlink.as_ref().map_or(0, |link| {
                u64::from(self.link_indices[&(link.id.clone(), link.uri.clone())]) + 1
            });
            self.varint(link);
            self.color(extra.underline_color);
        }
        self.out
    }
}

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    links: Vec<Arc<Hyperlink>>,
    extras: Vec<Arc<CellExtra>>,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            links: Vec::new(),
            extras: Vec::new(),
        }
    }

    pub(super) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(SnapshotError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(SnapshotError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    #[inline]
    pub(super) fn u8(&mut self) -> Result<u8> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or(SnapshotError::Truncated)?;
        self.offset += 1;
        Ok(byte)
    }

    pub(super) fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SnapshotError::Corrupt("boolean")),
        }
    }

    pub(super) fn u16(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(super) fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(super) fn u64(&mut self) -> Result<u64> {
        let bytes = self.take(8)?;
        let mut word = [0; 8];
        word.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(word))
    }

    #[inline]
    pub(super) fn varint(&mut self) -> Result<u64> {
        let mut value = 0_u64;
        for shift in (0..64).step_by(7) {
            let byte = self.u8()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(SnapshotError::Corrupt("integer"))
    }

    #[inline]
    pub(super) fn usize(&mut self) -> Result<usize> {
        usize::try_from(self.varint()?).map_err(|_| SnapshotError::Corrupt("integer"))
    }

    /// A count of items each at least `item_bytes` long, checked against
    /// the remaining input before anything is allocated for them.
    pub(super) fn count(&mut self, item_bytes: usize) -> Result<usize> {
        let count = self.usize()?;
        if count.saturating_mul(item_bytes.max(1)) > self.bytes.len() - self.offset {
            return Err(SnapshotError::Truncated);
        }
        Ok(count)
    }

    pub(super) fn u16_value(&mut self) -> Result<u16> {
        u16::try_from(self.varint()?).map_err(|_| SnapshotError::Corrupt("integer"))
    }

    pub(super) fn bytes(&mut self) -> Result<&'a [u8]> {
        let len = self.usize()?;
        self.take(len)
    }

    pub(super) fn string(&mut self) -> Result<String> {
        let bytes = self.bytes()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| SnapshotError::Corrupt("string"))
    }

    pub(super) fn option_bool(&mut self) -> Result<Option<bool>> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(false)),
            2 => Ok(Some(true)),
            _ => Err(SnapshotError::Corrupt("optional boolean")),
        }
    }

    #[inline]
    pub(super) fn color(&mut self) -> Result<Color> {
        let bytes = self.take(4)?;
        Color::from_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            .ok_or(SnapshotError::Corrupt("color"))
    }

    pub(super) fn option_color(&mut self) -> Result<Option<Color>> {
        Ok(if self.bool()? {
            Some(self.color()?)
        } else {
            None
        })
    }

    #[inline]
    pub(super) fn style(&mut self) -> Result<Style> {
        Ok(Style {
            foreground: self.color()?,
            background: self.color()?,
            attributes: self.u8()?,
            underline: UnderlineStyle::from_u8(self.u8()?)
                .ok_or(SnapshotError::Corrupt("underline style"))?,
        })
    }

    pub(super) fn cursor_shape(&mut self) -> Result<CursorShape> {
        match self.u8()? {
            0 => Ok(CursorShape::Block),
            1 => Ok(CursorShape::Beam),
            2 => Ok(CursorShape::Underline),
            _ => Err(SnapshotError::Corrupt("cursor shape")),
        }
    }

    pub(super) fn cursor(&mut self) -> Result<Cursor> {
        Ok(Cursor {
            row: self.usize()?,
            col: self.usize()?,
            visible: self.bool()?,
            shape: self.cursor_shape()?,
            blinking: self.bool()?,
        })
    }

    pub(super) fn link(&mut self) -> Result<Arc<Hyperlink>> {
        let index = self.usize()?;
        self.links
            .get(index)
            .cloned()
            .ok_or(SnapshotError::Corrupt("hyperlink index"))
    }

    #[inline]
    pub(super) fn extra(&mut self) -> Result<Arc<CellExtra>> {
        let index = self.usize()?;
        self.extras
            .get(index)
            .cloned()
            .ok_or(SnapshotError::Corrupt("cell metadata index"))
    }

    pub(super) fn option_extra(&mut self) -> Result<Option<Arc<CellExtra>>> {
        let index = self.usize()?;
        if index == 0 {
            return Ok(None);
        }
        self.extras
            .get(index - 1)
            .cloned()
            .map(Some)
            .ok_or(SnapshotError::Corrupt("cell metadata index"))
    }

    fn read_tables(&mut self) -> Result<()> {
        let links = self.count(2)?;
        self.links.reserve_exact(links);
        for _ in 0..links {
            let id = self.string()?;
            let uri = self.string()?;
            self.links.push(Arc::new(Hyperlink { id, uri }));
        }
        let extras = self.count(6)?;
        self.extras.reserve_exact(extras);
        for _ in 0..extras {
            let combining = self.string()?;
            if combining.len() > super::types::MAX_COMBINING_BYTES {
                return Err(SnapshotError::Corrupt("combining text"));
            }
            let link = self.usize()?;
            let hyperlink = if link == 0 {
                None
            } else {
                Some(
                    self.links
                        .get(link - 1)
                        .cloned()
                        .ok_or(SnapshotError::Corrupt("hyperlink index"))?,
                )
            };
            let underline_color = self.color()?;
            self.extras.push(Arc::new(CellExtra {
                combining,
                hyperlink,
                underline_color,
            }));
        }
        Ok(())
    }
}

impl Engine {
    /// Encode the engine's full state; see the module documentation.
    /// Queued events and replies are not part of it: drain them first.
    pub fn encode_snapshot(&mut self) -> Vec<u8> {
        self.encode_snapshot_with(false)
    }

    fn encode_snapshot_with(&mut self, canonical: bool) -> Vec<u8> {
        // Nothing outside parsing may see a partly blanked row, and expanded
        // history read caches are not state.
        self.state.grid.flush_pending_clear();
        self.state.grid.release_history_read_cache();
        let mut writer = Writer::new(canonical);
        writer.reserve(HEADER_BYTES + self.state.grid.snapshot_size_hint());
        writer.out.extend_from_slice(MAGIC);
        writer
            .out
            .extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
        writer.out.extend_from_slice(&0_u32.to_le_bytes());
        writer.u64(0);
        self.parser.write_snapshot(&mut writer);
        self.synchronized_update.write_snapshot(&mut writer);
        self.state.write_snapshot(&mut writer);
        writer.finish()
    }

    /// Rebuild an engine from [`Self::encode_snapshot`] output.
    pub fn decode_snapshot(bytes: &[u8]) -> Result<Self> {
        let version = snapshot_version(bytes)?;
        if version != SNAPSHOT_VERSION {
            return Err(SnapshotError::UnsupportedVersion(version));
        }
        let mut reader = Reader::new(bytes);
        reader.offset = 12;
        if reader.u32()? != 0 {
            return Err(SnapshotError::Corrupt("header flags"));
        }
        let tables =
            usize::try_from(reader.u64()?).map_err(|_| SnapshotError::Corrupt("table offset"))?;
        if tables < HEADER_BYTES || tables > bytes.len() {
            return Err(SnapshotError::Corrupt("table offset"));
        }
        let mut table_reader = Reader::new(bytes);
        table_reader.offset = tables;
        table_reader.read_tables()?;
        if table_reader.offset != bytes.len() {
            return Err(SnapshotError::Corrupt("trailing bytes"));
        }
        reader.bytes = &bytes[..tables];
        reader.links = table_reader.links;
        reader.extras = table_reader.extras;

        let parser = super::parser::Parser::read_snapshot(&mut reader)?;
        let synchronized_update = super::sync::SynchronizedUpdate::read_snapshot(&mut reader)?;
        let state = super::dispatch::State::read_snapshot(&mut reader)?;
        if reader.offset != reader.bytes.len() {
            return Err(SnapshotError::Corrupt("trailing bytes"));
        }
        Ok(Self {
            parser,
            synchronized_update,
            state,
            generation: 1,
        })
    }

    /// Snapshot bytes that are equal exactly when two engines' states are,
    /// independent of storage layout and metadata sharing.
    #[cfg(test)]
    pub(crate) fn canonical_snapshot(&mut self) -> Vec<u8> {
        self.encode_snapshot_with(true)
    }
}

#[cfg(test)]
#[path = "snapshot/tests.rs"]
mod tests;
