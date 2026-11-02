use super::{geometry, props_text};
use crate::canvas::EditorCanvas;
use gpui::{ClipboardItem, Context, KeyDownEvent};

fn prev_char(text: &str, at: usize) -> usize {
    text[..at].char_indices().next_back().map_or(0, |(i, _)| i)
}

fn next_char(text: &str, at: usize) -> usize {
    text[at..].chars().next().map_or(at, |c| at + c.len_utf8())
}

impl EditorCanvas {
    /// False for Ctrl+S, left to the editor, and plain characters, which arrive as text input.
    pub(in crate::canvas) fn text_key_down(
        &mut self,
        e: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        self.sync_text_edit(cx);
        let (Some(text), Some(edit)) = (self.editing_text(cx), self.text_edit.as_ref()) else {
            return false;
        };
        let m = e.keystroke.modifiers;
        let (sel, cursor) = (edit.selection.clone(), edit.cursor());
        let key = e.keystroke.key.as_str();
        if m.control && !m.alt {
            match key {
                "s" => return false,
                "a" => self.select_range(0..text.len(), cx),
                "c" | "x" if !sel.is_empty() => {
                    cx.write_to_clipboard(ClipboardItem::new_string(text[sel.clone()].to_owned()));
                    if key == "x" {
                        self.replace_text(sel, "", false, None, cx);
                    }
                }
                "v" => {
                    if let Some(pasted) = cx.read_from_clipboard().and_then(|i| i.text()) {
                        self.replace_text(sel, &pasted, false, None, cx);
                    }
                }
                _ => {}
            }
            return true;
        }
        let shift = m.shift;
        match key {
            "escape" => self.commit_text(cx),
            "enter" => self.replace_text(sel, "\n", false, None, cx),
            "backspace" if sel.is_empty() => {
                self.replace_text(prev_char(&text, cursor)..cursor, "", false, None, cx);
            }
            "delete" if sel.is_empty() => {
                self.replace_text(cursor..next_char(&text, cursor), "", false, None, cx);
            }
            "backspace" | "delete" => self.replace_text(sel, "", false, None, cx),
            "left" if !shift && !sel.is_empty() => self.move_caret(sel.start, false, cx),
            "right" if !shift && !sel.is_empty() => self.move_caret(sel.end, false, cx),
            "left" => self.move_caret(prev_char(&text, cursor), shift, cx),
            "right" => self.move_caret(next_char(&text, cursor), shift, cx),
            "up" | "down" | "home" | "end" => {
                let to = self.line_target(key, cursor, &text, cx);
                self.move_caret(to, shift, cx);
            }
            _ => return m.alt || m.platform || m.function,
        }
        true
    }

    fn line_target(&self, key: &str, cursor: usize, text: &str, cx: &Context<Self>) -> usize {
        let Some((shape, layout)) = self.editing_layout(cx) else {
            return cursor;
        };
        let Some(line) = geometry::line_of(&layout, cursor) else {
            return 0;
        };
        let lb = &layout.lines[line];
        match key {
            "home" => lb.range.start,
            "end" => {
                let end = lb.range.end.min(text.len());
                let newline = props_text(&shape).is_some_and(|t| t[..end].ends_with('\n'));
                if newline && end > lb.range.start {
                    end - 1
                } else {
                    end
                }
            }
            _ => {
                let x = layout.caret_x(cursor).1;
                let to = if key == "up" {
                    line.checked_sub(1)
                } else {
                    Some(line + 1).filter(|l| *l < layout.lines.len())
                };
                match to {
                    Some(l) => {
                        let t = &layout.lines[l];
                        layout.hit_test(x, t.top + t.height / 2.0)
                    }
                    None if key == "up" => 0,
                    None => text.len(),
                }
            }
        }
    }
}
