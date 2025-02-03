mod godot_window;
mod protocols;

use std::path::PathBuf;
use std::{fs, thread};
use godot::prelude::*;
use godot::classes::{Control, IControl, IDisplayServer, ISprite2D, Os, ProjectSettings, Sprite2D};
use http::header::CONTENT_TYPE;
use http::Response;
use wry::{Rect, WebViewAttributes, WebViewBuilder, WebViewBuilderExtWindows, WebContext, RGBA};
use wry::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use wry::http::{HeaderMap, Request};
use crate::godot_window::GodotWindow;
use crate::protocols::get_res_response;

struct GodotWRY;

#[gdextension]
unsafe impl ExtensionLibrary for GodotWRY {}

#[derive(GodotClass)]
#[class(base=Control)]
struct WebView {
    base: Base<Control>,
    web_context: WebContext,
    webview: Option<wry::WebView>,
    #[export]
    full_window_size: bool,
    #[export]
    url: GString,
    #[export]
    html: GString,
    #[export]
    transparent: bool,
    #[export]
    background_color: Color,
    #[export]
    devtools: bool,
    #[export]
    headers: Dictionary,
    #[export]
    user_agent: GString,
    #[export]
    theme: GString,
    #[export]
    zoom_hotkeys: bool,
    #[export]
    clipboard: bool,
    #[export]
    incognito: bool,
    #[export]
    focused: bool,
}

#[godot_api]
impl IControl for WebView {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            webview: None,
            web_context: WebContext::new(std::env::home_dir()),
            full_window_size: true,
            url: "https://github.com/doceazedo/godot_wry".into(),
            html: "".into(),
            transparent: false,
            background_color: Color::from_rgb(1.0, 1.0, 1.0),
            devtools: true,
            headers: Dictionary::new(),
            user_agent: "".into(),
            theme: "dark".into(),
            zoom_hotkeys: false,
            clipboard: true,
            incognito: false,
            focused: true
        }
    }

    #[cfg(target_os = "linux")]
    fn process(&mut self, _delta: f64) {
        if self.webview.is_none().clone() { return }
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
    }

    fn ready(&mut self) {
        let window = GodotWindow;
        let base = self.base().clone();
        let webview_builder = WebViewBuilder::with_attributes(WebViewAttributes {
            url: if self.html.is_empty() { Some(String::from(&self.url)) } else { None },
            html: if self.url.is_empty() { Some(String::from(&self.html)) } else { None },
            transparent: self.transparent,
            devtools: self.devtools,
            context: Some(&mut self.web_context),
            // headers: Some(HeaderMap::try_from(self.headers.iter_shared().typed::<GString, Variant>()).unwrap_or_default()),
            user_agent: Some(String::from(&self.user_agent)),
            zoom_hotkeys_enabled: self.zoom_hotkeys,
            clipboard: self.clipboard,
            incognito: self.incognito,
            focused: self.focused,
            ..Default::default()
        })
            .with_ipc_handler(move |req: Request<String>| {
                let body = req.body().as_str();
                base.clone().emit_signal("ipc_message".into(), &[body.to_variant()]);
            })
            .with_custom_protocol(
                "res".into(), move |_webview_id, request| get_res_response(request),
            )
            .with_theme(match &String::from_iter(self.theme.chars().clone())[..] {
                "light" => wry::Theme::Light,
                "dark" => wry::Theme::Dark,
                _ => wry::Theme::Auto
            });

        if !self.url.is_empty() && !self.html.is_empty() {
            godot_error!("You have entered both a URL and HTML code. You may only enter one at a time.")
        }

        let webview = webview_builder.build_as_child(&window).unwrap();
        self.webview.replace(webview);

        let mut viewport = self.base().get_tree().expect("Could not get tree").get_root().expect("Could not get viewport");
        viewport.connect("size_changed".into(), Callable::from_object_method(&*self.base(), "resize"));

        self.resize()
    }
}

#[godot_api]
impl WebView {
    #[signal]
    fn ipc_message(message: GString);

    #[func]
    fn post_message(&self, message: GString) {
        if let Some(webview) = &self.webview {
            let message = str::replace(&*String::from(message), "'", "\\'");
            let script = str::replace("document.dispatchEvent(new CustomEvent('message', { detail: '{}' }))", "{}", &*message);
            let _ = webview.evaluate_script(&*script);
        }
    }

    #[func]
    fn resize(&self) {
        if let Some(webview) = &self.webview {
            let rect = if self.full_window_size {
                let viewport_size = self.base().get_tree().expect("Could not get tree").get_root().expect("Could not get viewport").get_size();
                Rect {
                    position: PhysicalPosition::new(0, 0).into(),
                    size: PhysicalSize::new(viewport_size.x, viewport_size.y).into(),
                }
            } else {
                let rect = self.base().get_global_rect();
                Rect {
                    position: PhysicalPosition::new(rect.position.x, rect.position.y).into(),
                    size: PhysicalSize::new(rect.size.x, rect.size.y).into(),
                }
            };
            let _ = webview.set_bounds(rect);
        }
    }

    #[func]
    fn focus(&self) {
        if let Some(webview) = &self.webview {
            webview.focus().expect("oops")
        }
    }

    #[func]
    fn focus_parent(&self) {
        if let Some(webview) = &self.webview {
            let _ = webview.focus_parent();
        }
    }

    #[func]
    fn set_visible(&self, visible: bool) {
        if let Some(webview) = &self.webview {
            webview.set_visible(visible).expect("can't set visibility");
        }
    }

    #[func]
    fn force_free(&mut self) {
        self.webview.take();
    }
}
