// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
//! TUI based ASCII diagram editor.
// TODO: path mode for line and arrow
// TODO: shapes (diamond, hexagon, parallelogram, trapezoid)
// TODO: tools (resize, select)
// TODO: think of a way to do tests (dummy backend + injected events?)
// TODO: only store deltas in undo history
// TODO: use an undo file
// TODO: show a proper modeline
extern crate cursive;
extern crate lazy_static;
extern crate line_drawing;
extern crate log;
extern crate parking_lot;
extern crate structopt;

mod editor;
mod tools;
mod ui;

use editor::*;
use tools::*;
use ui::*;

use cursive::{
    event::{EventTrigger, Key},
    logger,
    menu::MenuTree,
    view::{scroll::Scroller, Identifiable, View},
    views::{Dialog, OnEventView, ScrollView},
    Cursive,
};
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
    // true : lines bend 45 degrees
    // false: lines bend 90 degrees
    #[structopt(skip = false)]
    line_snap45: bool,

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

const EDITOR_ID: &str = "editor";
const S90: &str = "Snap 90";
const S45: &str = "Snap 45";

fn main() -> Result<(), Box<dyn Error>> {
    // TODO: consider the case of incompatible terminals
    env::set_var("TERM", "xterm-1006");
    logger::init();

    let opts = Options::from_args();
    debug!("{:?}", opts);

    let editor = Editor::open(opts)?;
    let mut siv = Cursive::pancurses()?;

    siv.menubar()
        .add_subtree(
            "File",
            MenuTree::new()
                .leaf("(n) New", editor_new)
                .leaf("(o) Open", editor_open)
                .leaf("(s) Save", editor_save)
                .leaf("(S) Save As", editor_save_as)
                .delimiter()
                .leaf("(`) Debug", Cursive::toggle_debug_console)
                .leaf("(q) Quit", editor_quit),
        )
        .add_subtree(
            "Edit",
            MenuTree::new()
                .leaf("(u) Undo", editor_undo)
                .leaf("(r) Redo", editor_redo)
                .leaf("(m) Trim Margins", editor_trim_margins),
        )
        .add_leaf("Help", editor_help)
        .add_delimiter()
        .add_leaf("Box", editor_tool::<BoxTool, _>(|_| ()))
        .add_subtree(
            "Line",
            MenuTree::new()
                .leaf(S90, editor_tool::<LineTool, _>(|o| o.line_snap45 = false))
                .leaf(S45, editor_tool::<LineTool, _>(|o| o.line_snap45 = true)),
        )
        .add_subtree(
            "Arrow",
            MenuTree::new()
                .leaf(S90, editor_tool::<ArrowTool, _>(|o| o.line_snap45 = false))
                .leaf(S45, editor_tool::<ArrowTool, _>(|o| o.line_snap45 = true)),
        )
        .add_leaf("Text", editor_tool::<TextTool, _>(|_| ()))
        .add_leaf("Erase", editor_tool::<EraseTool, _>(|_| ()))
        .add_delimiter()
        .add_leaf(editor.active_tool(), |_| ());

    // * * c d * f g * i j k * * * * p * * * * * v w x y z
    // * B C D E F G H I J K * M N O P Q R * T U V W X Y Z

    siv.set_autohide_menu(false);
    siv.add_global_callback(Key::Esc, |s| s.select_menubar());

    // File
    siv.add_global_callback('n', editor_new);
    siv.add_global_callback('o', editor_open);
    siv.add_global_callback('s', editor_save);
    siv.add_global_callback('S', editor_save_as);
    siv.add_global_callback('`', Cursive::toggle_debug_console);
    siv.add_global_callback('q', editor_quit);

    // Edit
    siv.add_global_callback('u', editor_undo);
    siv.add_global_callback('r', editor_redo);
    siv.add_global_callback('m', editor_trim_margins);

    // Tools
    siv.add_global_callback('b', editor_tool::<BoxTool, _>(|_| ()));
    siv.add_global_callback('l', editor_tool::<LineTool, _>(|o| o.line_snap45 = false));
    siv.add_global_callback('L', editor_tool::<LineTool, _>(|o| o.line_snap45 = true));
    siv.add_global_callback('a', editor_tool::<ArrowTool, _>(|o| o.line_snap45 = false));
    siv.add_global_callback('A', editor_tool::<ArrowTool, _>(|o| o.line_snap45 = true));
    siv.add_global_callback('t', editor_tool::<TextTool, _>(|_| ()));
    siv.add_global_callback('e', editor_tool::<EraseTool, _>(|_| ()));

    // Help
    siv.add_global_callback('h', editor_help);

    siv.add_fullscreen_layer(
        OnEventView::new(new_scrollview(editor).with_id(EDITOR_ID)).on_pre_event_inner(
            EventTrigger::any(),
            |view, event| {
                let mut scroll = view.get_mut();
                let mut ctx = EditorCtx::new(&mut scroll);
                ctx.on_event(event)
            },
        ),
    );

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
            let mut view = siv.find_id::<Dialog>(id).unwrap();

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
        let mut view = siv.find_id::<Dialog>(id).unwrap();

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
}

fn editor_tool<'a, T: 'static, S: 'a>(apply: S) -> impl Fn(&mut Cursive) + 'a
where
    T: Tool + Default,
    S: Fn(&mut Options),
{
    move |siv| {
        let stat = with_editor_mut(siv, |editor| {
            editor.mut_opts(|o| apply(o));
            editor.set_tool(T::default());
            editor.active_tool()
        });

        let m = siv.menubar();
        m.remove(m.len() - 1);
        m.insert_leaf(m.len(), stat, |_| ());
    }
}

fn editor_help(siv: &mut Cursive) {
    let version_str = format!("askii {}", env!("CARGO_PKG_VERSION"));

    let authors = env!("CARGO_PKG_AUTHORS")
        .split(':')
        .map(|s| format!("* {}", s))
        .collect::<Vec<_>>()
        .join("\n");

    let author_str = format!("Made with love by:\n{}", authors);

    let help_str = vec![
        &*version_str,
        "",
        &*author_str,
        "",
        "# File",
        "(n) New",
        "(o) Open",
        "(s) Save",
        "(S) Save As",
        "(`) Debug Console",
        "(q) Quit",
        "",
        "# Edit",
        "(u) Undo",
        "(r) Redo",
        "(m) Trim Margins",
        "",
        "# Tools",
        "(b) Box",
        "(l) Line: Snap 90",
        "(L) Line: Snap 45",
        "(a) Arrow: Snap 90",
        "(A) Arrow: Snap 45",
        "(t) Text",
        "(e) Erase",
        "",
        "(h) Help",
    ]
    .join("\n");

    // TODO: (H): tool specific help

    notify_unique(siv, "editor_help", "Help", help_str);
}
