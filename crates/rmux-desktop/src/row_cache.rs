use gpui::*;
use rmux_terminal::{Cell, Color, Frame};
use std::{cell::RefCell, sync::{Arc, Weak}};

use crate::{CellMetrics, color};

/// One shaped glyph for the block cursor; bounded independently of row caches.
#[derive(Default)]
pub struct CursorCache {
    entry: Option<(Cell, CellMetrics, SharedString, f32, ShapedLine)>,
}
impl CursorCache {
    pub fn shape(&mut self, cell: &Cell, cells: CellMetrics, family: &SharedString, window: &Window, metrics: Option<&mut crate::diagnostics::Metrics>) -> ShapedLine {
        if let Some((previous, cell_metrics, font_family, scale, line)) = &self.entry
            && previous == cell && *cell_metrics == cells && font_family == family
            && *scale == window.scale_factor()
        {
            if let Some(metrics) = metrics { metrics.cursor_glyphs_reused += 1; }
            return line.clone();
        }
        if let Some(metrics) = metrics { metrics.cursor_glyphs_rebuilt += 1; }
        let mut cursor_font = font(family.clone());
        if cell.bold { cursor_font.weight = FontWeight::BOLD; }
        if cell.italic { cursor_font.style = FontStyle::Italic; }
        let run = TextRun {
            len: cell.text.len(), font: cursor_font, color: color(cell.background),
            background_color: None, underline: None, strikethrough: None,
        };
        let line = window.text_system().shape_line(
            cell.text.to_string().into(), px(cells.font_size), &[run], Some(px(cells.width)),
        );
        self.entry = Some((cell.clone(), cells, family.clone(), window.scale_factor(), line.clone()));
        line
    }
}

pub struct RowText {
    pub line: ShapedLine,
    pub x: Pixels,
}
pub struct RowBackground {
    pub x: Pixels,
    pub width: Pixels,
    pub color: Color,
}
pub struct ShapedRow {
    source: Arc<Vec<Cell>>,
    pub texts: Vec<RowText>,
    selected_texts: RefCell<Option<(Color, Vec<RowText>)>>,
    pub backgrounds: Vec<RowBackground>,
    overlines: Vec<RowBackground>,
}

#[derive(Default)]
pub struct RowCache {
    rows: Arc<Vec<Arc<ShapedRow>>>,
    source: Weak<Vec<Arc<Vec<Cell>>>>,
    selection_color: Option<Color>,
    environment: Option<(Color, f32, CellMetrics, SharedString)>,
}
impl RowCache {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn release(&mut self) -> usize {
        let released = self.rows.len();
        *self = Self::default();
        released
    }

    pub fn prepare_selection(&mut self, frame: &Frame, foreground: Option<Color>, cells: CellMetrics, family: &SharedString, window: &Window) {
        if self.selection_color != foreground {
            for row in self.rows.iter() { row.selected_texts.borrow_mut().take(); }
            self.selection_color = foreground;
        }
        let Some(foreground) = foreground else { return; };
        for (row, selection) in self.rows.iter().zip(frame.selections.iter()) {
            if selection.is_none() { continue; }
            let mut cached = row.selected_texts.borrow_mut();
            if cached.as_ref().is_none_or(|(color, _)| *color != foreground) {
                *cached = Some((foreground, shape_texts(&row.source, cells, family, window, Some(foreground))));
            }
        }
    }

    pub fn update(
        &mut self,
        frame: &Frame,
        cells: CellMetrics,
        family: &SharedString,
        window: &Window,
        mut metrics: Option<&mut crate::diagnostics::Metrics>,
    ) -> Arc<Vec<Arc<ShapedRow>>> {
        let environment = (
            frame.background,
            window.scale_factor(),
            cells,
            family.clone(),
        );
        if self.environment.as_ref() != Some(&environment) {
            self.rows = Arc::default();
            self.source = Weak::new();
            self.environment = Some(environment);
        }
        // The immutable outer snapshot also identifies all row identities.
        // A weak reference prevents address reuse without retaining old row data
        // or forcing extra strong-reference COW when the engine publishes output.
        if self.source.as_ptr() == Arc::as_ptr(&frame.rows) {
            if let Some(metrics) = metrics { metrics.rows_reused += self.rows.len() as u64; }
            return self.rows.clone();
        }
        self.source = Arc::downgrade(&frame.rows);
        if self.rows.len() == frame.rows.len()
            && self
                .rows
                .iter()
                .zip(frame.rows.iter())
                .all(|(row, source)| Arc::ptr_eq(&row.source, source))
        {
            if let Some(metrics) = metrics {
                metrics.rows_reused += self.rows.len() as u64;
            }
            return self.rows.clone();
        }
        let rows = Arc::make_mut(&mut self.rows);
        rows.truncate(frame.rows.len());
        for (index, source) in frame.rows.iter().enumerate() {
            if rows
                .get(index)
                .is_some_and(|row| Arc::ptr_eq(&row.source, source))
            {
                if let Some(metrics) = metrics.as_deref_mut() {
                    metrics.rows_reused += 1;
                }
                continue;
            }
            if let Some(metrics) = metrics.as_deref_mut() {
                metrics.rows_rebuilt += 1;
            }
            let row = Arc::new(shape_row(
                source.clone(),
                frame.background,
                cells,
                family,
                window,
            ));
            if index < rows.len() {
                rows[index] = row;
            } else {
                rows.push(row);
            }
        }
        // Share the entire paint snapshot. Unchanged repaints neither allocate
        // a row vector nor clone one Arc per terminal row.
        self.rows.clone()
    }
}

fn shape_row(
    source: Arc<Vec<Cell>>,
    background: Color,
    cells: CellMetrics,
    family: &SharedString,
    window: &Window,
) -> ShapedRow {
    let mut backgrounds: Vec<RowBackground> = Vec::new();
    let mut overlines = Vec::new();
    // Visit every cell, independently of text grouping. A wide glyph's trailing
    // cell and empty colored cells still need their backgrounds painted.
    for cell in source.iter() {
        if cell.overline && !cell.hidden {
            append_span(
                &mut overlines,
                px(cell.column as f32 * cells.width),
                cell.foreground,
                cells,
            );
        }
        if cell.background == background {
            continue;
        }
        let x = px(cell.column as f32 * cells.width);
        if let Some(previous) = backgrounds.last_mut()
            && previous.color == cell.background
            && previous.x + previous.width == x
        {
            previous.width += px(cells.width);
        } else {
            backgrounds.push(RowBackground {
                x,
                width: px(cells.width),
                color: cell.background,
            });
        }
    }
    let texts = shape_texts(&source, cells, family, window, None);
    ShapedRow { source, texts, selected_texts: RefCell::default(), backgrounds, overlines }
}

fn shape_texts(source: &[Cell], cells: CellMetrics, family: &SharedString, window: &Window, foreground: Option<Color>) -> Vec<RowText> {
    let mut texts = Vec::new();
    let mut i = 0;
    while i < source.len() {
        let cell = &source[i];
        if cell.hidden
            || cell.width == 0
            || (cell.text.is_empty() && !cell.underline && !cell.strikethrough)
        {
            i += 1;
            continue;
        }
        let (text, end) = text_run(source, i);
        if !text.is_empty() {
            let mut run_font = font(family.clone());
            if cell.bold {
                run_font.weight = FontWeight::BOLD;
            }
            if cell.italic {
                run_font.style = FontStyle::Italic;
            }
            let run = TextRun {
                len: text.len(),
                font: run_font,
                color: color(foreground.unwrap_or(cell.foreground)),
                background_color: None,
                underline: cell.underline.then_some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(color(foreground.unwrap_or(cell.underline_color))),
                    wavy: cell.underline_curly,
                }),
                strikethrough: cell.strikethrough.then_some(StrikethroughStyle {
                    thickness: px(1.),
                    color: None,
                }),
            };
            texts.push(RowText {
                line: window.text_system().shape_line(
                    text.into(),
                    px(cells.font_size),
                    &[run],
                    Some(px(cells.width)),
                ),
                x: px(cell.column as f32 * cells.width),
            });
        }
        i = end;
    }
    texts
}

/// Empty engine cells still occupy one column and may carry decorations.
fn text_run(source: &[Cell], start: usize) -> (String, usize) {
    let cell = &source[start];
    if cell.hidden {
        return (String::new(), start + 1);
    }
    let mut end = start + 1;
    let mut bytes = cell.text.len().max(1);
    if cell.text.is_ascii() {
        while let Some(next) = source.get(end) {
            if next.hidden
                || next.width != 1
                || next.column != source[end - 1].column + 1
                || !next.text.is_ascii()
                || next.foreground != cell.foreground
                || next.bold != cell.bold
                || next.italic != cell.italic
                || next.underline != cell.underline
                || next.underline_color != cell.underline_color
                || next.underline_curly != cell.underline_curly
                || next.strikethrough != cell.strikethrough
            {
                break;
            }
            bytes += next.text.len().max(1);
            end += 1;
        }
    }
    // Determine the style run before allocating, avoiding geometric growth for
    // long ASCII rows without over-reserving for short, differently styled runs.
    let mut text = String::with_capacity(bytes);
    for cell in &source[start..end] {
        text.push_str(if cell.text.is_empty() { " " } else { &cell.text });
    }
    if text.is_ascii() {
        // Backgrounds and overlines are painted independently.
        if !cell.underline && !cell.strikethrough {
            text.truncate(text.trim_end_matches(' ').len());
        }
    }
    (text, end)
}

#[cfg(test)]
mod tests {
    use super::text_run;

    #[test]
    fn releasing_cache_drops_rows_and_backing_capacity() {
        use super::*;
        let row = Arc::new(ShapedRow {
            source: Arc::new(Vec::new()),
            texts: Vec::new(),
            selected_texts: RefCell::default(),
            backgrounds: Vec::new(),
            overlines: Vec::new(),
        });
        let weak = Arc::downgrade(&row);
        let mut cache = RowCache {
            rows: Arc::new(vec![row]),
            ..Default::default()
        };
        assert!(!cache.is_empty());
        cache.release();
        assert!(cache.is_empty());
        assert_eq!(cache.rows.capacity(), 0);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn concealed_text_never_enters_shaped_runs() -> anyhow::Result<()> {
        let mut terminal = rmux_terminal::Terminal::new(20, 2)?;
        terminal.write(b"visible\x1b[8;4;9;53msecret\x1b[0mend");
        let frame = terminal.frame()?;
        let row = &frame.rows[0];
        assert_eq!(text_run(row, 0), ("visible".into(), 7));
        for index in 7..13 {
            assert!(row[index].hidden);
            assert_eq!(text_run(row, index), (String::new(), index + 1));
        }
        assert_eq!(text_run(row, 13), ("end".into(), 20));
        Ok(())
    }

    #[test]
    fn empty_decorated_cells_keep_their_columns() -> anyhow::Result<()> {
        let mut terminal = rmux_terminal::Terminal::new(8, 2)?;
        terminal.write(b"\x1b[4m   \x1b[0m");
        let frame = terminal.frame()?;
        assert!(frame.rows[0][0].underline);
        assert_eq!(text_run(&frame.rows[0], 0), ("   ".into(), 3));
        // Test the renderer's empty-cell contract directly. ECH strips these
        // decorations in Ghostty, so it is not a fixture for this representation.
        let mut row = (*frame.rows[0]).clone();
        for cell in &mut row[..3] {
            cell.text = Default::default();
        }
        assert_eq!(text_run(&row, 0), ("   ".into(), 3));
        Ok(())
    }

    #[test]
    fn batching_preserves_styles_wide_cells_and_blank_trimming() -> anyhow::Result<()> {
        let mut terminal = rmux_terminal::Terminal::new(16, 2)?;
        terminal.write("ab\x1b[31mcd\x1b[0m界x  ".as_bytes());
        let frame = terminal.frame()?;
        let row = &frame.rows[0];
        assert_eq!(text_run(row, 0), ("ab".into(), 2));
        assert_eq!(text_run(row, 2), ("cd".into(), 4));
        assert_eq!(text_run(row, 4), ("界".into(), 5));
        assert_eq!(row[5].width, 0);
        assert_eq!(text_run(row, 6), ("x".into(), 16));
        Ok(())
    }
}

pub fn paint_row(
    row: &ShapedRow,
    origin: Point<Pixels>,
    selection: Option<(u16, u16)>,
    selection_background: Hsla,
    cells: CellMetrics,
    window: &mut Window,
    cx: &mut App,
) {
    for background in &row.backgrounds {
        window.paint_quad(fill(
            Bounds::new(
                origin + point(background.x, px(0.)),
                size(background.width, px(cells.height)),
            ),
            color(background.color),
        ));
    }
    if let Some((start, end)) = selection {
        window.paint_quad(fill(
            Bounds::new(
                origin + point(px(start as f32 * cells.width), px(0.)),
                size(px((end - start + 1) as f32 * cells.width), px(cells.height)),
            ),
            selection_background,
        ));
    }
    let paint = |texts: &[RowText], foreground: Option<Color>, window: &mut Window, cx: &mut App| {
        for text in texts {
            let _ = text.line.paint(origin + point(text.x, px(0.)), px(cells.height), window, cx);
        }
        for span in &row.overlines {
            window.paint_quad(fill(
                Bounds::new(origin + point(span.x, px(1.)), size(span.width, px(1.))),
                color(foreground.unwrap_or(span.color)),
            ));
        }
    };
    let selected_texts = row.selected_texts.borrow();
    if let (Some((start, end)), Some((foreground, texts))) = (selection, selected_texts.as_ref()) {
        // Clip both variants so antialiased glyph edges never blend with the
        // original foreground underneath the selected variant.
        let left = start as f32 * cells.width;
        let right = (end as f32 + 1.) * cells.width;
        let width = row.source.len() as f32 * cells.width;
        for (x, width) in [(0., left), (right, (width - right).max(0.))] {
            if width > 0. {
                window.with_content_mask(Some(ContentMask { bounds: Bounds::new(origin + point(px(x), px(0.)), size(px(width), px(cells.height))) }), |window| paint(&row.texts, None, window, cx));
            }
        }
        window.with_content_mask(Some(ContentMask { bounds: Bounds::new(origin + point(px(left), px(0.)), size(px(right - left), px(cells.height))) }), |window| paint(texts, Some(*foreground), window, cx));
    } else {
        paint(&row.texts, None, window, cx);
    }
}
fn append_span(spans: &mut Vec<RowBackground>, x: Pixels, color: Color, cells: CellMetrics) {
    if let Some(previous) = spans.last_mut()
        && previous.color == color
        && previous.x + previous.width == x
    {
        previous.width += px(cells.width);
    } else {
        spans.push(RowBackground {
            x,
            width: px(cells.width),
            color,
        });
    }
}
