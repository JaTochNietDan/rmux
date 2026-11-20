use super::*;

pub(super) fn top_inset(height: f32) -> f32 {
    if height >= 480. { 48. } else { 8. }
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

#[cfg(test)]
mod navigation_tests {
    use super::navigated_result;
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
    Close,
}
pub struct CommandPalette {
    input: Entity<text_field::TextField>,
    commands: Vec<Command>,
    search_titles: Vec<String>,
    matches: Vec<usize>,
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
        let input = cx.new(|cx| text_field::TextField::new("", cx));
        input.read(cx).focus.focus(window);
        cx.subscribe(
            &input,
            |this, _, event: &text_field::EditEvent, cx| match event {
                text_field::EditEvent::Changed(query) => {
                    let normalized = query.to_lowercase();
                    let words: Vec<_> = normalized.split_whitespace().collect();
                    let mut ranked: Vec<_> = this
                        .search_titles
                        .iter()
                        .enumerate()
                        .filter_map(|(index, title)| {
                            palette_match::score(title, &words).map(|score| (score, index))
                        })
                        .collect();
                    ranked.sort_unstable();
                    this.matches = ranked.into_iter().map(|(_, index)| index).collect();
                    this.selected = 0;
                    this.scroll.reset(this.matches.len());
                    cx.notify();
                }
                text_field::EditEvent::Submitted { .. } => this.run(cx),
                text_field::EditEvent::Cancelled => cx.emit(PaletteEvent::Close),
            },
        )
        .detach();
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
        command!("New Window", NewWindow);
        command!("Split Right", Split);
        command!("Split Below", SplitBelow);
        command!("Toggle Pane Zoom", TogglePaneZoom);
        command!("Equalize Pane Sizes", EqualizePanes);
        command!("Swap Pane with Next", SwapPaneNext);
        command!("Move Tab to Split Right", BreakTabRight);
        command!("Move Tab to Split Below", BreakTabBelow);
        command!("Join Pane into Next", JoinPaneNext);
        command!("Move Tab to Next Workspace", MoveTabNextWorkspace);
        command!("Detach Tab to New Window", DetachTab);
        command!("Move Workspace to New Window", DetachWorkspace);
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
        command!("Previous Tab", PreviousTab);
        command!("Pin / Unpin Workspace", ToggleWorkspacePin);
        command!("Previous Workspace", PreviousWorkspace);
        command!("Next Unread Terminal", NextUnreadTerminal);
        command!("Mark All Read in This Window", MarkAllRead);
        command!("Toggle Notifications", ToggleNotifications);
        command!("Dismiss All Notifications", DismissAllNotifications);
        command!("Refresh Git Branch for Current Terminal", RefreshGitContext);
        command!("Toggle Program Status", ToggleProgramStatus);
        command!("Find in Terminal", Find);
        command!("Find Older Match", FindOlder);
        command!("Find Newer Match", FindNewer);
        command!("Select All", EditorSelectAll);
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
        command!("Use Configured Font", UseConfiguredFont);
        command!("Save Layout…", SaveLayout);
        command!("Save Session…", SaveSession);
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
        command!("Close Workspace", CloseWorkspace);
        command!("Close Window", CloseWindow);
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
        for command in &cx.global::<Preferences>().commands {
            commands.push(Command {
                title: format!("Run {}", command.name),
                subtitle: Some(command.argv.join(" ").chars().map(|ch| if ch.is_control() { ' ' } else { ch }).take(256).collect()),
                target: PaletteEvent::Run(Box::new(RunCustomCommand { name: command.name.clone() })),
                shortcut: None,
            });
        }
        if let Some(focus) = origin_focus {
            for command in &mut commands {
                let PaletteEvent::Run(action) = &command.target else {
                    continue;
                };
                command.shortcut = window
                    .highest_precedence_binding_for_action_in(action.as_ref(), &focus)
                    .map(|binding| {
                        binding
                            .keystrokes()
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" ")
                    });
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
        Self {
            input,
            matches: (0..commands.len()).collect(),
            search_titles: commands
                .iter()
                .map(|command| format!("{} {}", command.title, command.subtitle.as_deref().unwrap_or_default()).to_lowercase())
                .collect(),
            scroll: ListState::new(commands.len(), ListAlignment::Top, px(96.)),
            commands,
            selected: 0,
        }
    }
    fn render_result(&mut self, row: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(&command) = self.matches.get(row) else { return div().into_any_element(); };
        let selected = row == self.selected;
        let title_hint: SharedString = match &self.commands[command].subtitle {
            Some(subtitle) => format!("{}\n{subtitle}", self.commands[command].title).into(),
            None => self.commands[command].title.clone().into(),
        };
        div()
            .id(("command", command))
            .px_3()
            .py_2()
            .text_sm()
            .flex()
            .items_center()
            .gap_3()
            .cursor_pointer()
            .when(selected, |row| row.bg(rgb(0x354969)))
            .hover(move |row| row.bg(rgb(if selected { 0x40577b } else { 0x303b50 })))
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if !event.standard_click() { return; }
                let Some(row) = this.matches.iter().position(|index| *index == command) else { return; };
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
                        .when_some(self.commands[command].subtitle.clone(), |column, subtitle| column.child(div().truncate().text_xs().text_color(rgb(0xa0a0a0)).child(subtitle)))),
            )
            .when_some(self.commands[command].shortcut.clone(), |row, shortcut| {
                row.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(rgb(0xa0a0a0))
                        .child(shortcut),
                )
            }).into_any_element()
    }
    fn run(&self, cx: &mut Context<Self>) {
        if let Some(index) = self.matches.get(self.selected) {
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
                if let Some(selected) = navigated_result(this.selected, this.matches.len(), event.keystroke.key.as_str()) {
                    this.selected = selected;
                    this.scroll.scroll_to_reveal_item(this.selected);
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
                    .child("Search commands and workspaces"),
            ))
            .child(div().flex_shrink_0().p_3().child(self.input.clone()))
            .child(if self.matches.is_empty() {
                div().p_3().text_sm().child("No matching commands or workspaces").into_any_element()
            } else {
                let entity = cx.entity();
                list(self.scroll.clone(), move |row, _, cx| {
                    entity.update(cx, |palette, cx| palette.render_result(row, cx))
                })
                .with_sizing_behavior(ListSizingBehavior::Infer)
                .w_full().min_h_0().max_h(px(320.)).into_any_element()
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
