pub use crate::context::{CompositeOperationState, Path, Vertex};
pub use crate::*;

#[derive(Debug, Copy, Clone)]
pub enum TextureType {
    RGBA,
    Alpha,
}

#[derive(Debug, Copy, Clone)]
pub struct Scissor {
    pub xform: Transform,
    pub extent: Extent,
}

#[derive(Debug)]
pub enum RendererError {
    TextureNotFound,
    SystemError(failure::Error),
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RendererError::TextureNotFound => write!(f, "texture not found"),
            RendererError::SystemError(err) => write!(f, "renderer error: {}", err),
        }
    }
}

impl std::error::Error for RendererError {}

pub type RendererResult<T> = std::result::Result<T, RendererError>;

pub trait Renderer
where
    Self::ImageHandle: Clone,
{
    type ImageHandle;

    fn edge_antialias(&self) -> bool;

    fn create_texture(
        &mut self,
        texture_type: TextureType,
        width: usize,
        height: usize,
        flags: ImageFlags,
        data: Option<&[u8]>,
    ) -> RendererResult<Self::ImageHandle>;

    fn delete_texture(&mut self, handle: Self::ImageHandle) -> RendererResult<()>;

    fn update_texture(
        &mut self,
        handle: Self::ImageHandle,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        data: &[u8],
    ) -> RendererResult<()>;

    fn texture_size(&self, handle: Self::ImageHandle) -> RendererResult<(usize, usize)>;

    fn viewport(&mut self, extent: Extent, device_pixel_ratio: f32) -> RendererResult<()>;

    fn cancel(&mut self) -> RendererResult<()>;

    fn flush(&mut self) -> RendererResult<()>;

    fn fill(
        &mut self,
        paint: &Paint<Self::ImageHandle>,
        composite_operation: CompositeOperationState,
        scissor: &Scissor,
        fringe: f32,
        bounds: Bounds,
        paths: &[Path],
    ) -> RendererResult<()>;

    fn stroke(
        &mut self,
        paint: &Paint<Self::ImageHandle>,
        composite_operation: CompositeOperationState,
        scissor: &Scissor,
        fringe: f32,
        stroke_width: f32,
        paths: &[Path],
    ) -> RendererResult<()>;

    fn triangles(
        &mut self,
        paint: &Paint<Self::ImageHandle>,
        composite_operation: CompositeOperationState,
        scissor: &Scissor,
        vertexes: &[Vertex],
    ) -> RendererResult<()>;
}
