var Calculo=(function(){
"use strict";
var MAX_COLS=702, MAX_FILAS=100000, PROFUNDIDAD=400, DIAS=25569;
var E_DIV0='#¡DIV/0!', E_VALOR='#¡VALOR!', E_REF='#¡REF!', E_NOMBRE='#¿NOMBRE?', E_ND='#N/D', E_NUM='#¡NUM!', E_CIRC='#¡CIRC!';
var VACIO={t:'v'}, HUECO={k:'hueco'}, HONDO={hondo:1};
function N(n,f){return {t:'n',n:n,f:!!f};}
function S(s){return {t:'s',s:s};}
function B(b){return {t:'b',b:b};}
function E(e){return {t:'e',e:e};}
function toInt(x){ if(x!==x)return 0; if(x>=2147483647)return 2147483647; if(x<=-2147483648)return -2147483648; return x<0?Math.ceil(x):Math.floor(x); }
function cmpStr(a,b){return a<b?-1:a>b?1:0;}

// ---- Celdas ----
function letras(col){var n=col+1,s='';while(n>0){var r=(n-1)%26;s=String.fromCharCode(65+r)+s;n=Math.floor((n-1)/26);}return s;}
function columna(l){ if(!l||l.length>3)return -1; var n=0; for(var i=0;i<l.length;i++){var u=l.charCodeAt(i); if(u>=97&&u<=122)u-=32; if(u<65||u>90)return -1; n=n*26+(u-64);} return n-1; }
function nombre(f,c){return letras(c)+(f+1);}
var DIR=/^\$?([A-Za-z]{1,3})\$?([0-9]{1,6})$/;
function leer(s){var m=DIR.exec(String(s).trim()); if(!m)return null; var c=columna(m[1]), f=parseInt(m[2],10)-1; if(c<0||c>=MAX_COLS||f<0||f>=MAX_FILAS)return null; return [f,c];}

// ---- CalculoFormato ----
var MONEDA=/^(S\/\.?|US\$|\$|€|£)\s*/, CIFRAS=/^[0-9][0-9.,]*([eE][+-]?[0-9]+)?$|^[.,][0-9]+([eE][+-]?[0-9]+)?$/;
function contar(s,ch){var n=0;for(var i=0;i<s.length;i++)if(s[i]===ch)n++;return n;}
function numero(texto){
  var t=String(texto).trim(); if(!t)return null;
  var neg=false;
  if(t.length>2&&t[0]==='('&&t[t.length-1]===')'){neg=true;t=t.slice(1,-1).trim();}
  if(t[0]==='-'){neg=!neg;t=t.slice(1).trim();} else if(t[0]==='+') t=t.slice(1).trim();
  t=t.replace(MONEDA,'');
  var pc=false; if(t[t.length-1]==='%'){pc=true;t=t.slice(0,-1).trim();}
  t=t.replace(/[ \u00a0\u202f]/g,'');
  if(t[0]==='-'){neg=!neg;t=t.slice(1);}
  if(!CIFRAS.test(t))return null;
  var e=t.search(/[eE]/), cuerpo=e>=0?t.slice(0,e):t, exp=e>=0?t.slice(e):'';
  var p=contar(cuerpo,'.'), c=contar(cuerpo,',');
  if(p>0&&c>0){
    if(cuerpo.lastIndexOf('.')>cuerpo.lastIndexOf(',')){ cuerpo=cuerpo.replace(/,/g,''); if(p>1)return null; }
    else { cuerpo=cuerpo.replace(/\./g,''); if(c>1)return null; cuerpo=cuerpo.replace(',','.'); }
  } else if(c>0){
    var i=cuerpo.indexOf(',');
    if(c>1) cuerpo=cuerpo.replace(/,/g,'');
    else { var despues=cuerpo.length-i-1; cuerpo=(despues===3&&i>=1&&i<=3&&cuerpo[0]!=='0')?cuerpo.replace(',',''):cuerpo.replace(',','.'); }
  } else if(p>1) cuerpo=cuerpo.replace(/\./g,'');
  if(cuerpo[0]==='.')cuerpo='0'+cuerpo;
  if(cuerpo[cuerpo.length-1]==='.')cuerpo=cuerpo.slice(0,-1);
  if(!cuerpo)return null;
  var v=Number(cuerpo+exp); if(v!==v)return null;
  var r=pc?v/100:v; if(neg)r=-r;
  return isFinite(r)?r:null;
}
var FDMA=/^([0-9]{1,2})\/([0-9]{1,2})\/([0-9]{4})$/, FAMD=/^([0-9]{4})-([0-9]{1,2})-([0-9]{1,2})$/;
function diasDelMes(a,m){ if(m===2) return ((a%4===0&&a%100!==0)||a%400===0)?29:28; return (m===4||m===6||m===9||m===11)?30:31; }
function fecha(texto){
  var t=String(texto).trim(), m=FDMA.exec(t), d,mes,a;
  if(m){d=+m[1];mes=+m[2];a=+m[3];} else { m=FAMD.exec(t); if(!m)return null; d=+m[3];mes=+m[2];a=+m[1]; }
  if(mes<1||mes>12||d<1||d>diasDelMes(a,mes))return null;
  return serial(a,mes,d);
}
function serial(a,m,d){ var x=new Date(0); x.setUTCFullYear(a,m-1,d); return Math.round(x.getTime()/86400000)+DIAS; }
function partes(s){ var x=new Date((Math.floor(s)-DIAS)*86400000); return [x.getUTCFullYear(),x.getUTCMonth()+1,x.getUTCDate()]; }
function dos(n){return n<10?'0'+n:''+n;}
function textoDeFecha(s){
  if(!isFinite(s)||s< -657434||s>2958465)return E_NUM;
  var p=partes(s), base=dos(p[2])+'/'+dos(p[1])+'/'+p[0], min=Math.round((s-Math.floor(s))*1440);
  if(min<=0||min>=1440)return base;
  return base+' '+dos(Math.floor(min/60))+':'+dos(min%60);
}
function ceros(n){var s='';while(n-->0)s+='0';return s;}
// PixPin PC: el separador decimal del usuario (`data-decimal` en el body,
// `,` en es-ES) al ENSENAR un numero calculado, como lo escrito a mano. Lo que
// se guarda y se vuelve a leer no cambia. Sin DOM (node), el punto de siempre.
var DECIMAL=null;
function separadorDecimal(){
  if(DECIMAL===null){
    var b=typeof document!=='undefined'&&document.body;
    DECIMAL=(b&&b.getAttribute('data-decimal')===',')?',':'.';
  }
  return DECIMAL;
}
function general(d){
  var s=generalConPunto(d);
  return separadorDecimal()===','?s.replace('.',','):s;
}
function generalConPunto(d){
  if(!isFinite(d))return E_NUM;
  if(d===0)return '0';
  var a=Math.abs(d), s=d<0?'-':'';
  if(a<1e15&&a===Math.floor(a))return s+String(a);
  var x,m,exp;
  if(a>=1e15||a<1e-9){
    x=a.toExponential(5).split('e'); m=x[0]; exp=+x[1];
    if(m.indexOf('.')>=0) m=m.replace(/0+$/,'').replace(/\.$/,'');
    var e=Math.abs(exp);
    return s+m+'E'+(exp<0?'-':'+')+(e<10?'0'+e:''+e);
  }
  x=a.toExponential(9).split('e'); var dig=x[0].replace('.',''), r; exp=+x[1];
  if(exp>=0){ r=dig.length<=exp+1?dig+ceros(exp+1-dig.length):dig.slice(0,exp+1)+'.'+dig.slice(exp+1); }
  else r='0.'+ceros(-exp-1)+dig;
  if(r.indexOf('.')>=0) r=r.replace(/0+$/,'').replace(/\.$/,'');
  return s+r;
}
function redondear(x,dec,modo){
  if(!isFinite(x))return x;
  var n=Math.max(-15,Math.min(15,dec)), f=Math.pow(10,Math.abs(n));
  var esc=n>=0?Math.abs(x)*f:Math.abs(x)/f;
  var limpio=esc===0?0:parseFloat(esc.toPrecision(15));
  var ent=modo===1?Math.ceil(limpio):modo===-1?Math.floor(limpio):Math.floor(limpio+0.5);
  var r=n>=0?ent/f:ent*f;
  return x<0?-r:r;
}
function conFormato(v,formato){
  var sin=formato.replace(/"[^"]*"/g,'').toLowerCase(), i;
  if(sin.indexOf('dd')>=0||sin.indexOf('yy')>=0||sin.indexOf('aa')>=0){
    var p=partes(v), s=formato;
    s=s.replace(/yyyy|aaaa/gi,String(p[0])); s=s.replace(/yy|aa/gi,dos(p[0]%100));
    s=s.replace(/mm/gi,dos(p[1])); s=s.replace(/dd/gi,dos(p[2]));
    return s;
  }
  var primero=-1; for(i=0;i<formato.length;i++){ if(formato[i]==='0'||formato[i]==='#'){primero=i;break;} }
  if(primero<0) return formato.replace(/"/g,'');
  var ultimo=-1; for(i=formato.length-1;i>=0;i--){ if('0#%.,'.indexOf(formato[i])>=0){ultimo=i;break;} }
  var delante=formato.slice(0,primero).replace(/"/g,''), mascara=formato.slice(primero,ultimo+1), detras=formato.slice(ultimo+1).replace(/"/g,'');
  var pc=mascara.indexOf('%')>=0, punto=mascara.indexOf('.'), dec=0;
  if(punto>=0){ var rr=mascara.slice(punto+1); for(i=0;i<rr.length;i++) if(rr[i]==='0'||rr[i]==='#')dec++; }
  var miles=(punto<0?mascara:mascara.slice(0,punto)).indexOf(',')>=0;
  var x=redondear(pc?v*100:v,dec,0), cifras=Math.abs(x).toFixed(dec);
  if(miles){
    var q=cifras.indexOf('.'), ent=q<0?cifras:cifras.slice(0,q), resto=q<0?'':cifras.slice(q), sb='';
    for(i=0;i<ent.length;i++){ if(i>0&&(ent.length-i)%3===0)sb+=','; sb+=ent[i]; }
    cifras=sb+resto;
  }
  return delante+(x<0?'-':'')+cifras+(pc?'%':'')+detras;
}

// ---- CalculoLexico ----
var P_NUM=1,P_TXT=2,P_REF=3,P_COLS=4,P_NOM=5,P_OP=6,P_ABRE=7,P_CIERRA=8,P_SEP=9,P_DP=10,P_ERR=11,P_MAL=12;
var ERRORES=[['#¡DIV/0!','#¡DIV/0!'],['#DIV/0!','#¡DIV/0!'],['#¡VALOR!','#¡VALOR!'],['#VALUE!','#¡VALOR!'],
  ['#¡REF!','#¡REF!'],['#REF!','#¡REF!'],['#¿NOMBRE?','#¿NOMBRE?'],['#NAME?','#¿NOMBRE?'],['#N/D','#N/D'],['#N/A','#N/D'],
  ['#¡NUM!','#¡NUM!'],['#NUM!','#¡NUM!'],['#¡NULO!','#¡NULO!'],['#NULL!','#¡NULO!'],['#¡CIRC!','#¡CIRC!']];
var TILDES='ÁÉÍÓÚÜÑáéíóúüñ';
function esLetra(c){return c!==undefined&&((c>='A'&&c<='Z')||(c>='a'&&c<='z')||c==='_'||(c.length===1&&TILDES.indexOf(c)>=0));}
function esCifra(c){return c!==undefined&&c>='0'&&c<='9';}
function esAZ(c){return c!==undefined&&((c>='A'&&c<='Z')||(c>='a'&&c<='z'));}
var NUMERO_LITERAL=/^([0-9]+\.?[0-9]*|\.[0-9]+)([eE][+-]?[0-9]+)?$/;
function pz(t,x,d,h,o){ var p={t:t,x:x,d:d,h:h,n:0,f:0,c:0,ff:false,cf:false,c2:0,c2f:false}; if(o)for(var k in o)p[k]=o[k]; return p; }
function piezas(f){
  var out=[],i=0,n=f.length;
  while(i<n){
    var c=f[i];
    if(c===' '||c==='\t'||c==='\n'||c==='\r'||c==='\u00a0'){i++;continue;}
    var d=i;
    if(c==='"'){
      var sb='',cerrada=false; i++;
      while(i<n){ if(f[i]==='"'){ if(i+1<n&&f[i+1]==='"'){sb+='"';i+=2;continue;} i++;cerrada=true;break; } sb+=f[i];i++; }
      out.push(pz(cerrada?P_TXT:P_MAL,sb,d,i));
    } else if(esCifra(c)||(c==='.'&&i+1<n&&esCifra(f[i+1]))){
      while(i<n&&(esCifra(f[i])||f[i]==='.'))i++;
      if(i<n&&(f[i]==='e'||f[i]==='E')){ var j=i+1; if(j<n&&(f[j]==='+'||f[j]==='-'))j++; if(j<n&&esCifra(f[j])){ i=j; while(i<n&&esCifra(f[i]))i++; } }
      var t=f.slice(d,i);
      out.push(NUMERO_LITERAL.test(t)?pz(P_NUM,t,d,i,{n:Number(t)}):pz(P_MAL,t,d,i));
    } else if(c==='#'){
      var resto=f.slice(i), hall=null;
      for(var k=0;k<ERRORES.length;k++){ if(resto.slice(0,ERRORES[k][0].length).toUpperCase()===ERRORES[k][0].toUpperCase()){hall=ERRORES[k];break;} }
      if(!hall){i++;out.push(pz(P_MAL,'#',d,i));} else {i+=hall[0].length;out.push(pz(P_ERR,hall[1],d,i));}
    } else if(c==='$'||esLetra(c)){ var pp=palabra(f,i); out.push(pp); i=pp.h; }
    else if(c==='('){i++;out.push(pz(P_ABRE,'(',d,i));}
    else if(c===')'){i++;out.push(pz(P_CIERRA,')',d,i));}
    else if(c===','||c===';'){i++;out.push(pz(P_SEP,c,d,i));}
    else if(c===':'){i++;out.push(pz(P_DP,':',d,i));}
    else if(c==='<'&&i+1<n&&(f[i+1]==='='||f[i+1]==='>')){i+=2;out.push(pz(P_OP,f.slice(d,i),d,i));}
    else if(c==='>'&&i+1<n&&f[i+1]==='='){i+=2;out.push(pz(P_OP,'>=',d,i));}
    else if('+-*/^&=<>%'.indexOf(c)>=0){i++;out.push(pz(P_OP,c,d,i));}
    else {i++;out.push(pz(P_MAL,c,d,i));}
  }
  return out;
}
function palabra(f,d){
  var n=f.length, j=d, cf=j<n&&f[j]==='$'; if(cf)j++;
  var l0=j; while(j<n&&esAZ(f[j]))j++;
  var le=f.slice(l0,j);
  if(le.length>0&&le.length<=3){
    var k=j, ff=k<n&&f[k]==='$'; if(ff)k++;
    var c0=k; while(k<n&&esCifra(f[k]))k++;
    var ci=f.slice(c0,k), sigue=k<n?f[k]:' ';
    if(ci.length>0&&!esLetra(sigue)&&sigue!=='('&&sigue!=='.'&&sigue!=='_'){
      var col=columna(le), fila=ci.length<=9?parseInt(ci,10)-1:-1;
      if(col>=0&&col<MAX_COLS&&fila>=0&&fila<MAX_FILAS) return pz(P_REF,f.slice(d,k),d,k,{f:fila,c:col,ff:ff,cf:cf});
    }
    if(ci.length===0&&!ff&&k<n&&f[k]===':'){
      var m=k+1, c2f=m<n&&f[m]==='$'; if(c2f)m++;
      var l2=m; while(m<n&&esAZ(f[m]))m++;
      var le2=f.slice(l2,m), s2=m<n?f[m]:' ';
      if(le2.length>0&&le2.length<=3&&!esCifra(s2)&&!esLetra(s2)&&s2!=='('){
        var a1=columna(le), a2=columna(le2);
        if(a1>=0&&a1<MAX_COLS&&a2>=0&&a2<MAX_COLS) return pz(P_COLS,f.slice(d,m),d,m,{c:a1,cf:cf,c2:a2,c2f:c2f});
      }
    }
  }
  if(f[d]==='$') return pz(P_MAL,'$',d,d+1);
  var i=d; while(i<n&&(esLetra(f[i])||esCifra(f[i])||f[i]==='.'))i++;
  return pz(P_NOM,f.slice(d,i),d,i);
}
function Mal(){}
function arbol(formula){
  var p=piezas(formula); if(!p.length) return {k:'err',e:E_NOMBRE};
  var i=0;
  function esOp(ops){ var x=p[i]; return (x&&x.t===P_OP&&ops.indexOf(x.x)>=0)?x.x:null; }
  function comparacion(){ var a=concatenar(),op; while((op=esOp(['=','<>','<','>','<=','>=']))){i++;a={k:'op',op:op,a:a,b:concatenar()};} return a; }
  function concatenar(){ var a=suma(); while(esOp(['&'])){i++;a={k:'op',op:'&',a:a,b:suma()};} return a; }
  function suma(){ var a=producto(),op; while((op=esOp(['+','-']))){i++;a={k:'op',op:op,a:a,b:producto()};} return a; }
  function producto(){ var a=potencia(),op; while((op=esOp(['*','/']))){i++;a={k:'op',op:op,a:a,b:potencia()};} return a; }
  function potencia(){ var a=signo(); while(esOp(['^'])){i++;a={k:'op',op:'^',a:a,b:signo()};} return a; }
  function signo(){ var op=esOp(['+','-']); if(op){i++;var x=signo();return op==='-'?{k:'menos',x:x}:x;} return porCiento(); }
  function porCiento(){ var a=primario(); while(esOp(['%'])){i++;a={k:'pc',x:a};} return a; }
  function primario(){
    var x=p[i]; if(!x) throw new Mal();
    switch(x.t){
      case P_NUM: i++; return {k:'num',n:x.n};
      case P_TXT: i++; return {k:'txt',s:x.x};
      case P_ERR: i++; return {k:'err',e:x.x};
      case P_COLS: i++; return {k:'rango',f1:0,c1:Math.min(x.c,x.c2),f2:MAX_FILAS-1,c2:Math.max(x.c,x.c2),col:true};
      case P_REF:
        i++;
        var s=p[i];
        if(s&&s.t===P_DP){ var o=p[i+1]; if(o&&o.t===P_REF){ i+=2; return {k:'rango',f1:Math.min(x.f,o.f),c1:Math.min(x.c,o.c),f2:Math.max(x.f,o.f),c2:Math.max(x.c,o.c),col:false}; } throw new Mal(); }
        return {k:'ref',f:x.f,c:x.c};
      case P_NOM:
        i++;
        var nom=x.x.toUpperCase(), sg=p[i];
        if(sg&&sg.t===P_ABRE){
          i++; var args=[];
          if(p[i]&&p[i].t===P_CIERRA){i++;return {k:'fn',nombre:nom,args:args};}
          while(true){
            var a=p[i]; if(!a) throw new Mal();
            if(a.t===P_SEP||a.t===P_CIERRA) args.push(HUECO); else args.push(comparacion());
            var q=p[i]; if(!q) throw new Mal();
            if(q.t===P_SEP){i++;continue;}
            if(q.t===P_CIERRA){i++;break;}
            throw new Mal();
          }
          return {k:'fn',nombre:nom,args:args};
        }
        if(nom==='VERDADERO'||nom==='TRUE') return {k:'log',b:true};
        if(nom==='FALSO'||nom==='FALSE') return {k:'log',b:false};
        return {k:'err',e:E_NOMBRE};
      case P_ABRE:
        i++; var dentro=comparacion(); if(!p[i]||p[i].t!==P_CIERRA) throw new Mal(); i++; return dentro;
      default: throw new Mal();
    }
  }
  try{ var r=comparacion(); return i!==p.length?{k:'err',e:E_NOMBRE}:r; }
  catch(e){ if(e instanceof Mal) return {k:'err',e:E_NOMBRE}; throw e; }
}
function dir(f,c,ff,cf){return (cf?'$':'')+letras(c)+(ff?'$':'')+(f+1);}
function desplazar(formula,df,dc){
  if(df===0&&dc===0)return formula;
  if(formula[0]!=='=')return formula;
  var cuerpo=formula.slice(1), sb='=', ult=0, ps=piezas(cuerpo);
  for(var i=0;i<ps.length;i++){
    var x=ps[i], nuevo=null;
    if(x.t===P_REF){ var f=x.ff?x.f:x.f+df, c=x.cf?x.c:x.c+dc; nuevo=(f<0||f>=MAX_FILAS||c<0||c>=MAX_COLS)?E_REF:dir(f,c,x.ff,x.cf); }
    else if(x.t===P_COLS){ var c1=x.cf?x.c:x.c+dc, c2=x.c2f?x.c2:x.c2+dc; nuevo=(c1<0||c1>=MAX_COLS||c2<0||c2>=MAX_COLS)?E_REF:(x.cf?'$':'')+letras(c1)+':'+(x.c2f?'$':'')+letras(c2); }
    if(nuevo===null)continue;
    sb+=cuerpo.slice(ult,x.d)+nuevo; ult=x.h;
  }
  return sb+cuerpo.slice(ult);
}

// ---- PortapapelesDeTabla ----
var R1C1=/^R(\[-?[0-9]+\]|[0-9]+)?C(\[-?[0-9]+\]|[0-9]+)?/;
function letraODigito(c){ return c!==undefined&&/[0-9A-Za-zÀ-ɏ]/.test(c); }
function parte(t,desde){ if(!t)return [desde,false]; if(t[0]==='['){ var v=parseInt(t.slice(1,-1),10); return v!==v?null:[desde+v,false]; } var a=parseInt(t,10); return a!==a?null:[a-1,true]; }
function r1c1(formula,fila,col){
  var sb='',i=0,n=formula.length, up=formula.toUpperCase();
  while(i<n){
    var c=formula[i];
    if(c==='"'){
      var j=formula.indexOf('"',i+1); if(j<0)j=n-1;
      var fin=j; while(fin+1<n&&formula[fin+1]==='"'){ var otra=formula.indexOf('"',fin+2); fin=otra<0?n-1:otra; }
      sb+=formula.slice(i,fin+1); i=fin+1; continue;
    }
    var antes=i>0?formula[i-1]:' ';
    if((c==='R'||c==='r')&&!letraODigito(antes)&&antes!=='.'&&antes!=='_'){
      var m=R1C1.exec(up.slice(i));
      if(m){
        var ult=i+m[0].length, des=ult<n?formula[ult]:' ';
        if(!letraODigito(des)&&des!=='('&&des!=='.'&&des!=='_'){
          var f=parte(m[1],fila), k=parte(m[2],col);
          if(!f||!k||f[0]<0||f[0]>=MAX_FILAS||k[0]<0||k[0]>=MAX_COLS) sb+=E_REF;
          else sb+=(k[1]?'$':'')+letras(k[0])+(f[1]?'$':'')+(f[0]+1);
          i=ult; continue;
        }
      }
    }
    sb+=c; i++;
  }
  return sb;
}
function aR1c1(formula,fila,col){
  if(formula[0]!=='=')return formula;
  var cuerpo=formula.slice(1), sb='=', ult=0, ps=piezas(cuerpo);
  for(var i=0;i<ps.length;i++){
    var x=ps[i], nuevo=null;
    if(x.t===P_REF) nuevo=(x.ff?'R'+(x.f+1):'R['+(x.f-fila)+']')+(x.cf?'C'+(x.c+1):'C['+(x.c-col)+']');
    else if(x.t===P_COLS) nuevo=(x.cf?'C'+(x.c+1):'C['+(x.c-col)+']')+':'+(x.c2f?'C'+(x.c2+1):'C['+(x.c2-col)+']');
    if(nuevo===null)continue;
    sb+=cuerpo.slice(ult,x.d)+nuevo; ult=x.h;
  }
  return sb+cuerpo.slice(ult);
}
function deTsv(texto){
  var filas=[], fila=[], celda='', i=0, n=texto.length, alEmpezar=true;
  while(i<n){
    var c=texto[i];
    if(alEmpezar&&c==='"'){
      var j=i+1, sb='', cerrada=-1;
      while(j<n){ if(texto[j]==='"'){ if(j+1<n&&texto[j+1]==='"'){sb+='"';j+=2;continue;} cerrada=j;break; } sb+=texto[j];j++; }
      var tras=(cerrada>=0&&cerrada+1<n)?texto[cerrada+1]:(cerrada>=0?'\n':'x');
      if(cerrada>=0&&(tras==='\t'||tras==='\n'||tras==='\r')){ celda+=sb; i=cerrada+1; alEmpezar=false; continue; }
    }
    alEmpezar=false;
    if(c==='\t'){ fila.push(celda); celda=''; alEmpezar=true; }
    else if(c==='\r'||c==='\n'){ if(c==='\r'&&i+1<n&&texto[i+1]==='\n')i++; fila.push(celda); celda=''; filas.push(fila); fila=[]; alEmpezar=true; }
    else celda+=c;
    i++;
  }
  if(celda.length>0||fila.length>0){ fila.push(celda); filas.push(fila); }
  return filas;
}
function aTsv(filas){
  return filas.map(function(fila){ return fila.map(function(v){
    return (/[\t\n\r]/.test(v)||v[0]==='"')?'"'+v.replace(/"/g,'""')+'"':v;
  }).join('\t'); }).join('\n');
}
function referencias(formula){
  if(formula[0]!=='=')return [];
  var p=piezas(formula.slice(1)), out=[], k=0;
  while(k<p.length){
    var x=p[k];
    if(x.t===P_REF&&k+2<p.length&&p[k+1].t===P_DP&&p[k+2].t===P_REF){
      var y=p[k+2];
      out.push({f1:Math.min(x.f,y.f),c1:Math.min(x.c,y.c),f2:Math.max(x.f,y.f),c2:Math.max(x.c,y.c),desde:x.d+1,hasta:y.h+1});
      k+=3; continue;
    }
    if(x.t===P_REF)out.push({f1:x.f,c1:x.c,f2:x.f,c2:x.c,desde:x.d+1,hasta:x.h+1});
    else if(x.t===P_COLS)out.push({f1:0,c1:Math.min(x.c,x.c2),f2:MAX_FILAS-1,c2:Math.max(x.c,x.c2),desde:x.d+1,hasta:x.h+1});
    k++;
  }
  return out;
}

// ---- Calculadora ----
function esFormula(raw){return raw.length>1&&raw[0]==='=';}
function literal(raw){
  if(raw[0]==="'")return S(raw.slice(1));
  var x=numero(raw); if(x!==null)return N(x);
  x=fecha(raw); if(x!==null)return N(x,true);
  var u=raw.trim().toUpperCase();
  if(u==='VERDADERO'||u==='TRUE')return B(true);
  if(u==='FALSO'||u==='FALSE')return B(false);
  return S(raw);
}
function mostrar(v){
  switch(v.t){ case 'n': return v.f?textoDeFecha(v.n):general(v.n); case 's': return v.s; case 'b': return v.b?'VERDADERO':'FALSO'; case 'e': return v.e; default: return ''; }
}
function tipoDe(v){ return (v.t==='n'||v.t==='v')?0:v.t==='s'?1:v.t==='b'?2:3; }
function vacioComo(o){ return o.t==='s'?S(''):o.t==='b'?B(false):N(0); }
function numCmp(a,b){ if(a===b)return 0; if(Math.abs(a-b)<=1e-12*Math.max(Math.abs(a),Math.abs(b)))return 0; return a<b?-1:1; }
function comparar(a,b){
  var x=a.t==='v'?vacioComo(b):a, y=b.t==='v'?vacioComo(a):b, ra=tipoDe(x), rb=tipoDe(y);
  if(ra!==rb)return ra<rb?-1:1;
  if(x.t==='n')return numCmp(x.n,y.n);
  if(x.t==='s')return cmpStr(x.s.toLowerCase(),y.s.toLowerCase());
  if(x.t==='b')return x.b===y.b?0:!x.b?-1:1;
  return 0;
}
function comodin(p,s){
  var pi=0, si=0, estrella=-1, desde=0;
  while(si<s.length){
    if(pi<p.length){
      var ch=p[pi];
      if(ch==='*'){estrella=pi;desde=si;pi++;continue;}
      if(ch==='~'&&pi+1<p.length){ if(p[pi+1]===s[si]){pi+=2;si++;continue;} }
      else if(ch==='?'||ch===s[si]){pi++;si++;continue;}
    }
    if(estrella>=0){pi=estrella+1;desde++;si=desde;continue;}
    return false;
  }
  while(pi<p.length&&p[pi]==='*')pi++;
  return pi===p.length;
}
function cumple(v,crit){
  var vacia=v.t==='v'||(v.t==='s'&&v.s==='');
  switch(crit.t){
    case 'n': var nn=v.t==='n'?v.n:v.t==='s'?numero(v.s):null; if(nn===null)return false; return numCmp(nn,crit.n)===0;
    case 'b': return v.t==='b'&&v.b===crit.b;
    case 'e': return v.t==='e'&&v.e===crit.e;
    case 'v': return vacia;
  }
  var s=crit.s, ops=['>=','<=','<>','>','<','='], op='';
  for(var i=0;i<ops.length;i++) if(s.slice(0,ops[i].length)===ops[i]){op=ops[i];break;}
  var resto=s.slice(op.length);
  if(resto==='') return op==='<>'?!vacia:(op===''||op==='=')?vacia:false;
  var x=numero(resto); if(x===null)x=fecha(resto);
  if(x!==null){
    var vn=v.t==='n'?v.n:(v.t==='s'&&(op===''||op==='='))?numero(v.s):null;
    if(vn===null) return op==='<>';
    var k=numCmp(vn,x);
    return (op===''||op==='=')?k===0:op==='<>'?k!==0:op==='>'?k>0:op==='<'?k<0:op==='>='?k>=0:k<=0;
  }
  var u=resto.toUpperCase(), lg=(u==='VERDADERO'||u==='TRUE')?true:(u==='FALSO'||u==='FALSE')?false:null;
  if(lg!==null&&(op===''||op==='='||op==='<>')){ var ig=v.t==='b'&&v.b===lg; return op==='<>'?!ig:ig; }
  if(op===''||op==='='||op==='<>'){ var ig2=v.t==='s'&&comodin(resto.toLowerCase(),v.s.toLowerCase()); return op==='<>'?!ig2:ig2; }
  if(v.t!=='s')return false;
  var k2=cmpStr(v.s.toLowerCase(),resto.toLowerCase());
  return op==='>'?k2>0:op==='<'?k2<0:op==='>='?k2>=0:k2<=0;
}
var ALIAS={SUMA:'SUM',PROMEDIO:'AVERAGE',CONTAR:'COUNT',CONTARA:'COUNTA','CONTAR.BLANCO':'COUNTBLANK',PRODUCTO:'PRODUCT',
  MEDIANA:'MEDIAN',DESVEST:'STDEV','DESVEST.M':'STDEV','STDEV.S':'STDEV',SI:'IF','SI.CONJUNTO':'IFS',Y:'AND',O:'OR',NO:'NOT',
  'SI.ERROR':'IFERROR','SI.ND':'IFNA',ESERROR:'ISERROR',ESBLANCO:'ISBLANK',ESNUMERO:'ISNUMBER',ESTEXTO:'ISTEXT',
  REDONDEAR:'ROUND','REDONDEAR.MAS':'ROUNDUP','REDONDEAR.MENOS':'ROUNDDOWN',TRUNCAR:'TRUNC',ENTERO:'INT',RAIZ:'SQRT',
  'RAÍZ':'SQRT',POTENCIA:'POWER',RESIDUO:'MOD',SENO:'SIN',GRADOS:'DEGREES',RADIANES:'RADIANS',SIGNO:'SIGN',
  ALEATORIO:'RAND','ALEATORIO.ENTRE':'RANDBETWEEN',HOY:'TODAY',AHORA:'NOW',FECHA:'DATE',DIA:'DAY','DÍA':'DAY',MES:'MONTH',
  'AÑO':'YEAR',CONCATENAR:'CONCATENATE',LARGO:'LEN',MAYUSC:'UPPER',MINUSC:'LOWER',ESPACIOS:'TRIM',IZQUIERDA:'LEFT',
  DERECHA:'RIGHT',EXTRAE:'MID',ENCONTRAR:'FIND',HALLAR:'SEARCH',SUSTITUIR:'SUBSTITUTE',REPETIR:'REPT',TEXTO:'TEXT',
  VALOR:'VALUE',BUSCARV:'VLOOKUP',BUSCARH:'HLOOKUP',INDICE:'INDEX','ÍNDICE':'INDEX',COINCIDIR:'MATCH',BUSCARX:'XLOOKUP',
  'SUMAR.SI':'SUMIF','CONTAR.SI':'COUNTIF','PROMEDIO.SI':'AVERAGEIF','SUMAR.SI.CONJUNTO':'SUMIFS',
  'CONTAR.SI.CONJUNTO':'COUNTIFS','PROMEDIO.SI.CONJUNTO':'AVERAGEIFS',SUMAPRODUCTO:'SUMPRODUCT',ELEGIR:'CHOOSE',
  FILA:'ROW',COLUMNA:'COLUMN'};

function crear(){
  var crudos=new Map(), valores=new Map(), arboles=new Map(), calculando=new Set();
  var medidas=true, nF=0, nC=0;
  var yo={reloj:function(){return new Date();}, azar:Math.random};
  function medir(){ if(medidas)return; var f=0,c=0; crudos.forEach(function(v,k){ f=Math.max(f,Math.floor(k/1024)+1); c=Math.max(c,k%1024+1); }); nF=f;nC=c;medidas=true; }
  function filas(){medir();return nF;}
  function cols(){medir();return nC;}
  function poner(f,c,t){
    var k=f*1024+c;
    if(!t){ if(crudos.delete(k))medidas=false; }
    else { crudos.set(k,t); if(medidas){nF=Math.max(nF,f+1);nC=Math.max(nC,c+1);} }
    valores.clear();
  }
  function crudo(f,c){ var v=crudos.get(f*1024+c); return v===undefined?'':v; }
  function texto(f,c){ var raw=crudos.get(f*1024+c); if(raw===undefined)return ''; if(!esFormula(raw))return raw[0]==="'"?raw.slice(1):raw; return mostrar(valor(f,c)); }
  function alineacion(f,c){ var v=valor(f,c); return v.t==='n'?'d':(v.t==='b'||v.t==='e')?'c':'i'; }
  function valor(f,c){
    try{ return celda(f,c,0); }
    catch(x){ if(x!==HONDO)throw x; calentar(); try{ return celda(f,c,0); }catch(y){ if(y!==HONDO)throw y; return E(E_NUM); } }
  }
  function calentar(){
    var orden=[]; crudos.forEach(function(v,k){ if(esFormula(v))orden.push(k); });
    orden.sort(function(a,b){return a-b;});
    var pasadas=Math.floor(orden.length/PROFUNDIDAD)+2;
    for(var p=0;p<pasadas;p++){
      var falta=false;
      for(var q=0;q<orden.length;q++){
        var k=p%2===0?orden[q]:orden[orden.length-1-q];
        if(valores.has(k))continue;
        try{ celda(Math.floor(k/1024),k%1024,0); }catch(x){ if(x!==HONDO)throw x; falta=true; }
      }
      if(!falta)return;
    }
  }
  function celda(f,c,prof){
    var k=f*1024+c, v=valores.get(k);
    if(v!==undefined)return v;
    var raw=crudos.get(k);
    if(raw===undefined)return VACIO;
    if(!esFormula(raw)){ v=literal(raw); valores.set(k,v); return v; }
    if(calculando.has(k))return E(E_CIRC);
    if(prof>PROFUNDIDAD)throw HONDO;
    calculando.add(k);
    try{
      var a=arboles.get(raw); if(!a){ a=arbol(raw.slice(1)); arboles.set(raw,a); }
      v=evaluar(a,prof+1,f,c);
      if(v.t==='v')v=N(0);
      if(v.t==='n'&&!isFinite(v.n))v=E(E_NUM);
      valores.set(k,v);
      return v;
    } finally { calculando.delete(k); }
  }
  function evaluar(n,prof,f,c){
    switch(n.k){
      case 'num': return N(n.n);
      case 'txt': return S(n.s);
      case 'log': return B(n.b);
      case 'err': return E(n.e);
      case 'hueco': return VACIO;
      case 'ref': return celda(n.f,n.c,prof);
      case 'rango': return (!n.col&&n.f1===n.f2&&n.c1===n.c2)?celda(n.f1,n.c1,prof):E(E_VALOR);
      case 'menos': var x=aNumero(evaluar(n.x,prof,f,c)); return x.t==='n'?N(-x.n):x;
      case 'pc': var y=aNumero(evaluar(n.x,prof,f,c)); return y.t==='n'?N(y.n/100):y;
      case 'op': return operar(n,prof,f,c);
      case 'fn': return llamar(n,prof,f,c);
    }
    return E(E_NOMBRE);
  }
  function operar(n,prof,f,c){
    var a=evaluar(n.a,prof,f,c), b=evaluar(n.b,prof,f,c), op=n.op;
    if(op==='&'){ var x=aTexto(a); if(x.t==='e')return x; var y=aTexto(b); if(y.t==='e')return y; return S(x.s+y.s); }
    if(op==='='||op==='<>'||op==='<'||op==='>'||op==='<='||op==='>='){
      if(a.t==='e')return a; if(b.t==='e')return b;
      var k=comparar(a,b);
      return B(op==='='?k===0:op==='<>'?k!==0:op==='<'?k<0:op==='>'?k>0:op==='<='?k<=0:k>=0);
    }
    var p=aNumero(a); if(p.t==='e')return p;
    var q=aNumero(b); if(q.t==='e')return q;
    var r;
    if(op==='+')return N(p.n+q.n,p.f||q.f);
    if(op==='-')return N(p.n-q.n,p.f&&!q.f);
    if(op==='*')r=p.n*q.n;
    else if(op==='/'){ if(q.n===0)return E(E_DIV0); r=p.n/q.n; }
    else r=Math.pow(p.n,q.n);
    return isFinite(r)?N(r):E(E_NUM);
  }
  function aNumero(v){
    switch(v.t){
      case 'n': case 'e': return v;
      case 'b': return N(v.b?1:0);
      case 'v': return N(0);
      case 's': var x=numero(v.s); if(x!==null)return N(x); x=fecha(v.s); if(x!==null)return N(x,true); return E(E_VALOR);
    }
  }
  function aTexto(v){ return (v.t==='s'||v.t==='e')?v:S(mostrar(v)); }
  function aLogico(v){
    switch(v.t){
      case 'b': case 'e': return v;
      case 'n': return B(v.n!==0);
      case 'v': return B(false);
      case 's': var u=v.s.trim().toUpperCase(); if(u==='VERDADERO'||u==='TRUE')return B(true); if(u==='FALSO'||u==='FALSE')return B(false); return E(E_VALOR);
    }
  }
  function rangoDe(n){
    if(n.k==='rango') return n.col?{k:'rango',f1:n.f1,c1:n.c1,f2:Math.max(n.f1,filas()-1),c2:n.c2,col:true}:n;
    return {k:'rango',f1:n.f,c1:n.c,f2:n.f,c2:n.c,col:false};
  }
  function recorrer(r,prof,solo,visita){
    var f2=solo?Math.min(r.f2,filas()-1):r.f2, c2=solo?Math.min(r.c2,cols()-1):r.c2;
    for(var f=r.f1;f<=f2;f++) for(var c=r.c1;c<=c2;c++){ if(visita(celda(f,c,prof),f-r.f1,c-r.c1)===false)return; }
  }
  function numeros(a,prof,f,c,contando,cada){
    for(var i=0;i<a.length;i++){
      var x=a[i];
      if(x.k==='rango'||x.k==='ref'){
        recorrer(rangoDe(x),prof,true,function(v){ if(v.t==='n')cada(v.n); else if(v.t==='e'&&!contando)throw {fallo:v}; return true; });
      } else if(x.k!=='hueco'){
        var v=evaluar(x,prof,f,c);
        if(v.t==='n')cada(v.n);
        else if(v.t==='b')cada(v.b?1:0);
        else if(v.t==='s'){ var nn=numero(v.s); if(nn===null)nn=fecha(v.s); if(nn!==null)cada(nn); else if(!contando)throw {fallo:E(E_VALOR)}; }
        else if(v.t==='e'){ if(!contando)throw {fallo:v}; }
      }
    }
  }
  function buscarEn(largo,buscado,tipo,en){
    var ultimo=-1;
    for(var i=0;i<largo;i++){
      var v=en(i); if(v.t==='v')continue;
      var mismo=tipoDe(v)===tipoDe(buscado);
      if(tipo===0){
        if(mismo&&comparar(v,buscado)===0)return i;
        if(buscado.t==='s'&&v.t==='s'&&(buscado.s.indexOf('*')>=0||buscado.s.indexOf('?')>=0)&&comodin(buscado.s.toLowerCase(),v.s.toLowerCase()))return i;
        continue;
      }
      if(!mismo)continue;
      var k=comparar(v,buscado);
      if(k===0)return i;
      if(tipo===1){ if(k<0)ultimo=i; else break; } else { if(k>0)ultimo=i; else break; }
    }
    return ultimo;
  }
  function llamar(fn,prof,f,c){
    var nombre=ALIAS[fn.nombre]||fn.nombre, a=fn.args;
    function falla(e){ throw {fallo:e}; }
    function ev(i){ return i<a.length?evaluar(a[i],prof,f,c):VACIO; }
    function pide(mn,mx){ if(a.length<mn||a.length>mx)falla(E(E_VALOR)); }
    function num(i){ var v=aNumero(ev(i)); if(v.t==='n')return v.n; if(v.t==='e')falla(v); falla(E(E_VALOR)); }
    function numO(i,si){ return (i>=a.length||a[i].k==='hueco')?si:num(i); }
    function txt(i){ var v=aTexto(ev(i)); if(v.t==='s')return v.s; if(v.t==='e')falla(v); return ''; }
    function log(i){ var v=aLogico(ev(i)); if(v.t==='b')return v.b; if(v.t==='e')falla(v); return false; }
    function rango(i){ var n=a[i]; if(n&&(n.k==='rango'||n.k==='ref'))return rangoDe(n); falla(E(E_VALOR)); }
    function num1(d){ return isFinite(d)?N(d):E(E_NUM); }
    function mismoTamano(r,base){ return r.f2-r.f1===base.f2-base.f1&&r.c2-r.c1===base.c2-base.c1; }
    try{
      var s,n,m,i,l,r,x,v,t;
      switch(nombre){
        case 'SUM': s=0; numeros(a,prof,f,c,false,function(z){s+=z;}); return N(s);
        case 'AVERAGE': s=0;n=0; numeros(a,prof,f,c,false,function(z){s+=z;n++;}); return n===0?E(E_DIV0):N(s/n);
        case 'MIN': case 'MAX':
          m=nombre==='MIN'?Infinity:-Infinity; n=0;
          numeros(a,prof,f,c,false,function(z){ m=nombre==='MIN'?Math.min(m,z):Math.max(m,z); n++; });
          return N(n===0?0:m);
        case 'COUNT': n=0; numeros(a,prof,f,c,true,function(){n++;}); return N(n);
        case 'COUNTA':
          n=0;
          for(i=0;i<a.length;i++){ x=a[i];
            if(x.k==='rango'||x.k==='ref') recorrer(rangoDe(x),prof,true,function(z){ if(z.t!=='v')n++; return true; });
            else if(x.k!=='hueco'){ if(evaluar(x,prof,f,c).t!=='v')n++; }
          }
          return N(n);
        case 'COUNTBLANK': pide(1,1); n=0; recorrer(rango(0),prof,false,function(z){ if(z.t==='v'||(z.t==='s'&&z.s===''))n++; return true; }); return N(n);
        case 'PRODUCT': s=1;n=0; numeros(a,prof,f,c,false,function(z){s*=z;n++;}); return num1(n===0?0:s);
        case 'MEDIAN':
          l=[]; numeros(a,prof,f,c,false,function(z){l.push(z);});
          if(!l.length)return E(E_NUM);
          l.sort(function(p,q){return p-q;}); m=Math.floor(l.length/2);
          return N(l.length%2===1?l[m]:(l[m-1]+l[m])/2);
        case 'STDEV':
          l=[]; numeros(a,prof,f,c,false,function(z){l.push(z);});
          if(l.length<2)return E(E_DIV0);
          m=0; for(i=0;i<l.length;i++)m+=l[i]; m/=l.length;
          s=0; for(i=0;i<l.length;i++)s+=(l[i]-m)*(l[i]-m);
          return N(Math.sqrt(s/(l.length-1)));
        case 'IF': pide(1,3); return log(0)?(a.length>1?ev(1):B(true)):(a.length>2?ev(2):B(false));
        case 'IFS':
          if(!a.length||a.length%2!==0)falla(E(E_VALOR));
          for(i=0;i<a.length;i+=2){ if(log(i))return ev(i+1); }
          return E(E_ND);
        case 'AND': case 'OR':
          var hay=false, res=nombre==='AND';
          for(i=0;i<a.length;i++){ x=a[i];
            if(x.k==='rango'||x.k==='ref'){
              recorrer(rangoDe(x),prof,true,function(z){
                var bb=z.t==='b'?z.b:z.t==='n'?z.n!==0:null;
                if(z.t==='e')falla(z);
                if(bb!==null){ hay=true; res=nombre==='AND'?(res&&bb):(res||bb); }
                return true;
              });
            } else if(x.k!=='hueco'){
              v=aLogico(evaluar(x,prof,f,c)); if(v.t==='e')falla(v);
              var b2=v.t==='b'?v.b:false; hay=true; res=nombre==='AND'?(res&&b2):(res||b2);
            }
          }
          return hay?B(res):E(E_VALOR);
        case 'NOT': pide(1,1); return B(!log(0));
        case 'IFERROR': pide(2,2); v=ev(0); return v.t==='e'?ev(1):v;
        case 'IFNA': pide(2,2); v=ev(0); return (v.t==='e'&&v.e===E_ND)?ev(1):v;
        case 'ISERROR': pide(1,1); return B(ev(0).t==='e');
        case 'ISBLANK': pide(1,1); return B(ev(0).t==='v');
        case 'ISNUMBER': pide(1,1); return B(ev(0).t==='n');
        case 'ISTEXT': pide(1,1); return B(ev(0).t==='s');
        case 'ROUND': pide(1,2); return N(redondear(num(0),toInt(numO(1,0)),0));
        case 'ROUNDUP': pide(1,2); return N(redondear(num(0),toInt(numO(1,0)),1));
        case 'ROUNDDOWN': case 'TRUNC': pide(1,2); return N(redondear(num(0),toInt(numO(1,0)),-1));
        case 'INT': pide(1,1); return N(Math.floor(num(0)));
        case 'ABS': pide(1,1); return N(Math.abs(num(0)));
        case 'SQRT': pide(1,1); x=num(0); return x<0?E(E_NUM):N(Math.sqrt(x));
        case 'POWER': pide(2,2); return num1(Math.pow(num(0),num(1)));
        case 'MOD': pide(2,2); x=num(0); var y=num(1); return y===0?E(E_DIV0):N(x-y*Math.floor(x/y));
        case 'PI': pide(0,0); return N(Math.PI);
        case 'EXP': pide(1,1); return num1(Math.exp(num(0)));
        case 'LN': pide(1,1); x=num(0); return x<=0?E(E_NUM):N(Math.log(x));
        case 'LOG10': pide(1,1); x=num(0); return x<=0?E(E_NUM):N(Math.log10(x));
        case 'LOG': pide(1,2); x=num(0); var bs=numO(1,10); if(x<=0||bs<=0)return E(E_NUM); if(bs===1)return E(E_DIV0); return N(Math.log(x)/Math.log(bs));
        case 'SIN': pide(1,1); return N(Math.sin(num(0)));
        case 'COS': pide(1,1); return N(Math.cos(num(0)));
        case 'TAN': pide(1,1); return N(Math.tan(num(0)));
        case 'DEGREES': pide(1,1); return N(num(0)*180/Math.PI);
        case 'RADIANS': pide(1,1); return N(num(0)*Math.PI/180);
        case 'SIGN': pide(1,1); x=num(0); return N(x>0?1:x<0?-1:0);
        case 'RAND': pide(0,0); return N(yo.azar());
        case 'RANDBETWEEN': pide(2,2); var lo=Math.ceil(num(0)), hi=Math.floor(num(1)); return hi<lo?E(E_NUM):N(Math.floor(yo.azar()*(hi-lo+1))+lo);
        case 'TODAY': pide(0,0); t=yo.reloj(); return N(serial(t.getFullYear(),t.getMonth()+1,t.getDate()),true);
        case 'NOW': pide(0,0); t=yo.reloj(); return N(serial(t.getFullYear(),t.getMonth()+1,t.getDate())+(t.getHours()*3600+t.getMinutes()*60+t.getSeconds())/86400,true);
        case 'DATE':
          pide(3,3); var an=toInt(num(0)); if(an>=0&&an<=1899)an+=1900;
          if(an<0||an>9999)return E(E_NUM);
          return N(serial(an,toInt(num(1)),toInt(num(2))),true);
        case 'DAY': case 'MONTH': case 'YEAR':
          pide(1,1); var pr=partes(num(0)); return N(pr[nombre==='YEAR'?0:nombre==='MONTH'?1:2]);
        case 'CONCATENATE': case 'CONCAT':
          var sb='';
          for(i=0;i<a.length;i++){ x=a[i];
            if(x.k==='rango'&&!(x.f1===x.f2&&x.c1===x.c2&&!x.col)){
              recorrer(rango(i),prof,true,function(z){ var tt=aTexto(z); if(tt.t==='s')sb+=tt.s; else if(tt.t==='e')falla(tt); return true; });
            } else sb+=txt(i);
          }
          return S(sb);
        case 'LEN': pide(1,1); return N(txt(0).length);
        case 'UPPER': pide(1,1); return S(txt(0).toUpperCase());
        case 'LOWER': pide(1,1); return S(txt(0).toLowerCase());
        case 'TRIM': pide(1,1); return S(txt(0).split(' ').filter(function(z){return z.length>0;}).join(' '));
        case 'LEFT': case 'RIGHT':
          pide(1,2); t=txt(0); n=toInt(numO(1,1));
          if(n<0)return E(E_VALOR);
          return S(nombre==='LEFT'?t.slice(0,n):t.slice(Math.max(0,t.length-n)));
        case 'MID':
          pide(3,3); t=txt(0); var desde=toInt(num(1)); n=toInt(num(2));
          if(desde<1||n<0)return E(E_VALOR);
          return S(desde>t.length?'':t.slice(desde-1,Math.min(t.length,desde-1+n)));
        case 'FIND': case 'SEARCH':
          pide(2,3); var que=txt(0), donde=txt(1), ini=toInt(numO(2,1));
          if(ini<1||ini>donde.length+1)return E(E_VALOR);
          i=nombre==='FIND'?donde.indexOf(que,ini-1):donde.toLowerCase().indexOf(que.toLowerCase(),ini-1);
          return i<0?E(E_VALOR):N(i+1);
        case 'SUBSTITUTE':
          pide(3,4); t=txt(0); var viejo=txt(1), nuevo=txt(2);
          if(!viejo.length)return S(t);
          if(a.length<4)return S(t.split(viejo).join(nuevo));
          var cual=toInt(num(3)); if(cual<1)return E(E_VALOR);
          var pos=-1, visto=0, dd=0;
          while(true){ var jj=t.indexOf(viejo,dd); if(jj<0)break; visto++; if(visto===cual){pos=jj;break;} dd=jj+viejo.length; }
          return S(pos<0?t:t.slice(0,pos)+nuevo+t.slice(pos+viejo.length));
        case 'REPT': pide(2,2); n=toInt(num(1)); return (n<0||n>10000)?E(E_VALOR):S(txt(0).repeat(n));
        case 'TEXT': pide(2,2); v=aNumero(ev(0)); return v.t==='n'?S(conFormato(v.n,txt(1))):S(txt(0));
        case 'VALUE': pide(1,1); v=ev(0); if(v.t==='n')return N(v.n); x=aNumero(S(txt(0))); return x.t==='n'?N(x.n):x;
        case 'VLOOKUP': case 'HLOOKUP':
          pide(3,4); var bu=ev(0); if(bu.t==='e')falla(bu);
          r=rango(1); var col=toInt(num(2)), aprox=(a.length>3&&a[3].k!=='hueco')?log(3):true, vert=nombre==='VLOOKUP';
          var largo=vert?r.f2-r.f1+1:r.c2-r.c1+1, ancho=vert?r.c2-r.c1+1:r.f2-r.f1+1;
          if(col<1)falla(E(E_VALOR)); if(col>ancho)falla(E(E_REF));
          var hasta=Math.min(largo,vert?filas()-r.f1:cols()-r.c1);
          i=buscarEn(hasta,bu,aprox?1:0,function(z){ return vert?celda(r.f1+z,r.c1,prof):celda(r.f1,r.c1+z,prof); });
          if(i<0)return E(E_ND);
          return vert?celda(r.f1+i,r.c1+col-1,prof):celda(r.f1+col-1,r.c1+i,prof);
        case 'MATCH':
          pide(2,3); var bm=ev(0); if(bm.t==='e')falla(bm);
          r=rango(1); var tipo=Math.max(-1,Math.min(1,toInt(numO(2,1)))), vm=r.c1===r.c2;
          if(!vm&&r.f1!==r.f2)falla(E(E_ND));
          var lm=vm?Math.min(r.f2-r.f1+1,filas()-r.f1):Math.min(r.c2-r.c1+1,cols()-r.c1);
          i=buscarEn(lm,bm,tipo,function(z){ return vm?celda(r.f1+z,r.c1,prof):celda(r.f1,r.c1+z,prof); });
          return i<0?E(E_ND):N(i+1);
        case 'XLOOKUP':
          pide(3,4); var bx=ev(0); if(bx.t==='e')falla(bx);
          r=rango(1); var dx=rango(2), vx=r.c1===r.c2;
          var lx=vx?Math.min(r.f2-r.f1+1,filas()-r.f1):Math.min(r.c2-r.c1+1,cols()-r.c1);
          i=buscarEn(lx,bx,0,function(z){ return vx?celda(r.f1+z,r.c1,prof):celda(r.f1,r.c1+z,prof); });
          if(i<0)return a.length>3?ev(3):E(E_ND);
          return vx?celda(dx.f1+i,dx.c1,prof):celda(dx.f1,dx.c1+i,prof);
        case 'INDEX':
          pide(2,3); r=rango(0); var fi=toInt(num(1)), co=toInt(numO(2,1));
          if(a.length===2&&r.f1===r.f2&&r.c1!==r.c2){ co=fi; fi=1; }
          if(fi<1||co<1)return E(E_VALOR);
          if(fi>r.f2-r.f1+1||co>r.c2-r.c1+1)return E(E_REF);
          return celda(r.f1+fi-1,r.c1+co-1,prof);
        case 'SUMIF': case 'AVERAGEIF': case 'COUNTIF':
          pide(2,nombre==='COUNTIF'?2:3); r=rango(0);
          var crit=ev(1); if(crit.t==='e')falla(crit);
          var destino=a.length>2?rango(2):r; s=0;n=0;
          var conVacias=nombre==='COUNTIF'&&cumple(VACIO,crit);
          recorrer(r,prof,!conVacias,function(z,df,dc){
            if(cumple(z,crit)){
              if(nombre==='COUNTIF')n++;
              else { var xx=celda(destino.f1+df,destino.c1+dc,prof); if(xx.t==='n'){s+=xx.n;n++;} }
            }
            return true;
          });
          return nombre==='COUNTIF'?N(n):nombre==='SUMIF'?N(s):(n===0?E(E_DIV0):N(s/n));
        case 'SUMIFS': case 'AVERAGEIFS': case 'COUNTIFS':
          var cuenta=nombre==='COUNTIFS', primero=cuenta?0:1;
          if(a.length<primero+2||(a.length-primero)%2!==0)falla(E(E_VALOR));
          var base=rango(0), pares=[], vacias=cuenta;
          for(i=primero;i<a.length;i+=2){
            var rr=rango(i); if(!mismoTamano(rr,base))falla(E(E_VALOR));
            var cr=ev(i+1); if(cr.t==='e')falla(cr);
            if(!cumple(VACIO,cr))vacias=false;
            pares.push([rr,cr]);
          }
          s=0;n=0;
          recorrer(base,prof,!vacias,function(z,df,dc){
            for(var w=0;w<pares.length;w++){ if(!cumple(celda(pares[w][0].f1+df,pares[w][0].c1+dc,prof),pares[w][1]))return true; }
            if(cuenta)n++; else if(z.t==='n'){s+=z.n;n++;}
            return true;
          });
          return nombre==='COUNTIFS'?N(n):nombre==='SUMIFS'?N(s):(n===0?E(E_DIV0):N(s/n));
        case 'SUMPRODUCT':
          if(!a.length)falla(E(E_VALOR));
          var rs=[]; for(i=0;i<a.length;i++)rs.push(rango(i));
          for(i=0;i<rs.length;i++) if(!mismoTamano(rs[i],rs[0]))falla(E(E_VALOR));
          s=0;
          recorrer(rs[0],prof,true,function(z,df,dc){
            var pp=z.t==='n'?z.n:0;
            for(var w=1;w<rs.length;w++){
              if(pp===0)break;
              var xx=celda(rs[w].f1+df,rs[w].c1+dc,prof);
              if(xx.t==='e')falla(xx);
              pp*=xx.t==='n'?xx.n:0;
            }
            if(z.t==='e')falla(z);
            s+=pp; return true;
          });
          return N(s);
        case 'CHOOSE':
          if(a.length<2)falla(E(E_VALOR));
          i=toInt(num(0)); return (i<1||i>=a.length)?E(E_VALOR):ev(i);
        case 'ROW': case 'COLUMN':
          pide(0,1);
          if(!a.length)return N((nombre==='ROW'?f:c)+1);
          r=rango(0); return N((nombre==='ROW'?r.f1:r.c1)+1);
      }
      return E(E_NOMBRE);
    }catch(z){ if(z&&z.fallo)return z.fallo; throw z; }
  }
  yo.poner=poner; yo.crudo=crudo; yo.texto=texto; yo.valor=valor; yo.alineacion=alineacion;
  yo.filas=filas; yo.cols=cols; yo.olvidar=function(){valores.clear();};
  yo.esFormula=function(f,c){return esFormula(crudo(f,c));};
  yo.cada=function(fn){ crudos.forEach(function(v,k){ fn(Math.floor(k/1024),k%1024,v); }); };
  yo.caja=function(){
    if(!crudos.size)return null;
    var f1=Infinity,c1=Infinity,f2=-1,c2=-1;
    crudos.forEach(function(v,k){ var f=Math.floor(k/1024),c=k%1024; if(f<f1)f1=f; if(c<c1)c1=c; if(f>f2)f2=f; if(c>c2)c2=c; });
    return [f1,c1,f2,c2];
  };
  return yo;
}
return {crear:crear, letras:letras, columna:columna, nombre:nombre, leer:leer, numero:numero, fecha:fecha,
  general:general, redondear:redondear, conFormato:conFormato, textoDeFecha:textoDeFecha, serial:serial,
  piezas:piezas, arbol:arbol, desplazar:desplazar, r1c1:r1c1, aR1c1:aR1c1, deTsv:deTsv, aTsv:aTsv,
  esFormula:esFormula, literal:literal, mostrar:mostrar, cumple:cumple, comodin:comodin, referencias:referencias};
})();
