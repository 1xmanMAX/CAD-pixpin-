
function crearDibujo(caja, api){
"use strict";
var NS='http://www.w3.org/2000/svg';
var svg=caja.querySelector('svg');
if(!svg) return null;
svg.removeAttribute('width'); svg.removeAttribute('height');
svg.setAttribute('preserveAspectRatio','xMidYMid meet');
var b=svg.viewBox.baseVal;
var casa={x:b.x,y:b.y,w:b.width,h:b.height};
var v={x:casa.x,y:casa.y,w:casa.w,h:casa.h};
var MIN=casa.w/400, MAX=casa.w*20;
var croquis=svg.querySelector('#croquis');
if(!croquis){croquis=document.createElementNS(NS,'g');croquis.id='croquis';svg.appendChild(croquis);}
var hecho=[], rehecho=[], modo='mano', trazo=null, hayLapiz=false;
// Medir: cotas de plano —dos puntas, flechas y la cifra encima—, y se quedan puestas.
// Ver la sección «Medir» más abajo.
var cotas=[], medida=null, grupoMedida=null, agarreCota=null;
var escala=parseFloat(caja.dataset.escala)||0, unidad=caja.dataset.unidad||'', decimales=parseInt(caja.dataset.decimales)||2;
// El papel, si vino como geometría en vez de como foto. Ver [VisorPlano].
var plano=(typeof crearPlano==='function')?crearPlano(caja,{encuadrar:encuadrarEn}):null;

// **Un encuadre por fotograma.** Dos dedos mandan dos `pointermove` por fotograma y la rueda
// del ratón o la tableta, más; cambiar el `viewBox` repinta la hoja entera, así que se apunta
// que hace falta y se hace una vez, justo antes de pintar. [v] cambia en el acto: las cuentas
// de dónde cae el dedo siguen siendo exactas.
var fotograma=0;
function aplicar(){
  if(fotograma) return;
  fotograma=requestAnimationFrame(aplicarYa);
}
function aplicarYa(){
  if(fotograma){ cancelAnimationFrame(fotograma); fotograma=0; }
  svg.setAttribute('viewBox',v.x+' '+v.y+' '+v.w+' '+v.h);
  if(plano) plano.ver(v);
  // Las cotas se miden en pantalla —la flecha mide lo mismo de cerca que de lejos—, así que
  // cambiar el encuadre obliga a rehacerlas. Ver [pintarMedida].
  if(grupoMedida&&(cotas.length||medida)) pintarMedida();
  // Un trazo a medias se había dibujado con el encuadre de antes: se vuelve a dibujar.
  if(trazo) repintarTintaViva();
}
// Encuadra una caja del dibujo, con un dedo de margen: es lo que usa el cajón de capas para
// llevarte a donde está lo que acabas de encender.
function encuadrarEn(c){
  var m=Math.max(c.w,c.h)*0.06;
  v={x:c.x-m, y:c.y-m, w:c.w+m*2, h:c.h+m*2};
  aplicar();
}
function marco(){
  var r=svg.getBoundingClientRect();
  var k=Math.max(v.w/r.width,v.h/r.height);
  return {k:k, ox:v.x-(r.left+(r.width-v.w/k)/2)*k, oy:v.y-(r.top+(r.height-v.h/k)/2)*k};
}
function aEscena(px,py,m){ m=m||marco(); return {x:m.ox+px*m.k, y:m.oy+py*m.k, k:m.k}; }
function zoom(factor,px,py){
  var an=Math.min(Math.max(v.w*factor,MIN),MAX);
  var f=an/v.w; if(f===1)return;
  var p=aEscena(px,py);
  v.x=p.x-(p.x-v.x)*f; v.y=p.y-(p.y-v.y)*f; v.w*=f; v.h*=f;
  aplicar();
}
// Las rayas son **la línea tal cual la trazó el lápiz**: `M` y una `L` con todas las muestras.
// Las de antes del 14-sep-2026 eran curvas por los puntos medios —el punto de partida, el
// control de cada Q y la L final son las muestras— y se leen igual, como las polilíneas.
function leerPuntos(r){
  var lista=[], i;
  if(r.localName==='path'){
    var d=r.getAttribute('d')||'';
    var re=/([MLQ])([^MLQ]*)/g, m, ultimaL=null, curvas=d.indexOf('Q')>=0;
    while((m=re.exec(d))){
      var n=m[2].trim().split(/[\s,]+/).map(Number);
      if(m[1]==='M'&&n.length>=2)lista.push({x:n[0],y:n[1]});
      else if(m[1]==='Q'&&n.length>=4)lista.push({x:n[0],y:n[1]});
      else if(m[1]==='L'&&curvas&&n.length>=2)ultimaL={x:n[0],y:n[1]};
      else if(m[1]==='L')for(var j=0;j+1<n.length;j+=2)lista.push({x:n[j],y:n[j+1]});
    }
    if(ultimaL)lista.push(ultimaL);
  } else {
    var pts=(r.getAttribute('points')||'').trim().split(/\s+/);
    for(i=0;i<pts.length;i++){ var xy=pts[i].split(','); if(xy.length===2)lista.push({x:+xy[0],y:+xy[1]}); }
  }
  return lista.filter(function(q){return isFinite(q.x)&&isFinite(q.y);});
}
Array.prototype.forEach.call(croquis.children,function(r){r.puntos=leerPuntos(r);});

function deshacer(){
  var u=hecho.pop(); if(!u)return;
  if(u.que==='pinta'){ if(u.raya.parentNode)croquis.removeChild(u.raya); }
  else { croquis.insertBefore(u.raya,u.antes&&u.antes.parentNode===croquis?u.antes:null); }
  rehecho.push(u); api.refrescar();
}
function rehacer(){
  var u=rehecho.pop(); if(!u)return;
  if(u.que==='pinta'){ croquis.appendChild(u.raya); }
  else { u.antes=u.raya.nextSibling; if(u.raya.parentNode)croquis.removeChild(u.raya); }
  hecho.push(u); api.refrescar();
}
function apuntar(u){hecho.push(u);rehecho.length=0;api.refrescar();}
function distanciaAlTramo(p,a,c){
  var dx=c.x-a.x, dy=c.y-a.y, l2=dx*dx+dy*dy;
  var t=l2>0?Math.max(0,Math.min(1,((p.x-a.x)*dx+(p.y-a.y)*dy)/l2)):0;
  return Math.hypot(a.x+t*dx-p.x,a.y+t*dy-p.y);
}
// Se borra la raya cuyo trazado —no solo sus puntos— pasa bajo el dedo: una raya rápida
// tiene los puntos lejos unos de otros.
function borrarEn(px,py,m){
  olvidarLosImanes();
  var p=aEscena(px,py,m);
  var rayas=Array.prototype.slice.call(croquis.children);
  for(var i=0;i<rayas.length;i++){
    var r=rayas[i], pts=r.puntos||[];
    var radio=12*p.k+(+r.getAttribute('stroke-width')||0)/2;
    for(var j=0;j<pts.length;j++){
      if(distanciaAlTramo(p,pts[j],pts[j+1]||pts[j])<=radio){
        apuntar({que:'borra',raya:r,antes:r.nextSibling});
        croquis.removeChild(r);break;
      }
    }
  }
}
function r3(x){return Math.round(x*1000)/1000;}
// **La raya es lo que puso el lápiz, sin arreglos** (lo pidió el usuario el 14-sep-2026: con
// curvas por los puntos medios y la cola prevista, lo escrito «se movía» y parecía corregido).
// Cada muestra es un vértice, y entre dos, una recta.
//
// Y **ligera**: cada número lleva los decimales que hacen falta para medio píxel de pantalla
// en el aumento con el que se trazó, ni uno más, y la `L` se escribe una sola vez.
function redondeo(k){ return Math.pow(10,Math.max(0,Math.min(3,Math.ceil(-Math.log10(k*0.5))))); }
function anadirPunto(t,p){
  var pts=t.puntos, n=pts.length, f=t.f||(t.f=redondeo(t.m?t.m.k:1));
  pts.push(p);
  var x=Math.round(p.x*f)/f, y=Math.round(p.y*f)/f;
  if(n===0){t.abierto='M'+x+' '+y;return;}
  t.abierto+=(n===1?'L':' ')+x+' '+y;
}
// ---- La tinta viva ----
//
// **Mientras se escribe, la raya no está en el SVG.** Cambiar el `d` de un camino obliga al
// navegador a repintar la hoja entera —con un plano o una foto grande debajo, eso es cada
// muestra del lápiz— y el trazo se quedaba atrás de la punta. Así que la raya viva se dibuja
// en un lienzo transparente encima, **solo el tramo nuevo** cada vez, y al levantar el lápiz
// pasa al SVG de una vez. Con `desynchronized` el navegador lo enseña sin esperar al resto de
// la página, que es lo que usan las pizarras para ir pegadas a la punta. Se dibuja **lo mismo**
// que irá al SVG —rectas entre las muestras—, así que al soltar no cambia nada. Pedido el 13-sep-2026, probando con tableta gráfica. **Sin predecir** hacia dónde va la punta:
// esa cola se corregía en cada muestra y lo escrito parecía moverse (14-sep-2026).
var viva=null, vctx=null;
function prepararTintaViva(marca){
  if(!viva){
    viva=document.createElement('canvas');
    viva.className='tinta-viva';
    caja.appendChild(viva);
    try{ vctx=viva.getContext('2d',{desynchronized:true}); }catch(err){}
    vctx=vctx||viva.getContext('2d');
  }
  var r=caja.getBoundingClientRect(), dpr=window.devicePixelRatio||1;
  var w=Math.max(1,Math.round(r.width*dpr)), h=Math.max(1,Math.round(r.height*dpr));
  if(viva.width!==w||viva.height!==h){ viva.width=w; viva.height=h; }
  vctx.setTransform(dpr,0,0,dpr,-r.left*dpr,-r.top*dpr);
  vctx.clearRect(r.left,r.top,r.width,r.height);
  vctx.lineCap='round'; vctx.lineJoin='round';
  // El resaltador es translúcido y se funde con el papel: el lienzo entero, no cada tramo,
  // que si no los tramos se pisan y salen cuentas más oscuras.
  viva.style.opacity=marca?'0.55':'1';
  viva.style.mixBlendMode=marca?'var(--fusion)':'normal';
  return r;
}
function limpiarTintaViva(){
  if(!viva) return;
  vctx.setTransform(1,0,0,1,0,0); vctx.clearRect(0,0,viva.width,viva.height);
}
function estiloVivo(g,t){ g.strokeStyle=t.color; g.lineWidth=t.ancho; }
// El tramo que añade la muestra [n] de la raya: la misma recta que irá al SVG.
function tramoVivo(t,n){
  var q=t.pan, g=vctx;
  estiloVivo(g,t);
  g.beginPath();
  g.moveTo(q[n-1].x,q[n-1].y);
  g.lineTo(q[n].x,q[n].y);
  g.stroke();
}
// El punto del principio, para que un toque se vea mientras el lápiz sigue apoyado.
function colaViva(t){
  if(t.pan.length!==1) return;
  var g=vctx, q=t.pan[0];
  estiloVivo(g,t);
  g.beginPath(); g.moveTo(q.x,q.y); g.lineTo(q.x+0.01,q.y); g.stroke();
}
// El encuadre cambió con la raya a medias (un dedo pellizcando mientras escribe el lápiz): la
// raya se vuelve a poner en pantalla desde sus puntos del dibujo, que son los que valen.
function repintarTintaViva(){
  if(!trazo||!viva) return;
  var m=marco();
  trazo.r=prepararTintaViva(trazo.marca);
  trazo.m=m;
  trazo.ancho=trazo.anchoDibujo/m.k;
  trazo.pan=trazo.puntos.map(function(p){ return {x:(p.x-m.ox)/m.k, y:(p.y-m.oy)/m.k}; });
  colaViva(trazo);
  for(var i=1;i<trazo.pan.length;i++) tramoVivo(trazo,i);
}
function pintarTrazo(t){ t.setAttribute('d',t.abierto); }
// El navegador junta en un solo pointermove todas las muestras llegadas desde el fotograma
// anterior; un ratón da cientos por segundo. Sin pedirlas, una raya rápida salía a tramos.
function muestras(e){
  var lote=(e.getCoalescedEvents&&e.getCoalescedEvents())||[];
  return lote.length?lote:[e];
}
function empezarTrazo(px,py,marca,puntero){
  // **El marco se mide una vez por raya**: medirlo en cada muestra era preguntarle al
  // navegador dónde está el SVG cientos de veces por segundo. Si el encuadre cambia a mitad,
  // [aplicarYa] lo vuelve a medir.
  var m=marco(), p=aEscena(px,py,m);
  var ancho=marca?api.grosor()*3:api.grosor();
  trazo=document.createElementNS(NS,'path');
  trazo.puntero=puntero;
  trazo.setAttribute('fill','none');
  trazo.setAttribute('stroke',api.color());
  trazo.setAttribute('stroke-width',r3(ancho*p.k));
  trazo.setAttribute('stroke-linecap','round');
  trazo.setAttribute('stroke-linejoin','round');
  if(marca){trazo.setAttribute('stroke-opacity','0.55');trazo.setAttribute('class','marca');}
  trazo.puntos=[];trazo.ultimo={x:px,y:py};
  trazo.m=m; trazo.marca=marca; trazo.color=api.color(); trazo.ancho=ancho; trazo.anchoDibujo=ancho*p.k;
  trazo.pan=[{x:px,y:py}];
  anadirPunto(trazo,{x:p.x,y:p.y});
  trazo.r=prepararTintaViva(marca);
  colaViva(trazo);
}
/** Una muestra más de la raya viva, en píxeles de pantalla. */
function seguirTrazo(cx,cy){
  if(Math.hypot(cx-trazo.ultimo.x,cy-trazo.ultimo.y)<0.5) return false; // menos de medio píxel no se ve
  trazo.ultimo={x:cx,y:cy};
  anadirPunto(trazo,aEscena(cx,cy,trazo.m));
  trazo.pan.push({x:cx,y:cy});
  tramoVivo(trazo,trazo.pan.length-1);
  return true;
}
function soltarTrazo(){
  if(!trazo)return;
  // El dibujo ha cambiado: el imán tiene que volver a mirarlo. Ver [imanesDeLaHoja].
  olvidarLosImanes();
  var t=trazo;
  trazo=null;
  if(t.puntos.length>=2){        // un punto solo no se ve
    pintarTrazo(t);
    croquis.appendChild(t);
    apuntar({que:'pinta',raya:t});
  }
  // La tinta viva se borra **cuando el SVG ya enseña la raya**, dos fotogramas después; si se
  // borrase a la vez, se vería un parpadeo. Si mientras tanto empezó otra raya, se redibuja.
  requestAnimationFrame(function(){ requestAnimationFrame(function(){
    limpiarTintaViva();
    if(trazo) repintarTintaViva();
  }); });
}
// ---- Medir: cotas de plano ----
//
// **Una cota, no una cifra suelta.** Antes medir eran dos puntos gordos, una raya y un número
// en la barra de estado, y la medida siguiente borraba la anterior. En un plano eso no sirve:
// se miden varias cosas y se comparan, y la cifra tiene que estar **donde está lo medido**.
// Ahora cada medida es una cota como la de un plano —línea, flecha en cada punta y la cifra
// encima, en su sitio— que **se queda puesta**, se puede coger y mover, y se pega a las líneas
// del dibujo. Lo pidió el usuario el 8-sep-2026.
//
// Todo se guarda en unidades del dibujo y se pinta contando el aumento, para que la flecha y
// la letra midan lo mismo de cerca que de lejos. Por eso [aplicar] repinta al encuadrar.

/** Lo que mide una cota, ya escrito: en la unidad del dibujo si se calibró, en píxeles si no. */
function textoDeCota(c){
  var d=Math.hypot(c.b.x-c.a.x, c.b.y-c.a.y);
  return escala>0 ? (d*escala).toFixed(decimales)+' '+unidad : d.toFixed(1)+' px';
}
/** Una flecha en la punta (x,y), apuntando desde (dx,dy), del tamaño [t] en unidades. */
function flechaDe(x,y,dx,dy,t){
  var l=Math.hypot(dx,dy)||1, ux=dx/l, uy=dy/l, px=-uy, py=ux;
  var f=document.createElementNS(NS,'path');
  f.setAttribute('d','M'+x+' '+y+'L'+(x+ux*t+px*t*0.36)+' '+(y+uy*t+py*t*0.36)+
                     'L'+(x+ux*t-px*t*0.36)+' '+(y+uy*t-py*t*0.36)+'Z');
  return f;
}
function pintarMedida(){
  if(!grupoMedida){ grupoMedida=document.createElementNS(NS,'g'); grupoMedida.id='medida'; svg.appendChild(grupoMedida); }
  while(grupoMedida.firstChild) grupoMedida.removeChild(grupoMedida.firstChild);
  var k=aEscena(0,0).k;                 // unidades del dibujo por píxel de pantalla
  var todas=cotas.concat(medida&&medida.b?[medida]:[]);
  for(var i=0;i<todas.length;i++) pintarUnaCota(todas[i], k, i===agarreCota&&agarreCota!==null);
  // El primer punto de una cota a medias: un aspa, que no es una cota todavía.
  if(medida&&!medida.b){
    var t=7*k, a=medida.a;
    var x=document.createElementNS(NS,'path');
    x.setAttribute('d','M'+(a.x-t)+' '+a.y+'h'+(2*t)+'M'+a.x+' '+(a.y-t)+'v'+(2*t));
    x.setAttribute('class','pendiente');
    grupoMedida.appendChild(x);
  }
}
function pintarUnaCota(c, k, viva){
  var g=document.createElementNS(NS,'g');
  g.setAttribute('class','cota'+(viva?' viva':''));
  var dx=c.b.x-c.a.x, dy=c.b.y-c.a.y, l=Math.hypot(dx,dy);
  var linea=document.createElementNS(NS,'line');
  linea.setAttribute('x1',c.a.x); linea.setAttribute('y1',c.a.y);
  linea.setAttribute('x2',c.b.x); linea.setAttribute('y2',c.b.y);
  g.appendChild(linea);
  // Las flechas solo si la cota da para ellas: en una cota más corta que su propia punta,
  // dos flechas encontradas son una mancha y no se entiende nada.
  var t=9*k;
  if(l>t*2.4){
    g.appendChild(flechaDe(c.a.x,c.a.y, dx, dy, t));
    g.appendChild(flechaDe(c.b.x,c.b.y,-dx,-dy, t));
  }
  // La cifra, **encima de la línea y girada con ella**, como en un plano. Y siempre legible:
  // pasado el vertical se lee del revés, así que se le da la vuelta.
  var gr=Math.atan2(dy,dx)*180/Math.PI;
  if(gr>90||gr<-90) gr+=180;
  var mx=(c.a.x+c.b.x)/2, my=(c.a.y+c.b.y)/2;
  var texto=document.createElementNS(NS,'text');
  texto.setAttribute('x',mx); texto.setAttribute('y',my-5*k);
  texto.setAttribute('transform','rotate('+gr+' '+mx+' '+my+')');
  texto.setAttribute('font-size',(13*k));
  texto.textContent=textoDeCota(c);
  g.appendChild(texto);
  grupoMedida.appendChild(g);
}

/**
 * **Medir es libre**: el punto va exactamente donde se toca. Hubo un imán que pegaba el punto a
 * la geometría cercana, y en un plano pegaba a puntos que no se ven —muestras de una diagonal,
 * esquinas que no eran la buscada—; lo quitó el usuario el 14-sep-2026.
 */
function imantar(p){ return {x:p.x,y:p.y}; }
function olvidarLosImanes(){}

/** Qué cota y qué parte de ella cae bajo el dedo: una punta, o su mitad para moverla entera. */
function cotaBajoElDedo(p,k){
  var r=16*k, rr=r*r;
  for(var i=cotas.length-1;i>=0;i--){    // la de encima primero, que es la última puesta
    var c=cotas[i];
    if((c.a.x-p.x)*(c.a.x-p.x)+(c.a.y-p.y)*(c.a.y-p.y)<rr) return {i:i,parte:'a'};
    if((c.b.x-p.x)*(c.b.x-p.x)+(c.b.y-p.y)*(c.b.y-p.y)<rr) return {i:i,parte:'b'};
    // El cuerpo: distancia al segmento, para poder arrastrarla entera.
    var dx=c.b.x-c.a.x, dy=c.b.y-c.a.y, ll=dx*dx+dy*dy;
    if(!ll) continue;
    var t=((p.x-c.a.x)*dx+(p.y-c.a.y)*dy)/ll;
    if(t<0.18||t>0.82) continue;         // cerca de las puntas manda la punta
    var qx=c.a.x+dx*t, qy=c.a.y+dy*t;
    if((qx-p.x)*(qx-p.x)+(qy-p.y)*(qy-p.y)<rr) return {i:i,parte:'todo',t:t};
  }
  return null;
}

/**
 * Coger una cota que ya está puesta. Devuelve `true` si la ha cogido.
 *
 * Se guarda **dónde estaba** para poder devolverla si el gesto resulta no ser un arrastre
 * —por ejemplo si aparece un segundo dedo y lo que se quería era hacer zoom—. Ver
 * [cancelarArrastreDeCota].
 */
function cogerCotaEn(px,py){
  var k=aEscena(0,0).k, p=aEscena(px,py);
  var cogida=cotaBajoElDedo(p,k);
  if(!cogida) return false;
  var c=cotas[cogida.i];
  agarreCota=cogida.i;
  arrastreDeCota={i:cogida.i,parte:cogida.parte,x:p.x,y:p.y,
                  antes:{a:{x:c.a.x,y:c.a.y},b:{x:c.b.x,y:c.b.y}}};
  return true;
}
/** Devolver la cota a donde estaba y soltarla, sin dar la medida por buena. */
function cancelarArrastreDeCota(){
  if(!arrastreDeCota) return;
  var c=cotas[arrastreDeCota.i];
  if(c&&arrastreDeCota.antes){ c.a=arrastreDeCota.antes.a; c.b=arrastreDeCota.antes.b; }
  arrastreDeCota=null; agarreCota=null; pintarMedida();
}
/** Poner un punto de medida donde se ha tocado. */
function ponerPuntoDeMedida(px,py){
  var k=aEscena(0,0).k, p=aEscena(px,py);
  if(!medida){                       // primer punto
    medida={a:imantar(p,k)};
    api.decir('Primer punto puesto: toca el segundo');
  } else {                           // segundo: la cota se queda
    medida.b=imantar(p,k);
    if(Math.hypot(medida.b.x-medida.a.x,medida.b.y-medida.a.y)>0.0001){
      cotas.push(medida);
      api.decir(textoDeCota(medida)+'  ·  toca una cota para moverla');
    }
    medida=null;
  }
  pintarMedida();
}
var arrastreDeCota=null;
function arrastrarCota(px,py){
  if(!arrastreDeCota) return;
  var k=aEscena(0,0).k, p=aEscena(px,py), c=cotas[arrastreDeCota.i];
  if(!c) return;
  if(arrastreDeCota.parte==='todo'){
    var dx=p.x-arrastreDeCota.x, dy=p.y-arrastreDeCota.y;
    c.a={x:c.a.x+dx,y:c.a.y+dy}; c.b={x:c.b.x+dx,y:c.b.y+dy};
    arrastreDeCota.x=p.x; arrastreDeCota.y=p.y;
  } else {
    // La punta que se mueve sí se imanta; la cota entera no, que se movería a saltos.
    c[arrastreDeCota.parte]=imantar(p,k);
  }
  pintarMedida();
}
function soltarCota(){
  if(!arrastreDeCota) return;
  var c=cotas[arrastreDeCota.i];
  // Una cota arrastrada hasta quedar en un punto se ha querido borrar.
  if(c&&Math.hypot(c.b.x-c.a.x,c.b.y-c.a.y)<aEscena(0,0).k*6){
    cotas.splice(arrastreDeCota.i,1); api.decir('Cota quitada');
  } else if(c) api.decir(textoDeCota(c));
  arrastreDeCota=null; agarreCota=null; pintarMedida();
}
/** Quitarlas todas. Es lo que hace salir de medir, y la tecla de escape. */
function quitarMedida(){ cotas=[]; medida=null; arrastreDeCota=null; agarreCota=null; if(grupoMedida) pintarMedida(); }

// **Con lápiz a la vista, el dedo mueve el papel.** En cuanto se posa un lápiz la página se
// entera y no vuelve atrás: desde ese momento el lápiz traza y el dedo pasea, sin cambiar de
// modo, igual que en la aplicación. Y el otro extremo del lápiz —el que se declara como
// goma— borra mientras se use.
function loQueHace(e){
  if(modo==='medir') return 'medir';
  if(e.pointerType==='pen'){
    // La goma del lápiz solo borra si el borrador viene en el documento.
    if((e.buttons&32) && permitida('goma')) return 'goma';
    if(modo==='goma'||modo==='marcador'||modo==='lapiz') return modo;
    // Con la mano puesta, el lápiz traza con lo que haya: lápiz, si no resaltador, y si el
    // documento no trae ninguno de los dos, pasea como el dedo. Antes trazaba siempre, aunque
    // el lápiz se hubiera dejado fuera al exportar.
    if(permitida('lapiz')) return 'lapiz';
    if(permitida('marcador')) return 'marcador';
    return 'mano';
  }
  if(hayLapiz) return 'mano';
  return modo;
}
var dedos=new Map(), arrastre=null, pellizco=null;
// **Cada trazo es de un solo puntero**, y el pellizco es cosa de dos dedos: el lápiz nunca
// entra en él. Antes cualquier puntero que se moviera añadía puntos al trazo abierto, y con
// un dedo apoyado mientras se escribía con el lápiz salían rayas entre el dedo y la punta.
function losDedos(){ return Array.from(dedos.values()).filter(function(d){return d.tipo!=='pen';}); }
function arrastrando(e){ return arrastre&&arrastre.id===e.pointerId; }
caja.addEventListener('pointerdown',function(e){
  if(e.pointerType==='mouse'&&e.button!==0)return;
  if(e.pointerType==='pen'&&!hayLapiz){hayLapiz=true;api.decir('Lápiz a la vista: el dedo mueve el papel');}
  caja.setPointerCapture(e.pointerId);
  dedos.set(e.pointerId,{x:e.clientX,y:e.clientY,tipo:e.pointerType});
  var dd=losDedos();
  if(e.pointerType==='pen'){
    // El lápiz hace lo suyo aunque haya un dedo apoyado: es lo normal al escribir.
    if(trazo) return;
    var q=loQueHace(e);
    if(q==='lapiz'||q==='marcador') empezarTrazo(e.clientX,e.clientY,q==='marcador',e.pointerId);
    else if(q==='goma') borrarEn(e.clientX,e.clientY);
    // **El lápiz sí mide al posarse**: es una punta, no pellizca, y esperar al levantarlo
    // solo lo haría torpe. Lo que espera es el dedo. Ver [tocaMedir].
    else if(q==='medir'){ if(cogerCotaEn(e.clientX,e.clientY)) cotaConElDedo=e.pointerId; else ponerPuntoDeMedida(e.clientX,e.clientY); }
    else if(!arrastre){ arrastre={id:e.pointerId,x:e.clientX,y:e.clientY}; api.agarrado(true); }
    return;
  }
  if(dd.length===1){
    var q1=loQueHace(e);
    if(trazo) return;   // el lápiz está escribiendo: el dedo no le quita el trazo
    if(q1==='lapiz'||q1==='marcador') empezarTrazo(e.clientX,e.clientY,q1==='marcador',e.pointerId);
    else if(q1==='goma') borrarEn(e.clientX,e.clientY);
    // **Midiendo, el dedo no hace nada todavía.**
    //
    // El primer dedo de un pellizco es idéntico a un toque: no hay forma de distinguirlos
    // hasta que pasa algo más —llega el segundo dedo, se mueve, o se levanta—. Poniendo el
    // punto al posarse, cada zoom de dos dedos dejaba una medida a medias sin querer, que es
    // lo que reportó el usuario el 9-sep-2026. Así que se apunta lo que ha pasado y **se
    // decide al levantar**: si no llegó un segundo dedo y no se movió, es un toque y pone
    // punto. Ver [soltar].
    else if(q1==='medir'){
      tocaMedir={id:e.pointerId, x:e.clientX, y:e.clientY, movido:false, cogida:false};
      if(cogerCotaEn(e.clientX,e.clientY)){ tocaMedir.cogida=true; cotaConElDedo=e.pointerId; }
    }
    else { arrastre={id:e.pointerId,x:e.clientX,y:e.clientY}; api.agarrado(true); }
  } else if(dd.length===2){
    // El segundo dedo encuadra: un trazo del dedo a medias se queda como iba; el del lápiz
    // sigue, que es de otra mano.
    arrastre=null;
    if(trazo&&trazo.puntero!==undefined&&dedos.get(trazo.puntero)&&dedos.get(trazo.puntero).tipo!=='pen') soltarTrazo();
    // **Esto era un pellizco desde el principio.** Se deshace lo que el primer dedo hubiera
    // empezado a medir: la cota que hubiera cogido vuelve a donde estaba y no se pone punto.
    if(tocaMedir){
      if(tocaMedir.cogida) cancelarArrastreDeCota();
      cotaConElDedo=null; tocaMedir=null;
    }
    pellizco={d:Math.hypot(dd[0].x-dd[1].x,dd[0].y-dd[1].y)};
  }
});
caja.addEventListener('pointermove',function(e){
  if(!dedos.has(e.pointerId))return;
  var d0=dedos.get(e.pointerId); d0.x=e.clientX; d0.y=e.clientY;
  var dd=losDedos();
  if(pellizco&&dd.length===2&&e.pointerType!=='pen'){
    var dist=Math.hypot(dd[0].x-dd[1].x,dd[0].y-dd[1].y);
    // **La pinza no puede dar saltos** (16-sep-2026). Con la mano apoyada —presentando con la
    // mano o con el borrador, que es cuando el dedo no dibuja— el navegador manda dos puntos
    // que aparecen, desaparecen y saltan de un lado a otro de la pantalla: la razón entre dos
    // medidas seguidas se iba a cien, el encuadre salía disparado a kilómetros del dibujo y la
    // pantalla se quedaba vacía. Un pellizco de verdad, entre dos avisos, no dobla ni parte por
    // la mitad la distancia; lo que pase de ahí es la palma y se ignora.
    if(dist>0&&pellizco.d>0){
      var f=pellizco.d/dist;
      if(f>=0.5&&f<=2) zoom(f,(dd[0].x+dd[1].x)/2,(dd[0].y+dd[1].y)/2);
    }
    pellizco.d=dist;
  } else if(trazo&&trazo.puntero===e.pointerId){
    var lote=muestras(e);
    for(var i=0;i<lote.length;i++) seguirTrazo(lote[i].clientX,lote[i].clientY);
  } else if(!trazo&&!pellizco&&loQueHace(e)==='goma'){
    var mg=marco(), lg=muestras(e);
    for(var g=0;g<lg.length;g++)borrarEn(lg[g].clientX,lg[g].clientY,mg);
  } else if(tocaMedir&&tocaMedir.id===e.pointerId&&!pellizco){
    // Hasta que no se mueva de verdad no es un arrastre: un dedo tiembla, y un temblor no
    // puede convertir un toque en otra cosa. El umbral se mide en pantalla.
    if(!tocaMedir.movido&&Math.hypot(e.clientX-tocaMedir.x,e.clientY-tocaMedir.y)>8) tocaMedir.movido=true;
    if(tocaMedir.movido){
      if(tocaMedir.cogida) arrastrarCota(e.clientX,e.clientY);
      else {
        // **Y midiendo, un dedo que arrastra pasea el papel**, como en cualquier otro modo.
        // Antes no hacía nada: para moverse por el plano había que salir de medir.
        var km=aEscena(0,0).k;
        v.x-=(e.clientX-tocaMedir.x)*km; v.y-=(e.clientY-tocaMedir.y)*km;
        tocaMedir.x=e.clientX; tocaMedir.y=e.clientY;
        aplicar();
      }
    }
  } else if(cotaConElDedo===e.pointerId){
    arrastrarCota(e.clientX,e.clientY);
  } else if(arrastrando(e)){
    var k=aEscena(0,0).k;
    v.x-=(e.clientX-arrastre.x)*k; v.y-=(e.clientY-arrastre.y)*k;
    arrastre.x=e.clientX; arrastre.y=e.clientY;
    aplicar();
  }
});
// Qué dedo trae una cota cogida: mientras la trae, ese dedo no pasea el papel.
var cotaConElDedo=null;
// El dedo que está midiendo, mientras no se sabe todavía qué quiere. Ver el `pointerdown`.
var tocaMedir=null;
function soltar(e){
  dedos.delete(e.pointerId);
  if(tocaMedir&&tocaMedir.id===e.pointerId){
    // Ni segundo dedo ni movimiento: era un toque, y ahora sí se pone el punto.
    if(!tocaMedir.movido&&!tocaMedir.cogida) ponerPuntoDeMedida(e.clientX,e.clientY);
    else if(tocaMedir.cogida&&!tocaMedir.movido) cancelarArrastreDeCota();  // se rozó y ya
    tocaMedir=null;
  }
  if(cotaConElDedo===e.pointerId){ cotaConElDedo=null; soltarCota(); }
  if(losDedos().length<2)pellizco=null;
  if(trazo&&trazo.puntero===e.pointerId) soltarTrazo();
  if(arrastrando(e)){ arrastre=null; }
  if(dedos.size===0){ soltarTrazo(); arrastre=null; api.agarrado(false); }
}
caja.addEventListener('pointerup',soltar);
caja.addEventListener('pointercancel',soltar);
caja.addEventListener('wheel',function(e){
  e.preventDefault();
  zoom(e.deltaY>0?1.12:1/1.12,e.clientX,e.clientY);
},{passive:false});
// El doble toque encaja solo con la mano: con el lápiz, dos toques seguidos son dos puntos.
caja.addEventListener('dblclick',function(e){e.preventDefault();if(modo==='mano')encajar();});
function encajar(){v={x:casa.x,y:casa.y,w:casa.w,h:casa.h};aplicar();}
// Lo que deja coger el documento: lo que no tiene botón tampoco entra por el teclado.
var permitidas=(document.body.dataset.herramientas||'mano lapiz marcador goma medir').split(' ');
function permitida(h){ return permitidas.indexOf(h)>=0; }
function permitidasDe(lista){ return lista.filter(permitida); }

function rayasComoTexto(){
  var s='';
  Array.prototype.forEach.call(croquis.children,function(r){
    s+='<'+r.localName;
    for(var i=0;i<r.attributes.length;i++){
      var a=r.attributes[i];
      s+=' '+a.name+'="'+String(a.value).replace(/&/g,'&amp;').replace(/"/g,'&quot;')+'"';
    }
    s+='/>\n';
  });
  return s;
}
aplicarYa();
return {
 tipo:'dibujo',
 // Una hoja escondida mide cero: el plano se pinta al asomarse a ella, no antes.
 activar:function(){ aplicarYa(); },
 desactivar:function(){soltarTrazo();},
 medir:function(){ if(plano) plano.medir(); },
 capas:plano?plano.capas:null,
 soloLineas:plano?plano.soloLineas:null,
 esSoloLineas:plano?plano.esSoloLineas:null,
 hayRellenos:plano?plano.hayRellenos:null,
 encajar:encajar,
 zoom:function(f){var r=svg.getBoundingClientRect();zoom(f,r.left+r.width/2,r.top+r.height/2);},
 // El encuadre de ahora y cómo ponerlo: es lo que deja imprimir solo un trozo. Ver `marcarZona`.
 vista:function(){return {x:v.x,y:v.y,w:v.w,h:v.h};},
 // **¿Me he perdido?** Con la mano apoyada, presentando, salen pinzas involuntarias que
 // llevan el encuadre a kilómetros del dibujo: la pantalla se queda vacía —negra, con este
 // papel— y presentando no hay botón de encajar a mano, así que no había vuelta atrás
 // (usuario, 16-sep-2026). Esto dice si lo que se ve ya no toca al dibujo, o si lo que
 // queda de él en pantalla es una mota.
 perdido:function(){
   // **Perdido es no ver nada del dibujo**, y solo eso. Con un «casi nada» se colaban
   // encuadres legítimos —alejarse del todo, o meterse en un detalle— y la pantalla daba un
   // salto sola, que es peor que el problema.
   var ix=Math.min(v.x+v.w,casa.x+casa.w)-Math.max(v.x,casa.x);
   var iy=Math.min(v.y+v.h,casa.y+casa.h)-Math.max(v.y,casa.y);
   return ix<=0||iy<=0;
 },
 ponerVista:function(c){ v={x:c.x,y:c.y,w:c.w,h:c.h}; aplicar(); },
 deEscena:function(px,py){ return aEscena(px,py); },
 // **Salir de medir ya no borra las cotas**: para eso están puestas. Lo que se deja a medias
 // —un primer punto sin su pareja— sí se suelta, que no es nada todavía. Se quitan todas con
 // la tecla de escape, y una a una arrastrando su punta sobre la otra. Ver [quitarMedida].
 modo:function(m){ modo=m; if(m!=='medir'){ medida=null; if(grupoMedida) pintarMedida(); } else api.decir('Toca dos puntos: la cota se queda puesta'); },
 pintando:function(){ return !hayLapiz && modo!=='mano' && modo!=='medir'; },
 herramientas:permitidasDe(['mano','lapiz','marcador','goma','medir']),
 deshacer:deshacer, rehacer:rehacer,
 puedeDeshacer:function(){return hecho.length>0;},
 puedeRehacer:function(){return rehecho.length>0;},
 rayas:rayasComoTexto
};
}
