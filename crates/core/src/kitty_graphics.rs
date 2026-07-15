//! Renderer-neutral support for the Kitty terminal graphics protocol.
//!
//! The core owns parsing, upload assembly, storage, placement, deletion and
//! protocol replies. Hosts only turn the returned PNG bytes into textures.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use flate2::read::ZlibDecoder;
use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::TerminalSize;

const MAX_IMAGE_BYTES: usize = 128 * 1024 * 1024;
const MAX_COMMAND_BYTES: usize = MAX_IMAGE_BYTES * 2;
const MAX_DIMENSION: u32 = 32_768;
const MAX_PIXELS: u64 = (MAX_IMAGE_BYTES / 4) as u64;

#[derive(Clone, Debug)]
pub struct KittyGraphicsCommand {
    control: Vec<(char, String)>,
    payload: Vec<u8>,
    oversized: bool,
}

impl KittyGraphicsCommand {
    fn parse(bytes: Vec<u8>, oversized: bool) -> Self {
        let separator = bytes.iter().position(|byte| *byte == b';');
        let (control, payload) = match separator {
            Some(index) => (&bytes[..index], bytes[index + 1..].to_vec()),
            None => (bytes.as_slice(), Vec::new()),
        };
        let control = String::from_utf8_lossy(control)
            .split(',')
            .filter_map(|field| {
                let (key, value) = field.split_once('=')?;
                let mut chars = key.chars();
                let key = chars.next()?;
                (chars.next().is_none() && key.is_ascii()).then(|| (key, value.to_string()))
            })
            .collect();
        Self {
            control,
            payload,
            oversized,
        }
    }

    fn value(&self, key: char) -> Option<&str> {
        self.control
            .iter()
            .rev()
            .find_map(|(candidate, value)| (*candidate == key).then_some(value.as_str()))
    }

    fn char_value(&self, key: char) -> Option<char> {
        let mut chars = self.value(key)?.chars();
        let value = chars.next()?;
        chars.next().is_none().then_some(value)
    }

    fn u32_value(&self, key: char) -> Option<u32> {
        self.value(key)?.parse().ok()
    }

    fn i32_value(&self, key: char) -> Option<i32> {
        self.value(key)?.parse().ok()
    }
}

#[derive(Clone, Debug)]
pub enum KittyGraphicsItem {
    Text(Vec<u8>),
    Command(KittyGraphicsCommand),
}

#[derive(Clone, Copy, Debug, Default)]
enum InterceptorState {
    #[default]
    Ground,
    Escape,
    ApcStart {
        c1: bool,
    },
    OtherApc,
    OtherApcEscape,
    Kitty,
    KittyEscape,
}

#[derive(Default)]
pub struct KittyGraphicsInterceptor {
    state: InterceptorState,
    command: Vec<u8>,
    oversized: bool,
}

impl KittyGraphicsInterceptor {
    pub fn process(&mut self, bytes: &[u8]) -> Vec<KittyGraphicsItem> {
        let mut items = Vec::new();
        let mut text = Vec::with_capacity(bytes.len());

        let flush_text = |items: &mut Vec<KittyGraphicsItem>, text: &mut Vec<u8>| {
            if !text.is_empty() {
                items.push(KittyGraphicsItem::Text(std::mem::take(text)));
            }
        };

        for &byte in bytes {
            match self.state {
                InterceptorState::Ground => match byte {
                    0x1b => self.state = InterceptorState::Escape,
                    0x9f => self.state = InterceptorState::ApcStart { c1: true },
                    _ => text.push(byte),
                },
                InterceptorState::Escape => {
                    if byte == b'_' {
                        self.state = InterceptorState::ApcStart { c1: false };
                    } else if byte == 0x1b {
                        text.push(0x1b);
                        self.state = InterceptorState::Escape;
                    } else {
                        text.push(0x1b);
                        text.push(byte);
                        self.state = InterceptorState::Ground;
                    }
                }
                InterceptorState::ApcStart { c1 } => {
                    if byte == b'G' {
                        flush_text(&mut items, &mut text);
                        self.command.clear();
                        self.oversized = false;
                        self.state = InterceptorState::Kitty;
                    } else {
                        if c1 {
                            text.push(0x9f);
                        } else {
                            text.extend_from_slice(b"\x1b_");
                        }
                        text.push(byte);
                        self.state = InterceptorState::OtherApc;
                    }
                }
                InterceptorState::OtherApc => {
                    text.push(byte);
                    if byte == 0x9c {
                        self.state = InterceptorState::Ground;
                    } else if byte == 0x1b {
                        self.state = InterceptorState::OtherApcEscape;
                    }
                }
                InterceptorState::OtherApcEscape => {
                    text.push(byte);
                    if byte == b'\\' || byte == 0x9c {
                        self.state = InterceptorState::Ground;
                    } else if byte != 0x1b {
                        self.state = InterceptorState::OtherApc;
                    }
                }
                InterceptorState::Kitty => {
                    if byte == 0x1b {
                        self.state = InterceptorState::KittyEscape;
                    } else if byte == 0x9c {
                        flush_text(&mut items, &mut text);
                        items.push(KittyGraphicsItem::Command(KittyGraphicsCommand::parse(
                            std::mem::take(&mut self.command),
                            self.oversized,
                        )));
                        self.state = InterceptorState::Ground;
                    } else if self.command.len() < MAX_COMMAND_BYTES {
                        self.command.push(byte);
                    } else {
                        self.oversized = true;
                    }
                }
                InterceptorState::KittyEscape => {
                    if byte == b'\\' {
                        flush_text(&mut items, &mut text);
                        items.push(KittyGraphicsItem::Command(KittyGraphicsCommand::parse(
                            std::mem::take(&mut self.command),
                            self.oversized,
                        )));
                        self.state = InterceptorState::Ground;
                    } else {
                        if self.command.len() < MAX_COMMAND_BYTES {
                            self.command.push(0x1b);
                            self.command.push(byte);
                        } else {
                            self.oversized = true;
                        }
                        self.state = InterceptorState::Kitty;
                    }
                }
            }
        }
        flush_text(&mut items, &mut text);
        items
    }
}

#[derive(Clone, Debug)]
struct StoredImage {
    png: Arc<[u8]>,
    width: u32,
    height: u32,
    byte_len: usize,
    generation: u64,
}

#[derive(Clone, Debug)]
struct Placement {
    image_id: u32,
    placement_id: u32,
    anchor_line: i64,
    col: usize,
    source_x: u32,
    source_y: u32,
    source_width: u32,
    source_height: u32,
    display_cols: Option<u32>,
    display_rows: Option<u32>,
    occupied_cols: u32,
    occupied_rows: u32,
    x_offset: u32,
    y_offset: u32,
    z_index: i32,
}

#[derive(Clone, Debug)]
struct PendingUpload {
    command: KittyGraphicsCommand,
    decoded: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct KittyGraphicsRenderPlacement {
    pub image_id: u32,
    pub placement_id: u32,
    pub png: Arc<[u8]>,
    pub image_width: u32,
    pub image_height: u32,
    pub image_generation: u64,
    pub viewport_row: i32,
    pub col: usize,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub display_cols: Option<u32>,
    pub display_rows: Option<u32>,
    pub x_offset: u32,
    pub y_offset: u32,
    pub z_index: i32,
}

#[derive(Default)]
pub struct KittyGraphicsApplyResult {
    pub response: Option<Vec<u8>>,
    pub cursor_advance: Option<(u32, u32)>,
    pub changed: bool,
}

#[derive(Default)]
pub struct KittyGraphicsState {
    images: HashMap<u32, StoredImage>,
    placements: Vec<Placement>,
    insertion_order: VecDeque<u32>,
    pending: Option<PendingUpload>,
    next_anonymous_id: u32,
    stored_bytes: usize,
    next_generation: u64,
}

impl KittyGraphicsState {
    pub fn apply(
        &mut self,
        command: KittyGraphicsCommand,
        cursor_col: usize,
        cursor_row: usize,
        history_size: usize,
        size: TerminalSize,
    ) -> KittyGraphicsApplyResult {
        if command.oversized {
            let response_command = self
                .pending
                .take()
                .map_or_else(|| command.clone(), |pending| pending.command);
            return self.failure(
                &response_command,
                "EFBIG:image command exceeds storage limit",
            );
        }

        let action = command.char_value('a').unwrap_or('t');
        if action == 'd' {
            return self.delete(&command, cursor_col, cursor_row, history_size);
        }
        if action == 'p' {
            return self.put(&command, cursor_col, cursor_row, history_size, size);
        }
        if !matches!(action, 't' | 'T' | 'q') {
            return self.failure(&command, "EINVAL:unsupported graphics action");
        }

        let decoded = match BASE64.decode(&command.payload) {
            Ok(decoded) if decoded.len() <= MAX_IMAGE_BYTES => decoded,
            Ok(_) => {
                let response_command = self
                    .pending
                    .take()
                    .map_or_else(|| command.clone(), |pending| pending.command);
                return self.failure(
                    &response_command,
                    "EFBIG:image payload exceeds storage limit",
                );
            }
            Err(_) => {
                let response_command = self
                    .pending
                    .take()
                    .map_or_else(|| command.clone(), |pending| pending.command);
                return self.failure(&response_command, "EINVAL:invalid base64 payload");
            }
        };

        let more = command.u32_value('m').unwrap_or(0) == 1;
        if let Some(mut pending) = self.pending.take() {
            if pending.decoded.len().saturating_add(decoded.len()) > MAX_IMAGE_BYTES {
                return self.failure(
                    &pending.command,
                    "EFBIG:image payload exceeds storage limit",
                );
            }
            pending.decoded.extend_from_slice(&decoded);
            if more {
                self.pending = Some(pending);
                return KittyGraphicsApplyResult::default();
            }
            let mut first = pending.command;
            for (key, value) in command.control {
                if key != 'm' {
                    first.control.push((key, value));
                }
            }
            return self.finish_upload(
                first,
                pending.decoded,
                cursor_col,
                cursor_row,
                history_size,
                size,
            );
        }

        if more {
            self.pending = Some(PendingUpload { command, decoded });
            return KittyGraphicsApplyResult::default();
        }
        self.finish_upload(command, decoded, cursor_col, cursor_row, history_size, size)
    }

    pub fn render_placements(
        &self,
        history_size: usize,
        display_offset: usize,
        rows: usize,
        cols: usize,
    ) -> Vec<KittyGraphicsRenderPlacement> {
        let history_size = i64::try_from(history_size).unwrap_or(i64::MAX);
        let display_offset = i64::try_from(display_offset).unwrap_or(i64::MAX);
        let rows_i64 = i64::try_from(rows).unwrap_or(i64::MAX);
        self.placements
            .iter()
            .filter_map(|placement| {
                let image = self.images.get(&placement.image_id)?;
                let viewport_row = placement
                    .anchor_line
                    .saturating_sub(history_size)
                    .saturating_add(display_offset);
                let bottom = viewport_row.saturating_add(i64::from(placement.occupied_rows));
                if bottom <= 0
                    || viewport_row >= rows_i64
                    || placement.col >= cols
                    || placement.occupied_cols == 0
                {
                    return None;
                }
                Some(KittyGraphicsRenderPlacement {
                    image_id: placement.image_id,
                    placement_id: placement.placement_id,
                    png: image.png.clone(),
                    image_width: image.width,
                    image_height: image.height,
                    image_generation: image.generation,
                    viewport_row: i32::try_from(viewport_row).unwrap_or(if viewport_row < 0 {
                        i32::MIN
                    } else {
                        i32::MAX
                    }),
                    col: placement.col,
                    source_x: placement.source_x,
                    source_y: placement.source_y,
                    source_width: placement.source_width,
                    source_height: placement.source_height,
                    display_cols: placement.display_cols,
                    display_rows: placement.display_rows,
                    x_offset: placement.x_offset,
                    y_offset: placement.y_offset,
                    z_index: placement.z_index,
                })
            })
            .collect()
    }

    pub fn clear_visible(&mut self) -> bool {
        let changed = !self.placements.is_empty();
        self.placements.clear();
        self.pending = None;
        changed
    }

    fn finish_upload(
        &mut self,
        command: KittyGraphicsCommand,
        decoded: Vec<u8>,
        cursor_col: usize,
        cursor_row: usize,
        history_size: usize,
        size: TerminalSize,
    ) -> KittyGraphicsApplyResult {
        let data = match self.resolve_transmission_data(&command, decoded) {
            Ok(data) => data,
            Err(error) => return self.failure(&command, &error),
        };
        let data = match command.char_value('o') {
            None => data,
            Some('z') => match decompress_zlib(&data) {
                Ok(data) => data,
                Err(error) => return self.failure(&command, &error),
            },
            Some(_) => return self.failure(&command, "EINVAL:unsupported compression"),
        };
        let (png, width, height) = match normalize_image(&command, &data) {
            Ok(image) => image,
            Err(error) => return self.failure(&command, &error),
        };

        if command.char_value('a').unwrap_or('t') == 'q' {
            return self.success(&command, false, None);
        }

        let requested_id = command.u32_value('i').unwrap_or(0);
        let image_id = if requested_id == 0 {
            self.allocate_anonymous_id()
        } else {
            requested_id
        };
        self.remove_image(image_id);
        let byte_len = png.len();
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.images.insert(
            image_id,
            StoredImage {
                png: Arc::from(png),
                width,
                height,
                byte_len,
                generation: self.next_generation,
            },
        );
        self.insertion_order.push_back(image_id);
        self.stored_bytes = self.stored_bytes.saturating_add(byte_len);
        if !self.enforce_quota(Some(image_id)) {
            self.remove_image(image_id);
            return self.failure(&command, "ENOSPC:image storage quota exceeded");
        }

        let display = command.char_value('a').unwrap_or('t') == 'T';
        let cursor_advance = if display {
            match self.add_placement(
                image_id,
                &command,
                cursor_col,
                cursor_row,
                history_size,
                size,
            ) {
                Ok(advance) => advance,
                Err(error) => {
                    self.remove_image(image_id);
                    return self.failure(&command, &error);
                }
            }
        } else {
            None
        };
        self.success(&command, true, cursor_advance)
    }

    fn put(
        &mut self,
        command: &KittyGraphicsCommand,
        cursor_col: usize,
        cursor_row: usize,
        history_size: usize,
        size: TerminalSize,
    ) -> KittyGraphicsApplyResult {
        let image_id = command.u32_value('i').unwrap_or(0);
        if image_id == 0 || !self.images.contains_key(&image_id) {
            return self.failure(command, "ENOENT:image id not found");
        }
        match self.add_placement(
            image_id,
            command,
            cursor_col,
            cursor_row,
            history_size,
            size,
        ) {
            Ok(advance) => self.success(command, true, advance),
            Err(error) => self.failure(command, &error),
        }
    }

    fn add_placement(
        &mut self,
        image_id: u32,
        command: &KittyGraphicsCommand,
        cursor_col: usize,
        cursor_row: usize,
        history_size: usize,
        size: TerminalSize,
    ) -> Result<Option<(u32, u32)>, String> {
        if command.u32_value('U').unwrap_or(0) == 1 {
            return Err("EINVAL:Unicode placeholder placements are not supported".into());
        }
        let image = self
            .images
            .get(&image_id)
            .ok_or_else(|| "ENOENT:image id not found".to_string())?;
        let source_x = command.u32_value('x').unwrap_or(0).min(image.width);
        let source_y = command.u32_value('y').unwrap_or(0).min(image.height);
        let source_width = command
            .u32_value('w')
            .unwrap_or(image.width.saturating_sub(source_x))
            .min(image.width.saturating_sub(source_x));
        let source_height = command
            .u32_value('h')
            .unwrap_or(image.height.saturating_sub(source_y))
            .min(image.height.saturating_sub(source_y));
        if source_width == 0 || source_height == 0 {
            return Err("EINVAL:empty source rectangle".into());
        }

        let cell_width = size.cell_width.max(1.0);
        let cell_height = size.cell_height.max(1.0);
        let requested_cols = command.u32_value('c').filter(|value| *value > 0);
        let requested_rows = command.u32_value('r').filter(|value| *value > 0);
        let (display_cols, display_rows) = match (requested_cols, requested_rows) {
            (Some(cols), Some(rows)) => (Some(cols), Some(rows)),
            (Some(cols), None) => {
                let width_px = cols as f32 * cell_width;
                let height_px = width_px * source_height as f32 / source_width as f32;
                (
                    Some(cols),
                    Some((height_px / cell_height).ceil().max(1.0) as u32),
                )
            }
            (None, Some(rows)) => {
                let height_px = rows as f32 * cell_height;
                let width_px = height_px * source_width as f32 / source_height as f32;
                (
                    Some((width_px / cell_width).ceil().max(1.0) as u32),
                    Some(rows),
                )
            }
            (None, None) => (None, None),
        };
        let occupied_cols = display_cols
            .unwrap_or_else(|| (source_width as f32 / cell_width).ceil().max(1.0) as u32);
        let occupied_rows = display_rows
            .unwrap_or_else(|| (source_height as f32 / cell_height).ceil().max(1.0) as u32);
        let placement_id = command.u32_value('p').unwrap_or(0);
        if placement_id != 0 {
            self.placements.retain(|placement| {
                placement.image_id != image_id || placement.placement_id != placement_id
            });
        }
        self.placements.push(Placement {
            image_id,
            placement_id,
            anchor_line: i64::try_from(history_size)
                .unwrap_or(i64::MAX)
                .saturating_add(i64::try_from(cursor_row).unwrap_or(i64::MAX)),
            col: cursor_col,
            source_x,
            source_y,
            source_width,
            source_height,
            display_cols,
            display_rows,
            occupied_cols,
            occupied_rows,
            x_offset: command.u32_value('X').unwrap_or(0),
            y_offset: command.u32_value('Y').unwrap_or(0),
            z_index: command.i32_value('z').unwrap_or(0),
        });
        Ok((command.u32_value('C').unwrap_or(0) == 0).then_some((occupied_cols, occupied_rows)))
    }

    fn delete(
        &mut self,
        command: &KittyGraphicsCommand,
        cursor_col: usize,
        cursor_row: usize,
        history_size: usize,
    ) -> KittyGraphicsApplyResult {
        self.pending = None;
        let selector = command.char_value('d').unwrap_or('a');
        let free_data = selector.is_ascii_uppercase();
        let selector = selector.to_ascii_lowercase();
        let before = self.placements.len();
        match selector {
            'a' => self.placements.clear(),
            'i' => {
                let image_id = command.u32_value('i').unwrap_or(0);
                let placement_id = command.u32_value('p').unwrap_or(0);
                self.placements.retain(|placement| {
                    placement.image_id != image_id
                        || (placement_id != 0 && placement.placement_id != placement_id)
                });
                if free_data && !self.placements.iter().any(|p| p.image_id == image_id) {
                    self.remove_image(image_id);
                }
            }
            'c' => {
                let line = i64::try_from(history_size)
                    .unwrap_or(i64::MAX)
                    .saturating_add(i64::try_from(cursor_row).unwrap_or(i64::MAX));
                self.placements
                    .retain(|p| !placement_contains(p, line, cursor_col));
            }
            'p' | 'q' => {
                let col = command.u32_value('x').unwrap_or(1).saturating_sub(1) as usize;
                let row = command.u32_value('y').unwrap_or(1).saturating_sub(1) as i64
                    + i64::try_from(history_size).unwrap_or(i64::MAX);
                let z = command.i32_value('z');
                self.placements.retain(|p| {
                    !placement_contains(p, row, col)
                        || (selector == 'q' && z.is_some_and(|z| p.z_index != z))
                });
            }
            'x' => {
                let col = command.u32_value('x').unwrap_or(1).saturating_sub(1) as usize;
                self.placements.retain(|p| {
                    col < p.col || col >= p.col.saturating_add(p.occupied_cols as usize)
                });
            }
            'y' => {
                let row = command.u32_value('y').unwrap_or(1).saturating_sub(1) as i64
                    + i64::try_from(history_size).unwrap_or(i64::MAX);
                self.placements.retain(|p| {
                    row < p.anchor_line || row >= p.anchor_line + i64::from(p.occupied_rows)
                });
            }
            'z' => {
                let z = command.i32_value('z').unwrap_or(0);
                self.placements.retain(|p| p.z_index != z);
            }
            _ => return self.failure(command, "EINVAL:unsupported delete selector"),
        }
        if free_data {
            self.drop_unplaced_images();
        }
        self.success(command, before != self.placements.len(), None)
    }

    fn resolve_transmission_data(
        &self,
        command: &KittyGraphicsCommand,
        decoded: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        match command.char_value('t').unwrap_or('d') {
            'd' => Ok(decoded),
            'f' | 't' => {
                let temporary = command.char_value('t') == Some('t');
                let path = PathBuf::from(
                    std::str::from_utf8(&decoded).map_err(|_| "EINVAL:file path is not UTF-8")?,
                );
                let result = read_regular_file(
                    &path,
                    command.u32_value('O').unwrap_or(0) as u64,
                    command
                        .u32_value('S')
                        .filter(|size| *size > 0)
                        .map(u64::from),
                );
                if temporary && temporary_path_can_be_removed(&path) {
                    let _ = std::fs::remove_file(&path);
                }
                result
            }
            's' => Err("ENOTSUP:shared-memory transmission is not supported".into()),
            _ => Err("EINVAL:unsupported transmission medium".into()),
        }
    }

    fn allocate_anonymous_id(&mut self) -> u32 {
        if self.next_anonymous_id == 0 {
            self.next_anonymous_id = u32::MAX;
        }
        while self.images.contains_key(&self.next_anonymous_id) {
            self.next_anonymous_id = self.next_anonymous_id.saturating_sub(1).max(1);
        }
        let id = self.next_anonymous_id;
        self.next_anonymous_id = self.next_anonymous_id.saturating_sub(1).max(1);
        id
    }

    fn enforce_quota(&mut self, protected: Option<u32>) -> bool {
        while self.stored_bytes > MAX_IMAGE_BYTES {
            let Some(candidate) = self.insertion_order.pop_front() else {
                break;
            };
            if protected == Some(candidate)
                || self
                    .placements
                    .iter()
                    .any(|placement| placement.image_id == candidate)
            {
                self.insertion_order.push_back(candidate);
                if self.insertion_order.iter().all(|id| {
                    protected == Some(*id)
                        || self
                            .placements
                            .iter()
                            .any(|placement| placement.image_id == *id)
                }) {
                    break;
                }
                continue;
            }
            self.remove_image(candidate);
        }
        self.stored_bytes <= MAX_IMAGE_BYTES
    }

    fn drop_unplaced_images(&mut self) {
        let ids: Vec<u32> = self
            .images
            .keys()
            .copied()
            .filter(|id| {
                !self
                    .placements
                    .iter()
                    .any(|placement| placement.image_id == *id)
            })
            .collect();
        for id in ids {
            self.remove_image(id);
        }
    }

    fn remove_image(&mut self, image_id: u32) {
        if let Some(image) = self.images.remove(&image_id) {
            self.stored_bytes = self.stored_bytes.saturating_sub(image.byte_len);
        }
        self.placements
            .retain(|placement| placement.image_id != image_id);
        self.insertion_order.retain(|id| *id != image_id);
    }

    fn success(
        &self,
        command: &KittyGraphicsCommand,
        changed: bool,
        cursor_advance: Option<(u32, u32)>,
    ) -> KittyGraphicsApplyResult {
        KittyGraphicsApplyResult {
            response: response(command, true, "OK"),
            cursor_advance,
            changed,
        }
    }

    fn failure(&self, command: &KittyGraphicsCommand, message: &str) -> KittyGraphicsApplyResult {
        KittyGraphicsApplyResult {
            response: response(command, false, message),
            ..KittyGraphicsApplyResult::default()
        }
    }
}

fn placement_contains(placement: &Placement, line: i64, col: usize) -> bool {
    line >= placement.anchor_line
        && line
            < placement
                .anchor_line
                .saturating_add(i64::from(placement.occupied_rows))
        && col >= placement.col
        && col
            < placement
                .col
                .saturating_add(placement.occupied_cols as usize)
}

fn response(command: &KittyGraphicsCommand, success: bool, message: &str) -> Option<Vec<u8>> {
    let quiet = command.u32_value('q').unwrap_or(0);
    if (success && quiet >= 1) || (!success && quiet >= 2) {
        return None;
    }
    let image_id = command.u32_value('i').or_else(|| command.u32_value('I'))?;
    let mut control = format!("i={image_id}");
    if let Some(placement_id) = command.u32_value('p') {
        control.push_str(&format!(",p={placement_id}"));
    }
    Some(format!("\x1b_G{control};{message}\x1b\\").into_bytes())
}

fn normalize_image(
    command: &KittyGraphicsCommand,
    data: &[u8],
) -> Result<(Vec<u8>, u32, u32), String> {
    match command.u32_value('f').unwrap_or(32) {
        100 => {
            let decoder = png::Decoder::new(Cursor::new(data));
            let reader = decoder
                .read_info()
                .map_err(|_| "EINVAL:invalid PNG image".to_string())?;
            let info = reader.info();
            validate_dimensions(info.width, info.height, 4)?;
            Ok((data.to_vec(), info.width, info.height))
        }
        format @ (24 | 32) => {
            let width = command.u32_value('s').unwrap_or(0);
            let height = command.u32_value('v').unwrap_or(0);
            let channels = if format == 24 { 3 } else { 4 };
            let expected = validate_dimensions(width, height, channels)?;
            if data.len() != expected {
                return Err("EINVAL:pixel data length does not match dimensions".into());
            }
            let mut png = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut png, width, height);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.set_color(if channels == 3 {
                    png::ColorType::Rgb
                } else {
                    png::ColorType::Rgba
                });
                encoder
                    .write_header()
                    .and_then(|mut writer| writer.write_image_data(data))
                    .map_err(|_| "EINVAL:failed to encode pixel data".to_string())?;
            }
            Ok((png, width, height))
        }
        _ => Err("EINVAL:unsupported image format".into()),
    }
}

fn validate_dimensions(width: u32, height: u32, channels: usize) -> Result<usize, String> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err("EINVAL:invalid image dimensions".into());
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_PIXELS {
        return Err("EFBIG:image dimensions exceed storage limit".into());
    }
    usize::try_from(pixels)
        .ok()
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| "EFBIG:image dimensions overflow".into())
}

fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    ZlibDecoder::new(data)
        .take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut output)
        .map_err(|_| "EINVAL:invalid zlib payload".to_string())?;
    if output.len() > MAX_IMAGE_BYTES {
        return Err("EFBIG:decompressed image exceeds storage limit".into());
    }
    Ok(output)
}

fn read_regular_file(path: &Path, offset: u64, size: Option<u64>) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|_| "ENOENT:unable to open image file".to_string())?;
    let metadata = file
        .metadata()
        .map_err(|_| "EIO:unable to inspect image file".to_string())?;
    if !metadata.file_type().is_file() {
        return Err("EINVAL:image path is not a regular file".into());
    }
    if offset > metadata.len() {
        return Err("EINVAL:file offset is past end of file".into());
    }
    let available = metadata.len() - offset;
    let length = size.unwrap_or(available).min(available);
    if length > MAX_IMAGE_BYTES as u64 {
        return Err("EFBIG:image file exceeds storage limit".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| "EIO:unable to seek image file".to_string())?;
    let mut data = Vec::with_capacity(length as usize);
    file.take(length)
        .read_to_end(&mut data)
        .map_err(|_| "EIO:unable to read image file".to_string())?;
    Ok(data)
}

fn temporary_path_can_be_removed(path: &Path) -> bool {
    let path_text = path.to_string_lossy();
    if !path_text.contains("tty-graphics-protocol") {
        return false;
    }
    let Ok(canonical) = path.canonicalize() else {
        return false;
    };
    let mut roots = vec![PathBuf::from("/tmp"), PathBuf::from("/private/tmp")];
    roots.push(std::env::temp_dir());
    roots.into_iter().any(|root| {
        root.canonicalize()
            .is_ok_and(|root| canonical.starts_with(root))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn size() -> TerminalSize {
        TerminalSize {
            cols: 80,
            rows: 24,
            cell_width: 10.0,
            cell_height: 20.0,
        }
    }

    fn command(control: &str, payload: &[u8]) -> KittyGraphicsCommand {
        KittyGraphicsCommand::parse(
            [control.as_bytes(), b";", BASE64.encode(payload).as_bytes()].concat(),
            false,
        )
    }

    fn one_pixel_png() -> Vec<u8> {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&[255, 0, 0, 255])
                .unwrap();
        }
        png
    }

    #[test]
    fn interceptor_removes_kitty_apc_and_preserves_text() {
        let png = one_pixel_png();
        let sequence = format!(
            "before\x1b_Ga=T,f=100,i=7;{}\x1b\\after",
            BASE64.encode(png)
        );
        let mut interceptor = KittyGraphicsInterceptor::default();
        let items = interceptor.process(sequence.as_bytes());
        assert!(matches!(&items[0], KittyGraphicsItem::Text(text) if text == b"before"));
        assert!(matches!(&items[1], KittyGraphicsItem::Command(_)));
        assert!(matches!(&items[2], KittyGraphicsItem::Text(text) if text == b"after"));
    }

    #[test]
    fn interceptor_handles_sequence_split_across_reads() {
        let mut interceptor = KittyGraphicsInterceptor::default();
        assert!(
            matches!(interceptor.process(b"x\x1b_").as_slice(), [KittyGraphicsItem::Text(text)] if text == b"x")
        );
        assert!(
            interceptor
                .process(b"Ga=T,f=32,s=1,v=1,i=9;//AA")
                .is_empty()
        );
        let items = interceptor.process(b"/w==\x1b\\y");
        assert!(matches!(&items[0], KittyGraphicsItem::Command(_)));
        assert!(matches!(&items[1], KittyGraphicsItem::Text(text) if text == b"y"));
    }

    #[test]
    fn interceptor_preserves_non_kitty_apc_sequences() {
        let mut interceptor = KittyGraphicsInterceptor::default();
        let input = b"a\x1b_not-kitty\x1b\\b";
        let items = interceptor.process(input);
        assert!(matches!(items.as_slice(), [KittyGraphicsItem::Text(text)] if text == input));
    }

    #[test]
    fn uploads_raw_rgba_and_places_at_cursor() {
        let mut state = KittyGraphicsState::default();
        let result = state.apply(
            command("a=T,f=32,s=1,v=1,i=42,c=2,r=3", &[1, 2, 3, 255]),
            4,
            5,
            10,
            size(),
        );
        assert!(result.changed);
        assert_eq!(result.cursor_advance, Some((2, 3)));
        assert_eq!(result.response.unwrap(), b"\x1b_Gi=42;OK\x1b\\");
        let placements = state.render_placements(10, 0, 24, 80);
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].viewport_row, 5);
        assert_eq!(placements[0].col, 4);
        assert!(placements[0].png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn assembles_chunked_upload_before_displaying() {
        let png = one_pixel_png();
        let split = png.len() / 2;
        let first = command("a=T,f=100,i=8,m=1", &png[..split]);
        let second = command("m=0", &png[split..]);
        let mut state = KittyGraphicsState::default();
        assert!(!state.apply(first, 0, 0, 0, size()).changed);
        assert!(state.render_placements(0, 0, 24, 80).is_empty());
        assert!(state.apply(second, 0, 0, 0, size()).changed);
        assert_eq!(state.render_placements(0, 0, 24, 80).len(), 1);
    }

    #[test]
    fn delete_by_image_and_placement_id_is_precise() {
        let png = one_pixel_png();
        let mut state = KittyGraphicsState::default();
        state.apply(command("a=t,f=100,i=3,q=1", &png), 0, 0, 0, size());
        state.apply(command("a=p,i=3,p=1,q=1", &[]), 0, 0, 0, size());
        state.apply(command("a=p,i=3,p=2,q=1", &[]), 2, 0, 0, size());
        state.apply(command("a=d,d=i,i=3,p=1,q=1", &[]), 0, 0, 0, size());
        let placements = state.render_placements(0, 0, 24, 80);
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].placement_id, 2);
    }

    #[test]
    fn quiet_mode_suppresses_success_but_not_errors_at_level_one() {
        let mut state = KittyGraphicsState::default();
        let success = state.apply(
            command("a=T,f=32,s=1,v=1,i=1,q=1", &[0, 0, 0, 0]),
            0,
            0,
            0,
            size(),
        );
        assert!(success.response.is_none());
        let failure = state.apply(command("a=p,i=999,q=1", &[]), 0, 0, 0, size());
        assert!(failure.response.is_some());
    }

    #[test]
    fn accepts_zlib_compressed_raw_pixels() {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&[12, 34, 56, 255]).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut state = KittyGraphicsState::default();

        let result = state.apply(
            command("a=T,f=32,s=1,v=1,o=z,i=12,q=1", &compressed),
            0,
            0,
            0,
            size(),
        );

        assert!(result.changed);
        assert_eq!(state.render_placements(0, 0, 24, 80).len(), 1);
    }

    #[test]
    fn reads_and_removes_safe_temporary_file_transfers() {
        let png = one_pixel_png();
        let mut file = tempfile::Builder::new()
            .prefix("tty-graphics-protocol-")
            .tempfile()
            .unwrap();
        file.write_all(&png).unwrap();
        file.flush().unwrap();
        let path = file.path().to_path_buf();
        let mut state = KittyGraphicsState::default();

        let result = state.apply(
            command("a=T,f=100,t=t,i=13,q=1", path.to_string_lossy().as_bytes()),
            0,
            0,
            0,
            size(),
        );

        assert!(result.changed);
        assert!(!path.exists());
        assert_eq!(state.render_placements(0, 0, 24, 80).len(), 1);
    }
}
