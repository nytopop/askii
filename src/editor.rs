// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::tools::*;
use cursive::{theme::ColorStyle, view::View, Printer, Rect, Vec2, XY};
use line_drawing::Bresenham;
use std::{
    cmp,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    iter::{self, FromIterator},
    path::PathBuf,
};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    author = "Made with love by nytopop <ericizoita@gmail.com>.",
    help_message = "Prints help information.",
    version_message = "Prints version information."
)]
pub struct Options {
    // true : lines bend 45 degrees
    // false: lines bend 90 degrees
    #[structopt(skip = false)]
    pub line_snap45: bool,

    /// Text file to operate on.
    #[structopt(name = "FILE")]
    pub file: PathBuf,
}

// TODO: undo/redo
// TODO: new, open, save, save as
// TODO: help window
// TODO: path mode for line/arrow
// TODO: text tool
// TODO: resize tool
// TODO: select tool
// TODO: unicode mode
// TODO: diamond tool
// TODO: hexagon tool
// TODO: scroll when a tool goes offscreen, like w/ rpointer
pub struct Editor {
    opts: Options,
    file: File,
    buffer: Buffer,
    tool: Box<dyn Tool>,
}

impl View for Editor {
    fn draw(&self, printer: &Printer<'_, '_>) {
        let buf = &mut [0; 4];

        let tl = printer.content_offset;
        let br = printer.content_offset + printer.size;
        let area = Rect::from_corners(tl, br);

        for (hl, Cell { pos, c }) in self
            .buffer
            .iter()
            .map(Char::into_hl_pair)
            .filter(|(_, Cell { pos, .. })| area.contains(*pos))
        {
            let s = c.encode_utf8(buf);

            if hl {
                printer.with_color(ColorStyle::highlight_inactive(), |p| p.print(pos, s));
            } else {
                printer.print(pos, s);
            }
        }
    }

    fn required_size(&mut self, _size: Vec2) -> Vec2 {
        self.buffer.bounds()
    }
}

impl Editor {
    /// Open an editor instance with the provided options.
    pub fn open(opts: Options) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&opts.file)?;

        let mut editor = Self {
            opts,
            file,
            buffer: Buffer::default(),
            tool: Box::new(BoxTool::default()),
        };

        editor.load_from_file()?;

        Ok(editor)
    }

    /// Load buffer state from backing storage.
    fn load_from_file(&mut self) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(0))?;

        self.buffer = BufReader::new(&mut self.file)
            .lines()
            .map(|lr| lr.map(|s| s.chars().collect()))
            .collect::<io::Result<_>>()?;

        Ok(())
    }

    /// Returns a mutable reference to the loaded options.
    pub fn opts(&mut self) -> &mut Options {
        &mut self.opts
    }

    /// Set the active tool.
    pub fn set_tool<T: Tool + 'static>(&mut self, tool: T) {
        self.tool = Box::new(tool);
        self.tool.load_opts(&self.opts);
    }

    /// Returns the active tool as a human readable string.
    pub fn active_tool(&self) -> String {
        format!("{{ {} }}", self.tool)
    }

    pub fn press(&mut self, pos: Vec2) {
        let keep_changes = self.tool.on_press(pos);
        self.apply_toolstate(keep_changes);
    }

    pub fn hold(&mut self, pos: Vec2) {
        let keep_changes = self.tool.on_hold(pos);
        self.apply_toolstate(keep_changes);
    }

    pub fn release(&mut self, pos: Vec2) {
        let keep_changes = self.tool.on_release(pos);
        self.apply_toolstate(keep_changes);
        self.tool.reset();
    }

    fn apply_toolstate(&mut self, keep_changes: bool) {
        self.buffer.discard_edits();

        self.tool.render_to(&mut self.buffer);

        if keep_changes {
            self.buffer.save_edits();
        }
    }
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
const S_E: (isize, isize) = (1, 0);
const S_S: (isize, isize) = (0, 1);
const S_W: (isize, isize) = (-1, 0);
//const S_NE: (isize, isize) = (1, -1);
//const S_SE: (isize, isize) = (1, 1);
//const S_SW: (isize, isize) = (-1, 1);
//const S_NW: (isize, isize) = (-1, -1);

#[derive(Clone, Default)]
pub struct Buffer {
    chars: Vec<Vec<char>>,
    edits: Vec<Cell>,
}

#[derive(Copy, Clone)]
struct Cell {
    pos: Vec2,
    c: char,
}

#[derive(Copy, Clone)]
enum Char {
    Clean(Cell),
    Dirty(Cell),
}

impl Char {
    fn into_hl_pair(self) -> (bool, Cell) {
        match self {
            Char::Clean(cell) => (false, cell),
            Char::Dirty(cell) => (true, cell),
        }
    }
}

impl FromIterator<Vec<char>> for Buffer {
    fn from_iter<T: IntoIterator<Item = Vec<char>>>(iter: T) -> Self {
        let chars = iter.into_iter().collect();
        let edits = vec![];
        Self { chars, edits }
    }
}

impl Buffer {
    fn bounds(&self) -> Vec2 {
        let x = self.chars.iter().map(Vec::len).max().unwrap_or(0);
        let y = self.chars.len();

        let ex = self
            .edits
            .iter()
            .map(|Cell { pos, .. }| pos.x)
            .max()
            .unwrap_or(0);

        let ey = self
            .edits
            .iter()
            .map(|Cell { pos, .. }| pos.y)
            .max()
            .unwrap_or(0);

        Vec2::new(cmp::max(x, ex), cmp::max(y, ey))
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = Char> + 'a {
        self.chars
            .iter()
            .enumerate()
            .flat_map(|(y, xs)| {
                xs.iter()
                    .copied()
                    .enumerate()
                    .map(move |(x, c)| (Vec2::new(x, y), c))
                    .map(|(pos, c)| Cell { pos, c })
                    .map(Char::Clean)
            })
            .chain(self.edits.iter().copied().map(Char::Dirty))
    }

    /// Get the cell at `pos`, if it exists.
    ///
    /// Does not consider any pending edits.
    pub fn getv(&self, pos: Vec2) -> Option<char> {
        self.chars.get(pos.y).and_then(|v| v.get(pos.x)).copied()
    }

    /// Returns `true` iff the cell at `pos` exists and contains a non-whitespace
    /// character.
    ///
    /// Does not consider any pending edits.
    pub fn visible(&self, pos: Vec2) -> bool {
        self.getv(pos).map(|c| !c.is_whitespace()).unwrap_or(false)
    }

    /// Set the cell at `pos` to `c`.
    pub fn setv(&mut self, force: bool, pos: Vec2, c: char) {
        let existing = self
            .iter_at(pos)
            .map(|Cell { c, .. }| c)
            .map(precedence)
            .max()
            .unwrap_or(0);

        if force || precedence(c) >= existing {
            self.edits.push(Cell { pos, c });
        }
    }

    /// Set the cell at `(x, y)` to `c`.
    pub fn set(&mut self, force: bool, x: usize, y: usize, c: char) {
        self.setv(force, Vec2::new(x, y), c)
    }

    fn iter_at<'a>(&'a self, pos: Vec2) -> Box<dyn Iterator<Item = Cell> + 'a> {
        let tail = self.edits.iter().filter(move |c| c.pos == pos).copied();

        if self.chars.len() > pos.y && self.chars[pos.y].len() > pos.x {
            Box::new(
                iter::once(Cell {
                    pos,
                    c: self.chars[pos.y][pos.x],
                })
                .chain(tail),
            )
        } else {
            Box::new(tail)
        }
    }

    /// Flush any pending edits to the primary buffer, allocating as necessary.
    fn save_edits(&mut self) {
        for Cell {
            pos: Vec2 { x, y },
            c,
            ..
        } in self.edits.drain(..)
        {
            if self.chars.len() <= y {
                self.chars.resize_with(y + 1, Vec::default);
            }
            if self.chars[y].len() <= x {
                self.chars[y].resize(x + 1, SP);
            }
            self.chars[y][x] = c;
        }
    }

    /// Discard any pending edits.
    fn discard_edits(&mut self) {
        self.edits.clear();
    }

    /// Draw a line from `origin` to `target`.
    pub fn draw_line(&mut self, origin: Vec2, target: Vec2) {
        for (i, (s, e)) in Bresenham::new(origin.signed().pair(), target.signed().pair())
            .steps()
            .enumerate()
        {
            let c = match line_slope(s, e).pair() {
                _ if i == 0 => PLUS,
                (0, _) => PIPE,
                (_, 0) => DASH,
                (x, y) if (x > 0) == (y > 0) => GAID,
                _ => DIAG,
            };

            self.set(false, s.0 as usize, s.1 as usize, c);
        }

        self.setv(false, target, PLUS);
    }

    /// Draw an arrow from `origin` to `target`.
    pub fn draw_arrow(&mut self, origin: Vec2, target: Vec2) {
        let tip = match line_slope(origin, target).pair() {
            S_N => N,
            S_E => E,
            S_S => S,
            S_W => W,

            // SE
            (x, y) if x > 0 && y > 0 && self.visible(target.map_x(|x| x + 1)) => E,
            (x, y) if x > 0 && y > 0 => S,

            // NE
            (x, y) if x > 0 && y < 0 && self.visible(target.map_x(|x| x + 1)) => E,
            (x, y) if x > 0 && y < 0 => N,

            // SW
            (x, y) if x < 0 && y > 0 && target.x == 0 => S,
            (x, y) if x < 0 && y > 0 && self.visible(target.map_x(|x| x - 1)) => W,
            (x, y) if x < 0 && y > 0 => S,

            // NW
            (x, y) if x < 0 && y < 0 && target.x == 0 => N,
            (x, y) if x < 0 && y < 0 && self.visible(target.map_x(|x| x - 1)) => W,
            (x, y) if x < 0 && y < 0 => N,

            (_, _) => PLUS,
        };

        self.draw_line(origin, target);
        self.setv(true, target, tip);
    }
}

/// Returns the overlap precedence for `c`.
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

/// Returns the slope between points at `origin` and `target`.
///
/// The resulting fraction will be reduced to its simplest terms.
pub fn line_slope<P: Into<XY<isize>>>(origin: P, target: P) -> XY<isize> {
    let xy = target.into() - origin;

    match gcd(xy.x, xy.y) {
        0 => xy,
        d => xy / d,
    }
}

/// Returns the greatest common denominator between `a` and `b`.
pub fn gcd(mut a: isize, mut b: isize) -> isize {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs()
}
