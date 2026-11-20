use super::*;

pub(super) fn top_inset(height: f32) -> f32 {
    if height >= 480. { 48. } else { 8. }
}

fn result_row_height(has_subtitle: bool) -> f32 {
    if has_subtitle { 56. } else { 36. }
}
fn result_area_height(rows: impl IntoIterator<Item = bool>) -> f32 {
    let mut height = 0.;
    for subtitle in rows {
        height += result_row_height(subtitle);
        if height >= 320. { return 320.; }
    }
    height
}

pub(super) fn navigated_result(selected: usize, count: usize, key: &str) -> Option<usize> {
    if count == 0 { return None; }
    let selected = selected.min(count - 1);
    match key {
        "up" => Some(if selected == 0 { count - 1 } else { selected - 1 }),
        "down" => Some(if selected + 1 == count { 0 } else { selected + 1 }),
        "pageup" => Some(selected.saturating_sub(8)),
        "pagedown" => Some(selected.saturating_add(8).min(count - 1)),
        _ => None,
    }
}

// Keep the destination within one viewport of the starting row. Always advance
// at least one result, including in a window shorter than a single row.
pub(super) fn paged_result(selected: usize, count: usize, backwards: bool, viewport: f32, row_height: impl Fn(usize) -> f32) -> Option<usize> {
    if count == 0 { return None; }
    let mut next = selected.min(count - 1);
    let mut occupied = row_height(next);
    loop {
        let candidate = if backwards { next.checked_sub(1) } else { next.checked_add(1).filter(|index| *index < count) };
        let Some(candidate) = candidate else { break; };
        occupied += row_height(candidate);
        if occupied > viewport && next != selected.min(count - 1) { break; }
        next = candidate;
        if occupied >= viewport { break; }
    }
    Some(next)
}

#[cfg(test)]
mod navigation_tests {
    use super::{navigated_result, paged_result, result_area_height};
    #[test]
    fn paging_respects_visible_height_and_mixed_rows() {
        assert_eq!(paged_result(0, 20, false, 320., |_| 36.), Some(7));
        assert_eq!(paged_result(0, 20, false, 320., |_| 56.), Some(4));
        assert_eq!(paged_result(8, 20, true, 320., |_| 56.), Some(4));
        let heights = [36., 56., 36., 56.];
        assert_eq!(paged_result(0, 4, false, 128., |i| heights[i]), Some(2));
        assert_eq!(paged_result(2, 4, true, 128., |i| heights[i]), Some(0));
        assert_eq!(paged_result(1, 4, false, 20., |i| heights[i]), Some(2));
        assert_eq!(paged_result(3, 4, false, 320., |i| heights[i]), Some(3));
        assert_eq!(paged_result(99, 4, true, 20., |i| heights[i]), Some(2));
        assert_eq!(paged_result(0, 0, false, 320., |_| panic!("empty")), None);
    }
    #[test]
    fn palette_result_height_is_stable_and_bounded() {
        assert_eq!(result_area_height([]), 0.);
        assert_eq!(result_area_height([false]), 36.);
        assert_eq!(result_area_height([false, true]), 92.);
        assert_eq!(result_area_height(std::iter::repeat(false)), 320.);
        assert_eq!(result_area_height(std::iter::repeat(true)), 320.);
    }
    #[test]
    fn arrows_wrap_and_page_keys_clamp() {
        assert_eq!(navigated_result(0, 0, "down"), None);
        assert_eq!(navigated_result(0, 20, "up"), Some(19));
        assert_eq!(navigated_result(19, 20, "down"), Some(0));
        assert_eq!(navigated_result(0, 20, "pagedown"), Some(8));
        assert_eq!(navigated_result(17, 20, "pagedown"), Some(19));
        assert_eq!(navigated_result(3, 20, "pageup"), Some(0));
        assert_eq!(navigated_result(99, 2, "pageup"), Some(0));
        assert_eq!(navigated_result(0, 1, "pagedown"), Some(0));
        assert_eq!(navigated_result(0, 20, "home"), None);
    }
}

struct Command {
    title: String,
    subtitle: Option<String>,
    target: PaletteEvent,
    shortcut: Option<String>,
}
pub enum PaletteEvent {
    Run(Box<dyn Action>),
    SelectWorkspace(EntityId),
    SelectWindowWorkspace(AnyWindowHandle, EntityId),
    SelectTerminal(EntityId, EntityId),
    SelectWindowTerminal(AnyWindowHandle, EntityId, EntityId),
    MoveTabToWorkspace(EntityId),
    MoveTabToWindowWorkspace(AnyWindowHandle, EntityId),
    MoveWorkspaceToWindow(EntityId, AnyWindowHandle),
    MergeWorkspaces(EntityId, EntityId, AnyWindowHandle),
    Close,
}
#[derive(Clone, Copy, PartialEq)]
pub enum BufferMode { Paste, Delete, Rename, Copy }

pub struct CommandPalette {
    buffer_rename: Option<(String, u64)>,
    input: Entity<text_field::TextField>,
    heading: &'static str,
    empty_message: &'static str,
    commands: Vec<Command>,
    search: palette_match::RankedSearch,
    selected: usize,
    scroll: ListState,
}
impl EventEmitter<PaletteEvent> for CommandPalette {}
impl CommandPalette {
    pub fn new(
        workspaces: Vec<(EntityId, String)>,
        terminals: Vec<(Option<AnyWindowHandle>, EntityId, EntityId, String, Option<String>)>,
        windows: Vec<(AnyWindowHandle, String)>,
        external_workspaces: Vec<(AnyWindowHandle, EntityId, String)>,
        groups: Vec<(u64, String)>,
        active_workspace: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let origin_focus = window.focused(cx);
        let mut commands = Vec::new();
        macro_rules! command {
            ($title:literal, $action:expr) => {
                commands.push(Command {
                    title: $title.into(),
                    target: PaletteEvent::Run(Box::new($action)),
                    shortcut: None,
                subtitle: None,
                });
            };
        }
        command!("New Workspace", NewWorkspace);
        command!("Show / Refresh Terminal Memory", RefreshTerminalMemory);
        command!("Cycle Terminal Memory Sort", CycleTerminalMemorySort);
        command!("New Empty Workspace Group", NewWorkspaceGroup);
        command!("New Group from Current Workspace", GroupCurrentWorkspace);
        command!("Group Selected Workspaces", GroupSelectedWorkspaces);
        command!("Use Current Terminal Directory for Group", UseCurrentDirectoryForGroup);
        command!("Reset Group Directory to Follow Anchor", ResetGroupDirectory);
        command!("Rename Workspace Group", RenameWorkspaceGroup);
        command!("Edit Group Color", EditGroupColor);
        command!("Edit Group Icon", EditGroupIcon);
        command!("Ungroup Workspaces", UngroupWorkspaces);
        command!("Close Workspace Group…", CloseWorkspaceGroup);
        command!("Pin / Unpin Workspace Group", ToggleGroupPin);
        command!("Remove Workspace from Group", RemoveWorkspaceFromGroup);
        command!("Use Workspace as Group Anchor", SetGroupAnchor);
        for (group, name) in groups {
            commands.push(Command {
                title: format!("Move Workspace to Group: {name} (#{group})"),
                target: PaletteEvent::Run(Box::new(MoveWorkspaceToGroup { group })),
                shortcut: None,
                subtitle: None,
            });
        }
        command!("New Terminal Tab", NewTab);
        command!("Cancel New Terminal Tab", CancelNewTab);
        command!("Cancel Queued Paste", CancelQueuedPaste);
        command!("Cancel Split Terminal", CancelSplit);
        command!("New Window", NewWindow);
        command!("Split Right", Split);
        command!("Split Below", SplitBelow);
        command!("Split Left", SplitLeft);
        command!("Split Above", SplitAbove);
        command!("Toggle Pane Zoom", TogglePaneZoom);
        command!("Equalize Pane Sizes", EqualizePanes);
        command!("Resize Pane Left", ResizePaneLeft);
        command!("Resize Pane Right", ResizePaneRight);
        command!("Resize Pane Up", ResizePaneUp);
        command!("Resize Pane Down", ResizePaneDown);
        command!("Swap Pane with Next", SwapPaneNext);
        command!("Swap Pane Left", SwapPaneLeft);
        command!("Swap Pane Right", SwapPaneRight);
        command!("Swap Pane Up", SwapPaneUp);
        command!("Swap Pane Down", SwapPaneDown);
        command!("Move Tab to Split Right", BreakTabRight);
        command!("Move Tab to Split Below", BreakTabBelow);
        command!("Move Tab to Split Left", BreakTabLeft);
        command!("Move Tab to Split Above", BreakTabAbove);
        command!("Join Pane into Next", JoinPaneNext);
        command!("Join Pane Left", JoinPaneLeft);
        command!("Join Pane Right", JoinPaneRight);
        command!("Join Pane Up", JoinPaneUp);
        command!("Join Pane Down", JoinPaneDown);
        command!("Move Tab to Next Workspace", MoveTabNextWorkspace);
        command!("Detach Tab to New Window", DetachTab);
        command!("Move Workspace to New Window", DetachWorkspace);
        command!("Merge Workspace into…", ShowWorkspaceMergePicker);
        command!("Move Workspace to Window…", ShowWorkspaceWindowPicker);
        command!("Move Workspace to Next Window", MoveWorkspaceToNextWindow);
        command!("Toggle Sidebar", ToggleSidebar);
        command!("Rename Workspace", RenameWorkspace);
        command!("Edit Workspace Icon", EditWorkspaceIcon);
        command!("Edit Workspace Color", EditWorkspaceColor);
        command!("Edit Workspace Description", EditWorkspaceDescription);
        command!("Next Workspace", NextWorkspace);
        command!("Last Workspace", LastWorkspace);
        command!("Last Terminal", LastTerminal);
        command!("Rename Tab…", RenameTab);
        command!("Next Tab", NextTab);
        command!("Choose Terminal Tab", ShowTabPicker);
        command!("Show Tab Actions", ShowTabMenu);
        command!("Show Workspace Actions", ShowWorkspaceMenu);
        command!("Previous Tab", PreviousTab);
        command!("Pin / Unpin Workspace", ToggleWorkspacePin);
        command!("Previous Workspace", PreviousWorkspace);
        command!("Next Unread Terminal", NextUnreadTerminal);
        command!("Mark All Read in This Window", MarkAllRead);
        command!("Toggle Notifications", ToggleNotifications);
        command!("Dismiss All Notifications", DismissAllNotifications);
        command!("Refresh Git Branch for Current Terminal", RefreshGitContext);
        command!("Toggle Program Status", ToggleProgramStatus);
        command!("Show Listening Ports", ShowListeningPorts);
        command!("Close Listening Ports", CloseListeningPorts);
        command!("Find in Terminal", Find);
        command!("Find Older Match", FindOlder);
        command!("Find Newer Match", FindNewer);
        command!("Select All", EditorSelectAll);
        command!("Terminal Actions", ShowTerminalMenu);
        command!("Copy with Line Breaks", CopySelectionWithLineBreaks);
        command!("Copy Working Directory", CopyWorkingDirectory);
        command!("Reveal Working Directory", RevealWorkingDirectory);
        command!("Clear Scrollback", ClearScrollback);
        command!("Previous Prompt", PreviousPrompt);
        command!("Next Prompt", NextPrompt);
        command!("Last Prompt", LastPrompt);
        command!("Scroll to Top", ScrollToTop);
        command!("Scroll to Bottom", ScrollToBottom);
        command!("Increase Font Size", IncreaseFontSize);
        command!("Decrease Font Size", DecreaseFontSize);
        command!("Reset Font Size", ResetFontSize);
        command!("Enter Copy Mode", EnterCopyMode);
        command!("Paste Named Buffer…", ShowBufferPicker);
        command!("Delete Named Buffer…", ShowDeleteBufferPicker);
        command!("Rename Named Buffer…", ShowRenameBufferPicker);
        command!("Copy Named Buffer…", ShowCopyBufferPicker);
        command!("Choose Terminal Font…", ShowFontPicker);
        command!("Choose Default Font…", ShowDefaultFontPicker);
        command!("Use Configured Font", UseConfiguredFont);
        command!("Save Layout…", SaveLayout);
        command!("Save Session…", SaveSession);
        command!("Open Session in New Windows…", OpenSession);
        command!("Open Layout Preset…", ShowLayoutPresetPicker);
        command!("Run Configured Command…", ShowCustomCommandPicker);
        command!("Cancel Layout / Session Restore", CancelRestore);
        command!("Settings", ToggleSettings);
        command!("Save Appearance Settings", SaveAppearanceSettings);
        command!("Reload Settings", ReloadSettings);
        command!("Open Settings File", OpenSettingsFile);
        command!("Open Layout in New Window…", OpenLayout);
        command!("Focus Pane Left", FocusLeft);
        command!("Focus Pane Right", FocusRight);
        command!("Focus Pane Up", FocusUp);
        command!("Focus Pane Down", FocusDown);
        command!("Move Workspace Up", MoveWorkspaceUp);
        command!("Move Workspace Down", MoveWorkspaceDown);
        command!("Close Terminal Tab", ClosePane);
        command!("Close Other Terminal Tabs…", CloseOtherTabs);
        command!("Close Terminal Tabs to the Right…", CloseTabsToRight);
        command!("Close Finished Terminal Tabs…", CloseFinishedTabs);
        command!("Close Workspace", CloseWorkspace);
        command!("Close Window", CloseWindow);
        for theme in preferences::TerminalTheme::ALL {
            commands.push(Command {
                title: format!("Terminal Theme: {}", theme.label()),
                target: PaletteEvent::Run(Box::new(SetTerminalTheme { theme })),
                shortcut: None,
                subtitle: Some("Applies to this session; explicit color overrides stay active".into()),
            });
        }
        for (name, icon) in WORKSPACE_ICONS {
            commands.push(Command {
                title: format!("Workspace Icon: {name}"),
                target: PaletteEvent::Run(Box::new(SetWorkspaceIcon {
                    icon: icon.map(str::to_owned),
                })),
                shortcut: None,
                subtitle: None,
            });
        }
        for (name, color) in WORKSPACE_COLORS {
            commands.push(Command {
                title: format!("Workspace Color: {name}"),
                target: PaletteEvent::Run(Box::new(SetWorkspaceColor { color: *color })),
                shortcut: None,
                subtitle: None,
            });
        }
        for number in 1..=9 {
            commands.push(Command {
                title: format!("Select Tab {number}"),
                target: PaletteEvent::Run(Box::new(SelectTabNumber { number })),
                shortcut: None,
                subtitle: None,
            });
            commands.push(Command {
                title: format!("Focus Pane {number}"),
                target: PaletteEvent::Run(Box::new(SelectPaneNumber { number })),
                shortcut: None,
                subtitle: None,
            });
        }
        commands.extend(cx.global::<Preferences>().commands.iter().map(custom_command));
        commands.extend(cx.global::<Preferences>().layout_presets.iter().map(preset_command));
        if let Some(focus) = origin_focus {
            for command in &mut commands {
                let PaletteEvent::Run(action) = &command.target else {
                    continue;
                };
                command.shortcut = shortcuts::hint(action.as_ref(), &focus, window);
            }
        }
        for (index, (id, name)) in workspaces.into_iter().enumerate() {
            commands.push(Command {
                title: format!("Switch to Workspace {}: {name}", index + 1),
                target: PaletteEvent::SelectWorkspace(id),
                shortcut: None,
                subtitle: None,
            });
            if id != active_workspace {
                commands.push(Command {
                    title: format!("Move Tab to Workspace {}: {name}", index + 1),
                    target: PaletteEvent::MoveTabToWorkspace(id),
                    shortcut: None,
                subtitle: None,
                });
            }
        }
        for (destination, workspace, title) in external_workspaces {
            commands.push(Command {
                title: format!("Switch to {title}"),
                target: PaletteEvent::SelectWindowWorkspace(destination, workspace),
                shortcut: None,
                subtitle: None,
            });
            commands.push(Command {
                title: format!("Move Tab to {title}"),
                target: PaletteEvent::MoveTabToWindowWorkspace(destination, workspace),
                shortcut: None,
                subtitle: None,
            });
        }
        for (destination, title) in windows {
            commands.push(Command {
                title: format!("Move Workspace to {title}"),
                target: PaletteEvent::MoveWorkspaceToWindow(active_workspace, destination),
                shortcut: None,
                subtitle: None,
            });
        }
        for (destination, workspace, terminal, title, directory) in terminals {
            commands.push(Command {
                title: format!("Switch to Terminal: {title}"),
                target: match destination { Some(window) => PaletteEvent::SelectWindowTerminal(window, workspace, terminal), None => PaletteEvent::SelectTerminal(workspace, terminal) },
                shortcut: None,
                subtitle: directory,
            });
        }
        Self::with_commands(commands, "Search commands and workspaces", "No matching commands or workspaces", window, cx)
    }
    pub fn buffers(entries: Vec<(String, usize, String)>, revision: u64, surface_id: String, mode: BufferMode, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let commands = entries.into_iter().map(|(name, bytes, preview)| Command {
            title: name.clone(), subtitle: Some(format!("{bytes} bytes · {preview}")), shortcut: None,
            target: PaletteEvent::Run(if mode == BufferMode::Rename { Box::new(RenameNamedBuffer { name, revision }) }
                else if mode == BufferMode::Delete { Box::new(DeleteNamedBuffer { name, revision }) }
                else if mode == BufferMode::Copy { Box::new(CopyNamedBuffer { name, revision }) }
                else { Box::new(PasteNamedBuffer { name, revision, surface_id: surface_id.clone() }) }),
        }).collect();
        Self::with_commands(commands, match mode { BufferMode::Paste => "Paste named buffer", BufferMode::Delete => "Delete named buffer", BufferMode::Rename => "Rename named buffer", BufferMode::Copy => "Copy buffer to clipboard" }, "No matching buffers · Save a selection in Copy Mode first", window, cx)
    }
    pub fn custom_commands(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut commands: Vec<_> = cx.global::<Preferences>().commands.iter().map(custom_command).collect();
        let empty = if commands.is_empty() { "No commands · Add commands in Settings, then Reload Settings" } else { "No matching configured commands" };
        if let Some(focus) = window.focused(cx) {
            for command in &mut commands {
                if let PaletteEvent::Run(action) = &command.target { command.shortcut = shortcuts::hint(action.as_ref(), &focus, window); }
            }
        }
        Self::with_commands(commands, "Run configured command · Opens a new workspace", empty, window, cx)
    }
    pub fn layout_presets(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut commands: Vec<_> = cx.global::<Preferences>().layout_presets.iter().map(preset_command).collect();
        let empty = if commands.is_empty() { "No presets · Add layout_presets in Settings, then Reload Settings" } else { "No matching presets" };
        if let Some(focus) = window.focused(cx) {
            for command in &mut commands {
                if let PaletteEvent::Run(action) = &command.target { command.shortcut = shortcuts::hint(action.as_ref(), &focus, window); }
            }
        }
        Self::with_commands(commands, "Open layout preset · Fresh shells in new windows", empty, window, cx)
    }
    pub fn workspace_merge(entries: Vec<(AnyWindowHandle, EntityId, String)>, source: EntityId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let empty = if entries.is_empty() { "No other workspaces available" } else { "No matching workspaces" };
        let commands = entries.into_iter().map(|(target, destination, title)| Command {
            title, subtitle: Some("Keep destination metadata · Add source splits on the right".into()), shortcut: None,
            target: PaletteEvent::MergeWorkspaces(source, destination, target),
        }).collect();
        Self::with_commands(commands, "Merge workspace into…", empty, window, cx)
    }
    pub fn workspace_windows(entries: Vec<(AnyWindowHandle, String, String)>, source: EntityId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let empty = if entries.is_empty() { "No other windows · Open a new window first" } else { "No matching windows" };
        let commands = entries.into_iter().map(|(destination, title, subtitle)| Command {
            title, subtitle: Some(subtitle), shortcut: None,
            target: PaletteEvent::MoveWorkspaceToWindow(source, destination),
        }).collect();
        Self::with_commands(commands, "Move workspace to window", empty, window, cx)
    }
    pub fn fonts(mut families: Vec<String>, default: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        families.retain(|name| !name.trim().is_empty() && name.len() <= 128 && !name.chars().any(char::is_control) && !name.starts_with('.'));
        families.sort_unstable();
        families.dedup();
        let limited = families.len() > 2048;
        families.truncate(2048);
        let commands = families.into_iter().map(|family| Command {
            title: family.clone(), subtitle: None, shortcut: None,
            target: PaletteEvent::Run(if default { Box::new(SetDefaultFont { family }) } else { Box::new(SetTerminalFont { family }) }),
        }).collect();
        Self::with_commands(commands, match (default, limited) {
            (true, true) => "Choose default font · First 2,048 families",
            (true, false) => "Choose default font · Save in Settings to keep",
            (false, true) => "Choose terminal font · First 2,048 families",
            (false, false) => "Choose font for this terminal",
        }, "No matching installed fonts", window, cx)
    }
    fn with_commands(commands: Vec<Command>, heading: &'static str, empty_message: &'static str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| text_field::TextField::new("", cx));
        input.read(cx).focus.focus(window);
        cx.subscribe(
            &input,
            |this, _, event: &text_field::EditEvent, cx| match event {
                text_field::EditEvent::Changed(query) => {
                    if this.buffer_rename.is_some() {
                        this.empty_message = "Enter a unique buffer name (up to 128 bytes)";
                        cx.notify();
                        return;
                    }
                    this.search.update(query);
                    this.selected = 0;
                    this.scroll.reset(this.search.matches.len());
                    cx.notify();
                }
                text_field::EditEvent::Submitted { .. } => this.run(cx),
                text_field::EditEvent::Cancelled => cx.emit(PaletteEvent::Close),
            },
        )
        .detach();
        Self {
            input, heading, empty_message, buffer_rename: None,
            search: palette_match::RankedSearch::new(commands
                .iter()
                .map(|command| format!("{} {} {}", command.title, command.subtitle.as_deref().unwrap_or_default(),
                    command.shortcut.as_deref().map(shortcuts::search_terms).unwrap_or_default()).to_lowercase())
                .collect()),
            scroll: ListState::new(commands.len(), ListAlignment::Top, px(96.)),
            commands,
            selected: 0,
        }
    }
    fn render_result(&mut self, row: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(&command) = self.search.matches.get(row) else { return div().into_any_element(); };
        let selected = row == self.selected;
        let title_hint: SharedString = match &self.commands[command].subtitle {
            Some(subtitle) => format!("{}\n{subtitle}", self.commands[command].title).into(),
            None => self.commands[command].title.clone().into(),
        };
        div()
            .id(("command", command))
            .px_3()
            .h(px(result_row_height(self.commands[command].subtitle.is_some())))
            .text_sm().line_height(px(20.))
            .flex()
            .items_center()
            .gap_3()
            .cursor_pointer()
            .when(selected, |row| row.bg(rgb(0x354969)))
            .hover(move |row| row.bg(rgb(if selected { 0x40577b } else { 0x303b50 })))
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if !event.standard_click() { return; }
                let Some(row) = this.search.matches.iter().position(|index| *index == command) else { return; };
                this.selected = row;
                this.run(cx);
            }))
            .child(
                div()
                    .id(("command-title", command))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .tooltip(move |_, cx| cx.new(|_| pane_view::ControlHint(title_hint.clone())).into())
                    .child(div().flex().flex_col().gap_1()
                        .child(div().truncate().child(self.commands[command].title.clone()))
                        .when_some(self.commands[command].subtitle.clone(), |column, subtitle| column.child(div().truncate().text_xs().line_height(px(16.)).text_color(rgb(0xa0a0a0)).child(subtitle)))),
            )
            .when_some(self.commands[command].shortcut.clone(), |row, shortcut| {
                row.child(
                    div().id(("command-shortcut", command))
                        .flex_shrink_0().max_w(relative(0.4)).truncate()
                        .text_xs()
                        .text_color(rgb(0xa0a0a0))
                        .tooltip({ let hint: SharedString = shortcut.clone().into(); move |_, cx| cx.new(|_| pane_view::ControlHint(hint.clone())).into() })
                        .child(shortcut),
                )
            }).into_any_element()
    }
    fn run(&mut self, cx: &mut Context<Self>) {
        if let Some((name, revision)) = &self.buffer_rename {
            let next = self.input.read(cx).value().to_owned();
            match cx.global_mut::<copy_buffers::Store>().rename_if_unchanged(name, next, *revision) {
                Ok(_) => cx.emit(PaletteEvent::Close),
                Err(error) => { self.empty_message = error; cx.notify(); }
            }
            return;
        }
        let rename = self.search.matches.get(self.selected)
            .and_then(|index| match &self.commands[*index].target {
                PaletteEvent::Run(action) => action.as_any().downcast_ref::<RenameNamedBuffer>().cloned(),
                _ => None,
            });
        if let Some(rename) = rename {
            self.heading = "Rename buffer · Enter to save · Escape to cancel";
            self.empty_message = "Enter a unique buffer name (up to 128 bytes)";
            self.buffer_rename = Some((rename.name.clone(), rename.revision));
            self.commands.clear();
            self.search = palette_match::RankedSearch::new(Vec::new());
            self.scroll.reset(0);
            self.input.update(cx, |input, cx| {
                input.replace_draft(&rename.name, cx);
                input.select_all_text(cx);
            });
            cx.notify();
            return;
        }
        if let Some(index) = self.search.matches.get(self.selected) {
            cx.emit(match &self.commands[*index].target {
                PaletteEvent::Run(action) => PaletteEvent::Run(action.boxed_clone()),
                PaletteEvent::SelectWorkspace(id) => PaletteEvent::SelectWorkspace(*id),
                PaletteEvent::SelectWindowWorkspace(window, workspace) => PaletteEvent::SelectWindowWorkspace(*window, *workspace),
                PaletteEvent::SelectTerminal(workspace, terminal) => {
                    PaletteEvent::SelectTerminal(*workspace, *terminal)
                }
                PaletteEvent::SelectWindowTerminal(window, workspace, terminal) => PaletteEvent::SelectWindowTerminal(*window, *workspace, *terminal),
                PaletteEvent::MoveTabToWorkspace(id) => PaletteEvent::MoveTabToWorkspace(*id),
                PaletteEvent::MoveTabToWindowWorkspace(window, workspace) => PaletteEvent::MoveTabToWindowWorkspace(*window, *workspace),
                PaletteEvent::MergeWorkspaces(source, destination, target) => PaletteEvent::MergeWorkspaces(*source, *destination, *target),
                PaletteEvent::MoveWorkspaceToWindow(source, destination) => {
                    PaletteEvent::MoveWorkspaceToWindow(*source, *destination)
                }
                PaletteEvent::Close => PaletteEvent::Close,
            });
        }
    }
}
impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = f32::from(window.viewport_size().height);
        let compact = height < 300.;
        div()
            .id("command-palette")
            .w(px(520.))
            .max_w(relative(0.9))
            .h_auto()
            .max_h(px((height - top_inset(height) - 8.).max(0.)))
            .overflow_hidden()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x454545))
            .bg(rgb(0x252525))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.input.read(cx).is_composing() {
                    return;
                }
                let key = event.keystroke.key.as_str();
                let selected = if matches!(key, "pageup" | "pagedown") {
                    paged_result(this.selected, this.search.matches.len(), key == "pageup",
                        f32::from(this.scroll.viewport_bounds().size.height),
                        |row| result_row_height(this.commands[this.search.matches[row]].subtitle.is_some()))
                } else {
                    navigated_result(this.selected, this.search.matches.len(), key)
                };
                if let Some(selected) = selected {
                    this.selected = selected;
                    crate::list_navigation::reveal(&this.scroll, this.selected);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .when(!compact, |palette| palette.child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .pt_2()
                    .text_xs()
                    .text_color(rgb(0xa0a0a0))
                    .child(self.heading),
            ))
            .child(div().flex_shrink_0().p_3().child(self.input.clone()))
            .child(if self.search.matches.is_empty() {
                div().p_3().text_sm().child(self.empty_message).into_any_element()
            } else {
                let entity = cx.entity();
                list(self.scroll.clone(), move |row, _, cx| {
                    entity.update(cx, |palette, cx| palette.render_result(row, cx))
                })
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .w_full().min_h_0()
                .h(px(result_area_height(self.search.matches.iter().map(|index| self.commands[*index].subtitle.is_some()))))
                .into_any_element()
            })
            .when(!compact, |palette| palette.child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0xa0a0a0))
                    .child("↑↓ select · PgUp/PgDn jump · Enter run · Esc close"),
            ))
    }
}

fn custom_command(command: &preferences::CustomCommand) -> Command {
    // Quote ambiguous arguments for display only; launching always uses literal argv.
    // Keep long definitions bounded in tooltips and omit environment values.
    let argv = command.argv.iter().map(|arg| {
        if arg.is_empty() || arg.chars().any(|ch| ch.is_whitespace() || ch.is_control() || ch == '"' || ch == '\\') {
            serde_json::to_string(arg).expect("string argument serializes")
        } else { arg.clone() }
    }).collect::<Vec<_>>().join(" ");
    let directory = command.cwd.as_ref().map(|cwd| cwd.display().to_string())
        .unwrap_or_else(|| "Current terminal directory".into());
    let detail = format!("{directory} · {argv}");
    let mut chars = detail.chars().map(|ch| if ch.is_control() { ' ' } else { ch });
    let mut subtitle: String = chars.by_ref().take(768).collect();
    if chars.next().is_some() { subtitle.push('…'); }
    Command {
        title: format!("Run {}", command.name),
        subtitle: Some(subtitle),
        target: PaletteEvent::Run(Box::new(RunCustomCommand { name: command.name.clone() })),
        shortcut: None,
    }
}

fn preset_command(preset: &preferences::LayoutPreset) -> Command {
    let directory = preset.cwd.as_ref().map(|path| format!(" · Project {}", path.display())).unwrap_or_default();
    Command {
        title: format!("Open Preset: {}", preset.name),
        subtitle: Some(format!("{}{} · {}", if preset.session { "Session" } else { "Layout" }, directory, preset.path.display())),
        target: PaletteEvent::Run(Box::new(OpenLayoutPreset { name: preset.name.clone() })),
        shortcut: None,
    }
}
