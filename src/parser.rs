use oxc_allocator::Allocator;
use oxc_ast::{
    ast::{Argument, CallExpression, Expression, ImportExpression, Statement},
    visit::{walk, Visit},
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ParseIssue {
    pub message: String,
    pub line: usize,
    pub column: usize,
}
pub struct Parsed {
    pub imports: Vec<String>,
    pub issues: Vec<ParseIssue>,
}

#[cfg(test)]
pub fn imports(path: &Path, source: &str) -> Vec<String> {
    parse(path, source).imports
}

pub fn parse(path: &Path, source: &str) -> Parsed {
    if path.extension().and_then(|e| e.to_str()) == Some("vue") {
        let (scripts, mut imports) = extract_vue_scripts(source);
        let mut issues = Vec::new();
        for (script, jsx) in scripts {
            let parsed = parse(
                Path::new(if jsx { "script.tsx" } else { "script.ts" }),
                &script,
            );
            imports.extend(parsed.imports);
            issues.extend(parsed.issues);
        }
        let mut seen = HashSet::new();
        imports.retain(|value| seen.insert(value.clone()));
        return Parsed { imports, issues };
    }
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let mut found = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(d) => found.push(d.source.value.to_string()),
            Statement::ExportAllDeclaration(d) => found.push(d.source.value.to_string()),
            Statement::ExportNamedDeclaration(d) => {
                if let Some(s) = &d.source {
                    found.push(s.value.to_string());
                }
            }
            _ => {}
        }
    }
    let mut runtime = RuntimeImports::default();
    runtime.visit_program(&parsed.program);
    found.extend(runtime.imports);
    let mut seen = HashSet::new();
    found.retain(|item| seen.insert(item.clone()));
    let issues = parsed
        .errors
        .iter()
        .map(|error| {
            let offset = error
                .labels
                .as_ref()
                .and_then(|labels| labels.first())
                .map_or(0, |label| label.offset())
                .min(source.len());
            let prefix = String::from_utf8_lossy(&source.as_bytes()[..offset]);
            ParseIssue {
                message: error.to_string(),
                line: prefix.bytes().filter(|b| *b == b'\n').count() + 1,
                column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
            }
        })
        .collect();
    Parsed {
        imports: found,
        issues,
    }
}

#[derive(Default)]
struct RuntimeImports {
    imports: Vec<String>,
}
impl<'a> Visit<'a> for RuntimeImports {
    fn visit_import_expression(&mut self, expression: &ImportExpression<'a>) {
        if let Expression::StringLiteral(value) = &expression.source {
            self.imports.push(value.value.to_string());
        }
        walk::walk_import_expression(self, expression);
    }
    fn visit_call_expression(&mut self, expression: &CallExpression<'a>) {
        if matches!(&expression.callee, Expression::Identifier(name) if name.name == "require") {
            if let Some(Argument::StringLiteral(value)) = expression.arguments.first() {
                self.imports.push(value.value.to_string());
            }
        }
        walk::walk_call_expression(self, expression);
    }
}

fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut rest = tag;
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if name.is_empty() {
            break;
        }
        rest = rest[end..].trim_start();
        let mut value = "";
        if let Some(next) = rest.strip_prefix('=') {
            rest = next.trim_start();
            if let Some(quote) = rest.chars().next().filter(|c| *c == '\'' || *c == '"') {
                rest = &rest[1..];
                let end = rest.find(quote).unwrap_or(rest.len());
                value = &rest[..end];
                rest = &rest[end..];
                if rest.starts_with(quote) {
                    rest = &rest[1..];
                }
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                value = &rest[..end];
                rest = &rest[end..];
            }
        }
        result.push((name.to_owned(), value.to_owned()));
    }
    result
}
fn extract_vue_scripts(source: &str) -> (Vec<(String, bool)>, Vec<String>) {
    // Mask non-script bytes while preserving line/column offsets for diagnostics.
    let mut out: Vec<u8> = source
        .bytes()
        .map(|b| if b == b'\n' || b == b'\r' { b } else { b' ' })
        .collect();
    let mut external = Vec::new();
    let mut scripts = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find('<') {
        let start = cursor + relative;
        if source[start..].starts_with("<!--") {
            cursor = source[start + 4..]
                .find("-->")
                .map_or(source.len(), |end| start + 4 + end + 3);
            continue;
        }
        if !source[start..].starts_with("<script")
            || !source[start + 7..].starts_with(|c: char| c.is_whitespace() || c == '>')
        {
            cursor = start + 1;
            continue;
        }
        let Some(tag_end) = source[start..].find('>').map(|end| start + end) else {
            break;
        };
        let mut jsx = false;
        let attrs = attributes(&source[start + 7..tag_end]);
        for (name, value) in attrs {
            if name == "src" && !value.is_empty() {
                external.push(value);
            } else if name == "lang" && matches!(value.as_str(), "tsx" | "jsx") {
                jsx = true;
            }
        }
        let body = tag_end + 1;
        let Some(end) = source[body..].find("</script>").map(|end| body + end) else {
            break;
        };
        out[body..end].copy_from_slice(&source.as_bytes()[body..end]);
        scripts.push((String::from_utf8(out.clone()).expect("masked UTF-8"), jsx));
        for byte in &mut out[body..end] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
        cursor = end + 9;
    }
    (scripts, external)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_ts_imports_and_reexports() {
        let src = "import x from './x'; export { y } from './y'; export * from './z';";
        assert_eq!(imports(Path::new("app.ts"), src), vec!["./x", "./y", "./z"]);
    }
    #[test]
    fn parses_dynamic_and_commonjs() {
        let src = "const a = import('./lazy'); const b = require('./legacy');";
        assert_eq!(
            imports(Path::new("app.ts"), src),
            vec!["./lazy", "./legacy"]
        );
    }
    #[test]
    fn ignores_runtime_import_text_in_comments_and_strings() {
        let source = r#"// require('./comment')
            const text = "import('./string')";
            const pattern = /require('fake')/;
            const actual = import /* allowed */ ('./lazy');
            const other = require ('./legacy');
            thing.require('./method');
        "#;
        assert_eq!(
            imports(Path::new("app.ts"), source),
            vec!["./lazy", "./legacy"]
        );
    }
    #[test]
    fn parses_multiple_vue_scripts() {
        let src="<script>import a from './a'</script><script setup lang=\"ts\">import b from './b'</script>";
        assert_eq!(imports(Path::new("App.vue"), src), vec!["./a", "./b"]);
    }
}
