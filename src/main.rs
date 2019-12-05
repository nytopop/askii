// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
extern crate cursive;
extern crate lazy_static;
extern crate line_drawing;
extern crate log;
extern crate parking_lot;
extern crate structopt;

pub mod editor;
pub mod tools;
pub mod ui;

use editor::*;
use tools::*;
use ui::*;

use cursive::{
    event::{Event, EventResult, EventTrigger, Key, MouseButton, MouseEvent},
    logger,
    menu::MenuTree,
    traits::Identifiable,
    view::scroll::Scroller,
    views::{Dialog, OnEventView, Panel, ScrollView},
    Cursive, Vec2,
};
use lazy_static::lazy_static;
use log::debug;
use parking_lot::Mutex;
use std::{env, error::Error, path::PathBuf};
use structopt::StructOpt;

type MainResult<T> = Result<T, Box<dyn Error>>;

pub const EDITOR_ID: &'static str = "editor";
const S90: &'static str = "Snap 90";
const S45: &'static str = "Snap 45";

fn main() -> MainResult<()> {
    // TODO: consider the case of incompatible terminals
    env::set_var("TERM", "xterm-1006");
    logger::init();

    let opts = Options::from_args();
    debug!("{:?}", opts);

    let editor = Editor::open(opts)?;
    let mut siv = Cursive::ncurses()?;

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
                .leaf("(q) Quit", Cursive::quit),
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
        .add_delimiter()
        .add_leaf(editor.active_tool(), |_| ());

    siv.set_autohide_menu(false);

    siv.add_global_callback(Key::Esc, |s| s.select_menubar());

    // avail: c e f g i j k p v w x y z

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

    // Help
    siv.add_global_callback('h', editor_help);

    siv.add_fullscreen_layer(Panel::new(
        OnEventView::new(ScrollView::new(editor).scroll_x(true).scroll_y(true))
            .on_pre_event_inner(EventTrigger::mouse(), editor_mouse)
            .with_id(EDITOR_ID),
    ));

    siv.run();

    Ok(())
}

fn editor_new(siv: &mut Cursive) {
    with_clean_editor(siv, |siv| with_editor(siv, Editor::clear));
}

fn editor_open(siv: &mut Cursive) {
    with_clean_editor(siv, |siv| {
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

            if let Err(e) = with_editor(siv, |e| e.open_file(path)) {
                notify(siv, "open failed", format!("{:?}", e));
            }
        })
    });
}

fn editor_save(siv: &mut Cursive) {
    match with_editor(siv, Editor::save).map_err(|e| format!("{:?}", e)) {
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

        match with_editor(siv, |e| e.save_as(path)).map_err(|e| format!("{:?}", e)) {
            Ok(()) => notify(siv, "saved", ""),
            Err(e) => notify(siv, "save as failed", e),
        }
    });
}

fn editor_quit(siv: &mut Cursive) {
    with_clean_editor(siv, Cursive::quit);
}

fn editor_undo(siv: &mut Cursive) {
    with_editor(siv, Editor::undo);
}

fn editor_redo(siv: &mut Cursive) {
    with_editor(siv, Editor::redo);
}

fn editor_trim_margins(siv: &mut Cursive) {
    with_editor(siv, Editor::trim_margins);
}

fn editor_tool<T, S>(set: S) -> impl Fn(&mut Cursive) + 'static
where
    T: Tool + Default + 'static,
    S: Fn(&mut Options) + 'static,
{
    move |siv| {
        let tool = with_editor(siv, |editor| {
            set(editor.opts_mut());
            editor.set_tool(T::default());
            editor.active_tool()
        });

        let menu = siv.menubar();
        menu.remove(menu.len() - 1);
        menu.insert_leaf(menu.len(), tool, |_| ());
    }
}

fn editor_help(siv: &mut Cursive) {
    let keybinds = vec![
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
        "(b) Box Tool",
        "(l) Line Tool: Snap 90",
        "(L) Line Tool: Snap 45",
        "(a) Arrow Tool: Snap 90",
        "(A) Arrow Tool: Snap 45",
        "(t) Text Tool",
        "",
        "(h) Help",
    ]
    .join("\n");

    notify_uniq(siv, "help_dialog", "Help", keybinds);
}

lazy_static! {
    static ref LAST_LPRESS: Mutex<Option<Vec2>> = Mutex::new(None);
    static ref RPOINTER: Mutex<Option<Vec2>> = Mutex::new(None);
}

const CONSUMED: Option<EventResult> = Some(EventResult::Consumed(None));

fn editor_mouse(view: &mut ScrollView<Editor>, event: &Event) -> Option<EventResult> {
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

    let viewport = view.content_viewport();
    let content_pos = pos.saturating_sub(offset) + viewport.top_left();

    match event {
        Press(Left) if on_scrollbar(view, offset, pos) => {
            *LAST_LPRESS.lock() = Some(pos);
            None
        }

        Hold(Left)
            if LAST_LPRESS
                .lock()
                .map(|pos| on_scrollbar(view, offset, pos))
                .unwrap_or(false) =>
        {
            None
        }

        Release(Left)
            if LAST_LPRESS
                .lock()
                .take()
                .map(|pos| on_scrollbar(view, offset, pos))
                .unwrap_or(false) =>
        {
            None
        }

        WheelUp | WheelDown => None,

        Press(Left) => {
            *LAST_LPRESS.lock() = Some(pos);
            get_editor(view).press(content_pos);
            CONSUMED
        }

        // BUG: this scrolls even if the tool isn't a drag type
        Hold(Left) => {
            let editor = get_editor(view);
            editor.hold(content_pos);

            let pos = pos - offset;
            let bounds = viewport.bottom_right() - viewport.top_left();

            let mut offset = viewport.top_left();
            if pos.x > bounds.x {
                offset.x = offset.x.saturating_add(3);
                editor.set_x_bound(offset.x + bounds.x);
            } else if pos.x == 0 {
                offset.x = offset.x.saturating_sub(3);
            }

            if pos.y > bounds.y {
                offset.y = offset.y.saturating_add(3);
                editor.set_y_bound(offset.y + bounds.y);
            } else if pos.y == 0 {
                offset.y = offset.y.saturating_sub(3);
            }

            view.set_offset(offset);
            CONSUMED
        }

        Release(Left) => {
            get_editor(view).release(content_pos);
            CONSUMED
        }

        Press(Right) => {
            *RPOINTER.lock() = Some(pos);
            CONSUMED
        }

        Hold(Right) if RPOINTER.lock().is_none() => {
            *RPOINTER.lock() = Some(pos);
            CONSUMED
        }

        Hold(Right) => {
            let Vec2 { x, y } = RPOINTER.lock().replace(pos).unwrap();
            let mut offset = viewport.top_left();

            if pos.x > x {
                offset.x = offset.x.saturating_sub(pos.x - x);
            } else if pos.x < x {
                offset.x = offset.x.saturating_add(x - pos.x);

                if within(1, viewport.right(), view.inner_size().x) {
                    get_editor(view).bounds().x += x - pos.x;
                }
            }

            if pos.y > y {
                offset.y = offset.y.saturating_sub(pos.y - y);
            } else if pos.y < y {
                offset.y = offset.y.saturating_add(y - pos.y);

                if within(1, viewport.bottom(), view.inner_size().y) {
                    get_editor(view).bounds().y += y - pos.y;
                }
            }

            view.set_offset(offset);
            CONSUMED
        }

        Release(Right) => {
            *RPOINTER.lock() = None;
            CONSUMED
        }

        _ => None,
    }
}

fn on_scrollbar<S: Scroller>(scroll: &S, offset: Vec2, pos: Vec2) -> bool {
    let core = scroll.get_scroller();
    let max = core.last_size() + offset;
    let min = max - core.scrollbar_size();

    (min.x..=max.x).contains(&pos.x) || (min.y..=max.y).contains(&pos.y)
}

fn within(w: usize, x: usize, y: usize) -> bool {
    ((x as isize - y as isize).abs() as usize) <= w
}
