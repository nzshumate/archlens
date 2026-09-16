use crate::{advice, analyzer, git, metrics, rules};
use anyhow::{Context, Result};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    time::Duration,
};
const HTML: &str = include_str!("explorer.html");
const JS: &str = include_str!("explorer.js");
fn report(root: &Path, base: Option<&str>) -> Result<String> {
    let analysis = analyzer::analyze(root)?;
    let config = rules::load(root)?;
    let metrics = metrics::calculate(&analysis, &config)?;
    let violations = rules::evaluate(&analysis, &config);
    let guidance = advice::build(&analysis, &metrics, &config, &violations);
    let diff = base.map(|b| git::diff(root, b, &analysis)).transpose()?;
    Ok(serde_json::to_string(
        &serde_json::json!({"schema_version":1,"project":root.file_name().unwrap_or_default().to_string_lossy(),"analysis":analysis,"metrics":metrics,"violations":violations,"diff":diff,"guidance":guidance}),
    )?)
}
fn respond(mut stream: TcpStream, root: &Path, base: Option<&str>) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut request = Vec::new();
    let mut chunk = [0; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < 8192 {
        let size = stream.read(&mut chunk)?;
        if size == 0 {
            return Ok(());
        }
        request.extend_from_slice(&chunk[..size]);
    }
    let request = String::from_utf8_lossy(&request);
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    let (status, content_type, body) = if method != "GET" {
        (
            "405 Method Not Allowed",
            "text/plain",
            "Only GET is supported".to_string(),
        )
    } else {
        match path {
            "/" => ("200 OK", "text/html; charset=utf-8", HTML.to_string()),
            "/explorer.js" => ("200 OK", "text/javascript; charset=utf-8", JS.to_string()),
            "/api/report" => match report(root, base) {
                Ok(json) => ("200 OK", "application/json", json),
                Err(error) => (
                    "500 Internal Server Error",
                    "text/plain; charset=utf-8",
                    error.to_string(),
                ),
            },
            _ => ("404 Not Found", "text/plain", "Not found".to_string()),
        }
    };
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'\r\nConnection: close\r\n\r\n{body}",body.len())?;
    Ok(())
}
pub fn serve(root: &Path, port: u16, base: Option<&str>) -> Result<()> {
    let root = root.canonicalize()?;
    // Fail early for invalid paths or refs, rather than serving an unusable page.
    report(&root, base)?;
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("cannot bind port {port}"))?;
    println!("Oxarch explorer: http://{}", listener.local_addr()?);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = respond(stream, &root, base) {
                    eprintln!("Explorer request failed: {error}");
                }
            }
            Err(error) => eprintln!("Explorer connection failed: {error}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refresh_reanalyzes_files_and_reports_rule_errors() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("main.ts"), "export {};").unwrap();
        let first: serde_json::Value = serde_json::from_str(&report(root, None).unwrap()).unwrap();
        assert_eq!(first["analysis"]["source_files"], 1);
        std::fs::write(root.join("main.ts"), "import './added';").unwrap();
        std::fs::write(root.join("added.ts"), "export {};").unwrap();
        let second: serde_json::Value = serde_json::from_str(&report(root, None).unwrap()).unwrap();
        assert_eq!(second["analysis"]["dependencies"], 1);
        assert!(second["diff"].is_null());
        std::fs::write(root.join("oxarch.json"), "invalid").unwrap();
        assert!(report(root, None).is_err());
    }
}
