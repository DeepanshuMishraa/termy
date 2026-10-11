//! Bounded interning for OSC 8 hyperlinks.
//!
//! Cells share one `Arc<Hyperlink>` per `(id, uri)`, and the table caps the
//! bytes of distinct links that cells may reference. Without it, every OSC 8
//! allocated a fresh URI, so scrollback's cell bound did not bound memory.
use super::types::Hyperlink;
use std::{collections::HashMap, sync::Arc};

/// Longest URI accepted; longer links print as plain text (kitty uses 2 KiB).
const MAX_URI_BYTES: usize = 8 * 1024;
const MAX_ID_BYTES: usize = 256;
/// Bytes of distinct link text retained across the screen and scrollback.
const MAX_TABLE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TABLE_LINKS: usize = 1 << 16;
/// Overflowing requests rejected without rescanning after a full rescan, so
/// a table full of referenced links costs amortized O(1) per OSC 8.
const RESCAN_INTERVAL: u32 = 64;

#[derive(Default)]
pub(super) struct Hyperlinks {
    links: HashMap<(String, String), Arc<Hyperlink>>,
    bytes: usize,
    skip_rescans: u32,
}

impl Hyperlinks {
    /// The shared link for `(id, uri)`, or `None` when it is oversized or the
    /// table is full of links that cells still reference.
    pub(super) fn intern(&mut self, id: &str, uri: &str) -> Option<Arc<Hyperlink>> {
        if uri.len() > MAX_URI_BYTES || id.len() > MAX_ID_BYTES {
            return None;
        }
        let key = (id.to_owned(), uri.to_owned());
        if let Some(link) = self.links.get(&key) {
            return Some(link.clone());
        }
        // The key and the shared link each hold a copy of the text.
        let size = 2 * (id.len() + uri.len());
        let full = |links: &Self| {
            links.bytes + size > MAX_TABLE_BYTES || links.links.len() >= MAX_TABLE_LINKS
        };
        if full(self) {
            if self.skip_rescans > 0 {
                self.skip_rescans -= 1;
                return None;
            }
            // Links held only by this table no longer appear in any cell.
            self.links.retain(|_, link| Arc::strong_count(link) > 1);
            self.bytes = self
                .links
                .keys()
                .map(|(id, uri)| 2 * (id.len() + uri.len()))
                .sum();
            if full(self) {
                self.skip_rescans = RESCAN_INTERVAL;
                return None;
            }
        }
        let link = Arc::new(Hyperlink {
            id: key.0.clone(),
            uri: key.1.clone(),
        });
        self.bytes += size;
        self.links.insert(key, link.clone());
        Some(link)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_links_share_one_allocation() {
        let mut links = Hyperlinks::default();
        let first = links.intern("a", "https://example.test").unwrap();
        let second = links.intern("a", "https://example.test").unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(
            &first,
            &links.intern("b", "https://example.test").unwrap()
        ));
        assert!(links.intern("", &"x".repeat(MAX_URI_BYTES + 1)).is_none());
    }

    #[test]
    fn referenced_links_are_bounded_and_unreferenced_ones_are_reclaimed() {
        let mut links = Hyperlinks::default();
        let uri = "u".repeat(MAX_URI_BYTES);
        let held: Vec<_> = (0..)
            .map_while(|index| links.intern(&index.to_string(), &uri))
            .collect();
        assert!(2 * held.len() * MAX_URI_BYTES <= MAX_TABLE_BYTES);
        assert!(links.intern("next", &uri).is_none());
        drop(held);
        // Released links are reclaimed by the next rescan.
        let rejected = (0..=RESCAN_INTERVAL)
            .take_while(|_| links.intern("next", &uri).is_none())
            .count();
        assert!(rejected <= RESCAN_INTERVAL as usize);
        assert!(links.bytes <= MAX_TABLE_BYTES);
    }
}
