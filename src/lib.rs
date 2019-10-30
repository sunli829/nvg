#![feature(clamp)]
#![feature(ptr_offset_from)]

#[macro_use]
extern crate bitflags;

mod cache;
mod color;
mod context;
mod fonts;
mod math;
pub mod renderer;
mod result;

pub use color::*;
pub use context::{
    Align, BlendFactor, CompositeOperation, Context, Gradient, ImageFlags, ImagePattern, LineCap,
    LineJoin, Paint, Solidity, TextMetrics,
};
pub use fonts::FontId;
pub use math::*;
pub use renderer::Renderer;
pub use result::*;
