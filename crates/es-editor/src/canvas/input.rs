use super::EditorCanvas;
use crate::cursor;
use es_anno::{CursorKind, PointerButton, PointerInfo};
use gpui::{
    App, Context, CursorStyle, DispatchPhase, Entity, Hitbox, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Window,
};

/// GPUI reports no tablet data on Linux, so every pointer is a mouse at the engine's neutral pressure.
const MOUSE_PRESSURE: f64 = 0.5;

impl EditorCanvas {
    /// Window position to the engine's screen (element) and page coordinates.
    pub fn pointer_info(
        &self,
        position: Point<Pixels>,
        modifiers: Modifiers,
        click_count: usize,
        button: PointerButton,
        cx: &App,
    ) -> PointerInfo {
        let screen = self.local(position);
        let page = self.viewport.read(cx).viewport().screen_to_page(screen);
        PointerInfo {
            screen,
            page,
            shift: modifiers.shift,
            alt: modifiers.alt,
            ctrl: modifiers.control,
            is_pen: false,
            pressure: MOUSE_PRESSURE,
            click_count: click_count.min(usize::from(u8::MAX)) as u8,
            button,
        }
    }

    /// The cursor the last frame set over the canvas.
    pub fn cursor(&self) -> CursorStyle {
        self.cursor
    }

    fn gesture_active(&self, cx: &App) -> bool {
        !self.doc.read(cx).editor().is_idle()
    }

    fn cursor_at(&self, position: Point<Pixels>, cx: &App) -> CursorStyle {
        let editor = self.doc.read(cx).editor();
        let at = self.local(position);
        let frame = self.viewport.read(cx).math().canvas_rect();
        if editor.is_idle() && !frame.contains(at) {
            return CursorStyle::Arrow;
        }
        match editor.cursor_for(at) {
            CursorKind::Arrow => CursorStyle::Arrow,
            kind => cursor::style(kind, &self.cursors),
        }
    }

    fn pointer_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let button = match e.button {
            MouseButton::Left => PointerButton::Primary,
            MouseButton::Right => PointerButton::Secondary,
            _ => return,
        };
        if self.is_panning() {
            return;
        }
        window.focus(&self.focus, cx);
        let pointer = self.pointer_info(e.position, e.modifiers, e.click_count, button, cx);
        self.doc
            .update(cx, |d, cx| d.apply(cx, |ed| ed.pointer_down(pointer)));
        self.place_caret(e.position, e.modifiers.shift, cx);
        cx.notify();
    }

    pub(super) fn shape_at(&self, position: Point<Pixels>, cx: &App) -> bool {
        let page = self
            .viewport
            .read(cx)
            .viewport()
            .screen_to_page(self.local(position));
        self.doc.read(cx).editor().hit_shape(page).is_some()
    }

    pub(super) fn space_pointer_down(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if e.button != MouseButton::Left || !self.shape_at(e.position, cx) {
            return;
        }
        window.focus(&self.focus, cx);
        let pointer = self.pointer_info(
            e.position,
            e.modifiers,
            e.click_count,
            PointerButton::Primary,
            cx,
        );
        self.doc
            .update(cx, |d, cx| d.apply(cx, |ed| ed.pointer_down_move(pointer)));
        cx.notify();
    }

    fn pointer_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let pointer = self.pointer_info(e.position, e.modifiers, 0, PointerButton::Primary, cx);
        self.doc
            .update(cx, |d, cx| d.apply(cx, |ed| ed.pointer_move(pointer)));
        if self.cursor_at(e.position, cx) != self.cursor {
            cx.notify();
        }
    }

    fn pointer_up(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        let pointer = self.pointer_info(
            e.position,
            e.modifiers,
            e.click_count,
            PointerButton::Primary,
            cx,
        );
        self.doc
            .update(cx, |d, cx| d.apply(cx, |ed| ed.pointer_up(pointer)));
        cx.notify();
    }

    fn hover(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.cursor_at(position, cx) != self.cursor {
            cx.notify();
        }
    }
}

/// Sets this frame's cursor and routes the primary button into the engine; panning is handled before this.
pub(super) fn register(
    view: &Entity<EditorCanvas>,
    hitbox: &Hitbox,
    window: &mut Window,
    cx: &mut App,
) {
    let position = window.mouse_position();
    let style = view.update(cx, |c, cx| {
        c.cursor = c.cursor_at(position, cx);
        c.cursor
    });
    window.set_cursor_style(style, hitbox);

    let (v, hb) = (view.clone(), hitbox.clone());
    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && hb.is_hovered(window) {
            v.update(cx, |c, cx| c.pointer_down(e, window, cx));
        }
    });
    register_gesture(view, hitbox, window);
}

/// The press that moves a shape while Space is held; elsewhere Space still pans.
pub(super) fn register_space(
    view: &Entity<EditorCanvas>,
    hitbox: &Hitbox,
    window: &mut Window,
    cx: &mut App,
) {
    let position = window.mouse_position();
    let style = view.update(cx, |c, cx| {
        c.cursor = if c.shape_at(position, cx) || c.gesture_active(cx) {
            CursorStyle::PointingHand
        } else {
            CursorStyle::OpenHand
        };
        c.cursor
    });
    window.set_cursor_style(style, hitbox);

    let (v, hb) = (view.clone(), hitbox.clone());
    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && hb.is_hovered(window) {
            v.update(cx, |c, cx| c.space_pointer_down(e, window, cx));
        }
    });
    register_gesture(view, hitbox, window);
}

fn register_gesture(view: &Entity<EditorCanvas>, hitbox: &Hitbox, window: &mut Window) {
    let (v, hb) = (view.clone(), hitbox.clone());
    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
        let active = v.read(cx).gesture_active(cx);
        if active && phase == DispatchPhase::Capture {
            v.update(cx, |c, cx| c.pointer_move(e, cx));
        } else if !active && phase == DispatchPhase::Bubble && hb.is_hovered(window) {
            v.update(cx, |c, cx| c.hover(e.position, cx));
        }
    });
    let v = view.clone();
    window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture
            && e.button == MouseButton::Left
            && v.read(cx).gesture_active(cx)
        {
            v.update(cx, |c, cx| c.pointer_up(e, cx));
        }
    });
}
