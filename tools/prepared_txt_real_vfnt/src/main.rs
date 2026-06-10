use fontdue::{Font, FontSettings, Metrics};
use rustybuzz::{script, Direction, Face, UnicodeBuffer};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const VFNT_MAGIC: &[u8; 4] = b"VFNT";
const VFNT_VERSION: u16 = 1;
const VFNT_HEADER_LEN: usize = 44;
const VFNT_METRICS_LEN: usize = 16;
const VFNT_BITMAP_LEN: usize = 16;
const VFNT_ONE_BPP: u16 = 1;
const SCRIPT_LATIN: u16 = 1;
const SCRIPT_DEVANAGARI: u16 = 2;
const SCRIPT_GUJARATI: u16 = 3;

const VRUN_MAGIC: &[u8; 4] = b"VRUN";
const VRUN_VERSION: u16 = 1;
const VRUN_HEADER_LEN: usize = 20;
const VRUN_RECORD_LEN: usize = 20;

const FONT_LATIN: u32 = 1;
const FONT_DEVANAGARI: u32 = 2;
const FONT_GUJARATI: u32 = 3;

const CACHE_FORMAT_PAGE_LOCAL: usize = 2;
const PAGE_LOCAL_DIGITS: usize = 5;
const PAGE_LOCAL_CAPACITY: usize = 60_466_176; // 36^5
const MAX_PAGE_GLYPHS: usize = 1024;
const MAX_PAGE_BYTES: usize = 24 * 1024;
const MAX_FONT_BYTES: usize = 24 * 1024;

#[derive(Clone, Debug)]
struct Args {
    book: PathBuf,
    device_path: String,
    latin_font: PathBuf,
    devanagari_font: PathBuf,
    gujarati_font: Option<PathBuf>,
    out: PathBuf,
    title: String,
    latin_size: f32,
    devanagari_size: f32,
    gujarati_size: f32,
    line_height: Option<i16>,
    page_width: i16,
    page_height: i16,
    margin_x: i16,
    margin_y: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScriptKind {
    Latin,
    Devanagari,
    Gujarati,
}

#[derive(Clone, Debug)]
struct TextRun<'a> {
    script: ScriptKind,
    text: &'a str,
}

#[derive(Clone, Copy, Debug)]
struct ShapedGlyph {
    font_id: u32,
    glyph_id: u32,
    x_offset: i16,
    y_offset: i16,
    advance_x: i16,
    advance_y: i16,
    cluster: u32,
}

#[derive(Clone, Copy, Debug)]
struct PositionedGlyph {
    font_id: u32,
    glyph_id: u32,
    x: i16,
    y: i16,
    advance_x: i16,
    advance_y: i16,
    cluster: u32,
}

#[derive(Clone, Debug)]
struct PreparedPage {
    glyphs: Vec<PositionedGlyph>,
}

#[derive(Clone, Copy, Debug)]
struct GlyphResource {
    font_id: u32,
    glyph_id: u32,
    bitmap_bytes: usize,
}

struct MeasuredGlyph {
    shaped: ShapedGlyph,
    metrics: Metrics,
    resource: GlyphResource,
}

#[derive(Clone, Debug, Default)]
struct PageBudget {
    glyph_count: usize,
    font_bytes: [usize; 3],
    glyph_ids: BTreeSet<(u32, u32)>,
}

impl PageBudget {
    fn projected(&self, resources: &[GlyphResource]) -> Result<Option<Self>, String> {
        let glyph_count = self
            .glyph_count
            .checked_add(resources.len())
            .ok_or_else(|| "prepared page glyph count overflows".to_string())?;
        let page_bytes = VRUN_HEADER_LEN
            .checked_add(
                glyph_count
                    .checked_mul(VRUN_RECORD_LEN)
                    .ok_or_else(|| "prepared page byte count overflows".to_string())?,
            )
            .ok_or_else(|| "prepared page byte count overflows".to_string())?;
        if glyph_count > MAX_PAGE_GLYPHS || page_bytes > MAX_PAGE_BYTES {
            return Ok(None);
        }

        let mut projected = self.clone();
        projected.glyph_count = glyph_count;
        for resource in resources {
            let slot = font_budget_slot(resource.font_id)?;
            if projected
                .glyph_ids
                .insert((resource.font_id, resource.glyph_id))
            {
                if projected.font_bytes[slot] == 0 {
                    projected.font_bytes[slot] = VFNT_HEADER_LEN;
                }
                projected.font_bytes[slot] = projected.font_bytes[slot]
                    .checked_add(VFNT_METRICS_LEN + VFNT_BITMAP_LEN)
                    .and_then(|size| size.checked_add(resource.bitmap_bytes))
                    .ok_or_else(|| "page-local font byte count overflows".to_string())?;
                if projected.font_bytes[slot] > MAX_FONT_BYTES {
                    return Ok(None);
                }
            }
        }
        Ok(Some(projected))
    }

    fn add(&mut self, resources: &[GlyphResource]) -> Result<bool, String> {
        let Some(projected) = self.projected(resources)? else {
            return Ok(false);
        };
        *self = projected;
        Ok(true)
    }
}

#[derive(Clone, Debug)]
struct GlyphAsset {
    glyph_id: u32,
    advance_x: i16,
    bearing_x: i16,
    bearing_y: i16,
    width: u16,
    height: u16,
    row_stride: u16,
    bitmap: Vec<u8>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let text =
        fs::read_to_string(&args.book).map_err(|err| format!("read book as UTF-8: {err}"))?;
    let latin_data = read_required_file(&args.latin_font, "Latin font")?;
    let devanagari_data = read_required_file(&args.devanagari_font, "Devanagari font")?;
    let gujarati_data = args
        .gujarati_font
        .as_ref()
        .map(|path| read_required_file(path, "Gujarati font"))
        .transpose()?;

    let mut latin_face =
        Face::from_slice(&latin_data, 0).ok_or("parse Latin font with rustybuzz")?;
    latin_face.set_points_per_em(Some(args.latin_size));
    let mut devanagari_face =
        Face::from_slice(&devanagari_data, 0).ok_or("parse Devanagari font with rustybuzz")?;
    devanagari_face.set_points_per_em(Some(args.devanagari_size));
    let gujarati_face = match gujarati_data.as_ref() {
        Some(data) => {
            let mut face = Face::from_slice(data, 0).ok_or("parse Gujarati font with rustybuzz")?;
            face.set_points_per_em(Some(args.gujarati_size));
            Some(face)
        }
        None => None,
    };

    let latin_font = Font::from_bytes(latin_data.clone(), FontSettings::default())
        .map_err(|err| format!("parse Latin font for rasterization: {err}"))?;
    let devanagari_font = Font::from_bytes(devanagari_data.clone(), FontSettings::default())
        .map_err(|err| format!("parse Devanagari font for rasterization: {err}"))?;
    let gujarati_font = match gujarati_data.as_ref() {
        Some(data) => Some(
            Font::from_bytes(data.clone(), FontSettings::default())
                .map_err(|err| format!("parse Gujarati font for rasterization: {err}"))?,
        ),
        None => None,
    };

    let mut layout = layout_text(
        &args,
        &text,
        &latin_face,
        &devanagari_face,
        gujarati_face.as_ref(),
        &latin_font,
        &devanagari_font,
        gujarati_font.as_ref(),
    )?;
    while layout.pages.len() > 1
        && layout
            .pages
            .last()
            .map_or(false, |page| page.glyphs.is_empty())
    {
        layout.pages.pop();
    }
    if layout.pages.iter().all(|page| page.glyphs.is_empty()) {
        return Err("prepared page has no glyphs".to_string());
    }
    if layout.pages.len() > PAGE_LOCAL_CAPACITY {
        return Err(format!(
            "prepared cache has too many pages: {} > {PAGE_LOCAL_CAPACITY}",
            layout.pages.len()
        ));
    }

    let book_id = book_folder_for_path(&args.device_path);
    let final_cache_dir = args.out.join(&book_id);
    let cache_dir = args.out.join(format!("{book_id}.TMP"));
    let backup_cache_dir = args.out.join(format!("{book_id}.BAK"));
    fs::create_dir_all(&args.out).map_err(|err| format!("create output directory: {err}"))?;
    remove_dir_if_present(&cache_dir, "remove stale staging cache")?;
    remove_dir_if_present(&backup_cache_dir, "remove stale backup cache")?;
    fs::create_dir_all(&cache_dir)
        .map_err(|err| format!("create staging cache directory: {err}"))?;

    let mut max_page_bytes = 0usize;
    let mut max_font_bytes = 0usize;
    let mut latin_pages = 0usize;
    let mut devanagari_pages = 0usize;
    let mut gujarati_pages = 0usize;

    for (index, page) in layout.pages.iter().enumerate() {
        if page.glyphs.len() > MAX_PAGE_GLYPHS {
            return Err(format!(
                "page {index} has {} glyphs; firmware limit is {MAX_PAGE_GLYPHS}",
                page.glyphs.len()
            ));
        }
        let page_name = page_local_file_name('P', index, "VRN")?;
        let page_data = build_vrun(page, args.page_width as u16, args.page_height as u16)?;
        if page_data.len() > MAX_PAGE_BYTES {
            return Err(format!(
                "page {page_name} is {} bytes; firmware limit is {MAX_PAGE_BYTES}",
                page_data.len()
            ));
        }
        max_page_bytes = max_page_bytes.max(page_data.len());
        fs::write(cache_dir.join(&page_name), &page_data)
            .map_err(|err| format!("write {page_name}: {err}"))?;

        let latin_ids = write_page_font(
            &cache_dir,
            page,
            index,
            FONT_LATIN,
            'L',
            SCRIPT_LATIN,
            &latin_font,
            args.latin_size,
            &mut max_font_bytes,
        )?;
        if !latin_ids.is_empty() {
            latin_pages += 1;
        }
        let devanagari_ids = write_page_font(
            &cache_dir,
            page,
            index,
            FONT_DEVANAGARI,
            'D',
            SCRIPT_DEVANAGARI,
            &devanagari_font,
            args.devanagari_size,
            &mut max_font_bytes,
        )?;
        if !devanagari_ids.is_empty() {
            devanagari_pages += 1;
        }
        let gujarati_ids = glyph_ids_for_font(page, FONT_GUJARATI);
        if !gujarati_ids.is_empty() {
            let font = gujarati_font.as_ref().ok_or_else(|| {
                "book contains Gujarati text; pass --gujarati-font <TTF>".to_string()
            })?;
            write_page_font_ids(
                &cache_dir,
                index,
                'G',
                SCRIPT_GUJARATI,
                font,
                args.gujarati_size,
                &gujarati_ids,
                &mut max_font_bytes,
            )?;
            gujarati_pages += 1;
        }
        validate_page_references(page, &latin_ids, &devanagari_ids, &gujarati_ids)?;
    }

    fs::write(
        cache_dir.join("META.TXT"),
        format!(
            "book_id={book_id}\nsource=/{}\ntitle={}\ncache_format={CACHE_FORMAT_PAGE_LOCAL}\npage_count={}\npage_name_scheme=base36-local-v1\n",
            meta_value(args.device_path.trim_start_matches('/')),
            meta_value(&args.title),
            layout.pages.len()
        ),
    )
    .map_err(|err| format!("write META.TXT: {err}"))?;

    replace_cache_directory(&cache_dir, &final_cache_dir, &backup_cache_dir)?;

    println!("book_id={book_id}");
    println!("prepared cache: {}", final_cache_dir.display());
    println!("cache_format={CACHE_FORMAT_PAGE_LOCAL}");
    println!("pages={}", layout.pages.len());
    println!("latin_pages={latin_pages}");
    println!("devanagari_pages={devanagari_pages}");
    println!("gujarati_pages={gujarati_pages}");
    println!("resource_split_pages={}", layout.resource_split_pages);
    println!("max_page_bytes={max_page_bytes}");
    println!("max_font_bytes={max_font_bytes}");
    Ok(())
}

struct LayoutResult {
    pages: Vec<PreparedPage>,
    resource_split_pages: usize,
}

fn layout_text(
    args: &Args,
    text: &str,
    latin_face: &Face<'_>,
    devanagari_face: &Face<'_>,
    gujarati_face: Option<&Face<'_>>,
    latin_font: &Font,
    devanagari_font: &Font,
    gujarati_font: Option<&Font>,
) -> Result<LayoutResult, String> {
    let line_height = args.line_height.unwrap_or_else(|| {
        (args
            .latin_size
            .max(args.devanagari_size)
            .max(args.gujarati_size)
            .ceil() as i16)
            .saturating_add(8)
    });
    let baseline_start = args.margin_y + line_height;
    let mut pages = vec![PreparedPage { glyphs: Vec::new() }];
    let mut page_index = 0usize;
    let mut page_budget = PageBudget::default();
    let mut resource_split_pages = 0usize;
    let mut x = args.margin_x;
    let mut baseline = baseline_start;

    for line in text.split('\n') {
        for run in split_script_runs(line) {
            let shaped = match run.script {
                ScriptKind::Latin => shape_run(
                    run.text,
                    ScriptKind::Latin,
                    latin_face,
                    args.latin_size,
                    FONT_LATIN,
                )?,
                ScriptKind::Devanagari => shape_run(
                    run.text,
                    ScriptKind::Devanagari,
                    devanagari_face,
                    args.devanagari_size,
                    FONT_DEVANAGARI,
                )?,
                ScriptKind::Gujarati => {
                    let face = gujarati_face.ok_or_else(|| {
                        "book contains Gujarati text; pass --gujarati-font <TTF>".to_string()
                    })?;
                    shape_run(
                        run.text,
                        ScriptKind::Gujarati,
                        face,
                        args.gujarati_size,
                        FONT_GUJARATI,
                    )?
                }
            };
            let mut cluster_start = 0usize;
            while cluster_start < shaped.len() {
                let cluster_id = shaped[cluster_start].cluster;
                let mut cluster_end = cluster_start + 1;
                while cluster_end < shaped.len() && shaped[cluster_end].cluster == cluster_id {
                    cluster_end += 1;
                }
                let measured = measure_cluster(
                    &shaped[cluster_start..cluster_end],
                    args,
                    latin_font,
                    devanagari_font,
                    gujarati_font,
                )?;
                let cluster_advance = measured.iter().fold(0i16, |advance, glyph| {
                    advance.saturating_add(glyph.shaped.advance_x)
                });
                if x.saturating_add(cluster_advance) > args.page_width && x > args.margin_x {
                    if next_line(
                        &mut pages,
                        &mut page_index,
                        &mut x,
                        &mut baseline,
                        args,
                        line_height,
                        baseline_start,
                    ) {
                        page_budget = PageBudget::default();
                    }
                }

                let resources: Vec<_> = measured.iter().map(|glyph| glyph.resource).collect();
                if !page_budget.add(&resources)? {
                    if pages[page_index].glyphs.is_empty() {
                        return Err(format!(
                            "single shaped cluster at byte {} cannot fit the firmware page budget",
                            cluster_id
                        ));
                    }
                    next_page(
                        &mut pages,
                        &mut page_index,
                        &mut x,
                        &mut baseline,
                        args,
                        baseline_start,
                    );
                    resource_split_pages += 1;
                    page_budget = PageBudget::default();
                    if !page_budget.add(&resources)? {
                        return Err(format!(
                            "single shaped cluster at byte {} cannot fit the firmware page budget",
                            cluster_id
                        ));
                    }
                }

                for glyph in measured {
                    let draw_x = x
                        .saturating_add(glyph.shaped.x_offset)
                        .saturating_add(glyph.metrics.xmin as i16);
                    let draw_y = baseline
                        .saturating_sub(glyph.metrics.height as i16)
                        .saturating_sub(glyph.metrics.ymin as i16)
                        .saturating_add(glyph.shaped.y_offset);
                    pages[page_index].glyphs.push(PositionedGlyph {
                        font_id: glyph.shaped.font_id,
                        glyph_id: glyph.shaped.glyph_id,
                        x: draw_x,
                        y: draw_y,
                        advance_x: glyph.shaped.advance_x,
                        advance_y: glyph.shaped.advance_y,
                        cluster: glyph.shaped.cluster,
                    });
                    x = x.saturating_add(glyph.shaped.advance_x);
                }
                cluster_start = cluster_end;
            }
        }
        if next_line(
            &mut pages,
            &mut page_index,
            &mut x,
            &mut baseline,
            args,
            line_height,
            baseline_start,
        ) {
            page_budget = PageBudget::default();
        }
    }

    Ok(LayoutResult {
        pages,
        resource_split_pages,
    })
}

fn measure_cluster(
    shaped: &[ShapedGlyph],
    args: &Args,
    latin_font: &Font,
    devanagari_font: &Font,
    gujarati_font: Option<&Font>,
) -> Result<Vec<MeasuredGlyph>, String> {
    let mut out = Vec::with_capacity(shaped.len());
    for glyph in shaped {
        let (metrics, _) = font_metrics_for(
            glyph.font_id,
            glyph.glyph_id,
            args,
            latin_font,
            devanagari_font,
            gujarati_font,
        )?;
        out.push(MeasuredGlyph {
            shaped: *glyph,
            resource: GlyphResource {
                font_id: glyph.font_id,
                glyph_id: glyph.glyph_id,
                bitmap_bytes: one_bpp_bitmap_len(&metrics),
            },
            metrics,
        });
    }
    Ok(out)
}

fn one_bpp_bitmap_len(metrics: &Metrics) -> usize {
    metrics.width.div_ceil(8).saturating_mul(metrics.height)
}

fn font_budget_slot(font_id: u32) -> Result<usize, String> {
    match font_id {
        FONT_LATIN => Ok(0),
        FONT_DEVANAGARI => Ok(1),
        FONT_GUJARATI => Ok(2),
        _ => Err(format!("unknown font slot: {font_id}")),
    }
}

fn font_metrics_for(
    font_id: u32,
    glyph_id: u32,
    args: &Args,
    latin_font: &Font,
    devanagari_font: &Font,
    gujarati_font: Option<&Font>,
) -> Result<(Metrics, Vec<u8>), String> {
    let (font, size) = match font_id {
        FONT_LATIN => (latin_font, args.latin_size),
        FONT_DEVANAGARI => (devanagari_font, args.devanagari_size),
        FONT_GUJARATI => (
            gujarati_font.ok_or_else(|| {
                "book contains Gujarati text; pass --gujarati-font <TTF>".to_string()
            })?,
            args.gujarati_size,
        ),
        _ => return Err(format!("unknown font slot: {font_id}")),
    };
    rasterize_one(font, size, glyph_id)
}

fn next_line(
    pages: &mut Vec<PreparedPage>,
    page_index: &mut usize,
    x: &mut i16,
    baseline: &mut i16,
    args: &Args,
    line_height: i16,
    baseline_start: i16,
) -> bool {
    *x = args.margin_x;
    *baseline = baseline.saturating_add(line_height);
    if *baseline + line_height > args.page_height {
        next_page(pages, page_index, x, baseline, args, baseline_start);
        true
    } else {
        false
    }
}

fn next_page(
    pages: &mut Vec<PreparedPage>,
    page_index: &mut usize,
    x: &mut i16,
    baseline: &mut i16,
    args: &Args,
    baseline_start: i16,
) {
    pages.push(PreparedPage { glyphs: Vec::new() });
    *page_index += 1;
    *x = args.margin_x;
    *baseline = baseline_start;
}

fn shape_run(
    text: &str,
    script_kind: ScriptKind,
    face: &Face<'_>,
    px: f32,
    font_id: u32,
) -> Result<Vec<ShapedGlyph>, String> {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(Direction::LeftToRight);
    buffer.set_script(match script_kind {
        ScriptKind::Latin => script::LATIN,
        ScriptKind::Devanagari => script::DEVANAGARI,
        ScriptKind::Gujarati => script::GUJARATI,
    });
    let shaped = rustybuzz::shape(face, &[], buffer);
    let scale = px / face.units_per_em() as f32;
    let infos = shaped.glyph_infos();
    let positions = shaped.glyph_positions();
    let mut out = Vec::with_capacity(infos.len());
    for (info, pos) in infos.iter().zip(positions.iter()) {
        out.push(ShapedGlyph {
            font_id,
            glyph_id: info.glyph_id,
            x_offset: scale_i32(pos.x_offset, scale),
            y_offset: -scale_i32(pos.y_offset, scale),
            advance_x: scale_i32(pos.x_advance, scale).max(0),
            advance_y: scale_i32(pos.y_advance, scale),
            cluster: info.cluster,
        });
    }
    Ok(out)
}

fn split_script_runs(input: &str) -> Vec<TextRun<'_>> {
    let mut runs = Vec::new();
    let mut start = 0usize;
    let mut current = None;
    for (index, ch) in input.char_indices() {
        let script = script_for_char(ch);
        if let Some(active) = current {
            if active != script {
                runs.push(TextRun {
                    script: active,
                    text: &input[start..index],
                });
                start = index;
                current = Some(script);
            }
        } else {
            current = Some(script);
            start = index;
        }
    }
    if let Some(script) = current {
        runs.push(TextRun {
            script,
            text: &input[start..],
        });
    }
    runs
}

fn script_for_char(ch: char) -> ScriptKind {
    if is_devanagari(ch) {
        ScriptKind::Devanagari
    } else if is_gujarati(ch) {
        ScriptKind::Gujarati
    } else {
        ScriptKind::Latin
    }
}

fn is_devanagari(ch: char) -> bool {
    matches!(
        ch,
        '\u{0900}'..='\u{097F}'
            | '\u{1CD0}'..='\u{1CFF}'
            | '\u{A8E0}'..='\u{A8FF}'
            | '\u{11B00}'..='\u{11B5F}'
    )
}

fn is_gujarati(ch: char) -> bool {
    ('\u{0A80}'..='\u{0AFF}').contains(&ch)
}

fn rasterize_used_glyphs<I>(font: &Font, px: f32, glyph_ids: I) -> Result<Vec<GlyphAsset>, String>
where
    I: Iterator<Item = u32>,
{
    let mut assets = Vec::new();
    for glyph_id in glyph_ids {
        let (metrics, grayscale) = rasterize_one(font, px, glyph_id)?;
        let (bitmap, row_stride) = coverage_to_one_bpp(&grayscale, metrics.width, metrics.height);
        assets.push(GlyphAsset {
            glyph_id,
            advance_x: metrics.advance_width.round() as i16,
            bearing_x: metrics.xmin as i16,
            bearing_y: metrics.ymin as i16,
            width: metrics.width as u16,
            height: metrics.height as u16,
            row_stride,
            bitmap,
        });
    }
    Ok(assets)
}

fn rasterize_one(font: &Font, px: f32, glyph_id: u32) -> Result<(Metrics, Vec<u8>), String> {
    let glyph_index =
        u16::try_from(glyph_id).map_err(|_| format!("glyph id too large: {glyph_id}"))?;
    Ok(font.rasterize_indexed(glyph_index, px))
}

fn coverage_to_one_bpp(grayscale: &[u8], width: usize, height: usize) -> (Vec<u8>, u16) {
    let row_stride = width.div_ceil(8);
    let mut out = vec![0u8; row_stride * height];
    for y in 0..height {
        for x in 0..width {
            if grayscale[y * width + x] >= 96 {
                out[y * row_stride + x / 8] |= 1 << (7 - (x & 7));
            }
        }
    }
    (out, row_stride as u16)
}

fn glyph_ids_for_font(page: &PreparedPage, font_id: u32) -> BTreeSet<u32> {
    page.glyphs
        .iter()
        .filter(|glyph| glyph.font_id == font_id)
        .map(|glyph| glyph.glyph_id)
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn write_page_font(
    cache_dir: &Path,
    page: &PreparedPage,
    page_index: usize,
    font_id: u32,
    prefix: char,
    script_code: u16,
    font: &Font,
    size: f32,
    max_font_bytes: &mut usize,
) -> Result<BTreeSet<u32>, String> {
    let ids = glyph_ids_for_font(page, font_id);
    if !ids.is_empty() {
        write_page_font_ids(
            cache_dir,
            page_index,
            prefix,
            script_code,
            font,
            size,
            &ids,
            max_font_bytes,
        )?;
    }
    Ok(ids)
}

#[allow(clippy::too_many_arguments)]
fn write_page_font_ids(
    cache_dir: &Path,
    page_index: usize,
    prefix: char,
    script_code: u16,
    font: &Font,
    size: f32,
    ids: &BTreeSet<u32>,
    max_font_bytes: &mut usize,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let assets = rasterize_used_glyphs(font, size, ids.iter().copied())?;
    let vfnt = build_vfnt(script_code, size, &assets)?;
    validate_vfnt(&vfnt)?;
    if vfnt.len() > MAX_FONT_BYTES {
        return Err(format!(
            "page-local font for page {page_index} is {} bytes; firmware limit is {MAX_FONT_BYTES}",
            vfnt.len()
        ));
    }
    *max_font_bytes = (*max_font_bytes).max(vfnt.len());
    let file_name = page_local_file_name(prefix, page_index, "VFN")?;
    fs::write(cache_dir.join(&file_name), &vfnt)
        .map_err(|err| format!("write {file_name}: {err}"))?;
    Ok(())
}

fn page_local_file_name(prefix: char, page: usize, extension: &str) -> Result<String, String> {
    if !prefix.is_ascii_alphanumeric()
        || extension.len() != 3
        || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || page >= PAGE_LOCAL_CAPACITY
    {
        return Err("invalid page-local cache file name".to_string());
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
    let stem =
        String::from_utf8(digits.to_vec()).map_err(|_| "base36 encode failed".to_string())?;
    Ok(format!("{prefix}{stem}.{extension}"))
}

fn meta_value(value: &str) -> String {
    value.replace('\r', " ").replace('\n', " ")
}

fn remove_dir_if_present(path: &Path, context: &str) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path).map_err(|err| format!("{context}: {err}"))?;
    }
    Ok(())
}

fn replace_cache_directory(staged: &Path, final_dir: &Path, backup: &Path) -> Result<(), String> {
    let had_previous = final_dir.exists();
    if had_previous {
        fs::rename(final_dir, backup)
            .map_err(|err| format!("move previous cache to backup: {err}"))?;
    }
    match fs::rename(staged, final_dir) {
        Ok(()) => {
            remove_dir_if_present(backup, "remove replaced cache backup")?;
            Ok(())
        }
        Err(err) => {
            if had_previous && backup.exists() {
                let _ = fs::rename(backup, final_dir);
            }
            Err(format!("activate staged cache: {err}"))
        }
    }
}

fn build_vfnt(script_code: u16, pixel_size: f32, glyphs: &[GlyphAsset]) -> Result<Vec<u8>, String> {
    if glyphs.is_empty() {
        return Err("font has no used glyphs".to_string());
    }
    let mut sorted = glyphs.to_vec();
    sorted.sort_by_key(|glyph| glyph.glyph_id);

    let metrics_offset = VFNT_HEADER_LEN;
    let bitmap_index_offset = metrics_offset + sorted.len() * VFNT_METRICS_LEN;
    let bitmap_data_offset = bitmap_index_offset + sorted.len() * VFNT_BITMAP_LEN;
    let mut metrics = Vec::new();
    let mut bitmap_index = Vec::new();
    let mut bitmap_data = Vec::new();
    for glyph in &sorted {
        let offset = bitmap_data.len() as u32;
        bitmap_data.extend_from_slice(&glyph.bitmap);
        put_u32(&mut metrics, glyph.glyph_id);
        put_i16(&mut metrics, glyph.advance_x);
        put_i16(&mut metrics, 0);
        put_i16(&mut metrics, glyph.bearing_x);
        put_i16(&mut metrics, glyph.bearing_y);
        put_u16(&mut metrics, glyph.width);
        put_u16(&mut metrics, glyph.height);

        put_u32(&mut bitmap_index, glyph.glyph_id);
        put_u32(&mut bitmap_index, offset);
        put_u32(&mut bitmap_index, glyph.bitmap.len() as u32);
        put_u16(&mut bitmap_index, glyph.row_stride);
        put_u16(&mut bitmap_index, 0);
    }

    let line_height = (pixel_size.ceil() as u16).saturating_add(8);
    let ascent = pixel_size.ceil() as i16;
    let descent = -((line_height as i16).saturating_sub(ascent));
    let mut out = Vec::new();
    out.extend_from_slice(VFNT_MAGIC);
    put_u16(&mut out, VFNT_VERSION);
    put_u16(&mut out, VFNT_HEADER_LEN as u16);
    put_u32(&mut out, 0);
    put_u16(&mut out, pixel_size.round() as u16);
    put_u16(&mut out, line_height);
    put_i16(&mut out, ascent);
    put_i16(&mut out, descent);
    put_u32(&mut out, sorted.len() as u32);
    put_u32(&mut out, metrics_offset as u32);
    put_u32(&mut out, bitmap_index_offset as u32);
    put_u32(&mut out, bitmap_data_offset as u32);
    put_u32(&mut out, bitmap_data.len() as u32);
    put_u16(&mut out, script_code);
    put_u16(&mut out, VFNT_ONE_BPP);
    out.extend(metrics);
    out.extend(bitmap_index);
    out.extend(bitmap_data);
    Ok(out)
}

fn build_vrun(page: &PreparedPage, width: u16, height: u16) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(VRUN_HEADER_LEN + page.glyphs.len() * VRUN_RECORD_LEN);
    out.extend_from_slice(VRUN_MAGIC);
    put_u16(&mut out, VRUN_VERSION);
    put_u16(&mut out, VRUN_HEADER_LEN as u16);
    put_u32(&mut out, page.glyphs.len() as u32);
    put_u16(&mut out, width);
    put_u16(&mut out, height);
    put_u32(&mut out, 0);
    for glyph in &page.glyphs {
        put_u32(&mut out, glyph.font_id);
        put_u32(&mut out, glyph.glyph_id);
        put_i16(&mut out, glyph.x);
        put_i16(&mut out, glyph.y);
        put_i16(&mut out, glyph.advance_x);
        put_i16(&mut out, glyph.advance_y);
        put_u32(&mut out, glyph.cluster);
    }
    Ok(out)
}

fn validate_page_references(
    page: &PreparedPage,
    latin_ids: &BTreeSet<u32>,
    devanagari_ids: &BTreeSet<u32>,
    gujarati_ids: &BTreeSet<u32>,
) -> Result<(), String> {
    for glyph in &page.glyphs {
        match glyph.font_id {
            FONT_LATIN if latin_ids.contains(&glyph.glyph_id) => {}
            FONT_DEVANAGARI if devanagari_ids.contains(&glyph.glyph_id) => {}
            FONT_GUJARATI if gujarati_ids.contains(&glyph.glyph_id) => {}
            FONT_LATIN | FONT_DEVANAGARI | FONT_GUJARATI => {
                return Err(format!("VRN references missing glyph {}", glyph.glyph_id));
            }
            _ => {
                return Err(format!(
                    "VRN references unknown font slot {}",
                    glyph.font_id
                ))
            }
        }
    }
    Ok(())
}

fn validate_vfnt(data: &[u8]) -> Result<(), String> {
    if data.len() < VFNT_HEADER_LEN {
        return Err("VFNT header is truncated".to_string());
    }
    if &data[0..4] != VFNT_MAGIC {
        return Err("VFNT magic is invalid".to_string());
    }
    if read_u16(data, 4)? != VFNT_VERSION {
        return Err("VFNT version is unsupported".to_string());
    }
    if read_u16(data, 42)? != VFNT_ONE_BPP {
        return Err("VFNT bitmap format is unsupported".to_string());
    }
    let count = read_u32(data, 20)? as usize;
    let metrics_offset = read_u32(data, 24)? as usize;
    let bitmap_index_offset = read_u32(data, 28)? as usize;
    let bitmap_data_offset = read_u32(data, 32)? as usize;
    let bitmap_data_len = read_u32(data, 36)? as usize;
    checked_range(data.len(), metrics_offset, count * VFNT_METRICS_LEN)?;
    checked_range(data.len(), bitmap_index_offset, count * VFNT_BITMAP_LEN)?;
    checked_range(data.len(), bitmap_data_offset, bitmap_data_len)?;
    for index in 0..count {
        let off = bitmap_index_offset + index * VFNT_BITMAP_LEN;
        let bitmap_offset = read_u32(data, off + 4)? as usize;
        let bitmap_len = read_u32(data, off + 8)? as usize;
        checked_range(bitmap_data_len, bitmap_offset, bitmap_len)?;
    }
    Ok(())
}

#[cfg(test)]
fn vfnt_glyph_ids(data: &[u8]) -> Result<BTreeSet<u32>, String> {
    validate_vfnt(data)?;
    let count = read_u32(data, 20)? as usize;
    let metrics_offset = read_u32(data, 24)? as usize;
    let mut ids = BTreeSet::new();
    for index in 0..count {
        ids.insert(read_u32(data, metrics_offset + index * VFNT_METRICS_LEN)?);
    }
    Ok(ids)
}

fn checked_range(total: usize, start: usize, len: usize) -> Result<(), String> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| "range overflows".to_string())?;
    if start <= total && end <= total {
        Ok(())
    } else {
        Err("range is outside buffer".to_string())
    }
}

fn book_folder_for_path(path: &str) -> String {
    let mut value: u32 = 0x811C9DC5;
    for byte in normalized_path_key(path).bytes() {
        value ^= u32::from(byte);
        value = value.wrapping_mul(0x01000193);
    }
    format!("{value:08X}")
}

fn normalized_path_key(path: &str) -> String {
    path.chars()
        .map(|ch| if ch == '\\' { '/' } else { ch })
        .flat_map(char::to_lowercase)
        .collect()
}

fn read_required_file(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("{label} missing or unreadable: {err}"))
}

fn parse_args() -> Result<Args, String> {
    let mut values = BTreeMap::new();
    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--help" || arg == "-h" {
            return Err(usage());
        }
        if !arg.starts_with("--") {
            return Err(format!("unexpected argument: {arg}\n{}", usage()));
        }
        let Some(value) = iter.next() else {
            return Err(format!("missing value for {arg}"));
        };
        values.insert(arg, value);
    }
    let book = required_path(&values, "--book")?;
    let title = values
        .get("--title")
        .cloned()
        .unwrap_or_else(|| "Prepared TXT Smoke".to_string());
    Ok(Args {
        book,
        device_path: required_string(&values, "--device-path")?,
        latin_font: required_path(&values, "--latin-font")?,
        devanagari_font: required_path(&values, "--devanagari-font")?,
        gujarati_font: optional_path(&values, "--gujarati-font"),
        out: required_path(&values, "--out")?,
        title,
        latin_size: optional_f32(&values, "--latin-size", 18.0)?,
        devanagari_size: optional_f32(&values, "--devanagari-size", 22.0)?,
        gujarati_size: optional_f32(&values, "--gujarati-size", 22.0)?,
        line_height: optional_i16(&values, "--line-height")?,
        page_width: optional_i16(&values, "--page-width")?.unwrap_or(464),
        page_height: optional_i16(&values, "--page-height")?.unwrap_or(730),
        margin_x: optional_i16(&values, "--margin-x")?.unwrap_or(0),
        margin_y: optional_i16(&values, "--margin-y")?.unwrap_or(4),
    })
}

fn usage() -> String {
    "usage: prepared-txt-real-vfnt --book <TXT> --device-path <PATH> --latin-font <TTF> --devanagari-font <TTF> [--gujarati-font <TTF>] --out <FCACHE>".to_string()
}

fn required_path(values: &BTreeMap<String, String>, key: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(required_string(values, key)?))
}

fn optional_path(values: &BTreeMap<String, String>, key: &str) -> Option<PathBuf> {
    values.get(key).map(|value| PathBuf::from(value.as_str()))
}

fn required_string(values: &BTreeMap<String, String>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing required argument {key}"))
}

fn optional_f32(values: &BTreeMap<String, String>, key: &str, default: f32) -> Result<f32, String> {
    match values.get(key) {
        Some(value) => value.parse().map_err(|_| format!("invalid {key}: {value}")),
        None => Ok(default),
    }
}

fn optional_i16(values: &BTreeMap<String, String>, key: &str) -> Result<Option<i16>, String> {
    values
        .get(key)
        .map(|value| value.parse().map_err(|_| format!("invalid {key}: {value}")))
        .transpose()
}

fn scale_i32(value: i32, scale: f32) -> i16 {
    (value as f32 * scale)
        .round()
        .clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_i16(out: &mut Vec<u8>, value: i16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| "read past end".to_string())?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| "read past end".to_string())?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_existing_reader_book_id_policy() {
        assert_eq!(book_folder_for_path("MIXED.TXT"), "5E3BBD2E");
        assert_eq!(
            book_folder_for_path("Books\\MIXED.TXT"),
            book_folder_for_path("books/mixed.txt")
        );
    }

    #[test]
    fn splits_latin_and_devanagari_runs() {
        let runs = split_script_runs("Om ॐ नमः");
        assert_eq!(runs[0].script, ScriptKind::Latin);
        assert_eq!(runs[1].script, ScriptKind::Devanagari);
    }

    #[test]
    fn splits_devanagari_and_gujarati_runs() {
        let runs = split_script_runs("नमः ગુજરાતી");
        assert!(runs.iter().any(|run| run.script == ScriptKind::Devanagari));
        assert!(runs.iter().any(|run| run.script == ScriptKind::Gujarati));
    }

    #[test]
    fn classifies_vedic_marks_as_devanagari() {
        assert_eq!(script_for_char('\u{1CD0}'), ScriptKind::Devanagari);
        assert_eq!(script_for_char('\u{A8E0}'), ScriptKind::Devanagari);
    }

    #[test]
    fn generates_page_local_fat_83_names() {
        assert_eq!(page_local_file_name('P', 0, "VRN").unwrap(), "P00000.VRN");
        assert_eq!(page_local_file_name('D', 35, "VFN").unwrap(), "D0000Z.VFN");
        assert_eq!(page_local_file_name('G', 36, "VFN").unwrap(), "G00010.VFN");
    }

    #[test]
    fn generated_vfnt_contains_non_empty_bitmap() {
        let glyph = GlyphAsset {
            glyph_id: 42,
            advance_x: 5,
            bearing_x: 0,
            bearing_y: 0,
            width: 3,
            height: 2,
            row_stride: 1,
            bitmap: vec![0b1110_0000, 0b1010_0000],
        };
        let data = build_vfnt(SCRIPT_LATIN, 18.0, &[glyph]).unwrap();
        validate_vfnt(&data).unwrap();
        assert!(vfnt_glyph_ids(&data).unwrap().contains(&42));
        assert!(data.ends_with(&[0b1110_0000, 0b1010_0000]));
    }

    #[test]
    fn page_budget_splits_before_firmware_glyph_limit() {
        let resource = GlyphResource {
            font_id: FONT_LATIN,
            glyph_id: 1,
            bitmap_bytes: 1,
        };
        let mut budget = PageBudget::default();
        for _ in 0..MAX_PAGE_GLYPHS {
            assert!(budget.add(&[resource]).unwrap());
        }
        assert!(!budget.add(&[resource]).unwrap());
        assert_eq!(budget.glyph_count, MAX_PAGE_GLYPHS);
    }

    #[test]
    fn page_budget_counts_repeated_bitmap_once() {
        let resource = GlyphResource {
            font_id: FONT_DEVANAGARI,
            glyph_id: 42,
            bitmap_bytes: 9,
        };
        let mut budget = PageBudget::default();
        assert!(budget.add(&[resource]).unwrap());
        let font_bytes = budget.font_bytes[1];
        assert!(budget.add(&[resource]).unwrap());
        assert_eq!(budget.font_bytes[1], font_bytes);
        assert_eq!(budget.glyph_count, 2);
    }

    #[test]
    fn page_budget_rejects_page_local_font_overflow() {
        let resource = GlyphResource {
            font_id: FONT_GUJARATI,
            glyph_id: 7,
            bitmap_bytes: MAX_FONT_BYTES,
        };
        let mut budget = PageBudget::default();
        assert!(!budget.add(&[resource]).unwrap());
        assert_eq!(budget.glyph_count, 0);
    }

    #[test]
    fn generated_vrun_references_multiple_positioned_glyphs() {
        let page = PreparedPage {
            glyphs: vec![
                PositionedGlyph {
                    font_id: FONT_LATIN,
                    glyph_id: 1,
                    x: 0,
                    y: 0,
                    advance_x: 6,
                    advance_y: 0,
                    cluster: 0,
                },
                PositionedGlyph {
                    font_id: FONT_DEVANAGARI,
                    glyph_id: 2,
                    x: 8,
                    y: 1,
                    advance_x: 10,
                    advance_y: 0,
                    cluster: 1,
                },
            ],
        };
        let data = build_vrun(&page, 464, 730).unwrap();
        assert_eq!(&data[0..4], VRUN_MAGIC);
        assert_eq!(read_u32(&data, 8).unwrap(), 2);
        assert_eq!(data.len(), VRUN_HEADER_LEN + 2 * VRUN_RECORD_LEN);
    }

    #[test]
    fn rejects_missing_font_files() {
        let result = read_required_file(Path::new("/definitely/missing/font.ttf"), "font");
        assert!(result.is_err());
    }
}
