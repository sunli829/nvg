#[macro_use]
extern crate bitflags;

mod cache;
mod color;
mod context;
mod fonts;
mod math;
pub mod renderer;

pub use color::*;
pub use context::{
    Align, BasicCompositeOperation, BlendFactor, CompositeOperation, Context, Gradient, ImageFlags,
    ImageId, ImagePattern, LineCap, LineJoin, Paint, Solidity, TextMetrics,
};
pub use fonts::FontId;
pub use math::*;
pub use renderer::Renderer;
