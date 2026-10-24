
(function(){
"use strict";
var PLANTILLA='<!DOCTYPE html>\n'+document.documentElement.outerHTML;
function id(x){return document.getElementById(x);}
var cajaLienzo=id('lienzo'), estado=id('estado'), cajon=id('cajon');
var hojas=[].slice.call(document.querySelectorAll('#lienzo .hoja'));
if(!hojas.length) return;
// **La página se declara viva aquí y no antes.** De esta clase cuelga todo lo que le quita
// el control al navegador —el recorte a la ventana y el `touch-action` del dibujo—, así que
// se pone cuando ya se sabe que hay armazón que lo sustituya. Ver la hoja de estilo.
document.documentElement.className+=' vivo';
var color=(document.querySelector('#colores .activo')||{dataset:{}}).dataset.color||'#ff1744';
var grosor=+((document.querySelector('#grosores .activo')||{dataset:{}}).dataset.grosor||4);
var iHoja=-1, actual=null, avisoPendiente=null;

var api={
  decir:function(t){ estado.textContent=t||''; },
  refrescar:refrescar,
  agarrado:function(si){ cajaLienzo.classList.toggle('agarrado',!!si); },
  color:function(){return color;},
  grosor:function(){return grosor;}
};
var pagina=hojas.map(function(d){
  if(d.dataset.tipo==='espacio'){
    var t=d.querySelector('script.datos');
    return crearEspacio(d, JSON.parse(t.textContent), api);
  }
  if(d.dataset.tipo==='nota') return crearNota(d);
  if(d.dataset.tipo==='tabla') return crearTabla(d, api);
  return crearDibujo(d, api);
});
// **Una nota no se pinta: la compone el navegador**, y de ahí le viene lo que un dibujo no
// puede dar —se reparte al ancho que haya, se busca con la lupa del navegador y se puede
// seleccionar y copiar—. Lo que sí lleva es una **capa de tinta encima**: con el lápiz o el
// resaltador en la mano se raya sobre el texto, se subraya y se tacha, igual que en una hoja;
// con la mano, la capa no recibe el dedo y el texto vuelve a ser texto. Ver `.tinta`.
function crearNota(d){
  var NS='http://www.w3.org/2000/svg';
  // **Un documento para anotar** —Word, libro o PDF— es una nota con la columna de ancho fijo
  // y un margen a cada lado: se amplía entero (texto, márgenes y tinta a la vez) en vez de
  // cambiar la letra, y lo anotado va atado a su bloque. Ver [HojaWeb.Documento].
  var caja=d.querySelector('.doc-caja'), esDoc=!!caja, z=1, N=[];
  var art=d.querySelector('.nota')||d.querySelector('.doc'), svg=d.querySelector('svg.tinta'), tam=16;
  var croquis=svg&&svg.querySelector('#croquis');
  if(svg&&!croquis){croquis=document.createElementNS(NS,'g');croquis.id='croquis';svg.appendChild(croquis);}
  var hecho=[], rehecho=[], modo='mano', trazo=null;
  function poner(v){
    if(esDoc){
      // El centro de lo que se mira se queda en su sitio al ampliar.
      var cx=(d.scrollLeft+d.clientWidth/2)/z, cy=(d.scrollTop+d.clientHeight/2)/z;
      tam=Math.min(Math.max(v,4),64); z=tam/16;
      caja.style.transform=z===1?'':'scale('+z+')';
      medir();
      d.scrollLeft=cx*z-d.clientWidth/2; d.scrollTop=cy*z-d.clientHeight/2;
      return;
    }
    tam=Math.min(Math.max(v,11),30); if(art) art.style.fontSize=tam+'px'; medir();
  }
  // A qué altura cae ahora cada bloque del documento, en píxeles suyos (sin la ampliación).
  function alturas(){
    var q=art.querySelectorAll(caja.dataset.bloques||'p'), c=caja.getBoundingClientRect().top, n=[];
    for(var i=0;i<q.length;i++) n.push((q[i].getBoundingClientRect().top-c)/z);
    return n;
  }
  // **Cada cosa, junto a su párrafo.** Otro navegador tiene otras letras y el texto no mide
  // lo mismo de alto: lo anotado en la aplicación se corre lo que se haya corrido su bloque
  // desde que se exportó, y lo rayado aquí, lo que se haya corrido desde que se rayó.
  function colocar(){
    N=alturas();
    var T=(caja.dataset.tops||'').split(',').filter(String).map(Number);
    var e=caja.querySelectorAll('[data-y]');
    for(var k=0;k<e.length;k++){
      var i=+e[k].getAttribute('data-i'), y=+e[k].getAttribute('data-y'), f=e[k].getAttribute('data-f');
      if(f!==null&&i<0) y=(+f)*caja.offsetHeight;
      var c=(i>=0&&i<N.length&&i<T.length)?N[i]-T[i]:0;
      e[k].style.top=(y+c)+'px'; e[k]._y=y+c;
    }
    [].forEach.call(croquis.children,function(r){
      if(!r.hasAttribute('data-i')) return;
      var i=+r.getAttribute('data-i'), dy=i<N.length?N[i]-(+r.getAttribute('data-t')):0;
      r.dy=Math.abs(dy)<0.05?0:dy;
      if(r.dy) r.setAttribute('transform','translate(0,'+r3(r.dy)+')'); else r.removeAttribute('transform');
    });
  }
  // La capa mide lo que mida el texto, no lo que mida la ventana: lo rayado tiene que
  // quedarse donde se rayó aunque después se desplace la nota o cambie el tamaño de la letra.
  function medir(){
    if(!svg||!art) return;
    if(esDoc){
      // La caja se amplía con `transform`, que no mueve lo de alrededor: lo que ocupa de más o
      // de menos se le dice al desplazamiento con los márgenes.
      var W=caja.offsetWidth, H=caja.offsetHeight;
      svg.setAttribute('width',W); svg.setAttribute('height',H);
      svg.setAttribute('viewBox','0 0 '+W+' '+H);
      svg.style.height=H+'px';
      caja.style.marginLeft=Math.max(0,(d.clientWidth-W*z)/2)+'px';
      caja.style.marginRight=(W*z-W)+'px';
      caja.style.marginBottom=(H*z-H)+'px';
      colocar();
      return;
    }
    var alto=Math.max(art.scrollHeight,d.clientHeight);
    svg.setAttribute('height',alto);
    svg.setAttribute('viewBox','0 0 '+d.clientWidth+' '+alto);
    svg.style.height=alto+'px';
  }
  function donde(e){
    var r=svg.getBoundingClientRect();
    return {x:(e.clientX-r.left)/z, y:(e.clientY-r.top)/z};
  }
  function r3(x){return Math.round(x*1000)/1000;}
  // Una raya que ya venía en el archivo —guardada en otra sesión— se borra como las demás:
  // sus puntos se leen de su `d`, que aquí es siempre M y L.
  function puntosDe(r){
    var m=(r.getAttribute('d')||'').match(/-?[\d.]+/g)||[], o=[];
    for(var i=0;i+1<m.length;i+=2) o.push({x:+m[i],y:+m[i+1]});
    return o;
  }
  // Lo que puso el lápiz, recta a recta, y a medio píxel: ver `anadirPunto` del dibujo.
  function anadirPunto(t,p){
    var pts=t.puntos, n=pts.length;
    pts.push(p);
    var x=Math.round(p.x*2)/2, y=Math.round(p.y*2)/2;
    if(n===0){t.abierto='M'+x+' '+y;return;}
    t.abierto+=(n===1?'L':' ')+x+' '+y;
  }
  function pintar(t){ t.setAttribute('d',t.abierto); }
  function muestras(e){
    var lote=(e.getCoalescedEvents&&e.getCoalescedEvents())||[];
    return lote.length?lote:[e];
  }
  function borrarEn(p){
    var rayas=[].slice.call(croquis.children);
    for(var i=0;i<rayas.length;i++){
      var r=rayas[i], pts=r.puntos||[], gordo=(+r.getAttribute('stroke-width')||2)/2+10;
      for(var j=0;j<pts.length;j++){
        if(Math.hypot(pts[j].x-p.x,pts[j].y+(r.dy||0)-p.y)<=gordo){
          hecho.push({que:'borra',raya:r,antes:r.nextSibling}); rehecho.length=0;
          croquis.removeChild(r); api.refrescar(); break;
        }
      }
    }
  }
  if(svg){
    svg.addEventListener('pointerdown',function(e){
      if(modo==='mano') return;
      e.preventDefault();
      svg.setPointerCapture(e.pointerId);
      var p=donde(e);
      if(modo==='goma'){ borrarEn(p); trazo=null; return; }
      var marca=(modo==='marcador');
      trazo=document.createElementNS(NS,'path');
      trazo.setAttribute('fill','none');
      trazo.setAttribute('stroke',api.color());
      trazo.setAttribute('stroke-width',marca?api.grosor()*3:api.grosor());
      trazo.setAttribute('stroke-linecap','round');
      trazo.setAttribute('stroke-linejoin','round');
      if(marca){trazo.setAttribute('stroke-opacity','0.45');trazo.setAttribute('class','marca');}
      trazo.puntos=[]; anadirPunto(trazo,p); pintar(trazo);
      croquis.appendChild(trazo);
    });
    svg.addEventListener('pointermove',function(e){
      if(modo==='mano') return;
      if(modo==='goma'){ if(e.buttons) borrarEn(donde(e)); return; }
      if(!trazo) return;
      var lote=muestras(e);
      for(var i=0;i<lote.length;i++) anadirPunto(trazo,donde(lote[i]));
      pintar(trazo);
    });
    function soltar(){
      if(!trazo) return;
      if(trazo.puntos.length<2) croquis.removeChild(trazo);
      else {
        // Atada al bloque junto al que se rayó, y a qué altura caía él entonces. Ver `colocar`.
        if(esDoc&&N.length){
          var y=trazo.puntos[0].y, b=-1;
          for(var i=0;i<N.length;i++) if(N[i]<=y+6&&(b<0||N[i]>=N[b])) b=i;
          if(b>=0){ trazo.setAttribute('data-i',b); trazo.setAttribute('data-t',r3(N[b])); }
        }
        hecho.push({que:'pinta',raya:trazo}); rehecho.length=0; api.refrescar();
      }
      trazo=null;
    }
    svg.addEventListener('pointerup',soltar);
    svg.addEventListener('pointercancel',soltar);
    [].forEach.call(croquis.children,function(r){ r.puntos=puntosDe(r); });
  }
  function encajarDoc(){
    // Se abre **viendo el texto de borde a borde**, como en la aplicación, con lo anotado a los
    // lados a un gesto; en una pantalla donde cabe todo, entero y a su tamaño.
    var col=+caja.dataset.columna, m=+caja.dataset.margen, cw=d.clientWidth||col;
    z=1; poner(16*(cw>=col+2*m?1:Math.min(Math.max(cw/col,0.3),1.25)));
    d.scrollLeft=Math.max(0,m*z-Math.max(0,(cw-col*z)/2)); d.scrollTop=0;
  }
  if(esDoc){
    [].forEach.call(d.querySelectorAll('.pprail button'),function(b){
      b.onclick=function(){
        var m=document.getElementById(b.getAttribute('data-m'));
        if(m) d.scrollTo({left:d.scrollLeft,top:Math.max(0,(m._y||0)*z-24),behavior:'smooth'});
      };
    });
    window.addEventListener('load',function(){ if(!d.hidden) medir(); });
    if(document.fonts&&document.fonts.ready) document.fonts.ready.then(function(){ if(!d.hidden) medir(); });
  }
  var encajado=false;
  return {
    tipo:'nota',
    activar:function(){ if(esDoc&&!encajado){ encajado=true; encajarDoc(); } else medir(); },
    desactivar:function(){}, medir:medir,
    encajar:function(){ if(esDoc) return encajarDoc(); poner(16); d.scrollTop=0; },
    zoom:function(f){ poner(esDoc?tam/f:Math.round(tam/f)); },
    modo:function(m){ modo=m; },
    pintando:function(){ return modo!=='mano'; },
    herramientas:['mano','lapiz','marcador','goma'].filter(function(h){
      return (document.body.dataset.herramientas||'mano lapiz marcador goma').split(' ').indexOf(h)>=0; }),
    deshacer:function(){
      var u=hecho.pop(); if(!u)return;
      if(u.que==='pinta'){ if(u.raya.parentNode)croquis.removeChild(u.raya); }
      else croquis.insertBefore(u.raya,u.antes&&u.antes.parentNode===croquis?u.antes:null);
      rehecho.push(u); api.refrescar();
    },
    rehacer:function(){
      var u=rehecho.pop(); if(!u)return;
      if(u.que==='pinta') croquis.appendChild(u.raya);
      else { u.antes=u.raya.nextSibling; if(u.raya.parentNode)croquis.removeChild(u.raya); }
      hecho.push(u); api.refrescar();
    },
    puedeDeshacer:function(){return hecho.length>0;},
    puedeRehacer:function(){return rehecho.length>0;},
    rayas:function(){
      var s='';
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
  };
}
function grupoDe(i){ return pagina[i]&&pagina[i].tipo==='dibujo'?pagina[i]:null; }

function refrescar(){
  var d=id('deshacer'), r=id('rehacer'), td=id('t-deshacer'), tr=id('t-rehacer');
  if(d) d.disabled=!(actual&&actual.puedeDeshacer&&actual.puedeDeshacer());
  if(r) r.disabled=!(actual&&actual.puedeRehacer&&actual.puedeRehacer());
  if(td) td.disabled=!(actual&&actual.puedeDeshacer&&actual.puedeDeshacer());
  if(tr) tr.disabled=!(actual&&actual.puedeRehacer&&actual.puedeRehacer());
}
function marcarHerramienta(){
  ['mano','lapiz','marcador','goma','girar','mover','medir'].forEach(function(n){
    var b=id(n); if(b) b.classList.toggle('activo', actual&&actual.modoActual===n);
  });
  var p=id('paleta');
  if(p) p.hidden=!(actual&&(actual.modoActual==='lapiz'||actual.modoActual==='marcador'));
  cajaLienzo.classList.toggle('pintando', !!(actual&&actual.pintando&&actual.pintando()));
  cajaLienzo.classList.toggle('midiendo', !!(actual&&actual.modoActual==='medir'));
  [].forEach.call(document.querySelectorAll('#presentacion [data-m]'),function(b){
    b.classList.toggle('activo', !!(actual&&actual.modoActual===b.dataset.m));
    b.hidden=!(actual&&actual.herramientas.indexOf(b.dataset.m)>=0);
  });
}
function elegir(m){
  if(!actual||actual.herramientas.indexOf(m)<0) return;
  actual.modoActual=(actual.modoActual===m&&m!=='mano'&&m!=='girar')?actual.herramientas[0]:m;
  actual.modo(actual.modoActual);
  marcarHerramienta();
}
function irA(i){
  if(i<0||i>=pagina.length||i===iHoja) return;
  if(actual) actual.desactivar();
  iHoja=i; actual=pagina[i];
  hojas.forEach(function(d,k){d.hidden=k!==i;});
  document.body.style.background=hojas[i].dataset.fondo||'';
  document.body.classList.toggle('oscuro',hojas[i].dataset.oscuro==='1');
  document.body.classList.toggle('enElEspacio',hojas[i].dataset.tipo==='espacio');
  var esEspacio=hojas[i].dataset.tipo==='espacio';
  var esTabla=hojas[i].dataset.tipo==='tabla';
  document.body.classList.toggle('enLaTabla',esTabla);
  var hayPlano=hojas[i].dataset.plano==='1';
  // Una nota lleva los mismos mandos que un dibujo: se raya encima igual. Ver [crearNota].
  // Una tabla también: el lápiz raya encima de las celdas, y además lleva su grupo aparte.
  [].forEach.call(document.querySelectorAll('.solo-dibujo'),function(g){g.hidden=esEspacio;});
  [].forEach.call(document.querySelectorAll('.solo-tabla'),function(g){g.hidden=!esTabla;});
  [].forEach.call(document.querySelectorAll('.solo-espacio'),function(g){g.hidden=!esEspacio;});
  [].forEach.call(document.querySelectorAll('.solo-plano'),function(g){g.hidden=!hayPlano;});
  var mide=actual&&actual.herramientas.indexOf('medir')>=0;
  [].forEach.call(document.querySelectorAll('.solo-medir'),function(g){g.hidden=!mide;});
  if(!actual.modoActual) actual.modoActual=actual.herramientas[0];
  actual.activar();
  actual.modo(actual.modoActual);
  var a=id('anterior'), s=id('siguiente');
  if(a) a.disabled=i===0;
  if(s) s.disabled=i===pagina.length-1;
  if(!cajon.hidden){cajon.hidden=true;cajon.dataset.que='';}
  // El índice marca la hoja de ahora, y la dirección la nombra: así un enlace lleva a ella.
  [].forEach.call(document.querySelectorAll('#indice-fijo a'),function(a){a.classList.toggle('activo',+a.dataset.i===i);});
  var tit=document.querySelector('#indice-fijo .titulo'), cue=document.querySelector('#indice-fijo .cuenta');
  if(tit) tit.textContent=hojas[i].dataset.nombre||('Hoja '+(i+1));
  if(cue) cue.textContent=(i+1)+' / '+hojas.length;
  try{ if(location.hash!=='#hoja-'+(i+1)) history.replaceState(null,'','#hoja-'+(i+1)); }catch(e){}
  api.decir('');
  marcarHerramienta(); refrescar();
}
function desdeLaDireccion(){
  var h=location.hash||'';
  if(h.indexOf('#hoja-')!==0) return;
  var n=parseInt(h.slice(6),10);
  if(n>0) irA(n-1);
}
window.addEventListener('hashchange',desdeLaDireccion);
(function(){
  var d=id('indice-fijo');
  if(!d) return;
  if(window.innerWidth<600) d.open=false;
  [].forEach.call(d.querySelectorAll('a'),function(a){
    a.addEventListener('click',function(e){ e.preventDefault(); irA(+a.dataset.i); if(window.innerWidth<600) d.open=false; });
  });
})();

// ---- Los cajones: páginas, vistas y grupos ----
function abrir(titulo,filas,pie){
  if(cajon.dataset.que===titulo&&!cajon.hidden){cajon.hidden=true;cajon.dataset.que='';return;}
  cajon.dataset.que=titulo; cajon.hidden=false; cajon.innerHTML='';
  var t=document.createElement('h2'); t.textContent=titulo; cajon.appendChild(t);
  filas.forEach(function(f){cajon.appendChild(f);});
  if(pie) cajon.appendChild(pie);
}
function fila(texto,al,marcada){
  var d=document.createElement('div'); d.className='fila'+(marcada?' activo':''); d.tabIndex=0;
  var n=document.createElement('span'); n.className='n'; n.textContent=texto;
  d.appendChild(n);
  d.onclick=al;
  d.onkeydown=function(e){if(e.key==='Enter'||e.key===' '){e.preventDefault();al();}};
  return d;
}
function boton(t,al){var b=document.createElement('button');b.textContent=t;b.onclick=al;return b;}
if(id('indice')) id('indice').onclick=function(){
  abrir('Páginas', hojas.map(function(d,k){
    return fila(d.dataset.nombre||('Hoja '+(k+1)),function(){irA(k);cajon.hidden=true;cajon.dataset.que='';},k===iHoja);
  }));
};
if(id('anterior')) id('anterior').onclick=function(){irA(iHoja-1);};
if(id('siguiente')) id('siguiente').onclick=function(){irA(iHoja+1);};
if(id('vistas')) id('vistas').onclick=function(){
  if(!actual||!actual.vistas) return;
  abrir('Vistas', actual.vistas().map(function(v){
    return fila(v.n,function(){v.ir();cajon.hidden=true;cajon.dataset.que='';});
  }));
};
if(id('grupos')) id('grupos').onclick=function(){ pintarGrupos(); };
if(id('escena')) id('escena').onclick=function(){ pintarEscena(); };
if(id('guia')) id('guia').onclick=function(){ alternarGuia(); };
// **La escena del croquis**: interruptores, y abajo el OBJ para llevárselo a Blender.
function pintarEscena(){
  if(!actual||!actual.escena) return;
  var lista=actual.escena();
  var filas=lista.map(function(g){
    var d=document.createElement('div'); d.className='fila';
    var c=document.createElement('input'); c.type='checkbox'; c.checked=g.puesto();
    c.onchange=function(){g.poner(c.checked);};
    var n=document.createElement('span'); n.className='n'; n.textContent=g.nombre;
    n.onclick=function(){c.checked=!c.checked;c.onchange();};
    d.appendChild(c); d.appendChild(n);
    return d;
  });
  var pie=document.createElement('div'); pie.className='mini';
  if(actual.obj) pie.appendChild(boton('Bajar OBJ',function(){ bajarObj(); }));
  pie.appendChild(boton('Guía',function(){ alternarGuia(); }));
  abrir('Escena',filas,pie);
}
// El giradiscos se para solo al tocar: el cajón, si está abierto, tiene que enterarse.
api.escenaCambio=function(){ if(!cajon.hidden&&cajon.dataset.que==='Escena'){ cajon.dataset.que=''; pintarEscena(); } };
function bajarObj(){
  if(!actual||!actual.obj) return;
  var m=actual.obj(), nombre=(document.body.dataset.nombre||'croquis').replace(/[\\/:*?"<>|]+/g,'-').trim()||'croquis';
  function baja(texto,archivo){
    var a=document.createElement('a');
    a.href=URL.createObjectURL(new Blob([texto],{type:'text/plain'})); a.download=archivo;
    document.body.appendChild(a); a.click(); document.body.removeChild(a);
    setTimeout(function(){URL.revokeObjectURL(a.href);},4000);
  }
  baja(m.obj,nombre+'.obj'); setTimeout(function(){ baja(m.mtl,'croquis.mtl'); },300);
  avisar('OBJ y MTL bajados: ábrelos juntos');
}
// **La guía de gestos**: una tarjeta con lo que hace cada dedo, cada botón y cada tecla.
function alternarGuia(){
  var g=id('guia-caja');
  if(g){ g.parentNode.removeChild(g); return; }
  if(!actual||!actual.guia) return;
  g=document.createElement('div'); g.id='guia-caja';
  var t=document.createElement('div'); t.className='guia';
  var h=document.createElement('h2'); h.textContent='Cómo se mueve'; t.appendChild(h);
  actual.guia().forEach(function(b){
    var h3=document.createElement('h3'); h3.textContent=b.t; t.appendChild(h3);
    b.f.forEach(function(f){
      var r=document.createElement('div'); r.className='par';
      var a=document.createElement('span'); a.textContent=f[0];
      var c=document.createElement('span'); c.textContent=f[1];
      r.appendChild(a); r.appendChild(c); t.appendChild(r);
    });
  });
  var cerrar=boton('Entendido',function(){ alternarGuia(); }); cerrar.className='cerrar';
  t.appendChild(cerrar);
  g.appendChild(t);
  g.onclick=function(e){ if(e.target===g) alternarGuia(); };
  document.body.appendChild(g);
}
if(id('capas')) id('capas').onclick=function(){ pintarCapas(); };
// **Las capas del plano**: las mismas que traía el PDF de AutoCAD. Apagar las que estorban
// —las tramas, los sombreados— deja el dibujo en sus líneas, que es lo que se quiere ver, y
// además lo pinta más deprisa. `Solo líneas` hace eso de un golpe con todo lo relleno.
function pintarCapas(){
  if(!actual||!actual.capas) return;
  var lista=actual.capas();
  var filas=lista.map(function(g){
    var d=document.createElement('div'); d.className='fila';
    var c=document.createElement('input'); c.type='checkbox'; c.checked=g.puesto();
    c.onchange=function(){g.poner(c.checked);};
    var n=document.createElement('span'); n.className='n'; n.textContent=g.nombre;
    n.onclick=function(){c.checked=!c.checked;c.onchange();};
    var z=document.createElement('span'); z.className='lupa'; z.textContent='⤢';
    z.title='Ir a esta capa';
    z.onclick=function(ev){ev.stopPropagation();g.acercarse();};
    d.appendChild(c); d.appendChild(n); d.appendChild(z);
    return d;
  });
  if(!filas.length) filas=[fila('Este plano no trae capas',function(){})];
  var pie=document.createElement('div'); pie.className='mini';
  pie.appendChild(boton('Todas',function(){lista.forEach(function(g){g.poner(true);});refrescarCapas();}));
  pie.appendChild(boton('Ninguna',function(){lista.forEach(function(g){g.poner(false);});refrescarCapas();}));
  if(actual.hayRellenos&&actual.hayRellenos()){
    var b=boton(actual.esSoloLineas()?'Con relleno':'Solo líneas',function(){
      actual.soloLineas(!actual.esSoloLineas()); refrescarCapas();
    });
    pie.appendChild(b);
  }
  abrir('Capas',filas,pie);
}
function refrescarCapas(){ cajon.dataset.que=''; pintarCapas(); }
function pintarGrupos(){
  if(!actual||!actual.grupos) return;
  var lista=actual.grupos();
  if(!lista.length){ abrir('Grupos',[fila('Este croquis no tiene grupos',function(){})]); return; }
  var filas=lista.map(function(g){
    var d=document.createElement('div'); d.className='fila';
    var c=document.createElement('input'); c.type='checkbox'; c.checked=g.puesto();
    c.onchange=function(){g.poner(c.checked);};
    var n=document.createElement('span'); n.className='n'; n.textContent=g.nombre;
    n.onclick=function(){c.checked=!c.checked;c.onchange();};
    var z=document.createElement('span'); z.className='lupa'; z.textContent='⤢';
    z.title='Acercarse a este grupo';
    z.onclick=function(ev){ev.stopPropagation();g.acercarse();};
    d.appendChild(c); d.appendChild(n); d.appendChild(z);
    return d;
  });
  var pie=document.createElement('div'); pie.className='mini';
  pie.appendChild(boton('Todos',function(){lista.forEach(function(g){g.poner(true);});refrescarGrupos();}));
  pie.appendChild(boton('Ninguno',function(){lista.forEach(function(g){g.poner(false);});refrescarGrupos();}));
  abrir('Grupos',filas,pie);
}
// Repintar el cajón sin cerrarlo: `abrir` alterna, así que se le quita la marca antes.
function refrescarGrupos(){ cajon.dataset.que=''; pintarGrupos(); }

// ---- La barra ----
['mano','lapiz','marcador','goma','girar','mover','medir'].forEach(function(n){
  var b=id(n); if(b) b.onclick=function(){elegir(n);};
});
if(id('deshacer')) id('deshacer').onclick=function(){if(actual.deshacer)actual.deshacer();};
if(id('rehacer')) id('rehacer').onclick=function(){if(actual.rehacer)actual.rehacer();};
if(id('t-deshacer')) id('t-deshacer').onclick=function(){if(actual.deshacer)actual.deshacer();};
if(id('t-rehacer')) id('t-rehacer').onclick=function(){if(actual.rehacer)actual.rehacer();};
['t-negrita','t-suma','t-copiar','t-pegar','t-csv'].forEach(function(n){
  var b=id(n);
  // `mousedown` sin foco: pulsar el botón no le quita la celda elegida a la tabla.
  if(b){ b.onmousedown=function(e){e.preventDefault();}; b.onclick=function(){ if(actual&&actual.accion)actual.accion(n); }; }
});
if(id('encajar')) id('encajar').onclick=function(){actual.encajar();};
if(id('mas')) id('mas').onclick=function(){if(actual.zoom)actual.zoom(1/1.3);};
if(id('menos')) id('menos').onclick=function(){if(actual.zoom)actual.zoom(1.3);};
document.querySelectorAll('#colores .color').forEach(function(b){
  b.onclick=function(){color=b.dataset.color;marcar('#colores .color',b);};
});
document.querySelectorAll('#grosores .grosor').forEach(function(b){
  b.onclick=function(){grosor=+b.dataset.grosor;marcar('#grosores .grosor',b);};
});
function marcar(sel,cual){
  document.querySelectorAll(sel).forEach(function(o){o.classList.toggle('activo',o===cual);});
}

// ---- Guardar y compartir: la página se reescribe a sí misma ----
function avisar(texto){
  var a=id('aviso'); a.textContent=texto; a.hidden=false;
  clearTimeout(avisoPendiente); avisoPendiente=setTimeout(function(){a.hidden=true;},1800);
}
// Se reescribe el grupo de cada dibujo, en orden: la primera copia del patrón es la del
// primer dibujo. Las páginas del espacio no tienen grupo y no cuentan.
function paginaAnotada(){
  // Los dibujos y las notas llevan grupo; las páginas del espacio, no.
  var dibujos=pagina.filter(function(p){return p&&(p.tipo==='dibujo'||p.tipo==='nota'||p.tipo==='tabla');}), i=0;
  var re=/<g id="croquis"[^>]*>[\s\S]*?<\/g>/g;
  // **Solo los primeros, y son los del dibujo.**
  //
  // La plantilla es el documento **entero**, y el documento incluye este mismo guion, que
  // habla de `<g id="croquis">` dos veces: en el patrón de aquí arriba y en el texto que
  // devuelve esta función. Con una `replace` global, guardar reescribía también **su propia
  // fuente** —153 bytes, medidos—, el guion quedaba con un error de sintaxis y el archivo
  // guardado se abría **muerto**: sin poder dibujar, sin poder ampliar, sin nada. Es el fallo
  // que reportó el usuario el 8-sep-2026 («lo edito, lo guardo, lo reenvío, y ya no puedo
  // hacer zoom ni manejar ni nada»), y solo salía en el archivo guardado, nunca en el
  // exportado, porque hace falta guardar una vez para estropearlo.
  //
  // Los grupos de verdad están en el `#lienzo`, que va **antes** del guion: son las primeras
  // coincidencias, tantas como hojas con grupo. Lo que venga después es el guion hablando de
  // sí mismo y se deja **tal cual**.
  var salida=PLANTILLA.replace(re,function(entero){
    if(i>=dibujos.length) return entero;
    var p=dibujos[i++];
    return p?'<g id="croquis">\n'+p.rayas()+'</g>':'<g id="croquis"></g>';
  });
  // **Las tablas: su JSON y su tabla ya calculada**, con el mismo tope. Las etiquetas se
  // escriben a trozos para que el patrón no se encuentre a sí mismo en este guion.
  var tablas=pagina.filter(function(p){return p&&p.tipo==='tabla';});
  if(tablas.length){
    var j=0, m=0, abre='<'+'script type="application/json" class="'+'tabla">';
    salida=salida.replace(new RegExp(abre+'[\\s\\S]*?<\\/script>','g'),function(entero){
      return j>=tablas.length?entero:abre+tablas[j++].json()+'<\/script>';
    });
    salida=salida.replace(new RegExp('<'+'table class="'+'calc">[\\s\\S]*?<\\/table>','g'),function(entero){
      return m>=tablas.length?entero:tablas[m++].estatica();
    });
  }
  return salida;
}
// Para quien enseña esta página por dentro de una aplicación: así sabe si hay cambios sin guardar.
window.paginaAnotada=paginaAnotada;
function nombreDelArchivo(){
  var n=(document.body.dataset.nombre||document.title||'dibujo').replace(/[\\/:*?"<>|]+/g,'-').trim();
  return (n||'dibujo')+'.html';
}
function descargar(){
  var blob=new Blob([paginaAnotada()],{type:'text/html'});
  var a=document.createElement('a');
  a.href=URL.createObjectURL(blob); a.download=nombreDelArchivo();
  document.body.appendChild(a); a.click(); document.body.removeChild(a);
  setTimeout(function(){URL.revokeObjectURL(a.href);},4000);
  avisar('Guardado: '+a.download);
}
function compartir(){
  var archivo;
  try{ archivo=new File([paginaAnotada()],nombreDelArchivo(),{type:'text/html'}); }catch(e){ return descargar(); }
  if(navigator.share&&navigator.canShare&&navigator.canShare({files:[archivo]})){
    navigator.share({files:[archivo],title:document.title}).catch(function(){});
  } else { descargar(); avisar('Guardado; compártelo desde tus descargas'); }
}
// **Guardados con red.** Estos botones se pueden dejar fuera al exportar, y un `null.onclick`
// aquí tumbaba el armazón entero: la página arrancaba sin `irA(0)` y no funcionaba ningún
// botón, ni el de medir. Lo reportó el usuario exportando con una sola función marcada.
var puedeGuardar=!!id('guardar');
if(id('guardar')) id('guardar').onclick=descargar;
if(id('compartir')) id('compartir').onclick=compartir;

addEventListener('resize',function(){ if(actual&&actual.medir) actual.medir(); });
document.addEventListener('keydown',function(e){
  // La tabla primero: las letras que se escriben en ella no son atajos. Y en cualquier campo
  // de texto, tampoco.
  if(actual&&actual.tecla&&actual.tecla(e)) return;
  var tg=e.target;
  if(tg&&(tg.tagName==='INPUT'||tg.tagName==='TEXTAREA'||tg.isContentEditable)) return;
  var ctrl=e.ctrlKey||e.metaKey, k=(e.key||'').toLowerCase();
  if(ctrl&&k==='z'&&!e.shiftKey){e.preventDefault();if(actual.deshacer)actual.deshacer();return;}
  if(ctrl&&(k==='y'||(k==='z'&&e.shiftKey))){e.preventDefault();if(actual.rehacer)actual.rehacer();return;}
  if(ctrl&&k==='s'){e.preventDefault();if(puedeGuardar)descargar();return;}
  if(ctrl)return;
  if(e.key==='PageDown')irA(iHoja+1);
  else if(e.key==='PageUp')irA(iHoja-1);
  else if(k==='v'||k==='h')elegir(actual&&actual.tipo==='dibujo'?'mano':'mover');
  else if(k==='p')elegir('lapiz');
  else if(k==='m')elegir('marcador');
  else if(k==='e')elegir('goma');
  else if(k==='g')elegir('girar');
  else if(k==='d')elegir('medir');
  else if(k==='0')actual.encajar();
  else if(k==='o'&&actual.orto)actual.orto();
  else if(k===']'&&actual.lente)actual.lente(0.08);
  else if(k==='['&&actual.lente)actual.lente(-0.08);
  else if(k==='t'&&actual.giradiscos){actual.giradiscos();api.escenaCambio();}
  else if(k==='?'||(k==='/'&&e.shiftKey))alternarGuia();
  else if((k==='+'||k==='=')&&actual.zoom)actual.zoom(1/1.3);
  else if(k==='-'&&actual.zoom)actual.zoom(1.3);
  else if(k==='escape'){cajon.hidden=true;cajon.dataset.que='';api.decir('');}
});
// ---- Presentar (14-sep-2026) ----
// La hoja a pantalla completa y una pastilla abajo. Con la mano puesta, un toque en el tercio
// derecho o un barrido a la izquierda pasa a la siguiente; en el izquierdo, a la anterior; en
// el medio esconde la pastilla. Con el lápiz se anota encima como siempre.
var raiz=document.documentElement, pastilla=id('presentacion'), presentando=false, toque=null;
function cuenta(){
  if(!pastilla) return;
  id('p-cuenta').textContent=(iHoja+1)+' / '+pagina.length;
  id('p-anterior').disabled=iHoja<=0;
  id('p-siguiente').disabled=iHoja>=pagina.length-1;
}
function pasar(d){
  var antes=iHoja;
  irA(iHoja+d);
  if(iHoja!==antes&&actual&&actual.encajar) actual.encajar();
  cuenta();
}
function presentar(){
  if(!pastilla) return;
  presentando=true; raiz.classList.add('presentando'); pastilla.hidden=false;
  pastilla.classList.remove('escondida');
  if(actual&&actual.herramientas.indexOf('mano')>=0) elegir('mano');
  if(raiz.requestFullscreen) raiz.requestFullscreen().catch(function(){});
  setTimeout(function(){ if(actual&&actual.medir) actual.medir(); if(actual&&actual.encajar) actual.encajar(); },250);
  marcarHerramienta(); cuenta();
}
function dejarDePresentar(){
  if(!presentando) return;
  presentando=false; raiz.classList.remove('presentando'); if(pastilla) pastilla.hidden=true;
  if(document.fullscreenElement&&document.exitFullscreen) document.exitFullscreen().catch(function(){});
  setTimeout(function(){ if(actual&&actual.medir) actual.medir(); },250);
}
function enModoPasar(){ return presentando&&(!actual||!actual.modoActual||actual.modoActual==='mano'||actual.modoActual==='mover'||actual.modoActual==='girar'); }
if(id('presentar')) id('presentar').onclick=presentar;
if(pastilla){
  id('p-anterior').onclick=function(){pasar(-1);};
  id('p-siguiente').onclick=function(){pasar(1);};
  id('p-salir').onclick=dejarDePresentar;
  if(id('p-encajar')) id('p-encajar').onclick=function(){ if(actual&&actual.encajar) actual.encajar(); };
  [].forEach.call(pastilla.querySelectorAll('[data-m]'),function(b){
    b.onclick=function(){
      if(!actual||actual.herramientas.indexOf(b.dataset.m)<0) return;
      actual.modoActual=b.dataset.m; actual.modo(b.dataset.m); marcarHerramienta();
      // **Un empujón de repintado**: cambiar de herramienta hace aparecer y desaparecer la
      // paleta, y en pantalla completa hay navegadores que dejan la capa vieja pegada. Tocar
      // el tamaño de la hoja obliga a componer de nuevo y se limpia.
      if(actual.medir) actual.medir();
      cajaLienzo.style.transform='translateZ(0)';
      void cajaLienzo.offsetHeight;
      cajaLienzo.style.transform='';
    };
  });
  // **Presentando también se mueve y se amplía** (16-sep-2026). Antes el dedo se le quitaba al
  // visor en cuanto empezaba —se tragaba el `pointerdown`—, así que presentando no se podía ni
  // desplazar el dibujo ni hacer pinza: solo pasar hojas. Ahora **no se le quita nada**: el
  // visor mueve y amplía como siempre, y aquí solo se mira, al levantar el dedo, si aquello fue
  // un toque o un barrido rápido de un solo dedo; si lo fue, se pasa de hoja y el encuadre se
  // rehace ([pasar] llama a `encajar`). Con dos dedos —una pinza— no se pasa nunca de hoja.
  var dedos=0;
  cajaLienzo.addEventListener('pointerdown',function(e){
    dedos++;
    if(!enModoPasar()) return;
    toque=dedos===1?{x:e.clientX,y:e.clientY,t:Date.now()}:null;
  },true);
  cajaLienzo.addEventListener('pointercancel',function(){ dedos=Math.max(0,dedos-1); toque=null; if(dedos===0) noPerderse(); },true);
  // Al soltar el último dedo, si el dibujo se ha ido de la pantalla, se vuelve a encajar.
  function noPerderse(){
    if(!actual||!actual.perdido) return;
    if(actual.perdido()&&actual.encajar) actual.encajar();
  }
  cajaLienzo.addEventListener('pointerup',function(e){
    var eranVarios=dedos>1;
    dedos=Math.max(0,dedos-1);
    if(dedos===0) noPerderse();
    if(!enModoPasar()||!toque||eranVarios){ toque=null; return; }
    var dx=e.clientX-toque.x, dy=e.clientY-toque.y, t=Date.now()-toque.t; toque=null;
    // Un barrido es rápido y derecho; uno lento es que estabas moviendo el dibujo y se respeta.
    if(t<500&&Math.abs(dx)>60&&Math.abs(dx)>Math.abs(dy)*1.5){ pasar(dx<0?1:-1); return; }
    if(Math.abs(dx)<12&&Math.abs(dy)<12){
      var w=innerWidth;
      if(e.clientX<w/3) pasar(-1);
      else if(e.clientX>w*2/3) pasar(1);
      else pastilla.classList.toggle('escondida');
    }
  },true);
  document.addEventListener('fullscreenchange',function(){ if(!document.fullscreenElement&&presentando) dejarDePresentar(); });
  document.addEventListener('keydown',function(e){
    if(e.key==='F5'){ e.preventDefault(); if(presentando) dejarDePresentar(); else presentar(); e.stopImmediatePropagation(); return; }
    if(!presentando) return;
    var k=e.key;
    if(k==='ArrowRight'||k==='PageDown'||k===' '){ e.preventDefault(); e.stopImmediatePropagation(); pasar(1); }
    else if(k==='ArrowLeft'||k==='PageUp'){ e.preventDefault(); e.stopImmediatePropagation(); pasar(-1); }
    else if(k==='Escape'){ e.stopImmediatePropagation(); dejarDePresentar(); }
  },true);
}

// ---- Imprimir (14-sep-2026) ----
// El diálogo del navegador, con el papel que se elija; la hoja de estilo de impresión pone
// una hoja por página. Con varias hojas se pregunta si todas o solo la que se mira.
// **Los marcos del lienzo son sus páginas** (21-sep-2026). El lienzo entero trae sus marcos
// en `g.marcos`; al imprimir se pregunta cuáles —por número— y cada uno sale en su hoja,
// encuadrado, en vez de todo el lienzo encogido en una.
function marcosDe(d){ return d?[].slice.call(d.querySelectorAll('svg g.marcos .marco')):[]; }
function quitarImpresion(){ var v=id('impresion'); if(v) v.remove(); raiz.classList.remove('imprimir-marcos'); }
function imprimirMarcos(d, cuales, entero){
  quitarImpresion();
  var svg=d.querySelector('svg'); if(!svg) return;
  var caja=document.createElement('div'); caja.id='impresion';
  function pagina(vb){
    var s=svg.cloneNode(true), g=s.querySelector('g.marcos');
    if(g) g.remove();
    s.removeAttribute('width'); s.removeAttribute('height'); s.removeAttribute('style');
    s.setAttribute('viewBox',vb); s.setAttribute('preserveAspectRatio','xMidYMid meet');
    var p=document.createElement('div'); p.className='pagina'; p.appendChild(s); caja.appendChild(p);
  }
  if(entero){ var b=svg.getBBox(); pagina([b.x,b.y,b.width,b.height].join(' ')); }
  cuales.forEach(function(m){
    pagina([m.getAttribute('x'),m.getAttribute('y'),m.getAttribute('width'),m.getAttribute('height')].join(' '));
  });
  if(!caja.children.length) return;
  document.body.appendChild(caja);
  raiz.classList.add('imprimir-marcos');
  setTimeout(function(){ window.print(); },80);
}
function elegirMarcos(d, ms){
  var v=document.createElement('div'); v.id='elegir-marcos';
  var caja=document.createElement('div'); caja.className='caja';
  var t=document.createElement('b'); t.textContent='Qué imprimir'; caja.appendChild(t);
  function fila(texto, puesta, clave){
    var l=document.createElement('label'), c=document.createElement('input');
    c.type='checkbox'; c.checked=puesta; c.dataset.k=clave;
    l.appendChild(c); l.appendChild(document.createTextNode(texto)); caja.appendChild(l);
  }
  fila('Lienzo completo', false, 'entero');
  ms.forEach(function(m,i){
    var n=m.getAttribute('data-nombre');
    fila('Página '+(i+1)+(n?' · '+n:''), true, String(i));
  });
  var f=document.createElement('div'); f.className='fila';
  f.innerHTML='<button data-a="no">Cancelar</button><button data-a="si">Imprimir</button>';
  caja.appendChild(f); v.appendChild(caja); document.body.appendChild(v);
  v.addEventListener('click',function(e){
    var a=e.target&&e.target.dataset?e.target.dataset.a:null;
    if(e.target===v||a==='no'){ v.remove(); return; }
    if(a!=='si') return;
    var entero=false, cuales=[];
    [].forEach.call(caja.querySelectorAll('input'),function(c){
      if(!c.checked) return;
      if(c.dataset.k==='entero') entero=true; else cuales.push(ms[+c.dataset.k]);
    });
    v.remove();
    if(entero||cuales.length) imprimirMarcos(d, cuales, entero);
  });
}
function imprimir(){
  quitarImpresion();
  var losMarcos=(actual&&actual.tipo==='dibujo')?marcosDe(hojas[iHoja]):[];
  if(losMarcos.length){ elegirMarcos(hojas[iHoja], losMarcos); return; }
  hojas.forEach(function(d){d.classList.remove('a-imprimir');});
  raiz.classList.remove('imprimir-una');
  if(pagina.length>1&&!confirm('¿Imprimir todas las hojas?\n\nAceptar: todas · Cancelar: solo la que estás viendo')){
    raiz.classList.add('imprimir-una'); hojas[iHoja].classList.add('a-imprimir');
  }
  if(actual&&actual.encajar) actual.encajar();
  setTimeout(function(){ window.print(); },60);
}
if(id('imprimir')) id('imprimir').onclick=imprimir;

// ---- Marcar una zona para imprimir (16-sep-2026) ----
// Pedido por el usuario: «no me deja imprimir porque no hay marcos; añade marcos temporales
// para imprimir la zona seleccionada». Un marco de verdad es parte del dibujo y hay que ir a la
// aplicación a ponerlo; esto es **un rectángulo de usar y tirar**: se arrastra sobre lo que se
// quiere en el papel y se imprime solo eso. No se guarda ni sale en el dibujo.
var marcando=false, marcaCaja=null, marcaDesde=null;
function pintarMarca(a,b){
  if(!marcaCaja){ marcaCaja=document.createElement('div'); marcaCaja.id='zona-marca'; document.body.appendChild(marcaCaja); }
  var x=Math.min(a.x,b.x), y=Math.min(a.y,b.y);
  marcaCaja.style.left=x+'px'; marcaCaja.style.top=y+'px';
  marcaCaja.style.width=Math.abs(b.x-a.x)+'px'; marcaCaja.style.height=Math.abs(b.y-a.y)+'px';
}
function quitarMarca(){ if(marcaCaja){ marcaCaja.remove(); marcaCaja=null; } }
function dejarDeMarcar(){ marcando=false; marcaDesde=null; raiz.classList.remove('marcando'); quitarMarca(); }
function marcarZona(){
  if(!actual||actual.tipo!=='dibujo'||!actual.vista){ estado.textContent='Esto solo vale en una hoja de dibujo'; return; }
  if(marcando){ dejarDeMarcar(); estado.textContent=''; return; }
  marcando=true; raiz.classList.add('marcando');
  estado.textContent='Arrastra sobre lo que quieras imprimir';
}
if(id('marcar-zona')) id('marcar-zona').onclick=marcarZona;
// En captura y antes que el visor: marcando, el dedo marca y no dibuja ni mueve.
cajaLienzo.addEventListener('pointerdown',function(e){
  if(!marcando) return;
  marcaDesde={x:e.clientX,y:e.clientY};
  pintarMarca(marcaDesde,marcaDesde);
  e.stopPropagation(); e.preventDefault();
},true);
cajaLienzo.addEventListener('pointermove',function(e){
  if(!marcando||!marcaDesde) return;
  pintarMarca(marcaDesde,{x:e.clientX,y:e.clientY});
  e.stopPropagation(); e.preventDefault();
},true);
cajaLienzo.addEventListener('pointerup',function(e){
  if(!marcando||!marcaDesde) return;
  e.stopPropagation(); e.preventDefault();
  var a=marcaDesde, b={x:e.clientX,y:e.clientY};
  dejarDeMarcar();
  if(Math.abs(b.x-a.x)<24||Math.abs(b.y-a.y)<24){ estado.textContent='Zona demasiado pequeña'; return; }
  var p1=actual.deEscena(Math.min(a.x,b.x),Math.min(a.y,b.y));
  var p2=actual.deEscena(Math.max(a.x,b.x),Math.max(a.y,b.y));
  imprimirZona({x:p1.x,y:p1.y,w:p2.x-p1.x,h:p2.y-p1.y});
},true);
// Se imprime **solo esa zona**: se encuadra ahí, se manda a imprimir esta hoja sola y, al
// volver del diálogo, el dibujo se queda como estaba.
function imprimirZona(c){
  var antes=actual.vista(), quien=actual;
  function devolver(){ removeEventListener('afterprint',devolver); quien.ponerVista(antes); }
  addEventListener('afterprint',devolver);
  actual.ponerVista(c);
  hojas.forEach(function(d){d.classList.remove('a-imprimir');});
  raiz.classList.add('imprimir-una'); hojas[iHoja].classList.add('a-imprimir');
  estado.textContent='';
  setTimeout(function(){ window.print(); },80);
}
document.addEventListener('keydown',function(e){ if(e.key==='Escape'&&marcando){ dejarDeMarcar(); estado.textContent=''; } });
addEventListener('afterprint',function(){ quitarImpresion(); raiz.classList.remove('imprimir-una'); if(actual&&actual.medir) actual.medir(); });

// La dirección se lee antes de ir a la primera, que la reescribe.
var alAbrir=location.hash||'';
irA(0);
if(alAbrir.indexOf('#hoja-')===0){ var n0=parseInt(alAbrir.slice(6),10); if(n0>0) irA(n0-1); }
})();
// **Los saltos de tiempo de una transcripción.** Un párrafo que empieza por su minuto es
// un enlace: tocarlo lleva el audio de esa nota a ese punto y lo pone a sonar.
document.addEventListener('click',function(e){
  var a=e.target&&e.target.closest?e.target.closest('.salto'):null;
  if(!a) return;
  e.preventDefault();
  var nota=a.closest('.nota')||document;
  var au=nota.querySelector('audio');
  if(!au) return;
  au.currentTime=(+a.dataset.ms)/1000;
  au.play();
});
