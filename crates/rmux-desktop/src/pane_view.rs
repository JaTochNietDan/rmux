use super::*;

fn tab_drop_index(from: usize, target: usize) -> usize {
    target.saturating_sub(usize::from(from < target))
}

#[cfg(test)]
mod tab_order_tests {
    use super::tab_drop_index;
    #[test]
    fn drops_insert_before_target_from_either_direction() {
        for (from, target, expected) in [
            (0, 3, vec![1, 2, 0, 3]),
            (0, 4, vec![1, 2, 3, 0]),
            (3, 1, vec![0, 3, 1, 2]),
            (1, 2, vec![0, 1, 2, 3]),
            (2, 2, vec![0, 1, 2, 3]),
        ] {
            let mut tabs = vec![0, 1, 2, 3];
            let moved = tabs.remove(from);
            let selected = tab_drop_index(from, target);
            tabs.insert(selected, moved);
            assert_eq!(tabs, expected);
            assert_eq!(tabs[selected], from);
        }
    }
}

pub(super) struct ControlHint(pub(super) SharedString);
impl Render for ControlHint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgb(0x383838))
            .text_color(rgb(0xe5e5e5))
            .text_sm()
            .max_w(px(380.))
            .child(self.0.clone())
    }
}

pub(super) fn pane_control(id: &'static str, hint: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.))
        .h(px(28.))
        .flex_shrink_0()
        .rounded_sm()
        .text_color(rgb(0xaaaaaa))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x353535)).text_color(rgb(0xffffff)))
        .tooltip(move |_, cx| cx.new(|_| ControlHint(hint.into())).into())
}

pub(super) struct PaneView {
    // Metadata repaints preserve this offset; selection and structural edits
    // explicitly reveal the active tab. Busy background tabs must not reset it.
    tab_scroll: ScrollHandle,
    tab_picker: Option<(Entity<text_field::TextField>, Option<EntityId>, ScrollHandle)>,
    tab_picker_blur: Option<Subscription>,
    workspace: EntityId,
    titles: std::collections::HashMap<EntityId, Subscription>,
    pub terminals: Vec<Entity<TerminalView>>,
    active: usize,
    next_number: usize,
    labels: Vec<String>,
    benchmark: bool,
    error: Option<String>,
    rename: Option<Entity<text_field::TextField>>,
    close_prompt: Option<EntityId>,
}
pub(super) struct PaneEmpty;
impl EventEmitter<PaneEmpty> for PaneView {}
pub(super) struct PaneMetadataChanged;
impl EventEmitter<PaneMetadataChanged> for PaneView {}
pub(super) struct MoveTab {
    pub source: EntityId,
    pub terminal: EntityId,
    pub target_tab: Option<EntityId>,
}
impl EventEmitter<MoveTab> for PaneView {}
pub(super) struct TabTransfer {
    pub terminal: Entity<TerminalView>,
    label: String,
}

impl PaneView {
    fn show_tab_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminals.is_empty() || self.rename.is_some() || self.close_prompt.is_some() { return; }
        if let Some((input, _, _)) = &self.tab_picker { input.read(cx).focus.focus(window); return; }
        let input = cx.new(|cx| text_field::TextField::new("", cx));
        let focus = input.read(cx).focus.clone();
        let token = focus.clone();
        self.tab_picker_blur = Some(cx.on_blur(&focus, window, move |this, _, cx| {
            if this.tab_picker.as_ref().is_some_and(|(input, _, _)| input.read(cx).focus == token) {
                this.tab_picker = None;
                this.tab_picker_blur = None;
                cx.notify();
            }
        }));
        let scroll = ScrollHandle::new();
        scroll.scroll_to_item(self.active);
        focus.focus(window);
        cx.subscribe_in(&input, window, |this, _, event: &text_field::EditEvent, window, cx| {
            match event {
                text_field::EditEvent::Changed(_) => {
                    let first = this.matching_tabs(cx).first().map(|index| this.terminals[*index].entity_id());
                    if let Some((_, selected, scroll)) = &mut this.tab_picker { *selected = first; scroll.scroll_to_item(0); }
                }
                text_field::EditEvent::Submitted { .. } => {
                    if let Some((_, selected, _)) = &this.tab_picker {
                        let matches = this.matching_tabs(cx);
                        if let Some(index) = matches.iter().find(|index| Some(this.terminals[**index].entity_id()) == *selected)
                            .or_else(|| matches.first()).copied() { this.select(index, window, cx); }
                    }
                }
                text_field::EditEvent::Cancelled => { this.tab_picker = None; this.tab_picker_blur = None; this.focus(window, cx); }
            }
            cx.notify();
        }).detach();
        self.tab_picker = Some((input, self.active_terminal_id(), scroll));
        cx.notify();
    }
    fn matching_tabs(&self, cx: &App) -> Vec<usize> {
        let query = self.tab_picker.as_ref().map(|(input, _, _)| input.read(cx).value().to_lowercase()).unwrap_or_default();
        let words: Vec<_> = query.split_whitespace().collect();
        // An empty picker follows tab order; avoid allocating/lowercasing every
        // title and directory merely to assign identical scores and sort them.
        if words.is_empty() { return (0..self.terminals.len()).collect(); }
        let mut matches: Vec<_> = (0..self.terminals.len()).filter_map(|index| {
            let title = self.tab_title(index, cx).to_lowercase();
            let directory = self.terminals[index].read(cx).snapshot_directory.as_ref()
                .map(|path| path.to_string_lossy().to_lowercase()).unwrap_or_default();
            palette_match::tab_score(index + 1, &title, &directory, &words).map(|score| (score, index))
        }).collect();
        matches.sort_by_key(|(score, index)| (*score, *index));
        matches.into_iter().map(|(_, index)| index).collect()
    }

    pub(super) fn automation_busy(&self) -> bool {
        self.rename.is_some() || self.close_prompt.is_some() || self.tab_picker.is_some()
    }
    pub(super) fn automation_surfaces(&self, window_id: &str, workspace_id: &str, pane_id: &str, history: bool, cx: &App) -> Vec<serde_json::Value> {
        self.terminals.iter().enumerate().map(|(index, entity)| {
            let terminal = entity.read(cx);
            serde_json::json!({
                "id":entity.entity_id().to_string(), "window_id":window_id,
                "workspace_id":workspace_id, "pane_id":pane_id,
                "index":index, "type":"terminal", "title":self.tab_title(index, cx),
                "selected":index == self.active, "attention":terminal.attention.any(),
                "directory_label":terminal.directory_label,
                "git_branch":terminal.git_branch,
                "notification":terminal.notifications.latest()
                    .map(|message| serde_json::json!({"revision":message.revision,"title":message.title,"body":message.body})),
                "notifications":terminal.notifications.iter().rev().take(if history { usize::MAX } else { 0 }).map(|record| serde_json::json!({
                    "revision":record.message.revision,"title":record.message.title,"body":record.message.body,"unread":record.unread
                })).collect::<Vec<_>>(),
                "process_id":terminal.session.process_id(),
                "process_exited":terminal.session.process_exited(),
            })
        }).collect()
    }
    pub fn reset_window_editors(&mut self) {
        self.close_prompt = None;
        self.rename = None;
        self.tab_picker = None;
        self.tab_picker_blur = None;
    }
    pub fn tab_title(&self, index: usize, cx: &App) -> String {
        let view = self.terminals[index].read(cx);
        view.custom_title.clone().unwrap_or_else(|| {
            if view.title.is_empty() {
                self.labels[index].clone()
            } else {
                view.title.clone()
            }
        })
    }
    fn rename_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.rename {
            input.read(cx).focus.focus(window);
            return;
        }
        let Some(terminal) = self.terminals.get(self.active) else {
            return;
        };
        let id = terminal.entity_id();
        let name = terminal.read(cx).custom_title.clone().unwrap_or_default();
        let input = cx.new(|cx| text_field::TextField::new(&name, cx));
        cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &text_field::EditEvent, window, cx| {
                match event {
                    text_field::EditEvent::Submitted { text, .. } => {
                        if let Some(terminal) = this
                            .terminals
                            .iter()
                            .find(|terminal| terminal.entity_id() == id)
                        {
                            let title: String = text
                                .trim()
                                .chars()
                                .filter(|c| !c.is_control())
                                .take(128)
                                .collect();
                            terminal.update(cx, |terminal, cx| {
                                terminal.custom_title = (!title.is_empty()).then_some(title);
                                cx.emit(TabMetadataChanged);
                                cx.notify();
                            });
                        }
                    }
                    text_field::EditEvent::Cancelled => {}
                    text_field::EditEvent::Changed(_) => return,
                }
                this.rename = None;
                this.focus(window, cx);
                cx.notify();
            },
        )
        .detach();
        input.read(cx).focus.focus(window);
        self.rename = Some(input);
        cx.notify();
    }
    pub fn active_cell_metrics(&self, cx: &App) -> CellMetrics {
        self.terminals
            .get(self.active)
            .map(|terminal| terminal.read(cx).cells)
            .unwrap_or_else(|| CellMetrics::new(cx.global::<Preferences>().font_size))
    }
    pub fn active_font_family(&self, cx: &App) -> SharedString {
        self.terminals
            .get(self.active)
            .map(|terminal| terminal.read(cx).font_family.clone())
            .unwrap_or_else(|| cx.global::<Preferences>().font_family.clone().into())
    }
    pub fn active_terminal_id(&self) -> Option<EntityId> {
        self.terminals.get(self.active).map(Entity::entity_id)
    }

    pub fn release_render_caches(&mut self, cx: &mut Context<Self>) {
        for terminal in &self.terminals {
            if !terminal.read(cx).row_cache.is_empty() {
                terminal.update(cx, |terminal, _| terminal.release_render_cache());
            }
        }
    }

    pub fn take_all_tabs(&mut self, cx: &mut Context<Self>) -> (Vec<TabTransfer>, usize) {
        let active = self.active;
        self.titles.clear();
        let tabs = self
            .terminals
            .drain(..)
            .zip(self.labels.drain(..))
            .map(|(terminal, label)| TabTransfer { terminal, label })
            .collect();
        self.active = 0;
        cx.emit(PaneMetadataChanged);
        cx.notify();
        (tabs, active)
    }
    pub fn append_tabs(
        &mut self,
        tabs: Vec<TabTransfer>,
        active: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if tabs.is_empty() {
            return;
        }
        let selected = self.terminals.len() + active.min(tabs.len() - 1);
        for tab in tabs {
            let title = cx.subscribe(&tab.terminal, |_, _, _: &TabMetadataChanged, cx| {
                cx.emit(PaneMetadataChanged);
                cx.notify();
            });
            self.titles.insert(tab.terminal.entity_id(), title);
            self.terminals.push(tab.terminal);
            self.labels.push(tab.label);
        }
        cx.emit(PaneMetadataChanged);
        // Select only once, without acknowledging other unread tabs in transit.
        self.select(selected, window, cx);
    }

    pub fn active_tab_for_split(&self) -> Option<(EntityId, TabTransfer)> {
        if self.terminals.len() < 2 {
            return None;
        }
        self.active_tab_transfer()
    }
    pub fn active_tab_transfer(&self) -> Option<(EntityId, TabTransfer)> {
        self.tab_transfer(self.active_terminal_id()?)
    }
    pub fn tab_transfer(&self, id: EntityId) -> Option<(EntityId, TabTransfer)> {
        let index = self.terminals.iter().position(|terminal| terminal.entity_id() == id)?;
        let terminal = self.terminals[index].clone();
        Some((
            terminal.entity_id(),
            TabTransfer {
                terminal,
                label: self.labels[index].clone(),
            },
        ))
    }
    pub fn from_transfer(tab: TabTransfer, workspace: EntityId, cx: &mut Context<Self>) -> Self {
        Self::restored(vec![tab.terminal], vec![tab.label], 0, workspace, cx)
    }

    pub fn context_label(&self, cx: &App) -> Option<(Option<String>, Option<String>)> {
        let terminal = self.terminals.get(self.active)?.read(cx);
        let directory = terminal.directory_label.clone().or_else(|| terminal.snapshot_directory.as_ref().map(|path| path.to_string_lossy().into_owned()));
        (terminal.git_branch.is_some() || directory.is_some()).then(|| (terminal.git_branch.clone(), directory))
    }
    pub fn needs_attention(&self, cx: &App) -> bool {
        self.terminals
            .iter()
            .any(|terminal| terminal.read(cx).attention.any())
    }
    pub fn restored(
        terminals: Vec<Entity<TerminalView>>,
        labels: Vec<String>,
        active: usize,
        workspace: EntityId,
        cx: &mut Context<Self>,
    ) -> Self {
        let titles = terminals
            .iter()
            .map(|terminal| {
                (
                    terminal.entity_id(),
                    cx.subscribe(
                        terminal,
                        |_, _, _: &TabMetadataChanged, cx| {
                            cx.emit(PaneMetadataChanged);
                            cx.notify();
                        },
                    ),
                )
            })
            .collect();
        let tab_scroll = ScrollHandle::new();
        tab_scroll.scroll_to_item(active);
        Self {
            tab_scroll,
            tab_picker: None,
            tab_picker_blur: None,
            workspace,
            titles,
            next_number: terminals.len() + 1,
            terminals,
            labels,
            active,
            benchmark: false,
            error: None,
            rename: None,
            close_prompt: None,
        }
    }
    pub fn snapshot(&self, cx: &App) -> snapshot::Node {
        snapshot::Node::Pane {
            active_tab: self.active,
            tabs: self
                .terminals
                .iter()
                .zip(&self.labels)
                .map(|(terminal, label)| snapshot::TabSnapshot {
                    label: label.clone(),
                    custom_title: terminal.read(cx).custom_title.clone(),
                    font_size: terminal.read(cx).cells.font_size,
                    font_family: Some(terminal.read(cx).font_family.to_string()),
                    directory: terminal.read(cx).snapshot_directory.clone(),
                })
                .collect(),
        }
    }
    pub fn new(
        terminal: Entity<TerminalView>,
        label: Option<String>,
        benchmark: bool,
        workspace: EntityId,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.subscribe(
            &terminal,
            |_, _, _: &TabMetadataChanged, cx| {
                cx.emit(PaneMetadataChanged);
                cx.notify();
            },
        );
        Self {
            tab_scroll: ScrollHandle::new(),
            tab_picker: None,
            tab_picker_blur: None,
            workspace,
            titles: [(terminal.entity_id(), title)].into_iter().collect(),
            terminals: vec![terminal],
            active: 0,
            next_number: 2,
            labels: vec![label.unwrap_or_else(|| "Terminal 1".into())],
            benchmark,
            error: None,
            rename: None,
            close_prompt: None,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &App) {
        if let Some(terminal) = self.terminals.get(self.active) {
            terminal.read(cx).focus.focus(window);
        }
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.terminals
            .get(self.active)
            .is_some_and(|terminal| terminal.read(cx).focus.contains_focused(window, cx))
    }

    pub fn terminal_bounds(&self, cx: &App) -> Bounds<Pixels> {
        self.terminals
            .get(self.active)
            .map(|view| view.read(cx).last_bounds)
            .unwrap_or_default()
    }
    /// Cached OSC directory, or explicit launch directory before the first report.
    /// New-workspace creation must not wait for the terminal parser mutex.
    pub fn cached_directory(&self, cx: &App) -> Option<std::path::PathBuf> {
        self.terminals.get(self.active)?.read(cx).snapshot_directory.clone()
    }

    pub fn current_directory(&self, cx: &App) -> Option<std::path::PathBuf> {
        self.terminals
            .get(self.active)?
            .read(cx)
            .session
            .current_directory()
            .ok()
            .flatten()
    }

    pub fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.terminals.len() {
            return;
        }
        self.active = index;
        self.rename = None;
        self.tab_picker = None;
        self.tab_picker_blur = None;
        self.tab_scroll.scroll_to_item(index);
        self.focus(window, cx);
        cx.notify();
    }

    fn cycle(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminals.is_empty() {
            return;
        }
        self.select(
            (self.active as isize + delta).rem_euclid(self.terminals.len() as isize) as usize,
            window,
            cx,
        );
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if cx.global::<Shutdowns>().quitting {
            return;
        }
        let directory = self.current_directory(cx);
        let cells = self.active_cell_metrics(cx);
        let family = self.active_font_family(cx);
        match Session::shell_in_with_scrollback(
            80,
            24,
            directory
                .as_deref()
                .or(cx.global::<InitialDirectory>().0.as_deref()),
            cx.global::<Preferences>().scrollback_bytes(),
        ) {
            Ok(session) => {
                let terminal = cx.new(|cx| {
                    let mut terminal = TerminalView::new(session, window, cx);
                    terminal.cells = cells;
                    terminal.font_family = family;
                    terminal
                });
                let title = cx.subscribe(&terminal, |_, _, _: &TabMetadataChanged, cx| {
                    cx.emit(PaneMetadataChanged);
                    cx.notify();
                });
                self.titles.insert(terminal.entity_id(), title);
                self.terminals.push(terminal);
                self.labels.push(format!("Terminal {}", self.next_number));
                self.next_number += 1;
                self.error = None;
                self.select(self.terminals.len() - 1, window, cx);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn reorder(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self.active as isize + delta;
        if next < 0 || next >= self.terminals.len() as isize {
            return;
        }
        let next = next as usize;
        self.terminals.swap(self.active, next);
        self.labels.swap(self.active, next);
        self.active = next;
        self.tab_scroll.scroll_to_item(next);
        cx.emit(PaneMetadataChanged);
        cx.notify();
    }

    fn drop_tab(
        &mut self,
        item: &drag_item::DragItem,
        target: Option<EntityId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if item.container != cx.entity_id() {
            if item.scope == self.workspace {
                cx.emit(MoveTab {
                    source: item.container,
                    terminal: item.item,
                    target_tab: target,
                });
            }
            return;
        }
        let Some(from) = self
            .terminals
            .iter()
            .position(|view| view.entity_id() == item.item)
        else {
            return;
        };
        let to = match target {
            Some(target) => {
                let Some(index) = self.terminals.iter().position(|view| view.entity_id() == target) else { return; };
                index
            }
            None => self.terminals.len(),
        };
        let to = tab_drop_index(from, to);
        if from != to {
            let terminal = self.terminals.remove(from);
            let label = self.labels.remove(from);
            self.terminals.insert(to, terminal);
            self.labels.insert(to, label);
            cx.emit(PaneMetadataChanged);
        }
        self.select(to, window, cx);
    }

    pub fn take_tab(&mut self, id: EntityId, cx: &mut Context<Self>) -> Option<TabTransfer> {
        let index = self
            .terminals
            .iter()
            .position(|view| view.entity_id() == id)?;
        self.titles.remove(&id);
        let terminal = self.terminals.remove(index);
        let label = self.labels.remove(index);
        if index < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.terminals.len().saturating_sub(1));
        self.tab_scroll.scroll_to_item(self.active);
        cx.emit(PaneMetadataChanged);
        cx.notify();
        Some(TabTransfer { terminal, label })
    }

    /// The workspace resolves this index before detaching the source tab.
    pub fn insert_tab_at(
        &mut self,
        tab: TabTransfer,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = cx.subscribe(&tab.terminal, |_, _, _: &TabMetadataChanged, cx| {
            cx.emit(PaneMetadataChanged);
            cx.notify();
        });
        self.titles.insert(tab.terminal.entity_id(), title);
        self.terminals.insert(index, tab.terminal);
        self.labels.insert(index, tab.label);
        cx.emit(PaneMetadataChanged);
        self.select(index, window, cx);
    }

    fn close(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(terminal) = self.terminals.get(index) else {
            return;
        };
        let view = terminal.read(cx);
        if !cx
            .global::<ClosePolicy>()
            .confirms(true, || view.has_active_work())
        {
            self.close_confirmed(index, window, cx);
            return;
        }
        if self.close_prompt.is_some() {
            return;
        }
        let id = terminal.entity_id();
        let focus = view.focus.clone();
        let Some(permit) = cx.global::<PromptGate>().acquire() else {
            return;
        };
        self.close_prompt = Some(id);
        let answer = window.prompt(PromptLevel::Warning, "Close terminal?",
            Some("Closing this terminal terminates its owned process and discards its output history."),
            &["Cancel", "Close Terminal"], cx);
        cx.spawn_in(window, async move |entity, cx| {
            let confirmed = matches!(answer.await, Ok(1));
            drop(permit);
            let _ = cx.update(|window, cx| {
                entity.update(cx, |pane, cx| {
                    if pane.close_prompt != Some(id) {
                        return;
                    }
                    pane.close_prompt = None;
                    if confirmed {
                        if let Some(index) = pane.terminals.iter().position(|terminal| {
                            terminal.entity_id() == id && terminal.read(cx).focus == focus
                        }) {
                            pane.close_confirmed(index, window, cx);
                        }
                    }
                })
            });
        })
        .detach();
    }

    fn close_confirmed(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.terminals.len() {
            return;
        }
        // Closing an inactive tab must not steal focus from another terminal,
        // a find field or an editor elsewhere in the window.
        let replace_focus = self.terminals[index]
            .read(cx)
            .focus
            .contains_focused(window, cx);
        let removed = self.terminals.remove(index);
        self.titles.remove(&removed.entity_id());
        self.labels.remove(index);
        cx.emit(PaneMetadataChanged);
        if self.terminals.is_empty() {
            cx.emit(PaneEmpty);
        } else {
            if index < self.active {
                self.active -= 1;
            }
            self.active = self.active.min(self.terminals.len() - 1);
            self.tab_scroll.scroll_to_item(self.active);
            if replace_focus {
                self.focus(window, cx);
            }
        }
        cx.notify();
    }
}

impl Render for PaneView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Hidden tabs keep their engines, but need not retain shaped glyph rows.
        // Do not notify: releasing a cache cannot change visible output.
        for (index, terminal) in self.terminals.iter().enumerate() {
            if index != self.active && !terminal.read(cx).row_cache.is_empty() {
                terminal.update(cx, |terminal, _| terminal.release_render_cache());
            }
        }
        let focused = window.is_window_active() && self.is_focused(window, cx);
        let attention = self.needs_attention(cx);
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .when(!self.benchmark, |pane| pane.border_1().border_color(if attention { rgb(0xf0b85b).into() } else { gpui::transparent_black() }))
            .on_action(cx.listener(|this, _: &NewTab, window, cx| this.add(window, cx)))
            .on_action(cx.listener(|this, _: &RenameTab, window, cx| this.rename_tab(window, cx)))
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.cycle(1, window, cx)))
            .on_action(cx.listener(|this, _: &ShowTabPicker, window, cx| this.show_tab_picker(window, cx)))
            .on_action(cx.listener(|this, action: &SelectTabNumber, window, cx| {
                if let Some(index) = action.number.checked_sub(1) {
                    this.select(index, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &PreviousTab, window, cx| this.cycle(-1, window, cx)))
            .on_action(cx.listener(|this, _: &MoveTabLeft, _, cx| this.reorder(-1, cx)))
            .on_action(cx.listener(|this, _: &MoveTabRight, _, cx| this.reorder(1, cx)))
            .on_action(
                cx.listener(|this, _: &ClosePane, window, cx| this.close(this.active, window, cx)),
            )
            .when(!self.benchmark, |root| {
                root.child(
                    div()
                        .flex()
                        .h(px(30.))
                        .flex_shrink_0()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0x383838))
                        .bg(rgb(0x242424))
                        .child(
                            div()
                                .id("tabs")
                                .flex()
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .overflow_x_scroll()
                                .track_scroll(&self.tab_scroll)
                                .children(self.terminals.iter().enumerate().map(
                                    |(index, terminal)| {
                                        let container = cx.entity_id();
                                        let scope = self.workspace;
                                        let target = terminal.entity_id();
                                        let label: SharedString = self.tab_title(index, cx).into();
                                        let title_hint = label.clone();
                                        let display_title = label.clone();
                                        div()
                                            .id(("tab", terminal.entity_id()))
                                            .flex()
                                            .items_center()
                                            .flex_shrink_0()
                                            .px_2()
                                            .gap_2()
                                            .text_size(px(12.))
                                            .border_t_1()
                                            .border_l_2()
                                            .border_color(gpui::transparent_black())
                                            .cursor_pointer()
                                            .on_drag(drag_item::DragItem { scope, container, item: target, label }, |item, _, _, cx| cx.new(|_| item.clone()))
                                            .can_drop(move |item, _, _| item.downcast_ref::<drag_item::DragItem>().is_some_and(|item| item.scope == scope && item.item != target))
                                            .drag_over(move |style, item: &drag_item::DragItem, _, _| {
                                                if item.scope == scope && item.item != target {
                                                    style.bg(rgb(0x303b4a)).border_color(rgb(0x4599ef))
                                                } else { style }
                                            })
                                            .on_drop(cx.listener(move |this, item: &drag_item::DragItem, window, cx| this.drop_tab(item, Some(target), window, cx)))
                                            .text_color(rgb(0x9b9b9b))
                                            .when(index == self.active, |tab| tab
                                                .bg(rgb(if focused { 0x303030 } else { 0x292929 }))
                                                .text_color(rgb(if focused { 0xe5e5e5 } else { 0xb8bfcb }))
                                                .border_color(rgb(if focused { 0x4599ef } else { 0x4a5362 })))
                                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                                if !event.standard_click() { return; }
                                                let Some(index) = this.terminals.iter().position(|terminal| terminal.entity_id() == target) else { return; };
                                                this.select(index, window, cx);
                                                if matches!(event, ClickEvent::Mouse(mouse) if mouse.down.click_count == 2) {
                                                    this.rename_tab(window, cx);
                                                }
                                            }))
                                            .on_mouse_down(MouseButton::Middle, cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                if let Some(index) = this.terminals.iter().position(|terminal| terminal.entity_id() == target) {
                                                    this.close(index, window, cx);
                                                }
                                            }))
                                            .when(terminal.read(cx).attention.any(), |tab| tab.child(
                                                div().w(px(6.)).h(px(6.)).flex_shrink_0().rounded_full().bg(rgb(0xf0b85b))
                                            ))
                                            .when_some(terminal.read(cx).program_status.as_ref(), |tab, status| {
                                                use rmux_terminal::program_status::{Kind, State};
                                                let record = &status.record;
                                                let (label, tint) = match record.state {
                                                    State::Idle => ("Idle", 0xa0a0a0),
                                                    State::Working => ("Working", 0x86b8f5),
                                                    State::Done => ("Done", 0x8dd6a1),
                                                    State::Error => ("Error", 0xf18a8a),
                                                    State::Blocked => (match record.kind { Kind::Permission => "Approval", Kind::Question => "Question", Kind::Auth => "Login", Kind::None => "Blocked" }, 0xf0b85b),
                                                };
                                                let mut text = label.to_owned();
                                                if let Some(percent) = record.progress { text.push_str(&format!(" {percent}%")); }
                                                if status.count > 1 { text.push_str(&format!(" +{}", status.count - 1)); }
                                                let detail: SharedString = format!("{label} · {} · {}\n{}", record.app, record.title, record.message).into();
                                                tab.child(div().id(("program-status", terminal.entity_id())).text_xs().flex_shrink_0().text_color(rgb(tint))
                                                    .tooltip(move |_, cx| cx.new(|_| ControlHint(detail.clone())).into()).child(text))
                                            })
                                            .when_some(terminal.read(cx).progress, |tab, progress| {
                                                use rmux_terminal::ProgressState;
                                                let (label, tint) = match progress.state {
                                                    ProgressState::Running => ("", 0x86b8f5),
                                                    ProgressState::Error => ("Error", 0xf18a8a),
                                                    ProgressState::Indeterminate => ("Working", 0x86b8f5),
                                                    ProgressState::Paused => ("Paused", 0xf0b85b),
                                                };
                                                let text = match progress.percent {
                                                    Some(percent) if progress.state != ProgressState::Indeterminate => format!("{label} {percent}%").trim().to_owned(),
                                                    _ if label.is_empty() => "Working".to_owned(),
                                                    _ => label.to_owned(),
                                                };
                                                tab.child(div().text_xs().flex_shrink_0().text_color(rgb(tint)).child(text))
                                            })
                                            .child({
                                                div().id(("tab-title", target)).min_w(px(48.)).max_w(px(220.)).truncate()
                                                    .tooltip(move |_, cx| cx.new(|_| ControlHint(title_hint.clone())).into()).child(
                                                    display_title,
                                                )
                                            })
                                            .child(
                                                div()
                                                    .id(("close-tab", terminal.entity_id()))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .w(px(22.))
                                                    .h(px(22.))
                                                    .flex_shrink_0()
                                                    .rounded_sm()
                                                    .tooltip(|_, cx| cx.new(|_| ControlHint("Close Terminal Tab".into())).into())
                                                    .hover(|style| style.bg(rgb(0x505867)))
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            cx.stop_propagation();
                                                            if let Some(index) = this.terminals.iter().position(|terminal| terminal.entity_id() == target) {
                                                                this.close(index, window, cx);
                                                            }
                                                        },
                                                    ))
                                                    .child(icons::icon("close")),
                                            )
                                    },
                                ))
                                .child({
                                    let scope = self.workspace;
                                    div().id("tab-drop-end").w(px(20.)).h_full().flex_shrink_0()
                                        .border_l_2().border_color(gpui::transparent_black())
                                        .can_drop(move |item, _, _| item.downcast_ref::<drag_item::DragItem>().is_some_and(|item| item.scope == scope))
                                        .drag_over(move |style, item: &drag_item::DragItem, _, _| {
                                            if item.scope == scope { style.bg(rgb(0x303b4a)).border_color(rgb(0x4599ef)) } else { style }
                                        })
                                        .on_drop(cx.listener(|this, item: &drag_item::DragItem, window, cx| this.drop_tab(item, None, window, cx)))
                                }),
                        )
                        .when(self.terminals.len() > 1, |header| header.child(
                            pane_control("tab-picker", if cfg!(target_os = "macos") { "Choose Terminal Tab  ⌘⇧T" } else { "Choose Terminal Tab  Ctrl+Alt+Shift+T" })
                                .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                                    if !event.standard_click() { return; }
                                    this.show_tab_picker(window, cx);
                                })).child(icons::icon("chevron-down"))
                        ))
                        .child(
                            pane_control("new-tab", if cfg!(target_os = "macos") { "New Terminal Tab  ⌘T" } else { "New Terminal Tab  Ctrl+Shift+T" })
                                .on_click(cx.listener(|this, _, window, cx| this.add(window, cx)))
                                .child(icons::icon("plus")),
                        )
                        .child(
                            pane_control("split-right", if cfg!(target_os = "macos") { "Split Right  ⌘D" } else { "Split Right  Ctrl+Shift+D" })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.focus(window, cx);
                                    window.dispatch_action(Box::new(Split), cx);
                                }))
                                .child(icons::icon("split-right")),
                        )
                        .child(
                            pane_control("split-down", if cfg!(target_os = "macos") { "Split Down  ⌘⇧D" } else { "Split Down  Ctrl+Shift+E" })
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.focus(window, cx);
                                    window.dispatch_action(Box::new(SplitBelow), cx);
                                }))
                                .child(icons::icon("split-down")),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .when_some(self.terminals.get(self.active), |body, terminal| {
                        body.child(terminal.clone())
                    }),
            )
            .when_some(self.rename.as_ref(), |root, input| {
                root.child(div().flex().flex_col().gap_1().px_3().py_2().bg(rgb(0x292929))
                    .child(div().text_xs().text_color(rgb(0xaaaaaa)).child("Tab name · empty restores automatic title · Enter to save · Esc to cancel"))
                    .child(input.clone()))
            })
            .when_some(self.error.clone(), |root, error| {
                root.child(div_error(error, cx.listener(|this, event: &ClickEvent, _, cx| {
                    if event.standard_click() { this.error = None; cx.stop_propagation(); cx.notify(); }
                })))
            })
            .when_some(self.tab_picker.clone(), |root, (input, selected, scroll)| {
                let matches = self.matching_tabs(cx);
                let selected = matches.iter().map(|index| self.terminals[*index].entity_id())
                    .find(|id| Some(*id) == selected)
                    .or_else(|| matches.first().map(|index| self.terminals[*index].entity_id()));
                root.child(deferred(div().absolute().top(px(30.)).bottom_0().left_0().right_0()
                    .flex().flex_col().min_h_0()
                    .child(div().id("tab-picker-container").min_h_0().flex().flex_col()
                    .bg(rgb(0x292929)).border_1().border_color(rgb(0x465063)).rounded_md().shadow_md()
                    .on_mouse_down_out(cx.listener(|this, _, window, cx| { this.tab_picker = None; this.tab_picker_blur = None; this.focus(window, cx); cx.notify(); }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        let matches = this.matching_tabs(cx);
                        let Some((input, selected, scroll)) = &mut this.tab_picker else { return; };
                        if input.read(cx).is_composing() { return; }
                        let current = matches.iter().position(|index| Some(this.terminals[*index].entity_id()) == *selected).unwrap_or(0);
                        let key = event.keystroke.key.as_str();
                        let destination = if matches!(key, "pageup" | "pagedown") {
                            command_palette::paged_result(current, matches.len(), key == "pageup",
                                f32::from(scroll.bounds().size.height), |index| {
                                    scroll.bounds_for_item(index).map(|bounds| f32::from(bounds.size.height)).unwrap_or(1.).max(1.)
                                })
                        } else {
                            command_palette::navigated_result(current, matches.len(), key)
                        };
                        if let Some(next) = destination {
                            *selected = Some(this.terminals[matches[next]].entity_id());
                            scroll.scroll_to_item(next); cx.stop_propagation(); cx.notify();
                        }
                    }))
                    .child(div().px_3().pt_2().text_xs().text_color(rgb(0xaaaaaa))
                        .child(format!("{} of {} tabs · Search title or directory · ↑↓ choose · Enter open · Esc cancel", matches.len(), self.terminals.len())))
                    .child(div().flex_shrink_0().p_2().child(input))
                    .child(div().id("tab-picker-menu").min_h_0().max_h(px(240.)).overflow_y_scroll().track_scroll(&scroll)
                    .when(matches.is_empty(), |menu| menu.child(div().p_3().text_sm().text_color(rgb(0xaaaaaa)).child("No matching tabs")))
                    .children(matches.into_iter().map(|index| {
                        let terminal = &self.terminals[index];
                        let id = terminal.entity_id();
                        let title: SharedString = self.tab_title(index, cx).into();
                        let directory: Option<SharedString> = terminal.read(cx).snapshot_directory.as_ref()
                            .map(|path| path.to_string_lossy().into_owned().into());
                        let hint: SharedString = directory.as_ref().map(|directory| format!("{title}\n{directory}").into()).unwrap_or_else(|| title.clone());
                        div().id(("choose-tab", id)).flex().items_center().gap_2().px_3().py_2().cursor_pointer()
                            .tooltip(move |_, cx| cx.new(|_| ControlHint(hint.clone())).into())
                            .when(Some(id) == selected, |row| row.bg(rgb(0x354969)))
                            .hover(|row| row.bg(rgb(0x40516c)))
                            .child(div().w(px(16.)).flex_shrink_0().child(if index == self.active { "✓" } else { "" }))
                            .child(div().w(px(24.)).flex_shrink_0().text_xs().text_color(rgb(0xaaaaaa)).child((index + 1).to_string()))
                            .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                                .child(div().truncate().child(title))
                                .when_some(directory, |column, directory| column.child(div().text_xs().text_color(rgb(0xaaaaaa)).truncate().child(directory))))
                            .when(terminal.read(cx).attention.any(), |row| row.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(rgb(0xf0b85b))))
                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                if !event.standard_click() { return; }
                                cx.stop_propagation();
                                if let Some(index) = this.terminals.iter().position(|terminal| terminal.entity_id() == id) { this.select(index, window, cx); }
                            }))
                    }))))).with_priority(1))
            })
    }
}
