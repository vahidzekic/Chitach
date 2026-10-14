//! Minimal XML tree, parser, serializer and inclusive C14N 1.0.
//!
//! Port of bas-celik (MIT, (c) Nikola Ubavić). Bas Celik signs XML documents with the Go
//! libraries `beevik/etree` (parsing / serialisation), `moov-io/signedxml` and
//! `russellhaering/goxmldsig` (canonicalisation). This module reproduces their observable
//! behaviour byte for byte, so that signatures produced by Chitach are identical to the ones
//! produced by Bas Celik:
//!
//! * the tree is "raw": namespace prefixes are kept as written (`space` / `tag`),
//! * line endings are normalised (`\r\n` and `\r` become `\n`), entities are decoded,
//!   CDATA sections become plain text,
//! * normal serialisation escapes `& < > ' "` and writes childless elements as `<a/>`,
//! * canonical serialisation follows goxmldsig's C14N 1.0 (inclusive, without comments).

use std::collections::{BTreeMap, HashMap};

use crate::{Result, SignError};

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attr {
    pub space: String,
    pub key: String,
    pub value: String,
}

impl Attr {
    pub fn full_key(&self) -> String {
        if self.space.is_empty() {
            self.key.clone()
        } else {
            format!("{}:{}", self.space, self.key)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
    Comment(String),
    ProcInst { target: String, inst: String },
    Directive(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Element {
    pub space: String,
    pub tag: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
}

/// Splits `prefix:local` at the first colon (etree `spaceDecompose`).
fn space_decompose(key: &str) -> (&str, &str) {
    match key.find(':') {
        Some(i) => (&key[..i], &key[i + 1..]),
        None => ("", key),
    }
}

impl Element {
    pub fn new(name: &str) -> Self {
        let (space, tag) = space_decompose(name);
        Element {
            space: space.to_string(),
            tag: tag.to_string(),
            ..Default::default()
        }
    }

    pub fn full_tag(&self) -> String {
        if self.space.is_empty() {
            self.tag.clone()
        } else {
            format!("{}:{}", self.space, self.tag)
        }
    }

    /// Creates an attribute or replaces the value of an existing one (etree `CreateAttr`).
    pub fn create_attr(&mut self, key: &str, value: &str) {
        let (space, key) = space_decompose(key);
        if let Some(a) = self
            .attrs
            .iter_mut()
            .find(|a| a.space == space && a.key == key)
        {
            a.value = value.to_string();
            return;
        }
        self.attrs.push(Attr {
            space: space.to_string(),
            key: key.to_string(),
            value: value.to_string(),
        });
    }

    pub fn attr_value(&self, key: &str) -> Option<&str> {
        let (space, key) = space_decompose(key);
        self.attrs
            .iter()
            .find(|a| a.space == space && a.key == key)
            .map(|a| a.value.as_str())
    }

    /// Appends a new child element and returns a mutable reference to it.
    pub fn create_element(&mut self, name: &str) -> &mut Element {
        self.children.push(Node::Element(Element::new(name)));
        match self.children.last_mut() {
            Some(Node::Element(e)) => e,
            _ => unreachable!(),
        }
    }

    /// Replaces the text content (all text children) of the element.
    pub fn set_text(&mut self, text: &str) {
        self.children.retain(|c| !matches!(c, Node::Text(_)));
        if !text.is_empty() {
            self.children.insert(0, Node::Text(text.to_string()));
        }
    }

    /// Concatenation of all direct text children.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|c| match c {
                Node::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn child_elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// First direct child element with the given local name (any prefix), like etree's
    /// `SelectElement` / `FindElement("./tag")`.
    pub fn child(&self, tag: &str) -> Option<&Element> {
        self.child_elements().find(|e| e.tag == tag)
    }

    pub fn child_mut(&mut self, tag: &str) -> Option<&mut Element> {
        self.children.iter_mut().find_map(|c| match c {
            Node::Element(e) if e.tag == tag => Some(e),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Document {
    pub children: Vec<Node>,
}

impl Document {
    pub fn root(&self) -> Option<&Element> {
        self.children.iter().find_map(|c| match c {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn root_mut(&mut self) -> Option<&mut Element> {
        self.children.iter_mut().find_map(|c| match c {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn parse(input: &str) -> Result<Document> {
        Parser::new(input).parse()
    }

    /// Serialises the document with etree's default settings.
    pub fn to_xml_string(&self) -> String {
        let mut out = String::new();
        for c in &self.children {
            write_node(&mut out, c, false);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum EscapeMode {
    Normal,
    CanonicalText,
    CanonicalAttr,
}

fn is_in_character_range(c: char) -> bool {
    let r = c as u32;
    r == 0x09
        || r == 0x0A
        || r == 0x0D
        || (0x20..=0xD7FF).contains(&r)
        || (0xE000..=0xFFFD).contains(&r)
        || (0x10000..=0x10FFFF).contains(&r)
}

/// Port of etree `escapeString`.
fn escape_into(out: &mut String, s: &str, mode: EscapeMode) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' if mode != EscapeMode::CanonicalAttr => out.push_str("&gt;"),
            '\'' if mode == EscapeMode::Normal => out.push_str("&apos;"),
            '"' if mode != EscapeMode::CanonicalText => out.push_str("&quot;"),
            '\t' if mode == EscapeMode::CanonicalAttr => out.push_str("&#x9;"),
            '\n' if mode == EscapeMode::CanonicalAttr => out.push_str("&#xA;"),
            '\r' if mode != EscapeMode::Normal => out.push_str("&#xD;"),
            c if !is_in_character_range(c) => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
}

fn write_node(out: &mut String, node: &Node, canonical: bool) {
    match node {
        Node::Element(e) => write_element(out, e, canonical),
        Node::Text(t) => escape_into(
            out,
            t,
            if canonical {
                EscapeMode::CanonicalText
            } else {
                EscapeMode::Normal
            },
        ),
        Node::Comment(c) => {
            out.push_str("<!--");
            out.push_str(c);
            out.push_str("-->");
        }
        Node::ProcInst { target, inst } => {
            out.push_str("<?");
            out.push_str(target);
            if !inst.is_empty() {
                out.push(' ');
                out.push_str(inst);
            }
            out.push_str("?>");
        }
        Node::Directive(d) => {
            out.push_str("<!");
            out.push_str(d);
            out.push('>');
        }
    }
}

fn write_element(out: &mut String, e: &Element, canonical: bool) {
    let tag = e.full_tag();
    out.push('<');
    out.push_str(&tag);
    for a in &e.attrs {
        out.push(' ');
        out.push_str(&a.full_key());
        out.push_str("=\"");
        escape_into(
            out,
            &a.value,
            if canonical {
                EscapeMode::CanonicalAttr
            } else {
                EscapeMode::Normal
            },
        );
        out.push('"');
    }
    if !e.children.is_empty() {
        out.push('>');
        for c in &e.children {
            write_node(out, c, canonical);
        }
        out.push_str("</");
        out.push_str(&tag);
        out.push('>');
    } else if canonical {
        out.push_str("></");
        out.push_str(&tag);
        out.push('>');
    } else {
        out.push_str("/>");
    }
}

// ---------------------------------------------------------------------------
// Canonicalisation (goxmldsig C14N 1.0, without comments)
// ---------------------------------------------------------------------------

const XMLNS: &str = "xmlns";

/// goxmldsig `SortedAttrs.Less`, evaluated on the current state of the slice.
fn attr_less(a: &[Attr], i: usize, j: usize) -> bool {
    let (ai, aj) = (&a[i], &a[j]);
    if aj.space.is_empty() && aj.key == XMLNS {
        return false;
    }
    if ai.space.is_empty() && ai.key == XMLNS {
        return true;
    }
    if ai.space == XMLNS {
        if aj.space == XMLNS {
            return ai.key < aj.key;
        }
        return true;
    }
    if aj.space == XMLNS {
        return false;
    }
    if ai.space.is_empty() {
        if aj.space.is_empty() {
            return ai.key < aj.key;
        }
        return true;
    }
    if aj.space.is_empty() {
        return false;
    }
    if ai.space == aj.space {
        return ai.key < aj.key;
    }
    if ai.key == aj.key {
        let mut left = "";
        let mut right = "";
        for n in a {
            if ai.space == n.key {
                left = &n.value;
            }
            if aj.space == n.key {
                right = &n.value;
            }
        }
        return left < right;
    }
    ai.key < aj.key
}

/// Go's `sort.Sort` uses insertion sort for short slices; we use it for all lengths.
fn sort_attrs(a: &mut [Attr]) {
    for i in 1..a.len() {
        let mut j = i;
        while j > 0 && attr_less(a, j, j - 1) {
            a.swap(j, j - 1);
            j -= 1;
        }
    }
}

/// goxmldsig `canonicalPrepInner` (strip redundant namespace declarations, sort attributes,
/// remove comments).
fn canonical_prep(el: &Element, seen_so_far: &HashMap<String, String>) -> Element {
    let mut seen = seen_so_far.clone();
    let mut attrs = el.attrs.clone();
    sort_attrs(&mut attrs);

    let mut kept = Vec::with_capacity(attrs.len());
    for attr in attrs {
        if attr.space != XMLNS && !(attr.space.is_empty() && attr.key == XMLNS) {
            kept.push(attr);
            continue;
        }
        if attr.space == XMLNS {
            let key = format!("{}:{}", attr.space, attr.key);
            let keep = seen.get(&key).is_none_or(|uri| *uri != attr.value);
            if keep {
                seen.insert(key, attr.value.clone());
                kept.push(attr);
            }
        } else {
            let uri = seen.get(XMLNS).map(String::as_str).unwrap_or("");
            if attr.value != uri {
                seen.insert(XMLNS.to_string(), attr.value.clone());
                kept.push(attr);
            }
        }
    }

    let children = el
        .children
        .iter()
        .filter(|c| !matches!(c, Node::Comment(_)))
        .map(|c| match c {
            Node::Element(child) => Node::Element(canonical_prep(child, &seen)),
            other => other.clone(),
        })
        .collect();

    Element {
        space: el.space.clone(),
        tag: el.tag.clone(),
        attrs: kept,
        children,
    }
}

/// Canonical form (C14N 1.0, inclusive, without comments) of `el`, whose ancestors (from the
/// document root down to the direct parent) are `ancestors`.
pub(crate) fn canonicalize(el: &Element, ancestors: &[&Element]) -> String {
    let mut namespaces: BTreeMap<String, String> = BTreeMap::new();
    let mut xml_attrs: BTreeMap<String, String> = BTreeMap::new();
    for anc in ancestors {
        for attr in &anc.attrs {
            if attr.space == XMLNS && (attr.key != "xml" || attr.value != XML_NAMESPACE) {
                namespaces.insert(attr.key.clone(), attr.value.clone());
            } else if attr.space.is_empty() && attr.key == XMLNS {
                namespaces.insert(attr.key.clone(), attr.value.clone());
            } else if attr.space == "xml" {
                xml_attrs.insert(attr.key.clone(), attr.value.clone());
            }
        }
    }

    let mut copy = el.clone();
    for (prefix, uri) in &namespaces {
        if prefix == XMLNS {
            copy.create_attr(XMLNS, uri);
        } else {
            copy.create_attr(&format!("xmlns:{prefix}"), uri);
        }
    }
    for (key, value) in &xml_attrs {
        copy.create_attr(&format!("xml:{key}"), value);
    }

    let prepped = canonical_prep(&copy, &HashMap::new());
    let mut out = String::new();
    write_element(&mut out, &prepped, true);
    out
}

// ---------------------------------------------------------------------------
// Parsing (behaviour of Go's encoding/xml in strict mode + etree)
// ---------------------------------------------------------------------------

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

fn xml_err(msg: impl Into<String>) -> SignError {
    SignError::Xml(msg.into())
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | ':' | '.' | '-') || (c as u32) >= 0x80
}

impl Parser {
    fn new(input: &str) -> Self {
        let input = input.strip_prefix('\u{FEFF}').unwrap_or(input);
        let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
        Parser {
            chars: normalized.chars().collect(),
            pos: 0,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn starts_with(&self, s: &str) -> bool {
        let mut i = self.pos;
        for c in s.chars() {
            if self.chars.get(i) != Some(&c) {
                return false;
            }
            i += 1;
        }
        true
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    /// Reads up to (not including) `end`, consuming `end` as well.
    fn read_until(&mut self, end: &str, what: &str) -> Result<String> {
        let start = self.pos;
        while !self.eof() {
            if self.starts_with(end) {
                let s: String = self.chars[start..self.pos].iter().collect();
                self.pos += end.chars().count();
                return Ok(s);
            }
            self.pos += 1;
        }
        Err(xml_err(format!("unexpected EOF in {what}")))
    }

    fn name(&mut self) -> Result<String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if is_name_char(c) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(xml_err(format!("expected name at offset {start}")));
        }
        Ok(self.chars[start..self.pos].iter().collect())
    }

    /// Go `nsname`: returns (space, local).
    fn ns_name(&mut self) -> Result<(String, String)> {
        let s = self.name()?;
        if s.matches(':').count() > 1 {
            return Err(xml_err(format!("invalid name {s}")));
        }
        match s.split_once(':') {
            Some((space, local)) if !space.is_empty() && !local.is_empty() => {
                Ok((space.to_string(), local.to_string()))
            }
            _ => Ok((String::new(), s)),
        }
    }

    /// Parses an entity reference; `self.pos` is just after `&`.
    fn entity(&mut self, out: &mut String) -> Result<()> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == ';' {
                break;
            }
            if !(c.is_alphanumeric() || c == '#') {
                return Err(xml_err("invalid character entity"));
            }
            self.pos += 1;
        }
        if self.peek() != Some(';') {
            return Err(xml_err("invalid character entity (no semicolon)"));
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        self.pos += 1;
        let c = match name.as_str() {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "apos" => '\'',
            "quot" => '"',
            n if n.starts_with("#x") => u32::from_str_radix(&n[2..], 16)
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| xml_err(format!("invalid character entity &{n};")))?,
            n if n.starts_with('#') => n[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .ok_or_else(|| xml_err(format!("invalid character entity &{n};")))?,
            n => return Err(xml_err(format!("invalid character entity &{n};"))),
        };
        out.push(c);
        Ok(())
    }

    fn text(&mut self) -> Result<String> {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            match c {
                '<' => break,
                '&' => {
                    self.pos += 1;
                    self.entity(&mut out)?;
                }
                ']' if self.starts_with("]]>") => {
                    return Err(xml_err("unescaped ]]> not in CDATA section"));
                }
                c => {
                    out.push(c);
                    self.pos += 1;
                }
            }
        }
        Ok(out)
    }

    fn attr_value(&mut self) -> Result<String> {
        let quote = match self.peek() {
            Some(q @ ('"' | '\'')) => q,
            _ => return Err(xml_err("unquoted or missing attribute value in element")),
        };
        self.pos += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err(xml_err("unexpected EOF in attribute value")),
                Some(c) if c == quote => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some('<') => return Err(xml_err("unescaped < inside quoted string")),
                Some('&') => {
                    self.pos += 1;
                    self.entity(&mut out)?;
                }
                Some(c) => {
                    out.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    fn directive(&mut self) -> Result<String> {
        // self.pos is just after "<!"
        let start = self.pos;
        let mut depth = 0usize;
        while let Some(c) = self.peek() {
            match c {
                '"' | '\'' => {
                    self.pos += 1;
                    while let Some(d) = self.peek() {
                        self.pos += 1;
                        if d == c {
                            break;
                        }
                    }
                    continue;
                }
                '<' => depth += 1,
                '>' => {
                    if depth == 0 {
                        let s: String = self.chars[start..self.pos].iter().collect();
                        self.pos += 1;
                        return Ok(s);
                    }
                    depth -= 1;
                }
                _ => {}
            }
            self.pos += 1;
        }
        Err(xml_err("unexpected EOF in directive"))
    }

    fn parse(mut self) -> Result<Document> {
        let mut doc = Document::default();
        let mut stack: Vec<Element> = Vec::new();

        fn push(stack: &mut [Element], doc: &mut Document, node: Node) {
            match stack.last_mut() {
                Some(top) => top.children.push(node),
                None => doc.children.push(node),
            }
        }

        while !self.eof() {
            if self.starts_with("<?") {
                self.pos += 2;
                let target = self.name()?;
                self.skip_ws();
                let inst = self.read_until("?>", "processing instruction")?;
                push(&mut stack, &mut doc, Node::ProcInst { target, inst });
            } else if self.starts_with("<!--") {
                self.pos += 4;
                let comment = self.read_until("-->", "comment")?;
                if comment.contains("--") || comment.ends_with('-') {
                    return Err(xml_err(r#"invalid sequence "--" not allowed in comments"#));
                }
                push(&mut stack, &mut doc, Node::Comment(comment));
            } else if self.starts_with("<![CDATA[") {
                self.pos += 9;
                let data = self.read_until("]]>", "CDATA section")?;
                push(&mut stack, &mut doc, Node::Text(data));
            } else if self.starts_with("<!") {
                self.pos += 2;
                let d = self.directive()?;
                push(&mut stack, &mut doc, Node::Directive(d));
            } else if self.starts_with("</") {
                self.pos += 2;
                let (space, tag) = self.ns_name()?;
                self.skip_ws();
                if self.peek() != Some('>') {
                    return Err(xml_err(format!("invalid characters between </{tag} and >")));
                }
                self.pos += 1;
                let el = stack
                    .pop()
                    .ok_or_else(|| xml_err(format!("unexpected end element </{tag}>")))?;
                if el.space != space || el.tag != tag {
                    return Err(xml_err(format!(
                        "element <{}> closed by </{}>",
                        el.full_tag(),
                        if space.is_empty() {
                            tag
                        } else {
                            format!("{space}:{tag}")
                        }
                    )));
                }
                push(&mut stack, &mut doc, Node::Element(el));
            } else if self.starts_with("<") {
                self.pos += 1;
                let (space, tag) = self.ns_name()?;
                let mut el = Element {
                    space,
                    tag,
                    ..Default::default()
                };
                loop {
                    self.skip_ws();
                    match self.peek() {
                        None => return Err(xml_err("unexpected EOF in element")),
                        Some('/') => {
                            self.pos += 1;
                            if self.peek() != Some('>') {
                                return Err(xml_err("expected /> in element"));
                            }
                            self.pos += 1;
                            push(&mut stack, &mut doc, Node::Element(el));
                            break;
                        }
                        Some('>') => {
                            self.pos += 1;
                            stack.push(el);
                            break;
                        }
                        Some(_) => {
                            let (aspace, akey) = self.ns_name()?;
                            self.skip_ws();
                            if self.peek() != Some('=') {
                                return Err(xml_err("attribute name without = in element"));
                            }
                            self.pos += 1;
                            self.skip_ws();
                            let value = self.attr_value()?;
                            // etree: duplicate attributes keep the first position, last value
                            if let Some(a) = el
                                .attrs
                                .iter_mut()
                                .find(|a| a.space == aspace && a.key == akey)
                            {
                                a.value = value;
                            } else {
                                el.attrs.push(Attr {
                                    space: aspace,
                                    key: akey,
                                    value,
                                });
                            }
                        }
                    }
                }
            } else {
                let t = self.text()?;
                push(&mut stack, &mut doc, Node::Text(t));
            }
        }

        if let Some(open) = stack.last() {
            return Err(xml_err(format!(
                "unexpected EOF: element <{}> not closed",
                open.full_tag()
            )));
        }
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(s: &str) -> String {
        Document::parse(s).unwrap().to_xml_string()
    }

    fn c14n(s: &str) -> String {
        let doc = Document::parse(s).unwrap();
        canonicalize(doc.root().unwrap(), &[])
    }

    #[test]
    fn serializes_like_etree() {
        assert_eq!(
            roundtrip("<?xml version=\"1.0\"?>\n<a x='1&amp;2'><b></b>t&lt;\"'&#65;<![CDATA[<x>]]></a>\n"),
            "<?xml version=\"1.0\"?>\n<a x=\"1&amp;2\"><b/>t&lt;&quot;&apos;A&lt;x&gt;</a>\n"
        );
        assert_eq!(roundtrip("<a>\r\nb\rc</a>"), "<a>\nb\nc</a>");
        assert_eq!(
            roundtrip("<!DOCTYPE a [<!ENTITY x \"y\">]><a/>"),
            "<!DOCTYPE a [<!ENTITY x \"y\">]><a/>"
        );
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "<a>",
            "<a></b>",
            "<a x=1/>",
            "<a>&foo;</a>",
            "<a x=\"<\"/>",
            "<a>]]></a>",
            "</a>",
            "<a><!-- a -- b --></a>",
        ] {
            assert!(Document::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn canonical_form() {
        assert_eq!(
            c14n("<a z=\"1\" b:y=\"2\" xmlns:b=\"urn:b\" a=\"x&#9;\" xmlns=\"urn:d\"><!--c--><b:c xmlns:b=\"urn:b\" xmlns=\"urn:d\"/>t&gt;&#13;</a>"),
            "<a xmlns=\"urn:d\" xmlns:b=\"urn:b\" a=\"x&#x9;\" z=\"1\" b:y=\"2\"><b:c></b:c>t&gt;&#xD;</a>"
        );
        assert_eq!(c14n("<a xmlns=\"\"><b xmlns=\"\"/></a>"), "<a><b></b></a>");
    }

    #[test]
    fn canonical_form_with_ancestors() {
        let doc = Document::parse(
            "<r xmlns=\"urn:r\" xmlns:p=\"urn:p\" xml:lang=\"sr\" a=\"1\"><s xmlns=\"urn:s\"><i>x</i></s></r>",
        )
        .unwrap();
        let r = doc.root().unwrap();
        let s = r.child("s").unwrap();
        let i = s.child("i").unwrap();
        assert_eq!(
            canonicalize(i, &[r, s]),
            "<i xmlns=\"urn:s\" xmlns:p=\"urn:p\" xml:lang=\"sr\">x</i>"
        );
    }
}
