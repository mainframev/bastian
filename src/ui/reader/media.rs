use ratatui::Frame;
use ratatui::layout::Rect;

use crate::reader::Reader;
use crate::render::LinkSpan;

/// Pictures over the rows reserved for them. A picture partly scrolled
/// off shows its visible rows: the slot-sized picture is cropped from the
/// top or the bottom.
pub(super) fn draw_images(frame: &mut Frame, r: &mut Reader, content: Rect, end: usize) {
    let scroll = r.scroll();
    let slots: Vec<_> = r
        .rendered()
        .images
        .iter()
        .filter(|s| s.line < end && s.line + usize::from(s.rows) > scroll)
        .cloned()
        .collect();
    for slot in slots {
        let Some(protocol) = r.images.protocol(&slot.url, slot.cols, slot.rows) else {
            continue;
        };
        let top = slot.line.max(scroll);
        let bottom = (slot.line + usize::from(slot.rows)).min(end);
        let x = content.x + slot.col as u16;
        let area = Rect::new(
            x,
            content.y + (top - scroll) as u16,
            slot.cols.min(content.right().saturating_sub(x)),
            (bottom - top) as u16,
        );
        let crop = ratatui_image::CropOptions {
            clip_top: slot.line < scroll,
            clip_left: false,
        };
        frame.render_stateful_widget(
            ratatui_image::StatefulImage::default().resize(ratatui_image::Resize::Crop(Some(crop))),
            area,
            protocol,
        );
    }
}

/// The cells of `link` on screen, in `content` scrolled to `scroll`,
/// cut off at its right edge.
pub(super) fn link_rect(content: Rect, link: &LinkSpan, scroll: usize) -> Rect {
    let x = content.x + link.col as u16;
    let width = (link.width as u16).min(content.right().saturating_sub(x));
    Rect::new(x, content.y + (link.line - scroll) as u16, width, 1)
}

/// Make visible links clickable: an OSC 8 hyperlink over each link's text.
/// Lines with search matches keep their highlighting instead.
pub(super) fn draw_hyperlinks(
    frame: &mut Frame,
    r: &Reader,
    content: Rect,
    end: usize,
    skip: &[usize],
) {
    let first = r.rendered().links.partition_point(|l| l.line < r.scroll());
    for (i, link) in r.rendered().links[first..]
        .iter()
        .enumerate()
        .take_while(|(_, l)| l.line < end)
    {
        if skip.contains(&(first + i)) {
            continue;
        }
        if r.search.as_ref().is_some_and(|s| {
            // Matches are in line order.
            let i = s.matches.partition_point(|m| m.line < link.line);
            s.matches.get(i).is_some_and(|m| m.line == link.line)
        }) {
            continue;
        }
        let Some(target) = r.hyperlink_targets().get(first + i).cloned().flatten() else {
            continue;
        };
        let line = &r.rendered().lines[link.line];
        let (mut label, mut style, mut col) = (String::new(), None, 0);
        for span in &line.spans {
            let w = span.width();
            if col + w > link.col && col < link.col + link.width {
                // hyperrat writes the label raw, past ratatui's filter.
                label.push_str(&crate::render::strip_controls(&span.content));
                style.get_or_insert(span.style);
            }
            col += w;
        }
        let area = link_rect(content, link, r.scroll());
        frame.render_widget(
            hyperrat::Link::new(label, target).style(style.unwrap_or_default()),
            area,
        );
        // The first cell holds the whole escape sequence plus the label.
        // Tell the diff how many columns it really covers, or it measures the
        // escape sequence and skips the rest of the line.
        if let Some(width) = std::num::NonZeroU16::new(area.width)
            && let Some(cell) = frame.buffer_mut().cell_mut((area.x, area.y))
        {
            cell.set_diff_option(ratatui::buffer::CellDiffOption::ForcedWidth(width));
        }
    }
}
