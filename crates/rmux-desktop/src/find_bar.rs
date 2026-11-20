use super::*;
use rmux_terminal::{search::Direction, search_worker::SearchWorker};
use text_field::{EditEvent, TextField};

pub(super) struct Closed;
pub(super) struct FindBar {
    pub focus: FocusHandle,
    pub worker: SearchWorker,
    input: Entity<TextField>,
    status: String,
    _updates: Task<()>,
}
impl EventEmitter<Closed> for FindBar {}
impl FindBar {
    pub fn focus_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.select_all_text(cx));
        self.focus.focus(window);
    }
    pub fn query<'a>(&'a self, cx: &'a App) -> &'a str {
        self.input.read(cx).value()
    }
    pub fn new(
        worker: SearchWorker,
        parent: WeakEntity<TerminalView>,
        initial_query: &str,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextField::new(initial_query, cx));
        let status = if initial_query.is_empty() {
            "Find in terminal".into()
        } else {
            match worker.set_query(initial_query.to_owned()) {
                Ok(_) => "Searching…".into(),
                Err(error) => error.to_string(),
            }
        };
        let focus = input.read(cx).focus.clone();
        cx.subscribe(&input, |view, _, event: &EditEvent, cx| match event {
            EditEvent::Changed(text) => {
                view.status = if text.is_empty() { "Find in terminal" } else { "Searching…" }.into();
                if let Err(error) = view.worker.set_query(text.clone()) {
                    view.status = error.to_string();
                }
                cx.notify();
            }
            EditEvent::Submitted { reverse, .. } => view.worker.navigate(if *reverse {
                Direction::Newer
            } else {
                Direction::Older
            }),
            EditEvent::Cancelled => cx.emit(Closed),
        })
        .detach();
        let updates = worker.updates.clone();
        let task = cx.spawn(async move |entity, cx| {
            while updates.recv().await.is_ok() {
                if entity
                    .update(cx, |view, cx| {
                        if let Some(update) = view.worker.latest() {
                            view.status = match update.result {
                                Ok(_) if view.input.read(cx).value().is_empty() => "Find in terminal".into(),
                                Ok(p) if p.matches == 0 => if p.pending { "Searching…" } else { "No matches" }.into(),
                                Ok(p) => format!(
                                    "{} / {}{}",
                                    p.selected.map_or(0, |i| i + 1),
                                    p.matches,
                                    if p.pending { " …" } else { "" }
                                ),
                                Err(error) => error,
                            };
                            cx.notify();
                            let _ = parent.update(cx, |_, cx| cx.notify());
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            focus,
            worker,
            input,
            status,
            _updates: task,
        }
    }
}
impl Render for FindBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("FindInput")
            .h(px(34.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .bg(rgb(0x242424))
            .border_b_1()
            .border_color(rgb(0x353535))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus.focus(window);
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(div().flex_1().min_w_0().child(self.input.clone()))
            .child(
                div()
                    .text_xs()
                    .max_w(px(140.))
                    .flex_shrink_0()
                    .text_color(rgb(0xa0a0a0))
                    .truncate()
                    .child(self.status.clone()),
            )
            .child(
                pane_view::pane_control("find-older", "Older match · Enter")
                    .on_click(cx.listener(|this, _, _, _| this.worker.navigate(Direction::Older)))
                    .child(icons::icon("chevron-up")),
            )
            .child(
                pane_view::pane_control("find-newer", "Newer match · Shift+Enter")
                    .on_click(cx.listener(|this, _, _, _| this.worker.navigate(Direction::Newer)))
                    .child(icons::icon("chevron-down")),
            )
            .child(
                pane_view::pane_control("find-close", "Close Find · Escape")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(Closed)))
                    .child(icons::icon("close")),
            )
    }
}
