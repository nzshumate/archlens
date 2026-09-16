use oxc_allocator::Allocator;
use oxc_ast::{
    ast::{Argument, CallExpression, Expression, ImportExpression, Statement},
    visit::{walk, Visit},
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::{collections::HashSet, path::Path};

pub fn imports(path: &Path, source: &str) -> Vec<String> {
    let owned;
    let source = if path.extension().and_then(|e| e.to_str()) == Some("vue") {
        owned = extract_vue_scripts(source);
        &owned
    } else {
        source
    };
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
    found
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

fn extract_vue_scripts(source: &str) -> String {
    let mut out = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("<script") {
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let body = &after[tag_end + 1..];
        let Some(end) = body.find("</script>") else {
            break;
        };
        out.push_str(&body[..end]);
        out.push('\n');
        rest = &body[end + 9..];
    }
    out
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
