// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::Options;
use cursive::{direction::Absolute, Rect, Vec2, XY};
use line_drawing::Bresenham;
use log::warn;
use std::{cmp, fmt};

macro_rules! if_let {
    ($i:pat = $e:expr; $p:expr => $x:expr) => {
        #[allow(irrefutable_let_patterns)]
        {
            if let $i = $e {
                if $p {
                    $x
                }
            }
        }
    };
}

pub trait Tool: fmt::Display {
    /// Configure this tool with the provided options.
    // TODO: settings menu is difficult to use, alternatives:
    // * checkboxes that are scoped per tool, in a toolbar under the menubar
    fn load_opts(&mut self, opts: &Options);

    /// Render to the provided buffer, returning false iff no changes were made.
    fn render_to(&self, buffer: &mut Vec<Vec<char>>) -> bool;

    /// Callback to execute when the left mouse button is pressed. Returns whether the
    /// next call to `points` should be saved.
    // TODO: mem::swap will allow these to take &mut Cursive for popup windows.
    // * swap tool w/ NoopTool or something
    // * drop reference to editor so siv isn't aliased
    // * call tool handle w/ siv
    // * swap tool back to the editor
    //
    // kinda janky, but workable
    fn on_press(&mut self, pos: Vec2) -> bool;

    /// Callback to execute when the left mouse button is held. Returns whether the
    /// next call to `points` should be saved.
    fn on_hold(&mut self, pos: Vec2) -> bool;

    /// Callback to execute when the left mouse button is released. Returns whether the
    /// next call to `points` should be saved.
    fn on_release(&mut self, pos: Vec2) -> bool;

    /// Reset any internal state, if applicable.
    fn reset(&mut self);
}

#[derive(Copy, Clone, Default, Debug)]
pub struct BoxTool {
    origin: Option<Vec2>,
    target: Option<Vec2>,

    overlap_h: Option<bool>,
}

impl fmt::Display for BoxTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Box")
    }
}

impl Tool for BoxTool {
    fn load_opts(&mut self, opts: &Options) {
        self.overlap_h = opts.overlap_h;
    }

    fn render_to(&self, buffer: &mut Vec<Vec<char>>) -> bool {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            _ => return false,
        };

        let r = Rect::from_corners(origin, target);

        draw_line(self.overlap_h, buffer, r.top_left(), r.top_right());
        draw_line(self.overlap_h, buffer, r.top_right(), r.bottom_right());
        draw_line(self.overlap_h, buffer, r.bottom_right(), r.bottom_left());
        draw_line(self.overlap_h, buffer, r.bottom_left(), r.top_left());

        true
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

    direct: bool,
    snap45: bool,
    overlap_h: Option<bool>,
}

impl fmt::Display for LineTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.direct {
            write!(f, "Line [ Direct ]")
        } else if self.snap45 {
            write!(f, "Line [ Snap 45 ]")
        } else {
            write!(f, "Line [ Snap 90 ]")
        }
    }
}

impl Tool for LineTool {
    fn load_opts(&mut self, opts: &Options) {
        self.direct = opts.line_direct;
        self.snap45 = opts.line_snap45;
        self.overlap_h = opts.overlap_h;
    }

    fn render_to(&self, buffer: &mut Vec<Vec<char>>) -> bool {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            _ => return false,
        };

        if self.direct {
            draw_line(self.overlap_h, buffer, origin, target);
            return true;
        }

        let mid = if self.snap45 {
            let delta = cmp::min(diff(origin.y, target.y), diff(origin.x, target.x));

            match line_slope_v(origin, target) {
                s if s.x < 0 && s.y < 0 => target.map(|v| v + delta),
                s if s.x > 0 && s.y < 0 => target.map_x(|x| x - delta).map_y(|y| y + delta),
                s if s.x < 0 && s.y > 0 => target.map_x(|x| x + delta).map_y(|y| y - delta),
                s if s.x > 0 && s.y > 0 => target.map(|v| v - delta),
                _ => origin,
            }
        } else {
            match buffer.get(target.y).and_then(|buf| buf.get(target.x)) {
                Some('-') => Vec2::new(target.x, origin.y),
                _ => Vec2::new(origin.x, target.y),
            }
        };

        draw_line(self.overlap_h, buffer, origin, mid);
        draw_line(self.overlap_h, buffer, mid, target);

        true
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

/// Draw a line from origin to target.
fn draw_line(overlap_h: Option<bool>, buffer: &mut Vec<Vec<char>>, origin: Vec2, target: Vec2) {
    let s = (origin.x as isize, origin.y as isize);
    let e = (target.x as isize, target.y as isize);

    for (i, (s, e)) in Bresenham::new(s, e).steps().enumerate() {
        let c = match line_slope(s, e) {
            _ if i == 0 => '+',
            (0, _) => '|',
            (_, 0) => '-',
            (x, y) if (x > 0) == (y > 0) => '\\',
            _ => '/',
        };

        set(overlap_h, buffer, s.0 as usize, s.1 as usize, c);
    }

    set(overlap_h, buffer, target.x, target.y, '+');
}

/// Set the cell at (x, y) to c, respecting overlap rules.
///
/// Allocates additional storage if necessary, setting empty cells to ' '.
fn set(overlap_h: Option<bool>, buffer: &mut Vec<Vec<char>>, x: usize, y: usize, c: char) {
    while buffer.len() <= y {
        buffer.push(vec![]);
    }
    while buffer[y].len() <= x {
        buffer[y].push(' ');
    }

    match (buffer[y][x], c, overlap_h) {
        ('-', '|', Some(true)) => {}
        ('|', '-', Some(false)) => {}
        _ => buffer[y][x] = c,
    }
}

type IVec2 = XY<isize>;

/// Returns the slope between points origin and target.
fn line_slope_v(origin: Vec2, target: Vec2) -> IVec2 {
    line_slope(IVec2::from(origin).pair(), IVec2::from(target).pair()).into()
}

/// Returns the x and y slope between points origin and target.
fn line_slope(origin: (isize, isize), target: (isize, isize)) -> (isize, isize) {
    let mut x = target.0 - origin.0;
    let mut y = target.1 - origin.1;

    if_let!(d = gcd(x, y); d != 0 => {
        x /= d;
        y /= d;
    });

    (x, y)
}

/// Returns the greatest common denominator between x and y.
fn gcd(x: isize, y: isize) -> isize {
    let mut x = x;
    let mut y = y;
    while y != 0 {
        let t = y;
        y = x % y;
        x = t;
    }

    x.abs()
}

/// Returns the absolute difference between x and y.
fn diff(x: usize, y: usize) -> usize {
    (x as isize - y as isize).abs() as usize
}
