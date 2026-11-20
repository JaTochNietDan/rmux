use super::*;
use rmux_terminal::program_status::{Kind, Record, State};

pub fn panel(records: &[Record], loading: bool, cx: &mut Context<TerminalView>) -> AnyElement {
    div()
        .min_w_0()
        .overflow_hidden()
        .h(px(220.))
        .max_h(relative(0.45))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(rgb(0x222222))
        .border_b_1()
        .border_color(rgb(0x383838))
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
                .min_w_0()
                .gap_2()
                .text_sm()
                .child(div().flex_1().min_w_0().truncate().child(if loading { "Program status · Loading…".to_owned() } else { format!("Program status · {} records", records.len()) }))
                .child(
                    pane_view::pane_control("close-program-status", "Close Program Status")
                        .on_click(cx.listener(|view, event: &ClickEvent, _, cx| {
                            if !event.standard_click() { return; }
                            cx.stop_propagation();
                            view.status_details = None;
                            view.status_load = None;
                            cx.notify();
                        }))
                        .child(icons::icon("close")),
                ),
        )
        .child(
            div()
                .id("program-status-records")
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_y_scroll()
                .p_2()
                .when(records.is_empty(), |list| {
                    list.child(div().p_2().text_sm().child(if loading { "Loading reported tasks…" } else { "No reported tasks" }))
                })
                .children(records.iter().enumerate().map(|(index, record)| {
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
                    let heading: SharedString = format!("{id} · {state}{progress}").into();
                    let metadata = [&record.app, &record.title].into_iter().filter(|value| !value.is_empty())
                        .map(String::as_str).collect::<Vec<_>>().join(" · ");
                    let report = [heading.as_ref(), metadata.as_str(), record.message.as_str()].into_iter()
                        .filter(|line| !line.is_empty()).collect::<Vec<_>>().join("\n");
                    div()
                        .min_w_0().overflow_hidden()
                        .mb_2()
                        .ml(px(record.id.matches('/').count().min(8) as f32 * 8.))
                        .p_2()
                        .rounded_sm()
                        .bg(rgb(0x2b2b2b))
                        .text_sm()
                        .child(
                            div().flex().items_center().min_w_0().gap_2()
                                .child(div().id(("status-heading", index)).flex_1().min_w_0().truncate().text_color(rgb(tint))
                                    .tooltip({ let heading = heading.clone(); move |_, cx| cx.new(|_| pane_view::ControlHint(heading.clone())).into() })
                                    .child(heading))
                                .child(pane_view::pane_control(("copy-status", index), "Copy complete task details")
                                    .on_click(move |event: &ClickEvent, _, cx| {
                                        if !event.standard_click() { return; }
                                        cx.stop_propagation();
                                        clipboard::write(ClipboardItem::new_string(report.clone()), cx);
                                    }).child(icons::icon("copy"))),
                        )
                        .when(!record.app.is_empty() || !record.title.is_empty(), |row| {
                            row.child(
                                div().id(("status-metadata", index)).min_w_0().truncate()
                                    .text_xs().text_color(rgb(0xaaaaaa))
                                    .tooltip({ let metadata: SharedString = metadata.clone().into(); move |_, cx| cx.new(|_| pane_view::ControlHint(metadata.clone())).into() })
                                    .child(metadata),
                            )
                        })
                        .when(!record.message.is_empty(), |row| {
                            row.child(div().mt_1().min_w_0().whitespace_normal().child(record.message.clone()))
                        })
                })),
        )
        .into_any_element()
}
