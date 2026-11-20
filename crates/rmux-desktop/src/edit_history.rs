use crate::composition::Composition;
use std::collections::VecDeque;

type State = (Composition, bool);
#[derive(Default)]
pub struct EditHistory {
    undo: VecDeque<State>,
    redo: Vec<State>,
}
impl EditHistory {
    pub fn record(&mut self, document: &Composition, reversed: bool) {
        self.redo.clear();
        if self.undo.len() == 32 {
            self.undo.pop_front();
        }
        self.undo.push_back((document.clone(), reversed));
    }
    pub fn restore(&mut self, document: &mut Composition, reversed: &mut bool, redo: bool) -> bool {
        let state = if redo {
            self.redo.pop()
        } else {
            self.undo.pop_back()
        };
        let Some((previous, direction)) = state else {
            return false;
        };
        let current = (
            std::mem::replace(document, previous),
            std::mem::replace(reversed, direction),
        );
        if redo {
            self.undo.push_back(current);
        } else {
            self.redo.push(current);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_selection_branching_and_history_bound() {
        let mut history = EditHistory::default();
        let mut text = Composition::default();
        text.insert(None, "a👩🏽‍💻z");
        text.selected = 1..8;
        let mut reversed = true;
        history.record(&text, reversed);
        text.insert(Some(1..8), "new");
        assert!(history.restore(&mut text, &mut reversed, false));
        assert_eq!(text.text, "a👩🏽‍💻z");
        assert_eq!(text.selected, 1..8);
        assert!(reversed);
        assert!(history.restore(&mut text, &mut reversed, true));
        assert_eq!(text.text, "anewz");
        history.restore(&mut text, &mut reversed, false);
        history.record(&text, reversed);
        text.insert(Some(0..0), "branch");
        assert!(!history.restore(&mut text, &mut reversed, true));
        for _ in 0..40 {
            history.record(&text, false);
            text.insert(Some(0..0), "x");
        }
        let mut count = 0;
        while history.restore(&mut text, &mut reversed, false) {
            count += 1;
        }
        assert_eq!(count, 32);
    }
}
