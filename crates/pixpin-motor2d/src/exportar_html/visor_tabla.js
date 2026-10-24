function crearTabla(d,api){
"use strict";
var ANCHO=96, CAB=46, MAX_PINTADAS=5000, NS='http://www.w3.org/2000/svg';
// Los colores de las celdas citadas mientras se escribe una fórmula, en orden de aparición.
var COLORES_REF=['#1a73e8','#d93025','#8e24aa','#188038','#e8710a','#0097a7','#c2185b','#795548'];
// Ver [ExportarHtml.TABLAS_SOLO_VER]: con esto se mira, se elige, se copia y se raya; no se escribe.
var editable=document.body.dataset.tablaEditable!=='0';
var guion=d.querySelector('script.tabla');
var datos={};
try{ datos=JSON.parse(guion.textContent)||{}; }catch(e){ datos={}; }
var calc=Calculo.crear();
var estilos={}, anchos={}, nombreTabla=datos.nombre||'', protegida=!!datos.protegida;
(function(){
  var cs=datos.celdas||{}, k, p;
  for(k in cs){ p=Calculo.leer(k); if(p)calc.poner(p[0],p[1],String(cs[k])); }
  var es=datos.estilos||{};
  for(k in es){ p=Calculo.leer(k); if(p&&es[k])estilos[p[0]*1024+p[1]]=es[k]; }
  var an=datos.anchos||{};
  for(k in an){ var c=Calculo.columna(k); if(c>=0)anchos[c]=+an[k]; }
})();
var caja=d.querySelector('.tabla-caja'), dirEl=d.querySelector('.tabla-dir'), fx=d.querySelector('.tabla-fx-in');
if(!editable){ fx.readOnly=true; fx.placeholder='Solo lectura'; }
var svg=caja.querySelector('svg.tinta'), origen=svg?svg.querySelector('g.origen'):null, croquis=svg?svg.querySelector('#croquis'):null;
var vieja=caja.querySelector('table.calc'); if(vieja)caja.removeChild(vieja);
var tabla=document.createElement('table'); tabla.className='calc';
var marcas=document.createElement('div'); marcas.className='tabla-marcas';
// **La tabla va dentro de un mundo que se pasea y se amplía**, como un dibujo en el lienzo: así
// alrededor queda sitio para anotar, y se puede empezar a rayar fuera de las celdas.
var mundo=document.createElement('div'); mundo.className='tabla-mundo';
mundo.appendChild(tabla); mundo.appendChild(marcas); if(svg)mundo.appendChild(svg);
caja.appendChild(mundo);
var cabeza=null, cuerpo=null, filaCab=null, esq=null;
var B={f0:0,c0:0,f1:-1,c1:-1}, tds=[], zoom=1, activo=false, O={x:0,y:0};
// La vista: dónde cae la esquina de la tabla en la caja y a qué aumento. Ver [aplicarVista].
var V={x:16,y:16,z:1}, punteros=new Map(), gesto=null, hayLapiz=false, centrada=false;
var act={f:0,c:0}, ancla={f:0,c:0};
var editando=false, original='', mantener=false, refPuesta=null;
var hecho=[], rehecho=[], portapapeles=null;
var arrastrando=false, refArrastre=false, toque=null, ultimoToque=null;
var modo='mano', trazo=null;

// ---- El marco: lo escrito más uno alrededor ----
function limites(){
  var k=calc.caja();
  if(!k)return {f0:0,c0:0,f1:1,c1:1};
  var f0=Math.max(0,k[0]-1), c0=Math.max(0,k[1]-1);
  return {f0:f0,c0:c0,f1:Math.min(99999,k[2]+1,f0+MAX_PINTADAS-1),c1:Math.min(701,k[3]+1)};
}
function mismo(a,b){ return a.f0===b.f0&&a.c0===b.c0&&a.f1===b.f1&&a.c1===b.c1; }
/**
 * Si esta celda se deja cambiar: nada con la página en solo lectura; en una tabla protegida,
 * solo las marcadas como editables. Lo que dependa de ellas recalcula igual.
 */
function puedeEditar(f,c){ if(!editable)return false; if(!protegida)return true; var e=estilos[f*1024+c]; return !!(e&&e.e); }
function ancho(c){ return Math.round((anchos[c]||ANCHO)*zoom); }
function anchoTotal(){ var w=Math.round(CAB*zoom); for(var c=B.c0;c<=B.c1;c++)w+=ancho(c); return w; }
function celdaTd(f,c){ var r=tds[f-B.f0]; return r?(r[c-B.c0]||null):null; }
function nuevaTd(f,c){ var td=document.createElement('td'); td.dataset.f=f; td.dataset.c=c; return td; }
function thCol(c){ var h=document.createElement('th'); h.textContent=Calculo.letras(c); h.dataset.c=c; h.style.width=ancho(c)+'px'; return h; }
function filaNueva(f){
  var tr=cuerpo.insertRow(), n=document.createElement('th'), fila=[], c;
  n.textContent=f+1; n.dataset.f=f; tr.appendChild(n);
  for(c=B.c0;c<=B.c1;c++){ var td=nuevaTd(f,c); tr.appendChild(td); fila.push(td); }
  tds.push(fila);
  for(c=B.c0;c<=B.c1;c++)pintarCelda(f,c);
}
/**
 * Rehace la rejilla al marco que toca. Crecer por abajo o por la derecha —lo corriente al ir
 * llenando— añade lo que falta; cualquier otro cambio la rehace entera.
 */
function construir(){
  var nb=limites(), f, c;
  if(B.f1>=0&&nb.f0===B.f0&&nb.c0===B.c0&&nb.f1>=B.f1&&nb.c1>=B.c1){
    while(B.c1<nb.c1){
      B.c1++; filaCab.appendChild(thCol(B.c1));
      for(f=B.f0;f<=B.f1;f++){ var td=nuevaTd(f,B.c1); cuerpo.rows[f-B.f0].appendChild(td); tds[f-B.f0].push(td); pintarCelda(f,B.c1); }
    }
    while(B.f1<nb.f1){ B.f1++; filaNueva(B.f1); }
  } else {
    B={f0:nb.f0,c0:nb.c0,f1:nb.f0-1,c1:nb.c1};
    tabla.innerHTML=''; tds=[];
    cabeza=tabla.createTHead(); cuerpo=tabla.createTBody(); filaCab=cabeza.insertRow();
    esq=document.createElement('th'); esq.className='esq'; filaCab.appendChild(esq);
    for(c=B.c0;c<=B.c1;c++)filaCab.appendChild(thCol(c));
    while(B.f1<nb.f1){ B.f1++; filaNueva(B.f1); }
  }
  esq.style.width=esq.style.minWidth=Math.round(CAB*zoom)+'px';
  tabla.style.width=anchoTotal()+'px';
  marcarCabeceras(rango());
  medirTinta();
  pintarMarcas();
}
function rango(){ return {f:Math.min(act.f,ancla.f),c:Math.min(act.c,ancla.c),f2:Math.max(act.f,ancla.f),c2:Math.max(act.c,ancla.c)}; }
function enRango(r,f,c){ return f>=r.f&&f<=r.f2&&c>=r.c&&c<=r.c2; }
function clase(f,c){
  var td=celdaTd(f,c); if(!td)return;
  var r=rango(), k=td._b||'';
  if(enRango(r,f,c)&&(r.f!==r.f2||r.c!==r.c2))k+=' sel';
  if(f===act.f&&c===act.c)k+=' act';
  if(td.className!==k)td.className=k;
}
function pintarCelda(f,c){
  var td=celdaTd(f,c); if(!td)return;
  var raw=calc.crudo(f,c), t=raw?calc.texto(f,c):'', es=estilos[f*1024+c], v=raw?calc.valor(f,c):null;
  var al=es&&es.a?es.a:(v?(v.t==='n'?'d':(v.t==='b'||v.t==='e')?'c':'i'):'i');
  var b=al==='d'?'d':al==='c'?'c':'';
  if(es&&es.n)b+=(b?' ':'')+'n';
  if(v&&v.t==='e')b+=(b?' ':'')+'err';
  // Las celdas con fórmula, sombreadas: se ve de un vistazo qué se calcula y qué se escribió.
  if(Calculo.esFormula(raw))b+=(b?' ':'')+'f';
  if(protegida&&es&&es.e)b+=(b?' ':'')+'ed';
  td._b=b;
  if(td.textContent!==t)td.textContent=t;
  var fondo=es&&es.f&&/^#[0-9a-fA-F]{6}$/.test(es.f)?es.f:'';
  if(td.style.background!==fondo)td.style.background=fondo;
  clase(f,c);
}
/** Tras un cambio: el marco si se movió, lo tocado y todas las fórmulas, que pueden cambiar solas. */
function repintar(tocadas){
  var i;
  if(!mismo(limites(),B))construir();
  for(i=0;i<tocadas.length;i++)pintarCelda(tocadas[i][0],tocadas[i][1]);
  calc.cada(function(f,c,v){ if(v.length>1&&v[0]==='=')pintarCelda(f,c); });
  if(act.f<B.f0||act.f>B.f1||act.c<B.c0||act.c>B.c1||ancla.f<B.f0||ancla.f>B.f1||ancla.c<B.c0||ancla.c>B.c1)seleccionar(act.f,act.c,false,true);
  mostrarBarra(); estadoRango(); api.refrescar();
}
function marcarCabeceras(r){
  var i;
  if(!filaCab)return;
  for(i=1;i<filaCab.cells.length;i++){ var h=filaCab.cells[i], c=B.c0+i-1, m=c>=r.c&&c<=r.c2; if(h.classList.contains('marcada')!==m)h.classList.toggle('marcada',m); }
  for(i=0;i<cuerpo.rows.length;i++){ var th=cuerpo.rows[i].cells[0], f=B.f0+i, mm=f>=r.f&&f<=r.f2; if(th.classList.contains('marcada')!==mm)th.classList.toggle('marcada',mm); }
}
function seleccionar(f,c,extender,sinVer){
  f=Math.max(B.f0,Math.min(B.f1,f)); c=Math.max(B.c0,Math.min(B.c1,c));
  var viejo=rango(), x, y;
  act={f:f,c:c};
  if(!extender)ancla={f:f,c:c};
  else ancla={f:Math.max(B.f0,Math.min(B.f1,ancla.f)),c:Math.max(B.c0,Math.min(B.c1,ancla.c))};
  var nuevo=rango();
  for(x=Math.max(viejo.f,B.f0);x<=Math.min(viejo.f2,B.f1);x++)for(y=Math.max(viejo.c,B.c0);y<=Math.min(viejo.c2,B.c1);y++)clase(x,y);
  for(x=nuevo.f;x<=nuevo.f2;x++)for(y=nuevo.c;y<=nuevo.c2;y++)clase(x,y);
  marcarCabeceras(nuevo);
  mostrarBarra(); estadoRango();
  if(!sinVer)verCelda(f,c);
}
/** Pasea lo justo para que la celda se vea entera (moverse con las flechas o con Enter). */
function verCelda(f,c){
  var td=celdaTd(f,c); if(!td||!caja.clientWidth)return;
  var x0=V.x+td.offsetLeft*V.z, y0=V.y+td.offsetTop*V.z, x1=x0+td.offsetWidth*V.z, y1=y0+td.offsetHeight*V.z;
  var W=caja.clientWidth, H=caja.clientHeight-90, m=12, cambio=false;
  if(x0<m){ V.x+=m-x0; cambio=true; } else if(x1>W-m){ V.x-=x1-(W-m); cambio=true; }
  if(y0<m){ V.y+=m-y0; cambio=true; } else if(y1>H){ V.y-=y1-H; cambio=true; }
  if(cambio)aplicarVista();
}
function mostrarBarra(){
  var r=rango();
  dirEl.textContent=(r.f===r.f2&&r.c===r.c2)?Calculo.nombre(act.f,act.c):Calculo.nombre(r.f,r.c)+':'+Calculo.nombre(r.f2,r.c2);
  if(!editando){
    fx.value=calc.crudo(act.f,act.c);
    fx.readOnly=!puedeEditar(act.f,act.c);
    fx.placeholder=!editable?'Solo lectura':(fx.readOnly?'Celda protegida':'Valor o =SUMA(A1:A3)');
  }
}
/** Lo que dice la barra de estado de Excel: suma, promedio y cuenta de lo elegido. */
function estadoRango(){
  var r=rango();
  if(r.f===r.f2&&r.c===r.c2){ api.decir(''); return; }
  var s=0,n=0,llenas=0,f,c,f2=Math.min(r.f2,calc.filas()-1),c2=Math.min(r.c2,calc.cols()-1);
  for(f=r.f;f<=f2;f++)for(c=r.c;c<=c2;c++){ if(!calc.crudo(f,c))continue; llenas++; var v=calc.valor(f,c); if(v.t==='n'){s+=v.n;n++;} }
  api.decir(n?'Suma '+Calculo.general(s)+' · Promedio '+Calculo.general(s/n)+' · Cuenta '+llenas:(llenas?'Cuenta '+llenas:''));
}

// ---- Las celdas que cita la fórmula, cada una de su color ----
function pintarMarcas(){
  marcas.innerHTML='';
  if(!editando||fx.value[0]!=='=')return;
  var vistos={}, n=0;
  Calculo.referencias(fx.value).forEach(function(r){
    var k=r.f1+','+r.c1+','+r.f2+','+r.c2;
    if(!(k in vistos))vistos[k]=n++;
    var color=COLORES_REF[vistos[k]%COLORES_REF.length];
    var f1=Math.max(r.f1,B.f0), c1=Math.max(r.c1,B.c0), f2=Math.min(r.f2,B.f1), c2=Math.min(r.c2,B.c1);
    if(f1>f2||c1>c2)return;
    var a=celdaTd(f1,c1), b=celdaTd(f2,c2); if(!a||!b)return;
    var m=document.createElement('div'); m.className='tabla-marca';
    m.style.left=a.offsetLeft+'px'; m.style.top=a.offsetTop+'px';
    m.style.width=(b.offsetLeft+b.offsetWidth-a.offsetLeft)+'px';
    m.style.height=(b.offsetTop+b.offsetHeight-a.offsetTop)+'px';
    m.style.borderColor=color; m.style.background=color+'1f';
    m.dataset.ref=k;
    marcas.appendChild(m);
  });
}

// ---- Escribir, con deshacer ----
function escribir(cambios,estilosNuevos){
  if(!editable)return;
  var paso={celdas:[],estilos:[]}, tocadas=[], i, protegidas=0;
  for(i=0;i<cambios.length;i++){
    var f=cambios[i][0], c=cambios[i][1], t=cambios[i][2];
    if(f<0||c<0||f>=100000||c>=702)continue;
    if(!puedeEditar(f,c)){ if(calc.crudo(f,c)!==t)protegidas++; continue; }
    var antes=calc.crudo(f,c); if(antes===t)continue;
    paso.celdas.push([f*1024+c,antes,t]); calc.poner(f,c,t); tocadas.push([f,c]);
  }
  if(estilosNuevos)for(i=0;i<estilosNuevos.length;i++){
    var k=estilosNuevos[i][0], a=estilos[k]||null, dsp=estilosNuevos[i][1];
    if(JSON.stringify(a)===JSON.stringify(dsp))continue;
    if(!puedeEditar(Math.floor(k/1024),k%1024))continue;
    // La marca de editable no la cambia quien recibe la página.
    if(dsp&&a&&a.e)dsp.e=true; else if(dsp)delete dsp.e;
    paso.estilos.push([k,a,dsp]); if(dsp)estilos[k]=dsp; else delete estilos[k];
    tocadas.push([Math.floor(k/1024),k%1024]);
  }
  if(protegidas)api.decir(protegidas===1?'Esa celda está protegida':protegidas+' celdas protegidas no se cambiaron');
  if(!paso.celdas.length&&!paso.estilos.length)return;
  anotar(paso);
  repintar(tocadas);
}
function anotar(paso){ hecho.push(paso); if(hecho.length>200)hecho.shift(); rehecho.length=0; api.refrescar(); }
/** Un paso es de celdas o de tinta; los dos van en la misma pila, como se hicieron. */
function aplicar(paso,adelante){
  if(paso.tinta==='pinta'){
    if(adelante)croquis.appendChild(paso.raya); else if(paso.raya.parentNode)croquis.removeChild(paso.raya);
    api.refrescar(); return;
  }
  if(paso.tinta==='borra'){
    if(adelante){ paso.antes=paso.raya.nextSibling; if(paso.raya.parentNode)croquis.removeChild(paso.raya); }
    else croquis.insertBefore(paso.raya,paso.antes&&paso.antes.parentNode===croquis?paso.antes:null);
    api.refrescar(); return;
  }
  var tocadas=[], i;
  var cs=adelante?paso.celdas:paso.celdas.slice().reverse();
  for(i=0;i<cs.length;i++){ var k=cs[i][0]; calc.poner(Math.floor(k/1024),k%1024,adelante?cs[i][2]:cs[i][1]); tocadas.push([Math.floor(k/1024),k%1024]); }
  for(i=0;i<paso.estilos.length;i++){ var e=paso.estilos[i], x=adelante?e[2]:e[1]; if(x)estilos[e[0]]=x; else delete estilos[e[0]]; tocadas.push([Math.floor(e[0]/1024),e[0]%1024]); }
  repintar(tocadas);
}
function deshacer(){ if(editando)cancelar(); var p=hecho.pop(); if(!p)return; aplicar(p,false); rehecho.push(p); api.refrescar(); }
function rehacer(){ if(editando)cancelar(); var p=rehecho.pop(); if(!p)return; aplicar(p,true); hecho.push(p); api.refrescar(); }

// ---- La tinta: lápiz, resaltador y borrador encima de las celdas ----
function r3(x){return Math.round(x*1000)/1000;}
/**
 * La capa mide lo que mide la tabla y va en unidades de zoom 1 **contadas desde A1**, aunque A1
 * no se vea: así lo rayado se queda en su celda al ampliar y al crecer el marco por arriba.
 */
function medirTinta(){
  if(!svg||!origen)return;
  var td=celdaTd(B.f0,B.c0), antes=0, c;
  for(c=0;c<B.c0;c++)antes+=anchos[c]||ANCHO;
  O={x:td?td.offsetLeft-antes:CAB, y:td?td.offsetTop-B.f0*td.offsetHeight:26};
  origen.setAttribute('transform','translate('+r3(O.x)+' '+r3(O.y)+')');
}
/** Del sitio del puntero en la ventana al punto del mundo, contado desde A1. */
function donde(e){ var r=caja.getBoundingClientRect(); return {x:r3((e.clientX-r.left-V.x)/V.z-O.x), y:r3((e.clientY-r.top-V.y)/V.z-O.y)}; }
function anadirPunto(t,p){
  var pts=t.puntos, n=pts.length;
  pts.push(p);
  if(n===0){t.abierto='M '+p.x+' '+p.y;return;}
  var a=pts[n-1];
  t.abierto+=(n===1?' L ':' Q '+a.x+' '+a.y+' ')+r3((a.x+p.x)/2)+' '+r3((a.y+p.y)/2);
}
function pintarTrazo(t){
  var u=t.puntos[t.puntos.length-1];
  t.setAttribute('d',t.abierto+(t.puntos.length>1?' L '+u.x+' '+u.y:''));
}
function muestras(e){ var lote=(e.getCoalescedEvents&&e.getCoalescedEvents())||[]; return lote.length?lote:[e]; }
/** Los puntos de una raya: los que se trazaron, o los que se leen de su camino si vino guardada. */
function puntosDe(r){
  if(r.puntos&&r.puntos.length)return r.puntos;
  var n=(r.getAttribute('d')||'').match(/-?[0-9.]+/g)||[], pts=[];
  for(var i=0;i+1<n.length;i+=2)pts.push({x:+n[i],y:+n[i+1]});
  r.puntos=pts; return pts;
}
function borrarEn(p){
  var rayas=[].slice.call(croquis.children);
  for(var i=0;i<rayas.length;i++){
    var r=rayas[i], pts=puntosDe(r), gordo=(+r.getAttribute('stroke-width')||2)/2+10/V.z;
    for(var j=0;j<pts.length;j++){
      if(Math.hypot(pts[j].x-p.x,pts[j].y-p.y)<=gordo){
        anotar({tinta:'borra',raya:r,antes:r.nextSibling});
        croquis.removeChild(r); break;
      }
    }
  }
}
// La raya la empieza, sigue y suelta el manejador de la caja: así se raya también fuera de la tabla.
function empezarTrazo(e){
  var p=donde(e);
  if(modo==='goma'){ borrarEn(p); trazo=null; return; }
  var marca=modo==='marcador';
  trazo=document.createElementNS(NS,'path');
  trazo.setAttribute('fill','none');
  trazo.setAttribute('stroke',api.color());
  // El grosor se mide en la pantalla del momento: una raya fina de lejos sigue siendo fina.
  trazo.setAttribute('stroke-width',r3((marca?api.grosor()*3:api.grosor())/V.z));
  trazo.setAttribute('stroke-linecap','round');
  trazo.setAttribute('stroke-linejoin','round');
  if(marca){ trazo.setAttribute('stroke-opacity','0.45'); trazo.setAttribute('class','marca'); }
  trazo.puntos=[]; anadirPunto(trazo,p); pintarTrazo(trazo);
  croquis.appendChild(trazo);
}
function seguirTrazo(e){
  if(modo==='goma'){ borrarEn(donde(e)); return; }
  if(!trazo)return;
  var lote=muestras(e);
  for(var i=0;i<lote.length;i++)anadirPunto(trazo,donde(lote[i]));
  pintarTrazo(trazo);
}
function soltarTrazo(){
  if(!trazo)return;
  if(trazo.puntos.length<2){ if(trazo.parentNode)croquis.removeChild(trazo); }
  else anotar({tinta:'pinta',raya:trazo});
  trazo=null;
}
function rayas(){
  var s='';
  if(!croquis)return s;
  [].forEach.call(croquis.children,function(r){
    s+='<'+r.localName;
    for(var i=0;i<r.attributes.length;i++){
      var a=r.attributes[i];
      s+=' '+a.name+'="'+String(a.value).replace(/&/g,'&amp;').replace(/"/g,'&quot;')+'"';
    }
    s+='/>\n';
  });
  return s;
}

// ---- Editar en la barra de fórmula ----
function editar(inicial){
  if(!editable)return;
  if(!puedeEditar(act.f,act.c)){ api.decir('Esta celda está protegida'); return; }
  editando=true; original=calc.crudo(act.f,act.c); refPuesta=null;
  fx.value=inicial!==undefined?inicial:original;
  fx.focus();
  try{ fx.setSelectionRange(fx.value.length,fx.value.length); }catch(e){}
  pintarMarcas();
}
function confirmar(){
  if(!editando)return;
  editando=false; refPuesta=null;
  var t=fx.value;
  if(t!==original)escribir([[act.f,act.c,t]]); else pintarCelda(act.f,act.c);
  pintarMarcas();
}
function cancelar(){ editando=false; refPuesta=null; fx.value=original; pintarCelda(act.f,act.c); pintarMarcas(); caja.focus({preventScroll:true}); }
function vivo(){ var td=celdaTd(act.f,act.c); if(td&&editando)td.textContent=fx.value; pintarMarcas(); }
fx.addEventListener('focus',function(){ if(!editando&&puedeEditar(act.f,act.c)){ editando=true; original=calc.crudo(act.f,act.c); refPuesta=null; pintarMarcas(); } });
fx.addEventListener('input',vivo);
fx.addEventListener('click',pintarMarcas);
fx.addEventListener('keyup',pintarMarcas);
fx.addEventListener('keydown',function(e){
  if(e.key==='Enter'){ e.preventDefault(); confirmar(); seleccionar(act.f+(e.shiftKey?-1:1),act.c,false); caja.focus({preventScroll:true}); }
  else if(e.key==='Tab'){ e.preventDefault(); confirmar(); seleccionar(act.f,act.c+(e.shiftKey?-1:1),false); caja.focus({preventScroll:true}); }
  else if(e.key==='Escape'){ e.preventDefault(); cancelar(); }
});
fx.addEventListener('blur',function(){
  setTimeout(function(){ if(editando&&document.activeElement!==fx&&!mantener)confirmar(); mantener=false; },0);
});
/** Con una fórmula a medias, tocar una celda **escribe su dirección**, como en Excel. */
function puedePonerRef(){
  if(!editando||fx.value[0]!=='=')return false;
  if(refPuesta&&refPuesta.valor===fx.value)return true;
  var pos=fx.selectionStart==null?fx.value.length:fx.selectionStart;
  return /[=+\-*\/^&(;,:<>]\s*$/.test(fx.value.slice(0,pos));
}
function ponerRef(f,c,extender){
  var v=fx.value, texto;
  if(extender&&refPuesta&&refPuesta.valor===v){
    texto=Calculo.nombre(refPuesta.f,refPuesta.c)+':'+Calculo.nombre(f,c);
    v=v.slice(0,refPuesta.d)+texto+v.slice(refPuesta.h);
    refPuesta.h=refPuesta.d+texto.length;
  } else {
    var pos=fx.selectionStart==null?v.length:fx.selectionStart, fin=fx.selectionEnd==null?pos:fx.selectionEnd;
    if(refPuesta&&refPuesta.valor===v){ pos=refPuesta.d; fin=refPuesta.h; }
    texto=Calculo.nombre(f,c);
    v=v.slice(0,pos)+texto+v.slice(fin);
    refPuesta={d:pos,h:pos+texto.length,f:f,c:c};
  }
  fx.value=v; refPuesta.valor=v; mantener=true;
  fx.focus();
  try{ fx.setSelectionRange(refPuesta.h,refPuesta.h); }catch(e){}
  vivo();
}

// ---- Dedo, ratón y cabeceras ----
function celdaDe(el){ var td=el&&el.closest?el.closest('td'):null; return td&&td.dataset.f!=null?[+td.dataset.f,+td.dataset.c]:null; }
function aplicarVista(){ mundo.style.transform='translate('+r3(V.x)+'px,'+r3(V.y)+'px) scale('+r3(V.z)+')'; }
/** Amplía alrededor de un punto de la ventana: lo que hay debajo no se mueve. */
function zoomEn(cx,cy,factor){
  var r=caja.getBoundingClientRect(), z=Math.max(0.2,Math.min(4,V.z*factor));
  var wx=(cx-r.left-V.x)/V.z, wy=(cy-r.top-V.y)/V.z;
  V.z=z; V.x=cx-r.left-wx*z; V.y=cy-r.top-wy*z; aplicarVista();
}
/** Centra la tabla: entera si cabe, y si no, a un tamaño que todavía se lea. */
function encajar(){
  var W=caja.clientWidth, H=caja.clientHeight, w=tabla.offsetWidth, h=tabla.offsetHeight;
  if(!W||!H||!w){ V={x:16,y:16,z:1}; aplicarVista(); return; }
  var z=Math.max(0.45,Math.min(1,(W-32)/w,(H-110)/h));
  V.z=z; V.x=w*z>W-32?16:(W-w*z)/2; V.y=Math.max(16,(H-90-h*z)/2);
  aplicarVista();
}
function losPunteros(){ return Array.from(punteros.values()); }
/**
 * **Un solo manejador para toda la caja**, dentro y fuera de la tabla:
 * - con lápiz o resaltador en la mano, se raya donde se apoye (si ya se usó un lápiz de verdad,
 *   el dedo pasea, como en el lienzo);
 * - con la mano: el ratón elige celdas arrastrando sobre ellas y pasea arrastrando fuera; el dedo
 *   pasea, toca para elegir, dos toques editan y mantener elige un rango;
 * - dos dedos amplían y pasean; la rueda pasea y con Ctrl amplía.
 */
caja.addEventListener('mousedown',function(e){ if(editando&&puedePonerRef())e.preventDefault(); });
caja.addEventListener('pointerdown',function(e){
  punteros.set(e.pointerId,{x:e.clientX,y:e.clientY});
  if(e.pointerType==='pen')hayLapiz=true;
  if(punteros.size===2&&e.pointerType!=='mouse'){
    if(gesto&&gesto.timer)clearTimeout(gesto.timer);
    if(trazo){ if(trazo.parentNode)croquis.removeChild(trazo); trazo=null; }
    mantener=false;
    var ps=losPunteros();
    gesto={tipo:'pellizco',d0:Math.hypot(ps[0].x-ps[1].x,ps[0].y-ps[1].y)||1,mx:(ps[0].x+ps[1].x)/2,my:(ps[0].y+ps[1].y)/2,v:{x:V.x,y:V.y,z:V.z}};
    return;
  }
  if(punteros.size>1)return;
  try{ caja.setPointerCapture(e.pointerId); }catch(x){}
  if(modo!=='mano'&&svg&&croquis&&!(e.pointerType==='touch'&&hayLapiz)&&!(e.pointerType==='mouse'&&e.button!==0)){
    e.preventDefault(); empezarTrazo(e); gesto={tipo:'tinta'}; return;
  }
  var p=celdaDe(e.target), th=e.target.closest?e.target.closest('th'):null;
  if(e.pointerType==='mouse'&&e.button===0&&p&&modo==='mano'){
    if(puedePonerRef()){ e.preventDefault(); ponerRef(p[0],p[1],e.shiftKey); gesto={tipo:'refs'}; return; }
    if(editando)confirmar();
    seleccionar(p[0],p[1],e.shiftKey); gesto={tipo:'celdas'};
    caja.focus({preventScroll:true}); e.preventDefault();
    return;
  }
  var g={tipo:'pendiente',x0:e.clientX,y0:e.clientY,vx:V.x,vy:V.y,celda:modo==='mano'?p:null,th:modo==='mano'?th:null,ref:!!p&&modo==='mano'&&puedePonerRef()};
  gesto=g;
  if(g.ref)mantener=true;
  if(g.celda&&e.pointerType!=='mouse'){
    g.timer=setTimeout(function(){
      if(gesto!==g||g.tipo!=='pendiente')return;
      if(g.ref){ ponerRef(p[0],p[1],false); g.tipo='refs'; }
      else { if(editando)confirmar(); seleccionar(p[0],p[1],false,true); g.tipo='celdas'; api.decir('Arrastra para elegir varias celdas'); }
      if(navigator.vibrate)try{navigator.vibrate(12);}catch(x){}
    },430);
  }
});
caja.addEventListener('pointermove',function(e){
  if(punteros.has(e.pointerId))punteros.set(e.pointerId,{x:e.clientX,y:e.clientY});
  var g=gesto; if(!g)return;
  if(g.tipo==='pellizco'){
    if(punteros.size<2)return;
    var ps=losPunteros(), d=Math.hypot(ps[0].x-ps[1].x,ps[0].y-ps[1].y)||1;
    var mx=(ps[0].x+ps[1].x)/2, my=(ps[0].y+ps[1].y)/2, r=caja.getBoundingClientRect(), v=g.v;
    var z=Math.max(0.2,Math.min(4,v.z*d/g.d0)), wx=(g.mx-r.left-v.x)/v.z, wy=(g.my-r.top-v.y)/v.z;
    V.z=z; V.x=mx-r.left-wx*z; V.y=my-r.top-wy*z; aplicarVista();
    return;
  }
  if(g.tipo==='tinta'){ seguirTrazo(e); return; }
  if(g.tipo==='pendiente'){
    if(Math.hypot(e.clientX-g.x0,e.clientY-g.y0)<=6)return;
    if(g.timer)clearTimeout(g.timer);
    mantener=false; g.tipo='pan'; caja.classList.add('agarrada');
  }
  if(g.tipo==='pan'){ V.x=g.vx+(e.clientX-g.x0); V.y=g.vy+(e.clientY-g.y0); aplicarVista(); return; }
  var q=celdaDe(document.elementFromPoint(e.clientX,e.clientY)); if(!q)return;
  if(g.tipo==='refs')ponerRef(q[0],q[1],true); else if(g.tipo==='celdas')seleccionar(q[0],q[1],true,true);
});
function soltarPuntero(e){
  punteros.delete(e.pointerId);
  var g=gesto; if(!g)return;
  if(g.tipo==='pellizco'){ if(punteros.size===0)gesto=null; return; }
  if(g.timer)clearTimeout(g.timer);
  if(g.tipo==='tinta')soltarTrazo();
  else if(g.tipo==='pendiente'&&e.type==='pointerup'){
    if(g.celda){
      var t=g.celda, ahora=Date.now();
      if(g.ref)ponerRef(t[0],t[1],false);
      else if(ultimoToque&&ultimoToque.f===t[0]&&ultimoToque.c===t[1]&&ahora-ultimoToque.t<380){ ultimoToque=null; seleccionar(t[0],t[1],false); editar(); }
      else { if(editando)confirmar(); ultimoToque={f:t[0],c:t[1],t:ahora}; seleccionar(t[0],t[1],false,true); }
    } else if(g.th&&!g.th.classList.contains('esq')){
      if(editando)confirmar();
      if(g.th.dataset.c!=null){ var c=+g.th.dataset.c; ancla={f:B.f0,c:c}; seleccionar(B.f1,c,true,true); }
      else if(g.th.dataset.f!=null){ var f=+g.th.dataset.f; ancla={f:f,c:B.c0}; seleccionar(f,B.c1,true,true); }
    }
  } else if(g.tipo==='pendiente')mantener=false;
  caja.classList.remove('agarrada');
  gesto=null;
}
caja.addEventListener('pointerup',soltarPuntero);
caja.addEventListener('pointercancel',soltarPuntero);
caja.addEventListener('wheel',function(e){
  e.preventDefault();
  if(e.ctrlKey||e.metaKey)zoomEn(e.clientX,e.clientY,Math.exp(-e.deltaY*0.0015));
  else { V.x-=e.deltaX; V.y-=e.deltaY; aplicarVista(); }
},{passive:false});
tabla.addEventListener('dblclick',function(e){ var p=celdaDe(e.target); if(p&&modo==='mano'){ seleccionar(p[0],p[1],false); editar(); } });

// ---- Teclado ----
function mover(df,dc,ext){ seleccionar(act.f+df,act.c+dc,ext); }
function tecla(e){
  if(!activo)return false;
  if(e.target===fx)return true;
  var ctrl=e.ctrlKey||e.metaKey, k=e.key||'', l=k.toLowerCase();
  if(ctrl){
    if(l==='z'&&!e.shiftKey){ e.preventDefault(); deshacer(); return true; }
    if(l==='y'||(l==='z'&&e.shiftKey)){ e.preventDefault(); rehacer(); return true; }
    if(l==='c'||l==='x'||l==='v')return true;
    if(l==='a'){ e.preventDefault(); ancla={f:B.f0,c:B.c0}; seleccionar(B.f1,B.c1,true,true); return true; }
    if(editable&&l==='b'){ e.preventDefault(); accion('t-negrita'); return true; }
    if(editable&&l==='d'){ e.preventDefault(); rellenar(true); return true; }
    return false;
  }
  if(e.altKey)return false;
  switch(k){
    case 'ArrowUp': e.preventDefault(); mover(-1,0,e.shiftKey); return true;
    case 'ArrowDown': e.preventDefault(); mover(1,0,e.shiftKey); return true;
    case 'ArrowLeft': e.preventDefault(); mover(0,-1,e.shiftKey); return true;
    case 'ArrowRight': e.preventDefault(); mover(0,1,e.shiftKey); return true;
    case 'Tab': e.preventDefault(); mover(0,e.shiftKey?-1:1,false); return true;
    case 'Home': e.preventDefault(); seleccionar(act.f,B.c0,e.shiftKey); return true;
    case 'PageDown': case 'PageUp': return false;
  }
  // En solo lectura las letras vuelven a ser atajos del armazón (P lápiz, E borrador…).
  if(!editable)return false;
  if(!puedeEditar(act.f,act.c)&&(k==='Enter'||k==='F2'||(k.length===1&&modo==='mano'))){
    e.preventDefault(); api.decir('Esta celda está protegida'); return true;
  }
  switch(k){
    case 'Enter': e.preventDefault(); if(e.shiftKey)mover(-1,0,false); else editar(); return true;
    case 'F2': e.preventDefault(); editar(); return true;
    case 'Delete': case 'Backspace': e.preventDefault(); borrar(); return true;
  }
  if(k.length===1&&modo==='mano'){ e.preventDefault(); editar(k); return true; }
  return false;
}
function borrar(){
  var r=rango(), cambios=[];
  calc.cada(function(f,c){ if(enRango(r,f,c))cambios.push([f,c,'']); });
  escribir(cambios);
}
function rellenar(abajo){
  var r=rango(), cambios=[], f, c, raw;
  if(abajo){ for(c=r.c;c<=r.c2;c++){ raw=calc.crudo(r.f,c); for(f=r.f+1;f<=r.f2;f++)cambios.push([f,c,Calculo.esFormula(raw)?Calculo.desplazar(raw,f-r.f,0):raw]); } }
  else { for(f=r.f;f<=r.f2;f++){ raw=calc.crudo(f,r.c); for(c=r.c+1;c<=r.c2;c++)cambios.push([f,c,Calculo.esFormula(raw)?Calculo.desplazar(raw,0,c-r.c):raw]); } }
  escribir(cambios);
}

// ---- Portapapeles ----
function escaparHtml(s){ return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }
function copiaDe(r){
  var valores=[], crudos=[], html='<meta charset="utf-8"><'+'table>', f, c;
  for(f=r.f;f<=r.f2;f++){
    var fv=[], fc=[]; html+='<tr>';
    for(c=r.c;c<=r.c2;c++){
      var t=calc.texto(f,c), raw=calc.crudo(f,c), es=estilos[f*1024+c];
      fv.push(t); fc.push(raw);
      html+='<td'+(Calculo.esFormula(raw)?' data-sheets-formula="'+escaparHtml(Calculo.aR1c1(raw,f,c))+'"':'')+(es&&es.n?' style="font-weight:bold"':'')+'>'+escaparHtml(t).replace(/\n/g,'<br>')+'</td>';
    }
    valores.push(fv); crudos.push(fc); html+='</tr>';
  }
  html+='</'+'table>';
  return {tsv:Calculo.aTsv(valores), html:html, crudos:crudos, f:r.f, c:r.c};
}
document.addEventListener('copy',function(e){
  if(!activo||document.activeElement===fx||!e.clipboardData)return;
  var r=rango(), k=copiaDe(r);
  e.clipboardData.setData('text/plain',k.tsv); e.clipboardData.setData('text/html',k.html); e.preventDefault();
  portapapeles=k;
  api.decir('Copiado: '+(r.f2-r.f+1)+' × '+(r.c2-r.c+1));
});
document.addEventListener('cut',function(e){
  if(!activo||!editable||document.activeElement===fx||!e.clipboardData)return;
  var k=copiaDe(rango());
  e.clipboardData.setData('text/plain',k.tsv); e.clipboardData.setData('text/html',k.html); e.preventDefault();
  portapapeles=k; borrar(); api.decir('Cortado');
});
/** El número exacto que trae la hoja de origen, si lo que se ve es ese número redondeado. */
function exacto(td,txt){
  if(txt.indexOf('%')>=0||Calculo.numero(txt)===null)return null;
  var xn=td.getAttribute('x:num');
  if(xn&&Calculo.numero(xn)!==null)return xn;
  var sv=td.getAttribute('data-sheets-value');
  if(sv){ try{ var o=JSON.parse(sv); if(o&&o['1']===3&&typeof o['3']==='number')return String(o['3']); }catch(e){} }
  return null;
}
function deHtml(html){
  var doc; try{ doc=new DOMParser().parseFromString(html,'text/html'); }catch(e){ return null; }
  var t=doc.querySelector('table'); if(!t)return null;
  var filas=[];
  [].forEach.call(t.querySelectorAll('tr'),function(tr){
    var fila=[];
    [].forEach.call(tr.children,function(td){
      if(td.tagName!=='TD'&&td.tagName!=='TH')return;
      var fs=td.getAttribute('data-sheets-formula'), fe=td.getAttribute('x:fmla');
      var st=(td.getAttribute('style')||'').toLowerCase();
      var neg=/font-weight:\s*(bold|[6-9]00)/.test(st)||!!td.querySelector('b,strong')||/font-weight:\s*(bold|[6-9]00)/i.test(td.innerHTML);
      var h=td.innerHTML.replace(/<br\b[^>]*>/gi,'\u0000').replace(/<[^>]+>/g,'').replace(/[ \t\r\n]+/g,' ');
      var ta=document.createElement('textarea'); ta.innerHTML=h;
      var txt=ta.value.split('\u0000').map(function(s){return s.trim();}).join('\n').trim();
      if(fs&&fs[0]==='=')fila.push({texto:fs,r1c1:true,negrita:neg});
      else if(fe&&fe[0]==='=')fila.push({texto:fe,negrita:neg});
      else if(txt[0]==='=')fila.push({texto:"'"+txt,negrita:neg});
      else fila.push({texto:exacto(td,txt)||txt,negrita:neg});
      var span=parseInt(td.getAttribute('colspan')||'1',10)||1;
      for(var k=1;k<Math.min(span,51);k++)fila.push({texto:''});
    });
    filas.push(fila);
  });
  return filas;
}
function pegarBloque(html,texto){
  if(!editable)return false;
  var bloque=null, origenCopia=null;
  if(portapapeles&&texto&&texto.replace(/\r?\n$/,'')===portapapeles.tsv){
    bloque=portapapeles.crudos.map(function(fila){ return fila.map(function(x){ return {texto:x}; }); });
    origenCopia=[portapapeles.f,portapapeles.c];
  }
  if(!bloque&&html)bloque=deHtml(html);
  if(!bloque&&texto)bloque=Calculo.deTsv(texto).map(function(fila){ return fila.map(function(x){ return {texto:x}; }); });
  if(!bloque||!bloque.length)return false;
  if(editando){ editando=false; refPuesta=null; pintarMarcas(); }
  var cambios=[], negritas=[], ff=act.f, cc=act.c, ultF=ff, ultC=cc;
  for(var i=0;i<bloque.length;i++)for(var j=0;j<bloque[i].length;j++){
    var p=bloque[i][j], f=ff+i, c=cc+j, t=p.texto;
    if(f>=100000||c>=702)continue;
    if(p.r1c1)t=Calculo.r1c1(t,f,c);
    else if(origenCopia&&Calculo.esFormula(t))t=Calculo.desplazar(t,f-(origenCopia[0]+i),c-(origenCopia[1]+j));
    cambios.push([f,c,t]);
    if(p.negrita){ var k=f*1024+c, a=estilos[k]; if(!(a&&a.n)){ var n={}; for(var q in a||{})n[q]=a[q]; n.n=true; negritas.push([k,n]); } }
    ultF=Math.max(ultF,f); ultC=Math.max(ultC,c);
  }
  escribir(cambios,negritas);
  ancla={f:ff,c:cc}; seleccionar(ultF,ultC,true,true);
  api.decir('Pegado: '+bloque.length+' × '+(ultC-cc+1));
  return true;
}
document.addEventListener('paste',function(e){
  if(!activo||!editable||!e.clipboardData)return;
  var html=e.clipboardData.getData('text/html')||'', texto=e.clipboardData.getData('text/plain')||'';
  var varias=/[\t\n]/.test(texto.replace(/\r?\n$/,''))||/<table/i.test(html);
  if(document.activeElement===fx&&!varias)return;
  e.preventDefault();
  pegarBloque(html,texto);
});
function copiarConBoton(){
  var ok=false;
  try{ ok=document.execCommand('copy'); }catch(e){}
  if(!ok&&navigator.clipboard&&navigator.clipboard.writeText){
    var k=copiaDe(rango()); portapapeles=k;
    navigator.clipboard.writeText(k.tsv).then(function(){ api.decir('Copiado'); },function(){ api.decir('No se pudo copiar'); });
  }
}
function pegarConBoton(){
  var c=navigator.clipboard;
  if(c&&c.read){
    c.read().then(function(items){
      var html=null, texto=null, esperas=[];
      items.forEach(function(it){
        if(it.types.indexOf('text/html')>=0)esperas.push(it.getType('text/html').then(function(b){return b.text();}).then(function(t){html=t;}));
        if(it.types.indexOf('text/plain')>=0)esperas.push(it.getType('text/plain').then(function(b){return b.text();}).then(function(t){texto=t;}));
      });
      return Promise.all(esperas).then(function(){ if(!pegarBloque(html,texto))api.decir('No hay nada que pegar'); });
    }).catch(function(){ leerTexto(); });
  } else leerTexto();
  function leerTexto(){
    if(c&&c.readText) c.readText().then(function(t){ if(!pegarBloque(null,t))api.decir('No hay nada que pegar'); },function(){ aviso(); });
    else aviso();
  }
  function aviso(){ api.decir('Mantén pulsada la barra de fórmula y elige Pegar'); fx.focus(); }
}
/** El CSV lleva solo lo escrito: el rectángulo que ocupa, sin márgenes. */
function bajarCsv(){
  var k=calc.caja()||[0,0,0,0], lineas=[];
  for(var f=k[0];f<=k[2];f++){
    var fila=[];
    for(var c=k[1];c<=k[3];c++){ var t=calc.texto(f,c); fila.push(/[",\n\r;]/.test(t)?'"'+t.replace(/"/g,'""')+'"':t); }
    lineas.push(fila.join(','));
  }
  var a=document.createElement('a');
  a.href=URL.createObjectURL(new Blob(['\ufeff'+lineas.join('\r\n')],{type:'text/csv'}));
  a.download=((document.body.dataset.nombre||nombreTabla||'tabla').replace(/[\\/:*?"<>|]+/g,'-').trim()||'tabla')+'.csv';
  document.body.appendChild(a); a.click(); document.body.removeChild(a);
  setTimeout(function(){ URL.revokeObjectURL(a.href); },4000);
  api.decir('CSV bajado: '+a.download);
}
function accion(n){
  var r=rango(), cambios=[], f, c;
  switch(n){
    case 't-copiar': copiarConBoton(); break;
    case 't-pegar': pegarConBoton(); break;
    case 't-csv': bajarCsv(); break;
    case 't-borrar': borrar(); break;
    case 't-negrita':
      if(!editable)break;
      var todas=true;
      for(f=r.f;f<=r.f2&&todas;f++)for(c=r.c;c<=r.c2;c++){ var e=estilos[f*1024+c]; if(!(e&&e.n)){todas=false;break;} }
      for(f=r.f;f<=Math.min(r.f2,r.f+5000);f++)for(c=r.c;c<=r.c2;c++){
        var k=f*1024+c, a=estilos[k], nuevo={}; for(var q in a||{})nuevo[q]=a[q];
        if(todas)delete nuevo.n; else nuevo.n=true;
        cambios.push([k,Object.keys(nuevo).length?nuevo:null]);
      }
      escribir([],cambios); break;
    case 't-suma':
      if(!editable)break;
      var fs=act.f-1; while(fs>=0&&calc.valor(fs,act.c).t==='n')fs--;
      editar(fs<act.f-1?'=SUMA('+Calculo.nombre(fs+1,act.c)+':'+Calculo.nombre(act.f-1,act.c)+')':'=SUMA()'); break;
  }
}

// ---- Guardar ----
function json(){
  var celdas={}, es={}, an={}, k;
  calc.cada(function(f,c,v){ celdas[Calculo.nombre(f,c)]=v; });
  for(k in estilos)es[Calculo.nombre(Math.floor(k/1024),k%1024)]=estilos[k];
  for(k in anchos)an[Calculo.letras(+k)]=anchos[k];
  var o={}; if(nombreTabla)o.nombre=nombreTabla; o.celdas=celdas; if(protegida)o.protegida=true;
  if(Object.keys(an).length)o.anchos=an;
  if(Object.keys(es).length)o.estilos=es;
  return JSON.stringify(o).replace(/</g,'\\u003c');
}
/** La tabla ya calculada para el archivo guardado: el mismo marco que se ve. */
function estatica(){
  var m=limites(), f, c, f1=Math.min(m.f1,m.f0+1999), c1=Math.min(m.c1,m.c0+199);
  var s='<'+'table class="'+'calc"><thead><tr><th class="esq"></th>';
  for(c=m.c0;c<=c1;c++)s+='<th style="width:'+(anchos[c]||ANCHO)+'px">'+Calculo.letras(c)+'</th>';
  s+='</tr></thead><tbody>';
  for(f=m.f0;f<=f1;f++){
    s+='<tr><th>'+(f+1)+'</th>';
    for(c=m.c0;c<=c1;c++){
      var raw=calc.crudo(f,c), v=raw?calc.valor(f,c):null, es=estilos[f*1024+c];
      var al=es&&es.a?es.a:(v?(v.t==='n'?'d':(v.t==='b'||v.t==='e')?'c':'i'):'i'), cl=[];
      if(al==='d')cl.push('d'); else if(al==='c')cl.push('c');
      if(es&&es.n)cl.push('n'); if(v&&v.t==='e')cl.push('err'); if(Calculo.esFormula(raw))cl.push('f'); if(protegida&&es&&es.e)cl.push('ed');
      s+='<td'+(cl.length?' class="'+cl.join(' ')+'"':'')+(es&&es.f&&/^#[0-9a-fA-F]{6}$/.test(es.f)?' style="background:'+es.f+'"':'')+'>'+String(raw?calc.texto(f,c):'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')+'</td>';
    }
    s+='</tr>';
  }
  return s+'</tbody></'+'table>';
}
construir();
aplicarVista();
seleccionar(0,0,false,true);
return {
  tipo:'tabla',
  herramientas:['mano','lapiz','marcador','goma'].filter(function(h){
    return (document.body.dataset.herramientas||'mano lapiz marcador goma').split(' ').indexOf(h)>=0; }),
  activar:function(){
    activo=true; caja.focus({preventScroll:true}); medirTinta();
    // La primera vez que se mira, centrada: antes la hoja estaba oculta y no tenía medidas.
    if(!centrada){ centrada=true; encajar(); }
    mostrarBarra(); estadoRango();
  },
  desactivar:function(){ if(editando)confirmar(); activo=false; },
  medir:function(){ medirTinta(); pintarMarcas(); },
  modo:function(m){ modo=m; if(m!=='mano'&&editando)confirmar(); },
  pintando:function(){ return modo!=='mano'; },
  encajar:encajar,
  zoom:function(f){ var r=caja.getBoundingClientRect(); zoomEn(r.left+caja.clientWidth/2,r.top+caja.clientHeight/2,1/f); },
  deshacer:deshacer, rehacer:rehacer,
  puedeDeshacer:function(){ return hecho.length>0; },
  puedeRehacer:function(){ return rehecho.length>0; },
  tecla:tecla, accion:accion, json:json, estatica:estatica, rayas:rayas
};
}
