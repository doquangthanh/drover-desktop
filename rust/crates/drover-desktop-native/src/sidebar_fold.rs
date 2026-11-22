//! Folding sidebar groups (a machine's spaces, a space's agents): a click on a heading folds or opens it. Once a
//! section has many rows, its groups fold by themselves, except the one holding the current space / focused agent and
//! any with an agent that needs you. A click sticks until the app restarts.

use gpui::Context;

use crate::workspace_app::DroverApp;

/// Above this many spaces (all machines), space groups fold by themselves.
pub const AUTO_FOLD_SPACES: usize = 8;
/// Above this many agents, agent groups fold by themselves.
pub const AUTO_FOLD_AGENTS: usize = 10;

/// Whether a group is folded: the user's click if any, else folded when the section is long and nothing keeps it open.
pub fn folded(choice: Option<bool>, rows_in_section: usize, limit: usize, keep_open: bool) -> bool {
    choice.unwrap_or(rows_in_section > limit && !keep_open)
}

impl DroverApp {
    pub fn is_folded(&self, key: &str, rows_in_section: usize, limit: usize, keep_open: bool) -> bool {
        folded(self.folds.get(key).copied(), rows_in_section, limit, keep_open)
    }

    /// Flip a group from what it shows now.
    pub fn toggle_fold(&mut self, key: String, now: bool, cx: &mut Context<Self>) {
        self.folds.insert(key, !now);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::folded;

    #[test]
    fn folds_only_long_sections_and_never_the_current_group() {
        assert!(!folded(None, 5, 8, false));
        assert!(folded(None, 9, 8, false));
        assert!(!folded(None, 9, 8, true));
    }

    #[test]
    fn a_click_wins_over_the_automatic_choice() {
        assert!(!folded(Some(false), 50, 8, false));
        assert!(folded(Some(true), 1, 8, true));
    }
}
