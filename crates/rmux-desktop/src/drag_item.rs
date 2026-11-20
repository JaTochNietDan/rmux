use super::*;

/// Stable entity IDs make a drop safe if rows/tabs change during the gesture.
/// Scope identifies the source workspace for tabs and source window for workspaces.
/// Drop receivers validate membership before moving a live entity.
#[derive(Clone)]
pub(super) struct DragItem {
    pub scope: EntityId,
    pub container: EntityId,
    pub item: EntityId,
    pub label: SharedString,
}
impl Render for DragItem {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .max_w(px(240.))
            .rounded_md()
            .bg(rgb(0x30394b))
            .text_color(rgb(0xdcdfe6))
            .text_sm()
            .shadow_md()
            .truncate()
            .child(self.label.clone())
    }
}
