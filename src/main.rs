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
    event::{Callback, Event, EventResult, EventTrigger, Key, MouseButton, MouseEvent},
    logger,
    menu::MenuTree,
    view::scroll::Scroller,
    views::{Dialog, IdView, OnEventView, Panel, ScrollView, ViewRef},
    Cursive, Vec2,
};
use lazy_static::lazy_static;
use log::info;
use parking_lot::Mutex;
use std::error::Error;
use structopt::StructOpt;

type MainResult<T> = Result<T, Box<dyn Error>>;

fn main() -> MainResult<()> {
    logger::init();
    let opts = Options::from_args();
    info!("{:?}", opts);
    let editor = Editor::open(opts)?;

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
        .add_leaf("Text", set_tool::<TextTool, _>(|_| ()))
        .add_delimiter()
        .add_leaf(editor.active_tool(), |_| ());

    siv.set_autohide_menu(false);
    siv.add_global_callback('`', Cursive::toggle_debug_console);
    siv.add_global_callback('q', Cursive::quit);
    siv.add_global_callback(Key::Esc, |s| s.select_menubar());

    let editor_layer = Panel::new(IdView::new(
        "editor",
        OnEventView::new(ScrollView::new(editor).scroll_x(true).scroll_y(true))
            .on_pre_event_inner(EventTrigger::mouse(), editor_callback),
    ));
    siv.add_fullscreen_layer(editor_layer);

    siv.run();

    Ok(())
}

fn set_tool<T, S>(set: S) -> impl Fn(&mut Cursive) + 'static
where
    T: Tool + Default + 'static,
    S: Fn(&mut Options) + 'static,
{
    move |siv| {
        let tool = {
            let mut view = get_editor_view(siv);
            let editor = view.get_inner_mut().get_inner_mut();
            set(editor.opts());
            editor.set_tool(T::default());
            editor.active_tool()
        };

        let menu = siv.menubar();
        let idx = menu.len() - 1;
        menu.remove(idx);
        menu.insert_leaf(idx, tool, |_| ());
    }
}

fn get_editor_view(siv: &mut Cursive) -> ViewRef<OnEventView<ScrollView<Editor>>> {
    siv.find_id::<OnEventView<ScrollView<Editor>>>("editor")
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

fn editor_callback(scroll_view: &mut ScrollView<Editor>, event: &Event) -> Option<EventResult> {
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
                .unwrap_or(false) => {}

        WheelUp | WheelDown => return None,

        _ => {}
    }

    let viewport = scroll_view.content_viewport();

    let content_pos = pos.saturating_sub(offset) + viewport.top_left();

    // TODO: should be able to remove all this indirection now that the view gives us a
    // reference to the editor
    //
    // also get_editor_view isn't necessary, so neither is the IdView
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
            let mut view = get_editor_view(siv);
            let editor = view.get_inner_mut().get_inner_mut();
            editor.press(content_pos);
        }

        Hold(Left) => {
            let mut view = get_editor_view(siv);
            let editor = view.get_inner_mut().get_inner_mut();
            editor.hold(content_pos);
        }

        Release(Left) => {
            let mut view = get_editor_view(siv);
            let editor = view.get_inner_mut().get_inner_mut();
            editor.release(content_pos);
        }

        _ => {}
    })
}
