// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persistent on-disk cache for the parsed system-font index.
//!
//! [`fontdb::Database::load_system_fonts`] walks every system font directory and
//! `mmap`s + parses each font file to populate per-face metadata. The result lives only
//! in memory and is rebuilt from scratch on every process start.
//!
//! This module persists the small, parsed [`fontdb::FaceInfo`] set to disk and reloads it
//! via [`fontdb::Database::push_face_info`] instead of re-parsing every font file. It is a
//! pure-Rust, dependency-free reimplementation of what fontconfig's `fc-cache` does.
//!
//! The cache is keyed on a cheap fingerprint of the font directories and files that were
//! scanned (per-directory and per-file modification time, plus file size). On any mismatch
//! the normal scan runs and the cache is rewritten atomically.

use crate::HashMap;
use alloc::string::String;
use alloc::vec::Vec;
use fontdb::{Database, FaceInfo, Source, Stretch, Style, Weight, ID};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Magic bytes identifying a cosmic-text font cache file (`C`osmic `T`ext `F`ont `C`ache).
const MAGIC: [u8; 4] = *b"CTFC";

/// Cache format version. Bump whenever the on-disk layout changes so that stale caches
/// written by an older version are rejected instead of misparsed.
const VERSION: u32 = 1;

/// Loads the system fonts into `db`, using the on-disk cache at `cache_path` when it is
/// present and still valid.
///
/// On a cache hit no font file is parsed: the stored [`FaceInfo`] entries are inserted
/// directly with [`Database::push_face_info`]. On a miss (or an invalidated cache) the
/// normal [`Database::load_system_fonts`] scan runs and its result is written back to the
/// cache for the next start.
pub(crate) fn load_system_fonts_cached(db: &mut Database, cache_path: &Path) {
    match try_load(cache_path) {
        Some(faces) => {
            log::debug!(
                "Loaded {} font faces from cache '{}'.",
                faces.len(),
                cache_path.display()
            );
            for face in faces {
                db.push_face_info(face);
            }
        }
        None => {
            db.load_system_fonts();
            if let Err(err) = write(db, cache_path) {
                log::warn!(
                    "Failed to write font cache '{}': {err}",
                    cache_path.display()
                );
            }
        }
    }
}

/// Returns the default cache file path (`<cache-dir>/cosmic-text/fonts.cache`), following
/// the platform's conventional cache directory. Returns `None` if no cache directory can
/// be determined from the environment.
pub(crate) fn default_cache_path() -> Option<PathBuf> {
    let mut dir = cache_dir()?;
    dir.push("cosmic-text");
    dir.push("fonts.cache");
    Some(dir)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn cache_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        let path = PathBuf::from(xdg);
        if path.is_absolute() {
            return Some(path);
        }
    }
    let mut home = PathBuf::from(std::env::var_os("HOME")?);
    home.push(".cache");
    Some(home)
}

#[cfg(target_os = "macos")]
fn cache_dir() -> Option<PathBuf> {
    let mut home = PathBuf::from(std::env::var_os("HOME")?);
    home.push("Library");
    home.push("Caches");
    Some(home)
}

#[cfg(windows)]
fn cache_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(not(any(unix, windows)))]
fn cache_dir() -> Option<PathBuf> {
    None
}

//TODO: Redox?!

/// Reads `cache_path` and, if it is well-formed and its fingerprint still matches the
/// filesystem, returns the cached set of faces. Returns `None` on any error, version
/// mismatch, or fingerprint mismatch.
fn try_load(cache_path: &Path) -> Option<Vec<FaceInfo>> {
    let bytes = std::fs::read(cache_path).ok()?;
    let mut r = Reader::new(&bytes);

    if r.take(4)? != MAGIC {
        return None;
    }
    if r.u32()? != VERSION {
        return None;
    }

    // Directory fingerprint
    let dir_count = r.u32()?;
    for _ in 0..dir_count {
        let path = r.path()?;
        let mtime = r.mtime()?;
        if dir_mtime(&path) != Some(mtime) {
            return None;
        }
    }

    // File fingerprint
    let file_count = r.u32()? as usize;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let path = r.path()?;
        let mtime = r.mtime()?;
        let len = r.u64()?;
        match file_fingerprint(&path) {
            Some((m, l)) if m == mtime && l == len => {}
            _ => return None,
        }
        files.push(path);
    }

    // Face table. Each face references a file by index into `files`.
    let face_count = r.u32()? as usize;
    let mut faces = Vec::with_capacity(face_count);
    for _ in 0..face_count {
        let file_index = r.u32()? as usize;
        let source_path = files.get(file_index)?.clone();
        let index = r.u32()?;

        let family_count = r.u32()? as usize;
        let mut families = Vec::with_capacity(family_count);
        for _ in 0..family_count {
            // TODO: Language is not persisted, but it is also not used
            families.push((r.string()?, fontdb::Language::Unknown));
        }

        let post_script_name = r.string()?;
        let style = decode_style(r.u8()?)?;
        let weight = Weight(r.u16()?);
        let stretch = decode_stretch(r.u8()?)?;
        let monospaced = r.u8()? != 0;

        faces.push(FaceInfo {
            id: ID::dummy(),
            source: Source::File(source_path),
            index,
            families,
            post_script_name,
            style,
            weight,
            stretch,
            monospaced,
        });
    }

    Some(faces)
}

/// Serializes the file-backed faces in `db` to `cache_path`, writing atomically
fn write(db: &Database, cache_path: &Path) -> std::io::Result<()> {
    let mut file_indices: HashMap<PathBuf, u32> = HashMap::default();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut faces: Vec<(u32, &FaceInfo)> = Vec::new();

    for face in db.faces() {
        // cosmic-text's `std` feature always enables `fontdb/memmap`
        let path = match &face.source {
            Source::File(path) => path,
            Source::SharedFile(path, _) => path,
            // Binary sources (e.g. user-provided fonts) are not persistable.
            Source::Binary(_) => continue,
        };
        let index = *file_indices.entry(path.clone()).or_insert_with(|| {
            let i = files.len() as u32;
            files.push(path.clone());
            i
        });
        faces.push((index, face));
    }

    // Directories that directly contained a scanned font, deduplicated.
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut seen_dirs: HashSet<PathBuf> = HashSet::default();
    for path in &files {
        if let Some(parent) = path.parent() {
            if seen_dirs.insert(parent.to_path_buf()) {
                dirs.push(parent.to_path_buf());
            }
        }
    }

    let mut w = Writer::new();
    w.bytes(&MAGIC);
    w.u32(VERSION);

    w.u32(dirs.len() as u32);
    for dir in &dirs {
        w.path(dir);
        w.mtime(dir_mtime(dir).unwrap_or((0, 0)));
    }

    w.u32(files.len() as u32);
    for path in &files {
        w.path(path);
        let (mtime, len) = file_fingerprint(path).unwrap_or(((0, 0), 0));
        w.mtime(mtime);
        w.u64(len);
    }

    w.u32(faces.len() as u32);
    for (file_index, face) in &faces {
        w.u32(*file_index);
        w.u32(face.index);
        w.u32(face.families.len() as u32);
        for (name, _language) in &face.families {
            w.string(name);
        }
        w.string(&face.post_script_name);
        w.u8(encode_style(face.style));
        w.u16(face.weight.0);
        w.u8(face.stretch.to_number() as u8);
        w.u8(u8::from(face.monospaced));
    }

    write_atomic(cache_path, &w.into_inner())
}

/// Writes `data` to `path` atomically
fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut tmp = path.as_os_str().to_os_string();
    // A per-process suffix keeps concurrent writers from writing to eachother's temp file
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);

    std::fs::write(&tmp, data)?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            Err(err)
        }
    }
}

/// Returns the modification time of a directory as `(secs, nanos)` since the Unix epoch.
fn dir_mtime(path: &Path) -> Option<(u64, u32)> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_dir() {
        return None;
    }
    mtime_parts(metadata.modified().ok()?)
}

/// Returns `((secs, nanos), len)` for a regular file, or `None` if it is missing or not a file.
fn file_fingerprint(path: &Path) -> Option<((u64, u32), u64)> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    Some((mtime_parts(metadata.modified().ok()?)?, metadata.len()))
}

fn mtime_parts(time: SystemTime) -> Option<(u64, u32)> {
    let dur = time.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some((dur.as_secs(), dur.subsec_nanos()))
}

fn encode_style(style: Style) -> u8 {
    match style {
        Style::Normal => 0,
        Style::Italic => 1,
        Style::Oblique => 2,
    }
}

fn decode_style(value: u8) -> Option<Style> {
    match value {
        0 => Some(Style::Normal),
        1 => Some(Style::Italic),
        2 => Some(Style::Oblique),
        _ => None,
    }
}

fn decode_stretch(value: u8) -> Option<Stretch> {
    Some(match value {
        1 => Stretch::UltraCondensed,
        2 => Stretch::ExtraCondensed,
        3 => Stretch::Condensed,
        4 => Stretch::SemiCondensed,
        5 => Stretch::Normal,
        6 => Stretch::SemiExpanded,
        7 => Stretch::Expanded,
        8 => Stretch::ExtraExpanded,
        9 => Stretch::UltraExpanded,
        _ => return None,
    })
}

/// Append-only little-endian byte writer.
struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    fn bytes(&mut self, value: &[u8]) {
        self.buf.extend_from_slice(value);
    }

    fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    fn mtime(&mut self, (secs, nanos): (u64, u32)) {
        self.u64(secs);
        self.u32(nanos);
    }

    fn string(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes(value.as_bytes());
    }

    /// Encodes a path as its lossy UTF-8 string form
    fn path(&mut self, path: &Path) {
        self.string(&path.to_string_lossy());
    }
}

/// Bounds-checked little-endian byte reader. Every accessor returns `None` on truncation
/// or malformed data so a corrupt cache is rejected rather than panicking.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(len)?;
        let slice = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    fn mtime(&mut self) -> Option<(u64, u32)> {
        Some((self.u64()?, self.u32()?))
    }

    fn string(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    fn path(&mut self) -> Option<PathBuf> {
        Some(PathBuf::from(self.string()?))
    }
}
