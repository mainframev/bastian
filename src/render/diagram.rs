//! Mermaid diagrams drawn with box-drawing characters.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use merman::ascii::{AsciiTrimPolicy, AsciiViewportPolicy, OverflowPolicy};
use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};

/// Longest a diagram may take to lay out; past it the block shows its
/// source, so a pathological diagram can't stall the reader.
const DEADLINE: Duration = Duration::from_millis(250);

/// Most drawings kept; past it the cache starts over.
const CACHE_SIZE: usize = 256;

/// Drawings, and failures, by source and width. A drawing depends on
/// nothing else, so entries never go stale, and a block that hits the
/// deadline costs it once rather than on every render.
type Drawings = HashMap<(String, usize), Option<Vec<String>>>;
static CACHE: LazyLock<Mutex<Drawings>> = LazyLock::new(Mutex::default);

/// `src` drawn as lines at most `width` columns wide, or `None` when it
/// doesn't parse, isn't a diagram type that draws as text, doesn't fit, or
/// takes too long.
pub fn draw(src: &str, width: usize) -> Option<Vec<String>> {
    let key = (src.to_string(), width);
    if let Some(hit) = CACHE.lock().ok()?.get(&key) {
        return hit.clone();
    }
    let drawn = draw_uncached(src, width);
    if let Ok(mut cache) = CACHE.lock() {
        if cache.len() >= CACHE_SIZE {
            cache.clear();
        }
        cache.insert(key, drawn.clone());
    }
    drawn
}

fn draw_uncached(src: &str, width: usize) -> Option<Vec<String>> {
    let request = AsciiRequest {
        viewport: AsciiViewportPolicy::with_max_width(width.max(1))
            .overflow(OverflowPolicy::Error)
            .trim(AsciiTrimPolicy::TrimTrailingSpaces),
        ..AsciiRequest::default()
    };
    let control = OperationControl::new().with_deadline(DEADLINE);
    match Renderer::new().render(RenderRequest::ascii(src, control, request)) {
        Ok(RenderOutput::Ascii(Some(out))) => Some(
            out.as_text()
                .trim_end_matches('\n')
                .lines()
                .map(str::to_string)
                .collect(),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::parse;
    use crate::render::theme::Theme;
    use crate::render::{render, to_plain};

    const FLOW: &str = "flowchart TD\n  A[Start] --> B[Done]\n";

    #[test]
    fn draws_a_flowchart() {
        let lines = draw(FLOW, 80).unwrap();
        let text = lines.join("\n");
        assert!(text.contains("Start") && text.contains("Done"), "{text}");
        assert!(text.contains('▼'), "{text}");
        assert!(lines.iter().all(|l| l == l.trim_end()), "{lines:?}");
    }

    #[test]
    fn falls_back_when_it_cannot_draw() {
        assert_eq!(draw("flowchart TD\n  A --> \n", 80), None, "invalid");
        assert_eq!(draw("pie\n  \"a\" : 1\n", 80), None, "no text form");
        assert_eq!(draw(FLOW, 4), None, "too narrow");
    }

    #[test]
    fn mermaid_blocks_render_as_diagrams_or_their_source() {
        let doc = parse(&format!("```mermaid\n{FLOW}```\n"));
        let drawn = to_plain(&render(&doc, 80, &Theme::dark()).lines);
        assert!(drawn.contains('▼') && !drawn.contains("-->"), "{drawn}");
        // Four columns inside the frame: the source wraps at four.
        let narrow = to_plain(&render(&doc, 8, &Theme::dark()).lines);
        assert!(narrow.contains("flow") && !narrow.contains('▼'), "{narrow}");
    }
}
