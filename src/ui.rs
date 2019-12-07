// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::{editor::Editor, EDITOR_ID};
use cursive::{
    align::HAlign,
    view::Identifiable,
    views::{Dialog, EditView, ScrollView},
    Cursive,
};
use std::rc::Rc;

/// Run `f` if the editor's buffer has not been modified since the last save, or if user
/// has confirmed that they're ok with discarding unsaved changes.
pub(super) fn with_checked_editor<T, F: 'static>(siv: &mut Cursive, title: T, f: F)
where
    T: Into<String>,
    F: Fn(&mut Cursive),
{
    if with_editor(siv, Editor::is_dirty) {
        display_yesno(siv, title, "Discard unsaved changes?", f);
    } else {
        f(siv);
    }
}

/// Run `f` with a mutable reference to the editor, returning its result. Shorthand for
/// looking up the view any time it's needed.
pub(super) fn with_editor_mut<T, F>(siv: &mut Cursive, f: F) -> T
where
    F: FnOnce(&mut Editor) -> T,
{
    siv.find_id::<ScrollView<Editor>>(EDITOR_ID)
        .map(|mut view| f(view.get_inner_mut()))
        .unwrap()
}

/// Run `f` with an immutable reference to the editor, returning its result. Shorthand for
/// looking up the view any time it's needed.
pub(super) fn with_editor<T, F>(siv: &mut Cursive, f: F) -> T
where
    F: FnOnce(&Editor) -> T,
{
    siv.find_id::<ScrollView<Editor>>(EDITOR_ID)
        .map(|view| f(view.get_inner()))
        .unwrap()
}

const POPUP_ID: &str = "generic_popup";

/// Display a "Yes / No" prompt with the provided `title`, running `yes` iff "Yes" is
/// pressed. Defaults to "No".
pub(super) fn display_yesno<T, C, F: 'static>(siv: &mut Cursive, title: T, content: C, yes: F)
where
    T: Into<String>,
    C: Into<String>,
    F: Fn(&mut Cursive),
{
    if siv.find_id::<Dialog>(POPUP_ID).is_some() {
        return;
    }

    let popup = Dialog::text(content)
        .title(title)
        .padding(((0, 0), (0, 0)))
        .h_align(HAlign::Center)
        .dismiss_button("No")
        .button("Yes", move |siv| {
            siv.pop_layer();
            yes(siv);
        })
        .with_id(POPUP_ID);

    siv.add_layer(popup);
}

/// Display a single line input form, passing the submitted content into the provided
/// callback `form`.
pub(super) fn display_form<T, F: 'static>(siv: &mut Cursive, title: T, form: F)
where
    T: Into<String>,
    F: Fn(&mut Cursive, &'static str, &str),
{
    if siv.find_id::<Dialog>(POPUP_ID).is_some() {
        return;
    }

    const INPUT_ID: &str = "generic_input";

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

/// Display a notification dialog.
pub(super) fn notify<T, C>(siv: &mut Cursive, title: T, content: C)
where
    T: Into<String>,
    C: Into<String>,
{
    siv.add_layer(
        Dialog::info(content)
            .title(title)
            .h_align(HAlign::Center)
            .padding(((0, 0), (0, 0))),
    );
}

/// Display a unique notification dialog. No two dialogs with the same `unique_id` will
/// ever be shown at the same time.
pub(super) fn notify_unique<T, C>(siv: &mut Cursive, unique_id: &'static str, title: T, content: C)
where
    T: Into<String>,
    C: Into<String>,
{
    if siv.find_id::<Dialog>(unique_id).is_some() {
        return;
    }

    siv.add_layer(
        Dialog::info(content)
            .title(title)
            .h_align(HAlign::Center)
            .padding(((0, 0), (0, 0)))
            .with_id(unique_id),
    );
}
