//! One self-contained HTML page: the high-level component view of a diagnosed cluster.
//!
//! The audience is a person, not a pipeline: namespaces as sections, each with a diagram of its
//! **workloads, services and ingresses** — no pod or node boxes; pods appear only aggregated as
//! `ready/declared` on their workload — beside the namespace's findings, and the
//! [directions](mod@crate::directions) summary at the top of the page. Every component is
//! badge-colored by the worst finding against it, with pod findings rolled up to the owning
//! workload through the graph's derived ownership.
//!
//! # Network posture
//!
//! The page embeds the Mermaid *source* per namespace and loads the Mermaid renderer from a
//! **version-pinned CDN `<script>` tag**. Nothing in this workspace touches the network — the
//! gate renders and diffs the page as bytes; only the viewer's browser, on opening the file,
//! fetches the pinned script. Everything else (styles, data, diagram sources) is inline: one
//! file, no build step, no other external asset.
//!
//! # Determinism
//!
//! Same graph, diagnosis and properties in, byte-identical HTML out — every collection walked
//! here is ordered, and `tests/determinism.rs` holds the claim beside the crate's other
//! renderings.

use std::collections::{BTreeMap, BTreeSet};

use crate::code::Severity;
use crate::diagnose::{Diagnosis, Finding};
use crate::directions::directions;
use crate::graph::{EdgeRelation, GraphNode, InfraGraph, NodeKind};
use crate::properties::WorkloadProperties;

/// The Mermaid renderer the page loads — pinned, so the page a given build writes always names
/// the same script and the rendering cannot drift under the document.
const MERMAID_CDN: &str = "https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.min.js";

/// The node kinds the component view draws.
const COMPONENT_KINDS: [NodeKind; 3] = [NodeKind::Workload, NodeKind::Service, NodeKind::Ingress];

/// The relations that hold between drawn components.
const COMPONENT_RELATIONS: [EdgeRelation; 3] = [
    EdgeRelation::Selects,
    EdgeRelation::RoutesTo,
    EdgeRelation::GovernedBy,
];

/// Defuses text for an HTML context (element content and double-quoted attributes).
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Defuses what would end a quoted Mermaid string — the `graph` module's escaping, repeated
/// because this module writes its own labels.
fn mermaid_label(text: &str) -> String {
    text.replace('"', "#quot;").replace('\n', " ")
}

/// The namespace a finding's subject sits in: the first segment after the IR map name. Every
/// diagnosed map is namespaced, so this is total over real findings.
fn finding_namespace(finding: &Finding) -> Option<&str> {
    let (_, key) = finding.subject.split_once('/')?;
    key.split('/').next()
}

/// The drawn component a finding colors, when there is one: its own subject for workloads,
/// services and ingresses; the owning workload for a pod, through derived ownership.
fn component_of(finding: &Finding, graph: &InfraGraph) -> Option<GraphNode> {
    let (map, key) = finding.subject.split_once('/')?;
    match map {
        "workloads" => Some(GraphNode {
            kind: NodeKind::Workload,
            key: key.to_owned(),
        }),
        "services" => Some(GraphNode {
            kind: NodeKind::Service,
            key: key.to_owned(),
        }),
        "ingresses" => Some(GraphNode {
            kind: NodeKind::Ingress,
            key: key.to_owned(),
        }),
        "pods" => graph.owner_of(key).map(|workload| GraphNode {
            kind: NodeKind::Workload,
            key: workload.to_owned(),
        }),
        _ => None,
    }
}

/// The badge class of a component's worst severity.
const fn severity_class(severity: Option<Severity>) -> &'static str {
    match severity {
        Some(Severity::Error) => "sevError",
        Some(Severity::Warning) => "sevWarning",
        Some(Severity::Info) => "sevInfo",
        None => "sevNone",
    }
}

/// A component node's diagram label: `kind name` plus the pod aggregate for workloads.
fn component_label(node: &GraphNode, properties: &BTreeMap<&str, &WorkloadProperties>) -> String {
    match node.kind {
        NodeKind::Workload => {
            let mut parts = node.key.splitn(3, '/');
            let _ = parts.next();
            let kind = parts.next().unwrap_or_default();
            let name = parts.next().unwrap_or_default();
            match properties.get(node.key.as_str()) {
                Some(entry) => {
                    let declared = entry.replicas.unwrap_or(entry.observed_pods);
                    format!("{kind} {name} — {}/{declared} ready", entry.ready_pods)
                }
                None => format!("{kind} {name}"),
            }
        }
        other => format!("{} {}", other.noun(), node.name()),
    }
}

/// Renders the high-level component view of one diagnosed cluster as a single self-contained
/// HTML page; `namespace` restricts the page to one namespace's section, findings and
/// directions — the primary shape on a many-namespace cluster.
///
/// The graph, diagnosis and properties must come from one IR; the page renders what it is
/// handed and checks nothing.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn render_html(
    graph: &InfraGraph,
    diagnosis: &Diagnosis,
    properties: &[WorkloadProperties],
    namespace: Option<&str>,
) -> String {
    use std::fmt::Write as _;

    let properties_by_key: BTreeMap<&str, &WorkloadProperties> = properties
        .iter()
        .map(|entry| (entry.workload.as_str(), entry))
        .collect();

    // The findings in scope, and the worst severity per drawn component.
    let scoped: Vec<&Finding> = diagnosis
        .findings
        .iter()
        .filter(|finding| match namespace {
            Some(wanted) => finding_namespace(finding) == Some(wanted),
            None => true,
        })
        .collect();
    let mut worst: BTreeMap<GraphNode, Severity> = BTreeMap::new();
    for finding in &scoped {
        if let Some(component) = component_of(finding, graph) {
            let entry = worst.entry(component).or_insert(finding.severity);
            if finding.severity > *entry {
                *entry = finding.severity;
            }
        }
    }

    // The namespaces the page sections into: those with drawn components or findings.
    let mut namespaces: BTreeSet<&str> = graph
        .nodes()
        .iter()
        .filter(|node| COMPONENT_KINDS.contains(&node.kind))
        .filter_map(GraphNode::namespace)
        .collect();
    for finding in &scoped {
        namespaces.extend(finding_namespace(finding));
    }
    let namespaces: Vec<&str> = namespaces
        .into_iter()
        .filter(|candidate| namespace.is_none_or(|wanted| wanted == *candidate))
        .collect();

    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    let title = match namespace {
        Some(wanted) => format!("cluster components — namespace {wanted}"),
        None => "cluster components".to_owned(),
    };
    let _ = writeln!(out, "<title>{}</title>", escape(&title));
    out.push_str(
        "<style>\n\
         body { font: 14px/1.5 system-ui, sans-serif; margin: 2rem; color: #111827; }\n\
         h1 { font-size: 1.4rem; } h2 { font-size: 1.1rem; margin-top: 2.5rem;\n\
           border-bottom: 1px solid #e5e7eb; padding-bottom: .3rem; }\n\
         table { border-collapse: collapse; margin: .75rem 0; }\n\
         td, th { border: 1px solid #e5e7eb; padding: .25rem .6rem; text-align: left;\n\
           vertical-align: top; }\n\
         code { background: #f3f4f6; padding: 0 .25rem; }\n\
         .badge { display: inline-block; padding: 0 .5rem; border-radius: .6rem;\n\
           font-size: .8rem; }\n\
         .sevError { background: #fde8e8; color: #c81e1e; }\n\
         .sevWarning { background: #fdf6b2; color: #8e4b10; }\n\
         .sevInfo { background: #e1effe; color: #1a56db; }\n\
         .sevNone { background: #f3f4f6; color: #6b7280; }\n\
         pre.mermaid { background: #ffffff; }\n\
         </style>\n",
    );
    let _ = writeln!(out, "<script src=\"{MERMAID_CDN}\"></script>");
    out.push_str(
        "<script>if (window.mermaid) { mermaid.initialize({ startOnLoad: true, theme: \
         \"neutral\" }); }</script>\n</head>\n<body>\n",
    );
    let _ = writeln!(out, "<h1>{}</h1>", escape(&title));

    // Directions first: the page opens with what the findings add up to.
    let scoped_diagnosis = Diagnosis {
        findings: scoped.iter().map(|finding| (*finding).clone()).collect(),
    };
    let ranked = directions(&scoped_diagnosis, &[]);
    out.push_str("<h2>directions</h2>\n");
    if ranked.is_empty() {
        out.push_str("<p>no findings in scope.</p>\n");
    } else {
        out.push_str(
            "<table>\n<tr><th>severity</th><th>code</th><th>action</th>\
                      <th>subjects</th></tr>\n",
        );
        for direction in &ranked {
            let _ = writeln!(
                out,
                "<tr><td><span class=\"badge {}\">{}</span></td><td><code>{}</code></td>\
                 <td>{}</td><td title=\"{}\">{}</td></tr>",
                severity_class(Some(direction.severity)),
                direction.severity,
                escape(&direction.code),
                escape(&direction.action),
                escape(&direction.subjects.join(", ")),
                direction.subjects.len()
            );
        }
        out.push_str("</table>\n");
    }

    // One section per namespace: the diagram, then the findings beside it.
    for section in &namespaces {
        let _ = writeln!(out, "<h2>namespace {}</h2>", escape(section));

        let drawn: Vec<&GraphNode> = graph
            .nodes()
            .iter()
            .filter(|node| {
                COMPONENT_KINDS.contains(&node.kind) && node.namespace() == Some(section)
            })
            .collect();
        let mut ids: BTreeMap<&GraphNode, String> = BTreeMap::new();
        let mut per_kind: BTreeMap<NodeKind, usize> = BTreeMap::new();
        out.push_str("<pre class=\"mermaid\">\nflowchart LR\n");
        out.push_str(
            "    classDef sevError fill:#fde8e8,stroke:#c81e1e,stroke-width:2px;\n\
             \u{20}   classDef sevWarning fill:#fdf6b2,stroke:#8e4b10;\n\
             \u{20}   classDef sevInfo fill:#e1effe,stroke:#1a56db;\n\
             \u{20}   classDef sevNone fill:#f3f4f6,stroke:#6b7280;\n",
        );
        for node in &drawn {
            let index = per_kind.entry(node.kind).or_default();
            let id = format!(
                "n{}{index}",
                match node.kind {
                    NodeKind::Workload => "wl",
                    NodeKind::Service => "svc",
                    _ => "ing",
                }
            );
            *index += 1;
            let _ = writeln!(
                out,
                "    {id}[\"{}\"]:::{}",
                mermaid_label(&component_label(node, &properties_by_key)),
                severity_class(worst.get(*node).copied())
            );
            ids.insert(*node, id);
        }
        for edge in graph.edges() {
            if !COMPONENT_RELATIONS.contains(&edge.relation) {
                continue;
            }
            let (Some(from), Some(to)) = (ids.get(&edge.from), ids.get(&edge.to)) else {
                continue;
            };
            let _ = writeln!(
                out,
                "    {from} -->|\"{}\"| {to}",
                mermaid_label(edge.relation.verb())
            );
        }
        out.push_str("</pre>\n");

        // The same edges as text, each carrying its full evidence as a hover title — the
        // diagram shows the verb, the table shows the sites.
        let section_edges: Vec<_> = graph
            .edges()
            .filter(|edge| {
                COMPONENT_RELATIONS.contains(&edge.relation)
                    && edge.from.namespace() == Some(section)
                    && ids.contains_key(&edge.from)
                    && ids.contains_key(&edge.to)
            })
            .collect();
        if !section_edges.is_empty() {
            out.push_str(
                "<table>\n<tr><th>dependent</th><th>relation</th>\
                          <th>dependency</th></tr>\n",
            );
            for edge in &section_edges {
                let _ = writeln!(
                    out,
                    "<tr title=\"{}\"><td>{}</td><td>{}</td><td>{}</td></tr>",
                    escape(&edge.sites.join(", ")),
                    escape(&edge.from.to_string()),
                    escape(edge.relation.verb()),
                    escape(&edge.to.to_string())
                );
            }
            out.push_str("</table>\n");
        }

        let section_findings: Vec<&&Finding> = scoped
            .iter()
            .filter(|finding| finding_namespace(finding) == Some(section))
            .collect();
        if section_findings.is_empty() {
            out.push_str("<p>no findings in this namespace.</p>\n");
        } else {
            out.push_str(
                "<table>\n<tr><th>severity</th><th>code</th><th>subject</th>\
                          <th>message</th></tr>\n",
            );
            for finding in &section_findings {
                let evidence = finding
                    .evidence
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let subject = match &finding.site {
                    Some(site) => format!("{} ({site})", finding.subject),
                    None => finding.subject.clone(),
                };
                let _ = writeln!(
                    out,
                    "<tr title=\"{}\"><td><span class=\"badge {}\">{}</span></td>\
                     <td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
                    escape(&evidence),
                    severity_class(Some(finding.severity)),
                    finding.severity,
                    finding.code,
                    escape(&subject),
                    escape(&finding.message)
                );
            }
            out.push_str("</table>\n");
        }
    }

    if namespaces.is_empty() {
        out.push_str("<p>nothing observed in scope.</p>\n");
    }
    out.push_str("</body>\n</html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escaping_defuses_every_metacharacter_it_claims_to() {
        assert_eq!(escape("a<b>&\"c'"), "a&lt;b&gt;&amp;&quot;c&#39;");
    }

    #[test]
    fn the_severity_classes_cover_all_three_severities_and_none() {
        assert_eq!(severity_class(Some(Severity::Error)), "sevError");
        assert_eq!(severity_class(Some(Severity::Warning)), "sevWarning");
        assert_eq!(severity_class(Some(Severity::Info)), "sevInfo");
        assert_eq!(severity_class(None), "sevNone");
    }
}
