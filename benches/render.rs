//! Parse and render generated documents, after Glamour's render benchmarks.
//! Run with `cargo bench`; timings are for comparing runs on one machine.

use bastian::doc::parse;
use bastian::render::render;
use bastian::render::theme::Theme;
use divan::Bencher;
use divan::counter::BytesCount;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

const WIDTH: u16 = 120;

/// `n` paragraphs long enough to wrap over several lines.
fn prose_doc(n: usize) -> String {
    (0..n)
        .map(|i| {
            format!(
                "Considering angle {i} of the question in some detail, the constraint holds for every branch here and the wrapping needs to run across several lines to be representative.\n\n"
            )
        })
        .collect()
}

/// `n` blocks of a heading, styled prose, a list and a code block.
fn mixed_doc(n: usize) -> String {
    (0..n)
        .map(|i| {
            format!(
                "## Heading {i}\n\nSome **bold** and _italic_ prose with `code` and a [link](https://example.com) that wraps.\n\n- item one\n- item two\n\n```rust\nfn f() {{ return }}\n```\n\n"
            )
        })
        .collect()
}

fn bench(bencher: Bencher, doc: &str) {
    let theme = Theme::dark();
    bencher
        .counter(BytesCount::of_str(doc))
        .bench(|| render(&parse(divan::black_box(doc)), WIDTH, &theme));
}

#[divan::bench(args = [1, 10, 100])]
fn render_prose(bencher: Bencher, paras: usize) {
    bench(bencher, &prose_doc(paras));
}

#[divan::bench(args = [1, 10, 100])]
fn render_mixed(bencher: Bencher, blocks: usize) {
    bench(bencher, &mixed_doc(blocks));
}

fn main() {
    divan::main();
}
