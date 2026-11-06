pub const MAX_TEXT: usize = 4096;

#[derive(Default)]
pub struct Editor {
    pub text: String,
    pub cursor: usize,
    pub anchor: usize,
    /// Horizontal scroll in logical pixels.
    pub scroll: f64,
}

impl Editor {
    pub fn set_text(&mut self, text: &str) {
        self.text = text.chars().take(MAX_TEXT).collect();
        self.cursor = self.text.len();
        self.anchor = self.cursor;
        self.scroll = 0.0;
    }

    fn clamp(&self, mut i: usize) -> usize {
        i = i.min(self.text.len());
        while i > 0 && !self.text.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    pub fn prev(&self, i: usize) -> usize {
        self.text[..self.clamp(i)]
            .char_indices()
            .next_back()
            .map_or(0, |(j, _)| j)
    }

    pub fn next(&self, i: usize) -> usize {
        let i = self.clamp(i);
        self.text[i..]
            .chars()
            .next()
            .map_or(self.text.len(), |c| i + c.len_utf8())
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        if self.anchor == self.cursor {
            None
        } else if self.anchor < self.cursor {
            Some((self.anchor, self.cursor))
        } else {
            Some((self.cursor, self.anchor))
        }
    }

    fn delete_selection(&mut self) -> bool {
        if let Some((a, b)) = self.selection() {
            self.text.replace_range(a..b, "");
            self.cursor = a;
            self.anchor = a;
            true
        } else {
            false
        }
    }

    pub fn insert(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.delete_selection();
        // Single-line field: newlines become spaces.
        let clean: String = s
            .chars()
            .map(|c| {
                if c == '\n' || c == '\r' || c == '\t' {
                    ' '
                } else {
                    c
                }
            })
            .filter(|c| !c.is_control())
            .collect();
        if clean.is_empty() {
            return;
        }
        let room = MAX_TEXT.saturating_sub(self.text.len());
        if room == 0 {
            return;
        }
        let mut take = 0;
        for (i, c) in clean.char_indices() {
            let next = i + c.len_utf8();
            if next > room {
                break;
            }
            take = next;
        }
        if take == 0 {
            return;
        }
        self.text.insert_str(self.cursor, &clean[..take]);
        self.cursor += take;
        self.anchor = self.cursor;
    }

    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let p = self.prev(self.cursor);
        self.text.replace_range(p..self.cursor, "");
        self.cursor = p;
        self.anchor = p;
    }

    pub fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        if self.cursor >= self.text.len() {
            return;
        }
        let n = self.next(self.cursor);
        self.text.replace_range(self.cursor..n, "");
    }

    pub fn left(&mut self, extend: bool) {
        if !extend {
            if let Some((a, _)) = self.selection() {
                self.cursor = a;
                self.anchor = a;
                return;
            }
        }
        self.cursor = self.prev(self.cursor);
        if !extend {
            self.anchor = self.cursor;
        }
    }

    pub fn right(&mut self, extend: bool) {
        if !extend {
            if let Some((_, b)) = self.selection() {
                self.cursor = b;
                self.anchor = b;
                return;
            }
        }
        self.cursor = self.next(self.cursor);
        if !extend {
            self.anchor = self.cursor;
        }
    }

    pub fn home(&mut self, extend: bool) {
        self.cursor = 0;
        if !extend {
            self.anchor = 0;
        }
    }

    pub fn end(&mut self, extend: bool) {
        self.cursor = self.text.len();
        if !extend {
            self.anchor = self.cursor;
        }
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|(a, b)| self.text[a..b].to_string())
    }

    pub fn copy(&self) -> String {
        self.selected_text().unwrap_or_else(|| self.text.clone())
    }

    pub fn cut(&mut self) -> String {
        let out = self.copy();
        if self.selection().is_some() {
            self.delete_selection();
        } else {
            self.text.clear();
            self.cursor = 0;
            self.anchor = 0;
        }
        out
    }
}
