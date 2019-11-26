// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
#![allow(dead_code, unused_variables, unused_imports)]

extern crate cursive;
extern crate lazy_static;
extern crate line_drawing;
extern crate log;
extern crate parking_lot;
extern crate structopt;

mod tools;

use tools::*;

use cursive::{
    direction::Orientation,
    event,
    event::{Callback, Event, EventResult, EventTrigger, Key, MouseButton, MouseEvent},
    logger,
    menu::MenuTree,
    theme::Effect,
    view::scroll::Scroller,
    views::{
        Canvas, Checkbox, Dialog, EditView, IdView, LinearLayout, OnEventView, Panel, ScrollView,
        TextArea, TextContent, TextView, ViewRef,
    },
    Cursive, Rect, Vec2, With,
};
use lazy_static::lazy_static;
use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;
use std::{
    error::Error,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    iter,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

type MainResult<T> = Result<T, Box<dyn Error>>;

fn main() -> MainResult<()> {
    let opts = Options::from_args();
    info!("{:?}", opts);

    let editor = Editor::open(opts)?;
    logger::init();

    let mut siv = Cursive::default();

    siv.menubar()
        .add_subtree(
            "File",
            MenuTree::new()
                .leaf("New", |s| s.add_layer(Dialog::info("Clicked New")))
                .leaf("Open", |_| ())
                .leaf("Save", |_| ())
                .leaf("Save As", |_| ())
                .delimiter()
                .leaf("Debug", Cursive::toggle_debug_console)
                .leaf("Quit", Cursive::quit),
        )
        .add_delimiter()
        .add_leaf("Box", set_tool::<BoxTool, _>(|_| ()))
        .add_subtree(
            "Line",
            MenuTree::new()
                .leaf("Snap 45", set_tool::<LineTool, _>(|o| o.line_snap45 = true))
                .leaf(
                    "Snap 90",
                    set_tool::<LineTool, _>(|o| o.line_snap45 = false),
                ),
        )
        .add_subtree(
            "Arrow",
            MenuTree::new()
                .leaf(
                    "Snap 45",
                    set_tool::<ArrowTool, _>(|o| o.line_snap45 = true),
                )
                .leaf(
                    "Snap 90",
                    set_tool::<ArrowTool, _>(|o| o.line_snap45 = false),
                ),
        )
        .add_delimiter()
        .add_leaf(format!("{{ {} }}", editor.tool), |_| ());

    siv.set_autohide_menu(false);
    siv.add_global_callback('`', Cursive::toggle_debug_console);
    siv.add_global_callback('q', Cursive::quit);
    siv.add_global_callback(Key::Esc, |s| s.select_menubar());

    let editor_layer = Panel::new(IdView::new(
        "editor",
        OnEventView::new(
            ScrollView::new(TextView::new_with_content(editor.content()).no_wrap())
                .scroll_x(true)
                .scroll_y(true),
        )
        .on_pre_event_inner(EventTrigger::mouse(), editor_callback),
    ));
    siv.add_fullscreen_layer(editor_layer);
    siv.set_user_data(editor);

    siv.run();

    Ok(())
}

fn set_tool<T, S>(set: S) -> impl Fn(&mut Cursive) + 'static
where
    T: Tool + Default + 'static,
    S: Fn(&mut Options) + 'static,
{
    const ACTIVE_TOOL: usize = 6;

    move |siv| {
        let editor = get_editor(siv);
        set(&mut editor.opts);
        editor.tool = Box::new(T::default());
        editor.tool.load_opts(&editor.opts);
        let tool = format!("{{ {} }}", editor.tool);
        drop(editor);

        let menu = siv.menubar();
        menu.remove(ACTIVE_TOOL);
        menu.insert_leaf(ACTIVE_TOOL, tool, |_| ());
    }
}

fn get_editor(siv: &mut Cursive) -> &mut Editor {
    siv.user_data::<Editor>().unwrap()
}

fn get_editor_view(siv: &mut Cursive) -> ViewRef<OnEventView<ScrollView<TextView>>> {
    siv.find_id::<OnEventView<ScrollView<TextView>>>("editor")
        .unwrap()
}

fn on_scrollbar<S: Scroller>(scroll: &S, offset: Vec2, pos: Vec2) -> bool {
    let core = scroll.get_scroller();
    let max = core.last_size() + offset;
    let min = max - core.scrollbar_size();

    (min.x..=max.x).contains(&pos.x) || (min.y..=max.y).contains(&pos.y)
}

fn consume_event<F: Fn(&mut Cursive) + 'static>(f: F) -> Option<EventResult> {
    Some(EventResult::Consumed(Some(Callback::from_fn(f))))
}

lazy_static! {
    static ref LAST_LPRESS: Mutex<Option<Vec2>> = Mutex::new(None);
    static ref RPOINTER: Mutex<Option<Vec2>> = Mutex::new(None);
}

fn editor_callback(scroll_view: &mut ScrollView<TextView>, event: &Event) -> Option<EventResult> {
    let (offset, pos, event) = match *event {
        Event::Mouse {
            offset,
            position,
            event,
        } => (offset, position, event),

        _ => return None,
    };

    use MouseButton::*;
    use MouseEvent::*;

    match event {
        Press(Left) if on_scrollbar(scroll_view, offset, pos) => {
            *LAST_LPRESS.lock() = Some(pos);
            return None;
        }

        Press(Left) => {
            *LAST_LPRESS.lock() = Some(pos);
        }

        Hold(Left)
            if LAST_LPRESS
                .lock()
                .map(|pos| on_scrollbar(scroll_view, offset, pos))
                .unwrap_or(false) =>
        {
            return None;
        }

        Release(Left)
            if LAST_LPRESS
                .lock()
                .take()
                .map(|pos| on_scrollbar(scroll_view, offset, pos))
                .unwrap_or(false) =>
        {
            return None;
        }

        WheelUp | WheelDown => return None,

        _ => {}
    }

    let viewport = scroll_view.content_viewport();

    let content_pos = pos.saturating_sub(offset) + viewport.top_left();

    consume_event(move |siv| match event {
        Press(Right) => {
            *RPOINTER.lock() = Some(pos);
        }

        Hold(Right) if RPOINTER.lock().is_none() => {
            *RPOINTER.lock() = Some(pos);
        }

        Hold(Right) => {
            let Vec2 { x, y } = RPOINTER.lock().replace(pos).unwrap();

            let mut view = get_editor_view(siv);
            let scroll_view = view.get_inner_mut();

            let mut offset = scroll_view.content_viewport().top_left();

            if pos.x > x {
                offset.x = offset.x.saturating_sub(pos.x - x);
            } else if pos.x < x {
                offset.x = offset.x.saturating_add(x - pos.x);
            }

            if pos.y > y {
                offset.y = offset.y.saturating_sub(pos.y - y);
            } else if pos.y < y {
                offset.y = offset.y.saturating_add(y - pos.y);
            }

            scroll_view.set_offset(offset);
        }

        Release(Right) => {
            *RPOINTER.lock() = None;
        }

        Press(Left) => {
            let editor = get_editor(siv);
            let save = editor.tool.on_press(content_pos);
            editor.render(save);
        }

        Hold(Left) => {
            let editor = get_editor(siv);
            let save = editor.tool.on_hold(content_pos);
            editor.render(save);
        }

        Release(Left) => {
            let editor = get_editor(siv);
            let save = editor.tool.on_release(content_pos);
            editor.render(save);
            editor.tool.reset();
        }

        _ => {}
    })
}

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
// TODO: make Editor implement View for efficiency and easier styling
// TODO: highlight modifications while they're happening
// TODO: implement arrow tool
// TODO: implement text tool
// TODO: implement resize tool
// TODO: implement select tool
// TODO: implement unicode mode
// TODO: implement clickable buttons for each tool
// TODO: implement separate tool-settings bar
// TODO: experimental: implement routed lines, with as many bends as necessary to avoid
//       obstacles
struct Editor {
    opts: Options,
    file: File,
    buffer: Vec<Vec<char>>,
    render: TextContent,
    tool: Box<dyn Tool>,
}

impl Editor {
    fn open(opts: Options) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&opts.file)?;

        let mut editor = Self {
            opts,
            file,
            buffer: vec![],
            render: TextContent::new(""),
            tool: Box::new(BoxTool::default()),
        };

        editor.load_from_file()?;

        Ok(editor)
    }

    fn load_from_file(&mut self) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(0))?;

        self.buffer = BufReader::new(&mut self.file)
            .lines()
            .map(|lr| lr.map(|s| s.chars().collect()))
            .collect::<io::Result<_>>()?;

        let rendered: String = self
            .buffer
            .iter()
            .flat_map(|v| v.iter().chain(iter::once(&'\n')))
            .collect();

        self.render.set_content(rendered);

        Ok(())
    }

    fn content(&self) -> TextContent {
        self.render.clone()
    }

    fn render(&mut self, save: bool) {
        let mut preview_buffer;

        let buffer = if save {
            &mut self.buffer
        } else {
            preview_buffer = self.buffer.clone();
            &mut preview_buffer
        };

        if !self.tool.render_to(buffer) {
            return;
        }

        let rendered: String = buffer
            .iter()
            .flat_map(|v| v.iter().chain(iter::once(&'\n')))
            .collect();

        self.render.set_content(rendered);
    }
}
