// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::{
    editor::{Buffer, Cell, Char, EditorCtx, CONSUMED, SP},
    Options,
};
use cursive::{
    event::{Event, EventResult, Key, MouseButton::*, MouseEvent::*},
    Rect, Vec2,
};
use std::{cmp::min, fmt};

macro_rules! option {
    ($a:expr) => {
        match $a {
            Some(a) => a,
            _ => return,
        }
    };

    ($a:expr, $b:expr) => {
        match ($a, $b) {
            (Some(a), Some(b)) => (a, b),
            _ => return,
        }
    };
}

macro_rules! mouse_drag {
    ($ctx:expr, $event:expr) => {{
        let (pos, event) = match $ctx.relativize($event) {
            Event::Mouse {
                position, event, ..
            } => (position, event),

            _ => return None,
        };

        if let Hold(Left) = event {
            $ctx.scroll_to(pos, 2, 2);
        }

        (pos, event)
    }};
}

/// Provides an implementation of `Tool::on_event` for tools that contain a `src` and
/// `dst` field of type `Option<Vec2>`. The implementation performs basic left mouse
/// drag handling, calling the argument closure when relevant events occur.
macro_rules! fn_on_event_drag {
    ($render:expr) => {
    fn on_event(&mut self, ctx: &mut EditorCtx<'_>, event: &Event) -> Option<EventResult> {
        let (pos, event) = mouse_drag!(ctx, event);

        match event {
            Press(Left) => {
                self.src = Some(pos);
                self.dst = Some(pos);
                ctx.preview(|buf| $render(self, buf));
            }

            Hold(Left) => {
                self.dst = Some(pos);
                ctx.preview(|buf| $render(self, buf));
            }

            Release(Left) => {
                self.dst = Some(pos);
                ctx.clobber(|buf| $render(self, buf));
                self.src = None;
                self.dst = None;
            }

            _ => return None,
        }

        CONSUMED
    }
    }
}

macro_rules! simple_display {
    ($type:ty, $fstr:expr) => {
        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, $fstr)
            }
        }
    };
}

pub(crate) trait Tool: fmt::Display {
    fn load_opts(&mut self, _: &Options) {}

    fn on_event(&mut self, ctx: &mut EditorCtx<'_>, e: &Event) -> Option<EventResult>;
}

#[derive(Copy, Clone, Default)]
pub(crate) struct BoxTool {
    src: Option<Vec2>,
    dst: Option<Vec2>,
}

simple_display! { BoxTool, "Box" }

impl Tool for BoxTool {
    fn_on_event_drag!(|t: &Self, buf: &mut Buffer| {
        let (src, dst) = option!(t.src, t.dst);

        let r = Rect::from_corners(src, dst);

        buf.draw_line(r.top_left(), r.top_right());
        buf.draw_line(r.top_right(), r.bottom_right());
        buf.draw_line(r.bottom_right(), r.bottom_left());
        buf.draw_line(r.bottom_left(), r.top_left());
    });
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum PathMode {
    Snap90,
    Snap45,
    Routed,
}

impl Default for PathMode {
    fn default() -> Self {
        Self::Snap90
    }
}

#[derive(Copy, Clone, Default)]
pub(crate) struct LineTool {
    src: Option<Vec2>,
    dst: Option<Vec2>,
    path_mode: PathMode,
}

impl fmt::Display for LineTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Line: {:?}", self.path_mode)
    }
}

impl Tool for LineTool {
    fn load_opts(&mut self, opts: &Options) {
        self.path_mode = opts.path_mode;
    }

    fn_on_event_drag!(|t: &Self, buf: &mut Buffer| {
        let (src, dst) = option!(t.src, t.dst);

        if let PathMode::Routed = t.path_mode {
            buf.draw_path(src, dst);
            return;
        }

        let mid = match t.path_mode {
            PathMode::Snap90 => buf.snap90(src, dst),
            _ => buf.snap45(src, dst),
        };

        buf.draw_line(src, mid);
        buf.draw_line(mid, dst);
    });
}

#[derive(Copy, Clone, Default)]
pub(crate) struct ArrowTool {
    src: Option<Vec2>,
    dst: Option<Vec2>,
    path_mode: PathMode,
}

impl fmt::Display for ArrowTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Arrow: {:?}", self.path_mode)
    }
}

impl Tool for ArrowTool {
    fn load_opts(&mut self, opts: &Options) {
        self.path_mode = opts.path_mode;
    }

    fn_on_event_drag!(|t: &Self, buf: &mut Buffer| {
        let (src, dst) = option!(t.src, t.dst);

        if let PathMode::Routed = t.path_mode {
            let last = buf.draw_path(src, dst);
            buf.draw_arrow_tip(last, dst);
            return;
        }

        let mid = match t.path_mode {
            PathMode::Snap90 => buf.snap90(src, dst),
            _ => buf.snap45(src, dst),
        };

        if mid != dst {
            buf.draw_line(src, mid);
            buf.draw_line(mid, dst);
            buf.draw_arrow_tip(mid, dst);
        } else {
            buf.draw_line(src, dst);
            buf.draw_arrow_tip(src, dst);
        }
    });
}

#[derive(Clone)]
pub(crate) struct TextTool {
    src: Option<Vec2>,
    ready: bool,
    buffer: Vec<Vec<char>>,
    cursor: Vec2,
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            src: None,
            ready: false,
            buffer: vec![],
            cursor: Vec2::new(0, 0),
        }
    }
}

simple_display! { TextTool, "Text" }

impl Tool for TextTool {
    fn on_event(&mut self, ctx: &mut EditorCtx<'_>, event: &Event) -> Option<EventResult> {
        let Vec2 { x, y } = &mut self.cursor;

        match ctx.relativize(event) {
            Event::Mouse {
                event: Press(Left),
                position,
                ..
            } => {
                self.src = Some(position);
                self.ready = false;
                self.buffer.clear();
                self.buffer.push(vec![]);
                self.cursor = Vec2::new(0, 0);
                ctx.preview(|buf| self.render(buf));
            }

            Event::Mouse {
                event: Release(Left),
                ..
            } => {
                self.ready = true;
                ctx.preview(|buf| self.render(buf));
            }

            _ if !self.ready => return None,

            Event::Char(c) => {
                self.buffer[*y].insert(*x, c);
                *x += 1;
                ctx.preview(|buf| self.render(buf));
                ctx.scroll_to_cursor();
            }

            Event::Key(Key::Up) => {
                *y = y.saturating_sub(1);
                *x = min(self.buffer[*y].len(), *x);
                ctx.preview(|buf| self.render(buf));
                ctx.scroll_to_cursor();
            }

            Event::Key(Key::Down) => {
                *y = min(self.buffer.len() - 1, *y + 1);
                *x = min(self.buffer[*y].len(), *x);
                ctx.preview(|buf| self.render(buf));
                ctx.scroll_to_cursor();
            }

            Event::Key(Key::Left) => {
                *x = x.saturating_sub(1);
                ctx.preview(|buf| self.render(buf));
                ctx.scroll_to_cursor();
            }

            Event::Key(Key::Right) => {
                *x = min(self.buffer[*y].len(), *x + 1);
                ctx.preview(|buf| self.render(buf));
                ctx.scroll_to_cursor();
            }

            Event::Key(Key::Enter) => {
                let next = self.buffer[*y].split_off(*x);
                self.buffer.insert(*y + 1, next);
                *x = 0;
                *y += 1;
                ctx.preview(|buf| self.render(buf));
                ctx.scroll_to_cursor();
            }

            Event::Key(Key::Backspace) | Event::Key(Key::Del) => {
                if *x > 0 {
                    self.buffer[*y].remove(*x - 1);
                    *x -= 1;
                } else if *y > 0 {
                    let mut next = self.buffer.remove(*y);
                    *y -= 1;
                    *x = self.buffer[*y].len();
                    self.buffer[*y].append(&mut next);
                }
                ctx.preview(|buf| self.render(buf));
                ctx.scroll_to_cursor();
            }

            Event::Key(Key::Esc) => {
                ctx.clobber(|buf| self.render(buf));
                self.src = None;
                self.ready = false;
                self.buffer.clear();
                self.cursor = Vec2::new(0, 0);
            }

            _ => return None,
        }

        CONSUMED
    }
}

impl TextTool {
    fn render(&self, buf: &mut Buffer) {
        let src = option!(self.src);

        for (y, line) in self.buffer.iter().enumerate() {
            for (x, c) in line.iter().enumerate() {
                let pos = Vec2::new(x, y) + src;
                buf.setv(true, pos, *c);
            }
        }

        buf.set_cursor(self.cursor + src);
    }
}

#[derive(Copy, Clone, Default)]
pub(crate) struct EraseTool {
    src: Option<Vec2>,
    dst: Option<Vec2>,
}

simple_display! { EraseTool, "Erase" }

impl Tool for EraseTool {
    fn_on_event_drag!(|t: &Self, buf: &mut Buffer| {
        let state: Vec<_> = visible_cells(buf, option!(t.src, t.dst)).collect();

        for cell in state {
            buf.setv(true, cell.pos(), SP);
        }
    });
}

fn visible_cells<'a>(buf: &'a Buffer, cs: (Vec2, Vec2)) -> impl Iterator<Item = Cell> + 'a {
    let area = Rect::from_corners(cs.0, cs.1);

    buf.iter_within(area.top_left(), area.size())
        .flat_map(|c| match c {
            Char::Clean(cell) => Some(cell),
            Char::Dirty(cell) => Some(cell),
            _ => None,
        })
        .filter(|cell| !cell.is_whitespace())
}

#[derive(Copy, Clone, Default)]
pub(crate) struct MoveTool {
    src: Option<Vec2>,
    dst: Option<Vec2>,
    grab_src: Option<Vec2>,
    grab_dst: Option<Vec2>,
}

simple_display! { MoveTool, "Move" }

impl Tool for MoveTool {
    fn on_event(&mut self, ctx: &mut EditorCtx<'_>, e: &Event) -> Option<EventResult> {
        let (pos, event) = mouse_drag!(ctx, e);

        match event {
            Press(Left) => {
                if let Some(true) = self
                    .src
                    .and_then(|o| Some((o, self.dst?)))
                    .map(|(o, t)| Rect::from_corners(o, t))
                    .map(|r| r.contains(pos))
                {
                    self.grab_src = Some(pos);
                    self.grab_dst = Some(pos);
                } else {
                    self.src = Some(pos);
                    self.dst = Some(pos);
                    self.grab_src = None;
                    self.grab_dst = None;
                }
                ctx.preview(|buf| self.render(buf));
            }

            Hold(Left) => {
                if self.grab_src.is_some() {
                    self.grab_dst = Some(pos);
                } else {
                    self.dst = Some(pos);
                }
                ctx.preview(|buf| self.render(buf));
            }

            Release(Left) => {
                if self.grab_src.is_some() {
                    self.grab_dst = Some(pos);
                    ctx.clobber(|buf| self.render(buf));
                    self.src = None;
                    self.dst = None;
                    self.grab_src = None;
                    self.grab_dst = None;
                } else {
                    self.dst = Some(pos);
                    ctx.preview(|buf| self.render(buf));
                }
            }

            _ => return None,
        }

        CONSUMED
    }
}

impl MoveTool {
    fn render(&self, buf: &mut Buffer) {
        let (src, dst) = option!(self.src, self.dst);

        let state: Vec<_> = visible_cells(buf, (src, dst)).collect();

        if let (Some(grab_src), Some(grab_dst)) = (self.grab_src, self.grab_dst) {
            for cell in state.iter() {
                buf.setv(true, cell.pos(), SP);
            }

            let delta = grab_dst.signed() - grab_src.signed();

            for cell in state.into_iter().map(|cell| cell.translate(delta)) {
                buf.setv(true, cell.pos(), cell.c());
            }
        } else {
            for cell in state {
                buf.setv(true, cell.pos(), cell.c());
            }
        }
    }
}
