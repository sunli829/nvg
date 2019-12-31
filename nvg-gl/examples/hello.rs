use std::fs;

fn main() {
    let mut el = glutin::EventsLoop::new();
    let wb =
        glutin::WindowBuilder::new().with_dimensions(glutin::dpi::LogicalSize::new(1024.0, 768.0));
    let windowed_context = glutin::ContextBuilder::new()
        .build_windowed(wb, &el)
        .unwrap();
    let windowed_context = unsafe { windowed_context.make_current().unwrap() };
    gl::load_with(|p| windowed_context.get_proc_address(p) as *const _);

    let renderer = nvg_gl::Renderer::create().unwrap();
    let mut context = nvg::Context::create(renderer).unwrap();

    context
        .create_font("roboto", fs::read("Roboto-Bold.ttf").unwrap())
        .unwrap();

    el.run_forever(|event| match event {
        glutin::Event::WindowEvent { event, .. } => match event {
            glutin::WindowEvent::CloseRequested => glutin::ControlFlow::Break,
            glutin::WindowEvent::Resized(sz) => {
                windowed_context.resize(glutin::dpi::PhysicalSize {
                    width: sz.width,
                    height: sz.height,
                });
                glutin::ControlFlow::Continue
            }
            glutin::WindowEvent::Refresh => {
                let size = windowed_context.window().get_inner_size().unwrap();
                let device_pixel_ratio = windowed_context.window().get_hidpi_factor() as f32;

                unsafe {
                    gl::Viewport(
                        0,
                        0,
                        (size.width as f32 * device_pixel_ratio) as i32,
                        (size.height as f32 * device_pixel_ratio) as i32,
                    );
                    gl::ClearColor(0.3, 0.3, 0.3, 1.0);
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
                draw(&mut context);
                context.end_frame().unwrap();

                windowed_context.swap_buffers().unwrap();
                glutin::ControlFlow::Continue
            }
            _ => glutin::ControlFlow::Continue,
        },
        _ => glutin::ControlFlow::Continue,
    });
}

fn draw<R: nvg::Renderer>(ctx: &mut nvg::Context<R>) {
    ctx.begin_path();
    ctx.move_to((150, 20));
    ctx.line_to((150, 170));
    ctx.stroke_paint((1.0, 0.0, 0.0));
    ctx.stroke().unwrap();

    ctx.font("roboto");
    ctx.font_size(16.0);
    ctx.fill_paint((1.0, 1.0, 0.0));

    // horz align
    ctx.text_align(nvg::Align::LEFT);
    ctx.text((150, 60), "left").unwrap();
//
//    ctx.text_align(nvg::Align::CENTER);
//    ctx.text((150, 80), "center").unwrap();
//
//    ctx.text_align(nvg::Align::RIGHT);
//    ctx.text((150, 100), "right").unwrap();
//
//    // vert align
//    ctx.begin_path();
//    ctx.move_to((5, 270));
//    ctx.line_to((300, 270));
//    ctx.stroke_paint((1.0, 0.0, 0.0));
//    ctx.stroke().unwrap();
//
//    ctx.text_align(nvg::Align::TOP);
//    ctx.text((5, 270), "top").unwrap();
//
//    ctx.text_align(nvg::Align::MIDDLE);
//    ctx.text((50, 270), "middle").unwrap();
//
//    ctx.text_align(nvg::Align::BOTTOM);
//    ctx.text((120, 270), "bottom").unwrap();
//
//    ctx.text_align(nvg::Align::BASELINE);
//    ctx.text((200, 270), "baseline").unwrap();
}
