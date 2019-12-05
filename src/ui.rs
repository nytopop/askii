// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::{editor::Editor, EDITOR_ID};
use cursive::{
    align::HAlign,
    traits::Identifiable,
    views::{Dialog, EditView, OnEventView, ScrollView},
    Cursive,
};
use std::rc::Rc;

/// Run `f` if the editor's buffer has not been modified since the last save, or if user
/// has confirmed that they're ok with discarding unsaved changes.
pub fn with_clean_editor<F: Fn(&mut Cursive) + 'static>(siv: &mut Cursive, f: F) {
    let dirty = {
        let mut view = siv
            .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
            .unwrap();
        get_editor(view.get_inner_mut()).is_dirty()
    };

    if dirty {
        display_yesno(siv, "Discard unsaved changes?", f);
    } else {
        f(siv);
    }
}

/// Run `f` with a mutable reference to the editor, returning its result. Shorthand for
/// looking up the view any time it's needed.
pub fn with_editor<T, F: FnOnce(&mut Editor) -> T>(siv: &mut Cursive, f: F) -> T {
    let mut view = siv
        .find_id::<OnEventView<ScrollView<Editor>>>(EDITOR_ID)
        .unwrap();
    f(get_editor(view.get_inner_mut()))
}

/// Get a mutable reference to the editor, given a containing `&mut ScrollView`.
pub fn get_editor(scroll_view: &mut ScrollView<Editor>) -> &mut Editor {
    scroll_view.get_inner_mut()
}

/// Display a "Yes / No" prompt with the provided `title`, running `yes` iff "Yes" is
/// pressed. Defaults to "No".
pub fn display_yesno<T: Into<String>, F: 'static>(siv: &mut Cursive, title: T, yes: F)
where
    F: Fn(&mut Cursive),
{
    let popup = Dialog::new()
        .title(title)
        .padding(((0, 0), (0, 0)))
        .h_align(HAlign::Center)
        .dismiss_button("No")
        .button("Yes", move |siv| {
            siv.pop_layer();
            yes(siv);
        });

    siv.add_layer(popup);
}

/// Display a single line input form, passing the submitted content into the provided
/// callback `form`.
pub fn display_form<T: Into<String>, F: 'static>(siv: &mut Cursive, title: T, form: F)
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

/// Display a notification dialog.
pub fn notify<T: Into<String>, C: Into<String>>(siv: &mut Cursive, title: T, content: C) {
    siv.add_layer(
        Dialog::info(content)
            .title(title)
            .h_align(HAlign::Center)
            .padding(((0, 0), (0, 0))),
    );
}

/// Display a unique notification dialog. No two dialogs with the same `unique_id` will
/// ever be shown at the same time.
pub fn notify_uniq<T: Into<String>, C: Into<String>>(
    siv: &mut Cursive,
    unique_id: &'static str,
    title: T,
    content: C,
) {
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
