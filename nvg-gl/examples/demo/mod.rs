use nvg::{Context, Renderer};

pub trait Demo<R: Renderer> {
    fn init(&mut self, _ctx: &mut Context<R>) -> anyhow::Result<()> {
        Ok(())
    }

    fn update(&mut self, _width: f32, _height: f32, _ctx: &mut Context<R>) -> anyhow::Result<()> {
        Ok(())
    }

    fn cursor_moved(&mut self, _x: f32, _y: f32) {}
}

pub fn run<D: Demo<nvg_gl::Renderer>>(mut demo: D) -> anyhow::Result<()> {
    let mut el = glutin::EventsLoop::new();
    let wb =
        glutin::WindowBuilder::new().with_dimensions(glutin::dpi::LogicalSize::new(1024.0, 768.0));
    let windowed_context = glutin::ContextBuilder::new().build_windowed(wb, &el)?;
    let windowed_context = unsafe { windowed_context.make_current().unwrap() };
    gl::load_with(|p| windowed_context.get_proc_address(p) as *const _);

    let renderer = nvg_gl::Renderer::create()?;
    let mut context = nvg::Context::create(renderer)?;

    demo.init(&mut context)?;

    let mut loop_ = true;

    while loop_ {
        el.poll_events(|event| match event {
            glutin::Event::WindowEvent { event, .. } => match event {
                glutin::WindowEvent::CloseRequested => {
                    loop_ = false;
                }
                glutin::WindowEvent::Resized(sz) => {
                    windowed_context.resize(glutin::dpi::PhysicalSize {
                        width: sz.width,
                        height: sz.height,
                    });
                }
                glutin::WindowEvent::CursorMoved { position, .. } => {
                    demo.cursor_moved(position.x as f32, position.y as f32);
                }
                _ => {}
            },
            _ => {}
        });

        let size = windowed_context.window().get_inner_size().unwrap();
        let device_pixel_ratio = windowed_context.window().get_hidpi_factor() as f32;

        unsafe {
            gl::Viewport(
                0,
                0,
                (size.width as f32 * device_pixel_ratio) as i32,
                (size.height as f32 * device_pixel_ratio) as i32,
            );
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        }

        context
            .begin_frame(
                nvg::Extent {
                    width: size.width as f32,
                    height: size.height as f32,
                },
                device_pixel_ratio,
            )
            .unwrap();
        demo.update(size.width as f32, size.height as f32, &mut context)
            .unwrap();
        context.end_frame().unwrap();

        windowed_context.swap_buffers().unwrap();
    }

    Ok(())
}
