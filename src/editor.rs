// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::{tools::*, Options};
use clipboard::{ClipboardContext, ClipboardProvider};
use core::ops::Add;
use cursive::{
    event::{Event, EventResult, MouseButton::*, MouseEvent::*},
    theme::ColorStyle,
    view::{scroll::Scroller, View},
    views::ScrollView,
    Printer, Rect, Vec2, XY,
};
use lazy_static::lazy_static;
use line_drawing::Bresenham;
use num_traits::Zero;
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use pathfinding::directed::astar::astar;
use std::{
    cmp::{max, min},
    error::Error,
    f64::consts::SQRT_2,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, ErrorKind, Read, Write},
    iter, mem,
    path::{Path, PathBuf},
    rc::Rc,
};

pub(crate) const CONSUMED: Option<EventResult> = Some(EventResult::Consumed(None));

macro_rules! intercept_scrollbar {
    ($ctx:expr, $event:expr) => {{
        lazy_static! {
            static ref LAST_LPRESS: Mutex<Option<Vec2>> = Mutex::new(None);
        }

        if let Event::Mouse {
            offset,
            position: pos,
            event,
        } = $event
        {
            match event {
                Press(Left) if $ctx.on_scrollbar(*offset, *pos) => {
                    *LAST_LPRESS.lock() = Some(*pos);
                    return None;
                }

                Press(Left) => {
                    *LAST_LPRESS.lock() = Some(*pos);
                }

                Hold(Left)
                    if LAST_LPRESS
                        .lock()
                        .map(|pos| $ctx.on_scrollbar(*offset, pos))
                        .unwrap_or(false) =>
                {
                    return None;
                }

                Release(Left)
                    if LAST_LPRESS
                        .lock()
                        .take()
                        .map(|pos| $ctx.on_scrollbar(*offset, pos))
                        .unwrap_or(false) =>
                {
                    return None;
                }

                _ => {}
            }
        }
    }};
}

macro_rules! intercept_pan {
    ($ctx:expr, $event:expr) => {{
        lazy_static! {
            static ref OLD: Mutex<Option<Vec2>> = Mutex::new(None);
        }

        if let Event::Mouse {
            position: pos,
            event,
            ..
        } = $event
        {
            match event {
                Press(Right) => {
                    *OLD.lock() = Some(*pos);
                    return CONSUMED;
                }

                Hold(Right) if OLD.lock().is_none() => {
                    *OLD.lock() = Some(*pos);
                    return CONSUMED;
                }

                Hold(Right) => {
                    let old = OLD.lock().replace(*pos).unwrap();

                    let offset = ($ctx.0.content_viewport())
                        .top_left()
                        .map_x(|x| drag(x, pos.x, old.x))
                        .map_y(|y| drag(y, pos.y, old.y));

                    $ctx.0.set_offset(offset);

                    let p = $ctx.0.content_viewport();
                    let i = $ctx.0.inner_size();

                    let mut editor = $ctx.0.get_inner_mut().write();
                    if pos.x < old.x && within((old.x - pos.x + 1) * 4, p.right(), i.x) {
                        editor.canvas.x += old.x - pos.x;
                    }
                    if pos.y < old.y && within((old.y - pos.y + 1) * 2, p.bottom(), i.y) {
                        editor.canvas.y += old.y - pos.y;
                    }

                    return CONSUMED;
                }

                Release(Right) => {
                    *OLD.lock() = None;
                    return CONSUMED;
                }

                _ => {}
            }
        }
    }};
}

fn drag(x: usize, new: usize, old: usize) -> usize {
    if new > old {
        x.saturating_sub(new - old)
    } else {
        x + (old - new)
    }
}

pub(crate) struct EditorCtx<'a>(&'a mut ScrollView<EditorView>);

impl<'a> EditorCtx<'a> {
    /// Returns a new `EditorCtx`.
    pub(super) fn new(view: &'a mut ScrollView<EditorView>) -> Self {
        Self(view)
    }

    /// Handles an event using the active tool.
    pub(super) fn on_event(&mut self, event: &Event) -> Option<EventResult> {
        intercept_scrollbar!(self, event);
        intercept_pan!(self, event);

        let mut tool = self.0.get_inner_mut().write().active_tool.take().unwrap();
        let res = tool.on_event(self, event);
        self.0.get_inner_mut().write().active_tool = Some(tool);

        res
    }

    /// Returns `true` if `pos` is located on a scrollbar.
    fn on_scrollbar(&self, offset: Vec2, pos: Vec2) -> bool {
        let core = self.0.get_scroller();
        let max = core.last_available_size() + offset;
        let min = max - core.scrollbar_size();

        (min.x..=max.x).contains(&pos.x) || (min.y..=max.y).contains(&pos.y)
    }

    /// If `event` is a mouse event, relativize its position to the canvas plane.
    pub(crate) fn relativize(&self, event: &Event) -> Event {
        let mut event = event.clone();
        if let Event::Mouse {
            offset, position, ..
        } = &mut event
        {
            let tl = self.0.content_viewport().top_left();
            *position = position.saturating_sub(*offset) + tl;
        }
        event
    }

    /// Scroll to `pos`, moving at least `step_x` & `step_y` respectively if the x or y
    /// scroll offset needs to be modified.
    pub(crate) fn scroll_to(&mut self, pos: Vec2, step_x: usize, step_y: usize) {
        let port = self.0.content_viewport();
        let mut offset = port.top_left();

        if pos.x >= port.right() {
            offset.x += max(step_x, pos.x - port.right());
        } else if pos.x <= port.left() {
            offset.x -= max(min(step_x, offset.x), port.left() - pos.x);
        }
        if pos.y >= port.bottom() {
            offset.y += max(step_y, pos.y - port.bottom());
        } else if pos.y <= port.top() {
            offset.y -= max(min(step_y, offset.y), port.top() - pos.y);
        }
        self.0.set_offset(offset);

        // BUG: scrolling lags behind changes to the canvas bounds by 1 render tick. in
        // order to truly fix the issue, we need to implement scrolling as a function of
        // the editor itself.
        let mut editor = self.0.get_inner_mut().write();

        if pos.x + 1 >= editor.canvas.x {
            editor.canvas.x += max(step_x, (pos.x + 1) - editor.canvas.x);
        }
        if pos.y + 1 >= editor.canvas.y {
            editor.canvas.y += max(step_y, (pos.y + 1) - editor.canvas.y);
        }
    }

    /// Scroll to the edit buffer's current cursor, if one exists.
    pub(crate) fn scroll_to_cursor(&mut self) {
        let pos = self.0.get_inner_mut().read().buffer.cursor;

        if let Some(pos) = pos {
            self.scroll_to(pos, 1, 1);
        }
    }

    /// Modify the edit buffer using `render`, flushing any changes and saving a snapshot
    /// of the buffer's prior state in the editor's undo history.
    pub(crate) fn clobber<R: FnOnce(&mut Buffer)>(&mut self, render: R) {
        let mut editor = self.0.get_inner_mut().write();

        editor.with_snapshot(|ed| {
            render(&mut ed.buffer);
            ed.buffer.flush_edits();
            ed.buffer.drop_cursor();
        });
    }

    /// Modify the edit buffer using `render`, without flushing any changes.
    pub(crate) fn preview<R: FnOnce(&mut Buffer)>(&mut self, render: R) {
        let mut editor = self.0.get_inner_mut().write();
        editor.buffer.discard_edits();
        render(&mut editor.buffer);
    }
}

#[derive(Clone)]
pub(crate) struct EditorView {
    inner: Rc<RwLock<Editor>>,
}

impl View for EditorView {
    fn draw(&self, p: &Printer<'_, '_>) {
        let mut normal = print_styled(ColorStyle::primary());
        let mut change = print_styled(ColorStyle::highlight_inactive());
        let mut cursor = print_styled(ColorStyle::highlight());

        for c in self.read().buffer.iter_within(p.content_offset, p.size) {
            match c {
                Char::Clean(Cell { pos, c }) => normal(p, pos, c),
                Char::Dirty(Cell { pos, c }) => change(p, pos, c),
                Char::Cursor(Cell { pos, c }) => cursor(p, pos, c),
            }
        }
    }

    fn required_size(&mut self, size: Vec2) -> Vec2 {
        let mut editor = self.write();

        let buf_bounds = editor.buffer.bounds();

        editor.canvas = Vec2 {
            x: max(buf_bounds.x, editor.canvas.x),
            y: max(buf_bounds.y, editor.canvas.y),
        };

        Vec2 {
            x: max(size.x, editor.canvas.x),
            y: max(size.y, editor.canvas.y),
        }
    }
}

impl EditorView {
    pub(crate) fn new(inner: Editor) -> Self {
        Self {
            inner: Rc::new(RwLock::new(inner)),
        }
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<Editor> {
        self.inner.read()
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<Editor> {
        self.inner.write()
    }
}

pub(crate) struct Editor {
    opts: Options,
    buffer: Buffer,
    lsave: Buffer,
    dirty: bool,
    undo_history: Vec<Buffer>,
    redo_history: Vec<Buffer>,
    active_tool: Option<Box<dyn Tool>>,
    canvas: Vec2,
    rendered: String,
}

fn print_styled(style: ColorStyle) -> impl FnMut(&Printer<'_, '_>, Vec2, char) {
    let mut buf = vec![0; 4];
    move |p, pos, c| {
        p.with_color(style, |p| p.print(pos, c.encode_utf8(&mut buf)));
    }
}

impl Editor {
    /// Open an editor instance with the provided options.
    pub(crate) fn open(mut opts: Options) -> io::Result<Self> {
        let file = opts.file.take();

        let mut tool = BoxTool::default();
        tool.load_opts(&opts);

        let mut editor = Self {
            opts,
            buffer: Buffer::default(),
            lsave: Buffer::default(),
            dirty: false,
            undo_history: vec![],
            redo_history: vec![],
            active_tool: Some(Box::new(tool)),
            canvas: Vec2::new(0, 0),
            rendered: String::default(),
        };

        if let Some(path) = file {
            editor.open_file(path)?;
        }

        Ok(editor)
    }

    /// Mutate the loaded options with `apply`.
    pub(crate) fn mut_opts<F: FnOnce(&mut Options)>(&mut self, apply: F) {
        apply(&mut self.opts);
        if let Some(tool) = self.active_tool.as_mut() {
            tool.load_opts(&self.opts);
        }
    }

    /// Returns `true` if the buffer has been modified since the last save.
    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Set the active tool.
    pub(crate) fn set_tool<T: Tool + 'static>(&mut self, mut tool: T) {
        self.buffer.discard_edits();
        self.buffer.drop_cursor();
        tool.load_opts(&self.opts);
        self.active_tool = Some(Box::new(tool));
    }

    /// Returns the active tool as a human readable string.
    pub(crate) fn active_tool(&self) -> String {
        format!("({})", self.active_tool.as_ref().unwrap())
    }

    /// Returns the current save path.
    pub(crate) fn path(&self) -> Option<&PathBuf> {
        self.opts.file.as_ref()
    }

    /// Clear all buffer state and begin a blank diagram.
    pub(crate) fn clear(&mut self) {
        self.opts.file = None;
        self.buffer.clear();
        self.lsave.clear();
        self.dirty = false;
        self.undo_history.clear();
        self.redo_history.clear();
        self.canvas = Vec2::new(0, 0);
    }

    /// Open the file at `path`, discarding any unsaved changes to the current file, if
    /// there are any.
    ///
    /// No modifications have been performed if this returns `Err(_)`.
    pub(crate) fn open_file<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        let buffer = OpenOptions::new()
            .read(true)
            .open(path.as_ref())
            .and_then(Buffer::read_from);

        let buffer = match buffer {
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            r => Some(r?),
        };

        self.clear();
        self.opts.file = Some(path.as_ref().into());
        if let Some(buf) = buffer {
            self.lsave = buf.clone();
            self.buffer = buf;
        }

        Ok(())
    }

    /// Save the current buffer contents to disk.
    ///
    /// Returns `Ok(true)` if the buffer was saved, and `Ok(false)` if there is no path
    /// configured for saving.
    ///
    /// If the configured save path does not exist, this will recursively create it.
    pub(crate) fn save(&mut self) -> io::Result<bool> {
        if let Some(path) = self.path() {
            path.parent().map(fs::create_dir_all).transpose()?;

            let file = OpenOptions::new()
                .read(false)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;

            self.render_to_file(file)?;
            self.lsave = self.buffer.clone();
            self.dirty = false;
        }

        Ok(self.path().is_some())
    }

    /// Save the current buffer contents to the file at `path`, and setting that as the
    /// new path for future calls to `save`.
    pub(crate) fn save_as<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        self.opts.file = Some(path.as_ref().into());
        self.save()?;

        Ok(())
    }

    /// Render to `file`, performing whitespace cleanup if enabled.
    fn render_to_file(&mut self, mut file: File) -> io::Result<()> {
        self.canvas = Vec2::new(0, 0);

        self.with_snapshot(|ed| {
            if ed.opts.strip_margin_ws {
                ed.buffer.strip_margin_whitespace();
            } else if !ed.opts.keep_trailing_ws {
                ed.buffer.strip_trailing_whitespace();
            }
        });

        self.rendered.clear();
        self.rendered.extend(self.buffer.iter(""));

        file.write_all(self.rendered.as_bytes())?;
        file.flush()?;
        file.sync_all()?;

        Ok(())
    }

    /// Render to the clipboard, prefixing all lines with `prefix`.
    ///
    /// Trims all margins in the output without changing the buffer's state.
    pub(crate) fn render_to_clipboard(&self, prefix: &str) -> Result<(), Box<dyn Error>> {
        let mut ctx = ClipboardContext::new()?;

        let mut buf = self.buffer.clone();
        buf.strip_margin_whitespace();

        let mut rendered: String = buf.iter(prefix).collect();
        if let Some(c) = rendered.pop() {
            if c != '\n' {
                rendered.push(c);
            }
        }

        ctx.set_contents(rendered)
    }

    /// Trim all whitespace from margins.
    pub(crate) fn trim_margins(&mut self) {
        self.with_snapshot(|ed| {
            ed.canvas = Vec2::new(0, 0);
            ed.buffer.strip_margin_whitespace();
        });
    }

    /// Take a snapshot of the buffer, discard any pending edits, and run `apply`. If
    /// the buffer was modified, mark it as dirty. Otherwise, remove the snapshot.
    ///
    /// Use this function to execute any buffer modification that should be saved in the
    /// undo history.
    fn with_snapshot<F: FnOnce(&mut Self)>(&mut self, apply: F) {
        let snapshot = self.buffer.snapshot();
        self.undo_history.push(snapshot);
        self.buffer.discard_edits();

        apply(self);

        if self.undo_history.last().unwrap() == &self.buffer {
            self.undo_history.pop();
        } else {
            self.dirty = true;
        }
    }

    /// Undo the last buffer modification.
    ///
    /// Returns `false` if there was nothing to undo.
    pub(crate) fn undo(&mut self) -> bool {
        let undone = self
            .undo_history
            .pop()
            .map(|buffer| mem::replace(&mut self.buffer, buffer))
            .map(|buffer| self.redo_history.push(buffer))
            .is_some();

        if undone {
            self.dirty = self.buffer != self.lsave;
        }

        undone
    }

    /// Redo the last undone buffer modification.
    ///
    /// Returns `false` if there was nothing to redo.
    pub(crate) fn redo(&mut self) -> bool {
        let redone = self
            .redo_history
            .pop()
            .map(|buffer| mem::replace(&mut self.buffer, buffer))
            .map(|buffer| self.undo_history.push(buffer))
            .is_some();

        if redone {
            self.dirty = self.buffer != self.lsave;
        }

        redone
    }
}

pub(crate) const SP: char = ' ';
pub(crate) const DASH: char = '-';
pub(crate) const PIPE: char = '|';
pub(crate) const DIAG: char = '/';
pub(crate) const GAID: char = '\\';
pub(crate) const PLUS: char = '+';
pub(crate) const CURS: char = '_';

const N: char = '^';
const S: char = 'v';
const W: char = '<';
const E: char = '>';

const S_N: (isize, isize) = (0, -1);
const S_E: (isize, isize) = (1, 0);
const S_S: (isize, isize) = (0, 1);
const S_W: (isize, isize) = (-1, 0);

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct Buffer {
    chars: Vec<Vec<char>>,
    edits: Vec<Cell>,
    cursor: Option<Vec2>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct Cell {
    pos: Vec2,
    c: char,
}

impl Cell {
    pub(crate) fn pos(&self) -> Vec2 {
        self.pos
    }

    pub(crate) fn c(&self) -> char {
        self.c
    }

    pub(crate) fn is_whitespace(&self) -> bool {
        self.c.is_whitespace()
    }

    pub(crate) fn translate(mut self, by: XY<isize>) -> Self {
        self.pos = self.pos.saturating_add(by);
        self
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Char {
    Clean(Cell),
    Dirty(Cell),
    Cursor(Cell),
}

impl Buffer {
    fn read_from<R: Read>(r: R) -> io::Result<Self> {
        Ok(Self {
            chars: BufReader::new(r)
                .lines()
                .map(|lr| lr.map(|s| s.chars().collect()))
                .collect::<io::Result<_>>()?,
            edits: vec![],
            cursor: None,
        })
    }

    /// Returns a copy of this buffer without any pending edits.
    fn snapshot(&self) -> Self {
        Self {
            chars: self.chars.clone(),
            edits: vec![],
            cursor: None,
        }
    }

    /// Set the cursor position to `pos`.
    pub(crate) fn set_cursor(&mut self, pos: Vec2) {
        self.cursor = Some(pos);
    }

    /// Disable the cursor.
    fn drop_cursor(&mut self) {
        self.cursor = None;
    }

    /// Clears all content in the buffer.
    fn clear(&mut self) {
        self.chars.clear();
        self.edits.clear();
        self.cursor = None;
    }

    /// Returns the viewport size required to display all content within the buffer.
    fn bounds(&self) -> Vec2 {
        let mut bounds = Vec2 {
            x: self.chars.iter().map(Vec::len).max().unwrap_or(0),
            y: self.chars.len(),
        };

        bounds.x = max(
            bounds.x,
            self.edits
                .iter()
                .map(|Cell { pos, .. }| pos.x + 1)
                .max()
                .unwrap_or(0),
        );

        bounds.y = max(
            bounds.y,
            self.edits
                .iter()
                .map(|Cell { pos, .. }| pos.y + 1)
                .max()
                .unwrap_or(0),
        );

        if let Some(Vec2 { x, y }) = self.cursor {
            bounds.x = max(bounds.x, x + 1);
            bounds.y = max(bounds.y, y + 1);
        }

        bounds
    }

    /// Returns an iterator over all characters within the viewport formed by `offset`
    /// and `size`.
    pub(crate) fn iter_within<'a>(
        &'a self,
        offset: Vec2,
        size: Vec2,
    ) -> impl Iterator<Item = Char> + 'a {
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
            .chain(
                self.cursor
                    .map(|pos| Cell { pos, c: CURS })
                    .map(Char::Cursor),
            )
    }

    /// Returns an iterator over all characters in the buffer, injecting newlines
    /// where appropriate, with `prefix` before each line.
    fn iter<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = char> + 'a {
        self.chars.iter().flat_map(move |line| {
            prefix
                .chars()
                .chain(line.iter().copied())
                .chain(iter::once('\n'))
        })
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
                *line = new;
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
    pub(crate) fn getv(&self, pos: Vec2) -> Option<char> {
        self.chars.get(pos.y).and_then(|v| v.get(pos.x)).copied()
    }

    /// Returns `true` iff the cell at `pos` exists and contains a non-whitespace
    /// character.
    ///
    /// Does not consider any pending edits.
    pub(crate) fn visible(&self, pos: Vec2) -> bool {
        self.getv(pos).map(|c| !c.is_whitespace()).unwrap_or(false)
    }

    /// Set the cell at `pos` to `c`.
    pub(crate) fn setv(&mut self, force: bool, pos: Vec2, c: char) {
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
    pub(crate) fn set(&mut self, force: bool, x: usize, y: usize, c: char) {
        self.setv(force, Vec2::new(x, y), c)
    }

    /// Flush any pending edits to the primary buffer, allocating as necessary.
    fn flush_edits(&mut self) {
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

    /// Draw a line from `src` to `dst`.
    pub(crate) fn draw_line(&mut self, src: Vec2, dst: Vec2) {
        for (i, (s, e)) in Bresenham::new(src.signed().pair(), dst.signed().pair())
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

        self.setv(false, dst, PLUS);
    }

    /// Draw an arrow tip for an arrow from `src` to `dst`.
    pub(crate) fn draw_arrow_tip(&mut self, src: Vec2, dst: Vec2) {
        let dec = |v: usize| v - 1;
        let inc = |v: usize| v + 1;

        let north = dst.y > 0 && self.visible(dst.map_y(dec));
        let east = self.visible(dst.map_x(inc));
        let south = self.visible(dst.map_y(inc));
        let west = dst.x > 0 && self.visible(dst.map_x(dec));

        let tip = match line_slope(src, dst).pair() {
            S_N if north || (west && east) => N,
            S_N if west => W,
            S_N if east => E,
            S_N => N,

            S_E if east || (north && south) => E,
            S_E if north => N,
            S_E if south => S,
            S_E => E,

            S_S if south || (east && west) => S,
            S_S if east => E,
            S_S if west => W,
            S_S => S,

            S_W if west || (south && north) => W,
            S_W if south => S,
            S_W if north => N,
            S_W => W,

            // SE
            (x, y) if x > 0 && y > 0 && self.visible(dst.map_x(inc)) => E,
            (x, y) if x > 0 && y > 0 => S,

            // NE
            (x, y) if x > 0 && y < 0 && self.visible(dst.map_x(inc)) => E,
            (x, y) if x > 0 && y < 0 => N,

            // SW
            (x, y) if x < 0 && y > 0 && dst.x == 0 => S,
            (x, y) if x < 0 && y > 0 && self.visible(dst.map_x(dec)) => W,
            (x, y) if x < 0 && y > 0 => S,

            // NW
            (x, y) if x < 0 && y < 0 && dst.x == 0 => N,
            (x, y) if x < 0 && y < 0 && self.visible(dst.map_x(dec)) => W,
            (x, y) if x < 0 && y < 0 => N,

            (_, _) => PLUS,
        };

        self.setv(true, dst, tip);
    }

    pub(crate) fn snap45(&self, src: Vec2, dst: Vec2) -> Vec2 {
        let delta = min(diff(src.y, dst.y), diff(src.x, dst.x));

        match line_slope(src, dst).pair() {
            // nw
            (x, y) if x < 0 && y < 0 => dst.map(|v| v + delta),
            // ne
            (x, y) if x > 0 && y < 0 => dst.map_x(|x| x - delta).map_y(|y| y + delta),
            // sw
            (x, y) if x < 0 && y > 0 => dst.map_x(|x| x + delta).map_y(|y| y - delta),
            // se
            (x, y) if x > 0 && y > 0 => dst.map(|v| v - delta),

            _ => src,
        }
    }

    pub(crate) fn snap90(&self, src: Vec2, dst: Vec2) -> Vec2 {
        if let Some(DASH) = self.getv(dst) {
            Vec2::new(dst.x, src.y)
        } else {
            Vec2::new(src.x, dst.y)
        }
    }

    /// Draw the shortest path from `src` to `dst`. Returns the penultimate point
    /// along that path.
    pub(crate) fn draw_path(&mut self, src: Vec2, dst: Vec2) -> Vec2 {
        let mut path = astar(
            &src.pair(),
            |&pos| self.neighbors(pos),
            |&pos| heuristic(pos.into(), dst),
            |&pos| pos == dst.pair(),
        )
        .map(|(points, _)| points)
        .unwrap()
        .into_iter()
        .map(Vec2::from)
        .enumerate()
        .peekable();

        let decide = |i: usize, last: Vec2, pos: Vec2| -> char {
            match line_slope(last, pos).pair() {
                _ if i == 0 => PLUS,
                (0, _) => PIPE,
                (_, 0) => DASH,
                (x, y) if (x > 0) == (y > 0) => GAID,
                _ => DIAG,
            }
        };

        let mut last = src;
        while let Some((i, pos)) = path.next() {
            let mut c = decide(i, last, pos);

            if let Some(next) = path.peek().map(|(i, next)| decide(*i, pos, *next)) {
                if c != PLUS && next != c {
                    c = PLUS;
                }
                last = pos;
            }

            self.setv(false, pos, c);
        }
        self.setv(false, dst, PLUS);

        last
    }

    /// Returns the coordinates neighboring `pos`, along with the cost to reach each one.
    fn neighbors(&self, pos: (usize, usize)) -> Vec<((usize, usize), OrdFloat)> {
        let vis = |pos: (usize, usize)| self.visible(pos.into());

        let card = |pos| (pos, OrdFloat((vis(pos) as u8 as f64 * 64.0) + D));

        let diag = |pos, (c1, c2)| {
            let cost = {
                let spot = vis(pos) as u8 as f64 * 64.0;
                let edge = (vis(c1) && vis(c2)) as u8 as f64 * 64.0;
                spot + edge
            };

            (pos, OrdFloat(cost + D2))
        };

        let w = |(x, y)| (x - 1, y);
        let n = |(x, y)| (x, y - 1);
        let e = |(x, y)| (x + 1, y);
        let s = |(x, y)| (x, y + 1);

        let mut succ = Vec::with_capacity(8);
        if pos.0 > 0 && pos.1 > 0 {
            succ.push(diag(n(w(pos)), (n(pos), w(pos))));
        }
        if pos.0 > 0 {
            succ.push(card(w(pos)));
            succ.push(diag(s(w(pos)), (s(pos), w(pos))));
        }
        if pos.1 > 0 {
            succ.push(card(n(pos)));
            succ.push(diag(n(e(pos)), (n(pos), e(pos))));
        }
        succ.push(card(e(pos)));
        succ.push(card(s(pos)));
        succ.push(diag(s(e(pos)), (s(pos), e(pos))));

        succ
    }
}

/// Cost to move one step on the cardinal plane.
const D: f64 = 1.0;

/// Cost to move one step on the diagonal plane.
const D2: f64 = SQRT_2;

/// Returns a distance heuristic between `pos` and `dst`.
fn heuristic(pos: Vec2, dst: Vec2) -> OrdFloat {
    // base is diagonal distance:
    // http://theory.stanford.edu/~amitp/GameProgramming/Heuristics.html#diagonal-distance
    let dx = (pos.x as f64 - dst.x as f64).abs();
    let dy = (pos.y as f64 - dst.y as f64).abs();

    let dist = if dx > dy {
        D * (dx - dy) + D2 * dy
    } else {
        D * (dy - dx) + D2 * dx
    };

    // prefer to expand paths close to dst:
    // http://theory.stanford.edu/~amitp/GameProgramming/Heuristics.html#breaking-ties
    const P: f64 = 1.0 + (1.0 / 1000.0);

    OrdFloat(dist * P)
}

#[derive(PartialEq, PartialOrd, Copy, Clone)]
struct OrdFloat(f64);

impl Eq for OrdFloat {}

impl Ord for OrdFloat {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Add<Self> for OrdFloat {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Zero for OrdFloat {
    #[inline]
    fn zero() -> Self {
        Self(0.0)
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.0 == 0.0
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

/// Returns `true` if `a` is within `w` of `b` (inclusive).
fn within(w: usize, a: usize, b: usize) -> bool {
    diff(a, b) <= w
}

/// Returns the absolute difference between `a` and `b`.
fn diff(a: usize, b: usize) -> usize {
    (a as isize - b as isize).abs() as usize
}

/// Returns the slope between points at `src` and `dst`.
///
/// The resulting fraction will be reduced to its simplest terms.
fn line_slope<P: Into<XY<isize>>>(src: P, dst: P) -> XY<isize> {
    let xy = dst.into() - src;

    match gcd(xy.x, xy.y) {
        0 => xy,
        d => xy / d,
    }
}

/// Returns the greatest common denominator between `a` and `b`.
fn gcd(mut a: isize, mut b: isize) -> isize {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs()
}
