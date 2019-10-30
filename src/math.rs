use num_traits::AsPrimitive;
use std::ops::{Mul, MulAssign};

#[derive(Debug, Copy, Clone, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    pub(crate) fn equals(self, pt: Point, tol: f32) -> bool {
        let dx = pt.x - self.x;
        let dy = pt.y - self.y;
        dx * dx + dy * dy < tol * tol
    }

    pub(crate) fn dist_pt_seg(self, p: Point, q: Point) -> f32 {
        let pqx = q.x - p.x;
        let pqy = q.y - p.y;
        let dx = self.x - p.x;
        let dy = self.y - p.y;
        let d = pqx * pqx + pqy * pqy;
        let mut t = pqx * dx + pqy * dy;
        if d > 0.0 {
            t /= d;
        }
        if t < 0.0 {
            t = 0.0
        } else if t > 1.0 {
            t = 1.0
        };
        let dx = p.x + t * pqx - self.x;
        let dy = p.y + t * pqy - self.y;
        dx * dx + dy * dy
    }

    pub(crate) fn normalize(&mut self) -> f32 {
        let d = ((self.x) * (self.x) + (self.y) * (self.y)).sqrt();
        if d > 1e-6 {
            let id = 1.0 / d;
            self.x *= id;
            self.y *= id;
        }
        d
    }

    pub(crate) fn cross(pt1: Point, pt2: Point) -> f32 {
        pt2.x * pt1.y - pt1.x * pt2.y
    }

    pub fn offset(&self, tx: f32, ty: f32) -> Point {
        Point::new(self.x + tx, self.y + ty)
    }
}

impl<T: AsPrimitive<f32>> From<(T, T)> for Point {
    fn from((x, y): (T, T)) -> Self {
        Point::new(x.as_(), y.as_())
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Extent {
    pub width: f32,
    pub height: f32,
}

impl Extent {
    pub fn new(width: f32, height: f32) -> Extent {
        Extent { width, height }
    }
}

impl<T: AsPrimitive<f32>> From<(T, T)> for Extent {
    fn from((width, height): (T, T)) -> Self {
        Extent::new(width.as_(), height.as_())
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Rect {
    pub xy: Point,
    pub size: Extent,
}

impl Rect {
    pub fn new(xy: Point, size: Extent) -> Rect {
        Rect { xy, size }
    }

    pub fn intersect(self, rect: Rect) -> Rect {
        let Rect {
            xy: Point { x: ax, y: ay },
            size: Extent {
                width: aw,
                height: ah,
            },
        } = rect;

        let Rect {
            xy: Point { x: bx, y: by },
            size: Extent {
                width: bw,
                height: bh,
            },
        } = rect;

        let minx = ax.max(bx);
        let miny = ay.max(by);
        let maxx = (ax + aw).min(bx + bw);
        let maxy = (ay + ah).min(by + bh);
        Self::new(
            Point::new(minx, miny),
            Extent::new((maxx - minx).max(0.0), (maxy - miny).max(0.0)),
        )
    }

    pub fn grow(&self, width: f32, height: f32) -> Rect {
        Rect::new(
            self.xy.offset(-width / 2.0, -height / 2.0),
            Extent::new(self.size.width + width, self.size.height + height),
        )
    }
}

impl<T: AsPrimitive<f32>> From<(T, T, T, T)> for Rect {
    fn from((x, y, w, h): (T, T, T, T)) -> Self {
        Rect::new((x.as_(), y.as_()).into(), (w.as_(), h.as_()).into())
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn left_top(&self) -> Point {
        self.min
    }

    pub fn right_top(&self) -> Point {
        Point::new(self.max.x, self.min.y)
    }

    pub fn left_bottom(&self) -> Point {
        Point::new(self.min.x, self.max.y)
    }

    pub fn right_bottom(&self) -> Point {
        self.max
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Transform(pub [f32; 6]);

impl Transform {
    pub fn identity() -> Transform {
        Transform([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    }

    pub fn translate(tx: f32, ty: f32) -> Transform {
        Transform([1.0, 0.0, 0.0, 1.0, tx, ty])
    }

    pub fn scale(sx: f32, sy: f32) -> Transform {
        Transform([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }

    pub fn rotate(a: f32) -> Transform {
        let cs = a.cos();
        let sn = a.sin();
        Transform([cs, sn, -sn, cs, 0.0, 0.0])
    }

    pub fn skew_x(a: f32) -> Transform {
        Transform([1.0, 0.0, a.tan(), 1.0, 0.0, 0.0])
    }

    pub fn skew_y(a: f32) -> Transform {
        Transform([1.0, a.tan(), 0.0, 1.0, 0.0, 0.0])
    }

    pub fn pre_multiply(self, rhs: Self) -> Self {
        rhs * self
    }

    pub fn inverse(self) -> Transform {
        let t = &self.0;
        let det = t[0] * t[3] - t[2] * t[1];
        if det > -1e-6 && det < 1e-6 {
            return Transform::identity();
        }
        let invdet = 1.0 / det;
        let mut inv = [0f32; 6];
        inv[0] = t[3] * invdet;
        inv[2] = -t[2] * invdet;
        inv[4] = (t[2] * t[5] - t[3] * t[4]) * invdet;
        inv[1] = -t[1] * invdet;
        inv[3] = t[0] * invdet;
        inv[5] = (t[1] * t[4] - t[0] * t[5]) * invdet;
        Transform(inv)
    }

    pub fn transform_point(&self, pt: Point) -> Point {
        let t = &self.0;
        Point::new(
            pt.x * t[0] + pt.y * t[2] + t[4],
            pt.x * t[1] + pt.y * t[3] + t[5],
        )
    }

    pub(crate) fn average_scale(&self) -> f32 {
        let t = &self.0;
        let sx = (t[0] * t[0] + t[2] * t[2]).sqrt();
        let sy = (t[1] * t[1] + t[3] * t[3]).sqrt();
        (sx + sy) * 0.5
    }

    pub(crate) fn font_scale(&self) -> f32 {
        let a = self.average_scale();
        let d = 0.01f32;
        (a / d).ceil() * d
    }
}

impl Mul for Transform {
    type Output = Transform;

    fn mul(mut self, rhs: Self) -> Self::Output {
        let t = &mut self.0;
        let s = &rhs.0;
        let t0 = t[0] * s[0] + t[1] * s[2];
        let t2 = t[2] * s[0] + t[3] * s[2];
        let t4 = t[4] * s[0] + t[5] * s[2] + s[4];
        t[1] = t[0] * s[1] + t[1] * s[3];
        t[3] = t[2] * s[1] + t[3] * s[3];
        t[5] = t[4] * s[1] + t[5] * s[3] + s[5];
        t[0] = t0;
        t[2] = t2;
        t[4] = t4;
        self
    }
}

impl MulAssign for Transform {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl<T: AsPrimitive<f32>> From<(T, T, T, T, T, T)> for Transform {
    fn from((a1, a2, a3, a4, a5, a6): (T, T, T, T, T, T)) -> Self {
        Transform([a1.as_(), a2.as_(), a3.as_(), a4.as_(), a5.as_(), a6.as_()])
    }
}

impl<T: AsPrimitive<f32>> From<[T; 6]> for Transform {
    fn from(values: [T; 6]) -> Self {
        let mut values2 = [0.0; 6];
        for i in 0..6 {
            values2[i] = values[i].as_();
        }
        Transform(values2)
    }
}
