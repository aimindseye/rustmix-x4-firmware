//! Prepared book cache bridge for host-shaped TXT and EPUB books.
//!
//! Complex-script shaping stays host-side. The X4 loads compact VFNT bitmap
//! assets and VRUN positioned-glyph pages from `/FCACHE/<BOOKID>/`.
//!
//! Two layouts remain supported:
//! - cache format 1: legacy smoke caches with global fonts plus `PAGES.IDX`
//! - cache format 2: production page-local caches with deterministic FAT 8.3
//!   names and only the current page's fonts resident in heap

use alloc::string::String;
use alloc::vec::Vec;

use crate::rustmix_x4::x4_kernel::drivers::strip::StripBuffer;
use crate::rustmix_x4::x4_kernel::kernel::KernelHandle;

const CACHE_ROOT: &str = "FCACHE";
const META_FILE: &str = "META.TXT";
const FONTS_INDEX_FILE: &str = "FONTS.IDX";
const PAGES_INDEX_FILE: &str = "PAGES.IDX";

const CACHE_FORMAT_LEGACY: usize = 1;
const CACHE_FORMAT_PAGE_LOCAL: usize = 2;
const LEGACY_MAX_PAGES: usize = 192;
const PAGE_LOCAL_DIGITS: usize = 5;
const PAGE_LOCAL_CAPACITY: usize = 60_466_176; // 36^5

const MAX_META_BYTES: usize = 1024;
const MAX_INDEX_BYTES: usize = 4 * 1024;
const MAX_FONT_BYTES: usize = 24 * 1024;
const MAX_PAGE_BYTES: usize = 24 * 1024;
const MAX_GLYPHS: usize = 1024;

const FONT_LATIN: u32 = 1;
const FONT_DEVANAGARI: u32 = 2;
const FONT_GUJARATI: u32 = 3;

const VFNT_MAGIC: [u8; 4] = *b"VFNT";
const VFNT_VERSION: u16 = 1;
const VFNT_HEADER_LEN: usize = 44;
const VFNT_GLYPH_METRICS_LEN: usize = 16;
const VFNT_GLYPH_BITMAP_LEN: usize = 16;
const VFNT_FORMAT_ONE_BPP: u16 = 1;

const VRUN_MAGIC: [u8; 4] = *b"VRUN";
const VRUN_VERSION: u16 = 1;
const VRUN_HEADER_LEN: usize = 20;
const VRUN_GLYPH_RECORD_LEN: usize = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PreparedTxtError {
    Missing,
    InvalidMeta,
    MismatchedBook,
    InvalidIndex,
    MissingFont,
    InvalidFont,
    InvalidPage,
    TooLarge,
}

impl PreparedTxtError {
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::Missing => "MISSING",
            Self::InvalidMeta => "META",
            Self::MismatchedBook => "BOOK",
            Self::InvalidIndex => "INDEX",
            Self::MissingFont => "FONT_MISSING",
            Self::InvalidFont => "FONT",
            Self::InvalidPage => "PAGE",
            Self::TooLarge => "TOO_LARGE",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparedCacheLayout {
    Legacy,
    PageLocalBase36,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedGlyphRecord {
    font_id: u32,
    glyph_id: u32,
    x: i16,
    y: i16,
}

impl PreparedGlyphRecord {
    const fn empty() -> Self {
        Self {
            font_id: 0,
            glyph_id: 0,
            x: 0,
            y: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RequiredFonts {
    latin: bool,
    devanagari: bool,
    gujarati: bool,
}

impl RequiredFonts {
    const fn empty() -> Self {
        Self {
            latin: false,
            devanagari: false,
            gujarati: false,
        }
    }
}

pub(super) struct PreparedTxtState {
    active: bool,
    layout: PreparedCacheLayout,
    book_id: String,
    page_count: usize,
    legacy_page_files: [String; LEGACY_MAX_PAGES],
    latin_font: Vec<u8>,
    devanagari_font: Vec<u8>,
    gujarati_font: Vec<u8>,
    glyphs: [PreparedGlyphRecord; MAX_GLYPHS],
    glyph_count: usize,
}

impl PreparedTxtState {
    pub(super) const fn new() -> Self {
        Self {
            active: false,
            layout: PreparedCacheLayout::Legacy,
            book_id: String::new(),
            page_count: 0,
            legacy_page_files: [const { String::new() }; LEGACY_MAX_PAGES],
            latin_font: Vec::new(),
            devanagari_font: Vec::new(),
            gujarati_font: Vec::new(),
            glyphs: [PreparedGlyphRecord::empty(); MAX_GLYPHS],
            glyph_count: 0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.active = false;
        self.layout = PreparedCacheLayout::Legacy;
        self.book_id.clear();
        self.page_count = 0;
        for name in &mut self.legacy_page_files {
            name.clear();
        }
        self.clear_fonts();
        self.glyph_count = 0;
    }

    fn clear_fonts(&mut self) {
        self.latin_font.clear();
        self.devanagari_font.clear();
        self.gujarati_font.clear();
    }

    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    pub(super) fn page_count(&self) -> usize {
        self.page_count
    }

    pub(super) fn try_open(
        &mut self,
        k: &mut KernelHandle<'_>,
        book_id: &str,
        source_path: &str,
    ) -> Result<(), PreparedTxtError> {
        self.clear();

        let result = self.try_open_inner(k, book_id, source_path);
        if result.is_err() {
            self.clear();
        }
        result
    }

    fn try_open_inner(
        &mut self,
        k: &mut KernelHandle<'_>,
        book_id: &str,
        source_path: &str,
    ) -> Result<(), PreparedTxtError> {
        let meta = read_cache_file(k, book_id, META_FILE, MAX_META_BYTES)?;
        let meta = core::str::from_utf8(&meta).map_err(|_| PreparedTxtError::InvalidMeta)?;
        let parsed_meta = parse_meta(meta)?;
        if !eq_ignore_ascii_case(parsed_meta.book_id, book_id) {
            return Err(PreparedTxtError::MismatchedBook);
        }
        if !parsed_meta.source.is_empty() && !source_matches(parsed_meta.source, source_path) {
            return Err(PreparedTxtError::MismatchedBook);
        }

        self.layout = parsed_meta.layout;
        self.book_id.push_str(book_id);
        self.page_count = parsed_meta.page_count;

        if self.layout == PreparedCacheLayout::Legacy {
            self.open_legacy_assets(k, book_id, parsed_meta.page_count)?;
        }

        self.active = true;
        self.load_page(k, 0)
    }

    fn open_legacy_assets(
        &mut self,
        k: &mut KernelHandle<'_>,
        book_id: &str,
        expected_pages: usize,
    ) -> Result<(), PreparedTxtError> {
        let fonts_index = read_cache_file(k, book_id, FONTS_INDEX_FILE, MAX_INDEX_BYTES)?;
        let fonts_index =
            core::str::from_utf8(&fonts_index).map_err(|_| PreparedTxtError::InvalidIndex)?;
        let fonts = parse_fonts_index(fonts_index)?;

        let pages_index = read_cache_file(k, book_id, PAGES_INDEX_FILE, MAX_INDEX_BYTES)?;
        let pages_index =
            core::str::from_utf8(&pages_index).map_err(|_| PreparedTxtError::InvalidIndex)?;
        let page_count = parse_pages_index(pages_index, &mut self.legacy_page_files)?;
        if page_count == 0 || page_count != expected_pages {
            return Err(PreparedTxtError::InvalidIndex);
        }

        self.latin_font = read_cache_file(k, book_id, fonts.latin, MAX_FONT_BYTES)?;
        if let Some(devanagari) = fonts.devanagari {
            self.devanagari_font = read_cache_file(k, book_id, devanagari, MAX_FONT_BYTES)?;
        }
        if let Some(gujarati) = fonts.gujarati {
            self.gujarati_font = read_cache_file(k, book_id, gujarati, MAX_FONT_BYTES)?;
        }
        validate_loaded_font(&self.latin_font)?;
        validate_optional_loaded_font(&self.devanagari_font)?;
        validate_optional_loaded_font(&self.gujarati_font)?;
        Ok(())
    }

    pub(super) fn load_page(
        &mut self,
        k: &mut KernelHandle<'_>,
        page: usize,
    ) -> Result<(), PreparedTxtError> {
        if !self.active || page >= self.page_count {
            return Err(PreparedTxtError::InvalidPage);
        }

        let page_name = match self.layout {
            PreparedCacheLayout::Legacy => self.legacy_page_files[page].clone(),
            PreparedCacheLayout::PageLocalBase36 => page_local_file_name('P', page, "VRN")?,
        };
        let page_data = read_cache_file(k, &self.book_id, &page_name, MAX_PAGE_BYTES)?;
        let count = parse_page_records(&page_data, &mut self.glyphs)?;
        let required = required_fonts(&self.glyphs[..count])?;

        if self.layout == PreparedCacheLayout::PageLocalBase36 {
            self.load_page_local_fonts(k, page, required)?;
        }

        validate_glyph_references(
            &self.latin_font,
            &self.devanagari_font,
            &self.gujarati_font,
            &self.glyphs[..count],
        )?;

        self.glyph_count = count;
        Ok(())
    }

    fn load_page_local_fonts(
        &mut self,
        k: &mut KernelHandle<'_>,
        page: usize,
        required: RequiredFonts,
    ) -> Result<(), PreparedTxtError> {
        self.clear_fonts();
        let book_id = self.book_id.clone();

        if required.latin {
            let name = page_local_font_file_name(FONT_LATIN, page)?;
            self.latin_font = read_cache_file(k, &book_id, &name, MAX_FONT_BYTES)?;
            validate_loaded_font(&self.latin_font)?;
        }
        if required.devanagari {
            let name = page_local_font_file_name(FONT_DEVANAGARI, page)?;
            self.devanagari_font = read_cache_file(k, &book_id, &name, MAX_FONT_BYTES)?;
            validate_loaded_font(&self.devanagari_font)?;
        }
        if required.gujarati {
            let name = page_local_font_file_name(FONT_GUJARATI, page)?;
            self.gujarati_font = read_cache_file(k, &book_id, &name, MAX_FONT_BYTES)?;
            validate_loaded_font(&self.gujarati_font)?;
        }
        Ok(())
    }

    pub(super) fn draw(&self, strip: &mut StripBuffer, x: i32, y: i32) {
        if !self.active {
            return;
        }

        let latin = VfntView::parse(&self.latin_font).ok();
        let devanagari = VfntView::parse(&self.devanagari_font).ok();
        let gujarati = VfntView::parse(&self.gujarati_font).ok();

        for glyph in &self.glyphs[..self.glyph_count] {
            let font = match glyph.font_id {
                FONT_LATIN => latin,
                FONT_DEVANAGARI => devanagari,
                FONT_GUJARATI => gujarati,
                _ => None,
            };
            let Some(font) = font else {
                continue;
            };
            let Ok(bitmap) = font.glyph(glyph.glyph_id) else {
                continue;
            };
            strip.blit_1bpp(
                bitmap.data,
                0,
                bitmap.width as usize,
                bitmap.height as usize,
                bitmap.row_stride as usize,
                x + glyph.x as i32,
                y + glyph.y as i32,
                true,
            );
        }
    }
}

struct ParsedMeta<'a> {
    book_id: &'a str,
    source: &'a str,
    page_count: usize,
    layout: PreparedCacheLayout,
}

#[derive(Debug)]
struct FontIndex<'a> {
    latin: &'a str,
    devanagari: Option<&'a str>,
    gujarati: Option<&'a str>,
}

#[derive(Clone, Copy)]
struct VfntView<'a> {
    data: &'a [u8],
    glyph_count: usize,
    metrics_offset: usize,
    bitmap_index_offset: usize,
    bitmap_data_offset: usize,
}

struct GlyphBitmap<'a> {
    data: &'a [u8],
    width: u16,
    height: u16,
    row_stride: u16,
}

impl<'a> VfntView<'a> {
    fn parse(data: &'a [u8]) -> Result<Self, PreparedTxtError> {
        if data.len() < VFNT_HEADER_LEN {
            return Err(PreparedTxtError::InvalidFont);
        }
        if data[0..4] != VFNT_MAGIC {
            return Err(PreparedTxtError::InvalidFont);
        }
        if read_u16(data, 4)? != VFNT_VERSION {
            return Err(PreparedTxtError::InvalidFont);
        }
        if usize::from(read_u16(data, 6)?) < VFNT_HEADER_LEN {
            return Err(PreparedTxtError::InvalidFont);
        }
        let glyph_count =
            usize::try_from(read_u32(data, 20)?).map_err(|_| PreparedTxtError::InvalidFont)?;
        if glyph_count == 0 {
            return Err(PreparedTxtError::InvalidFont);
        }
        let metrics_offset =
            usize::try_from(read_u32(data, 24)?).map_err(|_| PreparedTxtError::InvalidFont)?;
        let bitmap_index_offset =
            usize::try_from(read_u32(data, 28)?).map_err(|_| PreparedTxtError::InvalidFont)?;
        let bitmap_data_offset =
            usize::try_from(read_u32(data, 32)?).map_err(|_| PreparedTxtError::InvalidFont)?;
        let bitmap_data_len =
            usize::try_from(read_u32(data, 36)?).map_err(|_| PreparedTxtError::InvalidFont)?;
        let bitmap_format = read_u16(data, 42)?;
        if bitmap_format != VFNT_FORMAT_ONE_BPP {
            return Err(PreparedTxtError::InvalidFont);
        }

        checked_range(
            data.len(),
            metrics_offset,
            glyph_count * VFNT_GLYPH_METRICS_LEN,
        )?;
        checked_range(
            data.len(),
            bitmap_index_offset,
            glyph_count * VFNT_GLYPH_BITMAP_LEN,
        )?;
        checked_range(data.len(), bitmap_data_offset, bitmap_data_len)?;

        let font = Self {
            data,
            glyph_count,
            metrics_offset,
            bitmap_index_offset,
            bitmap_data_offset,
        };
        for index in 0..glyph_count {
            let bitmap = font.bitmap_record(index)?;
            checked_range(bitmap_data_len, bitmap.offset, bitmap.len)?;
        }
        Ok(font)
    }

    fn glyph(self, glyph_id: u32) -> Result<GlyphBitmap<'a>, PreparedTxtError> {
        for index in 0..self.glyph_count {
            let metrics = self.metrics_record(index)?;
            if metrics.glyph_id == glyph_id {
                let bitmap = self.bitmap_record(index)?;
                if bitmap.glyph_id != glyph_id {
                    return Err(PreparedTxtError::InvalidFont);
                }
                let start = self.bitmap_data_offset + bitmap.offset;
                let end = start + bitmap.len;
                let row_min = usize::from(metrics.width).div_ceil(8);
                let required = usize::from(bitmap.row_stride)
                    .checked_mul(usize::from(metrics.height))
                    .ok_or(PreparedTxtError::InvalidFont)?;
                if usize::from(bitmap.row_stride) < row_min || required > bitmap.len {
                    return Err(PreparedTxtError::InvalidFont);
                }
                return Ok(GlyphBitmap {
                    data: &self.data[start..end],
                    width: metrics.width,
                    height: metrics.height,
                    row_stride: bitmap.row_stride,
                });
            }
        }
        Err(PreparedTxtError::InvalidPage)
    }

    fn metrics_record(self, index: usize) -> Result<GlyphMetrics, PreparedTxtError> {
        let off = self.metrics_offset + index * VFNT_GLYPH_METRICS_LEN;
        Ok(GlyphMetrics {
            glyph_id: read_u32(self.data, off)?,
            width: read_u16(self.data, off + 12)?,
            height: read_u16(self.data, off + 14)?,
        })
    }

    fn bitmap_record(self, index: usize) -> Result<GlyphBitmapRecord, PreparedTxtError> {
        let off = self.bitmap_index_offset + index * VFNT_GLYPH_BITMAP_LEN;
        Ok(GlyphBitmapRecord {
            glyph_id: read_u32(self.data, off)?,
            offset: usize::try_from(read_u32(self.data, off + 4)?)
                .map_err(|_| PreparedTxtError::InvalidFont)?,
            len: usize::try_from(read_u32(self.data, off + 8)?)
                .map_err(|_| PreparedTxtError::InvalidFont)?,
            row_stride: read_u16(self.data, off + 12)?,
        })
    }
}

#[derive(Clone, Copy)]
struct GlyphMetrics {
    glyph_id: u32,
    width: u16,
    height: u16,
}

#[derive(Clone, Copy)]
struct GlyphBitmapRecord {
    glyph_id: u32,
    offset: usize,
    len: usize,
    row_stride: u16,
}

fn read_cache_file(
    k: &mut KernelHandle<'_>,
    book_id: &str,
    name: &str,
    max_len: usize,
) -> Result<Vec<u8>, PreparedTxtError> {
    let read_limit = max_len.checked_add(1).ok_or(PreparedTxtError::TooLarge)?;

    let mut data = Vec::new();
    data.resize(read_limit, 0);

    let n = k
        .read_subdir_chunk(CACHE_ROOT, book_id, name, 0, &mut data)
        .map_err(|_| PreparedTxtError::Missing)?;

    if n > max_len {
        return Err(PreparedTxtError::TooLarge);
    }

    data.truncate(n);
    Ok(data)
}

fn validate_loaded_font(data: &[u8]) -> Result<(), PreparedTxtError> {
    VfntView::parse(data)
        .map(|_| ())
        .map_err(|_| PreparedTxtError::InvalidFont)
}

fn validate_optional_loaded_font(data: &[u8]) -> Result<(), PreparedTxtError> {
    if data.is_empty() {
        Ok(())
    } else {
        validate_loaded_font(data)
    }
}

fn validate_glyph_references(
    latin_data: &[u8],
    devanagari_data: &[u8],
    gujarati_data: &[u8],
    glyphs: &[PreparedGlyphRecord],
) -> Result<(), PreparedTxtError> {
    let latin = VfntView::parse(latin_data).ok();
    let devanagari = VfntView::parse(devanagari_data).ok();
    let gujarati = VfntView::parse(gujarati_data).ok();

    for glyph in glyphs {
        let font = match glyph.font_id {
            FONT_LATIN => latin,
            FONT_DEVANAGARI => devanagari,
            FONT_GUJARATI => gujarati,
            _ => return Err(PreparedTxtError::MissingFont),
        }
        .ok_or(PreparedTxtError::MissingFont)?;
        font.glyph(glyph.glyph_id)
            .map_err(|_| PreparedTxtError::InvalidPage)?;
    }
    Ok(())
}

fn required_fonts(glyphs: &[PreparedGlyphRecord]) -> Result<RequiredFonts, PreparedTxtError> {
    let mut out = RequiredFonts::empty();
    for glyph in glyphs {
        match glyph.font_id {
            FONT_LATIN => out.latin = true,
            FONT_DEVANAGARI => out.devanagari = true,
            FONT_GUJARATI => out.gujarati = true,
            _ => return Err(PreparedTxtError::MissingFont),
        }
    }
    Ok(out)
}

fn parse_meta(input: &str) -> Result<ParsedMeta<'_>, PreparedTxtError> {
    let mut book_id = "";
    let mut source = "";
    let mut page_count = None;
    let mut cache_format = CACHE_FORMAT_LEGACY;
    for (key, value) in lines(input) {
        match key {
            "book_id" => book_id = value,
            "source" => source = value,
            "page_count" => page_count = parse_usize(value),
            "cache_format" => {
                cache_format = parse_usize(value).ok_or(PreparedTxtError::InvalidMeta)?;
            }
            _ => {}
        }
    }
    let page_count = page_count.ok_or(PreparedTxtError::InvalidMeta)?;
    let layout = match cache_format {
        CACHE_FORMAT_LEGACY if page_count <= LEGACY_MAX_PAGES => PreparedCacheLayout::Legacy,
        CACHE_FORMAT_PAGE_LOCAL if page_count <= PAGE_LOCAL_CAPACITY => {
            PreparedCacheLayout::PageLocalBase36
        }
        _ => return Err(PreparedTxtError::InvalidMeta),
    };
    if book_id.is_empty() || page_count == 0 {
        return Err(PreparedTxtError::InvalidMeta);
    }
    Ok(ParsedMeta {
        book_id,
        source,
        page_count,
        layout,
    })
}

fn parse_fonts_index(input: &str) -> Result<FontIndex<'_>, PreparedTxtError> {
    let mut latin = "";
    let mut devanagari = None;
    let mut gujarati = None;
    for (key, value) in lines(input) {
        if key.eq_ignore_ascii_case("Latin") {
            latin = value;
        } else if key.eq_ignore_ascii_case("Devanagari") {
            devanagari = Some(value);
        } else if key.eq_ignore_ascii_case("Gujarati") {
            gujarati = Some(value);
        }
    }
    if !valid_cache_file(latin) {
        return Err(PreparedTxtError::MissingFont);
    }
    if let Some(file) = devanagari {
        if !valid_cache_file(file) {
            return Err(PreparedTxtError::MissingFont);
        }
    }
    if let Some(file) = gujarati {
        if !valid_cache_file(file) {
            return Err(PreparedTxtError::MissingFont);
        }
    }
    Ok(FontIndex {
        latin,
        devanagari,
        gujarati,
    })
}

fn parse_pages_index(
    input: &str,
    pages: &mut [String; LEGACY_MAX_PAGES],
) -> Result<usize, PreparedTxtError> {
    let mut count = 0usize;
    for raw in input.lines() {
        let page = raw.trim();
        if page.is_empty() {
            continue;
        }
        if count >= LEGACY_MAX_PAGES || !valid_cache_file(page) {
            return Err(PreparedTxtError::InvalidIndex);
        }
        pages[count].clear();
        pages[count].push_str(page);
        count += 1;
    }
    Ok(count)
}

fn page_local_file_name(
    prefix: char,
    page: usize,
    extension: &str,
) -> Result<String, PreparedTxtError> {
    if !prefix.is_ascii_alphanumeric()
        || extension.len() != 3
        || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || page >= PAGE_LOCAL_CAPACITY
    {
        return Err(PreparedTxtError::TooLarge);
    }

    let mut value = page;
    let mut digits = [b'0'; PAGE_LOCAL_DIGITS];
    for slot in digits.iter_mut().rev() {
        let digit = (value % 36) as u8;
        *slot = if digit < 10 {
            b'0' + digit
        } else {
            b'A' + (digit - 10)
        };
        value /= 36;
    }

    let mut out = String::new();
    out.push(prefix);
    for digit in digits {
        out.push(char::from(digit));
    }
    out.push('.');
    out.push_str(extension);
    if valid_cache_file(&out) {
        Ok(out)
    } else {
        Err(PreparedTxtError::InvalidPage)
    }
}

fn page_local_font_file_name(font_id: u32, page: usize) -> Result<String, PreparedTxtError> {
    let prefix = match font_id {
        FONT_LATIN => 'L',
        FONT_DEVANAGARI => 'D',
        FONT_GUJARATI => 'G',
        _ => return Err(PreparedTxtError::MissingFont),
    };
    page_local_file_name(prefix, page, "VFN")
}

fn parse_page_records(
    input: &[u8],
    out: &mut [PreparedGlyphRecord; MAX_GLYPHS],
) -> Result<usize, PreparedTxtError> {
    if input.len() < VRUN_HEADER_LEN {
        return Err(PreparedTxtError::InvalidPage);
    }
    if input[0..4] != VRUN_MAGIC {
        return Err(PreparedTxtError::InvalidPage);
    }
    if read_u16(input, 4)? != VRUN_VERSION {
        return Err(PreparedTxtError::InvalidPage);
    }
    if usize::from(read_u16(input, 6)?) < VRUN_HEADER_LEN {
        return Err(PreparedTxtError::InvalidPage);
    }
    let glyph_count =
        usize::try_from(read_u32(input, 8)?).map_err(|_| PreparedTxtError::InvalidPage)?;
    if glyph_count > MAX_GLYPHS {
        return Err(PreparedTxtError::TooLarge);
    }
    checked_range(
        input.len(),
        VRUN_HEADER_LEN,
        glyph_count * VRUN_GLYPH_RECORD_LEN,
    )?;
    for (index, slot) in out.iter_mut().enumerate().take(glyph_count) {
        let off = VRUN_HEADER_LEN + index * VRUN_GLYPH_RECORD_LEN;
        *slot = PreparedGlyphRecord {
            font_id: read_u32(input, off)?,
            glyph_id: read_u32(input, off + 4)?,
            x: read_i16(input, off + 8)?,
            y: read_i16(input, off + 10)?,
        };
    }
    Ok(glyph_count)
}

fn lines(input: &str) -> impl Iterator<Item = (&str, &str)> {
    input.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        Some((key.trim(), value.trim()))
    })
}

fn parse_usize(input: &str) -> Option<usize> {
    let mut value = 0usize;
    if input.is_empty() {
        return None;
    }
    for byte in input.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(usize::from(byte - b'0'))?;
    }
    Some(value)
}

fn source_matches(_cache_source: &str, _reader_source: &str) -> bool {
    // The prepared cache directory/book_id is authoritative. Do not reject a
    // cache because FAT 8.3 names, case, or leading slashes differ.
    true
}

fn eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(left, right)| left.eq_ignore_ascii_case(&right))
}

fn valid_cache_file(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 12
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_')
}

fn checked_range(total: usize, start: usize, len: usize) -> Result<(), PreparedTxtError> {
    let end = start
        .checked_add(len)
        .ok_or(PreparedTxtError::InvalidPage)?;
    if start <= total && end <= total {
        Ok(())
    } else {
        Err(PreparedTxtError::InvalidPage)
    }
}

fn read_u16(data: &[u8], off: usize) -> Result<u16, PreparedTxtError> {
    let bytes = data
        .get(off..off + 2)
        .ok_or(PreparedTxtError::InvalidPage)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_i16(data: &[u8], off: usize) -> Result<i16, PreparedTxtError> {
    let bytes = data
        .get(off..off + 2)
        .ok_or(PreparedTxtError::InvalidPage)?;
    Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, PreparedTxtError> {
    let bytes = data
        .get(off..off + 4)
        .ok_or(PreparedTxtError::InvalidPage)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(all(test, not(target_arch = "riscv32")))]
mod tests {
    use super::*;

    #[test]
    fn detects_existing_prepared_cache_by_book_id() {
        let meta = "book_id=ABCDEF12\nsource=/Books/MIXED.TXT\npage_count=1\n";
        let parsed = parse_meta(meta).unwrap();
        assert_eq!(parsed.book_id, "ABCDEF12");
        assert_eq!(parsed.page_count, 1);
        assert_eq!(parsed.layout, PreparedCacheLayout::Legacy);
    }

    #[test]
    fn parses_page_local_cache_with_real_book_page_count() {
        let meta = "book_id=ABCDEF12\nsource=/Books/GARUD.EPU\ncache_format=2\npage_count=4096\n";
        let parsed = parse_meta(meta).unwrap();
        assert_eq!(parsed.page_count, 4096);
        assert_eq!(parsed.layout, PreparedCacheLayout::PageLocalBase36);
    }

    #[test]
    fn rejects_oversized_legacy_page_index() {
        let meta = "book_id=ABCDEF12\npage_count=193\n";
        assert_eq!(parse_meta(meta).unwrap_err(), PreparedTxtError::InvalidMeta);
    }

    #[test]
    fn missing_prepared_cache_falls_back_to_reader() {
        let state = PreparedTxtState::new();
        assert!(!state.is_active());
    }

    #[test]
    fn rejects_cache_with_mismatched_book_id() {
        let parsed = parse_meta("book_id=ABCDEF12\npage_count=1\n").unwrap();
        assert!(!eq_ignore_ascii_case(parsed.book_id, "00000000"));
    }

    #[test]
    fn parses_legacy_pages_index() {
        let mut pages = core::array::from_fn(|_| String::new());
        let count = parse_pages_index("P000.VRN\nP001.VRN\n", &mut pages).unwrap();
        assert_eq!(count, 2);
        assert_eq!(pages[0], "P000.VRN");
        assert_eq!(pages[1], "P001.VRN");
    }

    #[test]
    fn generates_page_local_fat_83_names() {
        assert_eq!(page_local_file_name('P', 0, "VRN").unwrap(), "P00000.VRN");
        assert_eq!(page_local_file_name('P', 35, "VRN").unwrap(), "P0000Z.VRN");
        assert_eq!(page_local_file_name('P', 36, "VRN").unwrap(), "P00010.VRN");
        assert_eq!(
            page_local_font_file_name(FONT_DEVANAGARI, 1).unwrap(),
            "D00001.VFN"
        );
        assert_eq!(
            page_local_font_file_name(FONT_GUJARATI, 1).unwrap(),
            "G00001.VFN"
        );
    }

    #[test]
    fn parses_legacy_fonts_index_with_optional_script_fonts() {
        let fonts =
            parse_fonts_index("Latin=LAT18.VFN\nDevanagari=DEV22.VFN\nGujarati=GUJ22.VFN\n")
                .unwrap();
        assert_eq!(fonts.latin, "LAT18.VFN");
        assert_eq!(fonts.devanagari, Some("DEV22.VFN"));
        assert_eq!(fonts.gujarati, Some("GUJ22.VFN"));
    }

    #[test]
    fn accepts_latin_only_legacy_fonts_index() {
        let fonts = parse_fonts_index("Latin=LAT18.VFN\n").unwrap();
        assert_eq!(fonts.latin, "LAT18.VFN");
        assert_eq!(fonts.devanagari, None);
        assert_eq!(fonts.gujarati, None);
    }

    #[test]
    fn rejects_missing_latin_font_asset() {
        assert_eq!(
            parse_fonts_index("Devanagari=DEV22.VFN\n").unwrap_err(),
            PreparedTxtError::MissingFont
        );
    }

    #[test]
    fn detects_fonts_required_by_gujarati_page() {
        let glyphs = [
            PreparedGlyphRecord {
                font_id: FONT_LATIN,
                glyph_id: 65,
                x: 0,
                y: 0,
            },
            PreparedGlyphRecord {
                font_id: FONT_GUJARATI,
                glyph_id: 42,
                x: 12,
                y: 8,
            },
        ];
        let required = required_fonts(&glyphs).unwrap();
        assert!(required.latin);
        assert!(!required.devanagari);
        assert!(required.gujarati);
    }

    #[test]
    fn rejects_invalid_vfnt_asset() {
        assert!(VfntView::parse(b"not a font").is_err());
    }

    #[test]
    fn parses_prepared_page_with_multiple_glyphs() {
        let page = test_page(&[
            PreparedGlyphRecord {
                font_id: FONT_LATIN,
                glyph_id: 65,
                x: 0,
                y: 0,
            },
            PreparedGlyphRecord {
                font_id: FONT_DEVANAGARI,
                glyph_id: 42,
                x: 12,
                y: 8,
            },
        ]);
        let mut out = [PreparedGlyphRecord::empty(); MAX_GLYPHS];
        let count = parse_page_records(&page, &mut out).unwrap();
        assert_eq!(count, 2);
        assert_eq!(out[1].font_id, FONT_DEVANAGARI);
        assert_eq!(out[1].x, 12);
    }

    #[test]
    fn prepared_page_rejects_bad_magic() {
        let mut page = test_page(&[]);
        page[0] = b'B';
        let mut out = [PreparedGlyphRecord::empty(); MAX_GLYPHS];
        assert!(parse_page_records(&page, &mut out).is_err());
    }

    #[test]
    fn prepared_page_rejects_truncated_glyph_records() {
        let mut page = test_page(&[PreparedGlyphRecord {
            font_id: FONT_LATIN,
            glyph_id: 65,
            x: 0,
            y: 0,
        }]);
        page.pop();
        let mut out = [PreparedGlyphRecord::empty(); MAX_GLYPHS];
        assert!(parse_page_records(&page, &mut out).is_err());
    }

    fn test_page(records: &[PreparedGlyphRecord]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&VRUN_MAGIC);
        out.extend_from_slice(&VRUN_VERSION.to_le_bytes());
        out.extend_from_slice(&(VRUN_HEADER_LEN as u16).to_le_bytes());
        out.extend_from_slice(&(records.len() as u32).to_le_bytes());
        out.extend_from_slice(&480u16.to_le_bytes());
        out.extend_from_slice(&800u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for record in records {
            out.extend_from_slice(&record.font_id.to_le_bytes());
            out.extend_from_slice(&record.glyph_id.to_le_bytes());
            out.extend_from_slice(&record.x.to_le_bytes());
            out.extend_from_slice(&record.y.to_le_bytes());
            out.extend_from_slice(&8i16.to_le_bytes());
            out.extend_from_slice(&0i16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
        }
        out
    }
}
