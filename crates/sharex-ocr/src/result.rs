//! `ShareX.OCR/OCRResult.cs`: the recognized pieces of text, joined into lines in
//! reading order, with paragraph breaks found from the line spacing.

use crate::geometry::{self, Point, Rect, cs_max, cs_min};

/// Two regions are on the same line when they share at least this much of the
/// shorter one's height.
const SAME_LINE_OVERLAP: f32 = 0.5;
/// Line spacing assumed inside a paragraph, as a share of the text height, when
/// there are too few lines to measure it.
const DEFAULT_LINE_GAP: f32 = 0.35;
/// A gap splits a paragraph when it is at least twice the line spacing and wider by
/// at least this share of the text height.
const PARAGRAPH_GAP_MARGIN: f32 = 0.35;
/// Lines whose text heights differ this much, such as a heading and the text under
/// it, are in different paragraphs.
const PARAGRAPH_HEIGHT_RATIO: f32 = 1.6;

/// `OCRTextRegion`: one detected piece of text. `points` are its box's corners in
/// image coordinates: top-left, top-right, bottom-right, bottom-left.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrTextRegion {
    pub points: [Point; 4],
    pub text: String,
    pub score: f32,
    pub bounds: Rect,
}

impl OcrTextRegion {
    pub fn new(points: [Point; 4], text: String, score: f32) -> Self {
        let bounds = geometry::bounds(&points);
        Self {
            points,
            text,
            score,
            bounds,
        }
    }
}

/// `OCRTextLine`: regions side by side, joined in reading order. `height` is the
/// typical text height of the line; `starts_paragraph` is set when the line is set
/// apart from the one above it.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrTextLine {
    pub text: String,
    pub bounds: Rect,
    pub height: f32,
    pub starts_paragraph: bool,
}

/// `OCRResult`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OcrResult {
    pub regions: Vec<OcrTextRegion>,
    pub lines: Vec<OcrTextLine>,
}

/// `Environment.NewLine`.
pub const NEW_LINE: &str = if cfg!(windows) { "\r\n" } else { "\n" };

impl OcrResult {
    pub fn new(regions: Vec<OcrTextRegion>) -> Self {
        let lines = group_lines(&regions);
        Self { regions, lines }
    }

    /// `GetText`: lines joined with a new line, and an empty line between paragraphs
    /// when `separate_paragraphs`; or everything on one line, joined with spaces.
    pub fn text(&self, single_line: bool, separate_paragraphs: bool) -> String {
        if single_line {
            return self
                .lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
        }
        let mut text = String::new();
        for line in &self.lines {
            if !text.is_empty() {
                text.push_str(NEW_LINE);
                if separate_paragraphs && line.starts_paragraph {
                    text.push_str(NEW_LINE);
                }
            }
            text.push_str(&line.text);
        }
        text
    }
}

impl std::fmt::Display for OcrResult {
    /// `ToString`: `GetText()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text(false, true))
    }
}

/// `GroupLines`: regions side by side become lines, read top to bottom and left to right.
fn group_lines(regions: &[OcrTextRegion]) -> Vec<OcrTextLine> {
    let mut sorted: Vec<&OcrTextRegion> = regions.iter().collect();
    sorted.sort_by(|a, b| {
        a.bounds
            .mid_y()
            .partial_cmp(&b.bounds.mid_y())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut groups: Vec<Vec<&OcrTextRegion>> = Vec::new();
    for region in sorted {
        match groups.last_mut() {
            Some(group) if is_same_line(group, region) => group.push(region),
            _ => groups.push(vec![region]),
        }
    }
    let lines = groups.iter().map(|g| create_line(g)).collect();
    mark_paragraphs(lines)
}

fn create_line(group: &[&OcrTextRegion]) -> OcrTextLine {
    let mut ordered = group.to_vec();
    ordered.sort_by(|a, b| {
        a.bounds
            .left
            .partial_cmp(&b.bounds.left)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let text = ordered
        .iter()
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut bounds = ordered[0].bounds;
    for r in &ordered {
        bounds.union(&r.bounds);
    }
    // The median keeps one tall or short box, such as a symbol, from deciding the
    // height of the whole line.
    let height = median(ordered.iter().map(|r| r.bounds.height()).collect());
    OcrTextLine {
        text,
        bounds,
        height,
        starts_paragraph: false,
    }
}

/// `Enumerable.Min` / `Max` over floats (no NaNs here): the first smallest/largest.
fn linq_min(values: impl Iterator<Item = f32>) -> f32 {
    values
        .reduce(|m, v| if v < m { v } else { m })
        .unwrap_or(0.0)
}

fn linq_max(values: impl Iterator<Item = f32>) -> f32 {
    values
        .reduce(|m, v| if v > m { v } else { m })
        .unwrap_or(0.0)
}

fn is_same_line(line: &[&OcrTextRegion], region: &OcrTextRegion) -> bool {
    let top = linq_min(line.iter().map(|r| r.bounds.top));
    let bottom = linq_max(line.iter().map(|r| r.bounds.bottom));
    let overlap = cs_min(bottom, region.bounds.bottom) - cs_max(top, region.bounds.top);
    let height = cs_min(bottom - top, region.bounds.height());
    height > 0.0 && overlap >= height * SAME_LINE_OVERLAP
}

/// `MarkParagraphs`: a line starts a paragraph when it is clearly further from the
/// line above than the usual line spacing, when the text size changes (a heading),
/// or when it doesn't sit under the line above at all (text elsewhere on screen).
fn mark_paragraphs(mut lines: Vec<OcrTextLine>) -> Vec<OcrTextLine> {
    if lines.len() < 2 {
        return lines;
    }
    let line_gap = line_gap(&lines);
    for i in 1..lines.len() {
        let (previous, line) = (&lines[i - 1], &lines[i]);
        let height = cs_min(previous.height, line.height);
        let starts = height > 0.0
            && (gap(previous, line) / height
                > cs_max(line_gap * 2.0, line_gap + PARAGRAPH_GAP_MARGIN)
                || cs_max(previous.height, line.height) / height >= PARAGRAPH_HEIGHT_RATIO
                || !overlaps_horizontally(previous, line));
        lines[i].starts_paragraph = starts;
    }
    lines
}

/// `GetLineGap`: the line spacing inside paragraphs, as a share of the text height:
/// a low percentile of the gaps between lines that could share a paragraph.
fn line_gap(lines: &[OcrTextLine]) -> f32 {
    let mut gaps = Vec::new();
    for pair in lines.windows(2) {
        let (previous, line) = (&pair[0], &pair[1]);
        let height = cs_min(previous.height, line.height);
        if height > 0.0
            && cs_max(previous.height, line.height) / height < PARAGRAPH_HEIGHT_RATIO
            && overlaps_horizontally(previous, line)
        {
            gaps.push(cs_max(gap(previous, line), 0.0) / height);
        }
    }
    if gaps.len() < 2 {
        return DEFAULT_LINE_GAP;
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    gaps[(gaps.len() - 1) / 4]
}

fn gap(upper: &OcrTextLine, lower: &OcrTextLine) -> f32 {
    lower.bounds.top - upper.bounds.bottom
}

fn overlaps_horizontally(a: &OcrTextLine, b: &OcrTextLine) -> bool {
    cs_min(a.bounds.right, b.bounds.right) > cs_max(a.bounds.left, b.bounds.left)
}

fn median(mut values: Vec<f32>) -> f32 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(text: &str, l: f32, t: f32, r: f32, b: f32) -> OcrTextRegion {
        OcrTextRegion::new(
            [
                Point::new(l, t),
                Point::new(r, t),
                Point::new(r, b),
                Point::new(l, b),
            ],
            text.into(),
            0.9,
        )
    }

    #[test]
    fn regions_join_into_lines_and_paragraphs() {
        let result = OcrResult::new(vec![
            region("world", 60.0, 10.0, 110.0, 30.0),
            region("Hello", 0.0, 12.0, 50.0, 30.0),
            region("second", 0.0, 37.0, 60.0, 55.0),
            region("third", 0.0, 62.0, 50.0, 80.0),
            region("apart", 0.0, 140.0, 50.0, 158.0),
        ]);
        let n = NEW_LINE;
        assert_eq!(
            result.text(false, true),
            format!("Hello world{n}second{n}third{n}{n}apart")
        );
        assert_eq!(
            result.text(false, false),
            format!("Hello world{n}second{n}third{n}apart")
        );
        assert_eq!(result.text(true, true), "Hello world second third apart");
    }
}
