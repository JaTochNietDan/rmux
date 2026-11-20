use super::*;
use rmux_terminal::program_status::{Kind, Record, State};

pub fn panel(records: &[Record], loading: bool, cx: &mut Context<TerminalView>) -> AnyElement {
    div()
        .h(px(220.))
        .max_h(relative(0.45))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(rgb(0x1b1e27))
        .border_b_1()
        .border_color(rgb(0x3b4352))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .h(px(32.))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .text_sm()
                .child(if loading { "Program status · Loading…".to_owned() } else { format!("Program status · {} records", records.len()) })
                .child(
                    div()
                        .id("close-program-status")
                        .px_2()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x353b46)))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.status_details = None;
                            view.status_load = None;
                            cx.notify();
                        }))
                        .child("×"),
                ),
        )
        .child(
            div()
                .id("program-status-records")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_2()
                .when(records.is_empty(), |list| {
                    list.child(div().p_2().text_sm().child(if loading { "Loading reported tasks…" } else { "No reported tasks" }))
                })
                .children(records.iter().map(|record| {
                    let tint = match record.state {
                        State::Blocked => 0xf0b85b,
                        State::Error => 0xf18a8a,
                        State::Done => 0x8dd6a1,
                        State::Working => 0x86b8f5,
                        State::Idle => 0x9ba6b8,
                    };
                    let state = match record.kind {
                        Kind::Permission => "Approval".to_owned(),
                        Kind::Question => "Question".to_owned(),
                        Kind::Auth => "Login".to_owned(),
                        Kind::None => format!("{:?}", record.state),
                    };
                    let progress = record
                        .progress
                        .map_or(String::new(), |value| format!(" · {value}%"));
                    let id = if record.id.is_empty() {
                        "Root"
                    } else {
                        &record.id
                    };
                    div()
                        .mb_2()
                        .ml(px(record.id.matches('/').count().min(8) as f32 * 8.))
                        .p_2()
                        .rounded_sm()
                        .bg(rgb(0x252b36))
                        .text_sm()
                        .child(
                            div()
                                .text_color(rgb(tint))
                                .child(format!("{id} · {state}{progress}")),
                        )
                        .when(!record.app.is_empty() || !record.title.is_empty(), |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x9ba6b8))
                                    .child(format!("{} · {}", record.app, record.title)),
                            )
                        })
                        .when(!record.message.is_empty(), |row| {
                            row.child(div().mt_1().child(record.message.clone()))
                        })
                })),
        )
        .into_any_element()
}
