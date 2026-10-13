const NAV_SCREENS: &[crate::navigation::Screen] = &[
    crate::navigation::Screen {
        name: "Devices",
        key: '1',
    },
    crate::navigation::Screen {
        name: "Shares",
        key: '2',
    },
];
impl App {
    pub fn navigation_skin(&self) -> crate::navigation::Skin {
        let t = &self.theme;
        crate::navigation::Skin {
            base: t.base(),
            muted: ratatui::style::Style::default().fg(t.muted),
            accent: ratatui::style::Style::default().fg(t.accent),
            selected: t.selected(),
            border: ratatui::style::Style::default().fg(t.border),
        }
    }
    pub fn navigation_screen(&self) -> usize {
        self.navigation.screen
    }
    pub fn pane_names(&self) -> &'static [&'static str] {
        if self.navigation.screen == 1 {
            &["Shares", "Inspector", "Actions"]
        } else {
            &["Devices", "Inspector", "Actions"]
        }
    }
    pub fn focused_pane(&self) -> usize {
        match self.focus {
            Focus::Devices | Focus::Shares => 0,
            Focus::Inspector => 1,
            Focus::Actions => 2,
        }
    }
    pub fn navigation_editing(&self) -> bool {
        false
    }
    pub fn navigation_entries(&self) -> Vec<crate::navigation::Entry> {
        crate::navigation::entries(NAV_SCREENS, self.pane_names())
    }
    pub fn navigation_bar(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        self.navigation.bar(
            frame,
            area,
            NAV_SCREENS,
            self.navigation_screen(),
            self.pane_names(),
            self.focused_pane(),
            self.navigation_editing(),
            self.navigation_skin(),
        );
    }
    pub fn draw_navigation(&self, frame: &mut ratatui::Frame) {
        self.navigation
            .draw(frame, &self.navigation_entries(), self.navigation_skin());
    }
    pub fn focus_pane(&mut self, p: usize) {
        let p = p.min(self.pane_names().len().saturating_sub(1));
        let screen = self.navigation_screen();
        self.navigation.focus[screen] = p;
        self.focus = match p {
            0 if self.navigation.screen == 1 => Focus::Shares,
            0 => Focus::Devices,
            1 => Focus::Inspector,
            _ => Focus::Actions,
        };
    }
    pub fn select_screen(&mut self, screen: usize) {
        if screen >= NAV_SCREENS.len() {
            return;
        }
        let old = self.navigation_screen();
        if old == screen {
            return;
        }
        self.navigation.focus[old] = self.focused_pane();
        if old == 0 && !matches!(self.current_row(), Some(RowKind::Shares)) {
            self.navigation.remembered[0] = self.selected;
        }
        self.navigation.scroll[old] = self.detail_scroll;
        self.detail_scroll = self.navigation.scroll[screen];
        if screen == 1 {
            self.focus_shares();
            self.focus = Focus::Shares;
        } else {
            let previous = self.navigation.remembered[0];
            self.selected = previous.min(self.rows.len().saturating_sub(1));
            if matches!(self.current_row(), Some(RowKind::Shares)) {
                self.selected = self
                    .rows
                    .iter()
                    .position(|r| !matches!(r.kind, RowKind::Shares))
                    .unwrap_or(0);
            }
        }
        self.navigation.screen = screen;
        self.focus_pane(self.navigation.focus[screen]);
    }
    fn navigation_key(&mut self, key: KeyEvent) -> bool {
        if self.commands.open {
            return false;
        }
        if self.navigation.open {
            let entries = self.navigation_entries();
            if let Some(intent) = self.navigation.key(key, &entries) {
                self.dispatch_intent(intent);
            }
            return true;
        }
        match key.code {
            KeyCode::F(2) => {
                self.navigation.toggle();
                true
            }
            KeyCode::F(6) => {
                let direction = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    -1
                } else {
                    1
                };
                self.focus_pane(crate::navigation::step(
                    self.focused_pane(),
                    self.pane_names().len(),
                    direction,
                ));
                true
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if !self.navigation_editing() {
                    let d = if key.code == KeyCode::Tab {
                        -1i32 + 2
                    } else {
                        -1
                    };
                    self.select_screen(crate::navigation::step(
                        self.navigation_screen(),
                        NAV_SCREENS.len(),
                        d,
                    ));
                }
                true
            }
            KeyCode::Char(c)
                if !self.navigation_editing()
                    && key.modifiers.is_empty()
                    && NAV_SCREENS.iter().any(|s| s.key == c) =>
            {
                let i = NAV_SCREENS.iter().position(|s| s.key == c).unwrap();
                self.select_screen(i);
                true
            }
            KeyCode::Esc => {
                let p = if self.navigation_editing()
                    && self.focused_pane() == 0
                    && self.pane_names().len() > 1
                {
                    1
                } else {
                    self.focused_pane().saturating_sub(1)
                };
                self.focus_pane(p);
                true
            }
            KeyCode::Down | KeyCode::Up | KeyCode::Char('j' | 'k')
                if !self.navigation_editing()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let d = if matches!(key.code, KeyCode::Down | KeyCode::Char('j')) {
                    1
                } else {
                    -1
                };
                self.navigation_scroll(self.focused_pane(), d);
                true
            }
            KeyCode::PageDown | KeyCode::PageUp
                if !self.navigation_editing()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let d = if key.code == KeyCode::PageDown { 8 } else { -8 };
                self.navigation_scroll(self.focused_pane(), d);
                true
            }
            KeyCode::F(1) => {
                self.help = true;
                true
            }
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.navigation_cancel();
                true
            }
            _ => false,
        }
    }
    fn navigation_cancel(&mut self) {
        if self.progress.as_ref().is_some_and(|p| p.cancel) {
            self.cancel.store(true, Ordering::Relaxed);
            self.info = "Cancelling…".into();
        } else {
            self.cancel_job();
        }
    }
    pub fn dispatch_intent(&mut self, intent: crate::navigation::Intent) {
        use crate::navigation::Intent;
        match intent {
            Intent::Screen(screen) => {
                self.navigation.open = false;
                self.select_screen(screen)
            }
            Intent::Pane(p) => {
                self.navigation.open = false;
                self.focus_pane(p)
            }
            Intent::Menu => self.navigation.toggle(),
            Intent::Details(p) => self.focus_pane(p),
            Intent::ScrollDetails(p, d) => {
                self.focus_pane(p);
                self.navigation_details_scroll(p, d)
            }
            Intent::Selector(i) => {
                self.navigation.open = false;
                self.navigation_selector(i)
            }
            Intent::Theme(id) => self.apply_aurora_theme(id),
            Intent::Command(id) => self.navigation_command(id),
            Intent::Row(p, i) => {
                self.focus_pane(p);
                self.navigation_row(p, i, false)
            }
            Intent::Activate(p, i) => {
                self.focus_pane(p);
                self.navigation_row(p, i, true)
            }
            Intent::Scroll(p, d) => {
                self.focus_pane(p);
                self.navigation_scroll(p, d)
            }
        }
    }
    pub fn command_actions(&self) -> Vec<crate::commands::Action> {
        let mut actions = NAV_SCREENS
            .iter()
            .enumerate()
            .map(|(i, s)| crate::commands::Action::screen(s.name, i, s.key))
            .collect::<Vec<_>>();
        actions.extend(self.pane_names().iter().enumerate().map(|(i, p)| {
            crate::commands::Action::function(p, 6, crate::navigation::Intent::Pane(i))
        }));
        actions.push(crate::commands::Action::function(
            "Navigate",
            2,
            crate::navigation::Intent::Menu,
        ));
        actions.extend(
            ACTIONS
                .iter()
                .filter(|a| !matches!(a.key, KeyCode::Char('0'..='9')))
                .cloned(),
        );
        for action in self.actions() {
            let mut item = crate::commands::Action::key(
                action.id.label(),
                action.id.key().chars().next().unwrap_or('\0'),
            );
            if !action.enabled {
                item = item.unavailable(action.reason);
            }
            actions.push(item);
        }
        actions
    }
}

impl App {
    fn navigation_command(&mut self, id: &str) {
        match id {
            "Confirm" => match self.dialog {
                Dialog::Confirm(_) => self.submit_confirm(),
                Dialog::Form(_) => self.submit_form(),
                _ => {}
            },
            "Cancel" => self.dialog = Dialog::None,
            "Theme picker" => self.open_themes(),
            "Help" => self.help = true,
            _ => {
                if let Some(action) = self.actions().into_iter().find(|a| a.id.label() == id) {
                    if action.enabled {
                        self.open_action(action.id)
                    } else {
                        self.error = action.reason
                    }
                }
            }
        }
    }
    fn navigation_row(&mut self, pane: usize, index: usize, activate: bool) {
        match pane {
            0 if self.navigation.screen == 1 => {
                if index < self.shares.len() {
                    self.share_index = index;
                    if activate {
                        self.open_share(false);
                    }
                }
            }
            0 => {
                if index < self.rows.len() {
                    if !matches!(self.current_row(), Some(RowKind::Shares)) {
                        self.navigation.remembered[0] = self.selected;
                    }
                    self.selected = index;
                    self.action_index = 0;
                    if matches!(self.current_row(), Some(RowKind::Shares)) {
                        self.select_screen(1);
                    } else if activate {
                        self.focus_pane(1);
                    }
                }
            }
            2 => {
                if index < self.actions().len() {
                    self.action_index = index;
                    if activate {
                        self.activate();
                    }
                }
            }
            _ => {}
        }
    }
    fn navigation_scroll(&mut self, pane: usize, d: i32) {
        if pane == 1 {
            if d < 0 {
                self.detail_scroll = self.detail_scroll.saturating_sub(3)
            } else {
                self.detail_scroll = self.detail_scroll.saturating_add(3)
            }
        } else {
            if self.navigation.screen == 0 && !matches!(self.current_row(), Some(RowKind::Shares)) {
                self.navigation.remembered[0] = self.selected;
            }
            self.move_sel(d);
            if self.navigation.screen == 0 && matches!(self.current_row(), Some(RowKind::Shares)) {
                self.select_screen(1);
            }
        }
    }
    pub fn navigation_mouse(&mut self, event: crossterm::event::MouseEvent) {
        if self.help || self.picking_theme {
            return;
        }
        if !matches!(self.dialog, Dialog::None) {
            if let Some(intent) = self.navigation.mouse(event) {
                match intent {
                    crate::navigation::Intent::Command("Confirm" | "Cancel") => {
                        self.dispatch_intent(intent)
                    }
                    crate::navigation::Intent::Row(99, i) => {
                        if let Dialog::Form(form) = &mut self.dialog {
                            form.focus = i.min(form.fields.len().saturating_sub(1));
                        }
                    }
                    _ => {}
                }
            }
            return;
        }
        if self.navigation.open {
            let entries = self.navigation_entries();
            if let Some(intent) = self.navigation.menu_mouse(event, &entries) {
                self.dispatch_intent(intent);
            }
            return;
        }
        if self.commands.open {
            if let Some(action) = self.commands.mouse(event, &self.command_actions()) {
                self.dispatch_intent(action.target);
            }
            return;
        }
        if let Some(intent) = self.navigation.mouse(event) {
            self.dispatch_intent(intent);
        }
    }
}

impl App {
    fn navigation_details_scroll(&mut self, _p: usize, d: i32) {
        self.detail_scroll = (self.detail_scroll as i32 + d * 3).clamp(0, u16::MAX as i32) as u16;
    }
    fn navigation_selector(&mut self, _i: usize) {}
}

impl App {
    pub fn footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        match self.focused_pane() {
            0 if self.navigation_screen() == 1 => vec![
                ("Enter", "connect/disconnect"),
                ("Ctrl+K", "actions"),
                ("n", "new share"),
                ("e", "edit"),
            ],
            0 => vec![
                ("Enter", "inspect"),
                ("Ctrl+K", "actions"),
                ("↑/↓", "select"),
            ],
            1 => vec![
                ("PgUp/PgDn", "scroll"),
                ("Ctrl+K", "actions"),
                ("F6", "actions"),
            ],
            _ => vec![
                ("Enter", "review action"),
                ("Ctrl+K", "actions"),
                ("↑/↓", "select"),
            ],
        }
    }
}
