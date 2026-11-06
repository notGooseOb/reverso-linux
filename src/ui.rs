#![allow(clippy::too_many_arguments)]

use crate::app::App;
use crate::lang;
use crate::translator;
use cairo::{Context, Format, ImageSurface};
use std::f64::consts::PI;
use std::num::NonZeroU32;
use winit::dpi::{PhysicalPosition, PhysicalSize};

type Color = (f64, f64, f64);

const BG: Color = (0.106, 0.106, 0.129);
const PANEL: Color = (0.176, 0.176, 0.216);
const PANEL_HOVER: Color = (0.235, 0.235, 0.290);
const BORDER: Color = (0.275, 0.275, 0.333);
const TEXT: Color = (0.925, 0.925, 0.949);
const MUTED: Color = (0.600, 0.600, 0.660);
const ACCENT: Color = (0.420, 0.660, 1.000);
const ACCENT_DIM: Color = (0.145, 0.235, 0.410);
const ERROR: Color = (1.000, 0.420, 0.420);

#[derive(Clone, Copy, Default, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

#[derive(Default)]
pub struct Layout {
    pub from: Rect,
    pub to: Rect,
    pub swap: Rect,
    pub entry: Rect,
    pub translate: Rect,
    pub close: Rect,
    pub content: Rect,
    pub dropdown: Rect,
    pub dropdown_items: Vec<Rect>,
    pub dropdown_langs: Vec<usize>,
    pub chips: Vec<Rect>,
    pub content_height: f64,
}

fn set_color(cr: &Context, c: Color) {
    cr.set_source_rgb(c.0, c.1, c.2);
}

fn rounded_path(cr: &Context, r: &Rect, radius: f64) {
    let radius = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(r.x + r.w - radius, r.y + radius, radius, -PI / 2.0, 0.0);
    cr.arc(
        r.x + r.w - radius,
        r.y + r.h - radius,
        radius,
        0.0,
        PI / 2.0,
    );
    cr.arc(r.x + radius, r.y + r.h - radius, radius, PI / 2.0, PI);
    cr.arc(r.x + radius, r.y + radius, radius, PI, 3.0 * PI / 2.0);
    cr.close_path();
}

fn fill_round(cr: &Context, r: &Rect, radius: f64, color: Color) {
    set_color(cr, color);
    rounded_path(cr, r, radius);
    let _ = cr.fill();
}

fn stroke_round(cr: &Context, r: &Rect, radius: f64, color: Color, width: f64) {
    set_color(cr, color);
    cr.set_line_width(width);
    rounded_path(cr, r, radius);
    let _ = cr.stroke();
}

fn draw_check(cr: &Context, x: f64, y: f64, scale: f64) {
    set_color(cr, ACCENT);
    cr.set_line_width(1.5 * scale);
    cr.move_to(x - 5.0 * scale, y);
    cr.line_to(x - 1.0 * scale, y + 4.0 * scale);
    cr.line_to(x + 5.0 * scale, y - 4.0 * scale);
    let _ = cr.stroke();
}

fn text_layout(
    pctx: &pango::Context,
    text: &str,
    width: f64,
    px: f64,
    bold: bool,
) -> pango::Layout {
    let layout = pango::Layout::new(pctx);
    let mut desc = pango::FontDescription::from_string("Sans");
    desc.set_absolute_size(px * pango::SCALE as f64);
    if bold {
        desc.set_weight(pango::Weight::Bold);
    }
    layout.set_font_description(Some(&desc));
    if width > 0.0 {
        layout.set_width((width * pango::SCALE as f64) as i32);
        layout.set_wrap(pango::WrapMode::WordChar);
    } else {
        layout.set_width(-1);
    }
    layout.set_text(text);
    layout
}

fn markup_layout(
    pctx: &pango::Context,
    markup: &str,
    width: f64,
    px: f64,
    bold: bool,
) -> pango::Layout {
    let layout = text_layout(pctx, "", width, px, bold);
    layout.set_markup(markup);
    layout
}

fn line_height(pctx: &pango::Context, px: f64) -> f64 {
    let mut desc = pango::FontDescription::from_string("Sans");
    desc.set_absolute_size(px * pango::SCALE as f64);
    let metrics = pctx.metrics(Some(&desc), None);
    ((metrics.ascent() + metrics.descent()) as f64 / pango::SCALE as f64).max(px * 1.2)
}

fn layout_height(layout: &pango::Layout) -> f64 {
    layout.pixel_size().1 as f64
}

fn layout_width(layout: &pango::Layout) -> f64 {
    layout.pixel_size().0 as f64
}

fn draw_layout(cr: &Context, layout: &pango::Layout, x: f64, y: f64) {
    cr.move_to(x, y);
    pangocairo::functions::show_layout(cr, layout);
}

fn draw_text(
    cr: &Context,
    pctx: &pango::Context,
    text: &str,
    x: f64,
    y: f64,
    width: f64,
    px: f64,
    bold: bool,
    color: Color,
) -> f64 {
    let layout = text_layout(pctx, text, width, px, bold);
    let height = layout_height(&layout);
    set_color(cr, color);
    draw_layout(cr, &layout, x, y);
    height
}

fn button(
    cr: &Context,
    pctx: &pango::Context,
    rect: &Rect,
    label: &str,
    scale: f64,
    hovered: bool,
    font: f64,
    centered: bool,
) {
    fill_round(
        cr,
        rect,
        6.0 * scale,
        if hovered { PANEL_HOVER } else { PANEL },
    );
    stroke_round(cr, rect, 6.0 * scale, BORDER, 1.0 * scale);

    let layout = text_layout(pctx, label, -1.0, font, false);
    let (tw, th) = (layout_width(&layout), layout_height(&layout));
    let tx = if centered {
        rect.x + (rect.w - tw) / 2.0
    } else {
        rect.x + 9.0 * scale
    };
    let ty = rect.y + (rect.h - th) / 2.0;

    cr.save().ok();
    rounded_path(cr, rect, 6.0 * scale);
    cr.clip();
    set_color(cr, TEXT);
    draw_layout(cr, &layout, tx, ty);
    cr.restore().ok();
}

fn lang_button(
    cr: &Context,
    pctx: &pango::Context,
    rect: &Rect,
    label: &str,
    scale: f64,
    hovered: bool,
    open: bool,
    font: f64,
) {
    fill_round(
        cr,
        rect,
        6.0 * scale,
        if hovered { PANEL_HOVER } else { PANEL },
    );
    stroke_round(
        cr,
        rect,
        6.0 * scale,
        if open { ACCENT } else { BORDER },
        1.0 * scale,
    );

    let arrow = text_layout(pctx, "▾", -1.0, font, false);
    let arrow_w = layout_width(&arrow);
    let arrow_h = layout_height(&arrow);
    set_color(cr, MUTED);
    draw_layout(
        cr,
        &arrow,
        rect.x + rect.w - arrow_w - 8.0 * scale,
        rect.y + (rect.h - arrow_h) / 2.0,
    );

    let layout = text_layout(pctx, label, -1.0, font, false);
    let th = layout_height(&layout);
    cr.save().ok();
    let inner = Rect {
        x: rect.x,
        y: rect.y,
        w: rect.w - arrow_w - 22.0 * scale,
        h: rect.h,
    };
    rounded_path(cr, &inner, 6.0 * scale);
    cr.clip();
    set_color(cr, TEXT);
    draw_layout(
        cr,
        &layout,
        rect.x + 9.0 * scale,
        rect.y + (rect.h - th) / 2.0,
    );
    cr.restore().ok();
}

fn is_hover(app: &App, rect: &Rect) -> bool {
    rect.contains(app.mouse.0, app.mouse.1)
}

fn draw_response(
    cr: &Context,
    pctx: &pango::Context,
    response: &translator::Response,
    x: f64,
    width: f64,
    mut y: f64,
    scale: f64,
    chips: &mut Vec<Rect>,
    selected: &mut Option<usize>,
    loading: Option<usize>,
    mixed: &[translator::Example],
) -> f64 {
    let gap = 6.0 * scale;
    let short = translator::is_short_text(&response.input);
    let show_chips = short && response.options.len() > 1;

    if show_chips {
        let mut cx = x;
        let mut row_y = y;
        let mut row_h = 0.0f64;
        for (idx, opt) in response.options.iter().take(64).enumerate() {
            let count = if opt.frequency > 0 {
                opt.frequency
            } else {
                opt.count
            };
            let label = if count > 0 {
                format!("{} [{}]", opt.translation, count)
            } else {
                opt.translation.clone()
            };

            let max_text_w = (width - 22.0 * scale).max(40.0 * scale);
            let mut layout = text_layout(pctx, &label, -1.0, 12.0 * scale, false);
            let mut tw = layout_width(&layout);
            if tw > max_text_w {
                layout = text_layout(pctx, &label, max_text_w, 12.0 * scale, false);
                tw = layout_width(&layout).min(max_text_w);
            }
            let (tw, th) = (tw, layout_height(&layout));
            let chip_w = tw + 22.0 * scale;
            let chip_h = th + 10.0 * scale;

            if cx + chip_w > x + width && cx > x {
                cx = x;
                row_y += row_h + gap;
                row_h = 0.0;
            }

            let rect = Rect {
                x: cx,
                y: row_y,
                w: chip_w,
                h: chip_h,
            };
            let selected_now = *selected == Some(idx);
            fill_round(
                cr,
                &rect,
                chip_h / 2.0,
                if selected_now { ACCENT_DIM } else { PANEL },
            );
            stroke_round(
                cr,
                &rect,
                chip_h / 2.0,
                if selected_now { ACCENT } else { BORDER },
                1.0 * scale,
            );
            set_color(cr, TEXT);
            draw_layout(cr, &layout, cx + 11.0 * scale, row_y + (chip_h - th) / 2.0);

            chips.push(rect);
            cx += chip_w + gap;
            row_h = row_h.max(chip_h);
        }
        y = row_y + row_h + 12.0 * scale;
    } else {
        if response.translations.is_empty() {
            y += draw_text(
                cr,
                pctx,
                "No translation",
                x,
                y,
                width,
                14.0 * scale,
                false,
                MUTED,
            ) + 6.0 * scale;
        } else {
            for translation in response.translations.iter().take(3) {
                let line = format!("• {translation}");
                y += draw_text(cr, pctx, &line, x, y, width, 14.0 * scale, false, TEXT)
                    + 4.0 * scale;
            }
            y += 4.0 * scale;
        }
    }

    let selected_opt = selected
        .and_then(|i| response.options.get(i))
        .or_else(|| response.options.first());

    if let Some(opt) = selected_opt {
        let shown_index = selected.or(if response.options.is_empty() {
            None
        } else {
            Some(0)
        });
        let header = if selected.is_some() {
            format!("Examples for: {}", opt.translation)
        } else {
            "Examples".to_string()
        };
        y += draw_text(cr, pctx, &header, x, y, width, 12.0 * scale, true, MUTED) + 5.0 * scale;

        if show_chips && selected.is_none() {
            if mixed.is_empty() {
                y += draw_text(
                    cr,
                    pctx,
                    "Select a translation to see examples.",
                    x,
                    y,
                    width,
                    11.5 * scale,
                    false,
                    MUTED,
                ) + 4.0 * scale;
            } else {
                draw_examples(cr, pctx, mixed, x, width, &mut y, scale);
            }
        } else if opt.examples.is_empty() && loading.is_some() && loading == shown_index {
            y += draw_text(
                cr,
                pctx,
                "Fetching examples…",
                x,
                y,
                width,
                11.5 * scale,
                false,
                MUTED,
            ) + 4.0 * scale;
        } else if opt.examples.is_empty() {
            y += draw_text(
                cr,
                pctx,
                "No examples",
                x,
                y,
                width,
                11.5 * scale,
                false,
                MUTED,
            ) + 4.0 * scale;
        } else {
            draw_examples(cr, pctx, &opt.examples, x, width, &mut y, scale);
        }
    }

    y
}

fn draw_examples(
    cr: &Context,
    pctx: &pango::Context,
    examples: &[translator::Example],
    x: f64,
    width: f64,
    y: &mut f64,
    scale: f64,
) -> bool {
    let col_gap = 18.0 * scale;
    let col_w = ((width - col_gap) / 2.0).max(10.0);

    for example in examples {
        let source = markup_layout(pctx, &example.source_markup, col_w, 11.5 * scale, false);
        let target = markup_layout(pctx, &example.target_markup, col_w, 11.5 * scale, false);
        if pango::find_base_dir(&example.target) == pango::Direction::Rtl {
            target.set_alignment(pango::Alignment::Right);
        }
        let sh = layout_height(&source);
        let th = layout_height(&target);

        set_color(cr, MUTED);
        draw_layout(cr, &source, x, *y);
        set_color(cr, TEXT);
        draw_layout(cr, &target, x + col_w + col_gap, *y);

        *y += sh.max(th) + 5.0 * scale;
    }

    !examples.is_empty()
}

impl App {
    pub fn draw(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some(mut surface) = self.surface.take() else {
            return;
        };

        let size = window.inner_size();
        let pw = size.width as i32;
        let ph = size.height as i32;
        if pw <= 0 || ph <= 0 {
            self.surface = Some(surface);
            return;
        }

        let scale = window.scale_factor();
        let stride = (pw * 4) as usize;
        let mut pixels = std::mem::take(&mut self.pixels);
        pixels.resize(stride * ph as usize, 0);

        {
            let image = match unsafe {
                ImageSurface::create_for_data_unsafe(
                    pixels.as_mut_ptr(),
                    Format::ARgb32,
                    pw,
                    ph,
                    stride as i32,
                )
            } {
                Ok(image) => image,
                Err(_) => {
                    self.pixels = pixels;
                    self.surface = Some(surface);
                    return;
                }
            };
            let cr = match Context::new(&image) {
                Ok(cr) => cr,
                Err(_) => {
                    self.pixels = pixels;
                    self.surface = Some(surface);
                    return;
                }
            };
            self.render(&cr, pw as f64, ph as f64, scale);
        }

        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            self.pixels = pixels;
            self.surface = Some(surface);
            return;
        };
        if surface.resize(w, h).is_err() {
            self.pixels = pixels;
            self.surface = Some(surface);
            return;
        }

        if self.focused {
            if let Some((x, y, h)) = self.ime_area {
                window.set_ime_cursor_area(
                    PhysicalPosition::new(x as i32, y as i32),
                    PhysicalSize::new(2u32, h.max(1.0) as u32),
                );
            }
        }

        if let Ok(mut buffer) = surface.buffer_mut() {
            for (dst, src) in buffer.iter_mut().zip(pixels.chunks_exact(4)) {
                *dst = ((src[2] as u32) << 16) | ((src[1] as u32) << 8) | src[0] as u32;
            }
            window.pre_present_notify();
            let _ = buffer.present();
        }

        self.pixels = pixels;
        self.surface = Some(surface);
    }

    fn render(&mut self, cr: &Context, w: f64, h: f64, scale: f64) {
        let pad = 10.0 * scale;
        let gap = 6.0 * scale;
        let pctx = pangocairo::functions::create_context(cr);
        let font = 13.0 * scale;
        let small = 11.5 * scale;

        set_color(cr, BG);
        let _ = cr.paint();

        // --- language row -------------------------------------------------
        let row_h = 30.0 * scale;
        let y = pad;
        let label_w = 38.0 * scale;
        let swap_w = 30.0 * scale;
        let button_w =
            ((w - 2.0 * pad - 2.0 * label_w - swap_w - 3.0 * gap) / 2.0).max(80.0 * scale);

        let from = Rect {
            x: pad + label_w,
            y,
            w: button_w,
            h: row_h,
        };
        let swap = Rect {
            x: from.x + from.w + gap,
            y,
            w: swap_w,
            h: row_h,
        };
        let to_label_x = swap.x + swap.w + gap;
        let to = Rect {
            x: to_label_x + label_w,
            y,
            w: button_w,
            h: row_h,
        };

        self.layout.from = from;
        self.layout.swap = swap;
        self.layout.to = to;

        let label_y = y + (row_h - line_height(&pctx, small)) / 2.0;
        draw_text(cr, &pctx, "From", pad, label_y, label_w, small, true, MUTED);
        draw_text(
            cr, &pctx, "To", to_label_x, label_y, label_w, small, true, MUTED,
        );

        lang_button(
            cr,
            &pctx,
            &from,
            lang::name(self.source_idx),
            scale,
            is_hover(self, &from),
            matches!(
                self.dropdown.as_ref().map(|d| d.side),
                Some(crate::app::Side::Source)
            ),
            font,
        );
        lang_button(
            cr,
            &pctx,
            &to,
            lang::name(self.target_idx),
            scale,
            is_hover(self, &to),
            matches!(
                self.dropdown.as_ref().map(|d| d.side),
                Some(crate::app::Side::Target)
            ),
            font,
        );
        button(
            cr,
            &pctx,
            &swap,
            "⇄",
            scale,
            is_hover(self, &swap),
            font,
            true,
        );

        // --- input row ----------------------------------------------------
        let entry_y = y + row_h + gap;
        let translate_w = 112.0 * scale;
        let entry = Rect {
            x: pad,
            y: entry_y,
            w: (w - 2.0 * pad - translate_w - gap).max(60.0 * scale),
            h: 34.0 * scale,
        };
        let translate = Rect {
            x: entry.x + entry.w + gap,
            y: entry_y,
            w: translate_w,
            h: entry.h,
        };
        self.layout.entry = entry;
        self.layout.translate = translate;

        fill_round(cr, &entry, 6.0 * scale, PANEL);
        stroke_round(cr, &entry, 6.0 * scale, BORDER, 1.0 * scale);

        let text_x = entry.x + 9.0 * scale;
        let text_y = entry.y + (entry.h - line_height(&pctx, font)) / 2.0;
        let layout = text_layout(&pctx, &self.editor.text, -1.0, font, false);

        if let Some(mouse_x) = self.pending_cursor.take() {
            let px = ((mouse_x - text_x + self.editor.scroll) * pango::SCALE as f64) as i32;
            let (_, index, _) = layout.xy_to_index(px, 0);
            let mut index = index.max(0) as usize;
            index = index.min(self.editor.text.len());
            while index > 0 && !self.editor.text.is_char_boundary(index) {
                index -= 1;
            }
            if self.pending_anchor {
                self.editor.anchor = index;
                self.pending_anchor = false;
            }
            self.editor.cursor = index;
        }

        let cursor_rect = layout.index_to_pos(self.editor.cursor as i32);
        let mut caret_x =
            text_x + cursor_rect.x() as f64 / pango::SCALE as f64 - self.editor.scroll;
        let left_edge = entry.x + 9.0 * scale;
        let right_edge = entry.x + entry.w - 9.0 * scale;
        if caret_x < left_edge {
            self.editor.scroll -= left_edge - caret_x;
            caret_x = left_edge;
        } else if caret_x > right_edge {
            self.editor.scroll += caret_x - right_edge;
            caret_x = right_edge;
        }
        self.editor.scroll = self.editor.scroll.max(0.0);
        let draw_x = text_x - self.editor.scroll;
        let caret_h = line_height(&pctx, font);

        cr.save().ok();
        rounded_path(cr, &entry, 6.0 * scale);
        cr.clip();

        if let Some((a, b)) = self.editor.selection() {
            let pa = layout.index_to_pos(a as i32);
            let pb = layout.index_to_pos(b as i32);
            let x1 = draw_x + pa.x() as f64 / pango::SCALE as f64;
            let x2 = draw_x + pb.x() as f64 / pango::SCALE as f64;
            cr.set_source_rgba(ACCENT.0, ACCENT.1, ACCENT.2, 0.28);
            cr.rectangle(x1.min(x2), text_y, (x2 - x1).abs(), caret_h);
            let _ = cr.fill();
        }

        let mut caret_now = caret_x;
        if self.preedit.is_empty() {
            set_color(cr, TEXT);
            draw_layout(cr, &layout, draw_x, text_y);
        } else {
            let (before, after) = self.editor.text.split_at(self.editor.cursor);
            let before_layout = text_layout(&pctx, before, -1.0, font, false);
            let before_w = layout_width(&before_layout);
            let preedit_layout = text_layout(&pctx, &self.preedit, -1.0, font, false);
            let preedit_w = layout_width(&preedit_layout);

            set_color(cr, TEXT);
            draw_layout(cr, &before_layout, draw_x, text_y);

            let preedit_x = draw_x + before_w;
            set_color(cr, ACCENT);
            draw_layout(cr, &preedit_layout, preedit_x, text_y);
            cr.rectangle(
                preedit_x,
                text_y + caret_h - 1.5 * scale,
                preedit_w,
                1.5 * scale,
            );
            let _ = cr.fill();

            if self.editor.cursor < self.editor.text.len() {
                let after_layout = text_layout(&pctx, after, -1.0, font, false);
                set_color(cr, TEXT);
                draw_layout(cr, &after_layout, preedit_x + preedit_w, text_y);
            }
            caret_now = preedit_x + preedit_w;
        }

        if self.editor.text.is_empty() && self.preedit.is_empty() {
            set_color(cr, MUTED);
            let placeholder = text_layout(&pctx, "Enter text to translate…", -1.0, font, false);
            draw_layout(cr, &placeholder, draw_x, text_y);
        }

        if self.focused && self.caret_on {
            set_color(cr, ACCENT);
            cr.rectangle(caret_now, text_y, 1.5 * scale, caret_h);
            let _ = cr.fill();
        }
        self.ime_area = Some((caret_now, text_y, caret_h));

        cr.restore().ok();

        let translate_label = if self.busy {
            "Translating…"
        } else {
            "Translate"
        };
        button(
            cr,
            &pctx,
            &translate,
            translate_label,
            scale,
            is_hover(self, &translate),
            font,
            true,
        );

        // --- content ------------------------------------------------------
        let footer_h = 30.0 * scale;
        let content_y = entry_y + entry.h + 2.0 * gap;
        let content = Rect {
            x: pad,
            y: content_y,
            w: w - 2.0 * pad,
            h: (h - pad - footer_h - gap - content_y).max(40.0 * scale),
        };
        self.layout.content = content;
        self.layout.chips.clear();

        cr.save().ok();
        cr.rectangle(content.x, content.y, content.w, content.h);
        cr.clip();
        cr.translate(0.0, -self.scroll);

        let mut cy = content.y;
        if self.busy {
            cy += draw_text(
                cr,
                &pctx,
                "Translating…",
                content.x,
                cy,
                content.w,
                14.0 * scale,
                false,
                MUTED,
            );
        } else if let Some(error) = &self.error {
            cy += draw_text(
                cr,
                &pctx,
                error,
                content.x,
                cy,
                content.w,
                12.5 * scale,
                false,
                ERROR,
            );
        } else if let Some(response) = &self.response {
            cy = draw_response(
                cr,
                &pctx,
                response,
                content.x,
                content.w,
                cy,
                scale,
                &mut self.layout.chips,
                &mut self.selected_option,
                self.examples_loading,
                &self.mixed_examples,
            );
        } else {
            cy += draw_text(
                cr,
                &pctx,
                "Type or paste text, then press Translate.",
                content.x,
                cy,
                content.w,
                12.5 * scale,
                false,
                MUTED,
            );
        }

        cr.restore().ok();

        self.layout.content_height = (cy - content.y).max(0.0);
        let max_scroll = (self.layout.content_height - content.h).max(0.0);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }

        if max_scroll > 0.0 {
            let track = content.h;
            let thumb = (track * (content.h / self.layout.content_height)).max(18.0 * scale);
            let pos = content.y + (track - thumb) * (self.scroll / max_scroll);
            let bar = Rect {
                x: content.x + content.w - 3.0 * scale,
                y: pos,
                w: 3.0 * scale,
                h: thumb,
            };
            cr.set_source_rgba(MUTED.0, MUTED.1, MUTED.2, 0.5);
            rounded_path(cr, &bar, 1.5 * scale);
            let _ = cr.fill();
        }

        // --- footer -------------------------------------------------------
        let close_w = 140.0 * scale;
        let close_h = 28.0 * scale;
        let close = Rect {
            x: (w - close_w) / 2.0,
            y: h - pad - close_h,
            w: close_w,
            h: close_h,
        };
        self.layout.close = close;
        button(
            cr,
            &pctx,
            &close,
            "Close (Esc)",
            scale,
            is_hover(self, &close),
            small,
            true,
        );

        // --- dropdown overlay --------------------------------------------
        if let Some(dropdown) = &self.dropdown {
            let anchor = match dropdown.side {
                crate::app::Side::Source => from,
                crate::app::Side::Target => to,
            };
            let filtered = lang::filter(&dropdown.query);
            let inner_pad = 6.0 * scale;
            let search_h = 30.0 * scale;
            let item_h = 24.0 * scale;
            let rows = filtered.len().max(if filtered.is_empty() { 1 } else { 0 });
            let list_h = inner_pad * 2.0 + search_h + 4.0 * scale + rows as f64 * item_h;
            let list_w = anchor.w.max(190.0 * scale);

            let mut list_y = anchor.y + anchor.h + 2.0 * scale;
            if list_y + list_h > h - pad {
                list_y = (anchor.y - list_h - 2.0 * scale).max(pad);
            }
            let list = Rect {
                x: anchor.x,
                y: list_y,
                w: list_w,
                h: list_h,
            };
            self.layout.dropdown = list;
            self.layout.dropdown_items.clear();
            self.layout.dropdown_langs.clear();

            cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
            rounded_path(
                cr,
                &Rect {
                    x: list.x + 2.0 * scale,
                    y: list.y + 3.0 * scale,
                    w: list.w,
                    h: list.h,
                },
                6.0 * scale,
            );
            let _ = cr.fill();

            fill_round(cr, &list, 6.0 * scale, PANEL);
            stroke_round(cr, &list, 6.0 * scale, BORDER, 1.0 * scale);

            let search = Rect {
                x: list.x + inner_pad,
                y: list.y + inner_pad,
                w: list.w - 2.0 * inner_pad,
                h: search_h,
            };
            fill_round(cr, &search, 4.0 * scale, BG);
            stroke_round(
                cr,
                &search,
                4.0 * scale,
                if dropdown.query.is_empty() {
                    BORDER
                } else {
                    ACCENT
                },
                1.0 * scale,
            );

            let query_font = 12.5 * scale;
            let query_lh = line_height(&pctx, query_font);
            let query_y = search.y + (search.h - query_lh) / 2.0;
            let query_x = search.x + 8.0 * scale;
            let query_w = (search.w - 16.0 * scale).max(1.0);

            let _ = cr.save();
            cr.rectangle(
                search.x + 1.0 * scale,
                search.y + 1.0 * scale,
                search.w - 2.0 * scale,
                search.h - 2.0 * scale,
            );
            cr.clip();

            if dropdown.query.is_empty() {
                if self.focused && self.caret_on {
                    set_color(cr, ACCENT);
                    cr.rectangle(query_x, query_y, 1.5 * scale, query_lh);
                    let _ = cr.fill();
                }
                draw_text(
                    cr,
                    &pctx,
                    "Search languages…",
                    query_x + 4.0 * scale,
                    query_y,
                    query_w - 4.0 * scale,
                    query_font,
                    false,
                    MUTED,
                );
                self.ime_area = Some((query_x + 4.0 * scale, query_y, query_lh));
            } else {
                let query_layout = text_layout(&pctx, &dropdown.query, -1.0, query_font, false);
                let qw = layout_width(&query_layout);
                let qx = (query_x - (qw - query_w).max(0.0)).max(query_x);
                set_color(cr, TEXT);
                draw_layout(cr, &query_layout, qx, query_y);
                let caret_x = qx + qw + 1.0 * scale;
                if self.focused && self.caret_on {
                    set_color(cr, ACCENT);
                    cr.rectangle(caret_x, query_y, 1.5 * scale, query_lh);
                    let _ = cr.fill();
                }
                self.ime_area = Some((caret_x, query_y, query_lh));
            }
            let _ = cr.restore();

            let items_y = search.y + search.h + 4.0 * scale;
            let selected_idx = match dropdown.side {
                crate::app::Side::Source => self.source_idx,
                crate::app::Side::Target => self.target_idx,
            };

            if filtered.is_empty() {
                let lh = line_height(&pctx, 12.0 * scale);
                draw_text(
                    cr,
                    &pctx,
                    "No matches",
                    list.x + inner_pad + 3.0 * scale,
                    items_y + (item_h - lh) / 2.0,
                    list.w - 2.0 * inner_pad,
                    12.0 * scale,
                    false,
                    MUTED,
                );
            }

            for (vis, &lang_idx) in filtered.iter().enumerate() {
                let item = Rect {
                    x: list.x + 2.0 * scale,
                    y: items_y + vis as f64 * item_h,
                    w: list.w - 4.0 * scale,
                    h: item_h,
                };
                let is_current = lang_idx == selected_idx;
                if dropdown.highlight == Some(vis) {
                    fill_round(cr, &item, 4.0 * scale, ACCENT_DIM);
                }
                let item_layout =
                    text_layout(&pctx, lang::LANGS[lang_idx].name, -1.0, 12.0 * scale, false);
                let th = layout_height(&item_layout);
                set_color(cr, if is_current { ACCENT } else { TEXT });
                draw_layout(
                    cr,
                    &item_layout,
                    item.x + 9.0 * scale,
                    item.y + (item.h - th) / 2.0,
                );
                if is_current {
                    draw_check(
                        cr,
                        item.x + item.w - 14.0 * scale,
                        item.y + item.h / 2.0,
                        scale,
                    );
                }
                self.layout.dropdown_items.push(item);
                self.layout.dropdown_langs.push(lang_idx);
            }
        } else {
            self.layout.dropdown = Rect::default();
            self.layout.dropdown_items.clear();
            self.layout.dropdown_langs.clear();
        }
    }
}
