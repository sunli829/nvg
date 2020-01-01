use nvg::*;

mod demo;

struct DemoText;

impl<R: Renderer> demo::Demo<R> for DemoText {
    fn update(&mut self, _width: f32, _height: f32, ctx: &mut Context<R>) -> anyhow::Result<()> {
        ctx.begin_path();
        ctx.move_to((150, 20));
        ctx.line_to((150, 170));
        ctx.stroke_paint((1.0, 0.0, 0.0));
        ctx.stroke()?;

        ctx.font_size(16.0);
        ctx.fill_paint((1.0, 1.0, 0.0));

        // horz align
        ctx.text_align(nvg::Align::LEFT);
        ctx.text((150, 60), "left")?;

        ctx.text_align(nvg::Align::CENTER);
        ctx.text((150, 80), "center")?;

        ctx.text_align(nvg::Align::RIGHT);
        ctx.text((150, 100), "right")?;

        // vert align
        ctx.begin_path();
        ctx.move_to((5, 270));
        ctx.line_to((300, 270));
        ctx.stroke_paint((1.0, 0.0, 0.0));
        ctx.stroke()?;

        ctx.text_align(nvg::Align::TOP);
        ctx.text((5, 270), "top")?;

        ctx.text_align(nvg::Align::MIDDLE);
        ctx.text((50, 270), "middle")?;

        ctx.text_align(nvg::Align::BOTTOM);
        ctx.text((120, 270), "bottom")?;

        ctx.text_align(nvg::Align::BASELINE);
        ctx.text((200, 270), "baseline")?;

        // spaces
        ctx.text((200, 300), "a b  c   d")?;

        Ok(())
    }
}

fn main() {
    demo::run(DemoText).unwrap();
}
