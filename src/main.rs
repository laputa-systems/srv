use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::HashMap,
    env,
    fmt::Write as _,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};

const LISTING_PRELUDE: &str = "<head><link rel=icon href=data:,><style>* { font-family: monospace; } \
     table { border: none; margin: 1rem; } td { padding-right: 2rem; }</style></head>\n\
     <table>";

const MARKDOWN_PRELUDE: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>Markdown</title><link rel=icon href=data:,><style>\
:root { color-scheme: light; }\
body { margin: 0; background: #f6f8fa; color: #24292f; }\
.markdown-body { box-sizing: border-box; max-width: 980px; margin: 0 auto; padding: 40px; \
font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", Helvetica, Arial, sans-serif; \
line-height: 1.6; }\
@media (max-width: 767px) { .markdown-body { padding: 20px; } }\
.markdown-body h1, .markdown-body h2 { border-bottom: 1px solid #d0d7de; padding-bottom: .3em; }\
.markdown-body code, .markdown-body pre { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }\
.markdown-body pre { background: #f6f8fa; padding: 16px; border-radius: 6px; overflow: auto; }\
.markdown-body code { background: rgba(175,184,193,.2); padding: .2em .4em; border-radius: 6px; }\
.markdown-body pre code { background: transparent; padding: 0; }\
.markdown-body table { border-collapse: collapse; }\
.markdown-body th, .markdown-body td { border: 1px solid #d0d7de; padding: 6px 13px; }\
.markdown-body tr:nth-child(2n) { background: #f6f8fa; }\
.markdown-body blockquote { margin: 0; padding: 0 1em; color: #57606a; border-left: .25em solid #d0d7de; }\
.markdown-body a { color: #0969da; text-decoration: none; }\
.markdown-body a:hover { text-decoration: underline; }\
</style></head><body><article class=\"markdown-body\">";

const MARKDOWN_POSTLUDE: &str = "</article></body></html>";

const DOCS_PAGE: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Docs</title>
<link rel="icon" href="data:,">
<style>
* { box-sizing: border-box; }
body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; color: #24292f; background: #fff; }
#app { display: grid; grid-template-columns: 300px 1fr; height: 100vh; }
#tree { overflow: auto; border-right: 1px solid #ececf1; background: #f7f7f8; padding: 10px 8px 14px; }
#viewer { overflow: auto; padding: 24px; }
#status { color: #8e8ea0; font-size: 11px; letter-spacing: .08em; text-transform: uppercase; padding: 8px 10px; }
#tree-root { display: flex; flex-direction: column; gap: 2px; }
details { margin: 0; border-radius: 8px; }
summary { cursor: pointer; user-select: none; list-style: none; font-size: 13px; color: #353740; padding: 6px 10px; border-radius: 8px; display: flex; align-items: center; gap: 6px; }
summary::-webkit-details-marker { display: none; }
summary::before { content: "▸"; font-size: 10px; color: #8e8ea0; transform-origin: 45% 50%; transition: transform .12s ease; }
details[open] > summary::before { transform: rotate(90deg); }
summary:hover { background: #ececf1; }
.dir-children { margin-left: 14px; padding-left: 8px; border-left: 1px solid #e3e3e8; display: flex; flex-direction: column; gap: 2px; }
button.file { display: block; width: 100%; text-align: left; border: 0; background: transparent; padding: 6px 10px; border-radius: 8px; cursor: pointer; font: inherit; font-size: 13px; color: #2f3138; }
button.file:hover { background: #ececf1; }
button.file.active { background: #e3e3e8; font-weight: 600; }
.markdown-body { max-width: 980px; margin: 0 auto; line-height: 1.6; }
.markdown-body h1, .markdown-body h2 { border-bottom: 1px solid #d0d7de; padding-bottom: .3em; }
.markdown-body code, .markdown-body pre { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
.markdown-body pre { background: #f6f8fa; padding: 16px; border-radius: 6px; overflow: auto; }
.markdown-body code { background: rgba(175,184,193,.2); padding: .2em .4em; border-radius: 6px; }
.markdown-body pre code { background: transparent; padding: 0; }
.markdown-body table { border-collapse: collapse; }
.markdown-body th, .markdown-body td { border: 1px solid #d0d7de; padding: 6px 13px; }
.markdown-body tr:nth-child(2n) { background: #f6f8fa; }
@media (max-width: 900px) {
  #app { grid-template-columns: 1fr; }
  #tree { height: 40vh; border-right: 0; border-bottom: 1px solid #d0d7de; }
}
</style>
</head>
<body>
<div id="app">
  <aside id="tree">
    <div id="status">Loading files...</div>
    <div id="tree-root"></div>
  </aside>
  <main id="viewer">
    <article class="markdown-body">
      <p>Select a Markdown file from the left.</p>
    </article>
  </main>
</div>
<script>
const statusEl = document.getElementById("status");
const treeRootEl = document.getElementById("tree-root");
const viewerEl = document.getElementById("viewer");
let activeBtn = null;

function create(tag, attrs = {}) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "text") {
      el.textContent = v;
    } else {
      el.setAttribute(k, v);
    }
  }
  return el;
}

async function loadMarkdown(path, btn) {
  if (activeBtn) activeBtn.classList.remove("active");
  activeBtn = btn || null;
  if (activeBtn) activeBtn.classList.add("active");
  statusEl.textContent = path;
  const resp = await fetch("/__docs__/md?path=" + encodeURIComponent(path));
  if (!resp.ok) {
    viewerEl.innerHTML = '<article class="markdown-body"><p>Unable to load file.</p></article>';
    return;
  }
  const html = await resp.text();
  viewerEl.innerHTML = '<article class="markdown-body">' + html + '</article>';
}

function renderNode(node, parent) {
  if (node.dir) {
    const details = create("details");
    const summary = create("summary", { text: node.name });
    details.appendChild(summary);
    const container = create("div", { class: "dir-children" });
    details.appendChild(container);
    parent.appendChild(details);
    for (const child of node.children || []) renderNode(child, container);
    return;
  }

  const btn = create("button", { class: "file", text: node.name, type: "button" });
  btn.addEventListener("click", () => loadMarkdown(node.path, btn));
  parent.appendChild(btn);
}

async function init() {
  const resp = await fetch("/__docs__/tree");
  if (!resp.ok) {
    statusEl.textContent = "Failed to load file tree.";
    return;
  }
  const root = await resp.json();
  statusEl.textContent = "Ready";
  treeRootEl.innerHTML = "";
  for (const child of root.children || []) renderNode(child, treeRootEl);
}

init();
</script>
</body>
</html>
"#;

#[derive(Clone, Debug)]
struct DocsNode {
    name: String,
    path: String,
    dir: bool,
    children: Vec<DocsNode>,
}

#[derive(Clone, Debug)]
struct IgnorePattern {
    pattern: String,
    anchored: bool,
}

#[derive(Clone, Debug, Default)]
struct IgnoreRules {
    patterns: Vec<IgnorePattern>,
}

fn file_size(n: u64) -> String {
    if n < 1024 {
        return format!("{n}");
    }
    let mut f = n as f64;
    let mut exp = 0usize;
    loop {
        f /= 1024.0;
        if f < 1024.0 || exp >= 3 {
            break;
        }
        exp += 1;
    }
    format!("{f:.1}{}", b"KMGT"[exp] as char)
}

// Ported from github.com/fvbommel/util sortorder/natsort.go (via Go version)
fn natural_cmp(s1: &[u8], s2: &[u8]) -> Ordering {
    let (mut i1, mut i2) = (0, 0);
    while i1 < s1.len() && i2 < s2.len() {
        let (c1, c2) = (s1[i1], s2[i2]);
        let (d1, d2) = (c1.is_ascii_digit(), c2.is_ascii_digit());
        match (d1, d2) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => match c1.cmp(&c2) {
                Ordering::Equal => {
                    i1 += 1;
                    i2 += 1;
                }
                ord => return ord,
            },
            (true, true) => {
                while i1 < s1.len() && s1[i1] == b'0' {
                    i1 += 1;
                }
                while i2 < s2.len() && s2[i2] == b'0' {
                    i2 += 1;
                }
                let (nz1, nz2) = (i1, i2);
                while i1 < s1.len() && s1[i1].is_ascii_digit() {
                    i1 += 1;
                }
                while i2 < s2.len() && s2[i2].is_ascii_digit() {
                    i2 += 1;
                }
                match (i1 - nz1).cmp(&(i2 - nz2)) {
                    Ordering::Equal => {}
                    ord => return ord,
                }
                match s1[nz1..i1].cmp(&s2[nz2..i2]) {
                    Ordering::Equal => {}
                    ord => return ord,
                }
                match nz1.cmp(&nz2) {
                    Ordering::Equal => {}
                    ord => return ord,
                }
            }
        }
    }
    s1.len().cmp(&s2.len())
}

fn percent_decode(s: &str) -> Cow<'_, str> {
    if !s.as_bytes().contains(&b'%') {
        return Cow::Borrowed(s);
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2]))
        {
            out.push((h << 4) | l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    Cow::Owned(
        String::from_utf8(out)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
    )
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'!'
            | b'\''
            | b'('
            | b')'
            | b'*' => out.push(byte as char),
            b => write!(out, "%{b:02X}").unwrap(),
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

fn normalize_into(base: &Path, raw: &str, out: &mut PathBuf) -> bool {
    out.as_mut_os_string().clear();
    out.push(base);
    let raw = raw.split('?').next().unwrap_or("/");
    for seg in raw.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.starts_with(base)
}

/// A request path names nothing when a component is missing or is a file
/// used as a directory.
fn is_missing(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory)
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" | "md" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wave",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        "zip" => "application/zip",
        "gz" | "tgz" => "application/gzip",
        _ => "application/octet-stream",
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

fn gfm_options() -> Options {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts
}

fn render_markdown_fragment(markdown: &str, html_out: &mut String) {
    html_out.clear();
    let parser = Parser::new_ext(markdown, gfm_options());
    html::push_html(html_out, parser);
}

fn render_markdown(markdown: &str, html_out: &mut String) {
    html_out.clear();
    html_out.push_str(MARKDOWN_PRELUDE);
    let events = mermaid_events(Parser::new_ext(markdown, gfm_options()));
    html::push_html(html_out, events.into_iter());
    html_out.push_str(MARKDOWN_POSTLUDE);
}

fn mermaid_events<'a>(parser: Parser<'a>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    let mut iter = parser.into_iter();
    let mut diagram_index = 0usize;
    while let Some(event) = iter.next() {
        let is_mermaid = matches!(
            &event,
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if lang.trim().eq_ignore_ascii_case("mermaid")
        );
        if !is_mermaid {
            out.push(event);
            continue;
        }
        let mut raw = vec![event];
        let mut code = String::new();
        for event in iter.by_ref() {
            match event {
                Event::Text(text) => {
                    code.push_str(&text);
                    raw.push(Event::Text(text));
                }
                end @ Event::End(TagEnd::CodeBlock) => {
                    raw.push(end);
                    break;
                }
                other => raw.push(other),
            }
        }
        diagram_index += 1;
        match mermaid_flowchart_svg(&code, &format!("srv-mermaid-{diagram_index}")) {
            Some(svg) => out.push(Event::Html(svg.into())),
            None => out.extend(raw),
        }
    }
    out
}

fn mermaid_flowchart_svg(code: &str, diagram_id: &str) -> Option<String> {
    use merman::svg;
    use std::sync::Arc;

    let policy = svg::TextMeasurementPolicy::uniform(svg::TextMeasurementProfile::new(
        svg::TextMeasurementProfileIdentity::new(
            svg::MeasurementProfileId::new("srv.svg-text").ok()?,
            env!("CARGO_PKG_VERSION"),
        )
        .ok()?,
        Arc::new(svg::DeterministicTextMeasurer {
            char_width_factor: 0.62,
            line_height_factor: 0.0,
        }),
    ));
    let environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(policy);
    let request = merman::SvgRequest {
        environment,
        options: svg::SvgRenderOptions {
            diagram_id: Some(diagram_id.to_owned()),
            ..Default::default()
        },
        ..Default::default()
    };
    match merman::Renderer::new().render(merman::RenderRequest::svg(
        code,
        merman::OperationControl::new(),
        request,
    )) {
        Ok(merman::RenderOutput::Svg(Some(output))) => Some(output.svg().to_owned()),
        _ => None,
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let (mut i, mut j) = (0usize, 0usize);
    let (mut star, mut match_j) = (None::<usize>, 0usize);

    while j < t.len() {
        if i < p.len() && (p[i] == b'?' || p[i] == t[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == b'*' {
            star = Some(i);
            i += 1;
            match_j = j;
        } else if let Some(star_i) = star {
            i = star_i + 1;
            match_j += 1;
            j = match_j;
        } else {
            return false;
        }
    }

    while i < p.len() && p[i] == b'*' {
        i += 1;
    }
    i == p.len()
}

fn read_ignore_rules(root: &Path) -> IgnoreRules {
    let path = root.join(".gitignore");
    let mut rules = IgnoreRules::default();
    let Ok(raw) = fs::read_to_string(path) else {
        return rules;
    };

    for line in raw.lines() {
        let mut line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        if let Some(stripped) = line.strip_prefix("./") {
            line = stripped;
        }

        let anchored = line.starts_with('/');
        if anchored {
            line = line.trim_start_matches('/');
        }

        line = line.trim_end_matches('/');
        if line.is_empty() {
            continue;
        }

        rules.patterns.push(IgnorePattern {
            pattern: line.to_string(),
            anchored,
        });
    }

    rules
}

fn is_gitignored_dir(rel: &str, name: &str, rules: &IgnoreRules) -> bool {
    for rule in &rules.patterns {
        let pat = rule.pattern.as_str();
        if rule.anchored {
            if glob_match(pat, rel) || rel.starts_with(&format!("{pat}/")) {
                return true;
            }
            continue;
        }

        if pat.contains('/') {
            if glob_match(pat, rel) || rel.ends_with(&format!("/{pat}")) {
                return true;
            }
            continue;
        }

        if glob_match(pat, name) {
            return true;
        }
    }
    false
}

fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn node_to_json(node: &DocsNode, out: &mut String) {
    out.push('{');
    out.push_str("\"name\":");
    json_string(&node.name, out);
    out.push(',');
    out.push_str("\"path\":");
    json_string(&node.path, out);
    out.push(',');
    out.push_str("\"dir\":");
    out.push_str(if node.dir { "true" } else { "false" });
    if node.dir {
        out.push(',');
        out.push_str("\"children\":[");
        for (idx, child) in node.children.iter().enumerate() {
            if idx > 0 {
                out.push(',');
            }
            node_to_json(child, out);
        }
        out.push(']');
    }
    out.push('}');
}

fn rel_to_web_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn walk_docs_dir(
    root: &Path,
    rel: &Path,
    ignore: &IgnoreRules,
    parallel: bool,
) -> io::Result<Option<DocsNode>> {
    let dir_path = if rel.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };

    let mut entries: Vec<_> = fs::read_dir(&dir_path)?.filter_map(|e| e.ok()).collect();
    entries.sort_by(|a, b| {
        let an = a.file_name().to_string_lossy().to_lowercase();
        let bn = b.file_name().to_string_lossy().to_lowercase();
        natural_cmp(an.as_bytes(), bn.as_bytes())
    });

    let mut file_nodes = Vec::new();
    let mut subdirs = Vec::new();

    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if name.starts_with('.') {
                continue;
            }
            let child_rel = rel.join(&name);
            let rel_str = rel_to_web_path(&child_rel);
            if is_gitignored_dir(&rel_str, &name, ignore) {
                continue;
            }
            subdirs.push((name, child_rel));
        } else if ft.is_file() {
            let fp = entry.path();
            if is_markdown(&fp) {
                let rel_path = rel_to_web_path(&rel.join(&name));
                file_nodes.push(DocsNode {
                    name,
                    path: rel_path,
                    dir: false,
                    children: Vec::new(),
                });
            }
        }
    }

    let mut dir_nodes = Vec::new();
    if parallel {
        std::thread::scope(|scope| {
            let mut jobs = Vec::new();
            for (_, child_rel) in &subdirs {
                jobs.push(scope.spawn(move || walk_docs_dir(root, child_rel, ignore, false)));
            }

            for job in jobs {
                let joined = match job.join() {
                    Ok(v) => v,
                    Err(_) => return Err(io::Error::other("docs tree worker thread panicked")),
                }?;
                if let Some(node) = joined {
                    dir_nodes.push(node);
                }
            }
            Ok::<(), io::Error>(())
        })?;
    } else {
        for (_, child_rel) in &subdirs {
            if let Some(node) = walk_docs_dir(root, child_rel, ignore, false)? {
                dir_nodes.push(node);
            }
        }
    }

    let name = if rel.as_os_str().is_empty() {
        "/".to_string()
    } else {
        rel.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string()
    };

    let mut children = Vec::with_capacity(dir_nodes.len() + file_nodes.len());
    children.extend(dir_nodes);
    children.extend(file_nodes);

    if !rel.as_os_str().is_empty() && children.is_empty() {
        return Ok(None);
    }

    Ok(Some(DocsNode {
        name,
        path: rel_to_web_path(rel),
        dir: true,
        children,
    }))
}

fn render_docs_tree_json(root: &Path, out: &mut String) -> io::Result<()> {
    let ignore = read_ignore_rules(root);
    let root_node = walk_docs_dir(root, Path::new(""), &ignore, true)?.unwrap_or(DocsNode {
        name: "/".to_string(),
        path: String::new(),
        dir: true,
        children: Vec::new(),
    });
    out.clear();
    node_to_json(&root_node, out);
    Ok(())
}

fn query_param(path_raw: &str, key: &str) -> Option<String> {
    let query = path_raw.split_once('?')?.1;
    for part in query.split('&') {
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        if k == key {
            return Some(percent_decode(v).into_owned());
        }
    }
    None
}

fn send_file(file: &File, mut sock: &TcpStream, len: u64) -> io::Result<()> {
    io::copy(&mut file.take(len), &mut sock)?;
    Ok(())
}

const EOCD_LEN: usize = 22;
const EOCD_TAIL: u64 = EOCD_LEN as u64 + 65_535;
const EOCD_SCAN_LIMIT: usize = 16;
const EOCD_SIG: u32 = 0x0605_4B50;
const CENTRAL_SIG: u32 = 0x0201_4B50;
const LOCAL_SIG: u32 = 0x0403_4B50;
const METHOD_STORED: u16 = 0;
const ZIP64_16: u16 = 0xFFFF;
const ZIP64_32: u32 = 0xFFFF_FFFF;

/// An `extension-onefile` polyglot: an HTML document with a stored ZIP
/// archive appended, so one file serves as both. Entries are read from
/// disk per request, mirroring single-file mode.
struct OnefileArchive {
    path: PathBuf,
    file_len: u64,
    entries: Vec<OnefileEntry>,
    by_name: HashMap<String, usize>,
    skipped: Vec<String>,
}

struct OnefileEntry {
    name: String,
    data_offset: u64,
    len: u64,
    mime: Option<String>,
}

fn file_read_exact_at(file: &mut File, offset: u64, buf: &mut [u8]) -> io::Result<()> {
    file.seek(SeekFrom::Start(offset))?;
    file.read_exact(buf)
}

fn u16_at(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}

fn u32_at(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(buf[at..at + 4].try_into().expect("u32 field"))
}

/// Detects a onefile polyglot. `Ok(None)` means the file is not one: a
/// plain HTML or plain ZIP file keeps the usual single-file behavior.
/// `Err` means an archive is present but unreadable.
fn onefile_detect(path: &Path, len: u64) -> Result<Option<OnefileArchive>, String> {
    if len < EOCD_LEN as u64 + 1 {
        return Ok(None);
    }
    let mut file = File::open(path).map_err(|e| format!("cannot open: {e}"))?;
    let tail_len = EOCD_TAIL.min(len);
    let tail_base = len - tail_len;
    let mut tail = vec![0u8; tail_len as usize];
    file_read_exact_at(&mut file, tail_base, &mut tail)
        .map_err(|e| format!("cannot read the file tail: {e}"))?;

    let mut candidates = Vec::new();
    if tail.len() >= EOCD_LEN {
        let mut i = tail.len() - EOCD_LEN;
        loop {
            if u32_at(&tail, i) == EOCD_SIG {
                candidates.push(tail_base + i as u64);
                if candidates.len() == EOCD_SCAN_LIMIT {
                    break;
                }
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
    }

    let mut first_err: Option<String> = None;
    for eocd_abs in candidates.into_iter().rev() {
        match parse_onefile_candidate(&mut file, path, len, eocd_abs) {
            Ok(Some(archive)) => return Ok(Some(archive)),
            Ok(None) => {}
            Err(reason) => {
                if first_err.is_none() {
                    first_err = Some(reason);
                }
            }
        }
    }
    match first_err {
        Some(reason) => Err(reason),
        None => Ok(None),
    }
}

fn parse_onefile_candidate(
    file: &mut File,
    path: &Path,
    len: u64,
    eocd_abs: u64,
) -> Result<Option<OnefileArchive>, String> {
    let mut eocd = [0u8; EOCD_LEN];
    file_read_exact_at(file, eocd_abs, &mut eocd).map_err(|e| format!("cannot read: {e}"))?;
    let disk = u16_at(&eocd, 4);
    let cd_disk = u16_at(&eocd, 6);
    let n_disk = u16_at(&eocd, 8);
    let n_total = u16_at(&eocd, 10);
    let cd_size = u64::from(u32_at(&eocd, 12));
    let cd_offset = u64::from(u32_at(&eocd, 16));
    if disk != 0 || cd_disk != 0 || n_disk != n_total {
        return Err("the embedded ZIP spans multiple disks".to_owned());
    }
    if n_total == 0 {
        return Ok(None);
    }
    if n_total == ZIP64_16
        || cd_size == u64::from(ZIP64_32)
        || cd_offset == u64::from(ZIP64_32)
    {
        return Err("the embedded ZIP uses ZIP64 fields".to_owned());
    }
    let cd_abs = eocd_abs
        .checked_sub(cd_size)
        .ok_or_else(|| "the embedded ZIP central directory is out of bounds".to_owned())?;
    if cd_size > 16 * 1024 * 1024 {
        return Err("the embedded ZIP central directory is unreasonably large".to_owned());
    }

    // The onefile format bakes file-absolute offsets into the archive; a
    // conventional prefixed ZIP keeps offsets relative to the archive start.
    let (file_absolute, rel_prefix) = if cd_abs == cd_offset {
        (true, 0)
    } else if cd_abs > cd_offset {
        (false, cd_abs - cd_offset)
    } else {
        return Err("the embedded ZIP central directory offset is out of bounds".to_owned());
    };

    let mut cd = vec![0u8; cd_size as usize];
    file_read_exact_at(file, cd_abs, &mut cd)
        .map_err(|e| format!("cannot read the central directory: {e}"))?;

    struct RawEntry {
        name: String,
        compressed: u64,
        uncompressed: u64,
        local_offset: u64,
    }

    let mut raw: Vec<RawEntry> = Vec::with_capacity(n_total as usize);
    let mut skipped: Vec<String> = Vec::new();
    let mut min_local = u64::MAX;
    let mut off = 0usize;
    for i in 0..n_total {
        if off + 46 > cd.len() {
            return Err(format!("central entry {i} is out of bounds"));
        }
        if u32_at(&cd, off) != CENTRAL_SIG {
            return Err(format!("central entry {i} has a bad signature"));
        }
        let flags = u16_at(&cd, off + 8);
        let method = u16_at(&cd, off + 10);
        let compressed = u32_at(&cd, off + 20);
        let uncompressed = u32_at(&cd, off + 24);
        let name_len = u64::from(u16_at(&cd, off + 28));
        let extra_len = usize::from(u16_at(&cd, off + 30));
        let comment_len = usize::from(u16_at(&cd, off + 32));
        let local_offset = u64::from(u32_at(&cd, off + 42));
        let name_start = off + 46;
        let name_end = name_start
            .checked_add(name_len as usize)
            .filter(|end| *end <= cd.len())
            .ok_or_else(|| format!("central entry {i} is out of bounds"))?;
        off = name_end
            .checked_add(extra_len)
            .and_then(|end| end.checked_add(comment_len))
            .filter(|end| *end <= cd.len())
            .ok_or_else(|| format!("central entry {i} is out of bounds"))?;
        let name = String::from_utf8_lossy(&cd[name_start..name_end]).into_owned();
        min_local = min_local.min(local_offset);
        if compressed == ZIP64_32 || uncompressed == ZIP64_32 {
            skipped.push(name);
            continue;
        }
        if flags & 0x1 != 0 || method != METHOD_STORED {
            skipped.push(name);
            continue;
        }
        raw.push(RawEntry {
            name,
            compressed: u64::from(compressed),
            uncompressed: u64::from(uncompressed),
            local_offset,
        });
    }

    let prefix_len = if file_absolute {
        if min_local == u64::MAX {
            return Err("the embedded ZIP has no entries".to_owned());
        }
        min_local
    } else {
        rel_prefix
    };
    if prefix_len == 0 {
        return Ok(None);
    }
    if prefix_len > cd_abs {
        return Err("the embedded ZIP entries start beyond the central directory".to_owned());
    }
    if !prefix_looks_like_html(file, prefix_len)? {
        return Ok(None);
    }
    let resolve = |local: u64| {
        if file_absolute { local } else { prefix_len + local }
    };

    let mut entries: Vec<OnefileEntry> = Vec::with_capacity(raw.len());
    let mut by_name: HashMap<String, usize> = HashMap::with_capacity(raw.len());
    for entry in raw {
        let abs = resolve(entry.local_offset);
        let mut local = [0u8; 30];
        file_read_exact_at(file, abs, &mut local)
            .map_err(|e| format!("cannot read entry {}: {e}", entry.name))?;
        if u32_at(&local, 0) != LOCAL_SIG {
            return Err(format!("entry {} has a bad local header", entry.name));
        }
        let name_len = u64::from(u16_at(&local, 26));
        let extra_len = u64::from(u16_at(&local, 28));
        let data_offset = abs + 30 + name_len + extra_len;
        if entry.compressed != entry.uncompressed {
            return Err(format!(
                "entry {} is stored but its sizes differ",
                entry.name
            ));
        }
        if data_offset + entry.compressed > cd_abs {
            return Err(format!("entry {} overruns the archive", entry.name));
        }
        if by_name.insert(entry.name.clone(), entries.len()).is_some() {
            return Err(format!("entry {} appears twice", entry.name));
        }
        entries.push(OnefileEntry {
            name: entry.name,
            data_offset,
            len: entry.uncompressed,
            mime: None,
        });
    }

    if let Some(idx) = entries.iter().position(|e| e.name == "metadata.json")
        && entries[idx].len <= 8 * 1024 * 1024
    {
        let mut bytes = vec![0u8; entries[idx].len as usize];
        if file_read_exact_at(file, entries[idx].data_offset, &mut bytes).is_ok()
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && let Some(list) = value.get("entries").and_then(serde_json::Value::as_array)
        {
            for item in list {
                let (Some(name), Some(mime)) = (
                    item.get("name").and_then(serde_json::Value::as_str),
                    item.get("mime").and_then(serde_json::Value::as_str),
                ) else {
                    continue;
                };
                if let Some(entry) = entries.iter_mut().find(|entry| entry.name == name) {
                    entry.mime = Some(mime.to_owned());
                }
            }
        }
    }

    Ok(Some(OnefileArchive {
        path: path.to_path_buf(),
        file_len: len,
        entries,
        by_name,
        skipped,
    }))
}

fn prefix_looks_like_html(file: &mut File, prefix_len: u64) -> Result<bool, String> {
    let mut head = [0u8; 512];
    let n = head.len().min(prefix_len as usize);
    file_read_exact_at(file, 0, &mut head[..n])
        .map_err(|e| format!("cannot read the file start: {e}"))?;
    let mut bytes = &head[..n];
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        bytes = rest;
    }
    let first = bytes.iter().find(|b| !b.is_ascii_whitespace());
    Ok(first == Some(&b'<'))
}


fn write_headers(
    mut sock: &TcpStream,
    status: u16,
    reason: &str,
    ct: &str,
    cl: u64,
) -> io::Result<()> {
    let mut hdr = [0u8; 512];
    let n = {
        let mut c = io::Cursor::new(&mut hdr[..]);
        write!(
            c,
            "HTTP/1.1 {status} {reason}\r\n\
             Content-Type: {ct}\r\n\
             Content-Length: {cl}\r\n\
             Cache-Control: no-store\r\n\
             Connection: close\r\n\
             \r\n"
        )?;
        c.position() as usize
    };
    sock.write_all(&hdr[..n])
}

fn write_error(mut sock: &TcpStream, code: u16, msg: &str) -> io::Result<()> {
    let mut buf = [0u8; 512];
    let body_len = 4 + msg.len();
    let n = {
        let mut c = io::Cursor::new(&mut buf[..]);
        write!(
            c,
            "HTTP/1.1 {code} {msg}\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {body_len}\r\n\
             Cache-Control: no-store\r\n\
             Connection: close\r\n\
             \r\n\
             {code} {msg}"
        )?;
        c.position() as usize
    };
    sock.write_all(&buf[..n])
}

fn write_body(sock: &TcpStream, ct: &str, body: &[u8]) -> io::Result<()> {
    write_headers(sock, 200, "OK", ct, body.len() as u64)?;
    let mut w: &TcpStream = sock;
    w.write_all(body)
}

fn serve_file(sock: &TcpStream, path: &Path, len: u64, content_type: &str) -> io::Result<()> {
    let file = File::open(path)?;
    write_headers(sock, 200, "OK", content_type, len)?;
    send_file(&file, sock, len)
}

fn serve_markdown(sock: &TcpStream, path: &Path, html_out: &mut String) -> io::Result<()> {
    let markdown = fs::read(path)?;
    let markdown = String::from_utf8_lossy(&markdown);
    render_markdown(&markdown, html_out);
    write_headers(
        sock,
        200,
        "OK",
        "text/html; charset=utf-8",
        html_out.len() as u64,
    )?;
    let mut w: &TcpStream = sock;
    w.write_all(html_out.as_bytes())
}

fn serve_markdown_fragment(sock: &TcpStream, path: &Path, html_out: &mut String) -> io::Result<()> {
    let markdown = fs::read(path)?;
    let markdown = String::from_utf8_lossy(&markdown);
    render_markdown_fragment(&markdown, html_out);
    write_body(sock, "text/html; charset=utf-8", html_out.as_bytes())
}

fn render_listing(dir: &Path, html: &mut String) -> io::Result<()> {
    html.clear();
    html.push_str(LISTING_PRELUDE);

    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| {
            let key = e.file_name().to_string_lossy().to_lowercase();
            (key, e)
        })
        .collect();
    entries.sort_by(|(a, _), (b, _)| natural_cmp(a.as_bytes(), b.as_bytes()));

    for (_, entry) in &entries {
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let ft = meta.file_type();
        let text = html_escape(&name);
        if ft.is_dir() {
            let enc = percent_encode(&name);
            write!(html, "<tr><td><a href=\"{enc}/\">{text}/</a></td></tr>").unwrap();
        } else if ft.is_file() {
            let enc = percent_encode(&name);
            let sz = file_size(meta.len());
            write!(
                html,
                "<tr><td><a href=\"{enc}\">{text}</a></td><td>{sz}</td></tr>"
            )
            .unwrap();
        } else {
            write!(
                html,
                "<tr><td><p style=\"color: #777\">{text}</p></td></tr>"
            )
            .unwrap();
        }
    }
    html.push_str("</table>");
    Ok(())
}

/// Browsers send about 20 headers; more than this answers 431 like an
/// oversized request.
const MAX_HEADERS: usize = 100;

/// How long a connection may take to send its request, or leave a response
/// unread, before srv drops it.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Connections served at once; one more is answered 503 and closed.
const MAX_CONNECTIONS: usize = 64;

/// Counts a connection against MAX_CONNECTIONS until it is dropped.
struct ConnectionSlot(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Clone)]
struct Server {
    dir: PathBuf,
    quiet: bool,
    docs_mode: bool,
    single_file: Option<String>,
    onefile: Option<std::sync::Arc<OnefileArchive>>,
    fp: PathBuf,
    html: String,
}

impl Server {
    fn handle(&mut self, mut stream: TcpStream) -> io::Result<()> {
        let mut buf = [0u8; 8192];
        let mut pos = 0usize;
        stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
        stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;

        loop {
            if pos == buf.len() {
                return write_error(&stream, 431, "Request Header Fields Too Large");
            }
            let n = stream.read(&mut buf[pos..])?;
            if n == 0 {
                return Ok(());
            }
            pos += n;
            let mut hdrs = [httparse::EMPTY_HEADER; MAX_HEADERS];
            let mut req = httparse::Request::new(&mut hdrs);
            match req.parse(&buf[..pos]) {
                Ok(httparse::Status::Complete(_)) => {
                    return self.dispatch(
                        &stream,
                        req.method.unwrap_or(""),
                        req.path.unwrap_or("/"),
                    );
                }
                Ok(httparse::Status::Partial) => continue,
                Err(httparse::Error::TooManyHeaders) => {
                    return write_error(&stream, 431, "Request Header Fields Too Large");
                }
                Err(_) => return write_error(&stream, 400, "Bad Request"),
            }
        }
    }

    fn dispatch(&mut self, sock: &TcpStream, method: &str, path_raw: &str) -> io::Result<()> {
        if !self.quiet
            && let Ok(peer) = sock.peer_addr()
        {
            eprintln!("\t{peer}: {method} {path_raw}");
        }

        if method != "GET" {
            return write_error(sock, 405, "Method Not Allowed");
        }

        if self.docs_mode {
            return self.dispatch_docs(sock, path_raw);
        }

        if let Some(archive) = self.onefile.clone() {
            return self.dispatch_onefile(sock, path_raw, &archive);
        }

        let owned;
        let path_raw = if let Some(ref name) = self.single_file {
            owned = name.clone();
            owned.as_str()
        } else {
            path_raw
        };

        let decoded = percent_decode(path_raw);
        if !normalize_into(&self.dir, &decoded, &mut self.fp) {
            return write_error(sock, 403, "Forbidden");
        }

        let meta = match fs::symlink_metadata(self.fp.as_path()) {
            Ok(m) => m,
            Err(e) if is_missing(&e) => {
                return write_error(sock, 404, "Not Found");
            }
            Err(e) => return Err(e),
        };

        let ft = meta.file_type();
        if ft.is_dir() {
            self.fp.push("index.html");
            if let Ok(idx_meta) = fs::symlink_metadata(self.fp.as_path()) {
                if idx_meta.file_type().is_symlink() {
                    self.fp.pop();
                    return write_error(sock, 403, "Forbidden: symlinks not served");
                }
                if idx_meta.is_file() {
                    let result =
                        serve_file(sock, &self.fp, idx_meta.len(), "text/html; charset=utf-8");
                    self.fp.pop();
                    return result;
                }
            }
            self.fp.pop();

            render_listing(&self.fp, &mut self.html)?;
            write_headers(
                sock,
                200,
                "OK",
                "text/html; charset=utf-8",
                self.html.len() as u64,
            )?;
            let mut w: &TcpStream = sock;
            w.write_all(self.html.as_bytes())?;
        } else if ft.is_file() {
            if is_markdown(&self.fp) {
                serve_markdown(sock, &self.fp, &mut self.html)?;
            } else {
                serve_file(sock, &self.fp, meta.len(), mime(&self.fp))?;
            }
        } else if ft.is_symlink() {
            write_error(sock, 403, "Forbidden: symlinks not served")?;
        } else {
            write_error(sock, 403, "Forbidden: not a regular file or directory")?;
        }

        Ok(())
    }

    fn dispatch_onefile(
        &mut self,
        sock: &TcpStream,
        path_raw: &str,
        archive: &OnefileArchive,
    ) -> io::Result<()> {
        let path = path_raw.split('?').next().unwrap_or("/");
        let decoded = percent_decode(path);
        let mut key = String::with_capacity(decoded.len());
        for segment in decoded.split('/') {
            match segment {
                "" | "." => {}
                ".." => return write_error(sock, 403, "Forbidden"),
                s => {
                    if !key.is_empty() {
                        key.push('/');
                    }
                    key.push_str(s);
                }
            }
        }
        if key.is_empty() {
            // The onefile loader fetches the page's own URL and parses the
            // appended ZIP, so `/` must serve the file whole.
            return serve_file(sock, &archive.path, archive.file_len, "text/html; charset=utf-8");
        }
        let Some(&index) = archive.by_name.get(key.as_str()) else {
            return write_error(sock, 404, "Not Found");
        };
        let entry = &archive.entries[index];
        let ct = entry
            .mime
            .as_deref()
            .unwrap_or_else(|| mime(Path::new(&entry.name)));
        let mut file = File::open(&archive.path)?;
        file.seek(SeekFrom::Start(entry.data_offset))?;
        write_headers(sock, 200, "OK", ct, entry.len)?;
        send_file(&file, sock, entry.len)
    }

    fn dispatch_docs(&mut self, sock: &TcpStream, path_raw: &str) -> io::Result<()> {
        let path = path_raw.split('?').next().unwrap_or("/");
        match path {
            "/__docs__/tree" => self.serve_docs_tree(sock),
            "/__docs__/md" => self.serve_docs_markdown(sock, path_raw),
            _ => write_body(sock, "text/html; charset=utf-8", DOCS_PAGE.as_bytes()),
        }
    }

    fn serve_docs_tree(&mut self, sock: &TcpStream) -> io::Result<()> {
        render_docs_tree_json(&self.dir, &mut self.html)?;
        write_body(
            sock,
            "application/json; charset=utf-8",
            self.html.as_bytes(),
        )
    }

    fn serve_docs_markdown(&mut self, sock: &TcpStream, path_raw: &str) -> io::Result<()> {
        let Some(rel) = query_param(path_raw, "path") else {
            return write_error(sock, 400, "Bad Request: missing path parameter");
        };

        let rel = rel.trim_start_matches('/');
        if rel.is_empty() {
            return write_error(sock, 400, "Bad Request: empty path parameter");
        }
        let req_path = format!("/{rel}");
        if !normalize_into(&self.dir, &req_path, &mut self.fp) {
            return write_error(sock, 403, "Forbidden");
        }
        if !is_markdown(&self.fp) {
            return write_error(sock, 400, "Bad Request: path must be a markdown file");
        }
        let meta = match fs::symlink_metadata(&self.fp) {
            Ok(m) => m,
            Err(e) if is_missing(&e) => {
                return write_error(sock, 404, "Not Found");
            }
            Err(e) => return Err(e),
        };
        if meta.file_type().is_symlink() {
            return write_error(sock, 403, "Forbidden: symlinks not served");
        }
        if !meta.is_file() {
            return write_error(sock, 404, "Not Found");
        }

        serve_markdown_fragment(sock, &self.fp, &mut self.html)
    }
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1);
}

const OWNED_TOOL_FOOTER: &str = "Maintained by Joshua Li: https://github.com/laputa-systems/srv\n\
Bugs and features: TODO.md in that repository (follow its AGENTS.md), or its issue tracker.\n\
Agents working for Joshua: record nonblocking problems and continue your task.\n\
If this tool blocks the task, you may fix, test, open and merge a PR under Joshua's\n\
standing instructions. Release only when needed to deliver the verified fix.\n\
Procedure: https://github.com/joshuarli/usr/blob/main/dotfiles/base/.agents/instructions/owned-tools.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OptionAction {
    Quiet,
    Docs,
    Port,
    Bind,
    Version,
    Help,
}

const DEFAULT_PORT: &str = "8000";
const DEFAULT_BIND: &str = "127.0.0.1";
const DEFAULT_PATH: &str = ".";

struct CliOption {
    names: &'static [&'static str],
    value: Option<&'static str>,
    description: &'static str,
    default: Option<&'static str>,
    action: OptionAction,
}

const CLI_OPTIONS: &[CliOption] = &[
    CliOption { names: &["-q"], value: None, description: "disable request logs", default: None, action: OptionAction::Quiet },
    CliOption { names: &["--docs"], value: None, description: "launch docs browser for Markdown files", default: None, action: OptionAction::Docs },
    CliOption { names: &["-p"], value: Some("port"), description: "port to listen on", default: Some(DEFAULT_PORT), action: OptionAction::Port },
    CliOption { names: &["-b"], value: Some("address"), description: "bind address", default: Some(DEFAULT_BIND), action: OptionAction::Bind },
    CliOption { names: &["-V", "--version"], value: None, description: "print version and exit", default: None, action: OptionAction::Version },
    CliOption { names: &["-h", "--help"], value: None, description: "print this help and exit", default: None, action: OptionAction::Help },
];

const CLI_EXAMPLES: &[&str] = &["srv ./public", "srv -p 8080 -b 127.0.0.1 ./public", "srv --docs ./docs"];

fn help() -> String {
    let mut text = String::from("usage: srv");
    for option in CLI_OPTIONS.iter().filter(|option| !matches!(option.action, OptionAction::Help | OptionAction::Version)) {
        text.push_str(&format!(" [{}", option.names[0]));
        if let Some(value) = option.value {
            text.push_str(&format!(" {value}"));
        }
        text.push(']');
    }
    text.push_str(&format!(" [path]\n\nServe a file or directory over HTTP.\n\nArguments:\n  path  file or directory to serve (default: {DEFAULT_PATH})\n\nOptions:\n"));
    for option in CLI_OPTIONS {
        let mut names = option.names.join(", ");
        if let Some(value) = option.value {
            names.push(' ');
            names.push_str(value);
        }
        text.push_str(&format!("  {names:<15} {}", option.description));
        if let Some(default) = option.default {
            text.push_str(&format!(" (default: {default})"));
        }
        text.push('\n');
    }
    text.push_str("\nExamples:\n");
    for example in CLI_EXAMPLES {
        text.push_str(&format!("  {example}\n"));
    }
    text.push_str(&format!("\n{OWNED_TOOL_FOOTER}\n"));
    text
}

#[derive(Debug, PartialEq, Eq)]
struct CliArgs {
    quiet: bool,
    docs_mode: bool,
    port: String,
    bind: String,
    dir: String,
}

#[derive(Debug, PartialEq, Eq)]
enum CliResult {
    Serve(CliArgs),
    Help,
    Version,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<CliResult, String> {
    let mut parsed = CliArgs {
        quiet: false,
        docs_mode: false,
        port: String::from(DEFAULT_PORT),
        bind: String::from(DEFAULT_BIND),
        dir: String::from(DEFAULT_PATH),
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let Some(option) = CLI_OPTIONS.iter().find(|option| option.names.contains(&arg.as_str())) else {
            parsed.dir = arg;
            continue;
        };
        match option.action {
            OptionAction::Quiet => parsed.quiet = true,
            OptionAction::Docs => parsed.docs_mode = true,
            OptionAction::Port => parsed.port = args.next().ok_or_else(|| format!("{arg} requires a value"))?,
            OptionAction::Bind => parsed.bind = args.next().ok_or_else(|| format!("{arg} requires a value"))?,
            OptionAction::Version => return Ok(CliResult::Version),
            OptionAction::Help => return Ok(CliResult::Help),
        }
    }
    Ok(CliResult::Serve(parsed))
}

fn main() {
    let CliArgs { quiet, docs_mode, port, bind, dir } = match parse_args(env::args().skip(1)) {
        Ok(CliResult::Serve(args)) => args,
        Ok(CliResult::Help) => {
            print!("{}", help());
            return;
        }
        Ok(CliResult::Version) => {
            println!("srv {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Err(error) => die(&error),
    };

    let canonical = match fs::canonicalize(&dir) {
        Ok(p) => p,
        Err(e) => die(&format!("{dir}: {e}")),
    };

    let (dir, single_file, onefile, canonical_display) = if canonical.is_file() {
        let display = canonical.display().to_string();
        let name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| die("file has no valid name"))
            .to_owned();
        let len = fs::metadata(&canonical)
            .unwrap_or_else(|e| die(&format!("{}: {e}", canonical.display())))
            .len();
        let onefile = match onefile_detect(&canonical, len) {
            Ok(found) => found,
            Err(reason) => {
                eprintln!("\twarning: {display}: {reason}; serving the file as-is");
                None
            }
        };
        let parent = canonical.parent().unwrap_or(&canonical).to_path_buf();
        (parent, Some(format!("/{name}")), onefile, display)
    } else if canonical.is_dir() {
        (canonical, None, None, String::new())
    } else {
        die(&format!(
            "{} is not a file or directory",
            canonical.display()
        ));
    };
    let onefile = onefile.map(std::sync::Arc::new);
    if let Some(archive) = &onefile
        && !archive.skipped.is_empty()
    {
        eprintln!(
            "\twarning: {canonical_display}: {} archive entr{} not served (unsupported compression or encryption): {}",
            archive.skipped.len(),
            if archive.skipped.len() == 1 { "y is" } else { "ies are" },
            archive.skipped.join(", ")
        );
    }

    let requested = format!("{bind}:{port}");
    let listener = TcpListener::bind(&requested)
        .unwrap_or_else(|e| die(&format!("failed to bind {requested}: {e}")));
    let addr = listener
        .local_addr()
        .map_or(requested, |bound| bound.to_string());

    if let Some(ref name) = single_file {
        if let Some(archive) = &onefile {
            eprintln!(
                "\tServing {}{} (onefile archive, {} entries) over HTTP on {addr}",
                dir.display(),
                name,
                archive.entries.len()
            );
        } else {
            eprintln!("\tServing {}{} over HTTP on {addr}", dir.display(), name);
        }
    } else {
        eprintln!("\tServing {} over HTTP on {addr}", dir.display());
    }

    let srv = Server {
        dir,
        quiet,
        docs_mode,
        single_file,
        onefile,
        fp: PathBuf::new(),
        html: String::new(),
    };

    let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if active.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= MAX_CONNECTIONS {
                    active.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    let _ = write_error(&s, 503, "Service Unavailable");
                    continue;
                }
                let slot = ConnectionSlot(active.clone());
                let mut connection = srv.clone();
                let spawned = std::thread::Builder::new().spawn(move || {
                    let _slot = slot;
                    if let Err(e) = connection.handle(s) {
                        eprintln!("error: {e}");
                    }
                });
                if let Err(e) = spawned {
                    eprintln!("error: cannot start a connection thread: {e}");
                }
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn help_examples_parse_with_the_real_parser() {
        let expected = [
            (false, "8000", "127.0.0.1", "./public"),
            (false, "8080", "127.0.0.1", "./public"),
            (true, "8000", "127.0.0.1", "./docs"),
        ];
        assert_eq!(super::CLI_EXAMPLES.len(), expected.len());
        for (example, (docs_mode, port, bind, dir)) in super::CLI_EXAMPLES.iter().zip(expected) {
            let mut words = example.split_whitespace();
            assert_eq!(words.next(), Some("srv"));
            let parsed = super::parse_args(words.map(str::to_owned)).unwrap();
            assert_eq!(parsed, super::CliResult::Serve(super::CliArgs {
                quiet: false, docs_mode, port: port.into(), bind: bind.into(), dir: dir.into(),
            }));
        }
    }

    #[test]
    fn bare_invocation_keeps_serving_defaults() {
        assert_eq!(super::parse_args(Vec::new()).unwrap(), super::CliResult::Serve(super::CliArgs {
            quiet: false, docs_mode: false, port: "8000".into(), bind: "127.0.0.1".into(), dir: ".".into(),
        }));
    }

    #[test]
    fn cli_option_names_are_unique() {
        let mut names = std::collections::HashSet::new();
        for option in super::CLI_OPTIONS {
            for name in option.names {
                assert!(names.insert(name), "duplicate option {name}");
            }
        }
    }

    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::os::unix::fs::symlink;

    #[test]
    fn file_size_bytes() {
        assert_eq!(file_size(0), "0");
        assert_eq!(file_size(1), "1");
        assert_eq!(file_size(1023), "1023");
    }

    #[test]
    fn file_size_kilo() {
        assert_eq!(file_size(1024), "1.0K");
        assert_eq!(file_size(1536), "1.5K");
        assert_eq!(file_size(1024 * 1023), "1023.0K");
    }

    #[test]
    fn file_size_mega() {
        assert_eq!(file_size(1024 * 1024), "1.0M");
        assert_eq!(file_size(1024 * 1024 * 5 + 1024 * 512), "5.5M");
    }

    #[test]
    fn file_size_giga() {
        assert_eq!(file_size(1024 * 1024 * 1024), "1.0G");
    }

    #[test]
    fn file_size_tera() {
        assert_eq!(file_size(1024u64 * 1024 * 1024 * 1024), "1.0T");
    }

    #[test]
    fn natural_cmp_orders_both_directions() {
        for (a, b, expected) in [
            ("a", "b", Ordering::Less),
            ("a", "a", Ordering::Equal),
            ("file2", "file10", Ordering::Less),
            ("file1", "file2", Ordering::Less),
            ("file01", "file001", Ordering::Less),
            ("file1", "file01", Ordering::Less),
            ("file0", "file00", Ordering::Less),
            ("x007y", "x7y", Ordering::Greater),
            ("a10b2", "a10b10", Ordering::Less),
            ("1abc", "abc", Ordering::Less),
            ("file", "file1", Ordering::Less),
            ("", "a", Ordering::Less),
            ("", "", Ordering::Equal),
        ] {
            assert_eq!(natural_cmp(a.as_bytes(), b.as_bytes()), expected, "{a} vs {b}");
            assert_eq!(natural_cmp(b.as_bytes(), a.as_bytes()), expected.reverse(), "{b} vs {a}");
        }
    }

    #[test]
    fn percent_decode_passthrough() {
        assert_eq!(percent_decode("hello").as_ref(), "hello");
    }

    #[test]
    fn percent_decode_passthrough_borrows() {
        assert!(matches!(percent_decode("hello"), Cow::Borrowed(_)));
    }

    #[test]
    fn percent_decode_space() {
        assert_eq!(percent_decode("hello%20world").as_ref(), "hello world");
    }

    #[test]
    fn percent_decode_slash() {
        assert_eq!(percent_decode("%2Fetc%2Fpasswd").as_ref(), "/etc/passwd");
    }

    #[test]
    fn percent_decode_invalid_seq() {
        assert_eq!(percent_decode("100%").as_ref(), "100%");
        assert_eq!(percent_decode("%ZZ").as_ref(), "%ZZ");
        assert_eq!(percent_decode("%0").as_ref(), "%0");
    }

    #[test]
    fn percent_encode_unreserved() {
        assert_eq!(percent_encode("hello"), "hello");
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn percent_encode_special() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("a/b"), "a%2Fb");
        assert_eq!(percent_encode("a@b"), "a%40b");
    }

    #[test]
    fn percent_roundtrip() {
        let input = "file with spaces & (parens)";
        assert_eq!(percent_decode(&percent_encode(input)).as_ref(), input);
    }

    fn normalize_test(base: &str, raw: &str) -> Option<PathBuf> {
        let base = Path::new(base);
        let mut fp = PathBuf::new();
        if normalize_into(base, raw, &mut fp) {
            Some(fp)
        } else {
            None
        }
    }

    #[test]
    fn normalize_root() {
        assert_eq!(normalize_test("/srv", "/"), Some(PathBuf::from("/srv")));
    }

    #[test]
    fn normalize_subpath() {
        assert_eq!(
            normalize_test("/srv", "/foo/bar"),
            Some(PathBuf::from("/srv/foo/bar"))
        );
    }

    #[test]
    fn normalize_traversal_basic() {
        assert_eq!(normalize_test("/srv/public", "/../../../etc/passwd"), None);
    }

    #[test]
    fn normalize_traversal_partial() {
        assert_eq!(normalize_test("/srv/public", "/foo/../../etc/passwd"), None);
    }

    #[test]
    fn normalize_traversal_deep() {
        assert_eq!(
            normalize_test("/srv/public", "/foo/bar/../../../etc/passwd"),
            None
        );
    }

    #[test]
    fn normalize_benign_parent() {
        assert_eq!(
            normalize_test("/srv", "/foo/../bar"),
            Some(PathBuf::from("/srv/bar"))
        );
    }

    #[test]
    fn normalize_dot_segments() {
        assert_eq!(
            normalize_test("/srv", "/./foo/./bar"),
            Some(PathBuf::from("/srv/foo/bar"))
        );
    }

    #[test]
    fn normalize_strips_query() {
        assert_eq!(
            normalize_test("/srv", "/foo?v=123"),
            Some(PathBuf::from("/srv/foo"))
        );
    }

    #[test]
    fn normalize_reuses_buffer() {
        let base = Path::new("/srv");
        let mut fp = PathBuf::new();
        normalize_into(base, "/first", &mut fp);
        let ptr1 = fp.as_os_str().as_encoded_bytes().as_ptr();
        normalize_into(base, "/second", &mut fp);
        let ptr2 = fp.as_os_str().as_encoded_bytes().as_ptr();
        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn mime_extension_contract() {
        let cases = [
            ("html", "text/html; charset=utf-8"), ("htm", "text/html; charset=utf-8"),
            ("css", "text/css"), ("js", "text/javascript"),
            ("mjs", "text/javascript"), ("json", "application/json"),
            ("xml", "application/xml"), ("txt", "text/plain; charset=utf-8"),
            ("md", "text/plain; charset=utf-8"), ("png", "image/png"),
            ("jpg", "image/jpeg"), ("jpeg", "image/jpeg"), ("gif", "image/gif"),
            ("svg", "image/svg+xml"), ("ico", "image/x-icon"), ("webp", "image/webp"),
            ("mp4", "video/mp4"), ("webm", "video/webm"), ("mp3", "audio/mpeg"),
            ("ogg", "audio/ogg"), ("wav", "audio/wave"), ("woff", "font/woff"),
            ("woff2", "font/woff2"), ("pdf", "application/pdf"),
            ("wasm", "application/wasm"), ("zip", "application/zip"),
            ("gz", "application/gzip"), ("tgz", "application/gzip"),
        ];
        for (extension, expected) in cases {
            assert_eq!(mime(Path::new(&format!("file.{extension}"))), expected, "{extension}");
        }
    }

    #[test]
    fn mime_known() {
        assert_eq!(mime(Path::new("f.html")), "text/html; charset=utf-8");
        assert_eq!(mime(Path::new("f.css")), "text/css");
        assert_eq!(mime(Path::new("f.js")), "text/javascript");
        assert_eq!(mime(Path::new("f.png")), "image/png");
        assert_eq!(mime(Path::new("f.pdf")), "application/pdf");
    }

    #[test]
    fn mime_unknown() {
        assert_eq!(mime(Path::new("f.xyz")), "application/octet-stream");
        assert_eq!(mime(Path::new("noext")), "application/octet-stream");
    }

    #[test]
    fn markdown_extension_detection() {
        assert!(is_markdown(Path::new("README.md")));
        assert!(is_markdown(Path::new("README.Markdown")));
        assert!(!is_markdown(Path::new("README.txt")));
        assert!(!is_markdown(Path::new("README")));
    }

    fn render_page(markdown: &str) -> String {
        let mut out = String::new();
        render_markdown(markdown, &mut out);
        out
    }

    #[test]
    fn markdown_mermaid_flowchart_renders_inline_svg() {
        let html = render_page(
            "```mermaid\nflowchart TD\n  start[\"Parent agent\"] --> tier{\"Which tier?\"}\n  tier -->|expert| harness{\"Harness?\"}\n  harness -->|OpenCode| oc[\"OpenCode native expert agent<br/>effort max\"]\n  other --> start\n```\n",
        );
        assert!(html.contains("<svg"), "{html}");
        let view_box = html.split("viewBox=\"").nth(1).unwrap().split('"').next().unwrap();
        assert_eq!(view_box.split_ascii_whitespace().nth(3), Some("798"));
        assert!(html.contains("Parent agent"), "{html}");
        assert!(!html.contains("language-mermaid"), "{html}");
    }

    #[test]
    fn markdown_mermaid_diagrams_get_unique_scoped_ids() {
        let html = render_page(
            "```mermaid\nflowchart TD\n  a --> b\n```\n\n```mermaid\nflowchart TD\n  c --> d\n```\n",
        );
        assert!(html.contains("srv-mermaid-1"), "{html}");
        assert!(html.contains("srv-mermaid-2"), "{html}");
    }

    #[test]
    fn markdown_mermaid_unsupported_type_falls_back_to_code() {
        let html = render_page("```mermaid\nsequenceDiagram\n  Alice->>Bob: hi\n```\n");
        assert!(html.contains("language-mermaid"), "{html}");
        assert!(!html.contains("<svg"), "{html}");
    }

    #[test]
    fn markdown_mermaid_malformed_diagram_falls_back_to_code() {
        let html = render_page("```mermaid\nflowchart TD\n  broken[unclosed\n```\n");
        assert!(html.contains("language-mermaid"), "{html}");
        assert!(!html.contains("<svg"), "{html}");
    }

    struct TestServer {
        addr: std::net::SocketAddr,
        _dir: tempfile::TempDir,
        shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn new() -> Self {
            Self::new_with_docs(false)
        }

        fn new_docs() -> Self {
            Self::new_with_docs(true)
        }

        fn new_with_docs(docs_mode: bool) -> Self {
            let dir = tempfile::tempdir().unwrap();

            fs::write(dir.path().join("hello.txt"), "hello world").unwrap();
            fs::write(dir.path().join("style.css"), "body{}").unwrap();
            fs::write(
                dir.path().join("README.md"),
                "# Hello\n\nSome *markdown* text.\n\n- [x] one\n- [ ] two\n\n|a|b|\n|-|-|\n|1|2|\n",
            )
            .unwrap();
            fs::write(dir.path().join(".gitignore"), "ignored/\n").unwrap();
            fs::create_dir(dir.path().join("docs")).unwrap();
            fs::write(dir.path().join("docs").join("guide.md"), "# Guide").unwrap();
            fs::create_dir(dir.path().join("ignored")).unwrap();
            fs::write(dir.path().join("ignored").join("skip.md"), "# Skip").unwrap();
            fs::create_dir(dir.path().join(".hidden")).unwrap();
            fs::write(dir.path().join(".hidden").join("secret.md"), "# Secret").unwrap();
            fs::create_dir(dir.path().join("sub")).unwrap();
            fs::write(dir.path().join("sub").join("index.html"), "<h1>hi</h1>").unwrap();
            fs::write(dir.path().join("file 2.txt"), "spaced").unwrap();
            fs::write(dir.path().join("a1.txt"), "").unwrap();
            fs::write(dir.path().join("a10.txt"), "").unwrap();
            fs::write(dir.path().join("a2.txt"), "").unwrap();

            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let shut = shutdown.clone();

            let mut srv = Server {
                dir: fs::canonicalize(dir.path()).unwrap(),
                quiet: true,
                docs_mode,
                single_file: None,
                onefile: None,
                fp: PathBuf::new(),
                html: String::new(),
            };

            let thread = std::thread::spawn(move || {
                listener.set_nonblocking(true).unwrap();
                while !shut.load(std::sync::atomic::Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            let _ = srv.handle(stream);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });

            TestServer {
                addr,
                _dir: dir,
                shutdown,
                thread: Some(thread),
            }
        }

        fn get(&self, path: &str) -> (String, String) {
            let mut stream = TcpStream::connect(self.addr).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut resp = String::new();
            let _ = stream.read_to_string(&mut resp);
            let (headers, body) = resp.split_once("\r\n\r\n").unwrap_or((&resp, ""));
            (headers.to_owned(), body.to_owned())
        }

        fn status(&self, path: &str) -> u16 {
            let (headers, _) = self.get(path);
            headers
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        }

        fn post(&self, path: &str) -> u16 {
            let mut stream = TcpStream::connect(self.addr).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            write!(stream, "POST {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut resp = String::new();
            let _ = stream.read_to_string(&mut resp);
            resp.lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.shutdown
                .store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    struct SingleFileServer {
        addr: std::net::SocketAddr,
        _dir: tempfile::TempDir,
        shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl SingleFileServer {
        fn new_txt() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("hello.txt");
            fs::write(&path, "single file content").unwrap();
            let canonical = fs::canonicalize(&path).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let shut = shutdown.clone();
            let mut srv = Server {
                dir: canonical.parent().unwrap().to_path_buf(),
                quiet: true,
                docs_mode: false,
                single_file: Some(format!(
                    "/{}",
                    canonical.file_name().unwrap().to_str().unwrap()
                )),
                onefile: None,
                fp: PathBuf::new(),
                html: String::new(),
            };
            let thread = std::thread::spawn(move || {
                listener.set_nonblocking(true).unwrap();
                while !shut.load(std::sync::atomic::Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            let _ = srv.handle(stream);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            SingleFileServer {
                addr,
                _dir: dir,
                shutdown,
                thread: Some(thread),
            }
        }

        fn get(&self, path: &str) -> (String, String) {
            let mut stream = TcpStream::connect(self.addr).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut resp = String::new();
            let _ = stream.read_to_string(&mut resp);
            let (headers, body) = resp.split_once("\r\n\r\n").unwrap_or((&resp, ""));
            (headers.to_owned(), body.to_owned())
        }
    }

    impl Drop for SingleFileServer {
        fn drop(&mut self) {
            self.shutdown
                .store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    fn onefile_default_entries() -> Vec<(&'static str, Vec<u8>, &'static str)> {
        vec![
            ("resources/0.woff2", (0u8..64).collect(), "font/woff2"),
            ("resources/9.bin", b"<html>frame</html>".to_vec(), "text/html"),
        ]
    }

    fn onefile_prefix() -> Vec<u8> {
        let index_json = onefile_default_entries()
            .iter()
            .map(|(name, _, mime)| format!("{{\"name\":\"{name}\",\"mime\":\"{mime}\"}}"))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "<!DOCTYPE html><html><head><script id=\"onefile-metadata\" type=\"application/json\">{{\"sourceUrl\":\"https://example.com/\",\"capturedAtMs\":1,\"unresolvedTotal\":0,\"unresolvedSample\":[],\"entries\":[{index_json}]}}</script></head><body>page</body></html><noframes>\n"
        )
        .into_bytes()
    }

    fn onefile_fixture_with_prefix(prefix: Vec<u8>, file_absolute: bool, method: u16) -> Vec<u8> {
        let entries = onefile_default_entries();
        let metadata_json = format!(
            "{{\"generator\":\"extension-onefile\",\"sourceUrl\":\"https://example.com/\",\"capturedAtMs\":1,\"prefixLength\":{},\"entries\":[{}],\"unresolved\":[]}}",
            prefix.len(),
            entries
                .iter()
                .map(|(name, data, mime)| {
                    format!(
                        "{{\"name\":\"{name}\",\"mime\":\"{mime}\",\"size\":{},\"crc32\":0}}",
                        data.len()
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut all: Vec<(&str, Vec<u8>, u16)> = entries
            .into_iter()
            .map(|(name, data, _)| (name, data, method))
            .collect();
        all.push(("metadata.json", metadata_json.into_bytes(), 0));
        build_onefile_zip(prefix, &all, file_absolute)
    }

    fn onefile_fixture(file_absolute: bool) -> Vec<u8> {
        onefile_fixture_with_prefix(onefile_prefix(), file_absolute, 0)
    }

    fn build_onefile_zip(
        prefix: Vec<u8>,
        entries: &[(&str, Vec<u8>, u16)],
        file_absolute: bool,
    ) -> Vec<u8> {
        let prefix_len = prefix.len() as u64;
        let mut bytes = prefix;
        let mut central = Vec::new();
        for (name, data, method) in entries {
            let local_abs = bytes.len() as u64;
            let name = name.as_bytes();
            let mut lh = Vec::with_capacity(30 + name.len());
            lh.extend_from_slice(&0x0403_4B50_u32.to_le_bytes());
            lh.extend_from_slice(&20_u16.to_le_bytes());
            lh.extend_from_slice(&0_u16.to_le_bytes());
            lh.extend_from_slice(&method.to_le_bytes());
            lh.extend_from_slice(&0_u16.to_le_bytes());
            lh.extend_from_slice(&0_u16.to_le_bytes());
            lh.extend_from_slice(&0_u32.to_le_bytes());
            lh.extend_from_slice(&(data.len() as u32).to_le_bytes());
            lh.extend_from_slice(&(data.len() as u32).to_le_bytes());
            lh.extend_from_slice(&(name.len() as u16).to_le_bytes());
            lh.extend_from_slice(&0_u16.to_le_bytes());
            lh.extend_from_slice(name);
            bytes.extend_from_slice(&lh);
            bytes.extend_from_slice(data);

            let lho = if file_absolute {
                local_abs
            } else {
                local_abs - prefix_len
            };
            let mut ch = Vec::with_capacity(46 + name.len());
            ch.extend_from_slice(&0x0201_4B50_u32.to_le_bytes());
            ch.extend_from_slice(&20_u16.to_le_bytes());
            ch.extend_from_slice(&20_u16.to_le_bytes());
            ch.extend_from_slice(&0_u16.to_le_bytes());
            ch.extend_from_slice(&method.to_le_bytes());
            ch.extend_from_slice(&0_u16.to_le_bytes());
            ch.extend_from_slice(&0_u16.to_le_bytes());
            ch.extend_from_slice(&0_u32.to_le_bytes());
            ch.extend_from_slice(&(data.len() as u32).to_le_bytes());
            ch.extend_from_slice(&(data.len() as u32).to_le_bytes());
            ch.extend_from_slice(&(name.len() as u16).to_le_bytes());
            ch.extend_from_slice(&0_u16.to_le_bytes());
            ch.extend_from_slice(&0_u16.to_le_bytes());
            ch.extend_from_slice(&0_u16.to_le_bytes());
            ch.extend_from_slice(&0_u16.to_le_bytes());
            ch.extend_from_slice(&0_u32.to_le_bytes());
            ch.extend_from_slice(&(lho as u32).to_le_bytes());
            ch.extend_from_slice(name);
            central.extend_from_slice(&ch);
        }
        let cd_abs = bytes.len() as u64;
        let cd_offset = if file_absolute {
            cd_abs
        } else {
            cd_abs - prefix_len
        };
        bytes.extend_from_slice(&central);
        let mut eocd = Vec::with_capacity(22);
        eocd.extend_from_slice(&0x0605_4B50_u32.to_le_bytes());
        eocd.extend_from_slice(&0_u16.to_le_bytes());
        eocd.extend_from_slice(&0_u16.to_le_bytes());
        eocd.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        eocd.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        eocd.extend_from_slice(&(central.len() as u32).to_le_bytes());
        eocd.extend_from_slice(&(cd_offset as u32).to_le_bytes());
        eocd.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&eocd);
        bytes.extend_from_slice(b"</noframes>");
        bytes
    }

    fn write_temp_file(name: &str, bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        let canonical = fs::canonicalize(&path).unwrap();
        (dir, canonical)
    }

    #[test]
    fn onefile_rejects_each_multidisk_and_zip64_field() {
        let original = onefile_fixture(true);
        let eocd = original.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
        for (field, value, expected) in [
            (4, vec![1, 0], "the embedded ZIP spans multiple disks"),
            (6, vec![1, 0], "the embedded ZIP spans multiple disks"),
            (8, vec![0, 0], "the embedded ZIP spans multiple disks"),
            (12, vec![255; 4], "the embedded ZIP uses ZIP64 fields"),
            (16, vec![255; 4], "the embedded ZIP uses ZIP64 fields"),
        ] {
            let mut bytes = original.clone();
            bytes[eocd + field..eocd + field + value.len()].copy_from_slice(&value);
            let (_dir, path) = write_temp_file("archive", &bytes);
            assert_eq!(onefile_detect(&path, bytes.len() as u64).err().as_deref(), Some(expected), "field {field}");
        }
        let mut bytes = original;
        bytes[eocd + 8..eocd + 12].fill(255);
        let (_dir, path) = write_temp_file("archive", &bytes);
        assert_eq!(onefile_detect(&path, bytes.len() as u64).err().as_deref(), Some("the embedded ZIP uses ZIP64 fields"));
    }

    #[test]
    fn onefile_detect_reads_file_absolute_offsets() {
        let bytes = onefile_fixture(true);
        let (_dir, path) = write_temp_file("page.u.zip.html", &bytes);
        let len = fs::metadata(&path).unwrap().len();
        let archive = onefile_detect(&path, len)
            .unwrap()
            .expect("fixture is a onefile archive");
        assert_eq!(archive.entries.len(), 3);
        let font = archive
            .entries
            .iter()
            .find(|entry| entry.name == "resources/0.woff2")
            .unwrap();
        assert_eq!(
            archive.entries[archive.by_name["resources/9.bin"]]
                .mime
                .as_deref(),
            Some("text/html")
        );
        assert_eq!(
            archive.entries[archive.by_name["metadata.json"]]
                .mime
                .as_deref(),
            None
        );
        let mut file = File::open(&path).unwrap();
        let mut buf = vec![0u8; font.len as usize];
        file_read_exact_at(&mut file, font.data_offset, &mut buf).unwrap();
        assert_eq!(buf, (0u8..64).collect::<Vec<u8>>());
        assert_eq!(archive.file_len, len);
    }

    #[test]
    fn onefile_detect_reads_relative_offsets() {
        let bytes = onefile_fixture(false);
        let (_dir, path) = write_temp_file("page.u.zip.html", &bytes);
        let len = fs::metadata(&path).unwrap().len();
        let archive = onefile_detect(&path, len)
            .unwrap()
            .expect("fixture is a onefile archive");
        assert_eq!(archive.entries.len(), 3);
        let mut file = File::open(&path).unwrap();
        let font = &archive.entries[archive.by_name["resources/0.woff2"]];
        let mut buf = vec![0u8; font.len as usize];
        file_read_exact_at(&mut file, font.data_offset, &mut buf).unwrap();
        assert_eq!(buf, (0u8..64).collect::<Vec<u8>>());
    }

    #[test]
    fn onefile_detect_plain_html_is_not_an_archive() {
        let bytes = b"<!DOCTYPE html><html><body>plain</body></html>";
        let (_dir, path) = write_temp_file("plain.html", bytes);
        assert!(onefile_detect(&path, bytes.len() as u64).unwrap().is_none());
    }

    #[test]
    fn onefile_detect_html_with_trailing_garbage_is_not_an_archive() {
        let mut bytes = b"<!DOCTYPE html><html></html>".to_vec();
        bytes.extend_from_slice(b"PK\x03\x04not-a-zip");
        let (_dir, path) = write_temp_file("garbage.html", &bytes);
        assert!(onefile_detect(&path, bytes.len() as u64).unwrap().is_none());
    }

    #[test]
    fn onefile_detect_plain_zip_without_html_prefix_is_not_an_archive() {
        let bytes = onefile_fixture_with_prefix(Vec::new(), true, 0);
        let (_dir, path) = write_temp_file("plain.zip", &bytes);
        assert!(onefile_detect(&path, bytes.len() as u64).unwrap().is_none());
    }

    #[test]
    fn onefile_detect_requires_html_prefix() {
        let bytes = onefile_fixture_with_prefix(vec![0x00, 0x01, 0x02], true, 0);
        let (_dir, path) = write_temp_file("not-html.bin", &bytes);
        assert!(onefile_detect(&path, bytes.len() as u64).unwrap().is_none());
    }

    #[test]
    fn onefile_detect_corrupt_central_directory_is_an_error() {
        let mut bytes = onefile_fixture(true);
        let pos = bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .expect("fixture has a central directory");
        bytes[pos] = 0x03;
        let (_dir, path) = write_temp_file("corrupt.u.zip.html", &bytes);
        assert!(onefile_detect(&path, bytes.len() as u64).is_err());
    }

    #[test]
    fn onefile_detect_skips_entries_with_unsupported_methods() {
        let bytes = onefile_fixture_with_prefix(onefile_prefix(), true, 8);
        let (_dir, path) = write_temp_file("deflate.u.zip.html", &bytes);
        let archive = onefile_detect(&path, bytes.len() as u64)
            .unwrap()
            .expect("fixture is a onefile archive");
        assert_eq!(archive.entries.len(), 1);
        assert!(archive.by_name.contains_key("metadata.json"));
        assert_eq!(
            archive.skipped,
            vec![
                "resources/0.woff2".to_owned(),
                "resources/9.bin".to_owned()
            ]
        );
    }

    struct OnefileServer {
        addr: std::net::SocketAddr,
        _dir: tempfile::TempDir,
        shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl OnefileServer {
        fn new() -> Self {
            let bytes = onefile_fixture(true);
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("capture.u.zip.html");
            fs::write(&path, &bytes).unwrap();
            let canonical = fs::canonicalize(&path).unwrap();
            let len = fs::metadata(&canonical).unwrap().len();
            let archive = onefile_detect(&canonical, len).unwrap().unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let shut = shutdown.clone();
            let mut srv = Server {
                dir: canonical.parent().unwrap().to_path_buf(),
                quiet: true,
                docs_mode: false,
                single_file: Some(format!(
                    "/{}",
                    canonical.file_name().unwrap().to_str().unwrap()
                )),
                onefile: Some(std::sync::Arc::new(archive)),
                fp: PathBuf::new(),
                html: String::new(),
            };
            let thread = std::thread::spawn(move || {
                listener.set_nonblocking(true).unwrap();
                while !shut.load(std::sync::atomic::Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            let _ = srv.handle(stream);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            OnefileServer {
                addr,
                _dir: dir,
                shutdown,
                thread: Some(thread),
            }
        }

        fn get_bytes(&self, path: &str) -> (String, Vec<u8>) {
            let mut stream = TcpStream::connect(self.addr).unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();
            write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut resp = Vec::new();
            let _ = stream.read_to_end(&mut resp);
            let split = resp
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .unwrap_or(0);
            (
                String::from_utf8_lossy(&resp[..split]).into_owned(),
                resp[split + 4..].to_vec(),
            )
        }
    }

    impl Drop for OnefileServer {
        fn drop(&mut self) {
            self.shutdown
                .store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    #[test]
    fn integration_onefile_serves_the_page_and_archive_entries() {
        let srv = OnefileServer::new();
        let (headers, body) = srv.get_bytes("/");
        assert!(headers.contains("200 OK"), "{headers}");
        assert!(headers.contains("Content-Type: text/html"), "{headers}");
        assert_eq!(body, onefile_fixture(true));

        let (headers, body) = srv.get_bytes("/resources/0.woff2");
        assert!(headers.contains("Content-Type: font/woff2"), "{headers}");
        assert_eq!(body, (0u8..64).collect::<Vec<u8>>());

        let (headers, body) = srv.get_bytes("/resources/9.bin");
        assert!(headers.contains("Content-Type: text/html"), "{headers}");
        assert_eq!(body, b"<html>frame</html>");

        let (headers, _) = srv.get_bytes("/metadata.json");
        assert!(headers.contains("application/json"), "{headers}");

        let (headers, _) = srv.get_bytes("/nope");
        assert!(headers.contains("404"), "{headers}");

        let (headers, _) = srv.get_bytes("/resources/../metadata.json");
        assert!(headers.contains("403"), "{headers}");

        let (headers, _) = srv.get_bytes("/%2e%2e/secret");
        assert!(headers.contains("403"), "{headers}");
    }

    #[test]
    fn integration_single_file_any_path() {
        let srv = SingleFileServer::new_txt();
        for path in ["/", "/whatever", "/some/nested/path"] {
            let (headers, body) = srv.get(path);
            assert!(headers.contains("200 OK"), "path {path}: {headers}");
            assert!(headers.contains("Content-Type: text/plain"), "path {path}");
            assert_eq!(body, "single file content", "path {path}");
        }
    }

    #[test]
    fn integration_serve_file() {
        let srv = TestServer::new();
        let (headers, body) = srv.get("/hello.txt");
        assert_eq!(srv.status("/hello.txt"), 200);
        assert!(headers.contains("Content-Type: text/plain"));
        assert!(headers.contains("Content-Length: 11"));
        assert!(headers.contains("Cache-Control: no-store"));
        assert!(headers.contains("Connection: close"));
        assert_eq!(body, "hello world");
    }

    #[test]
    fn integration_mime_type() {
        let srv = TestServer::new();
        let (headers, _) = srv.get("/style.css");
        assert!(headers.contains("Content-Type: text/css"));
    }

    #[test]
    fn integration_markdown_renders_to_html() {
        let srv = TestServer::new();
        let (headers, body) = srv.get("/README.md");
        assert!(headers.contains("200 OK"));
        assert!(headers.contains("Content-Type: text/html; charset=utf-8"));
        assert!(body.contains("<article class=\"markdown-body\">"));
        assert!(body.contains("<h1>Hello</h1>"));
        assert!(body.contains("<em>markdown</em>"));
        assert!(body.contains("<table>"));
        assert!(body.contains("<td>1</td>"));
        assert!(body.contains("type=\"checkbox\""));
    }

    #[test]
    fn integration_markdown_mermaid_renders_inline_svg() {
        let srv = TestServer::new();
        fs::write(
            srv._dir.path().join("diagram.md"),
            "# Diagram\n\n```mermaid\nflowchart TD\n  start[\"Parent agent\"] --> tier{\"Which tier?\"}\n  tier -->|expert| harness{\"Harness?\"}\n  harness -->|OpenCode| oc[\"OpenCode native expert agent<br/>effort max\"]\n```\n",
        )
        .unwrap();
        let (headers, body) = srv.get("/diagram.md");
        assert!(headers.contains("200 OK"));
        assert!(body.contains("<svg"), "{body}");
        assert!(body.contains("Parent agent"), "{body}");
        assert!(!body.contains("language-mermaid"), "{body}");
    }

    #[test]
    fn integration_docs_mode_mermaid_stays_a_code_block() {
        let srv = TestServer::new_docs();
        fs::write(
            srv._dir.path().join("diagram.md"),
            "```mermaid\nflowchart TD\n  a --> b\n```\n",
        )
        .unwrap();
        let (headers, body) = srv.get("/__docs__/md?path=diagram.md");
        assert!(headers.contains("200 OK"));
        assert!(body.contains("language-mermaid"), "{body}");
        assert!(!body.contains("<svg"), "{body}");
    }

    #[test]
    fn integration_docs_mode_shell() {
        let srv = TestServer::new_docs();
        let (headers, body) = srv.get("/");
        assert!(headers.contains("200 OK"));
        assert!(headers.contains("Content-Type: text/html; charset=utf-8"));
        assert!(body.contains("id=\"tree-root\""));
        assert!(body.contains("/__docs__/tree"));
    }

    #[test]
    fn integration_docs_mode_tree_filters_hidden_and_gitignored_dirs() {
        let srv = TestServer::new_docs();
        let (headers, body) = srv.get("/__docs__/tree");
        assert!(headers.contains("200 OK"));
        assert!(headers.contains("Content-Type: application/json; charset=utf-8"));
        assert_eq!(
            body,
            concat!(
                r#"{"name":"/","path":"","dir":true,"children":["#,
                r#"{"name":"docs","path":"docs","dir":true,"children":["#,
                r#"{"name":"guide.md","path":"docs/guide.md","dir":false}]},"#,
                r#"{"name":"README.md","path":"README.md","dir":false}]}"#,
            )
        );
    }

    #[test]
    fn integration_docs_tree_keeps_backslashes_in_file_names() {
        let srv = TestServer::new_docs();
        fs::write(srv._dir.path().join("a\\b.md"), "# Backslash").unwrap();
        let (_, tree) = srv.get("/__docs__/tree");
        assert!(tree.contains(r#"{"name":"a\\b.md","path":"a\\b.md","dir":false}"#), "{tree}");
        let (headers, body) = srv.get("/__docs__/md?path=a%5Cb.md");
        assert!(headers.starts_with("HTTP/1.1 200"), "{headers}");
        assert!(body.contains("<h1>Backslash</h1>"));
    }

    #[test]
    fn integration_docs_mode_markdown_fragment() {
        let srv = TestServer::new_docs();
        let (headers, body) = srv.get("/__docs__/md?path=README.md");
        assert!(headers.contains("200 OK"));
        assert!(headers.contains("Content-Type: text/html; charset=utf-8"));
        assert!(body.contains("<h1>Hello</h1>"));
        assert!(!body.contains("<html>"));
    }

    #[test]
    fn integration_docs_markdown_rejects_bad_paths() {
        let srv = TestServer::new_docs();
        fs::create_dir(srv._dir.path().join("folder.md")).unwrap();
        std::os::unix::fs::symlink(srv._dir.path().join("README.md"), srv._dir.path().join("link.md")).unwrap();
        for (path, status) in [
            ("/__docs__/md", 400),
            ("/__docs__/md?path=", 400),
            ("/__docs__/md?path=hello.txt", 400),
            ("/__docs__/md?path=../../outside.md", 403),
            ("/__docs__/md?path=%2E%2E%2F%2E%2E%2Foutside.md", 403),
            ("/__docs__/md?path=missing.md", 404),
            ("/__docs__/md?path=folder.md", 404),
            ("/__docs__/md?path=link.md", 403),
            ("/__docs__/md?path=docs%2Fguide.md", 200),
        ] {
            assert_eq!(srv.status(path), status, "{path}");
        }
    }

    fn raw_request(srv: &TestServer, request: &[u8]) -> String {
        let mut stream = TcpStream::connect(srv.addr).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let _ = stream.write_all(request);
        let mut resp = String::new();
        let _ = stream.read_to_string(&mut resp);
        resp
    }

    #[test]
    fn integration_malformed_and_oversized_requests_are_rejected() {
        let srv = TestServer::new();
        assert_eq!(
            raw_request(&srv, b"NOT A REQUEST\r\n\r\n"),
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 15\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n400 Bad Request"
        );
        let mut oversized = b"GET / HTTP/1.1\r\nX-Pad: ".to_vec();
        oversized.resize(9000, b'a');
        assert!(raw_request(&srv, &oversized).starts_with("HTTP/1.1 431 Request Header Fields Too Large\r\n"));
        assert_eq!(srv.status("/hello.txt"), 200);
    }

    #[test]
    fn integration_a_path_beneath_a_file_is_not_found() {
        let srv = TestServer::new();
        assert_eq!(srv.status("/hello.txt/x"), 404);
        let docs = TestServer::new_docs();
        assert_eq!(docs.status("/__docs__/md?path=README.md/x.md"), 404);
    }

    #[test]
    fn integration_an_unsearchable_directory_is_not_reported_missing() {
        use std::os::unix::fs::PermissionsExt;
        let srv = TestServer::new();
        let docs = TestServer::new_docs();
        for root in [srv._dir.path(), docs._dir.path()] {
            let locked = root.join("locked.md");
            fs::create_dir(&locked).unwrap();
            fs::write(locked.join("x.md"), "# x").unwrap();
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let srv_status = srv.status("/locked.md/x.md");
        let docs_status = docs.status("/__docs__/md?path=locked.md/x.md");
        for root in [srv._dir.path(), docs._dir.path()] {
            fs::set_permissions(root.join("locked.md"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_ne!(srv_status, 404, "a permission error is not a missing path");
        assert_ne!(docs_status, 404, "a permission error is not a missing path");
    }

    #[test]
    fn integration_a_browser_sized_header_set_is_accepted() {
        let srv = TestServer::new();
        let mut request = String::from("GET /hello.txt HTTP/1.1\r\nHost: localhost\r\n");
        for index in 0..40 {
            request.push_str(&format!("X-Header-{index}: value\r\n"));
        }
        request.push_str("\r\n");
        let response = raw_request(&srv, request.as_bytes());
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
        assert!(response.ends_with("\r\n\r\nhello world"), "{response}");

        let mut flood = String::from("GET /hello.txt HTTP/1.1\r\n");
        for index in 0..=MAX_HEADERS {
            flood.push_str(&format!("X-{index}: v\r\n"));
        }
        flood.push_str("\r\n");
        assert!(raw_request(&srv, flood.as_bytes()).starts_with("HTTP/1.1 431 Request Header Fields Too Large\r\n"));
    }

    #[test]
    fn integration_symlinked_and_special_files_are_forbidden() {
        let srv = TestServer::new();
        std::os::unix::fs::symlink(srv._dir.path().join("hello.txt"), srv._dir.path().join("link.txt")).unwrap();
        let _socket = std::os::unix::net::UnixListener::bind(srv._dir.path().join("socket")).unwrap();
        let (headers, body) = srv.get("/link.txt");
        assert!(headers.starts_with("HTTP/1.1 403"), "{headers}");
        assert_eq!(body, "403 Forbidden: symlinks not served");
        let (headers, body) = srv.get("/socket");
        assert!(headers.starts_with("HTTP/1.1 403"), "{headers}");
        assert_eq!(body, "403 Forbidden: not a regular file or directory");
    }

    #[test]
    fn ignore_rules_follow_the_supported_gitignore_subset() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".gitignore"),
            "# comment\n!keep\n./build/\n/anchored\nnested/dir\n*.tmp\n\n",
        )
        .unwrap();
        let rules = read_ignore_rules(dir.path());
        for (rel, name, ignored) in [
            ("build", "build", true),
            ("x/build", "build", true),
            ("anchored", "anchored", true),
            ("anchored/sub", "sub", true),
            ("x/anchored", "anchored", false),
            ("nested/dir", "dir", true),
            ("a/nested/dir", "dir", true),
            ("dir", "dir", false),
            ("cache.tmp", "cache.tmp", true),
            ("keep", "keep", false),
            ("!keep", "!keep", false),
            ("# comment", "# comment", false),
        ] {
            assert_eq!(is_gitignored_dir(rel, name, &rules), ignored, "{rel}");
        }
        assert!(read_ignore_rules(&dir.path().join("absent")).patterns.is_empty());
    }

    #[test]
    fn glob_match_backtracks_over_stars() {
        for (pattern, text, matched) in [
            ("a*c", "abbbc", true),
            ("a*c", "abbbd", false),
            ("a*b*c", "axbyc", true),
            ("a*b*c", "axbyd", false),
            ("a?c", "abc", true),
            ("a?c", "ac", false),
            ("**", "", true),
            ("ab*", "a", false),
            ("*.md", "x.md", true),
        ] {
            assert_eq!(glob_match(pattern, text), matched, "{pattern} {text}");
        }
    }

    #[test]
    fn json_string_escapes_quotes_backslashes_and_controls() {
        let mut out = String::new();
        json_string("a\"b\\c\nd\re\tf\u{1}g", &mut out);
        assert_eq!(out, r#""a\"b\\c\nd\re\tf\u0001g""#);
    }

    #[test]
    fn percent_decode_replaces_invalid_utf8() {
        assert_eq!(percent_decode("a%FFb"), "a\u{FFFD}b");
    }

    #[test]
    fn integration_directory_listing() {
        let srv = TestServer::new();
        let (headers, body) = srv.get("/");
        assert!(headers.contains("200 OK"));
        assert!(headers.contains("text/html"));
        assert!(body.contains("font-family: monospace"));
        assert!(body.contains("<table>"));
        assert!(body.contains("</table>"));
        assert!(body.contains("hello.txt"));
        assert!(body.contains("sub/"));
    }

    #[test]
    fn integration_directory_natural_sort() {
        let srv = TestServer::new();
        let (_, body) = srv.get("/");
        let pos1 = body.find("a1.txt").unwrap();
        let pos2 = body.find("a2.txt").unwrap();
        let pos10 = body.find("a10.txt").unwrap();
        assert!(pos1 < pos2, "a1 should come before a2");
        assert!(pos2 < pos10, "a2 should come before a10");
    }

    #[test]
    fn integration_index_html() {
        let srv = TestServer::new();
        let (headers, body) = srv.get("/sub/");
        assert!(headers.contains("200 OK"));
        assert_eq!(body, "<h1>hi</h1>");
    }

    #[test]
    fn integration_not_found() {
        let srv = TestServer::new();
        assert_eq!(srv.status("/nonexistent"), 404);
    }

    #[test]
    fn integration_method_not_allowed() {
        let srv = TestServer::new();
        assert_eq!(srv.post("/hello.txt"), 405);
    }

    #[test]
    fn integration_path_traversal() {
        let srv = TestServer::new();
        assert_eq!(srv.status("/../../../etc/passwd"), 403);
        assert_eq!(srv.status("/foo/../../etc/passwd"), 403);
    }

    #[test]
    fn integration_encoded_path() {
        let srv = TestServer::new();
        let (_, body) = srv.get("/file%202.txt");
        assert_eq!(body, "spaced");
    }

    #[test]
    fn integration_encoded_traversal() {
        let srv = TestServer::new();
        assert_eq!(srv.status("/%2e%2e/%2e%2e/etc/passwd"), 403);
    }

    #[test]
    fn integration_directory_listing_encodes_filenames() {
        let srv = TestServer::new();
        let (_, body) = srv.get("/");
        assert!(body.contains("file%202.txt"));
        assert!(body.contains("file 2.txt"));
    }

    #[test]
    fn integration_directory_listing_escapes_html() {
        let srv = TestServer::new();
        let name = "<img src=x&y>";
        fs::write(srv._dir.path().join(name), "x").unwrap();
        let (_, body) = srv.get("/");
        assert!(body.contains("&lt;img src=x&amp;y&gt;"));
        assert!(!body.contains("<img"));
        assert!(body.contains("href=\"%3Cimg%20src%3Dx%26y%3E\""));
    }

    #[test]
    fn integration_index_symlink_forbidden() {
        let srv = TestServer::new();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret.txt");
        fs::write(&secret, "secret-outside").unwrap();
        let dir = srv._dir.path().join("with-index");
        fs::create_dir(&dir).unwrap();
        symlink(&secret, dir.join("index.html")).unwrap();
        let (headers, body) = srv.get("/with-index/");
        assert!(headers.contains("403"));
        assert!(headers.contains("Forbidden: symlinks not served"));
        assert!(!body.contains("secret-outside"));
    }

    #[test]
    fn integration_docs_symlink_forbidden() {
        let srv = TestServer::new_docs();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("note.md");
        fs::write(&secret, "secret-outside").unwrap();
        symlink(&secret, srv._dir.path().join("evil.md")).unwrap();
        let (headers, body) = srv.get("/__docs__/md?path=evil.md");
        assert!(headers.contains("403"));
        assert!(headers.contains("Forbidden: symlinks not served"));
        assert!(!body.contains("secret-outside"));
    }

    #[test]
    fn integration_directory_symlink_followed() {
        let srv = TestServer::new();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("a.txt"), "outside-file").unwrap();
        symlink(outside.path(), srv._dir.path().join("linked")).unwrap();
        let (headers, body) = srv.get("/linked/a.txt");
        assert!(headers.contains("200"));
        assert_eq!(body, "outside-file");
    }

    #[test]
    fn send_file_truncated_returns_and_server_stays_live() {
        use std::sync::mpsc::channel;
        use std::time::Duration;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        let original = vec![b'x'; 1024 * 1024];
        fs::write(&path, &original).unwrap();
        let stale_len = fs::metadata(&path).unwrap().len();
        fs::write(&path, b"").unwrap();

        let run_once = |len: u64| {
            let file = File::open(&path).unwrap();
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let (tx, rx) = channel();
            let reader = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut buf = Vec::new();
                let _ = stream.read_to_end(&mut buf);
                tx.send(buf).unwrap();
            });
            let writer = std::thread::spawn(move || {
                let sock = TcpStream::connect(addr).unwrap();
                send_file(&file, &sock, len)
            });
            let result = writer.join().expect("send_file thread panicked");
            let _ = reader.join();
            (result, rx.recv().unwrap_or_default())
        };

        let (tx, rx) = channel();
        std::thread::scope(|s| {
            s.spawn(|| {
                tx.send(run_once(stale_len).0).unwrap();
            });
            let result = rx
                .recv_timeout(Duration::from_secs(10))
                .expect("truncated send_file did not return");
            result.unwrap();
        });

        fs::write(&path, "alive and longer").unwrap();
        let (result, received) = run_once(5);
        result.unwrap();
        assert_eq!(received, b"alive", "send_file must stop at the advertised length");

        let srv = TestServer::new();
        assert_eq!(srv.status("/hello.txt"), 200);
    }

    #[test]
    fn release_artifact_names_match_docs() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workflow = fs::read_to_string(root.join(".github/workflows/release.yml")).unwrap();
        let readme = fs::read_to_string(root.join("README.md")).unwrap();
        let makefile = fs::read_to_string(root.join("Makefile")).unwrap();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();

        let mut zips = Vec::new();
        for line in workflow.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("dist/srv-") else {
                continue;
            };
            if !rest.contains('/') {
                continue;
            }
            for token in rest.split_whitespace() {
                let token = token.trim_end_matches([';', '\\']);
                if token.is_empty() || !token.contains('/') || token.contains('$') || token.contains('"') {
                    continue;
                }
                let full = format!("dist/srv-{token}");
                if !full.ends_with(".zip") {
                    let base = full.rsplit('/').next().unwrap().to_owned();
                    zips.push(format!("{base}.zip"));
                }
            }
        }
        zips.sort();
        zips.dedup();
        assert!(
            !zips.is_empty(),
            "release workflow should list packaged Linux binaries"
        );
        for zip in &zips {
            assert!(
                readme.contains(zip),
                "README.md should document release archive {zip}"
            );
        }

        assert!(
            makefile.contains("target/$(TARGET)/dist/$(NAME)"),
            "Makefile dist output should stay at target/$(TARGET)/dist/$(NAME)"
        );
        assert!(
            agents.contains("target/<rust-target>/dist/srv"),
            "AGENTS.md should document the Makefile dist output path"
        );
    }
}
