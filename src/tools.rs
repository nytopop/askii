// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::Options;
use cursive::{direction::Absolute, Rect, Vec2};
use line_drawing::Bresenham;
use log::warn;
use std::fmt;

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
    fn load_opts(&mut self, opts: &Options) {}

    /// Returns any points that need to be rendered, or `None` if there isn't anything
    /// to render.
    // TODO: this is inflexible. instead, provide access to &mut Vec<Vec<char>>
    fn points(&self) -> Option<Vec<Point>>;

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

pub struct Point {
    pub pos: Vec2,
    pub c: Option<char>,
    pub hl: bool,
}

impl Point {
    fn plain(pos: Vec2, c: char) -> Self {
        Point {
            pos,
            c: Some(c),
            hl: false,
        }
    }

    fn curry_plain(c: char) -> impl Fn(Vec2) -> Self {
        move |pos| Self::plain(pos, c)
    }
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
    fn points(&self) -> Option<Vec<Point>> {
        let rect = Rect::from_corners(self.origin?, self.target?);
        let mut points = Vec::with_capacity((rect.width() * 2) + (rect.height() * 2));

        points.extend(
            (rect.left()..rect.right())
                .map(|x| Vec2::new(x, rect.top()))
                .map(Point::curry_plain('-')),
        );
        points.extend(
            (rect.left()..rect.right())
                .map(|x| Vec2::new(x, rect.bottom()))
                .map(Point::curry_plain('-')),
        );
        points.extend(
            (rect.top()..rect.bottom())
                .map(|y| Vec2::new(rect.left(), y))
                .map(Point::curry_plain('|')),
        );
        points.extend(
            (rect.top()..rect.bottom())
                .map(|y| Vec2::new(rect.right(), y))
                .map(Point::curry_plain('|')),
        );

        points.extend(
            vec![
                rect.top_left(),
                rect.top_right(),
                rect.bottom_left(),
                rect.bottom_right(),
            ]
            .into_iter()
            .map(Point::curry_plain('+')),
        );

        Some(points)
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
    }

    fn points(&self) -> Option<Vec<Point>> {
        let origin = self.origin?;
        let target = self.target?;

        let mut points = vec![];

        if self.direct {
            let s = (origin.x as isize, origin.y as isize);
            let e = (target.x as isize, target.y as isize);

            points.extend(Bresenham::new(s, e).steps().map(|(s, e)| {
                Point::plain(
                    Vec2::new(s.0 as usize, s.1 as usize),
                    match line_slope(s, e) {
                        (0, _) => '|',
                        (_, 0) => '-',
                        (x, y) if (x > 0) == (y > 0) => '\\',
                        _ => '/',
                    },
                )
            }));

            points.first_mut().map(|p| p.c.replace('+'));
            points.push(Point::plain(target, '+'));

            return Some(points);
        }

        // TODO: complete this
        // we can still use bresenham, just change the endpoints
        // ah, problem is: the route requires buffer access
        //
        // so, for proper behavior:
        // * if the endpoint is -, do x -> y
        // * otherwise, do y -> x
        if self.snap45 {}

        // snap90

        Some(points)
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

fn line_slope(origin: (isize, isize), target: (isize, isize)) -> (isize, isize) {
    let mut x = target.0 - origin.0;
    let mut y = target.1 - origin.1;

    if_let!(d = gcd(x, y); d != 0 => {
        x /= d;
        y /= d;
    });

    (x, y)
}

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
