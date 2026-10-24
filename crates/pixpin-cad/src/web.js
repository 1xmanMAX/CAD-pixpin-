// El visor del plano en la pagina web (`web.rs`): lo mismo que hace la
// tarjeta grafica en el visor, con un lienzo 2D. Cada tramo se vuelve
// caminos (`Path2D`) la primera vez que se ve y se guarda; lo que en pantalla
// mediria menos de un pixel no se dibuja y de lo demas, solo lo que cae en la
// ventana. Las pasadas, en el orden del visor: imagenes, rellenos,
// sombreados, rayas, circulos y letras; encima, las cotas, los marcos y lo
// anotado con el motor del lienzo (un SVG).
(function(){
"use strict";
var D = JSON.parse(document.getElementById('datos').textContent);
var lienzo = document.getElementById('lienzo');
var cv = document.getElementById('plano'), ctx = cv.getContext('2d');
var cvE = document.getElementById('encima'), ctxE = cvE.getContext('2d');
var anot = document.getElementById('anot');
var panel = document.getElementById('panel');
var svg = anot.querySelector('svg');
var claro = !!D.claro, grosores = true, conAnot = true;
var capasOn = D.capas.map(function(c){ return !!c.on; });
var F = new Float32Array(0), U = new Uint32Array(0);
var vista = { cx: 0, cy: 0, px: 1 };
var W = 1, H = 1, dpr = 1, pedido = false;
var cache = new Map();
var imagenes = D.imagenes.map(function(i){ var im = new Image(); im.onload = redibujar; im.src = i.src; return { d: i, im: im }; });

// ---- colores (los del sombreador del visor)
function partes(c){ return [c & 255, (c >>> 8) & 255, (c >>> 16) & 255, (c >>> 24) & 255]; }
function luz(r, g, b){ return 0.2126 * r + 0.7152 * g + 0.0722 * b; }
function base7(c, cl){
  var x = partes(c);
  if (x[3] === 0) return cl ? [0, 0, 0, 1] : [1, 1, 1, 1];
  return [x[0] / 255, x[1] / 255, x[2] / 255, x[3] / 255];
}
function tinta(c, cl){
  var v = base7(c, cl), r = v[0], g = v[1], b = v[2];
  var l = luz(r, g, b), sat = Math.max(r, g, b) - Math.min(r, g, b);
  if (!cl) {
    if (sat < 0.15 && l < 0.5) { r = 1 - r * 0.8; g = 1 - g * 0.8; b = 1 - b * 0.8; }
    else if (l < 0.55) { var k = (0.55 - l) / (1 - l); r += (1 - r) * k; g += (1 - g) * k; b += (1 - b) * k; }
  } else if (l > 0.62) { var k2 = 0.5 / l; r *= k2; g *= k2; b *= k2; }
  return [r, g, b, v[3]];
}
function apagado(c, cl){
  var v = base7(c, cl);
  if (cl) return v;
  var sat = Math.max(v[0], v[1], v[2]) - Math.min(v[0], v[1], v[2]);
  if (sat < 0.15) return [0.17 + (1 - v[0]) * 0.22, 0.17 + (1 - v[1]) * 0.22, 0.17 + (1 - v[2]) * 0.22, v[3]];
  return [v[0] * 0.42, v[1] * 0.42, v[2] * 0.42, v[3]];
}
function css(v, a){ return 'rgba(' + Math.round(v[0] * 255) + ',' + Math.round(v[1] * 255) + ',' + Math.round(v[2] * 255) + ',' + (v[3] * (a === undefined ? 1 : a)).toFixed(3) + ')'; }
var paleta = {};
function colores(cl){
  var k = cl ? 'c' : 'o';
  if (!paleta[k]) paleta[k] = {
    raya: D.colores.map(function(c){ return css(tinta(c, cl)); }),
    relleno: D.colores.map(function(c){ return css(apagado(c, cl)); }),
    trama: D.colores.map(function(c){ return tinta(c, cl); })
  };
  return paleta[k];
}

// ---- tramos: [x0, y0, x1, y1, tamano, desde, cuantos]
function tramos(ap, minimo, caja, fn){
  for (var i = 0, p = ap[0]; i < ap[1]; i++, p += 7) {
    if (F[p + 4] < minimo) break;
    if (F[p] > caja[2] || F[p + 2] < caja[0] || F[p + 1] > caja[3] || F[p + 3] < caja[1]) continue;
    fn(p);
  }
}
function guardado(k, hacer){ var c = cache.get(k); if (!c) { c = hacer(); cache.set(k, c); } return c; }
function porColor(lista, col){
  for (var i = 0; i < lista.length; i++) if (lista[i][0] === col) return lista[i][1];
  var p = new Path2D(); lista.push([col, p]); return p;
}
function rayas(p){ return guardado('L' + p, function(){
  var c = [], q = U[p + 5];
  for (var s = 0; s < U[p + 6]; s++) {
    var col = U[q], m = U[q + 1], path = porColor(c, col); q += 2;
    path.moveTo(F[q], F[q + 1]);
    for (var j = 1; j < m; j++) path.lineTo(F[q + 2 * j], F[q + 2 * j + 1]);
    q += 2 * m;
  }
  return c;
}); }
function triangulos(path, q, n, caja){
  for (var j = 0; j < n; j++, q += 6) {
    path.moveTo(F[q], F[q + 1]); path.lineTo(F[q + 2], F[q + 3]); path.lineTo(F[q + 4], F[q + 5]); path.closePath();
    if (caja) for (var k = 0; k < 6; k += 2) {
      caja[0] = Math.min(caja[0], F[q + k]); caja[1] = Math.min(caja[1], F[q + k + 1]);
      caja[2] = Math.max(caja[2], F[q + k]); caja[3] = Math.max(caja[3], F[q + k + 1]);
    }
  }
}
function rellenos(p){ return guardado('T' + p, function(){
  var c = [], q = U[p + 5];
  for (var s = 0; s < U[p + 6]; s++) {
    var col = U[q], n = U[q + 1]; q += 2;
    triangulos(porColor(c, col), q, n, null); q += 6 * n;
  }
  return c;
}); }
function sombreados(p){ return guardado('H' + p, function(){
  var c = [], q = U[p + 5];
  for (var s = 0; s < U[p + 6]; s++) {
    var col = U[q], tr = U[q + 1], n = U[q + 2]; q += 3;
    var path = new Path2D(), caja = [Infinity, Infinity, -Infinity, -Infinity];
    triangulos(path, q, n, caja); q += 6 * n;
    c.push({ col: col, tr: tr, path: path, caja: caja });
  }
  return c;
}); }
function arcos(p){ return guardado('A' + p, function(){
  var c = [], q = U[p + 5];
  for (var s = 0; s < U[p + 6]; s++, q += 6) {
    var x = F[q], y = F[q + 1], r = F[q + 2], a0 = F[q + 3], b = F[q + 4];
    var path = porColor(c, U[q + 5]);
    path.moveTo(x + r * Math.cos(a0), y + r * Math.sin(a0));
    path.arc(x, y, r, a0, a0 + b, b < 0);
  }
  return c;
}); }
var glifos = new Map();
function glifo(g){
  var c = glifos.get(g);
  if (c) return c;
  var q = U[g], n = U[g + 1], deRayas = U[g + 2] === 1, path = new Path2D();
  if (deRayas) { for (var j = 0; j + 1 < n; j += 2) { path.moveTo(F[q + 2 * j], F[q + 2 * j + 1]); path.lineTo(F[q + 2 * j + 2], F[q + 2 * j + 3]); } }
  else { triangulos(path, q, Math.floor(n / 3), null); }
  c = { path: path, rayas: deRayas };
  glifos.set(g, c);
  return c;
}
function letras(p){ return guardado('G' + p, function(){
  var llenas = [], deRayas = [], q = U[p + 5];
  for (var s = 0; s < U[p + 6]; s++, q += 8) {
    var g = glifo(U[q + 7]);
    var m = new DOMMatrix([F[q + 2], F[q + 4], F[q + 3], F[q + 5], F[q], F[q + 1]]);
    porColor(g.rayas ? deRayas : llenas, U[q + 6]).addPath(g.path, m);
  }
  return { llenas: llenas, rayas: deRayas };
}); }

// ---- el patron de un sombreado, recortado a su contorno
function guiones(tr, pxl){
  var v = [];
  for (var i = 0; i < tr.length; i++) {
    var L = tr[i], on = L >= 0 ? 1 : 0, largo = L === 0 ? pxl : Math.abs(L);
    if (v.length && v[v.length - 1][1] === on) v[v.length - 1][0] += largo; else v.push([largo, on]);
  }
  if (!v.length) return null;
  if (v[0][1] === 0) v.unshift([0, 1]);
  if (v[v.length - 1][1] === 1) v.push([0, 0]);
  return v.map(function(x){ return x[0]; });
}
function patron(c, s, k, cl){
  var t = s.tr, cx = F[t], cy = F[t + 1], i0 = F[t + 2], i1 = F[t + 3], i2 = F[t + 4], i3 = F[t + 5];
  var esc = Math.max(F[t + 6], 1e-30), fam = U[t + 7], nf = U[t + 8];
  var det = i0 * i3 - i1 * i2;
  if (!isFinite(det) || Math.abs(det) < 1e-30) return;
  var pxl = vista.px / esc;
  var col = colores(cl).trama[s.col];
  var b = s.caja, esq = [[b[0], b[1]], [b[2], b[1]], [b[0], b[3]], [b[2], b[3]]].map(function(p){
    var dx = p[0] - cx, dy = p[1] - cy; return [i0 * dx + i1 * dy, i2 * dx + i3 * dy];
  });
  c.save();
  c.clip(s.path);
  var tono = false;
  c.transform(i3 / det, -i2 / det, -i1 / det, i0 / det, cx, cy);
  c.strokeStyle = css(col);
  c.lineWidth = pxl;
  c.lineCap = 'butt';
  for (var f = 0; f < nf && f < 32; f++) {
    var o = fam + f * 16, bx = F[o], by = F[o + 1], dx = F[o + 2], dy = F[o + 3], paso = F[o + 4], corr = F[o + 5], largo = F[o + 6], n = U[o + 7];
    var nx = -dy, ny = dx, smin = Infinity, smax = -Infinity, tmin = Infinity, tmax = -Infinity;
    esq.forEach(function(q){
      var vx = q[0] - bx, vy = q[1] - by, ss = vx * nx + vy * ny, tt = vx * dx + vy * dy;
      smin = Math.min(smin, ss); smax = Math.max(smax, ss); tmin = Math.min(tmin, tt); tmax = Math.max(tmax, tt);
    });
    var ap = Math.abs(paso);
    var k0 = Math.ceil(Math.min(smin / paso, smax / paso)), k1 = Math.floor(Math.max(smin / paso, smax / paso));
    // Rayas a menos de 3 pixeles (o demasiadas): un tono, no un muare.
    if (ap < pxl * 3 || k1 - k0 > 4000) { tono = true; continue; }
    var g = (n > 0 && largo > pxl) ? guiones(Array.prototype.slice.call(F, o + 8, o + 8 + Math.min(n, 8)), pxl) : null;
    c.setLineDash(g || []);
    for (var kk = k0; kk <= k1; kk++) {
      var ox = bx + nx * kk * paso, oy = by + ny * kk * paso;
      if (g) { var ph = (tmin - kk * corr) % largo; if (ph < 0) ph += largo; c.lineDashOffset = ph; }
      c.beginPath();
      c.moveTo(ox + dx * tmin, oy + dy * tmin);
      c.lineTo(ox + dx * tmax, oy + dy * tmax);
      c.stroke();
    }
  }
  c.restore();
  if (tono) { c.fillStyle = css(col, 0.3); c.fill(s.path); }
}

// ---- pintar
function ancho(g){
  if (!grosores || g <= 0 || g === D.normal) return 1;
  return Math.max(1, g / 100 * 96 / 25.4);
}
function matriz(o){
  var s = dpr / vista.px;
  return [s, 0, 0, -s, dpr * W / 2 + (o[0] - vista.cx) * s, dpr * H / 2 - (o[1] - vista.cy) * s];
}
function cajaVista(o){
  var mx = W / 2 * vista.px, my = H / 2 * vista.px;
  return [vista.cx - mx - o[0], vista.cy - my - o[1], vista.cx + mx - o[0], vista.cy + my - o[1]];
}
function pintar(){
  pedido = false;
  var c = ctx, cl = claro, P = colores(cl);
  c.setTransform(1, 0, 0, 1, 0, 0);
  c.fillStyle = cl ? '#fcfcf9' : '#21262f';
  c.fillRect(0, 0, cv.width, cv.height);
  c.lineJoin = 'round';
  // Las imagenes, debajo de todo.
  imagenes.forEach(function(x){
    var i = x.d;
    if (!capasOn[i.k] || !x.im.complete || !x.im.naturalWidth) return;
    var w = x.im.naturalWidth, h = x.im.naturalHeight;
    c.setTransform.apply(c, matriz([0, 0]));
    c.transform(i.u[0] / w, i.u[1] / w, -i.v[0] / h, -i.v[1] / h, i.e[0] + i.v[0], i.e[1] + i.v[1]);
    c.drawImage(x.im, 0, 0);
  });
  // Sin los datos aun no se pinta nada: lo guardado de cada tramo saldria vacio.
  var visibles = F.length ? D.grupos.filter(function(g){ return capasOn[g.k]; }) : [];
  var pasadas = [
    function(g, caja){ tramos(g.T, vista.px, caja, function(p){ rellenos(p).forEach(function(x){ c.fillStyle = P.relleno[x[0]]; c.fill(x[1]); }); }); },
    function(g, caja){ tramos(g.H, vista.px * 2, caja, function(p){ sombreados(p).forEach(function(s){ patron(c, s, p, cl); }); }); },
    function(g, caja){ c.lineWidth = ancho(g.g) * vista.px; c.lineCap = ancho(g.g) > 1.5 ? 'round' : 'butt';
      tramos(g.L, vista.px * 0.75, caja, function(p){ rayas(p).forEach(function(x){ c.strokeStyle = P.raya[x[0]]; c.stroke(x[1]); }); }); },
    function(g, caja){ c.lineWidth = ancho(g.g) * vista.px;
      tramos(g.A, vista.px * 0.75, caja, function(p){ arcos(p).forEach(function(x){ c.strokeStyle = P.raya[x[0]]; c.stroke(x[1]); }); }); },
    function(g, caja){ c.lineWidth = vista.px; c.lineCap = 'round';
      tramos(g.G, vista.px * 3, caja, function(p){ var l = letras(p);
        l.llenas.forEach(function(x){ c.fillStyle = P.raya[x[0]]; c.fill(x[1]); });
        l.rayas.forEach(function(x){ c.strokeStyle = P.raya[x[0]]; c.stroke(x[1]); }); }); }
  ];
  pasadas.forEach(function(pasada){
    visibles.forEach(function(g){
      c.setTransform.apply(c, matriz(g.o));
      pasada(g, cajaVista(g.o));
    });
  });
  encima();
}
function aPantalla(x, y){ return [W / 2 + (x - vista.cx) / vista.px, H / 2 - (y - vista.cy) / vista.px]; }
function medida(d){
  var dec = d >= 1000 ? 1 : d >= 100 ? 2 : 3;
  return d.toFixed(dec) + (D.unidad ? ' ' + D.unidad : '');
}
function encima(){
  var c = ctxE;
  c.setTransform(dpr, 0, 0, dpr, 0, 0);
  c.clearRect(0, 0, W, H);
  if (conAnot) {
    c.font = '12px "Segoe UI", system-ui, sans-serif';
    c.lineWidth = 1.5;
    D.marcos.forEach(function(m, i){
      var a = aPantalla(m[0], m[3]), b = aPantalla(m[2], m[1]);
      c.strokeStyle = '#3d8bff'; c.setLineDash([6, 4]);
      c.strokeRect(a[0], a[1], b[0] - a[0], b[1] - a[1]);
      c.setLineDash([]);
      c.fillStyle = '#3d8bff'; c.fillRect(a[0], a[1] - 18, 22, 18);
      c.fillStyle = '#fff'; c.fillText(String(i + 1), a[0] + 7, a[1] - 5);
    });
    c.strokeStyle = '#ff8a00'; c.fillStyle = '#ff8a00';
    D.cotas.forEach(function(k){
      var total = 0, ult = null;
      for (var i = 0; i + 3 < k.length; i += 2) {
        var a = aPantalla(k[i], k[i + 1]), b = aPantalla(k[i + 2], k[i + 3]);
        c.beginPath(); c.moveTo(a[0], a[1]); c.lineTo(b[0], b[1]); c.stroke();
        [a, b].forEach(function(p){ c.beginPath(); c.arc(p[0], p[1], 2.5, 0, 7); c.fill(); });
        var d = Math.hypot(k[i + 2] - k[i], k[i + 3] - k[i + 1]); total += d;
        var t = medida(d), w = c.measureText(t).width, mx = (a[0] + b[0]) / 2, my = (a[1] + b[1]) / 2;
        c.fillStyle = claro ? 'rgba(252,252,249,.85)' : 'rgba(33,38,47,.85)'; c.fillRect(mx + 2, my - 17, w + 6, 16);
        c.fillStyle = '#ff8a00'; c.fillText(t, mx + 5, my - 5);
        ult = b;
      }
      if (k.length > 4 && ult) c.fillText('Σ ' + medida(total), ult[0] + 6, ult[1] + 14);
    });
  }
  if (svg && D.anotado) {
    var a = D.anotado, k = a.u / vista.px;
    var tx = W / 2 + (a.cero[0] + a.caja[0] * a.u - vista.cx) / vista.px;
    var ty = H / 2 + (a.caja[1] * a.u + vista.cy - a.cero[1]) / vista.px;
    anot.style.transform = 'matrix(' + k + ',0,0,' + k + ',' + tx + ',' + ty + ')';
    anot.style.display = conAnot ? '' : 'none';
  }
}
function redibujar(){ if (!pedido) { pedido = true; requestAnimationFrame(pintar); } }

// ---- vista
function medir(){
  var r = lienzo.getBoundingClientRect();
  W = Math.max(1, r.width); H = Math.max(1, r.height); dpr = window.devicePixelRatio || 1;
  [cv, cvE].forEach(function(x){ x.width = Math.round(W * dpr); x.height = Math.round(H * dpr); });
}
function verTodo(){
  var v = D.vista, w = v[2] - v[0], h = v[3] - v[1];
  if (!(w > 0) || !(h > 0)) { w = h = 1; }
  vista.px = Math.max(w / W, h / H) * 1.06;
  vista.cx = (v[0] + v[2]) / 2; vista.cy = (v[1] + v[3]) / 2;
  redibujar();
}
function zoom(f, mx, my){
  var x = vista.cx + (mx - W / 2) * vista.px, y = vista.cy - (my - H / 2) * vista.px;
  vista.px = Math.min(Math.max(vista.px * f, 1e-9), 1e12);
  vista.cx = x - (mx - W / 2) * vista.px; vista.cy = y + (my - H / 2) * vista.px;
  redibujar();
}
lienzo.addEventListener('wheel', function(e){
  e.preventDefault();
  var r = lienzo.getBoundingClientRect();
  var d = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY;
  zoom(Math.pow(1.0015, d), e.clientX - r.left, e.clientY - r.top);
}, { passive: false });
var dedos = new Map(), antes = null;
lienzo.addEventListener('pointerdown', function(e){
  lienzo.setPointerCapture(e.pointerId); dedos.set(e.pointerId, [e.clientX, e.clientY]); antes = null;
  lienzo.classList.add('moviendo');
});
lienzo.addEventListener('pointermove', function(e){
  if (!dedos.has(e.pointerId)) return;
  var p = dedos.get(e.pointerId), dx = e.clientX - p[0], dy = e.clientY - p[1];
  dedos.set(e.pointerId, [e.clientX, e.clientY]);
  if (dedos.size === 2) {
    var v = Array.from(dedos.values()), dist = Math.hypot(v[0][0] - v[1][0], v[0][1] - v[1][1]);
    var r = lienzo.getBoundingClientRect();
    if (antes) zoom(antes / Math.max(dist, 1), (v[0][0] + v[1][0]) / 2 - r.left, (v[0][1] + v[1][1]) / 2 - r.top);
    antes = dist;
    return;
  }
  vista.cx -= dx * vista.px; vista.cy += dy * vista.px; redibujar();
});
function soltar(e){ dedos.delete(e.pointerId); antes = null; if (!dedos.size) lienzo.classList.remove('moviendo'); }
lienzo.addEventListener('pointerup', soltar);
lienzo.addEventListener('pointercancel', soltar);
lienzo.addEventListener('dblclick', verTodo);
window.addEventListener('resize', function(){ medir(); redibujar(); });

// ---- la barra y las capas
function boton(id, fn){ var b = document.getElementById(id); if (b) b.addEventListener('click', fn); return b; }
function tema(){ claro = !claro; document.body.className = claro ? 'claro' : 'oscuro'; redibujar(); }
var bGrosor = boton('b-grosor', function(){ grosores = !grosores; marcar(); redibujar(); });
var bAnot = boton('b-anot', function(){ conAnot = !conAnot; marcar(); redibujar(); });
var bCapas = boton('b-capas', function(){ panel.hidden = !panel.hidden; marcar(); });
boton('b-todo', verTodo);
boton('b-tema', tema);
function marcar(){
  if (bGrosor) bGrosor.classList.toggle('on', grosores);
  if (bAnot) bAnot.classList.toggle('on', conAnot);
  if (bCapas) bCapas.classList.toggle('on', !panel.hidden);
}
function hacerPanel(){
  panel.textContent = '';
  var fila = document.createElement('div'); fila.className = 'fila';
  [[D.t.todas, true], [D.t.ninguna, false]].forEach(function(x){
    var b = document.createElement('button'); b.textContent = x[0];
    b.onclick = function(){ capasOn = capasOn.map(function(){ return x[1]; }); hacerPanel(); redibujar(); };
    fila.appendChild(b);
  });
  panel.appendChild(fila);
  D.capas.forEach(function(c, i){
    var l = document.createElement('label'), k = document.createElement('input'), m = document.createElement('span'), t = document.createElement('span');
    k.type = 'checkbox'; k.checked = capasOn[i];
    k.onchange = function(){ capasOn[i] = k.checked; redibujar(); };
    m.className = 'muestra'; m.style.background = c.c === undefined ? 'transparent' : css(tinta(c.c, claro));
    t.textContent = c.n; l.title = c.n;
    l.appendChild(k); l.appendChild(m); l.appendChild(t); panel.appendChild(l);
  });
}
document.addEventListener('keydown', function(e){
  if (e.ctrlKey || e.metaKey || e.altKey) return;
  var k = e.key.toLowerCase();
  if (k === 'f' || k === 'home') verTodo();
  else if (k === 'b') tema();
  else if (k === 'l' && bCapas) bCapas.click();
  else if (k === 'w' && bGrosor) bGrosor.click();
  else if (k === 'a' && bAnot) bAnot.click();
  else if (k === '+' || k === '=') zoom(1 / 1.4, W / 2, H / 2);
  else if (k === '-') zoom(1.4, W / 2, H / 2);
  else if (k === 'escape') { panel.hidden = true; marcar(); }
});

// ---- cargar los datos
function deBase64(s){
  var b = atob(s.replace(/\s+/g, '')), a = new Uint8Array(b.length);
  for (var i = 0; i < b.length; i++) a[i] = b.charCodeAt(i);
  return a;
}
function avisar(t){ var a = document.getElementById('aviso'); a.textContent = t; a.hidden = false; }
if (svg && D.anotado) {
  svg.setAttribute('width', D.anotado.caja[2] - D.anotado.caja[0]);
  svg.setAttribute('height', D.anotado.caja[3] - D.anotado.caja[1]);
}
hacerPanel(); marcar(); medir(); verTodo();
var gz = deBase64(document.getElementById('plano-gz').textContent);
if (typeof DecompressionStream === 'undefined') { avisar(D.t.sin); return; }
new Response(new Blob([gz]).stream().pipeThrough(new DecompressionStream('gzip'))).arrayBuffer().then(function(b){
  F = new Float32Array(b); U = new Uint32Array(b);
  document.body.setAttribute('data-listo', '1');
  redibujar();
}, function(){ avisar(D.t.sin); });
})();
