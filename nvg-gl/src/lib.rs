#[macro_use]
extern crate anyhow;

use nvg::renderer::*;
use slab::Slab;
use std::ffi::c_void;

struct Shader {
    prog: gl::types::GLuint,
    frag: gl::types::GLuint,
    vert: gl::types::GLuint,
    loc_viewsize: i32,
    loc_tex: i32,
    loc_frag: u32,
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.prog);
            gl::DeleteShader(self.vert);
            gl::DeleteShader(self.frag);
        }
    }
}

impl Shader {
    unsafe fn load() -> anyhow::Result<Shader> {
        let mut status: gl::types::GLint = std::mem::zeroed();
        let prog = gl::CreateProgram();
        let vert = gl::CreateShader(gl::VERTEX_SHADER);
        let frag = gl::CreateShader(gl::FRAGMENT_SHADER);
        let vert_source =
            std::ffi::CString::from_vec_unchecked(include_bytes!("shader.vert").to_vec());
        let frag_source =
            std::ffi::CString::from_vec_unchecked(include_bytes!("shader.frag").to_vec());

        gl::ShaderSource(
            vert,
            1,
            [vert_source.as_ptr()].as_ptr() as *const *const i8,
            std::ptr::null(),
        );
        gl::ShaderSource(
            frag,
            1,
            [frag_source.as_ptr()].as_ptr() as *const *const i8,
            std::ptr::null(),
        );

        gl::CompileShader(vert);
        gl::GetShaderiv(vert, gl::COMPILE_STATUS, &mut status);
        if status != gl::TRUE as i32 {
            return Err(shader_error(vert, "shader.vert"));
        }

        gl::CompileShader(frag);
        gl::GetShaderiv(frag, gl::COMPILE_STATUS, &mut status);
        if status != gl::TRUE as i32 {
            return Err(shader_error(vert, "shader.frag"));
        }

        gl::AttachShader(prog, vert);
        gl::AttachShader(prog, frag);

        let name_vertex = std::ffi::CString::new("vertex").unwrap();
        let name_tcoord = std::ffi::CString::new("tcoord").unwrap();
        gl::BindAttribLocation(prog, 0, name_vertex.as_ptr() as *const i8);
        gl::BindAttribLocation(prog, 1, name_tcoord.as_ptr() as *const i8);

        gl::LinkProgram(prog);
        gl::GetProgramiv(prog, gl::LINK_STATUS, &mut status);
        if status != gl::TRUE as i32 {
            return Err(program_error(prog));
        }

        let name_viewsize = std::ffi::CString::new("viewSize").unwrap();
        let name_tex = std::ffi::CString::new("tex").unwrap();
        let name_frag = std::ffi::CString::new("frag").unwrap();

        Ok(Shader {
            prog,
            frag,
            vert,
            loc_viewsize: gl::GetUniformLocation(prog, name_viewsize.as_ptr() as *const i8),
            loc_tex: gl::GetUniformLocation(prog, name_tex.as_ptr() as *const i8),
            loc_frag: gl::GetUniformBlockIndex(prog, name_frag.as_ptr() as *const i8),
        })
    }
}

enum ShaderType {
    FillGradient,
    FillImage,
    Simple,
    Image,
}

#[derive(PartialEq, Eq)]
enum CallType {
    Fill,
    ConvexFill,
    Stroke,
    Triangles,
}

struct Blend {
    src_rgb: gl::types::GLenum,
    dst_rgb: gl::types::GLenum,
    src_alpha: gl::types::GLenum,
    dst_alpha: gl::types::GLenum,
}

impl From<CompositeOperationState> for Blend {
    fn from(state: CompositeOperationState) -> Self {
        Blend {
            src_rgb: convert_blend_factor(state.src_rgb),
            dst_rgb: convert_blend_factor(state.dst_rgb),
            src_alpha: convert_blend_factor(state.src_alpha),
            dst_alpha: convert_blend_factor(state.dst_alpha),
        }
    }
}

struct Call {
    call_type: CallType,
    image: Option<usize>,
    path_offset: usize,
    path_count: usize,
    triangle_offset: usize,
    triangle_count: usize,
    uniform_offset: usize,
    blend_func: Blend,
}

struct Texture {
    tex: gl::types::GLuint,
    width: usize,
    height: usize,
    texture_type: TextureType,
    flags: ImageFlags,
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &self.tex) }
    }
}

struct GLPath {
    fill_offset: usize,
    fill_count: usize,
    stroke_offset: usize,
    stroke_count: usize,
}

#[derive(Default)]
#[allow(dead_code)]
struct FragUniforms {
    scissor_mat: [f32; 12],
    paint_mat: [f32; 12],
    inner_color: Color,
    outer_color: Color,
    scissor_ext: [f32; 2],
    scissor_scale: [f32; 2],
    extent: [f32; 2],
    radius: f32,
    feather: f32,
    stroke_mult: f32,
    stroke_thr: f32,
    tex_type: i32,
    type_: i32,
}

pub struct Renderer {
    shader: Shader,
    textures: Slab<Texture>,
    view: Extent,
    vert_buf: gl::types::GLuint,
    vert_arr: gl::types::GLuint,
    frag_buf: gl::types::GLuint,
    frag_size: usize,
    calls: Vec<Call>,
    paths: Vec<GLPath>,
    vertexes: Vec<Vertex>,
    uniforms: Vec<u8>,
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.frag_buf);
            gl::DeleteBuffers(1, &self.vert_buf);
            gl::DeleteVertexArrays(1, &self.vert_arr);
        }
    }
}

impl Renderer {
    pub fn create() -> anyhow::Result<Renderer> {
        unsafe {
            let shader = Shader::load()?;

            let mut vert_arr: gl::types::GLuint = std::mem::zeroed();
            gl::GenVertexArrays(1, &mut vert_arr);

            let mut vert_buf: gl::types::GLuint = std::mem::zeroed();
            gl::GenBuffers(1, &mut vert_buf);

            gl::UniformBlockBinding(shader.prog, shader.loc_frag, 0);
            let mut frag_buf: gl::types::GLuint = std::mem::zeroed();
            gl::GenBuffers(1, &mut frag_buf);

            let mut align = std::mem::zeroed();
            gl::GetIntegerv(gl::UNIFORM_BUFFER_OFFSET_ALIGNMENT, &mut align);

            let frag_size = std::mem::size_of::<FragUniforms>() + (align as usize)
                - std::mem::size_of::<FragUniforms>() % (align as usize);

            gl::Finish();

            Ok(Renderer {
                shader,
                textures: Default::default(),
                view: Default::default(),
                vert_buf,
                vert_arr,
                frag_buf,
                frag_size,
                calls: Default::default(),
                paths: Default::default(),
                vertexes: Default::default(),
                uniforms: Default::default(),
            })
        }
    }

    unsafe fn set_uniforms(&self, offset: usize, img: Option<usize>) {
        gl::BindBufferRange(
            gl::UNIFORM_BUFFER,
            0,
            self.frag_buf,
            (offset * self.frag_size) as isize,
            std::mem::size_of::<FragUniforms>() as isize,
        );

        if let Some(img) = img {
            if let Some(texture) = self.textures.get(img) {
                gl::BindTexture(gl::TEXTURE_2D, texture.tex);
            }
        } else {
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
    }

    unsafe fn do_fill(&self, call: &Call) {
        let paths = &self.paths[call.path_offset..call.path_offset + call.path_count];

        gl::Enable(gl::STENCIL_TEST);
        gl::StencilMask(0xff);
        gl::StencilFunc(gl::ALWAYS, 0, 0xff);
        gl::ColorMask(gl::FALSE, gl::FALSE, gl::FALSE, gl::FALSE);

        self.set_uniforms(call.uniform_offset, call.image);

        gl::StencilOpSeparate(gl::FRONT, gl::KEEP, gl::KEEP, gl::INCR_WRAP);
        gl::StencilOpSeparate(gl::BACK, gl::KEEP, gl::KEEP, gl::DECR_WRAP);
        gl::Disable(gl::CULL_FACE);
        for path in paths {
            gl::DrawArrays(
                gl::TRIANGLE_FAN,
                path.fill_offset as i32,
                path.fill_count as i32,
            );
        }
        gl::Enable(gl::CULL_FACE);

        gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);

        self.set_uniforms(call.uniform_offset + 1, call.image);

        gl::StencilFunc(gl::EQUAL, 0x00, 0xff);
        gl::StencilOp(gl::KEEP, gl::KEEP, gl::KEEP);
        for path in paths {
            gl::DrawArrays(
                gl::TRIANGLE_STRIP,
                path.stroke_offset as i32,
                path.stroke_count as i32,
            );
        }

        gl::StencilFunc(gl::NOTEQUAL, 0x00, 0xff);
        gl::StencilOp(gl::ZERO, gl::ZERO, gl::ZERO);
        gl::DrawArrays(
            gl::TRIANGLE_STRIP,
            call.triangle_offset as i32,
            call.triangle_count as i32,
        );

        gl::Disable(gl::STENCIL_TEST);
    }

    unsafe fn do_convex_fill(&self, call: &Call) {
        let paths = &self.paths[call.path_offset..call.path_offset + call.path_count];
        self.set_uniforms(call.uniform_offset, call.image);
        for path in paths {
            gl::DrawArrays(
                gl::TRIANGLE_FAN,
                path.fill_offset as i32,
                path.fill_count as i32,
            );
            if path.stroke_count > 0 {
                gl::DrawArrays(
                    gl::TRIANGLE_STRIP,
                    path.stroke_offset as i32,
                    path.stroke_count as i32,
                );
            }
        }
    }

    unsafe fn do_stroke(&self, call: &Call) {
        let paths = &self.paths[call.path_offset..call.path_offset + call.path_count];

        gl::Enable(gl::STENCIL_TEST);
        gl::StencilMask(0xff);
        gl::StencilFunc(gl::EQUAL, 0x0, 0xff);
        gl::StencilOp(gl::KEEP, gl::KEEP, gl::INCR);
        self.set_uniforms(call.uniform_offset + 1, call.image);
        for path in paths {
            gl::DrawArrays(
                gl::TRIANGLE_STRIP,
                path.stroke_offset as i32,
                path.stroke_count as i32,
            );
        }

        self.set_uniforms(call.uniform_offset, call.image);
        gl::StencilFunc(gl::EQUAL, 0x0, 0xff);
        gl::StencilOp(gl::KEEP, gl::KEEP, gl::KEEP);
        for path in paths {
            gl::DrawArrays(
                gl::TRIANGLE_STRIP,
                path.stroke_offset as i32,
                path.stroke_count as i32,
            );
        }

        gl::ColorMask(gl::FALSE, gl::FALSE, gl::FALSE, gl::FALSE);
        gl::StencilFunc(gl::ALWAYS, 0x0, 0xff);
        gl::StencilOp(gl::ZERO, gl::ZERO, gl::ZERO);
        for path in paths {
            gl::DrawArrays(
                gl::TRIANGLE_STRIP,
                path.stroke_offset as i32,
                path.stroke_count as i32,
            );
        }
        gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);

        gl::Disable(gl::STENCIL_TEST);
    }

    unsafe fn do_triangles(&self, call: &Call) {
        self.set_uniforms(call.uniform_offset, call.image);
        gl::DrawArrays(
            gl::TRIANGLES,
            call.triangle_offset as i32,
            call.triangle_count as i32,
        );
    }

    fn convert_paint(
        &self,
        paint: &Paint,
        scissor: &Scissor,
        width: f32,
        fringe: f32,
        stroke_thr: f32,
    ) -> FragUniforms {
        let mut frag = FragUniforms {
            scissor_mat: Default::default(),
            paint_mat: Default::default(),
            inner_color: premul_color(paint.inner_color),
            outer_color: premul_color(paint.outer_color),
            scissor_ext: Default::default(),
            scissor_scale: Default::default(),
            extent: Default::default(),
            radius: 0.0,
            feather: 0.0,
            stroke_mult: 0.0,
            stroke_thr,
            tex_type: 0,
            type_: 0,
        };

        if scissor.extent.width < -0.5 || scissor.extent.height < -0.5 {
            frag.scissor_ext[0] = 1.0;
            frag.scissor_ext[1] = 1.0;
            frag.scissor_scale[0] = 1.0;
            frag.scissor_scale[1] = 1.0;
        } else {
            frag.scissor_mat = xform_to_3x4(scissor.xform.inverse());
            frag.scissor_ext[0] = scissor.extent.width;
            frag.scissor_ext[1] = scissor.extent.height;
            frag.scissor_scale[0] = (scissor.xform.0[0] * scissor.xform.0[0]
                + scissor.xform.0[2] * scissor.xform.0[2])
                .sqrt()
                / fringe;
            frag.scissor_scale[1] = (scissor.xform.0[1] * scissor.xform.0[1]
                + scissor.xform.0[3] * scissor.xform.0[3])
                .sqrt()
                / fringe;
        }

        frag.extent = [paint.extent.width, paint.extent.height];
        frag.stroke_mult = (width * 0.5 + fringe * 0.5) / fringe;

        let mut invxform = Transform::default();

        if let Some(img) = paint.image {
            if let Some(texture) = self.textures.get(img) {
                if texture.flags.contains(ImageFlags::FLIPY) {
                    let m1 = Transform::translate(0.0, frag.extent[1] * 0.5) * paint.xform;
                    let m2 = Transform::scale(1.0, -1.0) * m1;
                    let m1 = Transform::translate(0.0, -frag.extent[1] * 0.5) * m2;
                    invxform = m1.inverse();
                } else {
                    invxform = paint.xform.inverse();
                };

                frag.type_ = ShaderType::FillImage as i32;
                match texture.texture_type {
                    TextureType::RGBA => {
                        frag.tex_type = if texture.flags.contains(ImageFlags::PREMULTIPLIED) {
                            0
                        } else {
                            1
                        }
                    }
                    TextureType::Alpha => frag.tex_type = 2,
                }
            }
        } else {
            frag.type_ = ShaderType::FillGradient as i32;
            frag.radius = paint.radius;
            frag.feather = paint.feather;
            invxform = paint.xform.inverse();
        }

        frag.paint_mat = xform_to_3x4(invxform);

        frag
    }

    fn append_uniforms(&mut self, uniforms: FragUniforms) {
        self.uniforms
            .resize(self.uniforms.len() + self.frag_size, 0);
        unsafe {
            let idx = self.uniforms.len() - self.frag_size;
            let p = self.uniforms.as_mut_ptr().add(idx) as *mut FragUniforms;
            *p = uniforms;
        }
    }
}

impl renderer::Renderer for Renderer {
    fn edge_antialias(&self) -> bool {
        true
    }

    fn create_texture(
        &mut self,
        texture_type: TextureType,
        width: usize,
        height: usize,
        flags: ImageFlags,
        data: Option<&[u8]>,
    ) -> anyhow::Result<ImageId> {
        let tex = unsafe {
            let mut tex: gl::types::GLuint = std::mem::zeroed();
            gl::GenTextures(1, &mut tex);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

            match texture_type {
                TextureType::RGBA => {
                    gl::TexImage2D(
                        gl::TEXTURE_2D,
                        0,
                        gl::RGBA as i32,
                        width as i32,
                        height as i32,
                        0,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        match data {
                            Some(data) => data.as_ptr() as *const c_void,
                            None => std::ptr::null(),
                        },
                    );
                }
                TextureType::Alpha => {
                    gl::TexImage2D(
                        gl::TEXTURE_2D,
                        0,
                        gl::R8 as i32,
                        width as i32,
                        height as i32,
                        0,
                        gl::RED,
                        gl::UNSIGNED_BYTE,
                        match data {
                            Some(data) => data.as_ptr() as *const c_void,
                            None => std::ptr::null(),
                        },
                    );
                }
            }

            if flags.contains(ImageFlags::GENERATE_MIPMAPS) {
                if flags.contains(ImageFlags::NEAREST) {
                    gl::TexParameteri(
                        gl::TEXTURE_2D,
                        gl::TEXTURE_MIN_FILTER,
                        gl::NEAREST_MIPMAP_NEAREST as i32,
                    );
                } else {
                    gl::TexParameteri(
                        gl::TEXTURE_2D,
                        gl::TEXTURE_MIN_FILTER,
                        gl::LINEAR_MIPMAP_LINEAR as i32,
                    );
                }
            } else {
                if flags.contains(ImageFlags::NEAREST) {
                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
                } else {
                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
                }
            }

            if flags.contains(ImageFlags::NEAREST) {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            } else {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
            }

            if flags.contains(ImageFlags::REPEATX) {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as i32);
            } else {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            }

            if flags.contains(ImageFlags::REPEATY) {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as i32);
            } else {
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            }

            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);

            if flags.contains(ImageFlags::GENERATE_MIPMAPS) {
                gl::GenerateMipmap(gl::TEXTURE_2D);
            }

            gl::BindTexture(gl::TEXTURE_2D, 0);
            tex
        };

        let id = self.textures.insert(Texture {
            tex,
            width,
            height,
            texture_type,
            flags,
        });
        Ok(id)
    }

    fn delete_texture(&mut self, img: ImageId) -> anyhow::Result<()> {
        if let Some(texture) = self.textures.get(img) {
            unsafe { gl::DeleteTextures(1, &texture.tex) }
            self.textures.remove(img);
            Ok(())
        } else {
            bail!("texture '{}' not found", img);
        }
    }

    fn update_texture(
        &mut self,
        img: ImageId,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        data: &[u8],
    ) -> anyhow::Result<()> {
        if let Some(texture) = self.textures.get(img) {
            unsafe {
                gl::BindTexture(gl::TEXTURE_2D, texture.tex);
                gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);

                match texture.texture_type {
                    TextureType::RGBA => gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        x as i32,
                        y as i32,
                        width as i32,
                        height as i32,
                        gl::RGBA,
                        gl::UNSIGNED_BYTE,
                        data.as_ptr() as *const c_void,
                    ),
                    TextureType::Alpha => gl::TexSubImage2D(
                        gl::TEXTURE_2D,
                        0,
                        x as i32,
                        y as i32,
                        width as i32,
                        height as i32,
                        gl::RED,
                        gl::UNSIGNED_BYTE,
                        data.as_ptr() as *const c_void,
                    ),
                }

                gl::PixelStorei(gl::UNPACK_ALIGNMENT, 4);
                gl::BindTexture(gl::TEXTURE_2D, 0);
            }
            Ok(())
        } else {
            bail!("texture '{}' not found", img);
        }
    }

    fn texture_size(&self, img: ImageId) -> anyhow::Result<(usize, usize)> {
        if let Some(texture) = self.textures.get(img) {
            Ok((texture.width, texture.height))
        } else {
            bail!("texture '{}' not found", img);
        }
    }

    fn viewport(&mut self, extent: Extent, _device_pixel_ratio: f32) -> anyhow::Result<()> {
        self.view = extent;
        Ok(())
    }

    fn cancel(&mut self) -> anyhow::Result<()> {
        self.vertexes.clear();
        self.paths.clear();
        self.calls.clear();
        self.uniforms.clear();
        Ok(())
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        if !self.calls.is_empty() {
            unsafe {
                gl::UseProgram(self.shader.prog);

                gl::Enable(gl::CULL_FACE);
                gl::CullFace(gl::BACK);
                gl::FrontFace(gl::CCW);
                gl::Enable(gl::BLEND);
                gl::Disable(gl::DEPTH_TEST);
                gl::Disable(gl::SCISSOR_TEST);
                gl::ColorMask(gl::TRUE, gl::TRUE, gl::TRUE, gl::TRUE);
                gl::StencilMask(0xffffffff);
                gl::StencilOp(gl::KEEP, gl::KEEP, gl::KEEP);
                gl::StencilFunc(gl::ALWAYS, 0, 0xffffffff);
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, 0);

                gl::BindBuffer(gl::UNIFORM_BUFFER, self.frag_buf);
                gl::BufferData(
                    gl::UNIFORM_BUFFER,
                    self.uniforms.len() as isize,
                    self.uniforms.as_ptr() as *const c_void,
                    gl::STREAM_DRAW,
                );

                gl::BindVertexArray(self.vert_arr);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.vert_buf);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (self.vertexes.len() * std::mem::size_of::<Vertex>()) as isize,
                    self.vertexes.as_ptr() as *const c_void,
                    gl::STREAM_DRAW,
                );
                gl::EnableVertexAttribArray(0);
                gl::EnableVertexAttribArray(1);
                gl::VertexAttribPointer(
                    0,
                    2,
                    gl::FLOAT,
                    gl::FALSE,
                    std::mem::size_of::<Vertex>() as i32,
                    std::ptr::null(),
                );
                gl::VertexAttribPointer(
                    1,
                    2,
                    gl::FLOAT,
                    gl::FALSE,
                    std::mem::size_of::<Vertex>() as i32,
                    (2 * std::mem::size_of::<f32>()) as *const c_void,
                );

                gl::Uniform1i(self.shader.loc_tex, 0);
                gl::Uniform2fv(
                    self.shader.loc_viewsize,
                    1,
                    &self.view as *const Extent as *const f32,
                );

                gl::BindBuffer(gl::UNIFORM_BUFFER, self.frag_buf);

                for call in &self.calls {
                    let blend = &call.blend_func;

                    gl::BlendFuncSeparate(
                        blend.src_rgb,
                        blend.dst_rgb,
                        blend.src_alpha,
                        blend.dst_alpha,
                    );

                    match call.call_type {
                        CallType::Fill => self.do_fill(&call),
                        CallType::ConvexFill => self.do_convex_fill(&call),
                        CallType::Stroke => self.do_stroke(&call),
                        CallType::Triangles => self.do_triangles(&call),
                    }
                }

                gl::DisableVertexAttribArray(0);
                gl::DisableVertexAttribArray(1);
                gl::BindVertexArray(0);
                gl::Disable(gl::CULL_FACE);
                gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                gl::UseProgram(0);
                gl::BindTexture(gl::TEXTURE_2D, 0);
            }
        }

        self.vertexes.clear();
        self.paths.clear();
        self.calls.clear();
        self.uniforms.clear();
        Ok(())
    }

    fn fill(
        &mut self,
        paint: &Paint,
        composite_operation: CompositeOperationState,
        scissor: &Scissor,
        fringe: f32,
        bounds: Bounds,
        paths: &[Path],
    ) -> anyhow::Result<()> {
        let mut call = Call {
            call_type: CallType::Fill,
            image: paint.image,
            path_offset: self.paths.len(),
            path_count: paths.len(),
            triangle_offset: 0,
            triangle_count: 4,
            uniform_offset: 0,
            blend_func: composite_operation.into(),
        };

        if paths.len() == 1 && paths[0].convex {
            call.call_type = CallType::ConvexFill;
        }

        let mut offset = self.vertexes.len();
        for path in paths {
            let fill = path.get_fill();
            let mut gl_path = GLPath {
                fill_offset: 0,
                fill_count: 0,
                stroke_offset: 0,
                stroke_count: 0,
            };

            if !fill.is_empty() {
                gl_path.fill_offset = offset;
                gl_path.fill_count = fill.len();
                self.vertexes.extend(fill);
                offset += fill.len();
            }

            let stroke = path.get_stroke();
            if !stroke.is_empty() {
                gl_path.stroke_offset = offset;
                gl_path.stroke_count = stroke.len();
                self.vertexes.extend(stroke);
                offset += stroke.len();
            }

            self.paths.push(gl_path);
        }

        if call.call_type == CallType::Fill {
            call.triangle_offset = offset;
            self.vertexes
                .push(Vertex::new(bounds.max.x, bounds.max.y, 0.5, 1.0));
            self.vertexes
                .push(Vertex::new(bounds.max.x, bounds.min.y, 0.5, 1.0));
            self.vertexes
                .push(Vertex::new(bounds.min.x, bounds.max.y, 0.5, 1.0));
            self.vertexes
                .push(Vertex::new(bounds.min.x, bounds.min.y, 0.5, 1.0));

            call.uniform_offset = self.uniforms.len() / self.frag_size;
            self.append_uniforms(FragUniforms {
                stroke_thr: -1.0,
                type_: ShaderType::Simple as i32,
                ..FragUniforms::default()
            });
            self.append_uniforms(self.convert_paint(paint, scissor, fringe, fringe, -1.0));
        } else {
            call.uniform_offset = self.uniforms.len() / self.frag_size;
            self.append_uniforms(self.convert_paint(paint, scissor, fringe, fringe, -1.0));
        }

        self.calls.push(call);
        Ok(())
    }

    fn stroke(
        &mut self,
        paint: &Paint,
        composite_operation: CompositeOperationState,
        scissor: &Scissor,
        fringe: f32,
        stroke_width: f32,
        paths: &[Path],
    ) -> anyhow::Result<()> {
        let mut call = Call {
            call_type: CallType::Stroke,
            image: paint.image,
            path_offset: self.paths.len(),
            path_count: paths.len(),
            triangle_offset: 0,
            triangle_count: 0,
            uniform_offset: 0,
            blend_func: composite_operation.into(),
        };

        let mut offset = self.vertexes.len();
        for path in paths {
            let mut gl_path = GLPath {
                fill_offset: 0,
                fill_count: 0,
                stroke_offset: 0,
                stroke_count: 0,
            };

            let stroke = path.get_stroke();
            if !stroke.is_empty() {
                gl_path.stroke_offset = offset;
                gl_path.stroke_count = stroke.len();
                self.vertexes.extend(stroke);
                offset += stroke.len();
                self.paths.push(gl_path);
            }
        }

        call.uniform_offset = self.uniforms.len() / self.frag_size;
        self.append_uniforms(self.convert_paint(paint, scissor, stroke_width, fringe, -1.0));
        self.append_uniforms(self.convert_paint(
            paint,
            scissor,
            stroke_width,
            fringe,
            1.0 - 0.5 / 255.0,
        ));

        self.calls.push(call);
        Ok(())
    }

    fn triangles(
        &mut self,
        paint: &Paint,
        composite_operation: CompositeOperationState,
        scissor: &Scissor,
        vertexes: &[Vertex],
    ) -> anyhow::Result<()> {
        let call = Call {
            call_type: CallType::Triangles,
            image: paint.image,
            path_offset: 0,
            path_count: 0,
            triangle_offset: self.vertexes.len(),
            triangle_count: vertexes.len(),
            uniform_offset: self.uniforms.len() / self.frag_size,
            blend_func: composite_operation.into(),
        };

        self.calls.push(call);
        self.vertexes.extend(vertexes);

        let mut uniforms = self.convert_paint(paint, scissor, 1.0, 1.0, -1.0);
        uniforms.type_ = ShaderType::Image as i32;
        self.append_uniforms(uniforms);
        Ok(())
    }
}

fn shader_error(shader: gl::types::GLuint, filename: &str) -> anyhow::Error {
    unsafe {
        let mut data: [gl::types::GLchar; 512 + 1] = std::mem::zeroed();
        let mut len: gl::types::GLsizei = std::mem::zeroed();
        gl::GetShaderInfoLog(shader, 512, &mut len, data.as_mut_ptr());
        if len > 512 {
            len = 512;
        }
        data[len as usize] = 0;
        let err_msg = std::ffi::CStr::from_ptr(data.as_ptr());
        anyhow!(
            "failed to compile shader: {}: {}",
            filename,
            err_msg.to_string_lossy()
        )
    }
}

fn program_error(prog: gl::types::GLuint) -> anyhow::Error {
    unsafe {
        let mut data: [gl::types::GLchar; 512 + 1] = std::mem::zeroed();
        let mut len: gl::types::GLsizei = std::mem::zeroed();
        gl::GetProgramInfoLog(prog, 512, &mut len, data.as_mut_ptr());
        if len > 512 {
            len = 512;
        }
        data[len as usize] = 0;
        let err_msg = std::ffi::CStr::from_ptr(data.as_ptr());
        anyhow!("failed to link program: {}", err_msg.to_string_lossy())
    }
}

fn convert_blend_factor(factor: BlendFactor) -> gl::types::GLenum {
    match factor {
        BlendFactor::Zero => gl::ZERO,
        BlendFactor::One => gl::ONE,
        BlendFactor::SrcColor => gl::SRC_COLOR,
        BlendFactor::OneMinusSrcColor => gl::ONE_MINUS_SRC_COLOR,
        BlendFactor::DstColor => gl::DST_COLOR,
        BlendFactor::OneMinusDstColor => gl::ONE_MINUS_DST_COLOR,
        BlendFactor::SrcAlpha => gl::SRC_ALPHA,
        BlendFactor::OneMinusSrcAlpha => gl::ONE_MINUS_SRC_ALPHA,
        BlendFactor::DstAlpha => gl::DST_ALPHA,
        BlendFactor::OneMinusDstAlpha => gl::ONE_MINUS_DST_ALPHA,
        BlendFactor::SrcAlphaSaturate => gl::SRC_ALPHA_SATURATE,
    }
}

#[inline]
fn premul_color(color: Color) -> Color {
    Color {
        r: color.r * color.a,
        g: color.g * color.a,
        b: color.b * color.a,
        a: color.a,
    }
}

#[inline]
fn xform_to_3x4(xform: Transform) -> [f32; 12] {
    let mut m = [0f32; 12];
    let t = &xform.0;
    m[0] = t[0];
    m[1] = t[1];
    m[2] = 0.0;
    m[3] = 0.0;
    m[4] = t[2];
    m[5] = t[3];
    m[6] = 0.0;
    m[7] = 0.0;
    m[8] = t[4];
    m[9] = t[5];
    m[10] = 1.0;
    m[11] = 0.0;
    m
}
