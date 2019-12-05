// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::tools::*;
use cursive::{theme::ColorStyle, view::View, Printer, Rect, Vec2, XY};
use line_drawing::Bresenham;
use std::{
    cmp::{max, min},
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    iter::{self, FromIterator},
    mem,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    author = "Made with love by nytopop <ericizoita@gmail.com>.",
    help_message = "Print help information.",
    version_message = "Print version information."
)]
pub struct Options {
    // true : lines bend 45 degrees
    // false: lines bend 90 degrees
    #[structopt(skip = false)]
    pub line_snap45: bool,

    /// Keep trailing whitespace (on save).
    #[structopt(short, long)]
    pub keep_trailing_ws: bool,

    /// Strip all margin whitespace (on save).
    #[structopt(short, long)]
    pub strip_margin_ws: bool,

    /// Text file to operate on.
    #[structopt(name = "FILE")]
    pub file: Option<PathBuf>,
}

// TODO: path mode for line/arrow
// TODO: text tool
// TODO: resize tool
// TODO: select tool
// TODO: erase tool
// TODO: unicode mode
// TODO: diamond tool
// TODO: hexagon tool
// TODO: trapezoid tool
pub struct Editor {
    opts: Options,        // config options
    buffer: Buffer,       // editing buffer
    history: Vec<Buffer>, // undo history
    undone: Vec<Buffer>,  // undo undo history
    tool: Box<dyn Tool>,  // active tool
    bounds: Vec2,         // bounds of the canvas, if adjusted
    rendered: String,     // latest render (kept for the allocation)
}

impl View for Editor {
    fn draw(&self, p: &Printer<'_, '_>) {
        let buf = &mut [0; 4];
        let edit_style = ColorStyle::highlight_inactive();

        for c in self.buffer.iter_within(p.content_offset, p.size) {
            match c {
                Char::Clean(Cell { pos, c }) => {
                    p.print(pos, c.encode_utf8(buf));
                }
                Char::Dirty(Cell { pos, c }) => {
                    p.with_color(edit_style, |p| p.print(pos, c.encode_utf8(buf)));
                }
            }
        }
    }

    fn required_size(&mut self, size: Vec2) -> Vec2 {
        let buf_bounds = self.buffer.bounds();

        Vec2 {
            x: max(size.x, max(buf_bounds.x, self.bounds.x)),
            y: max(size.y, max(buf_bounds.y, self.bounds.y)),
        }
    }
}

impl Editor {
    /// Open an editor instance with the provided options.
    pub fn open(mut opts: Options) -> io::Result<Self> {
        let file = opts.file.take();

        let mut editor = Self {
            opts,
            buffer: Buffer::default(),
            history: vec![],
            undone: vec![],
            tool: Box::new(BoxTool::default()),
            bounds: Vec2::new(0, 0),
            rendered: String::default(),
        };

        if let Some(path) = file {
            editor.open_file(path)?;
        }

        Ok(editor)
    }

    /// Returns a mutable reference to the loaded options.
    pub fn opts_mut(&mut self) -> &mut Options {
        &mut self.opts
    }

    /// Returns `true` if the buffer has been modified since the last save.
    pub fn is_dirty(&self) -> bool {
        self.buffer.is_dirty()
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

    /// Returns a mutable reference to the canvas bounds.
    pub fn bounds(&mut self) -> &mut Vec2 {
        &mut self.bounds
    }

    /// Set the canvas x bound to the provided value.
    pub fn set_x_bound(&mut self, x: usize) {
        self.bounds.x = max(x, self.bounds.x);
    }

    /// Set the canvas y bound to the provided value.
    pub fn set_y_bound(&mut self, y: usize) {
        self.bounds.y = max(y, self.bounds.y);
    }

    fn path(&self) -> &Option<PathBuf> {
        &self.opts.file
    }

    /// Clear all buffer state and begin a blank diagram.
    pub fn clear(&mut self) {
        self.opts.file = None;
        self.buffer.clear();
        self.history.clear();
        self.undone.clear();
        self.bounds = Vec2::new(0, 0);
    }

    /// Open the file at `path`, discarding any changes to the current file, if any.
    ///
    /// No modifications have been performed if this returns `Err(_)`.
    pub fn open_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let file = OpenOptions::new().read(true).open(path.as_ref())?;

        let buffer = BufReader::new(file)
            .lines()
            .map(|lr| lr.map(|s| s.chars().collect()))
            .collect::<io::Result<_>>()?;

        self.clear();
        self.opts.file = Some(path.as_ref().into());
        self.buffer = buffer;

        Ok(())
    }

    /// Save the current buffer contents to disk.
    ///
    /// Returns `Ok(true)` if the buffer was saved, and `Ok(false)` if there is no path
    /// configured for saving.
    pub fn save(&mut self) -> io::Result<bool> {
        if let Some(path) = self.path() {
            let file = OpenOptions::new()
                .read(false)
                .write(true)
                .create(true)
                .open(path)?;

            self.render_to(file)?;
            self.buffer.clean();
        }

        Ok(self.path().is_some())
    }

    /// Save the current buffer contents to the file at `path`, and setting that as the
    /// new path for future calls to `save`.
    pub fn save_as<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        self.opts.file = Some(path.as_ref().into());
        self.save()?;

        Ok(())
    }

    /// Render to `file`, performing whitespace cleanup if enabled.
    fn render_to(&mut self, mut file: File) -> io::Result<()> {
        self.history.push(self.buffer.clone());
        self.bounds = Vec2::new(0, 0);
        self.buffer.discard_edits();

        if self.opts.strip_margin_ws {
            self.buffer.strip_margin_whitespace();
        } else if !self.opts.keep_trailing_ws {
            self.buffer.strip_trailing_whitespace();
        }

        if self.history.last().unwrap() == &self.buffer {
            self.history.pop();
        }

        self.rendered.clear();
        self.rendered.extend(self.buffer.iter());

        file.write_all(self.rendered.as_bytes())?;
        file.flush()?;
        file.sync_all()?;

        Ok(())
    }

    /// Trim whitespace from all margins.
    pub fn trim_margins(&mut self) {
        self.history.push(self.buffer.clone());

        self.bounds = Vec2::new(0, 0);
        self.buffer.discard_edits();
        self.buffer.strip_margin_whitespace();

        if self.history.last().unwrap() == &self.buffer {
            self.history.pop();
        }
    }

    /// Undo the last buffer modification.
    ///
    /// Returns `false` if there was nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.history
            .pop()
            .map(|buffer| mem::replace(&mut self.buffer, buffer))
            .map(|buffer| self.undone.push(buffer))
            .is_some()
    }

    /// Redo the last undone buffer modification.
    ///
    /// Returns `false` if there was nothing to redo.
    pub fn redo(&mut self) -> bool {
        self.undone
            .pop()
            .map(|buffer| mem::replace(&mut self.buffer, buffer))
            .map(|buffer| self.history.push(buffer))
            .is_some()
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

        if keep_changes {
            self.history.push(self.buffer.clone());
        }

        self.tool.render_to(&mut self.buffer);

        if keep_changes {
            self.buffer.save_edits();

            if self.history.last().unwrap() == &self.buffer {
                self.history.pop();
            } else {
                self.buffer.dirty();
            }
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

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Buffer {
    chars: Vec<Vec<char>>,
    edits: Vec<Cell>,
    dirty: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct Cell {
    pos: Vec2,
    c: char,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Char {
    Clean(Cell),
    Dirty(Cell),
}

impl FromIterator<Vec<char>> for Buffer {
    fn from_iter<T: IntoIterator<Item = Vec<char>>>(iter: T) -> Self {
        Self {
            chars: iter.into_iter().collect(),
            edits: vec![],
            dirty: false,
        }
    }
}

impl Buffer {
    /// Returns `true` if changes have been performed since the last call to `clean`.
    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn clean(&mut self) {
        self.dirty = false;
    }

    fn dirty(&mut self) {
        self.dirty = true;
    }

    /// Clears all content in the buffer.
    fn clear(&mut self) {
        self.chars.clear();
        self.edits.clear();
        self.dirty = false;
    }

    /// Returns the viewport size required to display all content within the buffer.
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

        Vec2::new(max(x, ex), max(y, ey))
    }

    /// Returns an iterator over all characters within the viewport formed by `offset`
    /// and `size`.
    fn iter_within<'a>(&'a self, offset: Vec2, size: Vec2) -> impl Iterator<Item = Char> + 'a {
        let area = Rect::from_corners(offset, offset + size);

        self.chars
            .iter()
            .enumerate()
            .skip(offset.y)
            .take(size.y)
            .flat_map(move |(y, xs)| {
                xs.iter()
                    .copied()
                    .enumerate()
                    .skip(offset.x)
                    .take(size.x)
                    .map(move |(x, c)| (Vec2::new(x, y), c))
                    .map(|(pos, c)| Cell { pos, c })
                    .map(Char::Clean)
            })
            .chain(
                self.edits
                    .iter()
                    .copied()
                    .filter(move |Cell { pos, .. }| area.contains(*pos))
                    .map(Char::Dirty),
            )
    }

    /// Returns an iterator over all characters in the buffer, injecting newlines
    /// where appropriate.
    fn iter<'a>(&'a self) -> impl Iterator<Item = char> + 'a {
        self.chars
            .iter()
            .flat_map(|line| line.iter().chain(iter::once(&'\n')))
            .copied()
    }

    /// Strip margin whitespace from the buffer.
    fn strip_margin_whitespace(&mut self) {
        let is_only_ws = |v: &[char]| v.iter().all(|c| c.is_whitespace());

        // upper margin
        for _ in 0..self
            .chars
            .iter()
            .take_while(|line| is_only_ws(line))
            .count()
        {
            self.chars.remove(0);
        }

        // lower margin
        for _ in 0..self
            .chars
            .iter()
            .rev()
            .take_while(|line| is_only_ws(line))
            .count()
        {
            self.chars.pop();
        }

        // left margin
        if let Some(min_ws) = self
            .chars
            .iter()
            .filter(|line| !is_only_ws(line))
            .map(|line| line.iter().position(|c| !c.is_whitespace()))
            .min()
            .flatten()
        {
            for line in self.chars.iter_mut() {
                if line.is_empty() {
                    continue;
                }
                let idx = min(line.len() - 1, min_ws);
                let new = line.split_off(idx);
                mem::replace(line, new);
            }
        }

        // right margin
        self.strip_trailing_whitespace();
    }

    /// Strip trailing whitespace from the buffer.
    fn strip_trailing_whitespace(&mut self) {
        for line in self.chars.iter_mut() {
            let idx = line
                .iter()
                .enumerate()
                .rfind(|p| !p.1.is_whitespace())
                .map(|p| p.0 + 1)
                .unwrap_or(0);

            line.truncate(idx);
        }
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
        if force {
            self.edits.push(Cell { pos, c });
            return;
        }

        let max_prec = precedence(c);
        let overrides = |_c| _c == c || precedence(_c) > max_prec;

        let mut overridden = false;
        if self.chars.len() > pos.y && self.chars[pos.y].len() > pos.x {
            overridden |= overrides(self.chars[pos.y][pos.x]);
        }

        overridden |= self
            .edits
            .iter()
            .filter(|cell| cell.pos == pos)
            .any(|cell| overrides(cell.c));

        if !overridden {
            self.edits.push(Cell { pos, c });
        }
    }

    /// Set the cell at `(x, y)` to `c`.
    pub fn set(&mut self, force: bool, x: usize, y: usize, c: char) {
        self.setv(force, Vec2::new(x, y), c)
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
        let dec = |v: usize| v - 1;
        let inc = |v: usize| v + 1;

        let n = target.y > 0 && self.visible(target.map_y(dec));
        let e = self.visible(target.map_x(inc));
        let s = self.visible(target.map_y(inc));
        let w = target.x > 0 && self.visible(target.map_x(dec));

        let tip = match line_slope(origin, target).pair() {
            S_N if n || (w && e) => N,
            S_N if w => W,
            S_N if e => E,
            S_N => N,

            S_E if e || (n && s) => E,
            S_E if n => N,
            S_E if s => S,
            S_E => E,

            S_S if s || (e && w) => S,
            S_S if e => E,
            S_S if w => W,
            S_S => S,

            S_W if w || (s && n) => W,
            S_W if s => S,
            S_W if n => N,
            S_W => W,

            // SE
            (x, y) if x > 0 && y > 0 && self.visible(target.map_x(inc)) => E,
            (x, y) if x > 0 && y > 0 => S,

            // NE
            (x, y) if x > 0 && y < 0 && self.visible(target.map_x(inc)) => E,
            (x, y) if x > 0 && y < 0 => N,

            // SW
            (x, y) if x < 0 && y > 0 && target.x == 0 => S,
            (x, y) if x < 0 && y > 0 && self.visible(target.map_x(dec)) => W,
            (x, y) if x < 0 && y > 0 => S,

            // NW
            (x, y) if x < 0 && y < 0 && target.x == 0 => N,
            (x, y) if x < 0 && y < 0 && self.visible(target.map_x(dec)) => W,
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
