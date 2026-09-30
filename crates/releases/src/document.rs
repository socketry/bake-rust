use bake::{Error, Result};
use std::ops::Range;

struct Heading<'document> {
    level: usize,
    title: &'document str,
    title_range: Range<usize>,
    start: usize,
    body_start: usize,
}

// Release documents use unindented ATX headings. Track fenced code blocks so
// example headings cannot accidentally delimit a release or become an update.
fn headings(document: &str) -> Vec<Heading<'_>> {
    let mut headings = Vec::new();
    let mut offset = 0;
    let mut fence: Option<(u8, usize)> = None;
    for line in document.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        let trimmed = text.trim_start_matches(' ');
        let indentation = text.len() - trimmed.len();
        let marker = trimmed.as_bytes().first().copied().unwrap_or_default();
        let count = trimmed.bytes().take_while(|byte| *byte == marker).count();
        if let Some((open_marker, open_count)) = fence {
            if indentation <= 3
                && marker == open_marker
                && count >= open_count
                && trimmed[count..].trim().is_empty()
            {
                fence = None;
            }
        } else if indentation <= 3
            && matches!(marker, b'`' | b'~')
            && count >= 3
            && (marker != b'`' || !trimmed[count..].contains('`'))
        {
            fence = Some((marker, count));
        } else if indentation == 0
            && marker == b'#'
            && (1..=6).contains(&count)
            && (text.len() == count || text.as_bytes()[count].is_ascii_whitespace())
        {
            let remainder = &text[count..];
            let title_start = offset + count + remainder.len() - remainder.trim_start().len();
            let title = remainder.trim();
            let without_closing = title.trim_end_matches('#');
            let title = if without_closing.is_empty() || without_closing.ends_with([' ', '\t']) {
                without_closing.trim_end()
            } else {
                title
            };
            headings.push(Heading {
                level: count,
                title,
                title_range: title_start..title_start + title.len(),
                start: offset,
                body_start: offset + line.len(),
            });
        }
        offset += line.len();
    }
    headings
}

fn unique_heading<'headings, 'document>(
    headings: &'headings [Heading<'document>],
    title: &str,
) -> Result<&'headings Heading<'document>> {
    let mut matching = headings.iter().filter(|heading| heading.title == title);
    let heading = matching
        .next()
        .ok_or_else(|| Error::new(format!("release heading {title:?} not found")))?;
    if matching.next().is_some() {
        return Err(Error::new(format!(
            "release heading {title:?} is ambiguous"
        )));
    }
    Ok(heading)
}

/// Extract the body under an exact ATX heading, retaining nested sections and
/// original Markdown bytes. Missing or duplicate headings are errors.
///
/// Headings must be unindented, e.g. `## v0.1.0`. Setext headings, HTML blocks,
/// and headings inside block quotes or lists are outside this document format.
pub fn extract_notes<'document>(document: &'document str, version: &str) -> Result<&'document str> {
    let headings = headings(document);
    let heading = unique_heading(&headings, version)?;
    let end = headings
        .iter()
        .find(|candidate| candidate.start > heading.start && candidate.level <= heading.level)
        .map_or(document.len(), |candidate| candidate.start);
    Ok(&document[heading.body_start..end])
}

/// Replace exactly one `Unreleased` heading without reformatting the document.
/// Existing version headings and multiline version strings are rejected.
pub fn update_document(document: &str, version: &str) -> Result<String> {
    if version.is_empty()
        || version.trim() != version
        || version.chars().any(char::is_control)
        || version.contains('#')
        || version == "Unreleased"
    {
        return Err(Error::new(
            "version must be a nonempty single-line heading other than Unreleased, without # characters",
        ));
    }
    let headings = headings(document);
    if headings.iter().any(|heading| heading.title == version) {
        return Err(Error::new(format!(
            "release heading {version:?} already exists"
        )));
    }
    let heading = unique_heading(&headings, "Unreleased")?;
    let mut output = document.to_owned();
    output.replace_range(heading.title_range.clone(), version);
    Ok(output)
}
