use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn move_to(&mut self, offset: usize, extend: bool) {
        self.head = offset;
        if !extend {
            self.anchor = offset;
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Buffer {
    pub text: String,
    pub committed: String,
    pub selection: Selection,
    pub marked: Option<Range<usize>>,
    pub digits_only: bool,
}

impl Buffer {
    pub fn new(text: String, digits_only: bool) -> Self {
        let end = text.len();
        Self {
            committed: text.clone(),
            text,
            selection: Selection {
                anchor: end,
                head: end,
            },
            marked: None,
            digits_only,
        }
    }

    pub fn set_text(&mut self, text: String) {
        *self = Self::new(text, self.digits_only);
    }

    pub fn range_from_utf16(&self, range: Range<usize>) -> Range<usize> {
        utf16_range(&self.text, range)
    }

    pub fn range_to_utf16(&self, range: Range<usize>) -> Range<usize> {
        to_utf16(&self.text, range.start)..to_utf16(&self.text, range.end)
    }

    pub fn replace(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        marked: bool,
        selection: Option<Range<usize>>,
    ) {
        let range = range
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection.range());
        let text: String = text
            .chars()
            .filter_map(|ch| match ch {
                '\r' | '\n' | '\t' => Some(' '),
                ch if ch.is_control() => None,
                ch => Some(ch),
            })
            .collect();
        let mut next = self.text.clone();
        next.replace_range(range.clone(), &text);
        if !marked && self.digits_only && !next.chars().all(|ch| ch.is_ascii_digit()) {
            // Reject an invalid commit without leaving its preedit in the field.
            self.set_text(self.committed.clone());
            return;
        }
        self.text = next;
        self.marked = (marked && !text.is_empty()).then_some(range.start..range.start + text.len());
        let selected = selection.map(|range| utf16_range(&text, range));
        self.selection = match selected {
            Some(selected) => Selection {
                anchor: range.start + selected.start,
                head: range.start + selected.end,
            },
            None => Selection {
                anchor: range.start + text.len(),
                head: range.start + text.len(),
            },
        };
        if !marked || text.is_empty() {
            self.commit();
        }
    }

    pub fn commit(&mut self) {
        self.marked = None;
        if self.digits_only && !self.text.chars().all(|ch| ch.is_ascii_digit()) {
            self.set_text(self.committed.clone());
        } else {
            self.committed = self.text.clone();
        }
    }

    pub fn previous_grapheme(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    pub fn next_grapheme(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.text.len())
    }

    pub fn previous_word(&self, offset: usize) -> usize {
        self.text
            .unicode_word_indices()
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    pub fn next_word(&self, offset: usize) -> usize {
        self.text
            .unicode_word_indices()
            .find_map(|(index, word)| (index + word.len() > offset).then_some(index + word.len()))
            .unwrap_or(self.text.len())
    }

    pub fn word_at(&self, offset: usize) -> Range<usize> {
        self.text
            .split_word_bound_indices()
            .find_map(|(index, part)| {
                (offset >= index && offset < index + part.len())
                    .then_some(index..index + part.len())
            })
            .unwrap_or(self.text.len()..self.text.len())
    }

    pub fn delete(&mut self, forward: bool, word: bool) {
        if self.selection.range().is_empty() {
            let head = self.selection.head;
            let end = match (forward, word) {
                (true, true) => self.next_word(head),
                (false, true) => self.previous_word(head),
                (true, false) => self.next_grapheme(head),
                (false, false) => self.previous_grapheme(head),
            };
            self.selection.move_to(end, true);
        }
        self.replace(None, "", false, None);
    }
}

pub(super) fn to_utf16(text: &str, offset: usize) -> usize {
    text.char_indices()
        .take_while(|(index, _)| *index < offset)
        .map(|(_, ch)| ch.len_utf16())
        .sum()
}

fn from_utf16(text: &str, offset: usize, ceil: bool) -> usize {
    let mut units = 0;
    for (index, ch) in text.char_indices() {
        if units == offset {
            return index;
        }
        if units + ch.len_utf16() > offset {
            return if ceil { index + ch.len_utf8() } else { index };
        }
        units += ch.len_utf16();
    }
    text.len()
}

pub(super) fn utf16_range(text: &str, range: Range<usize>) -> Range<usize> {
    let start = from_utf16(text, range.start, false);
    let end = if range.is_empty() {
        start
    } else {
        from_utf16(text, range.end.max(range.start), true)
    };
    start..end
}
