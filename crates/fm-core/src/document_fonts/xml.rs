//! A small, non-validating XML tokenizer: start, end and text events with
//! the five predefined entities and character references decoded. Enough for
//! Office parts and XMP packets; namespaces are not resolved, callers match
//! the local name (after the prefix).

use super::{Bad, Res};

/// One token.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Event<'a> {
    /// `<name attr="v">` or, with `empty`, `<name attr="v"/>`.
    Start {
        name: &'a str,
        attrs: Vec<(&'a str, String)>,
        empty: bool,
    },
    /// `</name>`.
    End { name: &'a str },
    /// Character data between tags, CDATA included.
    Text(String),
}

/// The part of a qualified name after the prefix (`w:rFonts` → `rFonts`).
pub(super) fn local(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, l)| l)
}

/// The value of the attribute whose local name is `key`.
pub(super) fn attr<'v>(attrs: &'v [(&str, String)], key: &str) -> Option<&'v str> {
    attrs
        .iter()
        .find(|(k, _)| local(k) == key)
        .map(|(_, v)| v.as_str())
}

/// Every event of `s`, in order.
pub(super) fn parse(s: &str) -> Res<Vec<Event<'_>>> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            push_text(&mut out, rest)?;
            break;
        };
        push_text(&mut out, &rest[..lt])?;
        rest = &rest[lt..];
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = after(r, "-->")?;
        } else if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let end = r.find("]]>").ok_or_else(|| unclosed("CDATA"))?;
            out.push(Event::Text(r[..end].to_owned()));
            rest = &r[end + 3..];
        } else if let Some(r) = rest.strip_prefix("<?") {
            rest = after(r, "?>")?;
        } else if let Some(r) = rest.strip_prefix("<!") {
            rest = skip_declaration(r)?;
        } else if let Some(r) = rest.strip_prefix("</") {
            let end = r.find('>').ok_or_else(|| unclosed("end tag"))?;
            out.push(Event::End {
                name: r[..end].trim(),
            });
            rest = &r[end + 1..];
        } else {
            rest = start_tag(&rest[1..], &mut out)?;
        }
    }
    Ok(out)
}

fn unclosed(what: &str) -> Bad {
    Bad::malformed(format!("XML {what} is not closed"))
}

fn after<'a>(s: &'a str, end: &str) -> Res<&'a str> {
    let i = s.find(end).ok_or_else(|| unclosed(end))?;
    Ok(&s[i + end.len()..])
}

/// `<!DOCTYPE …>` with an optional `[…]` internal subset.
fn skip_declaration(s: &str) -> Res<&str> {
    let mut depth = 0u32;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '>' if depth == 0 => return Ok(&s[i + 1..]),
            _ => {}
        }
    }
    Err(unclosed("declaration"))
}

fn push_text<'a>(out: &mut Vec<Event<'a>>, raw: &str) -> Res<()> {
    if !raw.is_empty() {
        out.push(Event::Text(unescape(raw)?));
    }
    Ok(())
}

/// `s` starts right after `<`; returns what follows the tag.
fn start_tag<'a>(s: &'a str, out: &mut Vec<Event<'a>>) -> Res<&'a str> {
    let is_space = |c: char| c.is_ascii_whitespace();
    let name_end = s
        .find(|c: char| is_space(c) || c == '/' || c == '>')
        .ok_or_else(|| unclosed("start tag"))?;
    let name = &s[..name_end];
    if name.is_empty() {
        return Err(Bad::malformed("XML tag without a name"));
    }
    let mut rest = &s[name_end..];
    let mut attrs = Vec::new();
    loop {
        rest = rest.trim_start_matches(is_space);
        if let Some(r) = rest.strip_prefix("/>") {
            out.push(Event::Start {
                name,
                attrs,
                empty: true,
            });
            return Ok(r);
        }
        if let Some(r) = rest.strip_prefix('>') {
            out.push(Event::Start {
                name,
                attrs,
                empty: false,
            });
            return Ok(r);
        }
        let eq = rest.find('=').ok_or_else(|| unclosed("attribute"))?;
        let key = rest[..eq].trim();
        if key.is_empty() || key.contains(|c: char| is_space(c) || c == '>' || c == '<') {
            return Err(Bad::malformed("XML attribute without a name"));
        }
        let r = rest[eq + 1..].trim_start_matches(is_space);
        let quote = r
            .chars()
            .next()
            .filter(|q| *q == '"' || *q == '\'')
            .ok_or_else(|| Bad::malformed("XML attribute value is not quoted"))?;
        let body = &r[1..];
        let end = body
            .find(quote)
            .ok_or_else(|| unclosed("attribute value"))?;
        attrs.push((key, unescape(&body[..end])?));
        rest = &body[end + 1..];
    }
}

/// Decodes `&lt;` `&gt;` `&amp;` `&quot;` `&apos;` and `&#…;`; an unknown
/// entity (declared in a DTD this reader does not read) stays as written.
fn unescape(s: &str) -> Res<String> {
    if !s.contains('&') {
        return Ok(s.to_owned());
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let r = &rest[amp + 1..];
        let semi = r
            .find(';')
            .ok_or_else(|| Bad::malformed("XML entity not closed"))?;
        let ent = &r[..semi];
        let ch = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent.strip_prefix('#').map(char_ref).transpose()?,
        };
        match ch {
            Some(c) => out.push(c),
            None => {
                out.push('&');
                out.push_str(ent);
                out.push(';');
            }
        }
        rest = &r[semi + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn char_ref(num: &str) -> Res<char> {
    let n = match num.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => num.parse(),
    };
    n.ok()
        .and_then(char::from_u32)
        .ok_or_else(|| Bad::malformed("XML character reference out of range"))
}
