use anyhow::{Context, Result};

/// What the user had copied, to be put back after a paste. On Windows that is every format
/// on the clipboard, so a picture, a file or a range of cells survives a dictation.
/// Elsewhere the richest form arboard can read back.
pub enum Contents {
    #[cfg(not(windows))]
    Files(Vec<std::path::PathBuf>),
    #[cfg(not(windows))]
    Html { html: String, text: Option<String> },
    #[cfg(not(windows))]
    Text(String),
    #[cfg(not(windows))]
    Image(arboard::ImageData<'static>),
    #[cfg(windows)]
    Formats(Vec<(u32, Vec<u8>)>),
}

#[cfg(windows)]
pub fn take(_: &mut arboard::Clipboard) -> Option<Contents> {
    win::take().map(Contents::Formats)
}

#[cfg(not(windows))]
pub fn take(clipboard: &mut arboard::Clipboard) -> Option<Contents> {
    // A file manager offers the names of copied files as text too, and a browser or an
    // office app offers formatted text next to the plain one and a picture of the cells:
    // the richer form first, but text before a picture.
    if let Some(files) = clipboard.get().file_list().ok().filter(|f| !f.is_empty()) {
        return Some(Contents::Files(files));
    }
    if let Ok(html) = clipboard.get().html() {
        let text = clipboard.get_text().ok();
        return Some(Contents::Html { html, text });
    }
    clipboard.get_text().ok().map(Contents::Text).or_else(|| {
        clipboard
            .get_image()
            .ok()
            .map(|image| Contents::Image(image.to_owned_img()))
    })
}

/// Text that is on the clipboard only for the paste, the user's own copy goes back right
/// after: it stays out of the clipboard history, the cloud clipboard and clipboard managers.
pub fn set_passing(clipboard: &mut arboard::Clipboard, text: &str) -> Result<()> {
    #[cfg(windows)]
    let set = {
        use arboard::SetExtWindows;
        clipboard
            .set()
            .exclude_from_monitoring()
            .exclude_from_cloud()
            .exclude_from_history()
    };
    #[cfg(not(windows))]
    let set = hidden(clipboard);
    set.text(text).context("cannot write to the clipboard")
}

/// Marked for clipboard managers to skip: Maccy, Raycast and the like on macOS, Klipper and
/// most others on Linux.
#[cfg(not(windows))]
fn hidden(clipboard: &mut arboard::Clipboard) -> arboard::Set<'_> {
    #[cfg(target_os = "macos")]
    use arboard::SetExtApple;
    #[cfg(not(target_os = "macos"))]
    use arboard::SetExtLinux;
    clipboard.set().exclude_from_history()
}

#[cfg(windows)]
pub fn put(_: &mut arboard::Clipboard, contents: Contents) -> Result<()> {
    let Contents::Formats(formats) = contents;
    win::put(&formats)
}

/// The user's own copy is in the managers' history already, putting it back must not add
/// it twice.
#[cfg(not(windows))]
pub fn put(clipboard: &mut arboard::Clipboard, contents: Contents) -> Result<()> {
    let set = hidden(clipboard);
    match contents {
        Contents::Files(files) => set.file_list(&files),
        Contents::Html { html, text } => set.html(html, text),
        Contents::Text(text) => set.text(text),
        Contents::Image(image) => set.image(image),
    }
    .context("cannot write to the clipboard")
}

/// Formats whose data is not a block of memory but a GDI object or a handle only the
/// owner understands. Windows makes a bitmap from the DIB that is kept anyway.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_memory(format: u32) -> bool {
    const BITMAP: u32 = 2;
    const METAFILEPICT: u32 = 3;
    const PALETTE: u32 = 9;
    const ENHMETAFILE: u32 = 14;
    const OWNERDISPLAY: u32 = 0x80;
    const DSPBITMAP: u32 = 0x82;
    const DSPMETAFILEPICT: u32 = 0x83;
    const DSPENHMETAFILE: u32 = 0x8E;
    // CF_PRIVATEFIRST to CF_GDIOBJLAST.
    const PRIVATE: std::ops::RangeInclusive<u32> = 0x200..=0x3FF;
    !matches!(
        format,
        BITMAP
            | METAFILEPICT
            | PALETTE
            | ENHMETAFILE
            | OWNERDISPLAY
            | DSPBITMAP
            | DSPMETAFILEPICT
            | DSPENHMETAFILE
    ) && !PRIVATE.contains(&format)
}

/// Formats that hold a pointer into the app that copied: by the time they go back that app
/// no longer owns the clipboard, and Office pastes them wrong.
const LIVE_OBJECT: &[&str] = &[
    "DataObject",
    "Ole Private Data",
    "OleClipboardPersistOnFlush",
];

/// What a paste most likely needs, taken whatever the time: text, files, a picture,
/// formatted text.
#[cfg_attr(not(windows), allow(dead_code))]
fn rank(format: u32, name: Option<&str>) -> Option<usize> {
    const UNICODETEXT: u32 = 13;
    const HDROP: u32 = 15;
    const DIBV5: u32 = 17;
    const DIB: u32 = 8;
    const NAMED: &[&str] = &["HTML Format", "Rich Text Format", "PNG"];
    match format {
        UNICODETEXT => Some(0),
        HDROP => Some(1),
        DIBV5 => Some(2),
        DIB => Some(3),
        _ => name
            .and_then(|name| NAMED.iter().position(|n| n.eq_ignore_ascii_case(name)))
            .map(|i| i + 4),
    }
}

/// The formats worth taking, from what the clipboard offers: those every paste needs first,
/// the rest in the order offered. Listing costs nothing, the data is rendered on request and
/// a big spreadsheet takes its time with each format.
#[cfg_attr(not(windows), allow(dead_code))]
fn order(offered: &[(u32, Option<String>)]) -> (Vec<u32>, Vec<u32>) {
    let kept = offered.iter().filter(|(format, name)| {
        is_memory(*format) && !name.as_deref().is_some_and(|n| LIVE_OBJECT.contains(&n))
    });
    let (mut first, rest): (Vec<_>, Vec<_>) =
        kept.partition(|(format, name)| rank(*format, name.as_deref()).is_some());
    first.sort_by_key(|(format, name)| rank(*format, name.as_deref()));
    let ids = |list: Vec<&(u32, Option<String>)>| list.into_iter().map(|(f, _)| *f).collect();
    (ids(first), ids(rest))
}

#[cfg(windows)]
mod win {
    use std::thread;
    use std::time::{Duration, Instant};

    use anyhow::{bail, Result};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
        GetClipboardFormatNameW, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
    };

    /// A huge picture is not worth holding a second copy of.
    const MAX_BYTES: usize = 64 << 20;
    /// The paste is not kept waiting for every format of a big spreadsheet.
    const MAX_WAIT: Duration = Duration::from_millis(500);

    fn format_id(name: &str) -> Option<u32> {
        let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let id = unsafe { RegisterClipboardFormatW(name.as_ptr()) };
        (id != 0).then_some(id)
    }

    /// Another app may hold the clipboard for a moment.
    fn open() -> bool {
        for _ in 0..10 {
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    pub fn take() -> Option<Vec<(u32, Vec<u8>)>> {
        if !open() {
            return None;
        }
        let began = Instant::now();
        let mut offered = Vec::new();
        let mut format = 0;
        loop {
            format = unsafe { EnumClipboardFormats(format) };
            if format == 0 {
                break;
            }
            offered.push((format, name(format)));
        }
        let (first, rest) = super::order(&offered);
        let mut formats = Vec::new();
        let mut total = 0;
        for (i, format) in first.iter().chain(&rest).enumerate() {
            if i >= first.len() && began.elapsed() > MAX_WAIT {
                break;
            }
            if let Some(data) = read(*format, MAX_BYTES - total) {
                total += data.len();
                formats.push((*format, data));
            }
        }
        unsafe { CloseClipboard() };
        (!formats.is_empty()).then_some(formats)
    }

    /// The name of a registered format; standard ones have none.
    fn name(format: u32) -> Option<String> {
        let mut buffer = [0u16; 128];
        let len =
            unsafe { GetClipboardFormatNameW(format, buffer.as_mut_ptr(), buffer.len() as i32) };
        (len > 0).then(|| String::from_utf16_lossy(&buffer[..len as usize]))
    }

    fn read(format: u32, room: usize) -> Option<Vec<u8>> {
        unsafe {
            let handle = GetClipboardData(format);
            if handle.is_null() {
                return None;
            }
            let size = GlobalSize(handle);
            if size > room {
                return None;
            }
            let data = GlobalLock(handle);
            if data.is_null() {
                return None;
            }
            let bytes = std::slice::from_raw_parts(data as *const u8, size).to_vec();
            GlobalUnlock(handle);
            Some(bytes)
        }
    }

    pub fn put(formats: &[(u32, Vec<u8>)]) -> Result<()> {
        // A copy of what the history already has from when the user copied it.
        let history = format_id("CanIncludeInClipboardHistory");
        let no_history = history
            .filter(|id| formats.iter().all(|(f, _)| f != id))
            .map(|id| (id, 0u32.to_ne_bytes().to_vec()));
        if !open() {
            bail!("the clipboard is busy");
        }
        unsafe {
            EmptyClipboard();
            for (format, bytes) in formats.iter().chain(&no_history) {
                let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1));
                if handle.is_null() {
                    continue;
                }
                let data = GlobalLock(handle);
                if data.is_null() {
                    GlobalFree(handle);
                    continue;
                }
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), data as *mut u8, bytes.len());
                GlobalUnlock(handle);
                // On success the clipboard owns the memory.
                if SetClipboardData(*format, handle).is_null() {
                    GlobalFree(handle);
                }
            }
            CloseClipboard();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(format: u32, name: &str) -> (u32, Option<String>) {
        (format, Some(name.into()))
    }

    #[test]
    fn what_a_paste_needs_comes_first() {
        // As Excel lists a range of cells: its own heavy formats lead.
        let offered = [
            named(0xC1A0, "Biff12"),
            named(0xC1A1, "XML Spreadsheet"),
            named(0xC0F0, "HTML Format"),
            (8, None),
            named(0xC009, "DataObject"),
            (2, None),
            named(0xC013, "Ole Private Data"),
            named(0xC0F5, "Rich Text Format"),
            (13, None),
            (17, None),
            named(0xC1B0, "PNG"),
            named(0xC1C0, "Csv"),
            named(0xC00E, "OleClipboardPersistOnFlush"),
        ];
        let (first, rest) = order(&offered);
        assert_eq!(first, [13, 17, 8, 0xC0F0, 0xC0F5, 0xC1B0]);
        assert_eq!(rest, [0xC1A0, 0xC1A1, 0xC1C0]);
    }

    #[test]
    fn files_rank_right_after_text() {
        let (first, rest) = order(&[(15, None), (13, None), named(0xC100, "Shell IDList Array")]);
        assert_eq!(first, [13, 15]);
        assert_eq!(rest, [0xC100]);
    }

    #[test]
    fn keeps_memory_formats_only() {
        const UNICODETEXT: u32 = 13;
        const DIB: u32 = 8;
        const DIBV5: u32 = 17;
        const HDROP: u32 = 15;
        for format in [UNICODETEXT, DIB, DIBV5, HDROP, 0xC0A1] {
            assert!(is_memory(format), "{format}");
        }
        for format in [2, 3, 9, 14, 0x82, 0x200, 0x300, 0x3FF] {
            assert!(!is_memory(format), "{format}");
        }
    }
}
