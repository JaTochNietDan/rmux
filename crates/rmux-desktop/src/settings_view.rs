use super::*;

#[derive(Clone)]
pub struct State {
    pub focus: FocusHandle,
    pub origin: Option<FocusHandle>,
    pub selected: usize,
    pub scroll: ScrollHandle,
}
pub const KEYS: &[&str] = &["font", "padding", "theme", "cursor", "copy", "close", "group_placement"];
pub fn next_control(selected: usize, key: &str, shift: bool) -> Option<usize> {
    let count = KEYS.len() + 2;
    match key {
        "up" => Some((selected + count - 1) % count),
        "down" => Some((selected + 1) % count),
        "tab" => Some((selected + if shift { count - 1 } else { 1 }) % count),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}


pub fn adjust(settings: &mut Preferences, key: &str, delta: i32) {
    match key {
        "font" => settings.font_size = (settings.font_size + delta as f32).clamp(8., 40.),
        "padding" => settings.terminal_padding = (settings.terminal_padding as i32 + delta).clamp(0, 32) as u8,
        "theme" => settings.terminal_theme = match settings.terminal_theme { preferences::TerminalTheme::Dark => preferences::TerminalTheme::Light, preferences::TerminalTheme::Light => preferences::TerminalTheme::Dark },
        "close" if !settings.close_policy_overridden() => {
            use close_policy::ClosePolicy::{Active, Always, Never};
            settings.confirm_close = match (settings.confirm_close, delta < 0) {
                (Active, false) | (Never, true) => Always,
                (Always, false) | (Active, true) => Never,
                _ => Active,
            };
        }
        "group_placement" => {
            use workspace_groups::Placement::{AfterCurrent, Top, End};
            settings.group_new_workspace_placement = match (settings.group_new_workspace_placement, delta < 0) {
                (AfterCurrent, false) | (End, true) => Top,
                (Top, false) | (AfterCurrent, true) => End,
                _ => AfterCurrent,
            };
        }
        "copy" => settings.copy_on_select = !settings.copy_on_select,
        "cursor" => settings.unfocused_cursor = match settings.unfocused_cursor { preferences::UnfocusedCursor::Hollow => preferences::UnfocusedCursor::Hidden, preferences::UnfocusedCursor::Hidden => preferences::UnfocusedCursor::Hollow },
        _ => {}
    }
}
fn button(id: String, label: String, key: &'static str, delta: i32) -> impl IntoElement {
    div().id(SharedString::from(id)).px_2().py_1().rounded_sm().cursor_pointer()
        .bg(rgb(0x353535)).hover(|s| s.bg(rgb(0x454545))).text_size(px(12.))
        .on_click(move |event, window, cx| { if event.standard_click() { window.dispatch_action(Box::new(AdjustSetting { key: key.into(), delta }), cx); } })
        .child(label)
}
fn row(label: &'static str, key: &'static str, value: String, stepper: bool, selected: bool, readonly: bool) -> impl IntoElement {
    div().px_1().py_3().when(selected, |s| s.bg(rgb(0x303b4a))).border_b_1().border_color(rgb(0x383838))
        .flex().flex_wrap().items_center().gap_2()
        .child(div().flex_1().min_w(px(120.)).text_size(px(13.)).child(label))
        .child(div().flex().items_center().gap_2().max_w_full()
            .when(stepper, |controls| controls
                .child(button(format!("{key}-less"), "−".into(), key, -1)))
            .child(if readonly {
                div().id(SharedString::from(format!("{key}-readonly")))
                    .px_2().py_1().text_size(px(12.)).text_color(rgb(0xa0a0a0))
                    .tooltip(|_, cx| cx.new(|_| pane_view::ControlHint("Controlled by RMUX_CONFIRM_CLOSE. Remove the override and restart rmux to edit this policy here.".into())).into())
                    .child(value).into_any_element()
            } else if stepper {
                div().min_w(px(44.)).text_center().text_size(px(12.)).child(value).into_any_element()
            } else {
                button(format!("{key}-value"), value, key, 1).into_any_element()
            })
            .when(stepper, |controls| controls
                .child(button(format!("{key}-more"), "+".into(), key, 1))))
}
pub fn panel(state: State, cx: &mut Context<window_view::WorkspaceWindow>) -> AnyElement {
    let settings = cx.global::<Preferences>();
    div().track_focus(&state.focus).key_context("Settings")
        .on_mouse_down(MouseButton::Left, { let focus = state.focus.clone(); move |_, window, _| { focus.focus(window); } })
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| this.settings_key(event, window, cx)))
        .w(px(340.)).max_w(relative(0.45)).h_full().flex_shrink_0().flex().flex_col()
        .bg(rgb(0x242424)).border_l_1().border_color(rgb(0x383838))
        .child(div().h(px(36.)).px_3().flex().items_center().gap_2().flex_shrink_0()
            .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child("Settings"))
            .child(pane_view::pane_control("close-settings", "Close Settings")
                .on_click(|event, window, cx| { if event.standard_click() { window.dispatch_action(Box::new(ToggleSettings), cx); } }).child(icons::icon("close"))))
        .child(div().id("settings-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&state.scroll).px_3()
            .child(div().pt_3().text_xs().text_color(rgb(0xa0a0a0)).child("TERMINAL & BEHAVIOR"))
            .child(row("Default text size", "font", format!("{} pt", settings.font_size), true, state.selected == 0, false))
            .child(row("Terminal padding", "padding", format!("{} px", settings.terminal_padding), true, state.selected == 1, false))
            .child(row("Terminal theme", "theme", match settings.terminal_theme { preferences::TerminalTheme::Dark => "Dark", preferences::TerminalTheme::Light => "Light" }.into(), false, state.selected == 2, false))
            .child(row("Unfocused cursor", "cursor", match settings.unfocused_cursor { preferences::UnfocusedCursor::Hollow => "Hollow", preferences::UnfocusedCursor::Hidden => "Hidden" }.into(), false, state.selected == 3, false))
            .child(row("Copy on select", "copy", if settings.copy_on_select { "On" } else { "Off" }.into(), false, state.selected == 4, false))
            .child(row("Confirm before closing", "close", format!("{}{}", match settings.confirm_close {
                close_policy::ClosePolicy::Active => "Active tasks", close_policy::ClosePolicy::Always => "Always", close_policy::ClosePolicy::Never => "Never",
            }, if settings.close_policy_overridden() { " · Environment" } else { "" }), false, state.selected == 5, settings.close_policy_overridden()))
            .child(row("New group workspace", "group_placement", match settings.group_new_workspace_placement {
                workspace_groups::Placement::AfterCurrent => "After current",
                workspace_groups::Placement::Top => "First",
                workspace_groups::Placement::End => "Last",
            }.into(), false, state.selected == 6, false))
            .child(div().py_3().text_xs().text_color(rgb(0xa0a0a0)).child("Tab / ↑↓ to navigate · ←→ to change · Enter to activate · Esc to close. Changes apply to this session. Text size updates terminals using the previous default. Per-terminal zoom stays unchanged; Reset Font Size rejoins the default. Custom colors in your config override the theme."))
            .child(div().id("settings-save").border_1().border_color(if state.selected == KEYS.len() { rgb(0xffffff) } else { rgb(0x007aff) }).px_3().py_2().rounded_sm().bg(rgb(0x007aff)).hover(|s| s.bg(rgb(0x1685ff))).text_sm().cursor_pointer()
                .on_click(|event, window, cx| { if event.standard_click() { window.dispatch_action(Box::new(SaveAppearanceSettings), cx); } }).child("Save these settings"))
            .child(div().id("settings-json").when(state.selected == KEYS.len() + 1, |s| s.bg(rgb(0x303b4a))).py_3().text_sm().text_color(rgb(0x8ebfff)).cursor_pointer()
                .on_click(|event, window, cx| { if event.standard_click() { window.dispatch_action(Box::new(OpenSettingsFile), cx); } }).child("Open all settings in editor")))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{adjust, next_control, Preferences, close_policy, workspace_groups, KEYS};
    #[test]
    fn keyboard_navigation_wraps_and_adjustments_stay_in_bounds() {
        assert_eq!(next_control(0, "tab", true), Some(KEYS.len() + 1));
        assert_eq!(next_control(KEYS.len() + 1, "down", false), Some(0));
        assert_eq!(next_control(3, "end", false), Some(KEYS.len() + 1));
        assert_eq!(next_control(3, "a", false), None);
        let mut prefs = Preferences::default();
        for _ in 0..100 { adjust(&mut prefs, "font", 1); adjust(&mut prefs, "padding", -1); }
        adjust(&mut prefs, "close", 1);
        assert_eq!(prefs.confirm_close, close_policy::ClosePolicy::Always);
        adjust(&mut prefs, "close", -1);
        assert_eq!(prefs.confirm_close, close_policy::ClosePolicy::Active);
        adjust(&mut prefs, "close", -1);
        assert_eq!(prefs.confirm_close, close_policy::ClosePolicy::Never);
        adjust(&mut prefs, "group_placement", 1);
        assert_eq!(prefs.group_new_workspace_placement, workspace_groups::Placement::Top);
        adjust(&mut prefs, "group_placement", -1);
        assert_eq!(prefs.group_new_workspace_placement, workspace_groups::Placement::AfterCurrent);
        adjust(&mut prefs, "group_placement", -1);
        assert_eq!(prefs.group_new_workspace_placement, workspace_groups::Placement::End);
        assert_eq!(prefs.font_size, 40.);
        assert_eq!(prefs.terminal_padding, 0);
    }
}
