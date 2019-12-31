use crate::context::TextMetrics;
use crate::renderer::TextureType;
use crate::{Align, Bounds, ErrorKind, Extent, ImageFlags, Renderer, Result};
use bitflags::_core::borrow::Borrow;
use failure::ResultExt;
use rusttype::gpu_cache::Cache;
use rusttype::{Font, Glyph, Point, PositionedGlyph, Scale};
use slab::Slab;
use std::collections::HashMap;

const TEX_WIDTH: usize = 1024;
const TEX_HEIGHT: usize = 1024;

pub type FontId = usize;

#[derive(Debug)]
pub struct LayoutChar {
    id: FontId,
    pub x: f32,
    pub next_x: f32,
    pub c: char,
    pub idx: usize,
    glyph: PositionedGlyph<'static>,
    pub uv: Bounds,
    pub bounds: Bounds,
}

struct FontData {
    font: Font<'static>,
    fallback_fonts: Vec<FontId>,
}

pub struct Fonts<R: Renderer> {
    fonts: Slab<FontData>,
    fonts_by_name: HashMap<String, FontId>,
    cache: Cache<'static>,
    pub img: R::ImageHandle,
}

impl<R: Renderer> Fonts<R> {
    pub fn new(renderer: &mut R) -> Result<Fonts<R>> {
        Ok(Fonts {
            fonts: Default::default(),
            fonts_by_name: Default::default(),
            img: renderer
                .create_texture(
                    TextureType::Alpha,
                    TEX_WIDTH,
                    TEX_HEIGHT,
                    ImageFlags::empty(),
                    None,
                )
                .context(ErrorKind::Renderer)?,
            cache: Cache::builder()
                .multithread(true)
                .dimensions(TEX_WIDTH as u32, TEX_HEIGHT as u32)
                .build(),
        })
    }

    pub fn add_font<N: Into<String>, D: Into<Vec<u8>>>(
        &mut self,
        name: N,
        data: D,
    ) -> Result<FontId> {
        let font = Font::<'static>::from_bytes(data.into()).context(ErrorKind::Font)?;
        let fd = FontData {
            font,
            fallback_fonts: Default::default(),
        };
        let id = self.fonts.insert(fd);
        self.fonts_by_name.insert(name.into(), id);
        Ok(id)
    }

    pub fn find<N: Borrow<str>>(&self, name: N) -> Option<FontId> {
        self.fonts_by_name.get(name.borrow()).map(ToOwned::to_owned)
    }

    pub fn add_fallback(&mut self, base: FontId, fallback: FontId) {
        if let Some(fd) = self.fonts.get_mut(base) {
            fd.fallback_fonts.push(fallback);
        }
    }

    fn glyph(&self, id: FontId, c: char) -> Option<(FontId, Glyph<'static>)> {
        if let Some(fd) = self.fonts.get(id) {
            let glyph = fd.font.glyph(c);
            if glyph.id().0 != 0 {
                Some((id, glyph))
            } else {
                for id in &fd.fallback_fonts {
                    if let Some(fd) = self.fonts.get(*id) {
                        let glyph = fd.font.glyph(c);
                        if glyph.id().0 != 0 {
                            return Some((*id, glyph));
                        }
                    }
                }
                None
            }
        } else {
            None
        }
    }

    fn render_texture(&mut self, renderer: &mut R) -> Result<()> {
        let img = self.img.clone();
        self.cache
            .cache_queued(move |rect, data| {
                renderer
                    .update_texture(
                        img.clone(),
                        rect.min.x as usize,
                        rect.min.y as usize,
                        (rect.max.x - rect.min.x) as usize,
                        (rect.max.y - rect.min.y) as usize,
                        data,
                    )
                    .unwrap();
            })
            .context(ErrorKind::Font)?;
        Ok(())
    }

    pub fn text_metrics(&self, id: FontId, size: f32) -> TextMetrics {
        if let Some(fd) = self.fonts.get(id) {
            let scale = Scale::uniform(size);
            let v_metrics = fd.font.v_metrics(scale);
            TextMetrics {
                ascender: v_metrics.descent,
                descender: v_metrics.descent,
                line_gap: v_metrics.line_gap,
            }
        } else {
            TextMetrics {
                ascender: 0.0,
                descender: 0.0,
                line_gap: 0.0,
            }
        }
    }

    pub fn text_size(&self, text: &str, id: FontId, size: f32, spacing: f32) -> Extent {
        if let Some(fd) = self.fonts.get(id) {
            let scale = Scale::uniform(size);
            let v_metrics = fd.font.v_metrics(scale);
            let mut extent = Extent::new(
                0.0,
                v_metrics.ascent - v_metrics.descent + v_metrics.line_gap,
            );
            let mut last_glyph = None;
            let mut char_count = 0;

            for c in text.chars() {
                if let Some((_, glyph)) = self.glyph(id, c) {
                    let glyph = glyph.scaled(scale);
                    let h_metrics = glyph.h_metrics();
                    extent.width += h_metrics.advance_width;

                    if let Some(last_glyph) = last_glyph {
                        extent.width += fd.font.pair_kerning(scale, last_glyph, glyph.id());
                    }

                    last_glyph = Some(glyph.id());
                    char_count += 1;
                }
            }

            if char_count >= 2 {
                extent.width += spacing * (char_count - 1) as f32;
            }

            extent
        } else {
            Default::default()
        }
    }

    pub fn layout_text(
        &mut self,
        renderer: &mut R,
        text: &str,
        id: FontId,
        position: crate::Point,
        size: f32,
        align: Align,
        spacing: f32,
        cache: bool,
        result: &mut Vec<LayoutChar>,
    ) -> Result<()> {
        result.clear();

        if let Some(fd) = self.fonts.get(id) {
            let mut offset = Point { x: 0.0, y: 0.0 };
            let scale = Scale::uniform(size);
            let v_metrics = fd.font.v_metrics(scale);

            let sz = if align.contains(Align::CENTER)
                || align.contains(Align::RIGHT)
                || align.contains(Align::MIDDLE)
            {
                self.text_size(text, id, size, spacing)
            } else {
                Extent::new(0.0, 0.0)
            };

            if align.contains(Align::CENTER) {
                offset.x -= sz.width / 2.0;
            } else if align.contains(Align::RIGHT) {
                offset.x -= sz.width;
            }

            if align.contains(Align::MIDDLE) {
                offset.y = v_metrics.descent + sz.height / 2.0;
            } else if align.contains(Align::BOTTOM) {
                offset.y = v_metrics.descent;
            } else if align.contains(Align::TOP) {
                offset.y = v_metrics.ascent;
            }

            let mut position = Point {
                x: position.x + offset.x,
                y: position.y + offset.y,
            };
            let mut last_glyph = None;

            for (idx, c) in text.chars().enumerate() {
                if let Some((id, glyph)) = self.glyph(id, c) {
                    let g = glyph.scaled(scale);
                    let h_metrics = g.h_metrics();

                    let glyph = g.positioned(Point {
                        x: position.x,
                        y: position.y,
                    });

                    let mut next_x = position.x + h_metrics.advance_width;
                    if let Some(last_glyph) = last_glyph {
                        next_x += fd.font.pair_kerning(scale, last_glyph, glyph.id());
                    }

                    if let Some(bb) = glyph.pixel_bounding_box() {
                        self.cache.queue_glyph(id, glyph.clone());

                        result.push(LayoutChar {
                            id,
                            idx,
                            c,
                            x: dbg!(position.x),
                            next_x,
                            glyph: glyph.clone(),
                            uv: Default::default(),
                            bounds: Bounds {
                                min: (bb.min.x, bb.min.y).into(),
                                max: (bb.max.x, bb.max.y).into(),
                            },
                        });
                    }

                    position.x = next_x;
                    last_glyph = Some(glyph.id());
                }
            }

            if cache {
                self.render_texture(renderer)?;

                for lc in result {
                    if let Ok(Some((uv, _))) = self.cache.rect_for(lc.id, &lc.glyph) {
                        lc.uv = Bounds {
                            min: crate::Point {
                                x: uv.min.x,
                                y: uv.min.y,
                            },
                            max: crate::Point {
                                x: uv.max.x,
                                y: uv.max.y,
                            },
                        };
                    }
                }
            }
        }

        Ok(())
    }
}
