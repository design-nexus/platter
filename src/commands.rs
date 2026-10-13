use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

#[derive(Clone, Copy)]
pub struct Action {
    pub label: &'static str,
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}
impl Action {
    pub const fn key(label: &'static str, key: char) -> Self {
        Self {
            label,
            key: KeyCode::Char(key),
            modifiers: KeyModifiers::NONE,
        }
    }
    #[allow(dead_code)]
    pub const fn ctrl(label: &'static str, key: char) -> Self {
        Self {
            label,
            key: KeyCode::Char(key),
            modifiers: KeyModifiers::CONTROL,
        }
    }
    pub const fn theme(label: &'static str, key: char) -> Self {
        Self {
            label,
            key: KeyCode::Char(key),
            modifiers: KeyModifiers::ALT,
        }
    }
    pub fn event(self) -> KeyEvent {
        KeyEvent::new(self.key, self.modifiers)
    }
}
#[derive(Default)]
pub struct Palette {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}
impl Palette {
    pub fn matches(&self, actions: &[Action]) -> Vec<Action> {
        let query = self.query.to_lowercase();
        actions
            .iter()
            .copied()
            .filter(|a| a.label.to_lowercase().contains(&query))
            .collect()
    }
    pub fn handle(&mut self, key: KeyEvent, actions: &[Action]) -> Option<Action> {
        if !self.open {
            if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL) {
                self.open = true;
                self.query.clear();
                self.selected = 0;
            }
            return None;
        }
        match key.code {
            KeyCode::Esc => self.open = false,
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => {
                self.selected =
                    (self.selected + 1).min(self.matches(actions).len().saturating_sub(1))
            }
            KeyCode::Enter => {
                let action = self.matches(actions).get(self.selected).copied();
                if action.is_some() {
                    self.open = false;
                }
                return action;
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.selected = 0;
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.query.push(c);
                self.selected = 0;
            }
            _ => {}
        }
        None
    }
    pub fn draw(
        &self,
        frame: &mut Frame,
        actions: &[Action],
        bg: Color,
        fg: Color,
        accent: Color,
        raised: Color,
    ) {
        if !self.open {
            return;
        }
        let area = frame.area();
        let width = area.width.saturating_sub(4).min(64);
        let height = area.height.saturating_sub(2).min(16);
        if width < 4 || height < 4 {
            return;
        }
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);
        let block = Block::bordered()
            .title(" Actions · Esc close ")
            .border_style(Style::default().fg(accent))
            .style(Style::default().bg(bg).fg(fg));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("› ", Style::default().fg(accent)),
                Span::raw(&self.query),
            ])),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        let items: Vec<_> = self
            .matches(actions)
            .iter()
            .map(|a| ListItem::new(a.label))
            .collect();
        if items.is_empty() {
            frame.render_widget(
                Paragraph::new("No matching actions"),
                Rect::new(inner.x, inner.y + 2, inner.width, 1),
            );
        } else {
            let mut state = ListState::default().with_selected(Some(self.selected));
            frame.render_stateful_widget(
                List::new(items)
                    .highlight_symbol("▸ ")
                    .highlight_style(Style::default().bg(raised).fg(accent)),
                Rect::new(
                    inner.x,
                    inner.y + 2,
                    inner.width,
                    inner.height.saturating_sub(2),
                ),
                &mut state,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_select_and_cancel() {
        let actions = [Action::key("Refresh", 'r'), Action::key("Help", '?')];
        let mut p = Palette::default();
        p.handle(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            &actions,
        );
        assert!(p.open);
        for c in "HELP".chars() {
            p.handle(
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT),
                &actions,
            );
        }
        assert_eq!(
            p.handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &actions)
                .unwrap()
                .key,
            KeyCode::Char('?')
        );
        assert!(!p.open);
    }
}
