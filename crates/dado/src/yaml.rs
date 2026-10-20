//! Strict YAML-1.2-core parsing.
//!
//! Scope is deliberately narrow. Supported: block mappings and sequences,
//! single-line flow collections, plain scalars, single- and double-quoted
//! scalars, block scalars (`|`, `>` with chomping), comments, and multi-line
//! plain scalars.
//!
//! Rejected with a positioned error: tags, directives, anchors, aliases,
//! merge keys, multi-document streams, complex keys, duplicate keys, tab
//! indentation, and unterminated quotes or flow collections.
//!
//! Booleans follow YAML 1.2 core: only `true` and `false`. `yes`, `no`, `on`,
//! and `off` are strings. Integers may be decimal, hexadecimal, octal, or
//! binary. Floats must be finite; `.inf` and `.nan` are rejected.
//!
//! Nesting is limited to 128 levels so untrusted input cannot overflow the
//! stack. Every error carries a stable code, a 1-based line and column, and a
//! 0-based byte offset. Error messages never echo scalar values.

use indexmap::IndexMap;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use serde::Deserializer;

/// Maximum nesting depth for block and flow collections.
const MAX_DEPTH: usize = 128;

/// A parse or deserialization failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Stable diagnostic code, e.g. `D1001`.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// 1-based line where the failure occurred.
    pub line: usize,
    /// 1-based column where the failure occurred.
    pub column: usize,
    /// 0-based byte offset where the failure occurred.
    pub offset: usize,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}: {} ({}:{}, offset {})",
            self.code, self.message, self.line, self.column, self.offset
        )
    }
}

impl core::error::Error for Error {}

impl de::Error for Error {
    fn custom<T: core::fmt::Display>(msg: T) -> Self {
        let raw = msg.to_string();
        let message = if raw.starts_with("invalid type:") {
            "value has the wrong type".to_owned()
        } else if raw.starts_with("invalid value:") {
            "value has an invalid format".to_owned()
        } else if raw.starts_with("unknown variant") {
            "unknown enum variant".to_owned()
        } else {
            raw
        };
        Self {
            code: "D1010",
            message,
            line: 1,
            column: 1,
            offset: 0,
        }
    }
}

/// Parse a YAML document into `T`.
///
/// The parser supports the strict subset documented on this module. It never
/// performs I/O and never guesses at unsupported constructs.
pub fn from_str<T>(src: &str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let parser = Parser::new(src);
    let node = parser.parse_document()?;
    T::deserialize(node)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Position {
    line: usize,
    column: usize,
    offset: usize,
}

impl Position {
    const fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }

    fn advance(self, text: &str, bytes: usize) -> Self {
        let bytes = bytes.min(text.len());
        Self {
            line: self.line,
            column: self.column + text[..bytes].chars().count(),
            offset: self.offset + bytes,
        }
    }

    fn error(self, code: &'static str, message: impl Into<String>) -> Error {
        Error {
            code,
            message: message.into(),
            line: self.line,
            column: self.column,
            offset: self.offset,
        }
    }
}

#[derive(Debug)]
struct Node {
    value: YamlValue,
    pos: Position,
}

#[derive(Debug)]
enum YamlValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Sequence(Vec<Node>),
    Mapping(IndexMap<String, Node>),
}

impl Node {
    fn null(pos: Position) -> Self {
        Self {
            value: YamlValue::Null,
            pos,
        }
    }

    fn boolean(value: bool, pos: Position) -> Self {
        Self {
            value: YamlValue::Bool(value),
            pos,
        }
    }

    fn integer(value: i64, pos: Position) -> Self {
        Self {
            value: YamlValue::Integer(value),
            pos,
        }
    }

    fn float(value: f64, pos: Position) -> Self {
        Self {
            value: YamlValue::Float(value),
            pos,
        }
    }

    fn string(value: String, pos: Position) -> Self {
        Self {
            value: YamlValue::String(value),
            pos,
        }
    }
}

struct Line<'a> {
    start: usize,
    text: &'a str,
    number: usize,
}

struct Parser<'a> {
    lines: Vec<Line<'a>>,
}

struct PendingItem {
    text: String,
    pos: Position,
    content_indent: usize,
    index: usize,
}

struct PendingEntry {
    key: String,
    key_pos: Position,
    value_text: String,
    value_pos: Position,
    entry_indent: usize,
    index: usize,
}

#[derive(Clone, Copy)]
enum Chomp {
    Clip,
    Strip,
    Keep,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        let (body, base) = match src.strip_prefix('\u{feff}') {
            Some(rest) => (rest, 3),
            None => (src, 0),
        };
        Self {
            lines: split_lines(body, base),
        }
    }

    fn parse_document(&self) -> Result<Node, Error> {
        let first = match self.next_content(0)? {
            Some(index) => index,
            None => return Ok(Node::null(Position::new(1, 1, 0))),
        };

        let indent = self.indent(first)?;
        if indent != 0 {
            return Err(self
                .pos_at(first, indent)
                .error("D1007", "unexpected indentation"));
        }

        let text = self.content(first, indent);
        if is_document_marker(text) {
            return Err(self
                .pos_at(first, indent)
                .error("D1003", "multi-document streams are not supported"));
        }
        if text.starts_with('%') {
            return Err(self
                .pos_at(first, indent)
                .error("D1003", "directives are not supported"));
        }

        let (node, next) = self.parse_block(first, 0, 1)?;
        if let Some(trailing) = self.next_content(next)? {
            let actual = self.indent(trailing)?;
            let text = self.content(trailing, actual);
            if actual == 0 && is_document_marker(text) {
                return Err(self
                    .pos_at(trailing, 0)
                    .error("D1003", "multi-document streams are not supported"));
            }
            return Err(self
                .pos_at(trailing, 0)
                .error("D1008", "trailing content after the document"));
        }
        Ok(node)
    }

    fn next_content(&self, mut index: usize) -> Result<Option<usize>, Error> {
        while index < self.lines.len() {
            let indent = self.indent(index)?;
            if !self.content(index, indent).is_empty() {
                return Ok(Some(index));
            }
            index += 1;
        }
        Ok(None)
    }

    fn indent(&self, index: usize) -> Result<usize, Error> {
        let line = &self.lines[index];
        let mut count = 0;
        for (byte, ch) in line.text.char_indices() {
            match ch {
                ' ' => count += 1,
                '\t' => {
                    return Err(self
                        .pos_at(index, byte)
                        .error("D1002", "tab indentation is not supported"));
                }
                _ => break,
            }
        }
        Ok(count)
    }

    fn content(&self, index: usize, indent: usize) -> &str {
        let raw = &self.lines[index].text[indent..];
        strip_comment(raw).trim_end()
    }

    fn raw_indent(&self, index: usize) -> usize {
        self.lines[index]
            .text
            .chars()
            .take_while(|ch| *ch == ' ')
            .count()
    }

    fn pos_at(&self, index: usize, byte: usize) -> Position {
        let line = &self.lines[index];
        let byte = byte.min(line.text.len());
        Position::new(
            line.number,
            line.text[..byte].chars().count() + 1,
            line.start + byte,
        )
    }

    fn parse_block(
        &self,
        index: usize,
        indent: usize,
        depth: usize,
    ) -> Result<(Node, usize), Error> {
        if depth > MAX_DEPTH {
            return Err(self
                .pos_at(index.min(self.lines.len().saturating_sub(1)), 0)
                .error("D1006", "nesting depth limit of 128 exceeded"));
        }

        let current = match self.next_content(index)? {
            Some(current) => current,
            None => return Ok((Node::null(Position::new(1, 1, 0)), index)),
        };
        let actual = self.indent(current)?;
        if actual < indent {
            return Ok((Node::null(self.pos_at(current, actual)), current));
        }
        if actual > indent {
            return Err(self
                .pos_at(current, actual)
                .error("D1007", "unexpected indentation"));
        }

        let text = self.content(current, actual);
        if actual == 0 && is_document_marker(text) {
            return Err(self
                .pos_at(current, actual)
                .error("D1003", "multi-document streams are not supported"));
        }
        if is_sequence_line(text) {
            self.parse_sequence(current, actual, depth, None)
        } else if find_mapping_colon(text).is_some() {
            self.parse_mapping(current, actual, depth, None)
        } else {
            self.parse_inline_value(
                text,
                self.pos_at(current, actual),
                actual,
                current + 1,
                depth.saturating_sub(1),
            )
        }
    }

    fn parse_sequence(
        &self,
        index: usize,
        indent: usize,
        depth: usize,
        first: Option<PendingItem>,
    ) -> Result<(Node, usize), Error> {
        if depth > MAX_DEPTH {
            return Err(self
                .pos_at(index.min(self.lines.len().saturating_sub(1)), 0)
                .error("D1006", "nesting depth limit of 128 exceeded"));
        }

        let start_pos = first
            .as_ref()
            .map_or_else(|| self.pos_at(index, indent), |item| item.pos);
        let mut items = Vec::new();
        let mut current = index;
        let mut first = first;

        loop {
            let item = match first.take() {
                Some(item) => item,
                None => {
                    let line_index = match self.next_content(current)? {
                        Some(line_index) => line_index,
                        None => break,
                    };
                    let actual = self.indent(line_index)?;
                    if actual < indent {
                        break;
                    }
                    if actual > indent {
                        return Err(self
                            .pos_at(line_index, actual)
                            .error("D1007", "unexpected indentation"));
                    }
                    let text = self.content(line_index, actual);
                    if actual == 0 && is_document_marker(text) {
                        return Err(self
                            .pos_at(line_index, actual)
                            .error("D1003", "multi-document streams are not supported"));
                    }
                    if !is_sequence_line(text) {
                        break;
                    }
                    let after = &text[1..];
                    let leading = after.len() - after.trim_start_matches(' ').len();
                    let remainder = after[leading..].trim_end();
                    let content_indent = actual + 1 + leading;
                    PendingItem {
                        text: remainder.to_owned(),
                        pos: self.pos_at(line_index, content_indent),
                        content_indent,
                        index: line_index + 1,
                    }
                }
            };

            let (value, next) = self.parse_item(&item, indent, depth)?;
            items.push(value);
            current = next;
        }

        Ok((
            Node {
                value: YamlValue::Sequence(items),
                pos: start_pos,
            },
            current,
        ))
    }

    fn parse_item(
        &self,
        item: &PendingItem,
        sequence_indent: usize,
        depth: usize,
    ) -> Result<(Node, usize), Error> {
        let text = item.text.trim_end();
        if text.is_empty() {
            let line_index = match self.next_content(item.index)? {
                Some(line_index) => line_index,
                None => return Ok((Node::null(item.pos), item.index)),
            };
            let actual = self.indent(line_index)?;
            if actual > sequence_indent {
                self.parse_block(line_index, actual, depth + 1)
            } else {
                Ok((Node::null(item.pos), item.index))
            }
        } else if text.starts_with('|') || text.starts_with('>') {
            self.parse_block_scalar(text, sequence_indent, item.index, item.pos)
        } else if is_sequence_line(text) {
            let after = &text[1..];
            let leading = after.len() - after.trim_start_matches(' ').len();
            let remainder = after[leading..].trim_end();
            let content_indent = item.content_indent + 1 + leading;
            let nested = PendingItem {
                text: remainder.to_owned(),
                pos: item.pos.advance(text, 1 + leading),
                content_indent,
                index: item.index,
            };
            self.parse_sequence(item.index, item.content_indent, depth + 1, Some(nested))
        } else if find_mapping_colon(text).is_some() {
            let entry =
                self.parse_entry_from_text(text, item.pos, item.content_indent, item.index)?;
            self.parse_mapping(item.index, item.content_indent, depth + 1, Some(entry))
        } else {
            self.parse_inline_value(text, item.pos, sequence_indent, item.index, depth)
        }
    }

    fn parse_mapping(
        &self,
        index: usize,
        indent: usize,
        depth: usize,
        first: Option<PendingEntry>,
    ) -> Result<(Node, usize), Error> {
        if depth > MAX_DEPTH {
            return Err(self
                .pos_at(index.min(self.lines.len().saturating_sub(1)), 0)
                .error("D1006", "nesting depth limit of 128 exceeded"));
        }

        let start_pos = first
            .as_ref()
            .map_or_else(|| self.pos_at(index, indent), |entry| entry.key_pos);
        let mut mapping = IndexMap::new();
        let mut current = index;
        let mut first = first;

        loop {
            let entry = match first.take() {
                Some(entry) => entry,
                None => {
                    let line_index = match self.next_content(current)? {
                        Some(line_index) => line_index,
                        None => break,
                    };
                    let actual = self.indent(line_index)?;
                    if actual < indent {
                        break;
                    }
                    if actual > indent {
                        return Err(self
                            .pos_at(line_index, actual)
                            .error("D1007", "unexpected indentation"));
                    }
                    let text = self.content(line_index, actual);
                    if actual == 0 && is_document_marker(text) {
                        return Err(self
                            .pos_at(line_index, actual)
                            .error("D1003", "multi-document streams are not supported"));
                    }
                    if is_sequence_line(text) {
                        break;
                    }
                    self.parse_entry_from_text(
                        text,
                        self.pos_at(line_index, actual),
                        actual,
                        line_index + 1,
                    )?
                }
            };

            if mapping.contains_key(&entry.key) {
                return Err(entry.key_pos.error("D1001", "duplicate mapping key"));
            }

            let (value, next) = self.parse_entry_value(&entry, depth)?;
            mapping.insert(entry.key, value);
            current = next;
        }

        Ok((
            Node {
                value: YamlValue::Mapping(mapping),
                pos: start_pos,
            },
            current,
        ))
    }

    fn parse_entry_from_text(
        &self,
        text: &str,
        start: Position,
        indent: usize,
        next_index: usize,
    ) -> Result<PendingEntry, Error> {
        let colon = find_mapping_colon(text)
            .ok_or_else(|| start.error("D1012", "expected a mapping entry"))?;
        let key_text = text[..colon].trim_end();
        let key = parse_key_text(key_text, start)?;
        let after = &text[colon + 1..];
        let leading = after.len() - after.trim_start().len();
        Ok(PendingEntry {
            key,
            key_pos: start,
            value_text: after.trim().to_owned(),
            value_pos: start.advance(text, colon + 1 + leading),
            entry_indent: indent,
            index: next_index,
        })
    }

    fn parse_entry_value(
        &self,
        entry: &PendingEntry,
        depth: usize,
    ) -> Result<(Node, usize), Error> {
        if entry.value_text.is_empty() {
            let line_index = match self.next_content(entry.index)? {
                Some(line_index) => line_index,
                None => return Ok((Node::null(entry.value_pos), entry.index)),
            };
            let actual = self.indent(line_index)?;
            if actual > entry.entry_indent {
                self.parse_block(line_index, actual, depth + 1)
            } else {
                Ok((Node::null(entry.value_pos), entry.index))
            }
        } else {
            self.parse_inline_value(
                &entry.value_text,
                entry.value_pos,
                entry.entry_indent,
                entry.index,
                depth,
            )
        }
    }

    fn parse_inline_value(
        &self,
        text: &str,
        pos: Position,
        parent_indent: usize,
        next_index: usize,
        depth: usize,
    ) -> Result<(Node, usize), Error> {
        if depth > MAX_DEPTH {
            return Err(pos.error("D1006", "nesting depth limit of 128 exceeded"));
        }

        let text = text.trim();
        if text.starts_with('|') || text.starts_with('>') {
            return self.parse_block_scalar(text, parent_indent, next_index, pos);
        }
        if text.starts_with('[') || text.starts_with('{') {
            let mut parser = FlowParser::new(text, pos, depth + 1);
            let node = parser.parse_document_value()?;
            return Ok((node, next_index));
        }
        if text.starts_with('"') || text.starts_with('\'') {
            let (value, used) = parse_quoted_at(text, 0, pos)?;
            if !text[used..].trim().is_empty() {
                return Err(pos.error("D1004", "unexpected content after quoted scalar"));
            }
            return Ok((Node::string(value, pos), next_index));
        }
        if text.starts_with('&') {
            return Err(pos.error("D1003", "anchors are not supported"));
        }
        if text.starts_with('*') {
            return Err(pos.error("D1003", "aliases are not supported"));
        }
        if text.starts_with('!') {
            return Err(pos.error("D1003", "tags are not supported"));
        }
        if text.starts_with('%') {
            return Err(pos.error("D1003", "directives are not supported"));
        }
        if text.starts_with('@') || text.starts_with('`') {
            return Err(pos.error("D1003", "reserved indicators are not supported"));
        }
        if text == "?" || text.starts_with("? ") {
            return Err(pos.error("D1003", "complex keys are not supported"));
        }

        self.parse_plain_value(text, pos, parent_indent, next_index)
    }

    fn parse_plain_value(
        &self,
        text: &str,
        pos: Position,
        parent_indent: usize,
        mut index: usize,
    ) -> Result<(Node, usize), Error> {
        let mut parts = vec![text.trim().to_owned()];
        while let Some(line_index) = self.next_content(index)? {
            let actual = self.indent(line_index)?;
            if actual <= parent_indent {
                break;
            }
            let continuation = self.content(line_index, actual);
            if is_sequence_line(continuation) || find_mapping_colon(continuation).is_some() {
                break;
            }
            parts.push(continuation.trim().to_owned());
            index = line_index + 1;
        }
        let joined = parts.join(" ");
        let node = resolve_scalar(&joined, pos)?;
        Ok((node, index))
    }

    fn parse_block_scalar(
        &self,
        indicator: &str,
        parent_indent: usize,
        start_index: usize,
        pos: Position,
    ) -> Result<(Node, usize), Error> {
        let indicator = indicator.trim();
        let style = indicator.as_bytes().first().copied().unwrap_or(b'|');
        if style != b'|' && style != b'>' {
            return Err(pos.error("D1004", "invalid block scalar indicator"));
        }
        let suffix = indicator.get(1..).unwrap_or("").trim();
        let chomp = match suffix {
            "" => Chomp::Clip,
            "-" => Chomp::Strip,
            "+" => Chomp::Keep,
            _ => return Err(pos.error("D1004", "unsupported block scalar indicator")),
        };

        let mut raw = Vec::new();
        let mut content_indent = None;
        let mut index = start_index;
        while index < self.lines.len() {
            let line = &self.lines[index];
            if line.text.trim().is_empty() {
                raw.push(String::new());
                index += 1;
                continue;
            }
            let actual = self.raw_indent(index);
            if actual <= parent_indent {
                break;
            }
            let required = *content_indent.get_or_insert(actual);
            if actual < required {
                return Err(self
                    .pos_at(index, actual)
                    .error("D1007", "inconsistent block scalar indentation"));
            }
            raw.push(line.text[required..].to_owned());
            index += 1;
        }

        if content_indent.is_none() {
            raw.clear();
        }

        let body = if style == b'|' {
            literal_block(&raw)
        } else {
            folded_block(&raw)
        };
        let text = apply_chomp(body, chomp);
        Ok((Node::string(text, pos), index))
    }
}

fn split_lines(src: &str, base: usize) -> Vec<Line<'_>> {
    let bytes = src.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut number = 1;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                lines.push(Line {
                    start: base + start,
                    text: &src[start..index],
                    number,
                });
                index += 1;
                start = index;
                number += 1;
            }
            b'\r' => {
                lines.push(Line {
                    start: base + start,
                    text: &src[start..index],
                    number,
                });
                index += 1;
                if index < bytes.len() && bytes[index] == b'\n' {
                    index += 1;
                }
                start = index;
                number += 1;
            }
            _ => index += 1,
        }
    }

    lines.push(Line {
        start: base + start,
        text: &src[start..],
        number,
    });
    lines
}

fn literal_block(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }
}

fn folded_block(lines: &[String]) -> String {
    let mut out = String::new();
    let mut previous_was_text = false;
    for line in lines {
        if line.is_empty() {
            out.push('\n');
            previous_was_text = false;
        } else {
            if previous_was_text {
                out.push(' ');
            }
            out.push_str(line);
            previous_was_text = true;
        }
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn apply_chomp(mut text: String, chomp: Chomp) -> String {
    match chomp {
        Chomp::Strip => {
            while text.ends_with('\n') {
                text.pop();
            }
            text
        }
        Chomp::Clip => {
            while text.ends_with('\n') {
                text.pop();
            }
            if !text.is_empty() {
                text.push('\n');
            }
            text
        }
        Chomp::Keep => text,
    }
}

fn parse_key_text(text: &str, pos: Position) -> Result<String, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(pos.error("D1012", "mapping key is empty"));
    }
    if text == "?" || text.starts_with("? ") {
        return Err(pos.error("D1003", "complex keys are not supported"));
    }
    if text.starts_with('&') {
        return Err(pos.error("D1003", "anchors are not supported"));
    }
    if text.starts_with('*') {
        return Err(pos.error("D1003", "aliases are not supported"));
    }
    if text.starts_with('!') {
        return Err(pos.error("D1003", "tags are not supported"));
    }
    if text.starts_with('%') {
        return Err(pos.error("D1003", "directives are not supported"));
    }
    if text.starts_with('[') || text.starts_with('{') {
        return Err(pos.error("D1003", "complex keys are not supported"));
    }

    let key = if text.starts_with('"') || text.starts_with('\'') {
        let (value, used) = parse_quoted_at(text, 0, pos)?;
        if !text[used..].trim().is_empty() {
            return Err(pos.error("D1004", "unexpected content after quoted key"));
        }
        value
    } else {
        text.to_owned()
    };

    if key == "<<" {
        return Err(pos.error("D1003", "merge keys are not supported"));
    }
    Ok(key)
}

fn parse_quoted_at(text: &str, start: usize, pos: Position) -> Result<(String, usize), Error> {
    let bytes = text.as_bytes();
    let quote = *bytes
        .get(start)
        .ok_or_else(|| pos.error("D1004", "expected a quoted scalar"))?;
    let mut out = String::new();
    let mut index = start + 1;

    if quote == b'\'' {
        while index < bytes.len() {
            if bytes[index] == b'\'' {
                if bytes.get(index + 1) == Some(&b'\'') {
                    out.push('\'');
                    index += 2;
                } else {
                    return Ok((out, index + 1));
                }
            } else {
                let ch = text[index..]
                    .chars()
                    .next()
                    .ok_or_else(|| pos.error("D1004", "unterminated single-quoted scalar"))?;
                out.push(ch);
                index += ch.len_utf8();
            }
        }
        return Err(pos.error("D1004", "unterminated single-quoted scalar"));
    }

    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Ok((out, index + 1)),
            b'\\' => {
                index += 1;
                if index >= bytes.len() {
                    return Err(pos.error("D1004", "unterminated escape sequence"));
                }
                match bytes[index] {
                    b'0' => out.push('\0'),
                    b'a' => out.push('\u{0007}'),
                    b'b' => out.push('\u{0008}'),
                    b't' => out.push('\t'),
                    b'n' => out.push('\n'),
                    b'v' => out.push('\u{000b}'),
                    b'f' => out.push('\u{000c}'),
                    b'r' => out.push('\r'),
                    b'e' => out.push('\u{001b}'),
                    b' ' => out.push(' '),
                    b'"' => out.push('"'),
                    b'/' => out.push('/'),
                    b'\\' => out.push('\\'),
                    b'N' => out.push('\u{0085}'),
                    b'_' => out.push('\u{00a0}'),
                    b'L' => out.push('\u{2028}'),
                    b'P' => out.push('\u{2029}'),
                    b'x' => {
                        let value = parse_hex_escape(text, index + 1, 2, pos)?;
                        out.push(value);
                        index += 2;
                    }
                    b'u' => {
                        let value = parse_hex_escape(text, index + 1, 4, pos)?;
                        out.push(value);
                        index += 4;
                    }
                    b'U' => {
                        let value = parse_hex_escape(text, index + 1, 8, pos)?;
                        out.push(value);
                        index += 8;
                    }
                    _ => return Err(pos.error("D1004", "invalid escape sequence")),
                }
                index += 1;
            }
            _ => {
                let ch = text[index..]
                    .chars()
                    .next()
                    .ok_or_else(|| pos.error("D1004", "unterminated double-quoted scalar"))?;
                out.push(ch);
                index += ch.len_utf8();
            }
        }
    }

    Err(pos.error("D1004", "unterminated double-quoted scalar"))
}

fn parse_hex_escape(text: &str, start: usize, digits: usize, pos: Position) -> Result<char, Error> {
    let end = start + digits;
    let slice = text
        .get(start..end)
        .ok_or_else(|| pos.error("D1004", "truncated escape sequence"))?;
    let value = u32::from_str_radix(slice, 16)
        .map_err(|_| pos.error("D1004", "invalid escape sequence"))?;
    char::from_u32(value).ok_or_else(|| pos.error("D1004", "invalid Unicode escape"))
}

fn resolve_scalar(text: &str, pos: Position) -> Result<Node, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Node::null(pos));
    }
    if matches!(text, "~" | "null" | "Null" | "NULL") {
        return Ok(Node::null(pos));
    }
    if matches!(text, "true" | "True" | "TRUE") {
        return Ok(Node::boolean(true, pos));
    }
    if matches!(text, "false" | "False" | "FALSE") {
        return Ok(Node::boolean(false, pos));
    }

    let lower = text.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        ".inf" | "+.inf" | "-.inf" | ".nan" | "+.nan" | "-.nan"
    ) {
        return Err(pos.error("D1009", "non-finite numbers are not supported"));
    }

    if looks_numeric(text) && text.contains('_') {
        return Err(pos.error("D1009", "numeric underscores are not supported"));
    }

    let (sign, rest) = if let Some(rest) = text.strip_prefix('+') {
        (1_i128, rest)
    } else if let Some(rest) = text.strip_prefix('-') {
        (-1_i128, rest)
    } else {
        (1_i128, text)
    };

    if rest.starts_with("0x") || rest.starts_with("0X") {
        return parse_radix(rest, 16, sign, pos);
    }
    if rest.starts_with("0o") || rest.starts_with("0O") {
        return parse_radix(rest, 8, sign, pos);
    }
    if rest.starts_with("0b") || rest.starts_with("0B") {
        return parse_radix(rest, 2, sign, pos);
    }

    if !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()) {
        let value = rest
            .parse::<i128>()
            .map_err(|_| pos.error("D1009", "integer is out of range"))?;
        return integer_node(sign * value, pos);
    }

    if looks_float(rest) {
        let normalized = normalize_float(text);
        let value = normalized
            .parse::<f64>()
            .map_err(|_| pos.error("D1009", "malformed floating-point number"))?;
        if !value.is_finite() {
            return Err(pos.error("D1009", "floating-point number is not finite"));
        }
        return Ok(Node::float(value, pos));
    }

    Ok(Node::string(text.to_owned(), pos))
}

fn looks_numeric(text: &str) -> bool {
    let rest = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    rest.as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
}

fn looks_float(rest: &str) -> bool {
    if rest.is_empty() {
        return false;
    }
    if !(rest.contains('.') || rest.contains('e') || rest.contains('E')) {
        return false;
    }

    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(position) => (&rest[..position], Some(&rest[position + 1..])),
        None => (rest, None),
    };

    let mut dots = 0;
    let mut digits = 0;
    for ch in mantissa.chars() {
        match ch {
            '0'..='9' => digits += 1,
            '.' => dots += 1,
            _ => return false,
        }
    }
    if dots > 1 || digits == 0 {
        return false;
    }

    if let Some(exponent) = exponent {
        let exponent = exponent
            .strip_prefix('+')
            .or_else(|| exponent.strip_prefix('-'))
            .unwrap_or(exponent);
        if exponent.is_empty() || !exponent.chars().all(|ch| ch.is_ascii_digit()) {
            return false;
        }
    }

    true
}

fn normalize_float(text: &str) -> String {
    let (sign, rest) = if let Some(rest) = text.strip_prefix('+') {
        ("+", rest)
    } else if let Some(rest) = text.strip_prefix('-') {
        ("-", rest)
    } else {
        ("", text)
    };
    let mut body = rest.to_owned();
    if body.starts_with('.') {
        body.insert(0, '0');
    }
    if body.ends_with('.') {
        body.push('0');
    }
    body = body.replace(".e", ".0e").replace(".E", ".0E");
    format!("{sign}{body}")
}

fn parse_radix(rest: &str, radix: u32, sign: i128, pos: Position) -> Result<Node, Error> {
    let digits = &rest[2..];
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_digit(radix)) {
        return Err(pos.error("D1009", "malformed integer literal"));
    }
    let value = i128::from_str_radix(digits, radix)
        .map_err(|_| pos.error("D1009", "integer is out of range"))?;
    integer_node(sign * value, pos)
}

fn integer_node(value: i128, pos: Position) -> Result<Node, Error> {
    let value = i64::try_from(value).map_err(|_| pos.error("D1009", "integer is out of range"))?;
    Ok(Node::integer(value, pos))
}

fn is_sequence_line(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.first() == Some(&b'-')
        && (bytes.len() == 1 || bytes.get(1) == Some(&b' ') || bytes.get(1) == Some(&b'\t'))
}

fn is_document_marker(text: &str) -> bool {
    let text = text.trim();
    text == "---" || text.starts_with("--- ") || text == "..." || text.starts_with("... ")
}

fn strip_comment(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut single = false;
    let mut double = false;
    let mut escape = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if double {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                double = false;
            }
            index += 1;
            continue;
        }
        if single {
            if byte == b'\'' {
                if bytes.get(index + 1) == Some(&b'\'') {
                    index += 2;
                    continue;
                }
                single = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => double = true,
            b'\'' => single = true,
            b'#' if index == 0
                || bytes.get(index - 1) == Some(&b' ')
                || bytes.get(index - 1) == Some(&b'\t') =>
            {
                return &text[..index];
            }
            _ => {}
        }
        index += 1;
    }

    text
}

fn find_mapping_colon(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut single = false;
    let mut double = false;
    let mut escape = false;
    let mut depth = 0_u32;

    while index < bytes.len() {
        let byte = bytes[index];
        if double {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                double = false;
            }
            index += 1;
            continue;
        }
        if single {
            if byte == b'\'' {
                if bytes.get(index + 1) == Some(&b'\'') {
                    index += 2;
                    continue;
                }
                single = false;
            }
            index += 1;
            continue;
        }

        match byte {
            b'"' => double = true,
            b'\'' => single = true,
            b'[' | b'{' => depth = depth.saturating_add(1),
            b']' | b'}' => depth = depth.saturating_sub(1),
            b':' if depth == 0
                && (bytes.get(index + 1).is_none()
                    || bytes.get(index + 1) == Some(&b' ')
                    || bytes.get(index + 1) == Some(&b'\t')) =>
            {
                return Some(index);
            }
            _ => {}
        }
        index += 1;
    }

    None
}

struct FlowParser<'a> {
    text: &'a str,
    start: Position,
    index: usize,
    depth: usize,
}

impl<'a> FlowParser<'a> {
    fn new(text: &'a str, start: Position, depth: usize) -> Self {
        Self {
            text,
            start,
            index: 0,
            depth,
        }
    }

    fn parse_document_value(&mut self) -> Result<Node, Error> {
        let node = self.parse_value(self.depth)?;
        self.skip_ws();
        if self.index < self.text.len() {
            return Err(self
                .pos_at(self.index)
                .error("D1005", "unexpected content after flow collection"));
        }
        Ok(node)
    }

    fn parse_value(&mut self, depth: usize) -> Result<Node, Error> {
        self.skip_ws();
        let byte = *self.text.as_bytes().get(self.index).ok_or_else(|| {
            self.pos_at(self.index)
                .error("D1005", "unexpected end of flow collection")
        })?;

        match byte {
            b'[' => self.parse_sequence(depth),
            b'{' => self.parse_mapping(depth),
            b'"' | b'\'' => {
                let start = self.index;
                let (value, used) = parse_quoted_at(self.text, start, self.pos_at(start))?;
                self.index = used;
                Ok(Node::string(value, self.pos_at(start)))
            }
            b',' | b']' | b'}' => Err(self
                .pos_at(self.index)
                .error("D1005", "expected a flow value")),
            _ => self.parse_plain(),
        }
    }

    fn parse_plain(&mut self) -> Result<Node, Error> {
        let start = self.index;
        while let Some(&byte) = self.text.as_bytes().get(self.index) {
            if matches!(byte, b',' | b']' | b'}') {
                break;
            }
            self.index += 1;
        }
        let raw = self.text[start..self.index].trim();
        if raw.is_empty() {
            return Err(self.pos_at(start).error("D1005", "expected a flow value"));
        }
        let pos = self.pos_at(start);
        if raw.starts_with('&') {
            return Err(pos.error("D1003", "anchors are not supported"));
        }
        if raw.starts_with('*') {
            return Err(pos.error("D1003", "aliases are not supported"));
        }
        if raw.starts_with('!') {
            return Err(pos.error("D1003", "tags are not supported"));
        }
        if raw.starts_with('%') {
            return Err(pos.error("D1003", "directives are not supported"));
        }
        if raw.starts_with('@') || raw.starts_with('`') {
            return Err(pos.error("D1003", "reserved indicators are not supported"));
        }
        if raw == "?" || raw.starts_with("? ") {
            return Err(pos.error("D1003", "complex keys are not supported"));
        }
        resolve_scalar(raw, pos)
    }

    fn parse_sequence(&mut self, depth: usize) -> Result<Node, Error> {
        if depth > MAX_DEPTH {
            return Err(self
                .pos_at(self.index)
                .error("D1006", "nesting depth limit of 128 exceeded"));
        }
        let pos = self.pos_at(self.index);
        self.index += 1;
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.index += 1;
            return Ok(Node {
                value: YamlValue::Sequence(items),
                pos,
            });
        }

        loop {
            self.skip_ws();
            if self.peek().is_none() {
                return Err(pos.error("D1005", "unterminated flow sequence"));
            }
            items.push(self.parse_value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b']') => {
                    self.index += 1;
                    break;
                }
                Some(b',') => {
                    self.index += 1;
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        self.index += 1;
                        break;
                    }
                }
                None => return Err(pos.error("D1005", "unterminated flow sequence")),
                Some(_) => {
                    return Err(self
                        .pos_at(self.index)
                        .error("D1005", "expected ',' or ']' in flow sequence"));
                }
            }
        }

        Ok(Node {
            value: YamlValue::Sequence(items),
            pos,
        })
    }

    fn parse_mapping(&mut self, depth: usize) -> Result<Node, Error> {
        if depth > MAX_DEPTH {
            return Err(self
                .pos_at(self.index)
                .error("D1006", "nesting depth limit of 128 exceeded"));
        }
        let pos = self.pos_at(self.index);
        self.index += 1;
        self.skip_ws();
        let mut mapping = IndexMap::new();
        if self.peek() == Some(b'}') {
            self.index += 1;
            return Ok(Node {
                value: YamlValue::Mapping(mapping),
                pos,
            });
        }

        loop {
            self.skip_ws();
            if self.peek().is_none() {
                return Err(pos.error("D1005", "unterminated flow mapping"));
            }
            let key_pos = self.pos_at(self.index);
            let key = match self.peek() {
                Some(b'"') | Some(b'\'') => {
                    let start = self.index;
                    let (key, used) = parse_quoted_at(self.text, start, key_pos)?;
                    self.index = used;
                    if key == "<<" {
                        return Err(key_pos.error("D1003", "merge keys are not supported"));
                    }
                    key
                }
                _ => {
                    let start = self.index;
                    while let Some(&byte) = self.text.as_bytes().get(self.index) {
                        if byte == b':' || matches!(byte, b',' | b'}') {
                            break;
                        }
                        self.index += 1;
                    }
                    parse_key_text(&self.text[start..self.index], key_pos)?
                }
            };

            self.skip_ws();
            match self.peek() {
                Some(b':') => self.index += 1,
                None => return Err(pos.error("D1005", "unterminated flow mapping")),
                Some(_) => {
                    return Err(self
                        .pos_at(self.index)
                        .error("D1005", "expected ':' after flow mapping key"));
                }
            }
            self.skip_ws();
            if self.peek().is_none() {
                return Err(pos.error("D1005", "unterminated flow mapping"));
            }
            let value = if matches!(self.peek(), Some(b',') | Some(b'}')) {
                Node::null(self.pos_at(self.index))
            } else {
                self.parse_value(depth + 1)?
            };

            if mapping.contains_key(&key) {
                return Err(key_pos.error("D1001", "duplicate mapping key"));
            }
            mapping.insert(key, value);

            self.skip_ws();
            match self.peek() {
                Some(b'}') => {
                    self.index += 1;
                    break;
                }
                Some(b',') => {
                    self.index += 1;
                    self.skip_ws();
                    if self.peek() == Some(b'}') {
                        self.index += 1;
                        break;
                    }
                }
                None => return Err(pos.error("D1005", "unterminated flow mapping")),
                Some(_) => {
                    return Err(self
                        .pos_at(self.index)
                        .error("D1005", "expected ',' or '}' in flow mapping"));
                }
            }
        }

        Ok(Node {
            value: YamlValue::Mapping(mapping),
            pos,
        })
    }

    fn skip_ws(&mut self) {
        while let Some(&byte) = self.text.as_bytes().get(self.index) {
            if byte == b' ' || byte == b'\t' {
                self.index += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.index).copied()
    }

    fn pos_at(&self, byte: usize) -> Position {
        self.start.advance(self.text, byte)
    }
}

struct SeqDeserializer {
    iter: std::vec::IntoIter<Node>,
}

impl<'de> SeqAccess<'de> for SeqDeserializer {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(node) => seed.deserialize(node).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct MapDeserializer {
    iter: indexmap::map::IntoIter<String, Node>,
    value: Option<Node>,
}

impl<'de> MapAccess<'de> for MapDeserializer {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(de::value::StringDeserializer::new(key))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: DeserializeSeed<'de>,
    {
        let value = self.value.take().ok_or_else(|| Error {
            code: "D1010",
            message: "map value is missing".to_owned(),
            line: 1,
            column: 1,
            offset: 0,
        })?;
        seed.deserialize(value)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

impl<'de> Deserializer<'de> for Node {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Null => visitor.visit_unit(),
            YamlValue::Bool(value) => visitor.visit_bool(value),
            YamlValue::Integer(value) => visitor.visit_i64(value),
            YamlValue::Float(value) => visitor.visit_f64(value),
            YamlValue::String(value) => visitor.visit_string(value),
            YamlValue::Sequence(items) => visitor.visit_seq(SeqDeserializer {
                iter: items.into_iter(),
            }),
            YamlValue::Mapping(mapping) => visitor.visit_map(MapDeserializer {
                iter: mapping.into_iter(),
                value: None,
            }),
        }
        .map_err(|error| at_position(error, pos))
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Bool(value) => visitor.visit_bool(value),
            _ => Err(pos.error("D1010", "expected a boolean")),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) => visitor.visit_i64(value),
            _ => Err(pos.error("D1010", "expected an integer")),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) if value >= 0 => visitor.visit_u64(value as u64),
            YamlValue::Integer(_) => Err(pos.error("D1010", "expected a non-negative integer")),
            _ => Err(pos.error("D1010", "expected a non-negative integer")),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) => match i8::try_from(value) {
                Ok(value) => visitor.visit_i8(value),
                Err(_) => Err(pos.error("D1010", "integer is out of range for i8")),
            },
            _ => Err(pos.error("D1010", "expected an integer")),
        }
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) => match i16::try_from(value) {
                Ok(value) => visitor.visit_i16(value),
                Err(_) => Err(pos.error("D1010", "integer is out of range for i16")),
            },
            _ => Err(pos.error("D1010", "expected an integer")),
        }
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) => match i32::try_from(value) {
                Ok(value) => visitor.visit_i32(value),
                Err(_) => Err(pos.error("D1010", "integer is out of range for i32")),
            },
            _ => Err(pos.error("D1010", "expected an integer")),
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) => match u8::try_from(value) {
                Ok(value) => visitor.visit_u8(value),
                Err(_) => Err(pos.error("D1010", "integer is out of range for u8")),
            },
            _ => Err(pos.error("D1010", "expected a non-negative integer")),
        }
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) => match u16::try_from(value) {
                Ok(value) => visitor.visit_u16(value),
                Err(_) => Err(pos.error("D1010", "integer is out of range for u16")),
            },
            _ => Err(pos.error("D1010", "expected a non-negative integer")),
        }
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Integer(value) => match u32::try_from(value) {
                Ok(value) => visitor.visit_u32(value),
                Err(_) => Err(pos.error("D1010", "integer is out of range for u32")),
            },
            _ => Err(pos.error("D1010", "expected a non-negative integer")),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Float(value) => {
                let value = value as f32;
                if value.is_finite() {
                    visitor.visit_f32(value)
                } else {
                    Err(pos.error("D1010", "number is out of range for f32"))
                }
            }
            YamlValue::Integer(value) => visitor.visit_f32(value as f32),
            _ => Err(pos.error("D1010", "expected a number")),
        }
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Float(value) => visitor.visit_f64(value),
            YamlValue::Integer(value) => visitor.visit_f64(value as f64),
            _ => Err(pos.error("D1010", "expected a number")),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::String(value) => visitor.visit_string(value),
            _ => Err(pos.error("D1010", "expected a string")),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::String(value) => {
                let mut chars = value.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) => visitor.visit_char(ch),
                    _ => Err(pos.error("D1010", "expected a single character")),
                }
            }
            _ => Err(pos.error("D1010", "expected a single character")),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Null => visitor.visit_none(),
            value => visitor.visit_some(Node { value, pos }),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Null => visitor.visit_unit(),
            _ => Err(pos.error("D1010", "expected null")),
        }
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Sequence(items) => visitor.visit_seq(SeqDeserializer {
                iter: items.into_iter(),
            }),
            _ => Err(pos.error("D1010", "expected a sequence")),
        }
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::Mapping(mapping) => visitor.visit_map(MapDeserializer {
                iter: mapping.into_iter(),
                value: None,
            }),
            _ => Err(pos.error("D1010", "expected a mapping")),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::String(variant) => visitor.visit_enum(UnitEnum { variant, pos }),
            YamlValue::Mapping(mapping) if mapping.len() == 1 => {
                let (variant, value) = mapping
                    .into_iter()
                    .next()
                    .ok_or_else(|| pos.error("D1010", "expected an enum variant"))?;
                visitor.visit_enum(ValueEnum {
                    variant,
                    value,
                    pos,
                })
            }
            _ => Err(pos.error("D1010", "expected an enum variant")),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let pos = self.pos;
        match self.value {
            YamlValue::String(value) => visitor.visit_byte_buf(value.into_bytes()),
            _ => Err(pos.error("D1010", "expected a string")),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }
}

struct UnitEnum {
    variant: String,
    pos: Position,
}

struct UnitOnly {
    pos: Position,
}

impl<'de> EnumAccess<'de> for UnitEnum {
    type Error = Error;
    type Variant = UnitOnly;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Error>
    where
        V: DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(de::value::StringDeserializer::new(self.variant))?;
        Ok((variant, UnitOnly { pos: self.pos }))
    }
}

impl<'de> VariantAccess<'de> for UnitOnly {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Error>
    where
        T: DeserializeSeed<'de>,
    {
        Err(self.pos.error("D1010", "expected a unit enum variant"))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        Err(self.pos.error("D1010", "expected a unit enum variant"))
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        Err(self.pos.error("D1010", "expected a unit enum variant"))
    }
}

struct ValueEnum {
    variant: String,
    value: Node,
    pos: Position,
}

impl<'de> EnumAccess<'de> for ValueEnum {
    type Error = Error;
    type Variant = ValueVariant;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Error>
    where
        V: DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(de::value::StringDeserializer::new(self.variant))?;
        Ok((
            variant,
            ValueVariant {
                value: self.value,
                pos: self.pos,
            },
        ))
    }
}

struct ValueVariant {
    value: Node,
    pos: Position,
}

impl<'de> VariantAccess<'de> for ValueVariant {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        let pos = self.pos;
        match self.value.value {
            YamlValue::Null => Ok(()),
            _ => Err(pos.error("D1010", "expected a unit enum variant")),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(self.value)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.value.deserialize_tuple(len, visitor)
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.value.deserialize_struct("", fields, visitor)
    }
}

fn at_position(mut error: Error, pos: Position) -> Error {
    if error.line == 1 && error.column == 1 && error.offset == 0 {
        error.line = pos.line;
        error.column = pos.column;
        error.offset = pos.offset;
    }
    error
}
