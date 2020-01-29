// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//! TUI based ASCII diagram editor.
// # TODO Features
// - shapes (diamond, hexagon, parallelogram, trapezoid)
// - resize tool
// - box with text header area
// - unicode
// - maximum canvas width
// - banner style text
//
// # TODO Enhancements
// - atomically write files (w/ backup)
// - only store deltas in undo history
// - store undo history in a file
// - display status changes in modeline (saved, opened, trimmed, clipped, etc)
//
// # TODO Correctness
// - think of a way to do tests (dummy backend + injected events?)
// - performance of a* is abysmal across large distances
#![allow(clippy::many_single_char_names)]
mod editor;
mod modeline;
mod tools;
mod ui;

use editor::*;
use modeline::*;
use tools::{PathMode::*, *};
use ui::*;

use cursive::{
    backend::{crossterm::Backend as CrossTerm, Backend},
    event::{EventTrigger, Key},
    logger,
    menu::MenuTree,
    view::{scroll::Scroller, Nameable, View},
    views::{Dialog, LinearLayout, OnEventView, ScrollView},
    Cursive,
};
use cursive_buffered_backend::BufferedBackend;
use log::debug;
use std::{env, error::Error, path::PathBuf};
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    author = "Made with love by nytopop <ericizoita@gmail.com>.",
    help_message = "Print help information.",
    version_message = "Print version information."
)]
struct Options {
    /// How paths are routed.
    #[structopt(skip = PathMode::Snap90)]
    path_mode: PathMode,

    /// Keep trailing whitespace (on save).
    #[structopt(short, long)]
    keep_trailing_ws: bool,

    /// Strip all margin whitespace (on save).
    #[structopt(short, long)]
    strip_margin_ws: bool,

    /// Text file to operate on.
    #[structopt(name = "FILE")]
    file: Option<PathBuf>,
}

impl Options {
    fn cycle_path_mode(&mut self) {
        self.path_mode = match self.path_mode {
            Snap90 => Snap45,
            Snap45 => Routed,
            Routed => Snap90,
        };
    }
}

const EDITOR_ID: &str = "editor";
const S90: &str = "Snap90";
const S45: &str = "Snap45";
const RTD: &str = "Routed";

fn main() -> Result<(), Box<dyn Error>> {
    logger::init();
    log::set_max_level(log::LevelFilter::Info);

    let opts = Options::from_args();
    debug!("{:?}", opts);

    let editor = EditorView::new(Editor::open(opts)?);
    let mut siv = Cursive::try_new(|| {
        CrossTerm::init()
            .map(|cross| BufferedBackend::new(cross))
            .map(|buf| -> Box<dyn Backend> { Box::new(buf) })
    })?;

    use PathMode::*;

    siv.menubar()
        .add_subtree(
            "File",
            MenuTree::new()
                .leaf("(n) New", editor_new)
                .leaf("(o) Open", editor_open)
                .leaf("(s) Save", editor_save)
                .leaf("(S) Save As", editor_save_as)
                .leaf("(c) Clip", editor_clip)
                .leaf("(C) Clip Prefix", editor_clip_prefix)
                .delimiter()
                .leaf("(`) Debug", Cursive::toggle_debug_console)
                .leaf("(q) Quit", editor_quit),
        )
        .add_subtree(
            "Edit",
            MenuTree::new()
                .leaf("(u) Undo", editor_undo)
                .leaf("(r) Redo", editor_redo)
                .leaf("(T) Trim Margins", editor_trim_margins),
        )
        .add_leaf("Help", editor_help)
        .add_delimiter()
        .add_leaf("Box", editor_tool::<BoxTool, _>(|_| ()))
        .add_subtree(
            "Line",
            MenuTree::new()
                .leaf(S90, editor_tool::<LineTool, _>(|o| o.path_mode = Snap90))
                .leaf(S45, editor_tool::<LineTool, _>(|o| o.path_mode = Snap45))
                .leaf(RTD, editor_tool::<LineTool, _>(|o| o.path_mode = Routed)),
        )
        .add_subtree(
            "Arrow",
            MenuTree::new()
                .leaf(S90, editor_tool::<ArrowTool, _>(|o| o.path_mode = Snap90))
                .leaf(S45, editor_tool::<ArrowTool, _>(|o| o.path_mode = Snap45))
                .leaf(RTD, editor_tool::<ArrowTool, _>(|o| o.path_mode = Routed)),
        )
        .add_leaf("Text", editor_tool::<TextTool, _>(|_| ()))
        .add_leaf("Erase", editor_tool::<EraseTool, _>(|_| ()))
        .add_leaf("Move", editor_tool::<MoveTool, _>(|_| ()));

    // * * * d * f g * i j k * * * * * * * * * * v w x y z
    // A B * D E F G H I J K L M N O P Q R * * U V W X Y Z

    siv.set_autohide_menu(false);
    siv.add_global_callback(Key::Esc, |s| s.select_menubar());

    // File
    siv.add_global_callback('n', editor_new);
    siv.add_global_callback('o', editor_open);
    siv.add_global_callback('s', editor_save);
    siv.add_global_callback('S', editor_save_as);
    siv.add_global_callback('c', editor_clip);
    siv.add_global_callback('C', editor_clip_prefix);
    siv.add_global_callback('`', Cursive::toggle_debug_console);
    siv.add_global_callback('q', editor_quit);

    // Edit
    siv.add_global_callback('u', editor_undo);
    siv.add_global_callback('r', editor_redo);
    siv.add_global_callback('T', editor_trim_margins);

    // Tools
    siv.add_global_callback('b', editor_tool::<BoxTool, _>(|_| ()));
    siv.add_global_callback('l', editor_tool::<LineTool, _>(|_| ()));
    siv.add_global_callback('a', editor_tool::<ArrowTool, _>(|_| ()));
    siv.add_global_callback('p', modify_opts(Options::cycle_path_mode));
    siv.add_global_callback('t', editor_tool::<TextTool, _>(|_| ()));
    siv.add_global_callback('e', editor_tool::<EraseTool, _>(|_| ()));
    siv.add_global_callback('m', editor_tool::<MoveTool, _>(|_| ()));

    // Help
    siv.add_global_callback('h', editor_help);

    let edit_view = OnEventView::new(new_scrollview(editor.clone()).with_name(EDITOR_ID))
        .on_pre_event_inner(EventTrigger::any(), |view, event| {
            let mut scroll = view.get_mut();
            let mut ctx = EditorCtx::new(&mut scroll);
            ctx.on_event(event)
        });

    let layout = LinearLayout::vertical()
        .child(edit_view)
        .weight(100)
        .child(ModeLine::new(editor))
        .weight(1);

    siv.add_fullscreen_layer(layout);

    siv.run();

    Ok(())
}

fn new_scrollview<V: View>(inner: V) -> ScrollView<V> {
    let mut scroll = ScrollView::new(inner).scroll_x(true).scroll_y(true);
    scroll.get_scroller_mut().set_scrollbar_padding((0, 0));
    scroll
}

fn editor_new(siv: &mut Cursive) {
    with_checked_editor(siv, "New", |siv| with_editor_mut(siv, Editor::clear));
}

fn editor_open(siv: &mut Cursive) {
    with_checked_editor(siv, "Open", |siv| {
        display_form(siv, "Open", |siv, id, raw_path| {
            let mut view = siv.find_name::<Dialog>(id).unwrap();

            if raw_path.is_empty() {
                view.set_title("Open: path is empty!");
                return;
            }

            let path: PathBuf = raw_path.into();
            if !path.exists() {
                view.set_title(format!("Open: {:?} does not exist!", path));
                return;
            }
            if !path.is_file() {
                view.set_title(format!("Open: {:?} is not a file!", path));
                return;
            }
            siv.pop_layer();

            if let Err(e) = with_editor_mut(siv, |e| e.open_file(path)) {
                notify(siv, "open failed", format!("{:?}", e));
            }
        })
    });
}

fn editor_save(siv: &mut Cursive) {
    match with_editor_mut(siv, Editor::save).map_err(|e| format!("{:?}", e)) {
        Ok(false) => editor_save_as(siv),
        Ok(true) => notify(siv, "saved", ""),
        Err(e) => notify(siv, "save failed", e),
    }
}

fn editor_save_as(siv: &mut Cursive) {
    display_form(siv, "Save As", |siv, id, raw_path| {
        let mut view = siv.find_name::<Dialog>(id).unwrap();

        if raw_path.is_empty() {
            view.set_title("Save As: path is empty!");
            return;
        }

        let path: PathBuf = raw_path.into();
        if path.is_dir() {
            view.set_title(format!("Save As: {:?} is a directory!", path));
            return;
        }
        siv.pop_layer();

        match with_editor_mut(siv, |e| e.save_as(path)).map_err(|e| format!("{:?}", e)) {
            Ok(()) => notify(siv, "saved", ""),
            Err(e) => notify(siv, "save as failed", e),
        }
    });
}

fn editor_clip(siv: &mut Cursive) {
    match with_editor(siv, |e| e.render_to_clipboard("")).map_err(|e| format!("{:?}", e)) {
        Ok(()) => notify(siv, "clipped", ""),
        Err(e) => notify(siv, "clip failed", e),
    }
}

fn editor_clip_prefix(siv: &mut Cursive) {
    display_form(siv, "Clip Prefix", |siv, _, prefix| {
        siv.pop_layer();

        match with_editor(siv, |e| e.render_to_clipboard(prefix)).map_err(|e| format!("{:?}", e)) {
            Ok(()) => notify(siv, "clipped", ""),
            Err(e) => notify(siv, "clip failed", e),
        }
    });
}

fn editor_quit(siv: &mut Cursive) {
    with_checked_editor(siv, "Quit", Cursive::quit);
}

fn editor_undo(siv: &mut Cursive) {
    with_editor_mut(siv, Editor::undo);
}

fn editor_redo(siv: &mut Cursive) {
    with_editor_mut(siv, Editor::redo);
}

fn editor_trim_margins(siv: &mut Cursive) {
    with_editor_mut(siv, Editor::trim_margins);
    notify(siv, "trimmed", "");
}

fn editor_tool<'a, T: 'static, S: 'a>(apply: S) -> impl Fn(&mut Cursive) + 'a
where
    T: Tool + Default,
    S: Fn(&mut Options),
{
    move |siv| {
        with_editor_mut(siv, |editor| {
            editor.mut_opts(|o| apply(o));
            editor.set_tool(T::default());
        });
    }
}

fn modify_opts<'a, S: 'a>(apply: S) -> impl Fn(&mut Cursive) + 'a
where
    S: Fn(&mut Options),
{
    move |siv| with_editor_mut(siv, |editor| editor.mut_opts(|o| apply(o)))
}

// TODO: H   Show tool specific help.
const HELP: &str = "KEYBINDS:
    Esc Focus the menu bar.
    n   New: Open a new (blank) file.
    o   Open: Open the specified file.
    s   Save: Save buffer to the current path. If there isn't one, this is equivalent to Save As.
    S   Save As: Save buffer to the specified path.
    c   Clip: Export buffer to the clipboard.
    C   Clip Prefix: Export buffer to the clipboard with a prefix before each line.
    `   Debug: Open the debug console.
    q   Quit: Quit without saving.
    u   Undo: Undo the last buffer modification.
    r   Redo: Redo the last undo.
    T   Trim Margins: Trim excess whitespace from all margins.
    b   Switch to the Box tool.
    l   Switch to the Line tool.
    a   Switch to the Arrow tool.
    p   Cycle the type of path that Line and Arrow tools will draw.
    t   Switch to the Text tool.
    e   Switch to the Erase tool.
    h   Help: Display this help message.

NAVIGATION:
    Scroll with the arrow keys or page-up and page-down.

    Pan around by dragging with the right mouse button.

    Menus are keyboard aware, too!

TOOLS:
    Box   Draw boxes. Click and drag to the desired dimensions.

    Line  Draw lines. Click and drag to the target position.

    Arrow Draw arrows. Click and drag to the target position.

    Text  Write text. Click somewhere and type. Esc will save the content, while clicking anywhere on the canvas discards it.

    Erase Erase things. Click and drag to form a box, everything inside of which will be erased.

    Move  Move existing content. Click and drag to select an area, then click and drag from inside the area to move its content. Clicking outside of the selected area resets the selection.";

fn editor_help(siv: &mut Cursive) {
    let version_str = format!("askii {}", env!("CARGO_PKG_VERSION"));

    let authors = env!("CARGO_PKG_AUTHORS")
        .split(':')
        .map(|s| format!("* {}", s))
        .collect::<Vec<_>>()
        .join("\n");

    let author_str = format!("Made with love by:\n{}", authors);

    let help_str = format!("{}\n\n{}\n\n{}", version_str, author_str, HELP);

    notify_unique(siv, "editor_help", "Help", help_str);
}
