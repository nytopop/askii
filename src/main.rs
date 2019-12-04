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

pub(crate) mod editor;
pub(crate) mod tools;

use editor::*;
use tools::*;

use cursive::{
    align::HAlign,
    event::{Event, EventResult, EventTrigger, Key, MouseButton, MouseEvent},
    logger,
    menu::MenuTree,
    traits::Identifiable,
    view::scroll::Scroller,
    views::{Dialog, EditView, IdView, OnEventView, Panel, ScrollView},
    Cursive, Vec2,
};
use lazy_static::lazy_static;
use log::info;
use parking_lot::Mutex;
use std::{env, error::Error, path::PathBuf, rc::Rc};
use structopt::StructOpt;

type MainResult<T> = Result<T, Box<dyn Error>>;

const EDITOR_ID: &'static str = "editor";

fn main() -> MainResult<()> {
    // TODO: consider the case of incompatible terminals
    env::set_var("TERM", "xterm-1006");

    logger::init();
    let opts = Options::from_args();
    info!("{:?}", opts);
    let editor = Editor::open(opts)?;

    let mut siv = Cursive::default();

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
        .add_leaf("Text", set_tool::<TextTool, _>(|_| ()))
        .add_delimiter()
        .add_leaf(editor.active_tool(), |_| ());

    siv.set_autohide_menu(false);
    siv.add_global_callback('`', Cursive::toggle_debug_console);
    siv.add_global_callback('q', Cursive::quit);
    siv.add_global_callback(Key::Esc, |s| s.select_menubar());

    let editor_layer = Panel::new(IdView::new(
        EDITOR_ID,
        OnEventView::new(ScrollView::new(editor).scroll_x(true).scroll_y(true))
            .on_pre_event_inner(EventTrigger::any(), editor_callback),
    ));
    siv.add_fullscreen_layer(editor_layer);

    siv.run();

    Ok(())
}

// TODO: are you sure?
fn editor_new(siv: &mut Cursive) {
    let mut view = siv
        .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
        .unwrap();
    get_editor(view.get_inner_mut()).clear();
}

/// Display a single line input form, passing the submitted content into the provided
/// callback `form`.
fn display_form<T: Into<String>, F: 'static>(siv: &mut Cursive, title: T, form: F)
where
    F: Fn(&mut Cursive, &'static str, &str),
{
    const INPUT_ID: &'static str = "generic_input";
    const POPUP_ID: &'static str = "generic_popup";

    let submit = Rc::new(move |siv: &mut Cursive, input: &str| {
        form(siv, POPUP_ID, input);
    });

    let submit_ok = Rc::clone(&submit);

    let input = EditView::new()
        .on_submit(move |siv, input| submit(siv, input))
        .with_id(INPUT_ID);

    let popup = Dialog::around(input)
        .title(title)
        .button("Ok", move |siv| {
            let input = siv
                .call_on_id(INPUT_ID, |view: &mut EditView| view.get_content())
                .unwrap();
            submit_ok(siv, &input);
        })
        .dismiss_button("Cancel")
        .with_id(POPUP_ID);

    siv.add_layer(popup);
}

// TODO: are you sure?
fn editor_open(siv: &mut Cursive) {
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

        let mut view = siv
            .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
            .unwrap();

        match get_editor(view.get_inner_mut()).open_file(path) {
            Err(e) => notify(siv, "open failed", format!("{:?}", e)),
            Ok(()) => {}
        }
    });
}

fn notify<T: Into<String>, C: Into<String>>(siv: &mut Cursive, title: T, content: C) {
    siv.add_layer(
        Dialog::info(content)
            .title(title)
            .h_align(HAlign::Center)
            .padding(((0, 0), (0, 0))),
    );
}

fn notify_uniq<T: Into<String>, C: Into<String>>(
    siv: &mut Cursive,
    uniq: &'static str,
    title: T,
    content: C,
) {
    if siv.find_id::<Dialog>(uniq).is_some() {
        return;
    }

    siv.add_layer(IdView::new(
        uniq,
        Dialog::info(content)
            .title(title)
            .h_align(HAlign::Center)
            .padding(((0, 0), (0, 0))),
    ));
}

fn editor_save(siv: &mut Cursive) {
    let mut view = siv
        .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
        .unwrap();
    let editor = get_editor(view.get_inner_mut());

    match editor.save().map_err(|e| format!("{:?}", e)) {
        Ok(false) => editor_save_as(siv),
        Ok(true) => notify(siv, "saved", ""),
        Err(e) => notify(siv, "save failed", e),
    }
}

fn editor_save_as(siv: &mut Cursive) {
    display_form(siv, "Save As", |siv, id, raw_path| {
        let mut view = siv.find_id::<Dialog>(id).unwrap();

        let path: PathBuf = raw_path.into();
        if path.exists() {
            // TODO: prompt for overwrite
            view.set_title(format!("Save As: {:?} already exists!", path));
            return;
        }
        if path.is_dir() {
            view.set_title(format!("Save As: {:?} is a directory!", path));
            return;
        }
        siv.pop_layer();

        let mut view = siv
            .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
            .unwrap();
        let editor = get_editor(view.get_inner_mut());

        match editor.save_as(path).map_err(|e| format!("{:?}", e)) {
            Ok(()) => notify(siv, "saved", ""),
            Err(e) => notify(siv, "save as failed", e),
        }
    });
}

fn editor_undo(siv: &mut Cursive) {
    let mut view = siv
        .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
        .unwrap();
    get_editor(view.get_inner_mut()).undo();
}

fn editor_redo(siv: &mut Cursive) {
    let mut view = siv
        .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
        .unwrap();
    get_editor(view.get_inner_mut()).redo();
}

fn editor_trim_margins(siv: &mut Cursive) {
    let mut view = siv
        .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
        .unwrap();
    get_editor(view.get_inner_mut()).trim_margins();
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
        "\n# Edit",
        "(u) Undo",
        "(r) Redo",
        "(m) Trim Margins",
        "\n# Tools",
        "(b) Box Tool",
        "(l) Line Tool: Snap 90",
        "(L) Line Tool: Snap 45",
        "(a) Arrow Tool: Snap 90",
        "(A) Arrow Tool: Snap 45",
        "(t) Text Tool",
        "",
        "(h) Help",
    ]
    .into_iter()
    .map(str::to_owned)
    .map(|mut s| {
        s.push('\n');
        s
    });

    notify_uniq(siv, "help_dialog", "Help", keybinds.collect::<String>());
}

fn set_tool<T, S>(set: S) -> impl Fn(&mut Cursive) + 'static
where
    T: Tool + Default + 'static,
    S: Fn(&mut Options) + 'static,
{
    move |siv| {
        let tool = {
            let mut view = siv
                .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
                .unwrap();
            let editor = get_editor(view.get_inner_mut());

            set(editor.opts_mut());
            editor.set_tool(T::default());
            editor.active_tool()
        };

        let menu = siv.menubar();
        menu.remove(menu.len() - 1);
        menu.insert_leaf(menu.len(), tool, |_| ());
    }
}

fn get_editor(scroll_view: &mut ScrollView<Editor>) -> &mut Editor {
    scroll_view.get_inner_mut()
}

const CONSUMED: Option<EventResult> = Some(EventResult::Consumed(None));

fn cb<F: Fn(&mut Cursive) + 'static>(f: F) -> Option<EventResult> {
    Some(EventResult::with_cb(f))
}

fn editor_callback(scroll_view: &mut ScrollView<Editor>, event: &Event) -> Option<EventResult> {
    match event {
        Event::Mouse {
            offset,
            position,
            event,
        } => handle_mouse(scroll_view, *offset, *position, *event),

        // avail: c e f g i j k p v w x y z

        // File
        Event::Char('n') => cb(editor_new),
        Event::Char('o') => cb(editor_open),
        Event::Char('s') => cb(editor_save),
        Event::Char('S') => cb(editor_save_as),

        // Edit
        Event::Char('u') => cb(editor_undo),
        Event::Char('r') => cb(editor_redo),
        Event::Char('m') => cb(editor_trim_margins),

        // Tools
        Event::Char('b') => cb(set_tool::<BoxTool, _>(|_| ())),
        Event::Char('l') => cb(set_tool::<LineTool, _>(|o| o.line_snap45 = false)),
        Event::Char('L') => cb(set_tool::<LineTool, _>(|o| o.line_snap45 = true)),
        Event::Char('a') => cb(set_tool::<ArrowTool, _>(|o| o.line_snap45 = false)),
        Event::Char('A') => cb(set_tool::<ArrowTool, _>(|o| o.line_snap45 = true)),
        Event::Char('t') => cb(set_tool::<TextTool, _>(|_| ())),

        // Help
        Event::Char('h') => cb(editor_help),

        _ => None,
    }
}

lazy_static! {
    static ref LAST_LPRESS: Mutex<Option<Vec2>> = Mutex::new(None);
    static ref RPOINTER: Mutex<Option<Vec2>> = Mutex::new(None);
}

fn handle_mouse(
    scroll_view: &mut ScrollView<Editor>,
    offset: Vec2,
    pos: Vec2,
    event: MouseEvent,
) -> Option<EventResult> {
    use MouseButton::*;
    use MouseEvent::*;

    let viewport = scroll_view.content_viewport();
    let content_pos = pos.saturating_sub(offset) + viewport.top_left();

    match event {
        Press(Left) if on_scrollbar(scroll_view, offset, pos) => {
            *LAST_LPRESS.lock() = Some(pos);
            None
        }

        Hold(Left)
            if LAST_LPRESS
                .lock()
                .map(|pos| on_scrollbar(scroll_view, offset, pos))
                .unwrap_or(false) =>
        {
            None
        }

        Release(Left)
            if LAST_LPRESS
                .lock()
                .take()
                .map(|pos| on_scrollbar(scroll_view, offset, pos))
                .unwrap_or(false) =>
        {
            None
        }

        WheelUp | WheelDown => None,

        Press(Left) => {
            *LAST_LPRESS.lock() = Some(pos);
            get_editor(scroll_view).press(content_pos);
            CONSUMED
        }

        // BUG: this scrolls even if the tool isn't a drag type
        Hold(Left) => {
            let editor = get_editor(scroll_view);
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

            scroll_view.set_offset(offset);
            CONSUMED
        }

        Release(Left) => {
            get_editor(scroll_view).release(content_pos);
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

                if within(1, viewport.right(), scroll_view.inner_size().x) {
                    get_editor(scroll_view).bounds().x += x - pos.x;
                }
            }

            if pos.y > y {
                offset.y = offset.y.saturating_sub(pos.y - y);
            } else if pos.y < y {
                offset.y = offset.y.saturating_add(y - pos.y);

                if within(1, viewport.bottom(), scroll_view.inner_size().y) {
                    get_editor(scroll_view).bounds().y += y - pos.y;
                }
            }

            scroll_view.set_offset(offset);
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
