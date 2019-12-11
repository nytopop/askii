// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::{
    editor::{Buffer, Char, EditorCtx, CONSUMED},
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

/// Provides an implementation of `Tool::on_event` for tools that contain an `origin`
/// and `target` field of type `Option<Vec2>`. The implementation performs basic left
/// mouse drag handling, calling the argument closure when relevant events occur.
macro_rules! fn_on_event_drag {
    ($render:expr) => {
    fn on_event(&mut self, ctx: &mut EditorCtx<'_>, event: &Event) -> Option<EventResult> {
        let (pos, event) = mouse_drag!(ctx, event);

        match event {
            Press(Left) => {
                self.origin = Some(pos);
                self.target = Some(pos);
                ctx.preview(|buf| $render(self, buf));
            }

            Hold(Left) => {
                self.target = Some(pos);
                ctx.preview(|buf| $render(self, buf));
            }

            Release(Left) => {
                self.target = Some(pos);
                ctx.clobber(|buf| $render(self, buf));
                self.origin = None;
                self.target = None;
            }

            _ => return None,
        }

        CONSUMED
    }
    }
}

pub(crate) trait Tool: fmt::Display {
    fn load_opts(&mut self, _: &Options) {}

    fn on_event(&mut self, ctx: &mut EditorCtx<'_>, e: &Event) -> Option<EventResult>;
}

#[derive(Copy, Clone, Default)]
pub(crate) struct BoxTool {
    origin: Option<Vec2>,
    target: Option<Vec2>,
}

impl fmt::Display for BoxTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Box")
    }
}

impl Tool for BoxTool {
    fn_on_event_drag!(|t: &Self, buf: &mut Buffer| {
        let (origin, target) = option!(t.origin, t.target);

        let r = Rect::from_corners(origin, target);

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
    origin: Option<Vec2>,
    target: Option<Vec2>,
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
        let (origin, target) = option!(t.origin, t.target);

        if let PathMode::Routed = t.path_mode {
            buf.draw_path(origin, target);
            return;
        }

        let mid = match t.path_mode {
            PathMode::Snap90 => buf.snap90(origin, target),
            _ => buf.snap45(origin, target),
        };

        buf.draw_line(origin, mid);
        buf.draw_line(mid, target);
    });
}

#[derive(Copy, Clone, Default)]
pub(crate) struct ArrowTool {
    origin: Option<Vec2>,
    target: Option<Vec2>,
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
        let (origin, target) = option!(t.origin, t.target);

        if let PathMode::Routed = t.path_mode {
            let last = buf.draw_path(origin, target);
            buf.draw_arrow_tip(last, target);
            return;
        }

        let mid = match t.path_mode {
            PathMode::Snap90 => buf.snap90(origin, target),
            _ => buf.snap45(origin, target),
        };

        if mid != target {
            buf.draw_line(origin, mid);
            buf.draw_line(mid, target);
            buf.draw_arrow_tip(mid, target);
        } else {
            buf.draw_line(origin, target);
            buf.draw_arrow_tip(origin, target);
        }
    });
}

#[derive(Clone)]
pub(crate) struct TextTool {
    origin: Option<Vec2>,
    ready: bool,
    buffer: Vec<Vec<char>>,
    cursor: Vec2,
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            origin: None,
            ready: false,
            buffer: vec![],
            cursor: Vec2::new(0, 0),
        }
    }
}

impl fmt::Display for TextTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Text")
    }
}

impl Tool for TextTool {
    fn on_event(&mut self, ctx: &mut EditorCtx<'_>, event: &Event) -> Option<EventResult> {
        let Vec2 { x, y } = &mut self.cursor;

        match ctx.relativize(event) {
            Event::Mouse {
                event: Press(Left),
                position,
                ..
            } => {
                self.origin = Some(position);
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
                self.origin = None;
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
        let origin = option!(self.origin);

        for (y, line) in self.buffer.iter().enumerate() {
            for (x, c) in line.iter().enumerate() {
                let pos = Vec2::new(x, y) + origin;
                buf.setv(true, pos, *c);
            }
        }

        buf.set_cursor(self.cursor + origin);
    }
}

#[derive(Copy, Clone, Default)]
pub(crate) struct EraseTool {
    origin: Option<Vec2>,
    target: Option<Vec2>,
}

impl fmt::Display for EraseTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Erase")
    }
}

impl Tool for EraseTool {
    fn_on_event_drag!(|t: &Self, buf: &mut Buffer| {
        let (origin, target) = option!(t.origin, t.target);

        let r = Rect::from_corners(origin, target);

        let cells: Vec<_> = buf
            .iter_within(r.top_left(), r.size())
            .flat_map(|c| match c {
                Char::Clean(cell) => Some(cell),
                Char::Dirty(cell) => Some(cell),
                _ => None,
            })
            .filter(|cell| !cell.is_whitespace())
            .map(|cell| cell.pos())
            .collect();

        for pos in cells {
            buf.setv(true, pos, ' ');
        }
    });
}
