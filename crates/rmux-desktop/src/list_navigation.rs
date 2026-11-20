use gpui::{ListOffset, ListState, px};

/// GPUI's pixel-based reveal needs measured row heights. A distant result may
/// never have been laid out; position that row directly instead of calculating
/// its position from zero-height placeholders.
pub fn reveal(scroll: &ListState, index: usize) {
    let count = scroll.item_count();
    if count == 0 { return; }
    let index = index.min(count - 1);
    if scroll.bounds_for_item(index).is_some() {
        scroll.scroll_to_reveal_item(index);
    } else {
        scroll.scroll_to(ListOffset { item_ix: index, offset_in_item: px(0.) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ListAlignment, ListOffset, px};

    #[test]
    fn jump_to_unmeasured_result_targets_that_row() {
        let scroll = ListState::new(256, ListAlignment::Top, px(96.));
        reveal(&scroll, 255);
        assert_eq!(scroll.logical_scroll_top().item_ix, 255);
        scroll.reset(256);
        reveal(&scroll, 128);
        assert_eq!(scroll.logical_scroll_top().item_ix, 128);
        scroll.scroll_to(ListOffset { item_ix: 200, offset_in_item: px(7.) });
        reveal(&scroll, 0);
        assert_eq!(scroll.logical_scroll_top().item_ix, 0);
    }

    #[test]
    fn removed_results_clamp_and_empty_lists_do_not_scroll() {
        let scroll = ListState::new(3, ListAlignment::Top, px(96.));
        reveal(&scroll, 255);
        assert_eq!(scroll.logical_scroll_top().item_ix, 2);
        scroll.reset(0);
        reveal(&scroll, 255);
        assert_eq!(scroll.logical_scroll_top().item_ix, 0);
    }
}
