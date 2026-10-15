//! A tiny PDF drawing surface (top-left origin in points, `Cell` text placement,
//! word wrap, char spacing, underline) used to lay out the document forms so that
//! they match the official application's output.

use std::collections::BTreeMap;

use chrono::{Datelike, Local, Offset, Timelike};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use subsetter::GlyphRemapper;
use ttf_parser::{Face, GlyphId};

use crate::ExportError;

/// A4 page size in points.
pub const PAGE_W: f64 = 595.0;
pub const PAGE_H: f64 = 842.0;

static FONT_REGULAR: &[u8] = include_bytes!("../../../../assets/fonts/liberationSansRegular.ttf");
static FONT_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/liberationSansBold.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Regular,
    Bold,
}

struct FontState {
    data: &'static [u8],
    face: Face<'static>,
    base_name: &'static str,
    remapper: GlyphRemapper,
    /// new glyph id -> (unicode char, width in 1/1000 em)
    used: BTreeMap<u16, (char, u32)>,
    upem: i64,
    typo_ascender: f64,
    win_ascent: f64,
    win_descent: f64,
    underline_position: f64,
    underline_thickness: f64,
}

impl FontState {
    fn new(data: &'static [u8], base_name: &'static str) -> Result<Self, ExportError> {
        let face =
            Face::parse(data, 0).map_err(|e| ExportError::Pdf(format!("parsing font: {e}")))?;
        let upem = face.units_per_em() as i64;
        let os2 = face.tables().os2;
        let typo_ascender = face.typographic_ascender().unwrap_or(face.ascender()) as f64;
        let (win_ascent, win_descent) = match os2 {
            Some(t) => (
                t.windows_ascender() as f64,
                (t.windows_descender() as f64).abs(),
            ),
            None => (face.ascender() as f64, (face.descender() as f64).abs()),
        };
        let (underline_position, underline_thickness) = face
            .underline_metrics()
            .map(|m| (m.position as f64, m.thickness as f64))
            .unwrap_or((-150.0, 100.0));
        Ok(Self {
            data,
            face,
            base_name,
            remapper: GlyphRemapper::new(),
            used: BTreeMap::new(),
            upem,
            typo_ascender,
            win_ascent,
            win_descent,
            underline_position,
            underline_thickness,
        })
    }

    /// Glyph width in PDF units (1/1000 em), truncated to an integer.
    fn pdf_width(&self, gid: GlyphId) -> u32 {
        let adv = self.face.glyph_hor_advance(gid).unwrap_or(0) as i64;
        (adv * 1000 / self.upem) as u32
    }
}

/// Converts font units to PDF units (1/1000 em) with integer rounding.
fn ttf_to_pdf_unit(n: i64, upem: i64) -> i64 {
    if n < 0 {
        -((-1000 * n) / upem)
    } else {
        (n / upem) * 1000 + ((n % upem) * 1000) / upem
    }
}

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub kind: ImageKind,
}

pub enum ImageKind {
    /// Raw JPEG data (DCTDecode) with number of color components.
    Jpeg { data: Vec<u8>, components: u8 },
    /// 8-bit samples, zlib compressed; `gray` = 1 component, else RGB; optional 8-bit alpha (compressed).
    Raw {
        data: Vec<u8>,
        gray: bool,
        alpha: Option<Vec<u8>>,
    },
}

impl Image {
    /// Decode an image (JPEG pass-through, PNG decoded). Returns `None` for unsupported data.
    pub fn from_bytes(bytes: &[u8]) -> Option<Image> {
        if bytes.starts_with(&[0xFF, 0xD8]) {
            return jpeg_info(bytes).map(|(width, height, components)| Image {
                width,
                height,
                kind: ImageKind::Jpeg {
                    data: bytes.to_vec(),
                    components,
                },
            });
        }
        if bytes.starts_with(b"\x89PNG") {
            return decode_png(bytes);
        }
        None
    }
}

/// Parse a JPEG SOF segment: (width, height, components).
fn jpeg_info(b: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = b[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        let is_sof = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_sof {
            let s = b.get(i + 4..i + 2 + len)?;
            if s.len() < 6 {
                return None;
            }
            let h = u16::from_be_bytes([s[1], s[2]]) as u32;
            let w = u16::from_be_bytes([s[3], s[4]]) as u32;
            let c = s[5];
            if w == 0 || h == 0 || !matches!(c, 1 | 3 | 4) {
                return None;
            }
            return Some((w, h, c));
        }
        i += 2 + len;
    }
    None
}

fn decode_png(bytes: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let (width, height) = (info.width, info.height);
    let (channels, has_alpha) = match info.color_type {
        png::ColorType::Grayscale => (1, false),
        png::ColorType::GrayscaleAlpha => (2, true),
        png::ColorType::Rgb => (3, false),
        png::ColorType::Rgba => (4, true),
        png::ColorType::Indexed => return None,
    };
    let color_n = if has_alpha { channels - 1 } else { channels };
    let mut color = Vec::with_capacity((width * height) as usize * color_n);
    let mut alpha = Vec::new();
    for px in buf.chunks_exact(channels) {
        color.extend_from_slice(&px[..color_n]);
        if has_alpha {
            alpha.push(px[channels - 1]);
        }
    }
    let alpha = if has_alpha && alpha.iter().any(|&a| a != 255) {
        Some(deflate(&alpha))
    } else {
        None
    };
    Some(Image {
        width,
        height,
        kind: ImageKind::Raw {
            data: deflate(&color),
            gray: color_n == 1,
            alpha,
        },
    })
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(data, 6)
}

/// `CoefUnderlinePosition` of the only underlined cells (vehicle PDF).
const UNDERLINE_COEF_POSITION: f64 = -2.0;

/// PDF document info.
pub struct Info {
    pub title: String,
    pub subject: String,
}

pub struct Canvas {
    content: Content,
    fonts: [FontState; 2],
    images: Vec<Image>,
    x: f64,
    y: f64,
    font: FontStyle,
    underline: bool,
    size: f64,
    char_spacing: f64,
}

impl Canvas {
    pub fn new() -> Result<Self, ExportError> {
        Ok(Self {
            content: Content::new(),
            fonts: [
                FontState::new(FONT_REGULAR, "LiberationSans")?,
                FontState::new(FONT_BOLD, "LiberationSans-Bold")?,
            ],
            images: Vec::new(),
            x: 0.0,
            y: 0.0,
            font: FontStyle::Regular,
            underline: false,
            size: 14.0,
            char_spacing: 0.0,
        })
    }

    fn font_state(&self) -> &FontState {
        &self.fonts[self.font as usize]
    }

    pub fn get_x(&self) -> f64 {
        self.x
    }
    pub fn get_y(&self) -> f64 {
        self.y
    }
    pub fn set_x(&mut self, x: f64) {
        self.x = x;
    }
    pub fn set_y(&mut self, y: f64) {
        self.y = y;
    }
    pub fn set_xy(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }

    /// Selects the font style and size; `underline` draws text with the regular face
    /// and an underline.
    pub fn set_font(&mut self, style: FontStyle, underline: bool, size: f64) {
        self.font = style;
        self.underline = underline;
        self.size = size;
    }
    pub fn set_font_size(&mut self, size: f64) {
        self.size = size;
    }
    pub fn set_char_spacing(&mut self, cs: f64) {
        self.char_spacing = cs;
    }

    pub fn set_line_width(&mut self, w: f64) {
        self.content.set_line_width(w as f32);
    }

    /// Straight line from (x1, y1) to (x2, y2).
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        self.content.save_state();
        self.content.move_to(x1 as f32, (PAGE_H - y1) as f32);
        self.content.line_to(x2 as f32, (PAGE_H - y2) as f32);
        self.content.stroke();
        self.content.restore_state();
    }

    /// Stroked rectangle with corners (x0, y0) and (x1, y1).
    pub fn rectangle_stroke(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.content.rect(
            x0 as f32,
            (PAGE_H - y1) as f32,
            (x1 - x0) as f32,
            (y1 - y0) as f32,
        );
        self.content.stroke();
    }

    /// Draws an image at (x, y) scaled to w x h.
    pub fn image(&mut self, img: Image, x: f64, y: f64, w: f64, h: f64) {
        let idx = self.images.len();
        self.images.push(img);
        let name = format!("Im{idx}");
        self.content.save_state();
        self.content.transform([
            w as f32,
            0.0,
            0.0,
            h as f32,
            x as f32,
            (PAGE_H - y - h) as f32,
        ]);
        self.content.x_object(Name(name.as_bytes()));
        self.content.restore_state();
    }

    /// Text width (no kerning, char spacing included).
    pub fn measure(&self, text: &str) -> f64 {
        let f = self.font_state();
        let mut sum: i64 = 0;
        for c in text.chars() {
            let Some(gid) = f.face.glyph_index(c) else {
                continue;
            };
            let width = f.pdf_width(gid) as i64;
            let space_in_units = (f.upem as f64 / self.size) * self.char_spacing;
            sum += width + ttf_to_pdf_unit(space_in_units as i64, f.upem);
        }
        sum as f64 * self.size / 1000.0
    }

    /// Text cell: text whose top-left is the current position; X advances.
    pub fn cell(&mut self, text: &str) {
        let width = self.measure(text);
        let size = self.size;
        let font_idx = self.font as usize;
        let mut encoded = Vec::with_capacity(text.len() * 2);
        {
            let f = &mut self.fonts[font_idx];
            for c in text.chars() {
                let Some(gid) = f.face.glyph_index(c) else {
                    continue;
                };
                let new = f.remapper.remap(gid.0);
                let w = f.pdf_width(gid);
                f.used.entry(new).or_insert((c, w));
                encoded.extend_from_slice(&new.to_be_bytes());
            }
        }
        let f = &self.fonts[font_idx];
        let ascender = f.typo_ascender * 1000.0 / f.upem as f64 * size / 1000.0;
        if !encoded.is_empty() {
            let font_name = format!("F{font_idx}");
            self.content.begin_text();
            self.content
                .set_font(Name(font_name.as_bytes()), size as f32);
            self.content.set_char_spacing(self.char_spacing as f32);
            self.content
                .next_line(self.x as f32, (PAGE_H - self.y - ascender) as f32);
            self.content.show(Str(&encoded));
            self.content.end_text();
        }
        if self.underline && width > 0.0 {
            // Underline with CoefUnderlinePosition -2 (the only use).
            let upem = f.upem as f64;
            let asc = f.win_ascent / upem * size;
            let desc = f.win_descent / upem * size;
            let leading = (asc + desc - size) / 2.0;
            let baseline = asc + leading;
            let pos = f.underline_position / upem * size * UNDERLINE_COEF_POSITION;
            let thick = f.underline_thickness / upem * size;
            let y = PAGE_H - self.y + pos - baseline;
            self.content
                .rect(self.x as f32, y as f32, width as f32, thick as f32);
            self.content.fill_nonzero();
        }
        self.x += width;
    }

    /// Word wrap to `width` (break on spaces, hard break on '\n').
    pub fn split_text_with_word_wrap(&self, text: &str, width: f64) -> Vec<String> {
        let runes: Vec<char> = text.chars().collect();
        let mut lines: Vec<String> = Vec::new();
        let mut line: Vec<char> = Vec::new();
        let n = runes.len() as isize;
        let mut i: isize = 0;
        while i < n {
            let r = runes[i as usize];
            let line_str: String = line.iter().collect();
            let line_width = self.measure(&line_str);
            let rune_width = self.measure(&r.to_string());
            if line_width + rune_width > width && r != '\n' {
                // performIndicatorSensitiveLineBreak
                let br = line.iter().rposition(|&c| c == ' ').filter(|&p| p > 0);
                if let Some(br) = br {
                    let diff = (line.len() - br) as isize;
                    line.truncate(br);
                    lines.push(line.iter().collect());
                    line.clear();
                    i -= diff;
                } else {
                    // performStrictLineBreak without separator
                    lines.push(line.iter().collect());
                    line.clear();
                    i -= 1;
                }
                i += 1;
                continue;
            }
            if r == '\n' {
                lines.push(line.iter().collect());
                line.clear();
                i += 1;
                continue;
            }
            if i == n - 1 {
                line.push(r);
                lines.push(line.iter().collect());
            }
            line.push(r);
            i += 1;
        }
        lines
    }

    /// Finish the document and return the PDF bytes.
    pub fn finish(self, info: Info) -> Result<Vec<u8>, ExportError> {
        let mut alloc = Ref::new(1);
        let catalog_id = alloc.bump();
        let pages_id = alloc.bump();
        let page_id = alloc.bump();
        let content_id = alloc.bump();
        let info_id = alloc.bump();

        let mut pdf = Pdf::new();
        pdf.catalog(catalog_id).pages(pages_id);
        pdf.pages(pages_id).kids([page_id]).count(1);

        let mut font_ids = Vec::new();
        for (idx, f) in self.fonts.iter().enumerate() {
            if f.used.is_empty() {
                continue;
            }
            let type0 = alloc.bump();
            let cid = alloc.bump();
            let descriptor = alloc.bump();
            let cmap_id = alloc.bump();
            let file_id = alloc.bump();
            let subset = subsetter::subset(f.data, 0, &f.remapper)
                .map_err(|e| ExportError::Pdf(format!("subsetting font: {e}")))?;
            let tag = subset_tag(&f.used);
            let base = format!("{tag}+{}", f.base_name);
            let system_info = SystemInfo {
                registry: Str(b"Adobe"),
                ordering: Str(b"Identity"),
                supplement: 0,
            };

            pdf.type0_font(type0)
                .base_font(Name(base.as_bytes()))
                .encoding_predefined(Name(b"Identity-H"))
                .descendant_font(cid)
                .to_unicode(cmap_id);

            let mut cid_font = pdf.cid_font(cid);
            cid_font
                .subtype(CidFontType::Type2)
                .base_font(Name(base.as_bytes()))
                .system_info(system_info)
                .font_descriptor(descriptor)
                .default_width(0.0)
                .cid_to_gid_map_predefined(Name(b"Identity"));
            {
                let mut widths = cid_font.widths();
                for (&g, &(_, w)) in &f.used {
                    widths.consecutive(g, [w as f32]);
                }
            }
            cid_font.finish();

            let scale = 1000.0 / f.upem as f32;
            let bb = f.face.global_bounding_box();
            let mut flags = FontFlags::NON_SYMBOLIC;
            if f.face.is_monospaced() {
                flags |= FontFlags::FIXED_PITCH;
            }
            pdf.font_descriptor(descriptor)
                .name(Name(base.as_bytes()))
                .flags(flags)
                .bbox(Rect::new(
                    bb.x_min as f32 * scale,
                    bb.y_min as f32 * scale,
                    bb.x_max as f32 * scale,
                    bb.y_max as f32 * scale,
                ))
                .italic_angle(f.face.italic_angle())
                .ascent(f.face.ascender() as f32 * scale)
                .descent(f.face.descender() as f32 * scale)
                .cap_height(f.face.capital_height().unwrap_or(f.face.ascender()) as f32 * scale)
                .stem_v(if idx == 1 { 140.0 } else { 80.0 })
                .font_file2(file_id);

            let mut cmap = UnicodeCmap::new(Name(b"Custom"), system_info);
            for (&g, &(c, _)) in &f.used {
                cmap.pair(g, c);
            }
            let cmap_data = cmap.finish();
            pdf.cmap(cmap_id, cmap_data.as_slice())
                .system_info(system_info);

            let compressed = deflate(&subset);
            pdf.stream(file_id, &compressed)
                .filter(Filter::FlateDecode)
                .pair(Name(b"Length1"), subset.len() as i32);

            font_ids.push((format!("F{idx}"), type0));
        }

        let mut image_ids = Vec::new();
        for (idx, img) in self.images.iter().enumerate() {
            let id = alloc.bump();
            let smask_id = match &img.kind {
                ImageKind::Raw { alpha: Some(_), .. } => Some(alloc.bump()),
                _ => None,
            };
            match &img.kind {
                ImageKind::Jpeg { data, components } => {
                    let mut x = pdf.image_xobject(id, data);
                    x.filter(Filter::DctDecode);
                    x.width(img.width as i32)
                        .height(img.height as i32)
                        .bits_per_component(8);
                    match components {
                        1 => x.color_space().device_gray(),
                        4 => x.color_space().device_cmyk(),
                        _ => x.color_space().device_rgb(),
                    };
                }
                ImageKind::Raw { data, gray, alpha } => {
                    let mut x = pdf.image_xobject(id, data);
                    x.filter(Filter::FlateDecode);
                    x.width(img.width as i32)
                        .height(img.height as i32)
                        .bits_per_component(8);
                    if *gray {
                        x.color_space().device_gray();
                    } else {
                        x.color_space().device_rgb();
                    }
                    if let Some(smask) = smask_id {
                        x.s_mask(smask);
                    }
                    x.finish();
                    if let (Some(smask), Some(alpha)) = (smask_id, alpha) {
                        let mut m = pdf.image_xobject(smask, alpha);
                        m.filter(Filter::FlateDecode);
                        m.width(img.width as i32)
                            .height(img.height as i32)
                            .bits_per_component(8);
                        m.color_space().device_gray();
                    }
                }
            }
            image_ids.push((format!("Im{idx}"), id));
        }

        {
            let mut page = pdf.page(page_id);
            page.media_box(Rect::new(0.0, 0.0, PAGE_W as f32, PAGE_H as f32))
                .parent(pages_id)
                .contents(content_id);
            let mut res = page.resources();
            {
                let mut fonts = res.fonts();
                for (name, id) in &font_ids {
                    fonts.pair(Name(name.as_bytes()), *id);
                }
            }
            {
                let mut xo = res.x_objects();
                for (name, id) in &image_ids {
                    xo.pair(Name(name.as_bytes()), *id);
                }
            }
        }

        let content = self.content.finish();
        let compressed = deflate(content.as_slice());
        pdf.stream(content_id, &compressed)
            .filter(Filter::FlateDecode);

        let now = Local::now();
        let offset_min = now.offset().fix().local_minus_utc() / 60;
        let date = pdf_writer::Date::new(now.year() as u16)
            .month(now.month() as u8)
            .day(now.day() as u8)
            .hour(now.hour() as u8)
            .minute(now.minute() as u8)
            .second(now.second() as u8)
            .utc_offset_hour((offset_min / 60) as i8)
            .utc_offset_minute((offset_min % 60).unsigned_abs() as u8);
        pdf.document_info(info_id)
            .title(TextStr(&info.title))
            .author(TextStr("Chitach"))
            .subject(TextStr(&info.subject))
            .creator(TextStr("Chitach"))
            .producer(TextStr(concat!(
                "Chitach celik-export ",
                env!("CARGO_PKG_VERSION")
            )))
            .creation_date(date);

        Ok(pdf.finish())
    }
}

/// Deterministic 6-letter subset tag derived from the used glyphs.
fn subset_tag(used: &BTreeMap<u16, (char, u32)>) -> String {
    let mut h: u32 = 2166136261;
    for (&g, &(c, _)) in used {
        for b in g.to_be_bytes().into_iter().chain((c as u32).to_be_bytes()) {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
    }
    (0..6)
        .map(|i| {
            let v = (h >> (i * 5)) % 26;
            (b'A' + v as u8) as char
        })
        .collect()
}
