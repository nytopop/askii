// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::{
    editor::{line_slope, Buffer},
    Options,
};
use cursive::{Rect, Vec2};
use std::{cmp, fmt};

pub trait Tool: fmt::Display {
    /// Configure this tool with the provided options.
    // TODO: settings menu is difficult to use, alternatives:
    // * checkboxes that are scoped per tool, in a toolbar under the menubar
    fn load_opts(&mut self, _: &Options) {}

    /// Render to the provided buffer, returning false iff no changes were made.
    fn render_to(&self, buffer: &mut Buffer);

    /// Callback to execute when the left mouse button is pressed. Returns whether the
    /// next call to `render_to` should be saved.
    // TODO: mem::swap will allow these to take &mut Cursive for popup windows.
    // * swap tool w/ NoopTool or something
    // * drop reference to editor so siv isn't aliased
    // * call tool handle w/ siv
    // * swap tool back to the editor
    //
    // kinda janky, but workable
    fn on_press(&mut self, pos: Vec2) -> bool;

    /// Callback to execute when the left mouse button is held. Returns whether the
    /// next call to `render_to` should be saved.
    fn on_hold(&mut self, pos: Vec2) -> bool;

    /// Callback to execute when the left mouse button is released. Returns whether the
    /// next call to `render_to` should be saved.
    fn on_release(&mut self, pos: Vec2) -> bool;

    /// Reset any internal state, if applicable.
    fn reset(&mut self) {}
}

#[derive(Copy, Clone, Default, Debug)]
pub struct BoxTool {
    origin: Option<Vec2>,
    target: Option<Vec2>,
}

impl fmt::Display for BoxTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Box")
    }
}

impl Tool for BoxTool {
    fn render_to(&self, buf: &mut Buffer) {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            (Some(o), None) => (o, o),
            _ => return,
        };

        let r = Rect::from_corners(origin, target);

        buf.draw_line(r.top_left(), r.top_right());
        buf.draw_line(r.top_right(), r.bottom_right());
        buf.draw_line(r.bottom_right(), r.bottom_left());
        buf.draw_line(r.bottom_left(), r.top_left());
    }

    fn on_press(&mut self, pos: Vec2) -> bool {
        self.origin = Some(pos);
        false
    }

    fn on_hold(&mut self, pos: Vec2) -> bool {
        self.target = Some(pos);
        false
    }

    fn on_release(&mut self, pos: Vec2) -> bool {
        self.target = Some(pos);
        true
    }

    fn reset(&mut self) {
        self.origin = None;
        self.target = None;
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct LineTool {
    origin: Option<Vec2>,
    target: Option<Vec2>,
    snap45: bool,
}

impl fmt::Display for LineTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.snap45 {
            write!(f, "Line: Snap 45")
        } else {
            write!(f, "Line: Snap 90")
        }
    }
}

impl Tool for LineTool {
    fn load_opts(&mut self, opts: &Options) {
        self.snap45 = opts.line_snap45;
    }

    fn render_to(&self, buf: &mut Buffer) {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            (Some(o), None) => (o, o),
            _ => return,
        };

        let mid = if self.snap45 {
            line_midpoint_45(origin, target)
        } else {
            match buf.getv(target) {
                Some('-') => Vec2::new(target.x, origin.y),
                _ => Vec2::new(origin.x, target.y),
            }
        };

        buf.draw_line(origin, mid);
        buf.draw_line(mid, target);
    }

    fn on_press(&mut self, pos: Vec2) -> bool {
        self.origin = Some(pos);
        false
    }

    fn on_hold(&mut self, pos: Vec2) -> bool {
        self.target = Some(pos);
        false
    }

    fn on_release(&mut self, pos: Vec2) -> bool {
        self.target = Some(pos);
        true
    }

    fn reset(&mut self) {
        self.origin = None;
        self.target = None;
    }
}

fn line_midpoint_45(origin: Vec2, target: Vec2) -> Vec2 {
    let delta = cmp::min(diff(origin.y, target.y), diff(origin.x, target.x));

    match line_slope(origin, target).pair() {
        (x, y) if x < 0 && y < 0 => target.map(|v| v + delta),
        (x, y) if x > 0 && y < 0 => target.map_x(|x| x - delta).map_y(|y| y + delta),
        (x, y) if x < 0 && y > 0 => target.map_x(|x| x + delta).map_y(|y| y - delta),
        (x, y) if x > 0 && y > 0 => target.map(|v| v - delta),
        _ => origin,
    }
}

/// Returns the absolute difference between x and y.
fn diff(x: usize, y: usize) -> usize {
    (x as isize - y as isize).abs() as usize
}

#[derive(Copy, Clone, Default, Debug)]
pub struct ArrowTool {
    origin: Option<Vec2>,
    target: Option<Vec2>,
    snap45: bool,
}

impl fmt::Display for ArrowTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.snap45 {
            write!(f, "Arrow: Snap 45")
        } else {
            write!(f, "Arrow: Snap 90")
        }
    }
}

impl Tool for ArrowTool {
    fn load_opts(&mut self, opts: &Options) {
        self.snap45 = opts.line_snap45;
    }

    fn render_to(&self, buf: &mut Buffer) {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            (Some(o), None) => (o, o),
            _ => return,
        };

        let mid = if self.snap45 {
            line_midpoint_45(origin, target)
        } else {
            // TODO(bug): this snaps the guiding line based on the position of the arrow
            // tip, when it should be based on the character immediately beyond the tip.
            // * check target x +/- 1, y +/- 1?
            match buf.getv(target) {
                Some('-') => Vec2::new(target.x, origin.y),
                _ => Vec2::new(origin.x, target.y),
            }
        };

        if mid != target {
            buf.draw_line(origin, mid);
            buf.draw_arrow(mid, target);
        } else {
            buf.draw_arrow(origin, target);
        }
    }

    fn on_press(&mut self, pos: Vec2) -> bool {
        self.origin = Some(pos);
        false
    }

    fn on_hold(&mut self, pos: Vec2) -> bool {
        self.target = Some(pos);
        false
    }

    fn on_release(&mut self, pos: Vec2) -> bool {
        self.target = Some(pos);
        true
    }

    fn reset(&mut self) {
        self.origin = None;
        self.target = None;
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct TextTool {
    //
}

impl fmt::Display for TextTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Text")
    }
}

impl Tool for TextTool {
    fn render_to(&self, _buf: &mut Buffer) {
        //
    }

    fn on_press(&mut self, _pos: Vec2) -> bool {
        false
    }

    fn on_hold(&mut self, _pos: Vec2) -> bool {
        false
    }

    fn on_release(&mut self, _pos: Vec2) -> bool {
        false
    }
}
