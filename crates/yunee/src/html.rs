//! Minimal, safe HTML → text / Pango markup.
//!
//! Canvas bodies (announcements, assignment descriptions) arrive as HTML. Yunee
//! never renders that HTML in a widget — it converts to either plain text or a
//! small, escaped subset of Pango markup, so nothing Canvas sends can inject
//! markup or break the label.

/// Strip HTML down to readable plain text.
pub fn to_plain(html: &str) -> String {
    convert(html, false)
}

/// Convert to safe Pango markup (bold/italic + line breaks only).
pub fn to_pango(html: &str) -> String {
    convert(html, true)
}

/// A piece of a Canvas HTML body: a run of markup, or an image to render.
pub enum Block {
    /// A fragment of HTML, rendered to text/Pango by [`to_pango`].
    Text(String),
    /// An `<img>` the view should fetch and draw.
    Image(Image),
}

/// An image referenced by a Canvas body.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub src: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

/// Split a Canvas HTML body into text runs and images, so a view can render
/// images inline at their position instead of dropping them.
pub fn blocks(html_text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut rest = html_text;
    while let Some(start) = find_img(rest) {
        let (before, tail) = rest.split_at(start);
        push_text(&mut out, before);
        let end = tail.find('>').map(|i| i + 1).unwrap_or(tail.len());
        if let Some(image) = parse_img(&tail[..end]) {
            out.push(Block::Image(image));
        }
        rest = &tail[end..];
    }
    push_text(&mut out, rest);
    out
}

fn find_img(s: &str) -> Option<usize> {
    s.to_ascii_lowercase().find("<img")
}

fn push_text(out: &mut Vec<Block>, fragment: &str) {
    if !fragment.trim().is_empty() {
        out.push(Block::Text(fragment.to_string()));
    }
}

fn parse_img(tag: &str) -> Option<Image> {
    let src = attr(tag, "src")?;
    Some(Image {
        src,
        alt: attr(tag, "alt"),
        width: attr(tag, "width").and_then(|v| v.trim().parse().ok()),
        height: attr(tag, "height").and_then(|v| v.trim().parse().ok()),
    })
}

/// Read `name="value"` (single or double quoted, or bare) from a tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=");
    let mut from = 0;
    while let Some(rel) = lower[from..].find(&needle) {
        let at = from + rel;
        let boundary = at == 0 || tag.as_bytes()[at - 1].is_ascii_whitespace();
        let value_start = at + needle.len();
        if boundary && value_start < tag.len() {
            let rest = &tag[value_start..];
            let (quote, body) = match rest.chars().next() {
                Some('"') => (Some('"'), &rest[1..]),
                Some('\'') => (Some('\''), &rest[1..]),
                _ => (None, rest),
            };
            let end = match quote {
                Some(q) => body.find(q).unwrap_or(body.len()),
                None => body
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(body.len()),
            };
            return Some(decode_entities(&body[..end]));
        }
        from = at + needle.len();
    }
    None
}

fn convert(html: &str, pango: bool) -> String {
    let chars: Vec<char> = html.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;

    while i < n {
        if chars[i] == '<' {
            let start = i + 1;
            let mut j = start;
            while j < n && chars[j] != '>' {
                j += 1;
            }
            let tag: String = chars[start..j].iter().collect();
            apply_tag(&tag, pango, &mut out);
            i = if j < n { j + 1 } else { n };
        } else {
            let start = i;
            let mut j = i;
            while j < n && chars[j] != '<' {
                j += 1;
            }
            let text: String = chars[start..j].iter().collect();
            let decoded = decode_entities(&text);
            if pango {
                push_escaped(&mut out, &decoded);
            } else {
                out.push_str(&decoded);
            }
            i = j;
        }
    }

    tidy(&out)
}

fn apply_tag(tag: &str, pango: bool, out: &mut String) {
    let tag = tag.trim();
    let closing = tag.starts_with('/');
    let name: String = tag
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    match name.as_str() {
        "br" => out.push('\n'),
        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "tr" | "ul" | "ol" => {
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }
        "li" => {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("• ");
        }
        "b" | "strong" if pango => out.push_str(if closing { "</b>" } else { "<b>" }),
        "i" | "em" if pango => out.push_str(if closing { "</i>" } else { "<i>" }),
        _ => {}
    }
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i] == '&' {
            if let Some(semi) = chars[i..].iter().position(|c| *c == ';').map(|p| i + p) {
                if semi - i <= 10 {
                    let entity: String = chars[i + 1..semi].iter().collect();
                    if let Some(decoded) = decode_entity(&entity) {
                        out.push(decoded);
                        i = semi + 1;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "mdash" => Some('—'),
        "ndash" => Some('–'),
        "hellip" => Some('…'),
        "rsquo" => Some('’'),
        "lsquo" => Some('‘'),
        "ldquo" => Some('“'),
        "rdquo" => Some('”'),
        _ => {
            if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = entity.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }
    }
}

fn push_escaped(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

/// Collapse the mess: trim line ends, and never more than one blank line.
fn tidy(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut blank_lines = 0;
    for raw in input.lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            blank_lines += 1;
            if blank_lines > 1 {
                continue;
            }
        } else {
            blank_lines = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_strips_tags_and_decodes() {
        let html =
            "<p>Read <b>chapter&nbsp;3</b> &amp; notes.</p><ul><li>One</li><li>Two</li></ul>";
        let text = to_plain(html);
        assert!(text.contains("Read chapter 3 & notes."), "got: {text:?}");
        assert!(text.contains("• One"));
        assert!(text.contains("• Two"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn pango_keeps_bold_but_escapes_input() {
        let html = "<p>a <b>bold</b> &lt;script&gt;</p>";
        let markup = to_pango(html);
        assert!(markup.contains("<b>bold</b>"));
        assert!(markup.contains("&lt;script&gt;"));
        assert!(!markup.contains("<script>"));
    }

    #[test]
    fn numeric_entities_decode() {
        assert_eq!(to_plain("A&#39;s &#x2014; B"), "A's — B");
    }

    #[test]
    fn splits_text_and_images_by_position() {
        let html = r#"<p>Before</p><img src="a.png" width="10" height="20" alt="A"><p>After</p>"#;
        let blocks = blocks(html);
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Text(t) if t.contains("Before")));
        match &blocks[1] {
            Block::Image(img) => {
                assert_eq!(img.src, "a.png");
                assert_eq!(img.alt.as_deref(), Some("A"));
                assert_eq!(img.width, Some(10));
                assert_eq!(img.height, Some(20));
            }
            _ => panic!("expected an image block"),
        }
        assert!(matches!(&blocks[2], Block::Text(t) if t.contains("After")));
    }

    #[test]
    fn image_source_entities_decode_and_quotes_work() {
        let html = "<img src='/a.png?x=1&amp;y=2' alt=''>";
        match &blocks(html)[0] {
            Block::Image(img) => assert_eq!(img.src, "/a.png?x=1&y=2"),
            _ => panic!("expected an image block"),
        }
    }
}
