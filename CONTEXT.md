# Context

bastian is a terminal Markdown reader. This glossary gives each domain term one
meaning; code, docs and discussion use these words and no synonyms.

## Terms

**Document**: a parsed Markdown text as bastian understands it: a sequence of
Blocks plus where each top-level Block began in the source. A Document does
not know where it came from.

**Source** (of a document): where a Document came from, which decides how its
links resolve, whether it is watched for changes and can be edited, whether
runs ask first, and whether its reading Position is remembered. Three kinds:
in memory (stdin, the theme preview), a file on disk, or a remote document
fetched over HTTP.

**Reader**: the view of one Document: its layout for a width, the scroll
position, search, focus, Overlays and running code. One Reader per open
document; going Back returns to the previous Reader.

**Block**: one unit of a Document: heading, paragraph, list, quote, callout,
code block, footnote, table, rule, image, label, details.

**Run**: one execution of a code block's program, with its output and status.
A code block has at most one Run at a time.

**Position** (reading position): where a document was left, as a source line
and a fraction inside its Block, so it survives re-rendering at another width.
Distinct from the scroll offset, which is a rendered line number.

**Overlay**: something that takes the keyboard away from the document until it
is dismissed: help, the link picker, the location popup, a run confirmation,
an open-external confirmation, the search prompt. At most one Overlay is open
at a time; opening one replaces another, draft and all.

**Focus**: a highlighted thing in the document that keys act on while the
document still scrolls: a code block, a link, or a details block. Leaving
focus (and an active search) returns to plain reading in one step.

**Table of contents** (ToC): a pane listing the document's headings; picking
one jumps to it. It sits beside the document and is not an Overlay. The word
"outline" is retired.

**Location**: where the document came from as shown to the user: a file path,
a URL, or just a name for stdin and in-memory documents.

**Image origin**: where a picture loads from: a file next to the document or a
URL. Not to be confused with a document's Source.

**Remote**: a URL, GitHub repository or GitLab project a document is fetched
from.

**Entry**: one Markdown file listed by the file browser, with its title and
dates.

**Theme**: a named set of colors for the document and for highlighted code,
built from a theme file; built-in or the user's own.

**Page background**: the color a Theme paints behind the whole reader and
browser screen. `background = "terminal"` leaves the terminal's own color
showing instead; printed output never paints it.
