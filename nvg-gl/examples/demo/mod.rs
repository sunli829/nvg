use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use nvg::{Align, Color, Context, Renderer};
use std::time::Instant;

pub trait Demo<R: Renderer> {
    fn init(&mut self, ctx: &mut Context<R>) -> anyhow::Result<()> {
        ctx.create_font_from_file("roboto", "nvg-gl/examples/Roboto-Bold.ttf")?;
        Ok(())
    }

    fn update(
        &mut self,
        _width: f32,
        _height: f32,
        _ctx: &mut Context<R>
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn cursor_moved(&mut self, _x: f32, _y: f32) {}
}

pub fn run<D: Demo<nvg_gl::Renderer> + 'static>(
    mut demo: D,
    title: &str
) {
    let el = EventLoop::new();
    let wb = glutin::window::WindowBuilder::new()
        .with_title(format!("nvg - {}", title))
        .with_inner_size(glutin::dpi::LogicalSize::new(1024.0, 768.0));
    let windowed_context = glutin::ContextBuilder::new()
        .build_windowed(wb, &el).unwrap();
    let windowed_context = unsafe { windowed_context.make_current().unwrap() };
    gl::load_with(|p| windowed_context.get_proc_address(p) as *const _);

    let mut window_size = windowed_context.window().inner_size();
    let scale_factor = windowed_context.window().scale_factor();

    let renderer = nvg_gl::Renderer::create().unwrap();
    let mut context = nvg::Context::create(renderer).unwrap();

    demo.init(&mut context).unwrap();

    let mut total_frames = 0;
    let start_time = Instant::now();

    el.run(move |evt, _, ctrl_flow| {
        windowed_context.window().request_redraw();
        match evt {
            Event::LoopDestroyed => return,
            Event::WindowEvent {event, ..} => match event {
                WindowEvent::CloseRequested => *ctrl_flow = ControlFlow::Exit,
                WindowEvent::Resized(psize) => window_size = psize,
                WindowEvent::CursorMoved {position, ..} =>
                    demo.cursor_moved(position.x as f32, position.y as f32),
                _ => ()
            }
            Event::RedrawRequested(_) => {
                unsafe {
                    gl::Viewport(
                        0,
                        0,
                        window_size.width as i32,
                        window_size.height as i32
                    );
                    gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                    gl::Clear(
                        gl::COLOR_BUFFER_BIT |
                        gl::DEPTH_BUFFER_BIT |
                        gl::STENCIL_BUFFER_BIT
                    );
                }
                context.begin_frame(
                    nvg::Extent {
                        width: window_size.width as f32,
                        height: window_size.height as f32
                    },
                    scale_factor as f32
                ).unwrap();

                context.save();
                demo.update(
                    window_size.width as f32,
                    window_size.height as f32,
                    &mut context
                ).unwrap();
                context.restore();

                context.save();
                total_frames += 1;
                let fps =
                    (total_frames as f32) /
                    (Instant::now() -
                     start_time).as_secs_f32();
                context.fill_paint(Color::rgb(1.0, 0.0, 0.0));
                context.font("roboto");
                context.font_size(20.0);
                context.text_align(Align::TOP | Align::LEFT);
                context.text((10, 10), format!("FPS: {:.2}", fps)).unwrap();
                context.fill().unwrap();
                context.restore();
                context.end_frame().unwrap();
                windowed_context.swap_buffers().unwrap();
            }
            _ => ()
        }
    });
}
