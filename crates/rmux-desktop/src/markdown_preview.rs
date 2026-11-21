//! Bounded, inert CommonMark reading blocks. No HTML/web engine or asset loads.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Kind { #[default] Paragraph, Heading(u8), Code, Quote, List { depth: usize, number: Option<u64>, marker: bool, checked: Option<bool> } }
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InlineStyle { pub bold: bool, pub italic: bool }
#[derive(Clone, Debug, PartialEq)]
pub struct Span { pub range: std::ops::Range<usize>, pub style: InlineStyle }
pub struct Block { pub text: String, pub kind: Kind, pub spans: Vec<Span> }
pub fn blocks(source: &str) -> (Vec<Block>, bool) {
    let mut output = Vec::new();
    let mut text = String::new();
    let mut kind = Kind::Paragraph;
    let mut spans: Vec<Span> = Vec::new();
    let (mut bold, mut italic) = (0usize, 0usize);
    let mut quote = 0usize;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut items: Vec<Kind> = Vec::new();
    let flush = |output: &mut Vec<Block>, text: &mut String, spans: &mut Vec<Span>, kind| {
        if text.trim().is_empty() { text.clear(); spans.clear(); return false; }
        if output.len() >= 512 { return true; }
        output.push(Block { text: std::mem::take(text), kind, spans: std::mem::take(spans) });
        false
    };
    for event in Parser::new_ext(source, Options::ENABLE_TASKLISTS) {
        match event {
            Event::Start(Tag::BlockQuote(_)) => quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote = quote.saturating_sub(1),
            Event::Start(Tag::List(start)) => {
                if !text.trim().is_empty() && let Some(Kind::List { marker, .. }) = items.last_mut() { *marker = false; }
                if flush(&mut output, &mut text, &mut spans, kind) { return (output, true); }
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => { lists.pop(); }
            Event::Start(Tag::Item) => {
                if flush(&mut output, &mut text, &mut spans, kind) { return (output, true); }
                let number = lists.last().copied().flatten();
                kind = Kind::List { depth: lists.len(), number, marker: true, checked: None };
                items.push(kind);
                if let Some(Some(next)) = lists.last_mut() { *next = next.saturating_add(1); }
            }
            Event::End(TagEnd::Item) => {
                if flush(&mut output, &mut text, &mut spans, kind) { return (output, true); }
                items.pop();
                kind = items.last().copied().unwrap_or(if quote > 0 { Kind::Quote } else { Kind::Paragraph });
            }
            Event::Start(tag @ (Tag::HtmlBlock | Tag::Paragraph | Tag::Heading { .. } | Tag::CodeBlock(_))) => {
                if flush(&mut output, &mut text, &mut spans, kind) { return (output, true); }
                kind = match tag { Tag::Heading { level, .. } => Kind::Heading(level as u8), Tag::CodeBlock(_) => Kind::Code,
                    _ if !items.is_empty() => *items.last().unwrap(), _ if quote > 0 => Kind::Quote, _ => Kind::Paragraph };
                if let Some(Kind::List { marker, .. }) = items.last_mut() { *marker = false; }
            }
            Event::End(TagEnd::HtmlBlock | TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock) => { if flush(&mut output, &mut text, &mut spans, kind) { return (output, true); } },
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic = italic.saturating_sub(1),
            Event::Text(value) | Event::Code(value) | Event::Html(value) | Event::InlineHtml(value) => {
                let start = text.len();
                text.push_str(&value);
                if start < text.len() && (bold > 0 || italic > 0) {
                    let style = InlineStyle { bold: bold > 0, italic: italic > 0 };
                    if let Some(previous) = spans.last_mut() && previous.range.end == start && previous.style == style { previous.range.end = text.len(); }
                    else { spans.push(Span { range: start..text.len(), style }); }
                }
            },
            Event::TaskListMarker(value) => {
                if let Kind::List { checked, .. } = &mut kind { *checked = Some(value); }
                if let Some(Kind::List { checked, .. }) = items.last_mut() { *checked = Some(value); }
            }
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            Event::Rule => {
                if flush(&mut output, &mut text, &mut spans, kind) || output.len() >= 512 { return (output, true); }
                output.push(Block { text: "────────".into(), kind: Kind::Paragraph, spans: Vec::new() });
            },
            _ => {},
        }
    }
    let limited = flush(&mut output, &mut text, &mut spans, kind);
    (output, limited)
}
/// Project original block styles through the same bounded/sanitized excerpt as
/// the file viewer, including Unicode byte lengths and a list-marker prefix.
pub fn excerpt_spans(text: &str, query: &str, spans: &[Span], prefix: usize) -> Vec<Span> {
    if spans.is_empty() { return Vec::new(); }
    let start = crate::file_browser::preview_excerpt_start(text, query);
    let mut display = prefix + if start > 0 { "…".len() } else { 0 };
    let mut current = 0;
    let mut output: Vec<Span> = Vec::new();
    for (source, ch) in text.char_indices().skip(start).take(1024) {
        let width = if ch.is_control() { 1 } else { ch.len_utf8() };
        while current < spans.len() && spans[current].range.end <= source { current += 1; }
        if let Some(span) = spans.get(current) && span.range.contains(&source) {
            if let Some(previous) = output.last_mut() && previous.range.end == display && previous.style == span.style { previous.range.end += width; }
            else { output.push(Span { range: display..display + width, style: span.style }); }
        }
        display += width;
    }
    output
}
/// Sorted, disjoint runs combine emphasis with search colors without overlap.
pub fn merge_highlights(spans: &[Span], search: &[std::ops::Range<usize>]) -> Vec<(std::ops::Range<usize>, InlineStyle, bool)> {
    if spans.is_empty() { return search.iter().cloned().map(|range| (range, InlineStyle::default(), true)).collect(); }
    if search.is_empty() { return spans.iter().map(|span| (span.range.clone(), span.style, false)).collect(); }
    let mut edges: Vec<_> = spans.iter().flat_map(|span| [span.range.start, span.range.end]).chain(search.iter().flat_map(|range| [range.start, range.end])).collect();
    edges.sort_unstable(); edges.dedup();
    let (mut style_index, mut search_index) = (0, 0);
    edges.windows(2).filter_map(|pair| {
        while style_index < spans.len() && spans[style_index].range.end <= pair[0] { style_index += 1; }
        while search_index < search.len() && search[search_index].end <= pair[0] { search_index += 1; }
        let style = spans.get(style_index).filter(|span| span.range.contains(&pair[0])).map(|span| span.style).unwrap_or_default();
        let matched = search.get(search_index).is_some_and(|range| range.contains(&pair[0]));
        (style.bold || style.italic || matched).then_some((pair[0]..pair[1], style, matched))
    }).collect()
}

/// Four source lines around the first contiguous query-word match. Bounded
/// display only: the block's original text remains the copy/search source.
pub fn code_excerpt(source: &str, query: &str) -> String {
    let query = query.to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    let found = if words.is_empty() { 0 } else {
        source.lines().position(|line| { let line = line.to_lowercase(); words.iter().any(|word| line.contains(word)) }).unwrap_or(0)
    };
    let start = found.saturating_sub(1);
    let mut lines = source.lines().skip(start).peekable();
    let mut rendered = Vec::new();
    for index in 0..4 {
        let Some(line) = lines.next() else { break; };
        let mut output = String::new();
        let mut columns = 0usize;
        let mut scalars = 0usize;
        let mut clipped = false;
        for ch in line.chars() {
            let width = if ch == '\t' { 4 - columns % 4 } else if ch.is_control() { 1 } else { unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0) };
            if columns + width > 256 || scalars >= 256 { clipped = true; break; }
            if ch == '\t' { output.extend(std::iter::repeat_n(' ', width)); }
            else { output.push(if ch.is_control() { ' ' } else { ch }); }
            columns += width;
            scalars += 1;
        }
        if start > 0 && index == 0 { output.insert_str(0, "… "); }
        if clipped || (index == 3 && lines.peek().is_some()) { output.push_str(" …"); }
        rendered.push(output);
    }
    if rendered.is_empty() { "(empty code block)".into() } else { rendered.join("\n") }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_emphasis_nests_and_resets_across_blocks() {
        let (result, _) = blocks("***both*** normal **bold** *italic*\n\nplain\n");
        assert_eq!(result[0].text, "both normal bold italic");
        let spans: Vec<_> = result[0].spans.iter().map(|span| (&result[0].text[span.range.clone()], span.style)).collect();
        assert_eq!(spans, [("both", InlineStyle { bold: true, italic: true }), ("bold", InlineStyle { bold: true, italic: false }), ("italic", InlineStyle { bold: false, italic: true })]);
        assert!(result[1].spans.is_empty());
    }
    #[test]
    fn inline_excerpt_ranges_handle_unicode_controls_and_search_overlap() {
        let text = format!("{}A\u{85}界 needle", "İ".repeat(1100));
        let start = text.find('A').unwrap();
        let spans = [Span { range: start..start + "A\u{85}界".len(), style: InlineStyle { bold: true, italic: true } }];
        let prefix = "☑ ";
        let title = format!("{prefix}{}", crate::file_browser::preview_line_title(&text, "needle"));
        let projected = excerpt_spans(&text, "needle", &spans, prefix.len());
        assert_eq!(projected.len(), 1);
        assert_eq!(&title[projected[0].range.clone()], "A 界");
        let search = crate::file_browser::preview_highlights(&title, "界");
        let runs = merge_highlights(&projected, &search);
        assert!(runs.windows(2).all(|pair| pair[0].0.end <= pair[1].0.start));
        assert!(runs.iter().any(|(range, style, matched)| &title[range.clone()] == "界" && style.bold && style.italic && *matched));
        assert!(excerpt_spans(&text, "", &spans, 0).is_empty());
    }
    #[test]
    fn task_lists_keep_checked_state_without_modifying_body_text() {
        let source = "- [ ] pending\n- [x] done\n  - [X] nested\n\n7. [x] ordered\n\n   continuation\n\nordinary [ ] text\n";
        let (result, limited) = blocks(source);
        assert!(!limited);
        assert_eq!(result[0].text, "pending");
        assert!(matches!(result[0].kind, Kind::List { checked: Some(false), depth: 1, marker: true, .. }));
        assert!(matches!(result[1].kind, Kind::List { checked: Some(true), depth: 1, .. }));
        assert!(matches!(result[2].kind, Kind::List { checked: Some(true), depth: 2, .. }));
        assert!(matches!(result[3].kind, Kind::List { checked: Some(true), number: Some(7), marker: true, .. }));
        assert!(matches!(result[4].kind, Kind::List { checked: Some(true), marker: false, .. }));
        assert_eq!(result.last().unwrap().text, "ordinary [ ] text");
    }
    #[test]
    fn heading_levels_preserve_atx_and_setext_structure() {
        let (result, limited) = blocks("# First\n\n## Second\n\n### Third\n\n#### Fourth\n\n##### Fifth\n\n###### Sixth\n\nSetext one\n==========\n\nSetext two\n----------\n");
        assert!(!limited);
        assert_eq!(result.iter().map(|block| block.kind).collect::<Vec<_>>(), [Kind::Heading(1), Kind::Heading(2), Kind::Heading(3), Kind::Heading(4), Kind::Heading(5), Kind::Heading(6), Kind::Heading(1), Kind::Heading(2)]);
        assert_eq!(result[6].text, "Setext one");
    }
    #[test]
    fn code_excerpts_keep_lines_indent_and_original_text() {
        let source = "zero\n\tif ready {\n\t\tprintln!(\"世界\");\n\t}\nlast\n";
        let excerpt = code_excerpt(source, "世界");
        assert_eq!(excerpt, "…     if ready {\n        println!(\"世界\");\n    }\nlast");
        assert_eq!(code_excerpt("one\ntwo\nthree\nfour\nfive", ""), "one\ntwo\nthree\nfour …");
        assert!(code_excerpt(&"界".repeat(300), "").chars().count() <= 258);
        assert_eq!(code_excerpt("a\x1bb", ""), "a b");
        assert!(source.contains('\t'));
        assert_eq!(code_excerpt("界\tx", ""), "界  x");
        assert!(code_excerpt(&"\u{301}".repeat(1000), "").chars().count() <= 258);
    }
    #[test]
    fn nested_lists_preserve_numbers_depth_and_loose_paragraphs() {
        let (result, limited) = blocks("3. first\n\n   continuation\n\n   - nested\n   - second nested\n\n4. last\n\nafter\n");
        assert!(!limited);
        let items: Vec<_> = result.iter().filter_map(|block| match block.kind {
            Kind::List { depth, number, .. } => Some((block.text.as_str(), depth, number)), _ => None,
        }).collect();
        assert_eq!(items, [("first", 1, Some(3)), ("continuation", 1, Some(3)), ("nested", 2, None), ("second nested", 2, None), ("last", 1, Some(4))]);
        assert!(matches!(result[0].kind, Kind::List { marker: true, .. }));
        assert!(matches!(result[1].kind, Kind::List { marker: false, .. }));
        assert_eq!(result.last().unwrap().kind, Kind::Paragraph);
    }
    #[test]
    fn exact_block_cap_does_not_label_closing_events_as_omitted_content() {
        for source in ["# heading\n\n".repeat(512), "- item\n".repeat(512), "> paragraph\n>\n".repeat(512)] {
            let (result, limited) = blocks(&source);
            assert_eq!(result.len(), 512);
            assert!(!limited);
        }
        let source = "# heading\n\n".repeat(512);
        for tail in ["extra", "\n---\n", "\n```\ncode\n```\n"] {
            let (result, limited) = blocks(&(source.clone() + tail));
            assert_eq!(result.len(), 512);
            assert!(limited);
        }
        let (result, limited) = blocks(&("# heading\n\n".repeat(511) + "---\n"));
        assert_eq!(result.len(), 512);
        assert!(!limited);
    }
    #[test]
    fn markdown_blocks_are_capped() {
        let (result, limited) = blocks(&"# heading\n\n".repeat(600));
        assert_eq!(result.len(), 512);
        assert!(limited);
    }
    #[test]
    fn reading_blocks_preserve_text_without_fetching_or_executing_assets() {
        let (blocks, truncated) = blocks("# Title\n\nHello **world** [link](https://example.invalid).\n\n- item\n\n> quote\n\n```sh\necho hello\n```\n\n![alt](file:///missing.png)\n\n<script>alert(1)</script>\n");
        assert!(!truncated);
        assert_eq!(blocks[0].kind, Kind::Heading(1));
        assert_eq!(blocks[1].text, "Hello world link.");
        assert!(blocks.iter().any(|b| matches!(b.kind, Kind::List { .. }) && b.text == "item"));
        assert!(blocks.iter().any(|b| b.kind == Kind::Quote && b.text == "quote"));
        assert!(blocks.iter().any(|b| b.kind == Kind::Code && b.text == "echo hello\n"));
        assert!(blocks.iter().any(|b| b.text == "alt"));
        assert!(blocks.iter().any(|b| b.text.contains("<script>")));
    }
}
