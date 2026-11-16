use gpui_kit::component::Theme;
use gpui_kit::*;
use roster::app::RosterApp;

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        // `gpui_kit::init` forces ThemeMode::Light; restore the OS appearance.
        Theme::sync_system_appearance(None, cx);

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(80.0), px(80.0)),
                size(px(1280.0), px(860.0)),
            ))),
            window_min_size: Some(size(px(960.0), px(640.0))),
            ..Default::default()
        };

        open_window(window_options, cx, |window, cx| {
            cx.new(|cx| RosterApp::new(window, cx))
        })
        .expect("failed to open window");
    });
}
