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
    event::{Event, EventResult, EventTrigger, Key, MouseButton, MouseEvent},
    logger,
    menu::MenuTree,
    view::scroll::Scroller,
    views::{Dialog, IdView, OnEventView, Panel, ScrollView, ViewRef},
    Cursive, Vec2,
};
use lazy_static::lazy_static;
use log::info;
use parking_lot::Mutex;
use std::{env, error::Error};
use structopt::StructOpt;

type MainResult<T> = Result<T, Box<dyn Error>>;

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
            scroll_view.get_inner_mut().press(content_pos);
            Some(EventResult::Consumed(None))
        }

        Hold(Left) => {
            scroll_view.get_inner_mut().hold(content_pos);

            let pos = pos - offset;
            let bounds = viewport.bottom_right() - viewport.top_left();
            let editor = scroll_view.get_inner_mut();

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

            Some(EventResult::Consumed(None))
        }

        Release(Left) => {
            scroll_view.get_inner_mut().release(content_pos);
            Some(EventResult::Consumed(None))
        }

        Press(Right) => {
            *RPOINTER.lock() = Some(pos);
            Some(EventResult::Consumed(None))
        }

        Hold(Right) if RPOINTER.lock().is_none() => {
            *RPOINTER.lock() = Some(pos);
            Some(EventResult::Consumed(None))
        }

        Hold(Right) => {
            let Vec2 { x, y } = RPOINTER.lock().replace(pos).unwrap();

            let mut offset = viewport.top_left();

            if pos.x > x {
                offset.x = offset.x.saturating_sub(pos.x - x);
            } else if pos.x < x {
                offset.x = offset.x.saturating_add(x - pos.x);

                if within(1, viewport.right(), scroll_view.inner_size().x) {
                    scroll_view.get_inner_mut().bounds().x += x - pos.x;
                }
            }

            if pos.y > y {
                offset.y = offset.y.saturating_sub(pos.y - y);
            } else if pos.y < y {
                offset.y = offset.y.saturating_add(y - pos.y);

                if within(1, viewport.bottom(), scroll_view.inner_size().y) {
                    scroll_view.get_inner_mut().bounds().y += y - pos.y;
                }
            }

            scroll_view.set_offset(offset);
            Some(EventResult::Consumed(None))
        }

        Release(Right) => {
            *RPOINTER.lock() = None;
            Some(EventResult::Consumed(None))
        }

        _ => None,
    }
}

fn within(w: usize, x: usize, y: usize) -> bool {
    ((x as isize - y as isize).abs() as usize) <= w
}
