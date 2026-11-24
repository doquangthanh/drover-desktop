//! Drawing the editor: only the visible rows (uniform_list), each with its gutter (git mark, line number) and the
//! line as styled text (syntax colours, selection, the input method's word underlined) and the caret. A click puts
//! the caret, a drag selects, a double click selects a word; Shift+wheel or a long caret line scroll sideways.

use gpui::{AnyElement, Context, ElementInputHandler, HighlightStyle, MouseButton, MouseDownEvent, MouseMoveEvent, ScrollStrategy,
    ScrollWheelEvent, StyledText, UnderlineStyle, Window, canvas, div, font, prelude::*, rgb, uniform_list};

use super::display::{Shown, merge};
use super::marks::Mark;
use super::syntax::{Tok, highlight};
use super::CodeEditor;
use crate::theme::{self, accent, dim, done, error, fg, muted, working, yellow};
use crate::ui_scale::px;

const LINE_H: f32 = 20.;
const FONT: f32 = 12.5;

fn tok_color(t: Tok) -> u32 {
    match t { Tok::Comment => dim(), Tok::Str => done(), Tok::Num => yellow(), Tok::Key => accent() }
}

impl CodeEditor {
    fn gutter_w(&self, lines: usize) -> f32 { lines.to_string().len().max(3) as f32 * self.cell_w + 22. }

    /// Caret at the char nearest window x on `line`; `extend` keeps the selection's other end.
    fn place(&mut self, line: usize, x: f32, extend: bool) {
        let (left, _) = *self.text_box.lock().unwrap();
        let drawn = ((x - left + self.hscroll) / self.cell_w).max(0.).round() as usize;
        let start = self.buf.pos(line, 0);
        let text = &self.buf.text[start..super::buffer::line_bounds(&self.buf.text, start).1];
        let col = Shown::source_col(text, drawn);
        let at = self.buf.pos(line, col);
        self.buf.move_to(at, extend);
    }

    fn on_down(&mut self, line: usize, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        self.place(line, f32::from(ev.position.x), ev.modifiers.shift);
        if ev.click_count == 2 {
            let w = self.buf.word_at(self.buf.cursor);
            self.buf.move_to(w.start, false);
            self.buf.move_to(w.end, true);
        }
        self.dragging = true;
        cx.stop_propagation();
        cx.notify();
    }

    fn on_move(&mut self, line: usize, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.dragging || ev.pressed_button != Some(MouseButton::Left) { self.dragging = false; return }
        self.place(line, f32::from(ev.position.x), true);
        cx.notify();
    }

    fn on_wheel(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let d = ev.delta.pixel_delta(px(LINE_H));
        let dx = if ev.modifiers.shift { f32::from(d.y) } else { f32::from(d.x) };
        if dx != 0. {
            self.hscroll = (self.hscroll - dx).max(0.);
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// One row: gutter and the drawn line.
    fn row(&mut self, i: usize, start: usize, end: usize, focused: bool, cx: &mut Context<Self>) -> AnyElement {
        let line = &self.buf.text[start..end];
        let mut shown = Shown::new(line);
        let mut runs: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
        if let Some(lang) = &self.lang {
            for (r, t) in highlight(lang, line) {
                runs.push((shown.range(&r), HighlightStyle { color: Some(rgb(tok_color(t)).into()), ..Default::default() }));
            }
        }
        if let Some(sel) = self.buf.selection().filter(|s| s.start <= end && s.end >= start) {
            let r = shown.range(&(sel.start.max(start) - start..sel.end.min(end) - start));
            runs.push((r, HighlightStyle { background_color: Some(theme::tint(accent(), 0.30).into()), ..Default::default() }));
        }
        let caret_here = (start..=end).contains(&self.buf.cursor);
        let mut caret_col = caret_here.then(|| shown.col(self.buf.cursor - start));
        if let (true, Some(m)) = (caret_here, self.marked.clone()) {
            let at = shown.at(self.buf.cursor - start);
            shown.insert(at, &m);
            for (r, _) in runs.iter_mut() {
                if r.start >= at { r.start += m.len(); }
                if r.end > at || (r.end == at && r.start > at) { r.end += m.len(); }
            }
            let underline = UnderlineStyle { thickness: px(1.), color: Some(rgb(fg()).into()), wavy: false };
            runs.push((at..at + m.len(), HighlightStyle { underline: Some(underline), ..Default::default() }));
            caret_col = caret_col.map(|c| c + m.chars().count());
        }
        let runs = merge(&runs, |a, b| *a = a.highlight(*b), HighlightStyle::default());
        let n = self.buf.line_count();
        let mark = self.marks.get(&i).map(|m| match m { Mark::Added => done(), Mark::Modified => working(), Mark::Deleted => error() });
        let (cell, hs) = (self.cell_w, self.hscroll);
        let gutter = div().w(px(self.gutter_w(n) / crate::ui_scale::zoom())).flex_none().h_full().flex().items_center().pr(px(10.))
            .child(div().w(px(3.)).h(px(if matches!(self.marks.get(&i), Some(Mark::Deleted)) { 5. } else { LINE_H })).flex_none()
                .when_some(mark, |d, c| d.bg(rgb(c))))
            .child(div().flex_1().flex().justify_end().text_color(rgb(if caret_here { muted() } else { dim() })).child((i + 1).to_string()));
        let text = div().relative().flex_1().min_w_0().h_full().overflow_hidden()
            .child(div().absolute().top_0().h_full().flex().items_center().whitespace_nowrap().left(gpui::px(-hs))
                .child(StyledText::new(shown.text).with_highlights(runs)))
            .when(caret_col.is_some() && focused, |d| d.child(div().absolute().top(px(2.)).w(px(1.5)).h(px(LINE_H - 4.)).bg(rgb(fg()))
                .left(gpui::px(caret_col.unwrap_or(0) as f32 * cell - hs))));
        div().h(px(LINE_H)).w_full().flex().text_color(rgb(fg()))
            .when(caret_here && focused && self.buf.selection().is_none(), |d| d.bg(theme::ink(0.04)))
            .child(gutter).child(text)
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev, window, cx| this.on_down(i, ev, window, cx)))
            .on_mouse_move(cx.listener(move |this, ev, _, cx| this.on_move(i, ev, cx)))
            .into_any_element()
    }

    /// Scroll so the caret's line and column are in view (after a key, a goto or a reload).
    fn follow_caret(&mut self) {
        let (line, _) = self.buf.line_col(self.buf.cursor);
        self.scroll.scroll_to_item(line, ScrollStrategy::Center);
        let start = super::buffer::line_bounds(&self.buf.text, self.buf.cursor).0;
        let x = Shown::new(&self.buf.text[start..self.buf.cursor]).text.chars().count() as f32 * self.cell_w;
        let width = self.text_box.lock().unwrap().1;
        if x < self.hscroll { self.hscroll = (x - 4. * self.cell_w).max(0.); }
        else if width > 0. && x > self.hscroll + width - 2. * self.cell_w { self.hscroll = x - width + 8. * self.cell_w; }
    }
}

impl Render for CodeEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ts = window.text_system();
        self.cell_w = ts.advance(ts.resolve_font(&font(self.family.clone())), px(FONT), 'm').map(|s| f32::from(s.width)).unwrap_or(FONT * 0.6);
        if std::mem::take(&mut self.follow) { self.follow_caret(); }
        let focused = self.focus.is_focused(window);
        // Line starts once per frame (the rows look up their text by index).
        let mut starts = vec![0usize];
        starts.extend(self.buf.text.match_indices('\n').map(|(i, _)| i + 1));
        let text_len = self.buf.text.len();
        let count = starts.len();
        let list = uniform_list("code-lines", count, cx.processor(move |this: &mut CodeEditor, range: std::ops::Range<usize>, _, cx| {
            range.filter(|i| *i < starts.len()).map(|i| {
                let end = starts.get(i + 1).map(|s| s - 1).unwrap_or(text_len);
                this.row(i, starts[i], end.min(this.buf.text.len()), focused, cx)
            }).collect()
        })).track_scroll(self.scroll.clone()).size_full();
        let (entity, focus, gutter, text_box) = (cx.entity(), self.focus.clone(), self.gutter_w(count), self.text_box.clone());
        div().relative().flex_1().min_h_0().min_w_0().overflow_hidden().font_family(self.family.clone()).text_size(px(FONT))
            .bg(rgb(theme::pane())).cursor_text()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_scroll_wheel(cx.listener(Self::on_wheel))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| this.dragging = false))
            .child(canvas(move |bounds, _, _| {
                *text_box.lock().unwrap() = (f32::from(bounds.origin.x) + gutter, f32::from(bounds.size.width) - gutter);
            }, move |bounds, _, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
            }).absolute().top_0().left_0().size_full())
            .child(list)
    }
}
