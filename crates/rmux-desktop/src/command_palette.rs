use super::*;

pub(super) fn top_inset(height: f32) -> f32 {
    if height >= 480. { 48. } else { 8. }
}

fn result_row_height(has_subtitle: bool) -> f32 {
    if has_subtitle { 56. } else { 36. }
}
fn literal_highlights(text: &str, query: &str) -> Vec<std::ops::Range<usize>> {
    if query.is_empty() { return Vec::new(); }
    // ASCII case folding preserves the original UTF-8 byte boundaries.
    text.to_ascii_lowercase().match_indices(&query.to_ascii_lowercase())
        .map(|(start, matched)| start..start + matched.len()).collect()
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
    fn literal_excerpt_highlights_preserve_unicode_boundaries() {
        let text = "…界 NEEDLE 世界 needle 世界 é É";
        let ranges = super::literal_highlights(text, "needle 世界");
        assert_eq!(ranges.iter().map(|range| &text[range.clone()]).collect::<Vec<_>>(), ["NEEDLE 世界", "needle 世界"]);
        assert_eq!(super::literal_highlights(text, "É").len(), 1);
        assert!(super::literal_highlights(text, "").is_empty());
        assert!(super::literal_highlights(text, "needle absent").is_empty());
    }
    #[test]
    fn preview_search_finds_text_beyond_display_cap_and_keeps_copy_source() {
        let text = format!("{}\tNeedle 世界{}", "İ界".repeat(800), "z".repeat(1400));
        let title = crate::file_browser::preview_line_title(&text, "");
        assert!(!title.contains("Needle"));
        let command = super::Command { title, subtitle: Some("Line 1".into()), shortcut: None,
            target: super::PaletteEvent::Run(Box::new(crate::CopyPreviewLine { text: text.clone() })) };
        let mut search = crate::palette_match::RankedSearch::new(vec![command.search_text()]);
        search.update("NEEDLE 世界");
        assert_eq!(search.matches, [0]);
        let excerpt = crate::file_browser::preview_line_title(command.preview_line().unwrap(), "NEEDLE 世界");
        assert!(excerpt.contains("Needle 世界"));
        assert!(excerpt.starts_with('…') && excerpt.ends_with('…'));
        assert!(excerpt.chars().count() <= 1026);
        assert!(!excerpt.contains('\t'));
        assert_eq!(command.preview_line(), Some(text.as_str()));
        assert_eq!(crate::file_browser::preview_line_title("", ""), "(empty line)");
        assert_eq!(crate::file_browser::preview_line_title("short", "missing"), "short");
    }
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
impl Command {
    fn preview_line(&self) -> Option<&str> {
        match &self.target {
            PaletteEvent::Run(action) => action.as_any().downcast_ref::<CopyPreviewLine>().map(|line| line.text.as_str()),
            _ => None,
        }
    }
    fn search_text(&self) -> String {
        format!("{} {} {}", self.preview_line().filter(|text| !text.is_empty()).unwrap_or(&self.title), self.subtitle.as_deref().unwrap_or_default(),
            self.shortcut.as_deref().map(shortcuts::search_terms).unwrap_or_default()).to_lowercase()
    }
}
pub enum PaletteEvent {
    Run(Box<dyn Action>),
    SelectWorkspace(EntityId),
    SelectWindowWorkspace(AnyWindowHandle, EntityId),
    SelectTerminal(EntityId, EntityId),
    SearchTerminal { window: Option<AnyWindowHandle>, workspace: EntityId, terminal: EntityId, query: String, preview: bool },
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
    location: Option<SharedString>,
    feedback: Option<&'static str>,
    buffer_rename: Option<(String, u64)>,
    directory_input: bool,
    terminal_search_input: bool,
    ssh_input: bool,
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
        command!("Browse Terminal Directory…", BrowseTerminalFiles);
        command!("Open Text Preview…", OpenTextPreview);
        command!("Search Terminal Output…", ShowTerminalSearch);
        command!("Connect via SSH…", ShowSshConnection);
        command!("SSH Hosts…", ShowSshHosts);
        command!("Open Folder…", OpenFolderBrowser);
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
    pub fn files(listing: file_browser::Listing, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let location: SharedString = if listing.unavailable > 0 {
            format!("{} · {} unavailable entries · Refresh to retry", listing.path.display(), listing.unavailable).into()
        } else { listing.path.display().to_string().into() };
        let mut commands = Vec::new();
        commands.push(Command { title: "Open Folder…".into(), subtitle: Some("Choose a local folder".into()), shortcut: None,
            target: PaletteEvent::Run(Box::new(OpenFolderBrowser)) });
        commands.push(Command { title: "Go to Directory…".into(), subtitle: Some(listing.path.display().to_string()), shortcut: None,
            target: PaletteEvent::Run(Box::new(EnterDirectory { path: listing.path.clone() })) });
        commands.push(Command { title: "New Workspace Here".into(), subtitle: Some(listing.path.display().to_string()), shortcut: None,
            target: PaletteEvent::Run(Box::new(NewWorkspaceInDirectory { path: listing.path.clone() })) });
        if let Some(parent) = listing.path.parent() {
            commands.push(Command { title: ".. · Parent directory".into(), subtitle: Some(parent.display().to_string()), shortcut: None,
                target: PaletteEvent::Run(Box::new(BrowseDirectory { path: parent.to_owned() })) });
        }
        commands.push(reveal_local_path(&listing.path));
        commands.push(Command { title: "Copy directory path".into(), subtitle: Some(listing.path.display().to_string()), shortcut: None,
            target: PaletteEvent::Run(Box::new(CopyLocalPath { path: listing.path.clone() })) });
        commands.push(Command { title: "Refresh this directory".into(), subtitle: Some(listing.path.display().to_string()), shortcut: None,
            target: PaletteEvent::Run(Box::new(BrowseDirectory { path: listing.path })) });
        commands.extend(listing.entries.into_iter().map(|entry| Command {
            title: entry.path.file_name().unwrap_or_default().to_string_lossy().chars()
                .map(|ch| if ch.is_control() { ' ' } else { ch }).collect(),
            subtitle: Some(if entry.directory { "Directory · Enter to browse" } else if entry.symlink { "Symbolic link · Enter to copy path" } else { "File · Enter to preview text" }.into()),
            shortcut: None,
            target: PaletteEvent::Run(if entry.directory { Box::new(BrowseDirectory { path: entry.path }) }
                else if entry.symlink { Box::new(CopyLocalPath { path: entry.path }) }
                else { Box::new(PreviewLocalFile { path: entry.path }) }),
        }));
        let mut palette = Self::with_commands(commands, if listing.truncated { "Local files · Limited to 2,048 entries; use a narrower directory" } else { "Local files · Browse directories or preview text" },
            "No matching entries", window, cx);
        palette.location = Some(location);
        palette
    }
    pub fn file_error(path: std::path::PathBuf, error: String, directory: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let location: SharedString = path.display().to_string().into();
        let mut commands = Vec::new();
        commands.push(Command { title: "Open Folder…".into(), subtitle: Some("Choose a different local folder".into()), shortcut: None,
            target: PaletteEvent::Run(Box::new(OpenFolderBrowser)) });
        commands.push(Command { title: "Go to Directory…".into(), subtitle: None, shortcut: None,
            target: PaletteEvent::Run(Box::new(EnterDirectory { path: path.parent().unwrap_or(&path).to_owned() })) });
        if let Some(parent) = path.parent() {
            commands.push(Command { title: "Back to directory".into(), subtitle: Some(parent.display().to_string()), shortcut: None,
                target: PaletteEvent::Run(Box::new(BrowseDirectory { path: parent.to_owned() })) });
        }
        commands.push(Command { title: "Copy path".into(), subtitle: Some(path.display().to_string()), shortcut: None,
            target: PaletteEvent::Run(Box::new(CopyLocalPath { path: path.clone() })) });
        commands.push(reveal_local_path(&path));
        commands.push(Command { title: "Retry".into(), subtitle: Some(error), shortcut: None,
            target: PaletteEvent::Run(if directory { Box::new(BrowseDirectory { path }) } else { Box::new(PreviewLocalFile { path }) }) });
        let mut palette = Self::with_commands(commands, if directory { "Could not read directory" } else { "Could not preview file" }, "Clear the search to see recovery actions", window, cx);
        palette.location = Some(location);
        palette
    }
    pub fn file_preview(preview: file_browser::Preview, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let location: SharedString = preview.path.display().to_string().into();
        let empty = preview.lines.is_empty();
        let mut commands = Vec::new();
        if let Some(parent) = preview.path.parent() {
            commands.push(Command { title: "Back to directory".into(), subtitle: Some(parent.display().to_string()), shortcut: None,
                target: PaletteEvent::Run(Box::new(BrowseDirectory { path: parent.to_owned() })) });
        }
        commands.push(Command { title: "Copy file path".into(), subtitle: Some(preview.path.display().to_string()), shortcut: None,
            target: PaletteEvent::Run(Box::new(CopyLocalPath { path: preview.path.clone() })) });
        commands.push(reveal_local_path(&preview.path));
        commands.push(Command { title: "Copy Preview Text".into(),
            subtitle: Some(if preview.truncated { "Copies captured text only · Preview is incomplete" } else { "Preserves original line endings" }.into()), shortcut: None,
            target: PaletteEvent::Run(Box::new(CopyPreviewText { text: preview.contents })) });
        commands.push(Command { title: "Refresh preview".into(), subtitle: empty.then(|| "Empty file · No text to preview".into()), shortcut: None,
            target: PaletteEvent::Run(Box::new(PreviewLocalFile { path: preview.path })) });
        commands.extend(preview.lines.into_iter().enumerate().map(|(index, text)| {
            let title = file_browser::preview_line_title(&text, "");
            let copy_hint = if preview.partial_line == Some(index) { "Partial line · Enter copies only the captured text" } else { "Enter copies this preview line" };
            Command { title, subtitle: Some(format!("Line {} · {copy_hint}", index + 1)), shortcut: None,
                target: PaletteEvent::Run(Box::new(CopyPreviewLine { text })) }
        }));
        let mut palette = Self::with_commands(commands, if preview.truncated { "Text preview · Limited to 64 KiB / 512 lines" } else if empty { "Text preview · Empty file" } else { "Text preview · Search lines · Read only" }, "No matching preview lines", window, cx);
        palette.location = Some(location);
        palette
    }
    pub fn terminal_search(query: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut palette = Self::with_commands(Vec::new(), "Search terminal output · All windows", "Enter submits a literal search · Retained history · Up to 256 terminals", window, cx);
        palette.terminal_search_input = true;
        palette.input.update(cx, |input, cx| {
            input.replace_draft(query, cx);
            input.select_all_text(cx);
        });
        palette
    }
    pub fn ssh_connection(destination: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut palette = Self::with_commands(Vec::new(), "Connect via SSH", ssh::INPUT_HINT, window, cx);
        palette.ssh_input = true;
        palette.input.update(cx, |input, cx| { input.replace_draft(destination, cx); input.select_all_text(cx); });
        palette
    }
    pub fn ssh_hosts(result: anyhow::Result<ssh::Aliases>, session_targets: Vec<ssh::HostTarget>, truncated: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut commands = vec![Command { title: "Enter SSH destination…".into(), subtitle: None, shortcut: None,
            target: PaletteEvent::Run(Box::new(ShowSshConnection)) }, Command { title: "Refresh SSH hosts".into(), subtitle: None, shortcut: None,
            target: PaletteEvent::Run(Box::new(ShowSshHosts)) }];
        let (aliases, mut summary) = match result {
            Ok(aliases) => {
                let summary = format!("SSH config/includes{} · Targets from this window", if aliases.limited { " · Discovery incomplete" } else { "" });
                (aliases.names, summary)
            }
            Err(error) => (Vec::new(), format!("Config unavailable: {error} · Targets from this window")),
        };
        if truncated { summary.push_str(" · First 256 terminals"); }
        commands.extend(ssh::host_targets(session_targets, &aliases).into_iter().map(|target| Command {
            title: target.destination.clone(),
            subtitle: Some(match target.source {
                ssh::TargetSource::Session => "Session target · Enter opens a new SSH workspace",
                ssh::TargetSource::Saved => "Saved target · Not connected · Enter opens a new SSH workspace",
                ssh::TargetSource::Config => "Config alias · Enter opens a new SSH workspace",
            }.into()), shortcut: None,
            target: PaletteEvent::Run(Box::new(ConnectSsh { destination: target.destination })),
        }));
        let mut palette = Self::with_commands(commands, "SSH hosts · Choose a destination", "No matching hosts · Clear search to enter a destination", window, cx);
        palette.location = Some(summary.into());
        if palette.commands.len() > 2 { palette.selected = 2; }
        palette
    }
    pub fn memory_sort(current: memory_view::Sort, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let commands = memory_view::Sort::ALL.into_iter().map(|sort| Command {
            title: sort.label().into(), subtitle: (sort == current).then(|| "Current sort order".into()), shortcut: None,
            target: PaletteEvent::Run(Box::new(SetTerminalMemorySort { sort })),
        }).collect();
        let mut palette = Self::with_commands(commands, "Sort terminal memory · Highest first", "No matching sort modes", window, cx);
        palette.selected = memory_view::Sort::ALL.iter().position(|sort| *sort == current).unwrap_or(0);
        palette
    }
    pub fn terminal_search_results(report: terminal_search::Report, query: String, extra: usize, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let summary = format!("{} matching terminals · {} scanned · {} unavailable · {} incomplete · {extra} beyond terminal limit", report.hits.len(), report.scanned, report.unavailable, report.limited);
        let mut commands = vec![Command { title: "Search again".into(), subtitle: Some(query.clone()), shortcut: None,
            target: PaletteEvent::Run(Box::new(SearchTerminalOutput { query: query.clone() })) },
            Command { title: "New search…".into(), subtitle: None, shortcut: None, target: PaletteEvent::Run(Box::new(ShowTerminalSearch)) }];
        commands.extend(report.hits.into_iter().map(|hit| Command { title: hit.name, subtitle: Some(hit.snippet), shortcut: None,
            target: PaletteEvent::SearchTerminal { window: hit.window, workspace: hit.workspace, terminal: hit.terminal, query: query.clone(), preview: hit.preview } }));
        let mut palette = Self::with_commands(commands, "Terminal output matches · Enter opens Find", "No matching results · Clear the filter to search again", window, cx);
        palette.location = Some(summary.into());
        if palette.commands.len() > 2 { palette.selected = 2; }
        palette
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
                    this.feedback = None;
                    if this.buffer_rename.is_some() || this.directory_input || this.terminal_search_input || this.ssh_input {
                        this.empty_message = if this.ssh_input { ssh::INPUT_HINT } else if this.terminal_search_input { "Enter submits a literal search · Retained history · Up to 256 terminals" } else if this.directory_input { "Enter an absolute directory path; no shell expansion" } else { "Enter a unique buffer name (up to 128 bytes)" };
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
            input, heading, empty_message, buffer_rename: None, feedback: None, directory_input: false, terminal_search_input: false, ssh_input: false, location: None,
            search: palette_match::RankedSearch::new(commands
                .iter()
                .map(Command::search_text)
                .collect()),
            scroll: ListState::new(commands.len(), ListAlignment::Top, px(96.)),
            commands,
            selected: 0,
        }
    }
    fn render_result(&mut self, row: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(&command) = self.search.matches.get(row) else { return div().into_any_element(); };
        let selected = row == self.selected;
        let title = self.commands[command].preview_line()
            .map(|text| file_browser::preview_line_title(text, self.input.read(cx).value()))
            .unwrap_or_else(|| self.commands[command].title.clone());
        let title_hint: SharedString = match &self.commands[command].subtitle {
            Some(subtitle) => format!("{title}\n{subtitle}").into(),
            None => title.clone().into(),
        };
        let subtitle = self.commands[command].subtitle.as_ref().map(|text| {
            let ranges = match &self.commands[command].target {
                PaletteEvent::SearchTerminal { query, preview: true, .. } => literal_highlights(text, query),
                _ => Vec::new(),
            };
            StyledText::new(text.clone()).with_highlights(ranges.into_iter().map(|range| (
                range, HighlightStyle { color: Some(rgb(0xffffff).into()), background_color: Some(rgb(0x41638c).into()), ..Default::default() }
            )))
        });
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
                        .child(div().truncate().child(title))
                        .when_some(subtitle, |column, subtitle| column.child(div().truncate().text_xs().line_height(px(16.)).text_color(rgb(0xa0a0a0)).child(subtitle)))),
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
    fn selected_search_excerpt(&self) -> Option<&str> {
        self.search.matches.get(self.selected)
            .map(|index| &self.commands[*index])
            .filter(|command| matches!(command.target, PaletteEvent::SearchTerminal { preview: true, .. }))
            .and_then(|command| command.subtitle.as_deref())
    }
    fn copy_search_excerpt(&mut self, cx: &mut Context<Self>) {
        if self.input.read(cx).is_composing() { return; }
        let Some(excerpt) = self.selected_search_excerpt() else { return; };
        clipboard::write(ClipboardItem::new_string(excerpt.to_owned()), cx);
        self.feedback = Some("Copied displayed excerpt");
        cx.notify();
    }
    fn run(&mut self, cx: &mut Context<Self>) {
        if self.input.read(cx).is_composing() { return; }
        if self.ssh_input {
            let destination = self.input.read(cx).value();
            match ssh::command(destination) {
                Ok(_) => cx.emit(PaletteEvent::Run(Box::new(ConnectSsh { destination: destination.to_owned() }))),
                Err(error) => { self.empty_message = error; cx.notify(); }
            }
            return;
        }
        if self.terminal_search_input {
            let query = self.input.read(cx).value();
            match terminal_search::validate(query) {
                Ok(()) => cx.emit(PaletteEvent::Run(Box::new(SearchTerminalOutput { query: query.to_owned() }))),
                Err(error) => { self.empty_message = error; cx.notify(); }
            }
            return;
        }
        if self.directory_input {
            match file_browser::entered_directory(self.input.read(cx).value()) {
                Ok(path) => cx.emit(PaletteEvent::Run(Box::new(BrowseDirectory { path }))),
                Err(error) => { self.empty_message = error; cx.notify(); }
            }
            return;
        }
        let directory = self.search.matches.get(self.selected).and_then(|index| match &self.commands[*index].target {
            PaletteEvent::Run(action) => action.as_any().downcast_ref::<EnterDirectory>().map(|action| action.path.clone()),
            _ => None,
        });
        if let Some(path) = directory {
            let Some(path) = path.to_str() else { self.feedback = Some("This path cannot be represented as UTF-8 text"); cx.notify(); return; };
            if path.len() > 4096 { self.feedback = Some("Path exceeds the editor's 4096-byte limit"); cx.notify(); return; }
            self.directory_input = true;
            self.heading = "Go to directory · Enter to open · Escape to cancel";
            self.empty_message = "Enter an absolute directory path; no shell expansion";
            self.commands.clear();
            self.search = palette_match::RankedSearch::new(Vec::new());
            self.scroll.reset(0);
            self.input.update(cx, |input, cx| { input.replace_draft(path, cx); input.select_all_text(cx); });
            cx.notify();
            return;
        }
        if let Some(PaletteEvent::Run(action)) = self.search.matches.get(self.selected)
            .map(|index| &self.commands[*index].target) {
            if let Some(line) = action.as_any().downcast_ref::<CopyPreviewLine>() {
                clipboard::write(ClipboardItem::new_string(line.text.clone()), cx);
                self.feedback = Some("Copied preview line");
                cx.notify();
                return;
            }
            if let Some(preview) = action.as_any().downcast_ref::<CopyPreviewText>() {
                clipboard::write(ClipboardItem::new_string(preview.text.clone()), cx);
                self.feedback = Some("Copied preview text");
                cx.notify();
                return;
            }
        }
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
                PaletteEvent::SearchTerminal { window, workspace, terminal, query, preview } => PaletteEvent::SearchTerminal { window: *window, workspace: *workspace, terminal: *terminal, query: query.clone(), preview: *preview },
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
        let has_search_excerpt = self.selected_search_excerpt().is_some();
        let copy_hint = shortcuts::hint(&CopySearchExcerpt, &self.input.read(cx).focus, window);
        div()
            .id("command-palette")
            .key_context("CommandPalette")
            .on_action(cx.listener(|this, _: &CopySearchExcerpt, _, cx| this.copy_search_excerpt(cx)))
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
                    this.feedback = None;
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
            .when_some(self.location.clone(), |palette, location| palette.child(
                div().id("file-browser-location").flex_shrink_0().min_w_0().px_3().pt_2()
                    .text_xs().text_color(rgb(0xb8b8b8)).truncate()
                    .tooltip({ let location = location.clone(); move |_, cx| cx.new(|_| pane_view::ControlHint(location.clone())).into() })
                    .child(location)
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
            .when(self.buffer_rename.is_some() || self.directory_input || self.terminal_search_input || self.ssh_input, |palette| palette.child(
                div().flex_shrink_0().px_3().py_2().flex().flex_wrap().items_center().gap_2()
                    .child(div().flex_1().min_w_0().text_xs().text_color(rgb(0xa0a0a0))
                        .child(if self.ssh_input { "Enter connects · Escape cancels" } else if self.terminal_search_input { "Enter searches · Escape cancels" } else if self.directory_input { "Enter opens · Escape cancels" } else { "Enter saves · Escape cancels" }))
                    .child(div().id("buffer-rename-cancel").px_3().py_1().rounded_sm()
                        .text_sm().cursor_pointer().bg(rgb(0x353535)).hover(|style| style.bg(rgb(0x454545)))
                        .on_click(cx.listener(|_, event: &ClickEvent, _, cx| {
                            if event.standard_click() { cx.emit(PaletteEvent::Close); }
                        })).child("Cancel"))
                    .child(div().id("buffer-rename-save").px_3().py_1().rounded_sm()
                        .text_sm().cursor_pointer().bg(rgb(0x007aff)).hover(|style| style.bg(rgb(0x1685ff)))
                        .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                            if event.standard_click() { this.run(cx); }
                        })).child(if self.ssh_input { "Connect" } else if self.terminal_search_input { "Search" } else if self.directory_input { "Open" } else { "Save" }))
            ))
            .when((!compact || self.feedback.is_some() || has_search_excerpt) && self.buffer_rename.is_none() && !self.directory_input && !self.terminal_search_input && !self.ssh_input, |palette| palette.child(
                div()
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .flex().flex_wrap().items_center().gap_2()
                    .text_xs()
                    .text_color(rgb(0xa0a0a0))
                    .child(div().flex_1().min_w_0().child(self.feedback.unwrap_or("↑↓ select · PgUp/PgDn jump · Enter run · Esc close")))
                    .when(has_search_excerpt, |footer| footer.child(
                        div().id("copy-search-excerpt").px_2().py_1().rounded_sm()
                            .cursor_pointer().text_color(rgb(0xffffff)).bg(rgb(0x353535))
                            .hover(|style| style.bg(rgb(0x454545)))
                            .tooltip(|_, cx| cx.new(|_| pane_view::ControlHint("Copy the displayed excerpt, including clipping ellipses".into())).into())
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                if !event.standard_click() { return; }
                                this.copy_search_excerpt(cx);
                                cx.stop_propagation();
                            }))
                            .child(copy_hint.map_or_else(|| "Copy excerpt".to_owned(), |hint| format!("Copy excerpt · {hint}")))
                    )),
            ))
    }
}

fn reveal_local_path(path: &std::path::Path) -> Command {
    let title = if cfg!(target_os = "macos") { "Reveal in Finder" }
        else if cfg!(target_os = "windows") { "Reveal in File Explorer" }
        else { "Reveal in File Manager" };
    Command { title: title.into(), subtitle: Some(path.display().to_string()), shortcut: None,
        target: PaletteEvent::Run(Box::new(RevealLocalPath { path: path.to_owned() })) }
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
