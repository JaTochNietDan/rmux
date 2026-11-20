use super::*;
#[derive(Default)]
pub(super) struct State {
    pub visible: bool,
    generation: u64,
    loading: bool,
    report: Option<ports::Report>,
    error: Option<String>,
}
impl TerminalView {
    pub(super) fn close_ports(&mut self, cx: &mut Context<Self>) {
        self.ports.visible = false;
        self.ports.report = None;
        self.ports.error = None;
        cx.notify();
    }
    pub(super) fn refresh_ports(&mut self, cx: &mut Context<Self>) {
        if self.ports.loading { self.ports.visible = true; cx.notify(); return; }
        self.ports.visible = true;
        self.ports.error = None;
        self.ports.report = None;
        let prepare = || -> Result<_> {
            anyhow::ensure!(self.session.exit_status().is_none(), "Terminal process has exited");
            let pid = self.session.process_id().ok_or_else(|| anyhow::anyhow!("Terminal has no local process ID"))?;
            Ok((pid, ports::Permit::acquire()?))
        };
        match prepare() {
            Err(error) => self.ports.error = Some(error.to_string()),
            Ok((pid, permit)) => {
                self.ports.loading = true;
                self.ports.generation = self.ports.generation.wrapping_add(1);
                let generation = self.ports.generation;
                cx.spawn(async move |entity, cx| {
                    let result = cx.background_executor().spawn(async move {
                        let _permit = permit;
                        ports::scan(pid)
                    }).await;
                    let _ = entity.update(cx, |this, cx| {
                        if this.ports.generation != generation { return; }
                        this.ports.loading = false;
                        if !this.ports.visible { return; }
                        if this.session.exit_status().is_some() { this.ports.error = Some("Terminal process has exited".into()); }
                        else { match result { Ok(report) => this.ports.report = Some(report), Err(error) => this.ports.error = Some(error.to_string()) } }
                        cx.notify();
                    });
                }).detach();
            }
        }
        cx.notify();
    }
}
pub(super) fn panel(state: &State, cx: &mut Context<TerminalView>) -> AnyElement {
    div().h(px(180.)).max_h(relative(0.4)).flex_shrink_0().flex().flex_col()
        .bg(rgb(0x242424)).border_b_1().border_color(rgb(0x383838)).text_size(px(12.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(div().h(px(32.)).flex_shrink_0().px_2().flex().items_center().gap_2()
            .child(div().flex_1().child(if state.loading { "Local TCP listeners · Scanning…" } else { "Local TCP listeners · Snapshot" }))
            .child(pane_view::pane_control("refresh-ports", "Refresh Listening Ports").on_click(cx.listener(|this, event: &ClickEvent, _, cx| { if event.standard_click() { this.refresh_ports(cx); } })).child(icons::icon("refresh")))
            .child(pane_view::pane_control("close-ports", "Close Listening Ports").on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                if event.standard_click() { this.close_ports(cx); }
            })).child(icons::icon("close"))))
        .child(div().id("port-list").flex_1().min_h_0().overflow_y_scroll().px_2().pb_2()
            .when_some(state.error.clone(), |list, error| list.child(div().text_color(rgb(0xf2a5a5)).child(error)))
            .when_some(state.report.as_ref(), |list, report| list
                .when(report.listeners.is_empty(), |list| list.child("No local TCP listeners found for this terminal."))
                .when(report.truncated, |list| list.child("Scan limit reached; results are incomplete."))
                .children(report.listeners.iter().enumerate().map(|(index, row)| {
                    let address = row.address.to_string();
                    div().py_1().flex().items_center().gap_2()
                        .child(div().flex_1().min_w_0().truncate().child(address.clone()))
                        .child(div().text_color(rgb(0xa0a0a0)).child(format!("PID {}", row.pid)))
                        .child(pane_view::pane_control("copy-port", "Copy Listener Address").id(("copy-port", index))
                            .on_click(move |event, _, cx| { if event.standard_click() { clipboard::write(ClipboardItem::new_string(address.clone()), cx); } })
                            .child(icons::icon("copy")))
                }))))
        .into_any_element()
}
