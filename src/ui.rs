// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::{
    editor::{Editor, EditorView},
    EDITOR_ID,
};
use cursive::{
    align::HAlign,
    view::{Margins, Nameable},
    views::{Dialog, EditView, ScrollView, TextView},
    Cursive,
};
use std::rc::Rc;

const NO_MARGIN: Margins = Margins {
    left: 0,
    right: 0,
    top: 0,
    bottom: 0,
};

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
    siv.find_name::<ScrollView<EditorView>>(EDITOR_ID)
        .map(|mut view| f(&mut view.get_inner_mut().write()))
        .unwrap()
}

/// Run `f` with an immutable reference to the editor, returning its result. Shorthand for
/// looking up the view any time it's needed.
pub(super) fn with_editor<T, F>(siv: &mut Cursive, f: F) -> T
where
    F: FnOnce(&Editor) -> T,
{
    siv.find_name::<ScrollView<EditorView>>(EDITOR_ID)
        .map(|view| f(&view.get_inner().read()))
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
    if siv.find_name::<Dialog>(POPUP_ID).is_some() {
        return;
    }

    let popup = Dialog::text(content)
        .title(title)
        .padding(NO_MARGIN)
        .h_align(HAlign::Center)
        .dismiss_button("No")
        .button("Yes", move |siv| {
            siv.pop_layer();
            yes(siv);
        })
        .with_name(POPUP_ID);

    siv.add_layer(popup);
}

/// Display a single line input form, passing the submitted content into the provided
/// callback `form`.
pub(super) fn display_form<T, F: 'static>(siv: &mut Cursive, title: T, form: F)
where
    T: Into<String>,
    F: Fn(&mut Cursive, &'static str, &str),
{
    if siv.find_name::<Dialog>(POPUP_ID).is_some() {
        return;
    }

    const INPUT_ID: &str = "generic_input";

    let submit = Rc::new(move |siv: &mut Cursive, input: &str| {
        form(siv, POPUP_ID, input);
    });

    let submit_ok = Rc::clone(&submit);

    let input = EditView::new()
        .on_submit(move |siv, input| submit(siv, input))
        .with_name(INPUT_ID);

    let popup = Dialog::around(input)
        .title(title)
        .button("Ok", move |siv| {
            let input = siv
                .call_on_name(INPUT_ID, |view: &mut EditView| view.get_content())
                .unwrap();
            submit_ok(siv, &input);
        })
        .dismiss_button("Cancel")
        .with_name(POPUP_ID);

    siv.add_layer(popup);
}

/// Display a notification dialog.
pub(super) fn notify<T, C>(siv: &mut Cursive, title: T, content: C)
where
    T: Into<String>,
    C: Into<String>,
{
    let content = ScrollView::new(TextView::new(content))
        .scroll_x(false)
        .scroll_y(true);

    siv.add_layer(
        Dialog::around(content)
            .title(title)
            .dismiss_button("Ok")
            .h_align(HAlign::Center)
            .padding(NO_MARGIN),
    );
}

/// Display a unique notification dialog. No two dialogs with the same `unique_id` will
/// ever be shown at the same time.
pub(super) fn notify_unique<T, C>(siv: &mut Cursive, unique_id: &'static str, title: T, content: C)
where
    T: Into<String>,
    C: Into<String>,
{
    if siv.find_name::<Dialog>(unique_id).is_some() {
        return;
    }

    let content = ScrollView::new(TextView::new(content))
        .scroll_x(false)
        .scroll_y(true);

    siv.add_layer(
        Dialog::around(content)
            .title(title)
            .dismiss_button("Ok")
            .h_align(HAlign::Center)
            .padding(NO_MARGIN)
            .with_name(unique_id),
    );
}
