use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

pub fn imports(path: &Path, source: &str) -> Vec<String> {
    // Vue SFCs need their script block isolated before JS/TS parsing.
    let source = if path.extension().and_then(|e| e.to_str()) == Some("vue") {
        extract_vue_script(source).unwrap_or(source)
    } else { source };

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let mut found = Vec::new();

    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(decl) => found.push(decl.source.value.to_string()),
            Statement::ExportAllDeclaration(decl) => found.push(decl.source.value.to_string()),
            Statement::ExportNamedDeclaration(decl) => {
                if let Some(source) = &decl.source { found.push(source.value.to_string()); }
            }
            _ => {}
        }
    }
    found
}

fn extract_vue_script(source: &str) -> Option<&str> {
    let start_tag = source.find("<script")?;
    let content_start = source[start_tag..].find('>')? + start_tag + 1;
    let content_end = source[content_start..].find("</script>")? + content_start;
    Some(&source[content_start..content_end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ts_imports_and_reexports() {
        let src = "import x from './x'; export { y } from './y'; export * from './z';";
        let got = imports(Path::new("app.ts"), src);
        assert_eq!(got, vec!["./x", "./y", "./z"]);
    }

    #[test]
    fn parses_vue_script_setup() {
        let src = "<template><div /></template><script setup lang=\"ts\">import x from './x'</script>";
        assert_eq!(imports(Path::new("App.vue"), src), vec!["./x"]);
    }
}
