use gl;
use glutin::ContextBuilder;
use glutin::dpi::LogicalSize;
use glutin::event::{Event, StartCause, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::WindowBuilder;
use nvg;
use nvg_gl;

fn main() {
    let el = EventLoop::new();
    let wb = WindowBuilder::new()
        .with_title("nvg - demo-square")
        .with_inner_size(LogicalSize::new(800.0, 600.0));
    let wc = ContextBuilder::new().build_windowed(wb, &el).unwrap();
    let wc = unsafe { wc.make_current().unwrap() };
    gl::load_with(|p| wc.get_proc_address(p) as *const _);
    let renderer = nvg_gl::Renderer::create().unwrap();
    let mut nvg_ctx = nvg::Context::create(renderer).unwrap();
    el.run(move |evt, _, ctrl_flow| {
        match evt {
            Event::NewEvents(StartCause::Init) =>
                *ctrl_flow = ControlFlow::Wait,
            Event::LoopDestroyed => return,
            Event::WindowEvent {event, ..} => match event {
                WindowEvent::CloseRequested => *ctrl_flow = ControlFlow::Exit,
                _ => ()
            }
            Event::RedrawRequested(_) => {
                let size = wc.window().inner_size();
                let sf = wc.window().scale_factor();
                unsafe {
                    gl::Viewport(0, 0, size.width as i32, size.height as i32);
                    gl::ClearColor(0.0, 0.0, 0.0, 1.0);
                    gl::Clear(
                        gl::COLOR_BUFFER_BIT |
                        gl::DEPTH_BUFFER_BIT |
                        gl::STENCIL_BUFFER_BIT
                    );
                }
                nvg_ctx.begin_frame(
                    nvg::Extent {
                        width: size.width as f32,
                        height: size.height as f32
                    },
                    sf as f32
                ).unwrap();
                nvg_ctx.save();
                nvg_ctx.fill_paint(nvg::Color::rgb(1.0, 0.0, 0.0));
                nvg_ctx.rect(nvg::Rect::new(
                    nvg::Point::new(10.0, 10.0),
                    nvg::Extent::new(40.0, 40.0)
                ));
                nvg_ctx.fill().unwrap();
                nvg_ctx.restore();
                nvg_ctx.end_frame().unwrap();
                wc.swap_buffers().unwrap();
            }
            _ => ()
        }
    });
}
