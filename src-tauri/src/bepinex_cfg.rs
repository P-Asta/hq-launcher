use std::{io::BufRead, path::Path};
use std::ops::Range;

use serde::{Deserialize, Serialize};

pub const FLAGS_MESSAGE: &str =
    "# Multiple values can be set at the same time by separating them with , (e.g. Debug, Warning)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Num<T> {
    pub value: T,
    #[serde(default)]
    pub range: Option<Range<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub mod_name: String,
    pub mod_version: String,
    pub mod_guid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    pub metadata: Option<Metadata>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub description: Option<String>,
    pub default: Option<Value>,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Value {
    Bool(bool),
    String(String),
    Int(Num<i32>),
    Float(Num<f32>),
    Enum { index: usize, options: Vec<String> },
    Flags { indicies: Vec<usize>, options: Vec<String> },
}

#[derive(Debug, Default)]
struct EntryBuilder {
    description: Option<String>,
    type_name: Option<String>,
    default_value: Option<String>,
    acceptable_values: Option<Vec<String>>,
    is_flags: bool,
    range: Option<(String, String)>,
    name: Option<String>,
    value: Option<String>,
}

fn parse_num_i32(value: &str, range: Option<&(String, String)>) -> Result<Num<i32>, String> {
    let value: i32 = value
        .trim()
        .parse::<i32>()
        .map_err(|e| e.to_string())?;
    let range = match range {
        Some((min, max)) => {
            let min: i32 = min
                .trim()
                .parse::<i32>()
                .map_err(|e| e.to_string())?;
            let max: i32 = max
                .trim()
                .parse::<i32>()
                .map_err(|e| e.to_string())?;
            Some(min..max)
        }
        None => None,
    };
    Ok(Num { value, range })
}

fn parse_num_f32(value: &str, range: Option<&(String, String)>) -> Result<Num<f32>, String> {
    // support commas as decimal separators
    let value: f32 = value
        .trim()
        .replace(',', ".")
        .parse::<f32>()
        .map_err(|e| e.to_string())?;
    let range = match range {
        Some((min, max)) => {
            let min: f32 = min
                .trim()
                .replace(',', ".")
                .parse::<f32>()
                .map_err(|e| e.to_string())?;
            let max: f32 = max
                .trim()
                .replace(',', ".")
                .parse::<f32>()
                .map_err(|e| e.to_string())?;
            Some(min..max)
        }
        None => None,
    };
    Ok(Num { value, range })
}

fn parse_orphaned_entry_line(line: &str) -> Option<(&str, &str)> {
    line.split_once('=')
        .map(|(name, value)| (name.trim(), value.trim()))
}

fn check_value_type(value: &str) -> String {
    let first_char = value.chars().next().unwrap_or_default();
    match first_char {
        '0'..='9' => {
            if value.contains('.') {
                "Single".to_string()
            } else {
                "Int32".to_string()
            }
        }
        't' | 'f' => "Boolean".to_string(),
        _ => "String".to_string(),
    }
}

impl EntryBuilder {
    fn build(self) -> Result<ParsedEntry, String> {
        let name = self.name.ok_or("No entry name".to_string())?;
        let value_raw = self.value.unwrap_or_else(|| "".to_string());

        let type_name = self.type_name.unwrap_or_else(|| {
            check_value_type(&value_raw)
        });

        let default = match self.default_value {
            Some(v) => Some(Self::parse_value(
                v,
                self.acceptable_values.clone(),
                &type_name,
                self.range.as_ref(),
                self.is_flags,
            )?),
            None => None,
        };

        let value = Self::parse_value(
            value_raw,
            self.acceptable_values,
            &type_name,
            self.range.as_ref(),
            self.is_flags,
        )?;

        Ok(ParsedEntry {
            name,
            description: self.description,
            type_name,
            default,
            value,
        })
    }

    fn parse_value(
        string: String,
        options: Option<Vec<String>>,
        type_name: &str,
        range: Option<&(String, String)>,
        is_flags: bool,
    ) -> Result<ParsedValue, String> {
        match options {
            Some(options) => Ok(ParsedValue::EnumLike(Self::parse_enum(
                string, options, is_flags,
            ))),
            None => Ok(ParsedValue::Simple(Self::parse_simple_value(
                string, type_name, range,
            )?)),
        }
    }

    fn parse_enum(string: String, options: Vec<String>, is_flags: bool) -> EnumLikeValue {
        if is_flags {
            let indicies = string
                .split(", ")
                .filter_map(|value| options.iter().position(|opt| opt == value))
                .collect();
            EnumLikeValue::Flags { indicies, options }
        } else {
            let index = options
                .iter()
                .position(|opt| *opt == string)
                .unwrap_or_default();
            EnumLikeValue::Enum { index, options }
        }
    }

    fn parse_simple_value(
        value: String,
        type_name: &str,
        range: Option<&(String, String)>,
    ) -> Result<SimpleValue, String> {
        match type_name {
            "Boolean" => Ok(SimpleValue::Bool(
                value
                    .trim()
                    .parse::<bool>()
                    .map_err(|e| e.to_string())?,
            )),
            "String" => Ok(SimpleValue::String(value.replace(r"\n", "\n"))),
            "Int32" | "Number" => Ok(SimpleValue::Int(parse_num_i32(&value, range)?)),
            "Single" | "Double" => Ok(SimpleValue::Float(parse_num_f32(&value, range)?)),
            _ => Ok(SimpleValue::String(value)),
        }
    }
}


// {
//     "name": "WebSocketSharp_netstandard",
//     "description": "NuGet WebSocketSharp-netstandard package re-bundled for convenient consumption and dependency management.",
//     "version_number": "1.0.100",
//     "dependencies": [],
//     "website_url": "https://nuget.org/packages/WebSocketSharp-netstandard/1.0.1",
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BepInExManifest {
    pub name: String,
    pub description: String,
    pub version_number: String,
    pub dependencies: Vec<String>,
    pub website_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallerEntry {
    identifier: String,
}

pub fn read_manifest(path: &Path) -> Result<BepInExManifest, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str::<BepInExManifest>(&text).map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
struct ParsedEntry {
    name: String,
    description: Option<String>,
    #[allow(dead_code)]
    type_name: String,
    default: Option<ParsedValue>,
    value: ParsedValue,
}

#[derive(Debug, Clone)]
enum ParsedValue {
    Simple(SimpleValue),
    EnumLike(EnumLikeValue),
}

#[derive(Debug, Clone)]
enum SimpleValue {
    Bool(bool),
    String(String),
    Int(Num<i32>),
    Float(Num<f32>),
}

#[derive(Debug, Clone)]
enum EnumLikeValue {
    Enum { index: usize, options: Vec<String> },
    Flags { indicies: Vec<usize>, options: Vec<String> },
}

fn parsed_to_value(v: ParsedValue) -> Value {
    match v {
        ParsedValue::Simple(s) => match s {
            SimpleValue::Bool(b) => Value::Bool(b),
            SimpleValue::String(s) => Value::String(s),
            SimpleValue::Int(n) => Value::Int(n),
            SimpleValue::Float(n) => Value::Float(n),
        },
        ParsedValue::EnumLike(e) => match e {
            EnumLikeValue::Enum { index, options } => Value::Enum { index, options },
            EnumLikeValue::Flags { indicies, options } => Value::Flags { indicies, options },
        },
    }
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.replace('\n', r"\n"),
        Value::Int(n) => n.value.to_string(),
        Value::Float(n) => {
            // keep dot decimal
            format!("{}", n.value)
        }
        Value::Enum { index, options } => options.get(*index).cloned().unwrap_or_default(),
        Value::Flags { indicies, options } => {
            if indicies.is_empty() {
                return "0".to_string();
            }
            indicies
                .iter()
                .filter_map(|i| options.get(*i))
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

fn render_entry_comments(entry: &Entry, type_name: &str, options: Option<&[String]>) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    if let Some(desc) = &entry.description {
        for line in desc.lines() {
            out.push(format!("## {line}"));
        }
    }
    out.push(format!("# Setting type: {type_name}"));
    out.push(match &entry.default {
        Some(d) => format!("# Default value: {}", value_to_string(d)),
        None => "# Default value:".to_string(),
    });
    if let Some(opts) = options {
        out.push(format!("# Acceptable values: {}", opts.join(", ")));
    }
    match &entry.value {
        Value::Flags { .. } => out.push(FLAGS_MESSAGE.to_string()),
        Value::Int(n) => {
            if let Some(r) = &n.range {
                out.push(format!(
                    "# Acceptable value range: From {} to {}",
                    r.start, r.end
                ));
            }
        }
        Value::Float(n) => {
            if let Some(r) = &n.range {
                out.push(format!(
                    "# Acceptable value range: From {} to {}",
                    r.start, r.end
                ));
            }
        }
        _ => {}
    }
    out
}

fn infer_type_name(entry: &Entry) -> String {
    match &entry.value {
        Value::Bool(_) => "Boolean".to_string(),
        Value::String(_) => "String".to_string(),
        Value::Int(_) => "Int32".to_string(),
        Value::Float(_) => "Single".to_string(),
        Value::Enum { .. } => "String".to_string(),
        Value::Flags { .. } => "String".to_string(),
    }
}

fn value_options(entry: &Entry) -> Option<&[String]> {
    match &entry.value {
        Value::Enum { options, .. } => Some(options),
        Value::Flags { options, .. } => Some(options),
        _ => None,
    }
}

fn read_metadata_line(line: &str) -> Option<(String, String)> {
    // "## Settings file was created by plugin NAME VERSION"
    let prefix = "## Settings file was created by plugin ";
    let rest = line.strip_prefix(prefix)?;
    let mut parts: Vec<&str> = rest.split(' ').collect();
    if parts.len() < 2 {
        return None;
    }
    let version = parts.pop()?.to_string();
    let name = parts.join(" ");
    Some((name, version))
}

pub fn parse(text: &str) -> Result<FileData, String> {
    let reader = std::io::Cursor::new(text);
    parse_reader(reader)
}

pub fn parse_reader<R: BufRead>(mut reader: R) -> Result<FileData, String> {
    let mut line = String::new();
    let mut metadata: Option<Metadata> = None;
    let mut sections: Vec<Section> = vec![];
    let mut current_section: Option<Section> = None;

    // state while parsing an entry
    let mut pending_builder: Option<EntryBuilder> = None;
    let mut pending_desc_lines: Vec<String> = vec![];

    while {
        line.clear();
        reader.read_line(&mut line).map_err(|e| e.to_string())? > 0
    } {
        // trim newline
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }

        if line.is_empty() {
            continue;
        }

        // metadata header
        if line.starts_with("## Settings file was created by plugin ") {
            if let Some((name, version)) = read_metadata_line(&line) {
                // next line should be guid
                let mut guid_line = String::new();
                reader
                    .read_line(&mut guid_line)
                    .map_err(|e| e.to_string())?;
                while guid_line.ends_with('\n') || guid_line.ends_with('\r') {
                    guid_line.pop();
                }
                let guid = guid_line
                    .strip_prefix("## Plugin GUID: ")
                    .unwrap_or("")
                    .to_string();
                metadata = Some(Metadata {
                    mod_name: name,
                    mod_version: version,
                    mod_guid: guid,
                });
            }
            continue;
        }

        // section header
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(sec) = current_section.take() {
                sections.push(sec);
            }
            current_section = Some(Section {
                name: line[1..line.len() - 1].to_string(),
                entries: vec![],
            });
            continue;
        }

        // comments and entry parsing
        if line.starts_with("##") {
            // description line(s)
            pending_desc_lines.push(line.trim_start_matches("##").trim().to_string());
            continue;
        }

        if line == FLAGS_MESSAGE {
            let b = pending_builder.get_or_insert_with(EntryBuilder::default);
            b.is_flags = true;
            continue;
        }

        if let Some(meta) = line.strip_prefix("# ") {
            let b = pending_builder.get_or_insert_with(EntryBuilder::default);
            if let Some(t) = meta.strip_prefix("Setting type: ") {
                b.type_name = Some(t.to_string());
            } else if let Some(d) = meta.strip_prefix("Default value: ") {
                b.default_value = Some(d.to_string());
            } else if meta == "Default value:" {
                b.default_value = None;
            } else if let Some(v) = meta.strip_prefix("Acceptable values: ") {
                b.acceptable_values = Some(v.split(", ").map(|s| s.to_string()).collect());
            } else if let Some(range) = meta.strip_prefix("Acceptable value range: From ") {
                if let Some((min, max)) = range.split_once(" to ") {
                    b.range = Some((min.to_string(), max.to_string()));
                }
            }
            continue;
        }

        // actual entry line
        let Some((name, value)) = parse_orphaned_entry_line(&line) else {
            continue;
        };

        let b = pending_builder.take().unwrap_or_default();
        let mut b = EntryBuilder { ..b };
        if !pending_desc_lines.is_empty() {
            b.description = Some(pending_desc_lines.join("\n"));
        }
        pending_desc_lines.clear();
        b.name = Some(name.to_string());
        b.value = Some(value.to_string());

        let parsed = b.build()?;
        let entry = Entry {
            name: parsed.name,
            description: parsed.description,
            default: parsed.default.map(parsed_to_value),
            value: parsed_to_value(parsed.value),
        };

        current_section
            .as_mut()
            .ok_or("entry has no section".to_string())?
            .entries
            .push(entry);
    }

    if let Some(sec) = current_section.take() {
        sections.push(sec);
    }

    Ok(FileData { metadata, sections })
}

pub fn write(file: &FileData) -> Result<String, String> {
    let mut out: Vec<String> = vec![];

    if let Some(m) = &file.metadata {
        out.push(format!(
            "## Settings file was created by plugin {} {}",
            m.mod_name, m.mod_version
        ));
        out.push(format!("## Plugin GUID: {}", m.mod_guid));
        out.push(String::new());
    }

    for section in &file.sections {
        out.push(format!("[{}]", section.name));
        out.push(String::new());

        for entry in &section.entries {
            let type_name = infer_type_name(entry);
            let comments = render_entry_comments(entry, &type_name, value_options(entry));
            out.extend(comments);
            out.push(format!("{} = {}", entry.name, value_to_string(&entry.value)));
            out.push(String::new());
        }
    }

    Ok(out.join("\n"))
}

