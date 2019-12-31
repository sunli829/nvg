use clamped::Clamp;
use num_traits::AsPrimitive;
use std::ops::Rem;

#[derive(Debug, Copy, Clone, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    pub fn rgb(r: f32, g: f32, b: f32) -> Color {
        Color { r, g, b, a: 1.0 }
    }

    pub fn rgba_i(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub fn rgb_i(r: u8, g: u8, b: u8) -> Color {
        Self::rgba_i(r, g, b, 255)
    }

    pub fn lerp(self, c: Color, u: f32) -> Color {
        let u = u.clamped(0.0, 1.0);
        let om = 1.0 - u;
        Color {
            r: self.r * om + c.r * u,
            g: self.g * om + c.g * u,
            b: self.b * om + c.b * u,
            a: self.a * om + c.a * u,
        }
    }

    pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color {
        let mut h = h.rem(1.0);
        if h < 0.0 {
            h += 1.0;
        }
        let s = s.clamped(0.0, 1.0);
        let l = l.clamped(0.0, 1.0);
        let m2 = if l <= 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let m1 = 2.0 * l - m2;
        Color {
            r: hue(h + 1.0 / 3.0, m1, m2).clamped(0.0, 1.0),
            g: hue(h, m1, m2).clamped(0.0, 1.0),
            b: hue(h - 1.0 / 3.0, m1, m2).clamped(0.0, 1.0),
            a,
        }
    }

    pub fn hsl(h: f32, s: f32, l: f32) -> Color {
        Self::hsla(h, s, l, 1.0)
    }
}

impl<T: AsPrimitive<f32>> From<(T, T, T)> for Color {
    fn from((r, g, b): (T, T, T)) -> Self {
        Color::rgb(r.as_(), g.as_(), b.as_())
    }
}

impl<T: AsPrimitive<f32>> From<(T, T, T, T)> for Color {
    fn from((r, g, b, a): (T, T, T, T)) -> Self {
        Color::rgba(r.as_(), g.as_(), b.as_(), a.as_())
    }
}

fn hue(mut h: f32, m1: f32, m2: f32) -> f32 {
    if h < 0.0 {
        h += 1.0;
    }
    if h > 1.0 {
        h -= 1.0
    };
    if h < 1.0 / 6.0 {
        return m1 + (m2 - m1) * h * 6.0;
    } else if h < 3.0 / 6.0 {
        m2
    } else if h < 4.0 / 6.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - h) * 6.0
    } else {
        m1
    }
}
