use crate::{analyzer, metrics};
use anyhow::{Context, Result};
use std::{io::{Read, Write}, net::TcpListener, path::Path};

const HTML: &str = r#"<!doctype html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width'><title>Archlens</title><style>body{font:14px system-ui;margin:0;background:#0b1020;color:#e8ecf3}header{padding:20px 28px;border-bottom:1px solid #283149}main{padding:28px;max-width:1200px;margin:auto}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:12px}.card{background:#141b2d;border:1px solid #283149;border-radius:12px;padding:16px}.value{font-size:28px;font-weight:700}input{width:100%;box-sizing:border-box;padding:12px;margin:20px 0;background:#141b2d;border:1px solid #34405d;color:white;border-radius:8px}table{width:100%;border-collapse:collapse}td,th{text-align:left;padding:9px;border-bottom:1px solid #202940}.bad{color:#ffb4a9}</style></head><body><header><b>ARCHLENS</b> · See your frontend architecture</header><main><div class='cards' id='cards'></div><input id='q' placeholder='Filter modules…'><table><thead><tr><th>Module</th><th>Outgoing</th><th>Incoming</th></tr></thead><tbody id='rows'></tbody></table></main><script>let data;fetch('/api/report').then(r=>r.json()).then(d=>{data=d;render('')});q.oninput=e=>render(e.target.value.toLowerCase());function render(q){let a=data.analysis,m=data.metrics;cards.innerHTML=`<div class=card><div>Health</div><div class=value>${m.health_score}/100</div></div><div class=card><div>Modules</div><div class=value>${a.source_files}</div></div><div class=card><div>Dependencies</div><div class=value>${a.dependencies}</div></div><div class=card><div>Cycles</div><div class='value ${a.cycles.length?'bad':''}'>${a.cycles.length}</div></div>`;let inc={},out={};a.edges.forEach(e=>{out[e.from]=(out[e.from]||0)+1;inc[e.to]=(inc[e.to]||0)+1});rows.innerHTML=a.nodes.filter(n=>n.toLowerCase().includes(q)).map(n=>`<tr><td>${n}</td><td>${out[n]||0}</td><td>${inc[n]||0}</td></tr>`).join('')}}</script></body></html>"#;

pub fn serve(root: &Path, port: u16) -> Result<()> {
    let report = analyzer::analyze(root)?;
    let metrics = metrics::calculate(&report);
    let json = serde_json::to_string(&serde_json::json!({"analysis": report, "metrics": metrics}))?;
    let listener = TcpListener::bind(("127.0.0.1", port)).with_context(|| format!("cannot bind port {port}"))?;
    println!("Archlens explorer: http://127.0.0.1:{port}");
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut request = [0u8; 2048];
        let size = stream.read(&mut request)?;
        let req = String::from_utf8_lossy(&request[..size]);
        let api = req.starts_with("GET /api/report ");
        let (content_type, body) = if api { ("application/json", json.as_str()) } else { ("text/html; charset=utf-8", HTML) };
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())?;
    }
    Ok(())
}
