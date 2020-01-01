use crate::cache::PathCache;
use crate::fonts::{FontId, Fonts, LayoutChar};
use crate::renderer::{Renderer, Scissor, TextureType};
use crate::{Color, Extent, Point, Rect, Transform};
use clamped::Clamp;
use std::f32::consts::PI;

const KAPPA90: f32 = 0.5522847493;

#[derive(Debug, Copy, Clone)]
pub struct Paint<H: Clone> {
    pub xform: Transform,
    pub extent: Extent,
    pub radius: f32,
    pub feather: f32,
    pub inner_color: Color,
    pub outer_color: Color,
    pub image: Option<H>,
}

#[derive(Debug, Copy, Clone)]
pub enum Gradient {
    Linear {
        start: Point,
        end: Point,
        inner_color: Color,
        outer_color: Color,
    },
    Radial {
        center: Point,
        in_radius: f32,
        out_radius: f32,
        inner_color: Color,
        outer_color: Color,
    },
    Box {
        rect: Rect,
        radius: f32,
        feather: f32,
        inner_color: Color,
        outer_color: Color,
    },
}

#[derive(Debug, Copy, Clone)]
pub struct ImagePattern<H: Clone> {
    pub center: Point,
    pub size: Extent,
    pub angle: f32,
    pub handle: H,
    pub alpha: f32,
}

impl<H: Clone> From<Gradient> for Paint<H> {
    fn from(grad: Gradient) -> Self {
        match grad {
            Gradient::Linear {
                start,
                end,
                inner_color,
                outer_color,
            } => {
                const LARGE: f32 = 1e5;

                let mut dx = end.x - start.x;
                let mut dy = end.y - start.y;
                let d = (dx * dx + dy * dy).sqrt();

                if d > 0.0001 {
                    dx /= d;
                    dy /= d;
                } else {
                    dx = 0.0;
                    dy = 1.0;
                }

                Paint {
                    xform: Transform([dy, -dx, dx, dy, start.x - dx * LARGE, start.y - dy * LARGE]),
                    extent: Extent {
                        width: LARGE,
                        height: LARGE + d * 0.5,
                    },
                    radius: 0.0,
                    feather: d.max(1.0),
                    inner_color,
                    outer_color,
                    image: None,
                }
            }
            Gradient::Radial {
                center,
                in_radius,
                out_radius,
                inner_color,
                outer_color,
            } => {
                let r = (in_radius + out_radius) * 0.5;
                let f = out_radius - in_radius;
                Paint {
                    xform: Transform([1.0, 0.0, 0.0, 1.0, center.x, center.y]),
                    extent: Extent {
                        width: r,
                        height: r,
                    },
                    radius: r,
                    feather: f.max(1.0),
                    inner_color,
                    outer_color,
                    image: None,
                }
            }
            Gradient::Box {
                rect,
                radius,
                feather,
                inner_color,
                outer_color,
            } => {
                let Rect { xy, size } = rect;
                Paint {
                    xform: Transform([
                        1.0,
                        0.0,
                        0.0,
                        1.0,
                        xy.x + size.width * 0.5,
                        xy.y + size.height * 0.5,
                    ]),
                    extent: Extent::new(size.width * 0.5, size.height * 0.5),
                    radius,
                    feather: feather.max(1.0),
                    inner_color,
                    outer_color,
                    image: None,
                }
            }
        }
    }
}

impl<H: Clone> From<ImagePattern<H>> for Paint<H> {
    fn from(pat: ImagePattern<H>) -> Self {
        let mut xform = Transform::rotate(pat.angle);
        xform.0[4] = pat.center.x;
        xform.0[5] = pat.center.y;
        Paint {
            xform,
            extent: pat.size,
            radius: 0.0,
            feather: 0.0,
            inner_color: Color::rgba(1.0, 1.0, 1.0, pat.alpha),
            outer_color: Color::rgba(1.0, 1.0, 1.0, pat.alpha),
            image: Some(pat.handle),
        }
    }
}

impl<H: Clone, T: Into<Color> + Clone> From<T> for Paint<H> {
    fn from(color: T) -> Self {
        Paint {
            xform: Transform::identity(),
            extent: Default::default(),
            radius: 0.0,
            feather: 1.0,
            inner_color: color.clone().into(),
            outer_color: color.into(),
            image: None,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Solidity {
    Solid,
    Hole,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

bitflags! {
    pub struct Align: u32 {
        const LEFT = 0x1;
        const CENTER = 0x2;
        const RIGHT = 0x4;
        const TOP = 0x8;
        const MIDDLE = 0x10;
        const BOTTOM = 0x20;
        const BASELINE = 0x40;
    }
}

#[derive(Debug, Copy, Clone)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColor,
    OneMinusSrcColor,
    DstColor,
    OneMinusDstColor,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturate,
}

#[derive(Debug, Copy, Clone)]
pub enum BasicCompositeOperation {
    SrcOver,
    SrcIn,
    SrcOut,
    Atop,
    DstOver,
    DstIn,
    DstOut,
    DstAtop,
    Lighter,
    Copy,
    Xor,
}

#[derive(Debug, Copy, Clone)]
pub enum CompositeOperation {
    Basic(BasicCompositeOperation),
    BlendFunc {
        src: BlendFactor,
        dst: BlendFactor,
    },
    BlendFuncSeparate {
        src_rgb: BlendFactor,
        dst_rgb: BlendFactor,
        src_alpha: BlendFactor,
        dst_alpha: BlendFactor,
    },
}

impl Into<CompositeOperationState> for CompositeOperation {
    fn into(self) -> CompositeOperationState {
        match self {
            CompositeOperation::Basic(op) => {
                let (src_factor, dst_factor) = match op {
                    BasicCompositeOperation::SrcOver => {
                        (BlendFactor::One, BlendFactor::OneMinusSrcAlpha)
                    }
                    BasicCompositeOperation::SrcIn => (BlendFactor::DstAlpha, BlendFactor::Zero),
                    BasicCompositeOperation::SrcOut => {
                        (BlendFactor::OneMinusDstAlpha, BlendFactor::Zero)
                    }
                    BasicCompositeOperation::Atop => {
                        (BlendFactor::DstAlpha, BlendFactor::OneMinusSrcAlpha)
                    }
                    BasicCompositeOperation::DstOver => {
                        (BlendFactor::OneMinusDstAlpha, BlendFactor::One)
                    }
                    BasicCompositeOperation::DstIn => (BlendFactor::Zero, BlendFactor::SrcAlpha),
                    BasicCompositeOperation::DstOut => {
                        (BlendFactor::Zero, BlendFactor::OneMinusSrcAlpha)
                    }
                    BasicCompositeOperation::DstAtop => {
                        (BlendFactor::OneMinusDstAlpha, BlendFactor::SrcAlpha)
                    }
                    BasicCompositeOperation::Lighter => (BlendFactor::One, BlendFactor::One),
                    BasicCompositeOperation::Copy => (BlendFactor::One, BlendFactor::Zero),
                    BasicCompositeOperation::Xor => {
                        (BlendFactor::OneMinusDstAlpha, BlendFactor::OneMinusSrcAlpha)
                    }
                };

                CompositeOperationState {
                    src_rgb: src_factor,
                    dst_rgb: dst_factor,
                    src_alpha: src_factor,
                    dst_alpha: dst_factor,
                }
            }
            CompositeOperation::BlendFunc { src, dst } => CompositeOperationState {
                src_rgb: src,
                dst_rgb: dst,
                src_alpha: src,
                dst_alpha: dst,
            },
            CompositeOperation::BlendFuncSeparate {
                src_rgb,
                dst_rgb,
                src_alpha,
                dst_alpha,
            } => CompositeOperationState {
                src_rgb,
                dst_rgb,
                src_alpha,
                dst_alpha,
            },
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CompositeOperationState {
    pub src_rgb: BlendFactor,
    pub dst_rgb: BlendFactor,
    pub src_alpha: BlendFactor,
    pub dst_alpha: BlendFactor,
}

bitflags! {
    pub struct ImageFlags: u32 {
        const GENERATE_MIPMAPS = 0x1;
        const REPEATX = 0x2;
        const REPEATY = 0x4;
        const FLIPY	= 0x8;
        const PREMULTIPLIED = 0x10;
        const NEAREST = 0x20;
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub u: f32,
    pub v: f32,
}

impl Vertex {
    pub fn new(x: f32, y: f32, u: f32, v: f32) -> Vertex {
        Vertex { x, y, u, v }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Path {
    pub(crate) first: usize,
    pub(crate) count: usize,
    pub(crate) closed: bool,
    pub(crate) num_bevel: usize,
    pub(crate) solidity: Solidity,
    pub(crate) fill: *mut Vertex,
    pub(crate) num_fill: usize,
    pub(crate) stroke: *mut Vertex,
    pub(crate) num_stroke: usize,
    pub convex: bool,
}

impl Path {
    pub fn get_fill(&self) -> &[Vertex] {
        if self.fill.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.fill, self.num_fill) }
        }
    }

    pub fn get_stroke(&self) -> &[Vertex] {
        if self.stroke.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.stroke, self.num_stroke) }
        }
    }
}

#[derive(Copy, Clone)]
pub struct TextMetrics {
    pub ascender: f32,
    pub descender: f32,
    pub line_gap: f32,
}

impl TextMetrics {
    pub fn line_height(&self) -> f32 {
        self.ascender - self.descender + self.line_gap
    }
}

struct State<R: Renderer> {
    composite_operation: CompositeOperationState,
    shape_antialias: bool,
    fill: Paint<R::ImageHandle>,
    stroke: Paint<R::ImageHandle>,
    stroke_width: f32,
    miter_limit: f32,
    line_join: LineJoin,
    line_cap: LineCap,
    alpha: f32,
    xform: Transform,
    scissor: Scissor,
    font_size: f32,
    letter_spacing: f32,
    line_height: f32,
    text_align: Align,
    font_id: FontId,
}

impl<R: Renderer> Clone for State<R> {
    fn clone(&self) -> Self {
        State {
            composite_operation: self.composite_operation,
            shape_antialias: self.shape_antialias,
            fill: self.fill.clone(),
            stroke: self.stroke.clone(),
            stroke_width: self.stroke_width,
            miter_limit: self.miter_limit,
            line_join: self.line_join.clone(),
            line_cap: self.line_cap.clone(),
            alpha: self.alpha,
            xform: self.xform.clone(),
            scissor: self.scissor,
            font_size: self.font_size,
            letter_spacing: self.letter_spacing,
            line_height: self.line_height,
            text_align: self.text_align,
            font_id: self.font_id,
        }
    }
}

impl<R: Renderer> Default for State<R> {
    fn default() -> Self {
        State {
            composite_operation: CompositeOperation::Basic(BasicCompositeOperation::SrcOver).into(),
            shape_antialias: true,
            fill: Color::rgb(1.0, 1.0, 1.0).into(),
            stroke: Color::rgb(0.0, 0.0, 0.0).into(),
            stroke_width: 1.0,
            miter_limit: 10.0,
            line_join: LineJoin::Miter,
            line_cap: LineCap::Butt,
            alpha: 1.0,
            xform: Transform::identity(),
            scissor: Scissor {
                xform: Default::default(),
                extent: Extent {
                    width: -1.0,
                    height: -1.0,
                },
            },
            font_size: 16.0,
            letter_spacing: 0.0,
            line_height: 1.0,
            text_align: Align::LEFT | Align::BASELINE,
            font_id: 0,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Command {
    MoveTo(Point),
    LineTo(Point),
    BezierTo(Point, Point, Point),
    Close,
    Solidity(Solidity),
}

pub struct Context<R: Renderer> {
    renderer: R,
    commands: Vec<Command>,
    last_position: Point,
    states: Vec<State<R>>,
    cache: PathCache,
    tess_tol: f32,
    dist_tol: f32,
    fringe_width: f32,
    device_pixel_ratio: f32,
    fonts: Fonts<R>,
    layout_chars: Vec<LayoutChar>,
    draw_call_count: usize,
    fill_triangles_count: usize,
    stroke_triangles_count: usize,
    text_triangles_count: usize,
}

impl<R: Renderer> Context<R> {
    pub fn create(mut renderer: R) -> anyhow::Result<Context<R>> {
        let fonts = Fonts::new(&mut renderer)?;
        Ok(Context {
            renderer,
            commands: Default::default(),
            last_position: Default::default(),
            states: vec![Default::default()],
            cache: Default::default(),
            tess_tol: 0.0,
            dist_tol: 0.0,
            fringe_width: 0.0,
            device_pixel_ratio: 0.0,
            fonts,
            layout_chars: Default::default(),
            draw_call_count: 0,
            fill_triangles_count: 0,
            stroke_triangles_count: 0,
            text_triangles_count: 0,
        })
    }

    fn set_device_pixel_ratio(&mut self, ratio: f32) {
        self.tess_tol = 0.25 / ratio;
        self.dist_tol = 0.01 / ratio;
        self.fringe_width = 1.0 / ratio;
        self.device_pixel_ratio = ratio;
    }

    pub fn begin_frame<E: Into<Extent>>(
        &mut self,
        window_extent: E,
        device_pixel_ratio: f32,
    ) -> anyhow::Result<()> {
        self.states.clear();
        self.states.push(Default::default());
        self.set_device_pixel_ratio(device_pixel_ratio);
        self.renderer
            .viewport(window_extent.into(), device_pixel_ratio)?;
        self.draw_call_count = 0;
        self.fill_triangles_count = 0;
        self.stroke_triangles_count = 0;
        self.text_triangles_count = 0;
        Ok(())
    }

    pub fn cancel_frame(&mut self) -> anyhow::Result<()> {
        self.renderer.cancel()?;
        Ok(())
    }

    pub fn end_frame(&mut self) -> anyhow::Result<()> {
        self.renderer.flush()?;
        Ok(())
    }

    pub fn save(&mut self) {
        if let Some(last) = self.states.last() {
            let last = last.clone();
            self.states.push(last);
        }
    }

    pub fn restore(&mut self) {
        if self.states.len() <= 1 {
            return;
        }
        self.states.pop();
    }

    fn state(&mut self) -> &State<R> {
        self.states.last().unwrap()
    }

    fn state_mut(&mut self) -> &mut State<R> {
        self.states.last_mut().unwrap()
    }

    pub fn reset(&mut self) {
        *self.state_mut() = Default::default();
    }

    pub fn shape_antialias(&mut self, enabled: bool) {
        self.state_mut().shape_antialias = enabled;
    }

    pub fn stroke_width(&mut self, width: f32) {
        self.state_mut().stroke_width = width;
    }

    pub fn miter_limit(&mut self, limit: f32) {
        self.state_mut().miter_limit = limit;
    }

    pub fn line_cap(&mut self, cap: LineCap) {
        self.state_mut().line_cap = cap;
    }

    pub fn line_join(&mut self, join: LineJoin) {
        self.state_mut().line_join = join;
    }

    pub fn global_alpha(&mut self, alpha: f32) {
        self.state_mut().alpha = alpha;
    }

    pub fn transform(&mut self, xform: Transform) {
        let state = self.state_mut();
        state.xform = xform * state.xform;
    }

    pub fn reset_transform(&mut self) {
        self.state_mut().xform = Transform::identity();
    }

    pub fn translate(&mut self, tx: f32, ty: f32) {
        self.transform(Transform::translate(tx, ty));
    }

    pub fn rotate(&mut self, angle: f32) {
        self.transform(Transform::rotate(angle));
    }

    pub fn skew_x(&mut self, angle: f32) {
        self.transform(Transform::skew_x(angle));
    }

    pub fn skew_y(&mut self, angle: f32) {
        self.transform(Transform::skew_y(angle));
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.transform(Transform::scale(sx, sy));
    }

    pub fn current_transform(&mut self) -> Transform {
        self.state().xform
    }

    pub fn stroke_paint<T: Into<Paint<R::ImageHandle>>>(&mut self, paint: T) {
        let mut paint = paint.into();
        paint.xform *= self.state().xform;
        self.state_mut().stroke = paint;
    }

    pub fn fill_paint<T: Into<Paint<R::ImageHandle>>>(&mut self, paint: T) {
        let mut paint = paint.into();
        paint.xform *= self.state().xform;
        self.state_mut().fill = paint;
    }

    pub fn create_image<D: AsRef<[u8]>>(
        &mut self,
        flags: ImageFlags,
        data: D,
    ) -> anyhow::Result<R::ImageHandle> {
        let img = image::load_from_memory(data.as_ref())?;
        let img = img.to_rgba();
        let dimensions = img.dimensions();
        let handle = self.renderer.create_texture(
            TextureType::RGBA,
            dimensions.0 as usize,
            dimensions.1 as usize,
            flags,
            Some(&img.into_raw()),
        )?;
        Ok(handle)
    }

    pub fn update_image(&mut self, handle: R::ImageHandle, data: &[u8]) -> anyhow::Result<()> {
        let (w, h) = self.renderer.texture_size(handle.clone())?;
        self.renderer.update_texture(handle, 0, 0, w, h, data)?;
        Ok(())
    }

    pub fn image_size(&self, handle: R::ImageHandle) -> anyhow::Result<(usize, usize)> {
        let res = self.renderer.texture_size(handle)?;
        Ok(res)
    }

    pub fn delete_image(&mut self, handle: R::ImageHandle) -> anyhow::Result<()> {
        self.renderer.delete_texture(handle)?;
        Ok(())
    }

    pub fn scissor<T: Into<Rect>>(&mut self, rect: T) {
        let rect = rect.into();
        let state = self.state_mut();
        let x = rect.xy.x;
        let y = rect.xy.y;
        let width = rect.size.width.max(0.0);
        let height = rect.size.height.max(0.0);
        state.scissor.xform = Transform::identity();
        state.scissor.xform.0[4] = x + width * 0.5;
        state.scissor.xform.0[5] = y + height * 0.5;
        state.scissor.xform *= state.xform;
        state.scissor.extent.width = width * 0.5;
        state.scissor.extent.height = height * 0.5;
    }

    pub fn intersect_scissor<T: Into<Rect>>(&mut self, rect: T) {
        let rect = rect.into();
        let state = self.state_mut();

        if state.scissor.extent.width < 0.0 {
            self.scissor(rect);
            return;
        }

        let Extent {
            width: ex,
            height: ey,
        } = state.scissor.extent;
        let invxorm = state.xform.inverse();
        let pxform = state.scissor.xform * invxorm;
        let tex = ex * pxform.0[0].abs() + ey * pxform.0[2].abs();
        let tey = ex * pxform.0[1].abs() + ey * pxform.0[3].abs();
        self.scissor(
            Rect::new(
                Point::new(pxform.0[4] - tex, pxform.0[5] - tey),
                Extent::new(tex * 2.0, tey * 2.0),
            )
            .intersect(rect),
        );
    }

    pub fn reset_scissor(&mut self) {
        let state = self.state_mut();
        state.scissor.xform = Transform::default();
        state.scissor.extent.width = -1.0;
        state.scissor.extent.height = -1.0;
    }

    pub fn global_composite_operation(&mut self, op: CompositeOperation) {
        self.state_mut().composite_operation = op.into();
    }

    fn append_command(&mut self, cmd: Command) {
        let state = self.state();
        let xform = &state.xform;
        let cmd = match cmd {
            Command::MoveTo(pt) => Command::MoveTo(xform.transform_point(pt)),
            Command::LineTo(pt) => Command::LineTo(xform.transform_point(pt)),
            Command::BezierTo(pt1, pt2, pt3) => Command::BezierTo(
                xform.transform_point(pt1),
                xform.transform_point(pt2),
                xform.transform_point(pt3),
            ),
            _ => cmd,
        };
        self.commands.push(cmd);
    }

    pub fn begin_path(&mut self) {
        self.commands.clear();
        self.cache.clear();
    }

    pub fn move_to<P: Into<Point>>(&mut self, pt: P) {
        self.append_command(Command::MoveTo(pt.into()));
    }

    pub fn line_to<P: Into<Point>>(&mut self, pt: P) {
        self.append_command(Command::LineTo(pt.into()));
    }

    pub fn bezier_to<P: Into<Point>>(&mut self, cp1: P, cp2: P, pt: P) {
        self.append_command(Command::BezierTo(cp1.into(), cp2.into(), pt.into()));
    }

    pub fn quad_to<P: Into<Point>>(&mut self, cp: P, pt: P) {
        let x0 = self.last_position.x;
        let y0 = self.last_position.y;
        let cp = cp.into();
        let pt = pt.into();
        self.append_command(Command::BezierTo(
            Point::new(x0 + 2.0 / 3.0 * (cp.x - x0), y0 + 2.0 / 3.0 * (cp.y - y0)),
            Point::new(
                pt.x + 2.0 / 3.0 * (cp.x - pt.x),
                pt.y + 2.0 / 3.0 * (cp.y - pt.y),
            ),
            pt,
        ));
    }

    pub fn arc_to<P: Into<Point>>(&mut self, pt1: P, pt2: P, radius: f32) {
        let pt0 = self.last_position;

        if self.commands.is_empty() {
            return;
        }

        let pt1 = pt1.into();
        let pt2 = pt2.into();
        if pt0.equals(pt1, self.dist_tol)
            || pt1.equals(pt2, self.dist_tol)
            || pt1.dist_pt_seg(pt0, pt2) < self.dist_tol * self.dist_tol
            || radius < self.dist_tol
        {
            self.line_to(pt1);
            return;
        }

        let d0 = Point::new(pt0.x - pt1.x, pt0.y - pt1.y);
        let d1 = Point::new(pt2.x - pt1.x, pt2.y - pt1.y);
        let a = (d0.x * d1.x + d0.y * d1.y).cos();
        let d = radius / (a / 2.0).tan();

        if d > 10000.0 {
            self.line_to(pt1);
            return;
        }

        let (cx, cy, a0, a1, dir) = if Point::cross(d0, d1) > 0.0 {
            (
                pt1.x + d0.x * d + d0.y * radius,
                pt1.y + d0.y * d + -d0.x * radius,
                d0.x.atan2(-d0.y),
                -d1.x.atan2(d1.y),
                Solidity::Hole,
            )
        } else {
            (
                pt1.x + d0.x * d + -d0.y * radius,
                pt1.y + d0.y * d + d0.x * radius,
                -d0.x.atan2(d0.y),
                d1.x.atan2(-d1.y),
                Solidity::Solid,
            )
        };

        self.arc(Point::new(cx, cy), radius, a0, a1, dir);
    }

    pub fn close_path(&mut self) {
        self.commands.push(Command::Close);
    }

    pub fn path_solidity(&mut self, dir: Solidity) {
        self.commands.push(Command::Solidity(dir));
    }

    pub fn arc<P: Into<Point>>(&mut self, cp: P, radius: f32, a0: f32, a1: f32, dir: Solidity) {
        let cp = cp.into();
        let move_ = self.commands.is_empty();

        let mut da = a1 - a0;
        if dir == Solidity::Hole {
            if da.abs() >= PI * 2.0 {
                da = PI * 2.0;
            } else {
                while da < 0.0 {
                    da += PI * 2.0;
                }
            }
        } else {
            if da.abs() >= PI * 2.0 {
                da = -PI * 2.0;
            } else {
                while da > 0.0 {
                    da -= PI * 2.0;
                }
            }
        }

        let ndivs = ((da.abs() / (PI * 0.5) + 0.5) as i32).min(5).max(1);
        let hda = (da / (ndivs as f32)) / 2.0;
        let mut kappa = (4.0 / 3.0 * (1.0 - hda.cos()) / hda.sin()).abs();

        if dir == Solidity::Solid {
            kappa = -kappa;
        }

        let mut px = 0.0;
        let mut py = 0.0;
        let mut ptanx = 0.0;
        let mut ptany = 0.0;

        for i in 0..=ndivs {
            let a = a0 + da * ((i as f32) / (ndivs as f32));
            let dx = a.cos();
            let dy = a.sin();
            let x = cp.x + dx * radius;
            let y = cp.y + dy * radius;
            let tanx = -dy * radius * kappa;
            let tany = dx * radius * kappa;

            if i == 0 {
                if move_ {
                    self.append_command(Command::MoveTo(Point::new(x, y)));
                } else {
                    self.append_command(Command::LineTo(Point::new(x, y)));
                }
            } else {
                self.append_command(Command::BezierTo(
                    Point::new(px + ptanx, py + ptany),
                    Point::new(x - tanx, y - tany),
                    Point::new(x, y),
                ));
            }
            px = x;
            py = y;
            ptanx = tanx;
            ptany = tany;
        }
    }

    pub fn rect<T: Into<Rect>>(&mut self, rect: T) {
        let rect = rect.into();
        self.append_command(Command::MoveTo(Point::new(rect.xy.x, rect.xy.y)));
        self.append_command(Command::LineTo(Point::new(
            rect.xy.x,
            rect.xy.y + rect.size.height,
        )));
        self.append_command(Command::LineTo(Point::new(
            rect.xy.x + rect.size.width,
            rect.xy.y + rect.size.height,
        )));
        self.append_command(Command::LineTo(Point::new(
            rect.xy.x + rect.size.width,
            rect.xy.y,
        )));
        self.append_command(Command::Close);
    }

    pub fn rounded_rect<T: Into<Rect>>(&mut self, rect: T, radius: f32) {
        let rect = rect.into();
        self.rounded_rect_varying(rect, radius, radius, radius, radius);
    }

    pub fn rounded_rect_varying<T: Into<Rect>>(
        &mut self,
        rect: T,
        lt: f32,
        rt: f32,
        rb: f32,
        lb: f32,
    ) {
        let rect = rect.into();
        if lt < 0.1 && rt < 0.1 && lb < 0.1 && rb < 0.1 {
            self.rect(rect);
        } else {
            let halfw = rect.size.width.abs() * 0.5;
            let halfh = rect.size.height.abs() * 0.5;
            let rxlb = lb.min(halfw) * rect.size.width.signum();
            let rylb = lb.min(halfh) * rect.size.height.signum();
            let rxrb = rb.min(halfw) * rect.size.width.signum();
            let ryrb = rb.min(halfh) * rect.size.height.signum();
            let rxrt = rt.min(halfw) * rect.size.width.signum();
            let ryrt = rt.min(halfh) * rect.size.height.signum();
            let rxlt = lt.min(halfw) * rect.size.width.signum();
            let rylt = lt.min(halfh) * rect.size.height.signum();

            self.append_command(Command::MoveTo(Point::new(rect.xy.x, rect.xy.y + rylt)));
            self.append_command(Command::LineTo(Point::new(
                rect.xy.x,
                rect.xy.y + rect.size.height - rylb,
            )));
            self.append_command(Command::BezierTo(
                Point::new(
                    rect.xy.x,
                    rect.xy.y + rect.size.height - rylb * (1.0 - KAPPA90),
                ),
                Point::new(
                    rect.xy.x + rxlb * (1.0 - KAPPA90),
                    rect.xy.y + rect.size.height,
                ),
                Point::new(rect.xy.x + rxlb, rect.xy.y + rect.size.height),
            ));
            self.append_command(Command::LineTo(Point::new(
                rect.xy.x + rect.size.width - rxrb,
                rect.xy.y + rect.size.height,
            )));
            self.append_command(Command::BezierTo(
                Point::new(
                    rect.xy.x + rect.size.width - rxrb * (1.0 - KAPPA90),
                    rect.xy.y + rect.size.height,
                ),
                Point::new(
                    rect.xy.x + rect.size.width,
                    rect.xy.y + rect.size.height - ryrb * (1.0 - KAPPA90),
                ),
                Point::new(
                    rect.xy.x + rect.size.width,
                    rect.xy.y + rect.size.height - ryrb,
                ),
            ));
            self.append_command(Command::LineTo(Point::new(
                rect.xy.x + rect.size.width,
                rect.xy.y + ryrt,
            )));
            self.append_command(Command::BezierTo(
                Point::new(
                    rect.xy.x + rect.size.width,
                    rect.xy.y + ryrt * (1.0 - KAPPA90),
                ),
                Point::new(
                    rect.xy.x + rect.size.width - rxrt * (1.0 - KAPPA90),
                    rect.xy.y,
                ),
                Point::new(rect.xy.x + rect.size.width - rxrt, rect.xy.y),
            ));
            self.append_command(Command::LineTo(Point::new(rect.xy.x + rxlt, rect.xy.y)));
            self.append_command(Command::BezierTo(
                Point::new(rect.xy.x + rxlt * (1.0 - KAPPA90), rect.xy.y),
                Point::new(rect.xy.x, rect.xy.y + rylt * (1.0 - KAPPA90)),
                Point::new(rect.xy.x, rect.xy.y + rylt),
            ));
            self.append_command(Command::Close);
        }
    }

    pub fn ellipse<P: Into<Point>>(&mut self, center: P, radius_x: f32, radius_y: f32) {
        let center = center.into();
        self.append_command(Command::MoveTo(Point::new(center.x - radius_x, center.y)));
        self.append_command(Command::BezierTo(
            Point::new(center.x - radius_x, center.y + radius_y * KAPPA90),
            Point::new(center.x - radius_x * KAPPA90, center.y + radius_y),
            Point::new(center.x, center.y + radius_y),
        ));
        self.append_command(Command::BezierTo(
            Point::new(center.x + radius_x * KAPPA90, center.y + radius_y),
            Point::new(center.x + radius_x, center.y + radius_y * KAPPA90),
            Point::new(center.x + radius_x, center.y),
        ));
        self.append_command(Command::BezierTo(
            Point::new(center.x + radius_x, center.y - radius_y * KAPPA90),
            Point::new(center.x + radius_x * KAPPA90, center.y - radius_y),
            Point::new(center.x, center.y - radius_y),
        ));
        self.append_command(Command::BezierTo(
            Point::new(center.x - radius_x * KAPPA90, center.y - radius_y),
            Point::new(center.x - radius_x, center.y - radius_y * KAPPA90),
            Point::new(center.x - radius_x, center.y),
        ));
        self.append_command(Command::Close);
    }

    pub fn circle<P: Into<Point>>(&mut self, center: P, radius: f32) {
        self.ellipse(center.into(), radius, radius);
    }

    pub fn fill(&mut self) -> anyhow::Result<()> {
        let state = self.states.last_mut().unwrap();
        let mut fill_paint = state.fill.clone();

        self.cache
            .flatten_paths(&self.commands, self.dist_tol, self.tess_tol);
        if self.renderer.edge_antialias() && state.shape_antialias {
            self.cache
                .expand_fill(self.fringe_width, LineJoin::Miter, 2.4, self.fringe_width);
        } else {
            self.cache
                .expand_fill(0.0, LineJoin::Miter, 2.4, self.fringe_width);
        }

        fill_paint.inner_color.a *= state.alpha;
        fill_paint.outer_color.a *= state.alpha;

        self.renderer.fill(
            &fill_paint,
            state.composite_operation,
            &state.scissor,
            self.fringe_width,
            self.cache.bounds,
            &self.cache.paths,
        )?;

        for path in &self.cache.paths {
            if path.num_fill > 2 {
                self.fill_triangles_count += path.num_fill - 2;
            }
            if path.num_stroke > 2 {
                self.fill_triangles_count += path.num_stroke - 2;
            }
            self.draw_call_count += 2;
        }

        Ok(())
    }

    pub fn stroke(&mut self) -> anyhow::Result<()> {
        let state = self.states.last_mut().unwrap();
        let scale = state.xform.average_scale();
        let mut stroke_width = (state.stroke_width * scale).clamped(0.0, 200.0);
        let mut stroke_paint = state.stroke.clone();

        if stroke_width < self.fringe_width {
            let alpha = (stroke_width / self.fringe_width).clamped(0.0, 1.0);
            stroke_paint.inner_color.a *= alpha * alpha;
            stroke_paint.outer_color.a *= alpha * alpha;
            stroke_width = self.fringe_width;
        }

        stroke_paint.inner_color.a *= state.alpha;
        stroke_paint.outer_color.a *= state.alpha;

        self.cache
            .flatten_paths(&self.commands, self.dist_tol, self.tess_tol);

        if self.renderer.edge_antialias() && state.shape_antialias {
            self.cache.expand_stroke(
                stroke_width * 0.5,
                self.fringe_width,
                state.line_cap,
                state.line_join,
                state.miter_limit,
                self.tess_tol,
            );
        } else {
            self.cache.expand_stroke(
                stroke_width * 0.5,
                0.0,
                state.line_cap,
                state.line_join,
                state.miter_limit,
                self.tess_tol,
            );
        }

        self.renderer.stroke(
            &stroke_paint,
            state.composite_operation,
            &state.scissor,
            self.fringe_width,
            stroke_width,
            &self.cache.paths,
        )?;

        for path in &self.cache.paths {
            self.fill_triangles_count += path.num_stroke - 2;
            self.draw_call_count += 1;
        }

        Ok(())
    }

    pub fn create_font_from_file<N: Into<String>, P: AsRef<std::path::Path>>(
        &mut self,
        name: N,
        path: P,
    ) -> anyhow::Result<FontId> {
        self.create_font(name, std::fs::read(path)?)
    }

    pub fn create_font<N: Into<String>, D: Into<Vec<u8>>>(
        &mut self,
        name: N,
        data: D,
    ) -> anyhow::Result<FontId> {
        self.fonts.add_font(name, data)
    }

    pub fn find_font<N: AsRef<str>>(&self, name: N) -> Option<FontId> {
        self.fonts.find(name.as_ref())
    }

    pub fn add_fallback_fontid(&mut self, base: FontId, fallback: FontId) {
        self.fonts.add_fallback(base, fallback);
    }

    pub fn add_fallback_font<N1: AsRef<str>, N2: AsRef<str>>(&mut self, base: N1, fallback: N2) {
        if let (Some(base), Some(fallback)) = (self.find_font(base), self.find_font(fallback)) {
            self.fonts.add_fallback(base, fallback);
        }
    }

    pub fn font_size(&mut self, size: f32) {
        self.state_mut().font_size = size;
    }

    pub fn text_letter_spacing(&mut self, spacing: f32) {
        self.state_mut().letter_spacing = spacing;
    }

    pub fn text_line_height(&mut self, line_height: f32) {
        self.state_mut().line_height = line_height;
    }

    pub fn text_align(&mut self, align: Align) {
        self.state_mut().text_align = align;
    }

    pub fn fontid(&mut self, id: FontId) {
        self.state_mut().font_id = id;
    }

    pub fn font<N: AsRef<str>>(&mut self, name: N) {
        if let Some(id) = self.find_font(name) {
            self.state_mut().font_id = id;
        }
    }

    pub fn text<S: AsRef<str>, P: Into<Point>>(&mut self, pt: P, text: S) -> anyhow::Result<()> {
        let state = self.states.last().unwrap();
        let scale = state.xform.font_scale() * self.device_pixel_ratio;
        let invscale = 1.0 / scale;
        let pt = pt.into();

        self.fonts.layout_text(
            &mut self.renderer,
            text.as_ref(),
            state.font_id,
            (pt.x * scale, pt.y * scale).into(),
            state.font_size * scale,
            state.text_align,
            state.letter_spacing * scale,
            true,
            &mut self.layout_chars,
        )?;

        self.cache.vertexes.clear();

        for lc in &self.layout_chars {
            let lt = Point::new(lc.bounds.min.x * invscale, lc.bounds.min.y * invscale);
            let rt = Point::new(lc.bounds.max.x * invscale, lc.bounds.min.y * invscale);
            let lb = Point::new(lc.bounds.min.x * invscale, lc.bounds.max.y * invscale);
            let rb = Point::new(lc.bounds.max.x * invscale, lc.bounds.max.y * invscale);

            self.cache
                .vertexes
                .push(Vertex::new(lt.x, lt.y, lc.uv.min.x, lc.uv.min.y));
            self.cache
                .vertexes
                .push(Vertex::new(rb.x, rb.y, lc.uv.max.x, lc.uv.max.y));
            self.cache
                .vertexes
                .push(Vertex::new(rt.x, rt.y, lc.uv.max.x, lc.uv.min.y));

            self.cache
                .vertexes
                .push(Vertex::new(lt.x, lt.y, lc.uv.min.x, lc.uv.min.y));
            self.cache
                .vertexes
                .push(Vertex::new(lb.x, lb.y, lc.uv.min.x, lc.uv.max.y));
            self.cache
                .vertexes
                .push(Vertex::new(rb.x, rb.y, lc.uv.max.x, lc.uv.max.y));
        }

        let mut paint = state.fill.clone();
        paint.image = Some(self.fonts.img.clone());
        paint.inner_color.a *= state.alpha;
        paint.outer_color.a *= state.alpha;

        self.renderer.triangles(
            &paint,
            state.composite_operation,
            &state.scissor,
            &self.cache.vertexes,
        )?;
        Ok(())
    }

    pub fn text_metrics(&self) -> TextMetrics {
        let state = self.states.last().unwrap();
        let scale = state.xform.font_scale() * self.device_pixel_ratio;
        self.fonts
            .text_metrics(state.font_id, state.font_size * scale)
    }

    pub fn text_size<S: AsRef<str>>(&self, text: S) -> Extent {
        let state = self.states.last().unwrap();
        let scale = state.xform.font_scale() * self.device_pixel_ratio;
        self.fonts.text_size(
            text.as_ref(),
            state.font_id,
            state.font_size * scale,
            state.letter_spacing * scale,
        )
    }
}
