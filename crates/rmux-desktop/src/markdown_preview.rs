//! Bounded, inert CommonMark reading blocks. No HTML/web engine or asset loads.
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Kind { #[default] Paragraph, Heading, Code, Quote, List { depth: usize, number: Option<u64>, marker: bool } }
pub struct Block { pub text: String, pub kind: Kind }
pub fn blocks(source: &str) -> (Vec<Block>, bool) {
    let mut output = Vec::new();
    let mut text = String::new();
    let mut kind = Kind::Paragraph;
    let mut quote = 0usize;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut items: Vec<Kind> = Vec::new();
    let flush = |output: &mut Vec<Block>, text: &mut String, kind| {
        if !text.trim().is_empty() { output.push(Block { text: std::mem::take(text), kind }); }
        else { text.clear(); }
    };
    for event in Parser::new(source) {
        if output.len() >= 512 { return (output, true); }
        match event {
            Event::Start(Tag::BlockQuote(_)) => quote += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote = quote.saturating_sub(1),
            Event::Start(Tag::List(start)) => {
                if !text.trim().is_empty() && let Some(Kind::List { marker, .. }) = items.last_mut() { *marker = false; }
                flush(&mut output, &mut text, kind);
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => { lists.pop(); }
            Event::Start(Tag::Item) => {
                flush(&mut output, &mut text, kind);
                let number = lists.last().copied().flatten();
                kind = Kind::List { depth: lists.len(), number, marker: true };
                items.push(kind);
                if let Some(Some(next)) = lists.last_mut() { *next = next.saturating_add(1); }
            }
            Event::End(TagEnd::Item) => {
                flush(&mut output, &mut text, kind);
                items.pop();
                kind = items.last().copied().unwrap_or(if quote > 0 { Kind::Quote } else { Kind::Paragraph });
            }
            Event::Start(tag @ (Tag::HtmlBlock | Tag::Paragraph | Tag::Heading { .. } | Tag::CodeBlock(_))) => {
                flush(&mut output, &mut text, kind);
                kind = match tag { Tag::Heading { .. } => Kind::Heading, Tag::CodeBlock(_) => Kind::Code,
                    _ if !items.is_empty() => *items.last().unwrap(), _ if quote > 0 => Kind::Quote, _ => Kind::Paragraph };
                if let Some(Kind::List { marker, .. }) = items.last_mut() { *marker = false; }
            }
            Event::End(TagEnd::HtmlBlock | TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock) => flush(&mut output, &mut text, kind),
            Event::Text(value) | Event::Code(value) | Event::Html(value) | Event::InlineHtml(value) => text.push_str(&value),
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            Event::Rule => { flush(&mut output, &mut text, kind); output.push(Block { text: "────────".into(), kind: Kind::Paragraph }); },
            _ => {},
        }
    }
    let limited = output.len() >= 512 && !text.trim().is_empty();
    if output.len() < 512 { flush(&mut output, &mut text, kind); }
    (output, limited)
}
#[cfg(test)]
mod tests {
    use super::*;
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
    fn markdown_blocks_are_capped() {
        let (result, limited) = blocks(&"# heading\n\n".repeat(600));
        assert_eq!(result.len(), 512);
        assert!(limited);
    }
    #[test]
    fn reading_blocks_preserve_text_without_fetching_or_executing_assets() {
        let (blocks, truncated) = blocks("# Title\n\nHello **world** [link](https://example.invalid).\n\n- item\n\n> quote\n\n```sh\necho hello\n```\n\n![alt](file:///missing.png)\n\n<script>alert(1)</script>\n");
        assert!(!truncated);
        assert_eq!(blocks[0].kind, Kind::Heading);
        assert_eq!(blocks[1].text, "Hello world link.");
        assert!(blocks.iter().any(|b| matches!(b.kind, Kind::List { .. }) && b.text == "item"));
        assert!(blocks.iter().any(|b| b.kind == Kind::Quote && b.text == "quote"));
        assert!(blocks.iter().any(|b| b.kind == Kind::Code && b.text == "echo hello\n"));
        assert!(blocks.iter().any(|b| b.text == "alt"));
        assert!(blocks.iter().any(|b| b.text.contains("<script>")));
    }
}
