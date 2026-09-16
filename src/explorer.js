'use strict';
const $ = id => document.getElementById(id);
const svgNS = 'http://www.w3.org/2000/svg';
let data, selected = null, page = 0, viewport = [0, 0, 1000, 560], drag;
const PAGE_SIZE = 100;
function element(tag, text, parent, className) {
  const e = document.createElement(tag);
  if (text !== undefined) e.textContent = text;
  if (className) e.className = className;
  if (parent) parent.append(e);
  return e;
}
function svg(tag, attrs, parent) {
  const e = document.createElementNS(svgNS, tag);
  for (const [key, value] of Object.entries(attrs)) e.setAttribute(key, value);
  parent.append(e); return e;
}
function key(e) { return JSON.stringify([e.from, e.to]); }
function card(parent, label, value) { const c = element('div', label, parent, 'card'); element('span', value, c, 'value'); }
async function refresh() {
  $('refresh').disabled = true; $('status').textContent = 'Analyzing source files…';
  try {
    const r = await fetch('/api/report', {cache: 'no-store'});
    if (!r.ok) throw new Error(await r.text());
    data = await r.json(); render();
    $('status').textContent = 'Analysis updated. Refresh after editing your source files.';
  } catch (e) { $('status').textContent = `Analysis failed: ${e.message}`; }
  finally { $('refresh').disabled = false; }
}
function moduleButton(name, parent) {
  const b = element('button', name, parent); b.onclick = () => { selected = name; render(); }; return b;
}
function render() {
  if (!data) return;
  const a = data.analysis, d = data.diff, m = data.metrics;
  const cycles = new Set(a.cycles.flat()), changed = new Set(d?.changed_source_files || []), deleted = new Set(d?.deleted_source_files || []), affected = new Set(d?.affected_modules || []);
  const nodes = [...new Set([...a.nodes, ...deleted])].sort();
  const q = $('search').value.toLowerCase(), scope = $('scope').value;
  const filtered = nodes.filter(n => n.toLowerCase().includes(q) && (scope === 'all' || (scope === 'cycles' ? cycles : scope === 'changed' ? changed : affected).has(n)));
  page = Math.max(0, Math.min(page, Math.ceil(filtered.length / PAGE_SIZE) - 1));
  const visible = filtered.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);
  $('cards').replaceChildren();
  for (const [label, value] of [['Health', `${m.health_score}/100`], ['Modules', a.source_files], ['Dependencies', a.dependencies], ['Cycles', a.cycles.length], ['Boundary violations', data.violations.length]]) card($('cards'), label, value);
  $('impact').replaceChildren();
  $('changes').hidden = !d;
  if (d) {
    element('h2', `Branch impact vs ${d.base}`, $('impact'));
    element('p', `Compared with merge base ${d.merge_base.slice(0, 12)}; includes working-tree changes.`, $('impact'), 'muted');
    const cards = element('div', undefined, $('impact'), 'cards');
    for (const [label, value] of [['Changed files', changed.size], ['Affected modules', affected.size], ['Added edges', d.added_edges.length], ['Removed edges', d.removed_edges.length], ['New cycles', d.new_cycles.length]]) card(cards, label, value);
  }
  $('rows').replaceChildren();
  const incoming = new Map(), outgoing = new Map();
  for (const e of a.edges) { incoming.set(e.to, (incoming.get(e.to) || 0) + 1); outgoing.set(e.from, (outgoing.get(e.from) || 0) + 1); }
  for (const n of visible) {
    const tr = element('tr', undefined, $('rows')); moduleButton(n, element('td', undefined, tr));
    element('td', outgoing.get(n) || 0, tr); element('td', incoming.get(n) || 0, tr);
    element('td', deleted.has(n) ? 'Deleted' : changed.has(n) ? 'Changed' : cycles.has(n) ? 'Cycle' : affected.has(n) ? 'Affected' : 'Unchanged', tr);
  }
  $('cycles').replaceChildren();
  for (const c of a.cycles) element('li', `Cycle group: ${c.join(', ')}`, $('cycles'));
  if (!a.cycles.length) element('li', 'No dependency cycles detected.', $('cycles'));
  $('violations').replaceChildren();
  for (const v of data.violations) element('li', `${v.from} → ${v.to}: ${v.message}`, $('violations'));
  if (!data.violations.length) element('li', 'No boundary violations.', $('violations'));
  $('edge-changes').replaceChildren();
  for (const [kind, edges] of [['Added', d?.added_edges || []], ['Removed', d?.removed_edges || []]]) for (const e of edges) element('li', `${kind}: ${e.from} → ${e.to}`, $('edge-changes'), kind.toLowerCase());
  if (d && !d.added_edges.length && !d.removed_edges.length) element('li', 'No dependency changes.', $('edge-changes'));
  const edges = [...a.edges, ...(d?.removed_edges || [])], added = new Set((d?.added_edges || []).map(key)), removed = new Set((d?.removed_edges || []).map(key));
  draw(visible, edges, {cycles, changed, deleted, added, removed});
  $('graph-status').textContent = filtered.length ? `Showing ${page * PAGE_SIZE + 1}–${page * PAGE_SIZE + visible.length} of ${filtered.length} matching modules. Edges appear when both endpoints are on this page.` : 'No matching modules.';
  $('previous').disabled = page === 0; $('next').disabled = (page + 1) * PAGE_SIZE >= filtered.length;
  if (selected) {
    $('details').replaceChildren(); element('h2', selected, $('details'));
    element('p', deleted.has(selected) ? 'Deleted from the current tree' : `${a.lines[selected] || 0} ${(a.lines[selected] || 0) === 1 ? 'line' : 'lines'}`, $('details'));
    for (const [label, names] of [['Imports', a.edges.filter(e => e.from === selected).map(e => e.to)], ['Imported by', a.edges.filter(e => e.to === selected).map(e => e.from)], ['Removed connections', (d?.removed_edges || []).filter(e => e.from === selected || e.to === selected).map(e => e.from === selected ? e.to : e.from)]]) {
      element('h3', label, $('details')); names.forEach(n => moduleButton(n, $('details'))); if (!names.length) element('p', 'None', $('details'), 'muted');
    }
  }
}
function draw(nodes, edges, flags) {
  const root = $('graph'); root.replaceChildren();
  const defs = svg('defs', {}, root);
  for (const [id, color] of [['normal', '#65809f'], ['added', '#66dab7'], ['removed', '#f38da0']]) {
    const marker = svg('marker', {id, viewBox:'0 0 10 10', refX:9, refY:5, markerWidth:7, markerHeight:7, orient:'auto-start-reverse'}, defs);
    svg('path', {d:'M 0 0 L 10 5 L 0 10 z', fill:color}, marker);
  }
  const pos = new Map(), visible = new Set(nodes);
  // Collapse cycle groups before assigning dependency layers so cycles cannot
  // repeatedly increase each other's depth.
  const group = new Map(nodes.map(n => [n, n]));
  for (const cycle of data.analysis.cycles) { const representative = cycle[0]; for (const n of cycle) if (visible.has(n)) group.set(n, representative); }
  const levels = new Map([...group.values()].map(n => [n, 0]));
  for (let i = 0; i < levels.size; i++) {
    let changed = false;
    for (const e of data.analysis.edges) if (visible.has(e.from) && visible.has(e.to)) {
      const from = group.get(e.from), to = group.get(e.to);
      if (from !== to && levels.get(from) <= levels.get(to)) { levels.set(from, levels.get(to) + 1); changed = true; }
    }
    if (!changed) break;
  }
  const counts = new Map();
  nodes.forEach((n, i) => {
    if ($('layout').value === 'circle') { const radius = Math.max(180, nodes.length * 24); const t = i / nodes.length * Math.PI * 2; pos.set(n, [radius + Math.cos(t) * radius + 130, radius + Math.sin(t) * radius + 30]); }
    else { const layer = levels.get(group.get(n)), row = counts.get(layer) || 0; counts.set(layer, row + 1); pos.set(n, [140 + layer * 290, 45 + row * 70]); }
  });
  for (const e of edges) if (pos.has(e.from) && pos.has(e.to)) {
    const [x,y] = pos.get(e.from), [tx,ty] = pos.get(e.to), kind = flags.removed.has(key(e)) ? 'removed' : flags.added.has(key(e)) ? 'added' : 'normal';
    const self = e.from === e.to;
    const dx = tx-x, dy=ty-y;
    // Curve reciprocal and long edges so direction and intermediate nodes stay readable.
    const bend = dx === 0 ? (dy > 0 ? 90 : -90) : Math.abs(dx) > 300 ? -80 : -25;
    const mx = (x+tx)/2 + (dx === 0 ? bend : 0), my = (y+ty)/2 + (dx === 0 ? 0 : bend);
    const exit = Math.max(Math.abs(mx-x)/120, Math.abs(my-y)/20) || 1;
    const entry = Math.max(Math.abs(tx-mx)/120, Math.abs(ty-my)/20) || 1;
    const startX=x+(mx-x)/exit, startY=y+(my-y)/exit;
    const endX=tx-(tx-mx)/entry, endY=ty-(ty-my)/entry;
    const path = self ? `M ${x+80} ${y-20} C ${x+180} ${y-90},${x+180} ${y+90},${x+80} ${y+20}` : `M ${startX} ${startY} Q ${mx} ${my} ${endX} ${endY}`;
    svg('path', {d:path, class:`edge ${kind}`, 'marker-end':`url(#${kind})`}, root);
  }
  for (const n of nodes) {
    const [x,y] = pos.get(n), classes = ['node', selected === n ? 'selected' : '', flags.cycles.has(n) ? 'cycle' : '', flags.changed.has(n) ? 'changed' : '', flags.deleted.has(n) ? 'deleted' : ''].join(' ');
    const g = svg('g', {class:classes, transform:`translate(${x},${y})`, role:'button', tabindex:0, 'aria-label':n}, root);
    svg('title', {}, g).textContent = n;
    svg('rect', {x:-120,y:-20,width:240,height:40,rx:7}, g);
    svg('text', {x:0,y:4,'text-anchor':'middle'}, g).textContent = n.length > 31 ? `…${n.slice(-30)}` : n;
    g.onclick = () => { selected=n; render(); };
    g.onkeydown = e => { if(e.key==='Enter'||e.key===' ') { e.preventDefault(); selected=n; render(); root.querySelector('.node.selected')?.focus(); } };
  }
  applyView();
}
function applyView() { $('graph').setAttribute('viewBox', viewport.join(' ')); }
function fit() { const box = $('graph').getBBox(); viewport = box.width ? [box.x-30,box.y-30,box.width+60,Math.max(box.height+60,200)] : [0,0,1000,560]; applyView(); }
function zoom(factor) { const [x,y,w,h]=viewport; const width=Math.max(80,Math.min(100000,w*factor)), height=h*width/w; viewport=[x+(w-width)/2,y+(h-height)/2,width,height]; applyView(); }
$('graph').onwheel=e=>{e.preventDefault();zoom(e.deltaY>0?1.15:1/1.15);};
$('graph').onpointerdown=e=>{if(e.target.closest('.node'))return;drag=[e.clientX,e.clientY,...viewport];$('graph').setPointerCapture(e.pointerId);};
$('graph').onpointermove=e=>{if(!drag)return;const rect=$('graph').getBoundingClientRect();const scale=Math.max(drag[4]/rect.width,drag[5]/rect.height);viewport=[drag[2]-(e.clientX-drag[0])*scale,drag[3]-(e.clientY-drag[1])*scale,drag[4],drag[5]];applyView();};
$('graph').onpointerup=$('graph').onpointercancel=()=>{drag=null;};
$('refresh').onclick=refresh;
$('search').oninput=$('scope').onchange=()=>{page=0;render();fit();};
$('layout').onchange=()=>{render();fit();};
$('fit').onclick=fit;$('zoom-in').onclick=()=>zoom(1/1.25);$('zoom-out').onclick=()=>zoom(1.25);
$('previous').onclick=()=>{page--;render();fit();};$('next').onclick=()=>{page++;render();fit();};
refresh().then(fit);
