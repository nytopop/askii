// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use super::editor::EditorView;
use cursive::{theme::ColorStyle, view::View, Printer, Vec2};

pub(crate) struct ModeLine {
    editor: EditorView,
}

impl View for ModeLine {
    fn draw(&self, p: &Printer<'_, '_>) {
        let at = |x: usize| Vec2::new(x, 0);

        let editor = self.editor.read();

        let path = editor
            .path()
            .map(|p| p.to_str().unwrap())
            .unwrap_or("*scratch buffer*");

        if editor.is_dirty() {
            p.with_color(ColorStyle::title_primary(), |p| p.print(at(1), &path));
        } else {
            p.with_color(ColorStyle::primary(), |p| p.print(at(1), &path));
        }

        let tool = editor.active_tool();
        p.print(at(p.size.x.saturating_sub(tool.len() + 1)), &tool);
    }

    fn required_size(&mut self, size: Vec2) -> Vec2 {
        size.map_y(|_| 1)
    }
}

impl ModeLine {
    pub(crate) fn new(editor: EditorView) -> Self {
        Self { editor }
    }
}
