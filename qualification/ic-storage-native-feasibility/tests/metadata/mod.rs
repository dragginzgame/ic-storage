//! Narrow LLVM debug-metadata reader for this diagnostic fixture only.
//! Unknown reachable shapes refuse; this does not issue persistence admission.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub path: String,
    pub kind: &'static str,
    pub name: String,
    pub size_bits: u64,
    pub align_bits: Option<u64>,
    pub offset_bits: u64,
}

pub fn inspect(ir: &str) -> Result<Vec<Observation>, String> {
    if ir.len() > 16 * 1024 * 1024 {
        return Err("diagnostic input exceeds 16 MiB".into());
    }
    let mut nodes = BTreeMap::new();
    for line in ir.lines() {
        let Some((id, value)) = line.split_once(" = ") else {
            continue;
        };
        let Some(digits) = id.strip_prefix('!') else {
            continue;
        };
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let id = digits.parse::<usize>().map_err(|e| e.to_string())?;
        if nodes.insert(id, value).is_some() {
            return Err(format!("duplicate node !{id}"));
        }
    }
    let mut roots = Vec::new();
    for (&id, value) in &nodes {
        if value.starts_with("!DICompositeType(") {
            let attrs = attributes(value, "!DICompositeType(")?;
            if attrs.get("name") == Some(&"\"LayoutProbeRecord\"") {
                roots.push(id);
            }
        }
    }
    if roots.len() != 1 {
        return Err(format!("expected one root, found {}", roots.len()));
    }
    let mut reader = Reader {
        nodes,
        active: BTreeSet::new(),
        output: Vec::new(),
    };
    reader.visit(roots[0], "record", 0)?;
    Ok(reader.output)
}

struct Reader<'a> {
    nodes: BTreeMap<usize, &'a str>,
    active: BTreeSet<usize>,
    output: Vec<Observation>,
}

impl Reader<'_> {
    fn node(&self, id: usize) -> Result<&str, String> {
        self.nodes
            .get(&id)
            .copied()
            .ok_or_else(|| format!("missing node !{id}"))
    }

    fn visit(&mut self, id: usize, path: &str, offset: u64) -> Result<(), String> {
        if self.active.len() >= 32 || !self.active.insert(id) {
            return Err("recursive or too-deep diagnostic shape".into());
        }
        let node = self.node(id)?.to_owned();
        let (kind, attrs) = if node.starts_with("!DICompositeType(") {
            let attrs = attributes(&node, "!DICompositeType(")?;
            if required(&attrs, "tag")? != "DW_TAG_structure_type" {
                return Err("unsupported composite kind".into());
            }
            if attrs
                .get("flags")
                .is_some_and(|flags| flags.contains("DIFlagFwdDecl"))
            {
                return Err("incomplete forward declaration".into());
            }
            ("structure", attrs)
        } else if node.starts_with("!DIBasicType(") {
            ("scalar", attributes(&node, "!DIBasicType(")?)
        } else if node.starts_with("!DIDerivedType(") {
            let attrs = attributes(&node, "!DIDerivedType(")?;
            if required(&attrs, "tag")? != "DW_TAG_pointer_type" {
                return Err("unsupported derived kind".into());
            }
            self.node(reference(required(&attrs, "baseType")?)?)?;
            ("pointer", attrs)
        } else {
            return Err(format!("unsupported type node !{id}"));
        };
        let size = number(&attrs, "size")?.unwrap_or(0);
        let align = number(&attrs, "align")?;
        if kind != "structure" && size == 0 {
            return Err("missing scalar/pointer size".into());
        }
        if kind == "structure" && align.is_none() {
            return Err("missing structure alignment".into());
        }
        self.output.push(Observation {
            path: path.to_owned(),
            kind,
            name: quoted(required(&attrs, "name")?)?.to_owned(),
            size_bits: size,
            align_bits: align,
            offset_bits: offset,
        });
        if kind == "structure" {
            let elements = self
                .node(reference(required(&attrs, "elements")?)?)?
                .to_owned();
            let fields = elements
                .strip_prefix("!{")
                .and_then(|s| s.strip_suffix('}'))
                .ok_or("invalid element list")?;
            let mut names = BTreeSet::new();
            let fields: Vec<_> = if fields.is_empty() {
                Vec::new()
            } else {
                fields.split(", ").collect()
            };
            for field in fields {
                let member_id = reference(field)?;
                let member = self.node(member_id)?.to_owned();
                let attrs = attributes(&member, "!DIDerivedType(")?;
                if required(&attrs, "tag")? != "DW_TAG_member"
                    || reference(required(&attrs, "scope")?)? != id
                {
                    return Err("member kind/scope mismatch".into());
                }
                let name = quoted(required(&attrs, "name")?)?;
                if !names.insert(name.to_owned()) {
                    return Err("duplicate member name".into());
                }
                let field_offset = number(&attrs, "offset")?.unwrap_or(0);
                let field_size = number(&attrs, "size")?.unwrap_or(0);
                if field_offset
                    .checked_add(field_size)
                    .is_none_or(|end| end > size)
                {
                    return Err("member outside containing type".into());
                }
                let child = reference(required(&attrs, "baseType")?)?;
                let start = self.output.len();
                self.visit(child, &format!("{path}.{name}"), field_offset)?;
                if self.output[start].size_bits != field_size {
                    return Err("member/type size mismatch".into());
                }
            }
        }
        // Pointers are leaves: their pointee does not describe the allocation's
        // element count, initialization, provenance or allocator metadata.
        self.active.remove(&id);
        Ok(())
    }
}

fn attributes<'a>(node: &'a str, prefix: &str) -> Result<BTreeMap<&'a str, &'a str>, String> {
    let body = node
        .strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(')'))
        .ok_or("unexpected metadata node")?;
    let mut result = BTreeMap::new();
    let (mut quoted, mut start) = (false, 0);
    for (index, byte) in body.bytes().enumerate() {
        if byte == b'\\' {
            return Err("escaped diagnostic attributes unsupported".into());
        }
        if byte == b'"' {
            quoted = !quoted;
        }
        if byte == b',' && !quoted {
            insert_attribute(&mut result, &body[start..index])?;
            start = index + 1;
        }
    }
    if quoted {
        return Err("unterminated attribute string".into());
    }
    insert_attribute(&mut result, &body[start..])?;
    Ok(result)
}

fn insert_attribute<'a>(
    attrs: &mut BTreeMap<&'a str, &'a str>,
    part: &'a str,
) -> Result<(), String> {
    let (key, value) = part
        .trim()
        .split_once(": ")
        .ok_or("invalid metadata attribute")?;
    if attrs.insert(key, value).is_some() {
        return Err("duplicate metadata attribute".into());
    }
    Ok(())
}

fn required<'a>(attrs: &BTreeMap<&str, &'a str>, key: &str) -> Result<&'a str, String> {
    attrs
        .get(key)
        .copied()
        .ok_or_else(|| format!("missing {key}"))
}

fn number(attrs: &BTreeMap<&str, &str>, key: &str) -> Result<Option<u64>, String> {
    attrs
        .get(key)
        .map(|n| n.parse().map_err(|e| format!("invalid {key}: {e}")))
        .transpose()
}

fn reference(value: &str) -> Result<usize, String> {
    value
        .strip_prefix('!')
        .ok_or_else(|| "expected node reference".to_owned())?
        .parse()
        .map_err(|e| format!("invalid reference: {e}"))
}

fn quoted(value: &str) -> Result<&str, String> {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .ok_or_else(|| "expected quoted name".into())
}

#[cfg(test)]
mod tests;
