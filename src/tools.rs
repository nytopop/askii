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
    // TODO: make a better buffer type, with drawing primitives as methods
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

const SP: char = ' ';
const DASH: char = '-';
const PIPE: char = '|';
const DIAG: char = '/';
const GAID: char = '\\';
const PLUS: char = '+';

const N: char = '^';
const S: char = 'v';
const W: char = '<';
const E: char = '>';

const S_N: (isize, isize) = (0, -1);
const S_NE: (isize, isize) = (1, -1);
const S_E: (isize, isize) = (1, 0);
const S_SE: (isize, isize) = (1, 1);
const S_S: (isize, isize) = (0, 1);
const S_SW: (isize, isize) = (-1, 1);
const S_W: (isize, isize) = (-1, 0);
const S_NW: (isize, isize) = (-1, -1);

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
    fn load_opts(&mut self, opts: &Options) {}

    fn render_to(&self, buffer: &mut Vec<Vec<char>>) -> bool {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            _ => return false,
        };

        let r = Rect::from_corners(origin, target);

        draw_line(buffer, r.top_left(), r.top_right());
        draw_line(buffer, r.top_right(), r.bottom_right());
        draw_line(buffer, r.bottom_right(), r.bottom_left());
        draw_line(buffer, r.bottom_left(), r.top_left());

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

    fn render_to(&self, buffer: &mut Vec<Vec<char>>) -> bool {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            _ => return false,
        };

        let mid = if self.snap45 {
            line_midpoint_45(origin, target)
        } else {
            match getv(buffer, target) {
                Some(DASH) => Vec2::new(target.x, origin.y),
                _ => Vec2::new(origin.x, target.y),
            }
        };
        draw_line(buffer, origin, mid);
        draw_line(buffer, mid, target);

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

fn line_midpoint_45(origin: Vec2, target: Vec2) -> Vec2 {
    let delta = cmp::min(diff(origin.y, target.y), diff(origin.x, target.x));

    match line_slope_v(origin, target) {
        s if s.x < 0 && s.y < 0 => target.map(|v| v + delta),
        s if s.x > 0 && s.y < 0 => target.map_x(|x| x - delta).map_y(|y| y + delta),
        s if s.x < 0 && s.y > 0 => target.map_x(|x| x + delta).map_y(|y| y - delta),
        s if s.x > 0 && s.y > 0 => target.map(|v| v - delta),
        _ => origin,
    }
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

    fn render_to(&self, buffer: &mut Vec<Vec<char>>) -> bool {
        let (origin, target) = match (self.origin, self.target) {
            (Some(o), Some(t)) => (o, t),
            _ => return false,
        };

        let mid = if self.snap45 {
            line_midpoint_45(origin, target)
        } else {
            // TODO(bug): this snaps the guiding line based on the position of the arrow
            // tip, when it should be based on the character immediately beyond the tip.
            // * check target x +/- 1, y +/- 1?
            match getv(buffer, target) {
                Some(DASH) => Vec2::new(target.x, origin.y),
                __________ => Vec2::new(origin.x, target.y),
            }
        };

        if mid != target {
            draw_line(buffer, origin, mid);
            draw_arrow(buffer, mid, target);
        } else {
            draw_arrow(buffer, origin, target);
        }

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
pub struct TextTool {
    //
}

impl fmt::Display for TextTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Text")
    }
}

impl Tool for TextTool {
    fn load_opts(&mut self, opts: &Options) {}

    fn render_to(&self, buffer: &mut Vec<Vec<char>>) -> bool {
        false
    }

    fn on_press(&mut self, pos: Vec2) -> bool {
        false
    }

    fn on_hold(&mut self, pos: Vec2) -> bool {
        false
    }

    fn on_release(&mut self, pos: Vec2) -> bool {
        false
    }

    fn reset(&mut self) {}
}

/// Draw an arrow from origin to target.
fn draw_arrow(buffer: &mut Vec<Vec<char>>, origin: Vec2, target: Vec2) {
    draw_line(buffer, origin, target);

    let c = match line_slope_v(origin, target).pair() {
        S_N => N,
        S_E => E,
        S_S => S,
        S_W => W,

        // SE
        (x, y) if x > 0 && y > 0 && exists(buffer, target.x + 1, target.y) => E,
        (x, y) if x > 0 && y > 0 => S,

        // NE
        (x, y) if x > 0 && y < 0 && exists(buffer, target.x + 1, target.y) => E,
        (x, y) if x > 0 && y < 0 => N,

        // SW
        (x, y) if x < 0 && y > 0 && target.x == 0 => S,
        (x, y) if x < 0 && y > 0 && exists(buffer, target.x - 1, target.y) => W,
        (x, y) if x < 0 && y > 0 => S,

        // NW
        (x, y) if x < 0 && y < 0 && target.x == 0 => N,
        (x, y) if x < 0 && y < 0 && exists(buffer, target.x - 1, target.y) => W,
        (x, y) if x < 0 && y < 0 => N,

        _ => PLUS,
    };

    setf_v(buffer, target, c);
}

/// Draw a line from origin to target.
fn draw_line(buffer: &mut Vec<Vec<char>>, origin: Vec2, target: Vec2) {
    let s = (origin.x as isize, origin.y as isize);
    let e = (target.x as isize, target.y as isize);

    for (i, (s, e)) in Bresenham::new(s, e).steps().enumerate() {
        let c = match line_slope(s, e) {
            _ if i == 0 => PLUS,
            (0, _) => PIPE,
            (_, 0) => DASH,
            (x, y) if (x > 0) == (y > 0) => GAID,
            _ => DIAG,
        };

        setp(buffer, s.0 as usize, s.1 as usize, c);
    }

    setp_v(buffer, target, PLUS);
}

/// Set the cell at pos to c, respecting character precedence.
///
/// Allocates additional storage if necessary, setting empty cells to `SP`.
fn setp_v(buffer: &mut Vec<Vec<char>>, pos: Vec2, c: char) {
    setp(buffer, pos.x, pos.y, c)
}

/// Set the cell at (x, y) to c, respecting character precedence.
///
/// Allocates additional storage if necessary, setting empty cells to `SP`.
fn setp(buffer: &mut Vec<Vec<char>>, x: usize, y: usize, c: char) {
    while buffer.len() <= y {
        buffer.push(vec![]);
    }
    while buffer[y].len() <= x {
        buffer[y].push(SP);
    }

    if precedence(c) >= precedence(buffer[y][x]) {
        buffer[y][x] = c;
    }
}

/// Set the cell at pos to c, ignoring character precedence.
///
/// Allocates additional storage if necessary, setting empty cells to `SP`.
fn setf_v(buffer: &mut Vec<Vec<char>>, pos: Vec2, c: char) {
    setf(buffer, pos.x, pos.y, c)
}

/// Set the cell at (x, y) to c, ignoring character precedence.
///
/// Allocates additional storage if necessary, setting empty cells to `SP`.
fn setf(buffer: &mut Vec<Vec<char>>, x: usize, y: usize, c: char) {
    while buffer.len() <= y {
        buffer.push(vec![]);
    }
    while buffer[y].len() <= x {
        buffer[y].push(SP);
    }
    buffer[y][x] = c;
}

/// Returns the overlap precedence for c.
fn precedence(c: char) -> usize {
    match c {
        PLUS => 5,
        DASH => 4,
        PIPE => 3,
        DIAG => 2,
        GAID => 1,
        _ => 0,
    }
}

/// Returns true if the cell at (x, y) exists and is not `SP`.
fn exists(buffer: &Vec<Vec<char>>, x: usize, y: usize) -> bool {
    get(buffer, x, y).unwrap_or(SP) != SP
}

/// Returns the character at pos, if it exists.
fn getv(buffer: &Vec<Vec<char>>, pos: Vec2) -> Option<char> {
    get(buffer, pos.x, pos.y)
}

/// Returns the character at (x, y), if it exists.
fn get(buffer: &Vec<Vec<char>>, x: usize, y: usize) -> Option<char> {
    buffer.get(y).and_then(|v| v.get(x)).copied()
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
