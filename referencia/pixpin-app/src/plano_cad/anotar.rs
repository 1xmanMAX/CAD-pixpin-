//! **Anotar sobre un plano con el motor del lienzo**: el mismo anfitrion que
//! usan los lectores de Word, libro y PDF (`lector_tinta::Tinta`), con el
//! mismo gesto, la misma barra, el mismo panel y las mismas teclas que el
//! editor, y la tinta optimizada de siempre. Como PixPin Android v0.114
//! («anotar en el visor de planos usa las herramientas del motor del
//! lienzo, sin herramientas propias») y como pidio el usuario el 9-oct-2026:
//! no otra tinta, la que ya hay.
//!
//! El visor de planos (`pixpin_cad::anotado::Anotador`) le pasa sus eventos
//! y le deja pintar con Direct2D sobre cada fotograma, en su mismo
//! dispositivo: la tinta va en la misma pasada que el plano y no se separa
//! de el al mover o acercar.
//!
//! **La capa** es la `anot-<uid>` del plano, como la de un Word anotado: la
//! lee y la guarda [`Capa`], que deja intacto lo que puso el movil, y viaja
//! al sincronizar con su **`.hoja`** al lado (la caja del plano en unidades
//! de la capa, `CapaDelPlano.kt` de Android). En memoria la capa va siempre
//! en la unidad de la letra del plano (`pixpin_cad::anotado::unidad_de_la_capa`,
//! la de Android desde su v0.115: con ella el lapiz no sale desproporcionado,
//! 12-oct-2026); el fichero, en la que diga su `.hoja` (la de Android, la de
//! antes...), y lo que no se toca vuelve a el tal cual.
//!
//! Tambien van en la capa, como en Android, **las cotas y los marcos para
//! imprimir** (figuras del motor de los grupos `plano-cota` y `plano-marco`):
//! asi todo lo hecho sobre el plano viaja, se junta figura por figura y lo
//! borrado viaja como borrado. Un plano que no es de ningun chat (abierto
//! desde el disco) guarda su capa en la cache, solo en este PC.

use std::path::{Path, PathBuf};

use pixpin_cad::anotado::{
    Anotador, Encaje, EventoPlano, GRUPO_COTA, GRUPO_MARCO, Respuesta, VistaPlano,
};
use pixpin_cad::modelo::Modelo;
use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2};
use pixpin_render::{Color, MotorRender};
use pixpin_shell::overlay::EventoOverlay;
use pixpin_sincro::anotado::MarcoDeLaHoja;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::core::Interface;

use crate::lector_tinta::{self, Capa, Parte, Tinta, Unidades};

/// Los ficheros de lo anotado sobre un plano: la capa y su `.hoja`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rutas {
    pub tinta: PathBuf,
    pub hoja: PathBuf,
}

/// Donde vive lo anotado sobre `plano`: si es un adjunto de un chat, en los
/// ficheros de su mensaje (`anot-<uid>.excalidraw.gz` y `anot-<uid>.hoja`,
/// los que viajan); si no, en la cache.
pub fn rutas_de_la_capa(raiz: &Path, plano: &Path) -> Rutas {
    use pixpin_proyecto::anotado as an;
    if let Some(r) = an::raiz_de(plano)
        && let Some(b) = an::base_del_documento(&r, plano)
    {
        return Rutas {
            tinta: b.tinta(),
            hoja: b.marco(),
        };
    }
    let clave = super::clave(plano).unwrap_or_else(|| "plano".into());
    let carpeta = raiz.join("cache").join("cad");
    Rutas {
        tinta: carpeta.join(format!("{clave}.anot.excalidraw")),
        hoja: carpeta.join(format!("{clave}.anot.hoja")),
    }
}

/// **Lo anotado sobre un marcador de un modelo 3D** (19-oct-2026): una
/// capa por marcador, con su `.hoja`. De un adjunto del chat, en los
/// ficheros de su mensaje, que viajan (`anot-<uid>-m<id>.excalidraw.gz` y
/// `anot-<uid>-m<id>.hoja`, ver [`super::SitioDeMarcadores`]); si no, en la
/// carpeta local (`<id>.excalidraw` y `<id>.hoja`).
///
/// La capa cuenta la pantalla desde su centro, en milesimas del alto de la
/// ventana, con la y hacia abajo ([`modelo_de_pantalla`]); su `.hoja` es
/// la de esa caja, `-2500,-2500,2500,2500`.
pub fn rutas_de_marcador(sitio: &super::SitioDeMarcadores, id: &str) -> Rutas {
    match sitio {
        super::SitioDeMarcadores::Adjunto(b) => {
            let b = super::base_de_marcador(b, id);
            Rutas {
                tinta: b.tinta(),
                hoja: b.marco(),
            }
        }
        super::SitioDeMarcadores::Local(carpeta) => {
            let id = super::id_limpio(id);
            Rutas {
                tinta: carpeta.join(format!("{id}.excalidraw")),
                hoja: carpeta.join(format!("{id}.hoja")),
            }
        }
    }
}

/// **Quita lo anotado sobre un marcador** (al quitar el marcador) sin
/// borrar sus ficheros: la sincronizacion no lleva borrados de ficheros, y
/// uno que falta volveria del otro aparato. Cada trazo queda borrado en la
/// capa (y asi se va tambien alli) y la `.hoja` se queda como estaba.
pub fn vaciar_capa(r: &Rutas) -> std::io::Result<()> {
    if !r.tinta.is_file() {
        return Ok(());
    }
    let mut capa = Capa::leer(&r.tinta);
    if capa.vacia() {
        return Ok(());
    }
    capa.vaciar();
    capa.guardar(&r.tinta)
}

/// **El «plano» de la pantalla de un modelo 3D**, para el motor: lo anotado
/// sobre un marcador va en la pantalla contada desde su centro en milesimas
/// del alto de la ventana (`pixpin_cad::ventana3d`), asi que su unidad es 1
/// (sin letras, la unidad es el lado mayor entre 5000) y su caja, de sobra
/// para la pantalla.
pub fn modelo_de_pantalla() -> Modelo {
    Modelo {
        caja: [-2500.0, -2500.0, 2500.0, 2500.0],
        ..Default::default()
    }
}

/// El motor del lienzo sobre lo anotado del marcador `id`.
pub fn motor_de_marcador(sitio: &super::SitioDeMarcadores, id: &str) -> MotorDelPlano {
    MotorDelPlano::nuevo(rutas_de_marcador(sitio, id), &modelo_de_pantalla())
}

/// La `.hoja` escrita (aqui o en el movil), si se entiende.
fn leer_hoja(ruta: &Path) -> Option<[f64; 4]> {
    let m = MarcoDeLaHoja::de_texto(&pixpin_proyecto::anotado::leer(ruta)?)?;
    Some([m.x0 as f64, m.y0 as f64, m.x1 as f64, m.y1 as f64])
}

/// Escribe la `.hoja` solo si no dice ya lo mismo (una fecha nueva es un
/// envio mas al sincronizar).
fn escribir_hoja(ruta: &Path, h: [f64; 4]) -> std::io::Result<()> {
    let m = MarcoDeLaHoja::nuevo(h[0] as f32, h[1] as f32, h[2] as f32, h[3] as f32);
    if let Some(ya) = pixpin_proyecto::anotado::leer(ruta).and_then(|t| MarcoDeLaHoja::de_texto(&t))
        && ya.casi_igual(&m)
    {
        return Ok(());
    }
    if let Some(d) = ruta.parent() {
        std::fs::create_dir_all(d)?;
    }
    pixpin_proyecto::anotado::escribir(ruta, &m.a_texto())
}

/// **Las unidades del fichero respecto de las de la memoria** (ver
/// [`Unidades`]: `fichero = memoria * e + d`), las dos dichas por su encaje
/// en el plano. Casi las mismas (un movil con la misma letra), las mismas.
fn unidades_entre(memoria: &Encaje, fichero: &Encaje) -> Unidades {
    let e = (memoria.u / fichero.u) as f32;
    let u = Unidades {
        ex: e,
        ey: e,
        dx: ((memoria.cero_x - fichero.cero_x) / fichero.u) as f32,
        dy: ((fichero.cero_y - memoria.cero_y) / fichero.u) as f32,
    };
    if u.casi_iguales(&Unidades::DEL_PC) {
        Unidades::DEL_PC
    } else {
        u
    }
}

/// **Lee lo anotado sobre el plano `m`**: la capa en las unidades de la
/// memoria (la letra del plano, con el cero en su origen) y el encaje del
/// fichero. El del fichero es el de su `.hoja`; sin ella (o si no se
/// entiende), el de antes si ya habia capa (`EscalaDeLaCapa.leerOPoner`
/// de Android) o el de la memoria si es nueva.
pub fn leer_capa(r: &Rutas, m: &Modelo) -> (Capa, Encaje, Encaje) {
    let memoria = Encaje::de_unidad(pixpin_cad::anotado::unidad_de_la_capa(m));
    let antigua = Encaje::de_unidad(pixpin_cad::anotado::unidad_antigua(m));
    let fichero = encaje_del_fichero(r, m.caja, &antigua, &memoria);
    (
        Capa::leer_en(&r.tinta, unidades_entre(&memoria, &fichero)),
        memoria,
        fichero,
    )
}

/// El encaje de lo que hay ahora en el disco (ver [`leer_capa`]).
fn encaje_del_fichero(r: &Rutas, caja: [f32; 4], antigua: &Encaje, memoria: &Encaje) -> Encaje {
    leer_hoja(&r.hoja)
        .and_then(|h| Encaje::de_la_hoja(h, caja))
        .unwrap_or_else(|| {
            if r.tinta.is_file() {
                *antigua
            } else {
                *memoria
            }
        })
}

/// **Como esta en el disco lo anotado**: fecha y tamano de la capa y de su
/// `.hoja`. Cambia cuando la sincronizacion trae algo del movil.
type Sello = [Option<(std::time::SystemTime, u64)>; 2];

fn sello_de(r: &Rutas) -> Sello {
    let de = |f: &Path| {
        std::fs::metadata(f)
            .ok()
            .and_then(|m| Some((m.modified().ok()?, m.len())))
    };
    [de(&r.tinta), de(&r.hoja)]
}

/// **Una capa del disco pasada a otras unidades** `u` (`fichero = antes * e
/// + d`), sin tocar nada mas: posicion, caja, puntos (relativos a su x, y),
/// huecos, grosor y letra, como `CapaDelPlano.transformar` de Android. Ni
/// version ni fecha: no es un cambio, es la misma capa contada en otra
/// unidad. `None` si no se entiende.
fn en_otras_unidades(texto: &str, u: Unidades) -> Option<String> {
    use serde_json::Value;
    let mut v: Value = serde_json::from_str(texto).ok()?;
    let (ex, ey, k) = (u.ex as f64, u.ey as f64, u.media() as f64);
    let por = |e: &mut serde_json::Map<String, Value>, clave: &str, f: f64| {
        if let Some(n) = e.get(clave).and_then(Value::as_f64) {
            e.insert(clave.into(), Value::from(n * f));
        }
    };
    let puntos = |l: &mut Value| {
        for p in l.as_array_mut().into_iter().flatten() {
            match p {
                Value::Array(a) if a.len() >= 2 => {
                    let (x, y) = (a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0));
                    a[0] = Value::from(x * ex);
                    a[1] = Value::from(y * ey);
                }
                Value::Object(o) => {
                    let (x, y) = (
                        o.get("x").and_then(Value::as_f64),
                        o.get("y").and_then(Value::as_f64),
                    );
                    if let (Some(x), Some(y)) = (x, y) {
                        o.insert("x".into(), Value::from(x * ex));
                        o.insert("y".into(), Value::from(y * ey));
                    }
                }
                _ => {}
            }
        }
    };
    for e in v.get_mut("elements")?.as_array_mut()?.iter_mut() {
        let Some(e) = e.as_object_mut() else { continue };
        if let (Some(x), Some(y)) = (
            e.get("x").and_then(Value::as_f64),
            e.get("y").and_then(Value::as_f64),
        ) {
            e.insert("x".into(), Value::from(x * ex + u.dx as f64));
            e.insert("y".into(), Value::from(y * ey + u.dy as f64));
        }
        por(e, "width", ex);
        por(e, "height", ey);
        por(e, "strokeWidth", k);
        por(e, "fontSize", k);
        if let Some(l) = e.get_mut("points") {
            puntos(l);
        }
        if let Some(Value::Array(anillos)) = e.get_mut("huecos") {
            anillos.iter_mut().for_each(puntos);
        }
    }
    serde_json::to_string(&v).ok()
}

/// **Junta lo de aqui con lo que hay ahora en el disco**, como junta la
/// sincronizacion (`pixpin_sincro::fusion::json`, la de Android): `base` es
/// la capa tal como se leyo o escribio aqui la ultima vez, `mio` lo que
/// se escribiria ahora y `suyo` lo que dejo la sincronizacion mientras el
/// plano estaba abierto. Lo nuevo de cualquiera entra; lo que uno borro se
/// va si el otro no lo toco. `None` si `mio` o `suyo` no se entienden.
/// La usa tambien el lector de PDF y Word (`lector_pdf::al_dia`).
pub(crate) fn juntar_con_el_disco(base: Option<&str>, mio: &str, suyo: &str) -> Option<String> {
    use pixpin_sincro::canonico::Json;
    use pixpin_sincro::fusion;
    // Los tres por el mismo camino: un mismo numero escrito de dos maneras
    // (`1.0E-7`, `1e-7`) seria un cambio que no hizo nadie.
    let json = |t: &str| {
        serde_json::from_str::<serde_json::Value>(t)
            .ok()
            .map(|v| Json::de_valor(&v))
    };
    let (mio, suyo) = (json(mio)?, json(suyo)?);
    let base = base.and_then(json);
    let junto = fusion::json(
        base.as_ref(),
        Some(&mio),
        Some(&suyo),
        &fusion::Criterio::default(),
        &mut fusion::Cuenta::default(),
    )?;
    Some(junto.a_texto())
}

/// Si es una cota o un marco del visor (no anotado a mano).
fn es_del_plano(e: &Elemento) -> bool {
    e.grupos.iter().any(|g| g == GRUPO_COTA || g == GRUPO_MARCO)
}

/// Las cotas y los marcos de la capa (`CapaDelPlano.leer`), en coordenadas del plano.
fn leer_cotas_y_marcos(
    elementos: &[Elemento],
    memoria: &Encaje,
) -> (Vec<Vec<[f64; 2]>>, Vec<[f64; 4]>) {
    let mut cotas = Vec::new();
    let mut marcos = Vec::new();
    for e in elementos.iter().filter(|e| !e.borrado) {
        if e.grupos.iter().any(|g| g == GRUPO_COTA) {
            if let Figura::Linea { puntos } = &e.figura
                && puntos.len() >= 2
            {
                cotas.push(
                    puntos
                        .iter()
                        .map(|p| memoria.a_plano([p.x as f64, p.y as f64]))
                        .collect(),
                );
            }
        } else if e.grupos.iter().any(|g| g == GRUPO_MARCO)
            && matches!(e.figura, Figura::Marco { .. })
        {
            let a = memoria.a_plano([e.x as f64, e.y as f64]);
            let b = memoria.a_plano([(e.x + e.ancho) as f64, (e.y + e.alto) as f64]);
            marcos.push([
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[0].max(b[0]),
                a[1].max(b[1]),
            ]);
        }
    }
    (cotas, marcos)
}

fn color(hex: u32) -> ColorRgba {
    let c = |d: u32| ((hex >> d) & 0xff) as f32 / 255.0;
    ColorRgba::opaco(c(16), c(8), c(0))
}

/// Una cota como figura de la capa (`CapaDelPlano.elementoDeCota`): una
/// raya naranja con los puntos de la cadena, y su id de su forma.
fn elemento_de_cota(c: &[[f64; 2]], memoria: &Encaje) -> Elemento {
    let puntos: Vec<Punto2> = c
        .iter()
        .map(|p| {
            let q = memoria.a_capa(*p);
            Punto2::nuevo(q[0] as f32, q[1] as f32)
        })
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &puntos {
        (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
    }
    let numeros: Vec<f64> = c.iter().flat_map(|p| [p[0], p[1]]).collect();
    let mut e = Elemento {
        x: puntos[0].x,
        y: puntos[0].y,
        ancho: x1 - x0,
        alto: y1 - y0,
        trazo: color(0xff8a00),
        grosor: 1.0,
        grupos: vec![GRUPO_COTA.into()],
        figura: Figura::Linea { puntos },
        ..Default::default()
    };
    e.extras.id_de_fichero = Some(pixpin_cad::anotado::huella(GRUPO_COTA, &numeros));
    e
}

/// Un marco para imprimir como figura de la capa (`elementoDeMarco`).
fn elemento_de_marco(m: &[f64; 4], memoria: &Encaje) -> Elemento {
    let a = memoria.a_capa([m[0], m[3]]);
    let mut e = Elemento {
        x: a[0] as f32,
        y: a[1] as f32,
        ancho: ((m[2] - m[0]) / memoria.u) as f32,
        alto: ((m[3] - m[1]) / memoria.u) as f32,
        trazo: color(0x3d8bff),
        grosor: 1.0,
        grupos: vec![GRUPO_MARCO.into()],
        figura: Figura::Marco {
            nombre: String::new(),
        },
        ..Default::default()
    };
    e.extras.id_de_fichero = Some(pixpin_cad::anotado::huella(GRUPO_MARCO, m));
    e
}

/// La forma de una cota o un marco de la capa en el plano: si es cota y sus
/// numeros (los puntos de la cota; `[x0, y0, x1, y1]` del marco).
fn forma_en_el_plano(e: &Elemento, memoria: &Encaje) -> Option<(bool, Vec<f64>)> {
    let (cotas, marcos) = leer_cotas_y_marcos(
        std::slice::from_ref(&Elemento {
            borrado: false,
            ..e.clone()
        }),
        memoria,
    );
    if let Some(c) = cotas.first() {
        return Some((true, c.iter().flat_map(|p| [p[0], p[1]]).collect()));
    }
    marcos.first().map(|m| (false, m.to_vec()))
}

/// **La capa con estas cotas y estos marcos** (`CapaDelPlano.con`): lo que ya
/// estaba igual se queda tal cual (mismo id, misma version); lo nuevo entra;
/// lo que ya no esta se marca borrado (y viaja como borrado); lo que se
/// quito y vuelve, revive. Dice si cambio algo.
///
/// «Igual» es por su id (el de su forma, como en Android) **o por su forma**:
/// una cota que llego del movil, leida aqui en `f32`, ya no da el mismo id
/// al recalcularlo, y sin esto se borraria y volveria a nacer con otro en
/// cada cambio, viajando de mas.
fn poner_en_la_escena(
    escena: &mut pixpin_motor2d::Escena,
    memoria: &Encaje,
    cotas: &[Vec<[f64; 2]>],
    marcos: &[[f64; 4]],
) -> bool {
    let id_de = |e: &Elemento| e.extras.id_de_fichero.clone().unwrap_or_default();
    // Lo querido: su elemento nuevo, si es cota y su forma.
    let mut queridos: Vec<(Elemento, bool, Vec<f64>)> = Vec::new();
    let de_cotas = cotas.iter().filter(|c| c.len() >= 2).map(|c| {
        (
            elemento_de_cota(c, memoria),
            true,
            c.iter().flat_map(|p| [p[0], p[1]]).collect(),
        )
    });
    let de_marcos = marcos
        .iter()
        .map(|m| (elemento_de_marco(m, memoria), false, m.to_vec()));
    for q in de_cotas.chain(de_marcos) {
        if !queridos.iter().any(|(e, ..)| id_de(e) == id_de(&q.0)) {
            queridos.push(q);
        }
    }
    // Menos de una milesima de unidad de la capa es la misma forma.
    let tope = memoria.u * 1e-3;
    let igual = |a: &[f64], b: &[f64]| {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tope)
    };
    let mut usados = vec![false; queridos.len()];
    let mut cambio = false;
    for e in escena.elementos.iter_mut().filter(|e| es_del_plano(e)) {
        let forma = forma_en_el_plano(e, memoria);
        let id = id_de(e);
        let cual = queridos
            .iter()
            .enumerate()
            .position(|(i, (q, cota, numeros))| {
                !usados[i]
                    && (id_de(q) == id
                        || forma
                            .as_ref()
                            .is_some_and(|(c, f)| c == cota && igual(f, numeros)))
            });
        match cual {
            Some(i) if e.borrado => {
                // Vuelve: con su id de siempre, para que el otro aparato lo junte.
                let (n, version, suyo) = (e.id, e.version, e.extras.id_de_fichero.clone());
                *e = queridos[i].0.clone();
                e.id = n;
                e.version = version;
                e.extras.id_de_fichero = suyo;
                e.tocar();
                usados[i] = true;
                cambio = true;
            }
            Some(i) => usados[i] = true,
            None if e.borrado => {}
            None => {
                e.borrado = true;
                e.tocar();
                cambio = true;
            }
        }
    }
    for (i, (q, ..)) in queridos.into_iter().enumerate() {
        if !usados[i] {
            escena.anadir(q);
            cambio = true;
        }
    }
    cambio
}

/// El papel del visor (el fondo claro u oscuro del plano): sobre el oscuro,
/// la tinta oscura se aclara (`dibujo::tema`), como en los lectores.
fn papel(claro: bool) -> pixpin_motor2d::ColorRgba {
    let c = if claro {
        Color {
            r: 0.99,
            g: 0.988,
            b: 0.976,
            a: 1.0,
        }
    } else {
        Color {
            r: 0.13,
            g: 0.15,
            b: 0.19,
            a: 1.0,
        }
    };
    lector_tinta::papel(c)
}

fn camara_de(v: &VistaPlano) -> Camara {
    let (x, y, zoom) = v.camara();
    Camara {
        x: x as f32,
        y: y as f32,
        zoom: zoom as f32,
    }
}

fn area_de(v: &VistaPlano) -> Rect {
    Rect {
        x: v.origen_pantalla.0,
        y: v.origen_pantalla.1,
        ancho: v.ancho,
        alto: v.alto,
    }
}

/// El motor del lienzo sobre un plano.
pub struct MotorDelPlano {
    rutas: Rutas,
    capa: Capa,
    tinta: Tinta,
    /// La caja del plano (para su `.hoja`).
    caja: [f32; 4],
    /// Como cae en el plano la capa en memoria y como la del fichero.
    memoria: Encaje,
    fichero: Encaje,
    /// El de Direct2D, sobre el dispositivo del visor (se rehace si cambia).
    motor: Option<(usize, MotorRender)>,
    /// Lo ultimo que se vio de cada elemento (su grosor y, si es texto, su
    /// letra): asi se sabe que nacio o que se le cambio el tamano desde el
    /// panel, y solo eso se pone a la escala del plano.
    visto: std::collections::HashMap<u64, (f32, Option<f32>)>,
    /// **La foto de lo anotado** (ver [`MotorDelPlano::pintar`]): mientras el
    /// plano se acerca o se arrastra, la capa no se repinta trazo a trazo.
    foto: pixpin_render::FotoDeCapa,
    /// La camara del ultimo fotograma: si la de ahora es otra, se esta moviendo.
    ultima_camara: Option<(f32, f32, f32)>,
    /// Lo ultimo pintado no quedo nitido (foto estirada, trazos de otra
    /// escala): hace falta otro fotograma.
    falta: bool,
    /// El tiempo de teselar de un fotograma quieto ([`PRESUPUESTO_DE_TESELAR`]).
    presupuesto_quieto: std::time::Duration,
    /// Para medir el antes (`rendimiento.rs`): trazo a trazo y todo de golpe.
    #[cfg(test)]
    como_antes: bool,
    /// Para medir (`rendimiento.rs`): el modo del ultimo fotograma y lo que
    /// costo elegirlo (rehacer la foto incluido).
    #[cfg(test)]
    medida: (Modo, std::time::Duration),
    /// La unidad de antes del plano (para una capa sin `.hoja`).
    antigua: Encaje,
    /// **La capa del disco tal como se leyo o escribio aqui la ultima vez**
    /// y su [`Sello`]: si el sello cambia, la sincronizacion trajo algo con
    /// el plano abierto, y con esto se junta (13-oct-2026).
    base: Option<String>,
    sello: Sello,
    /// Cuando se miro el disco por ultima vez (se mira sin prisa).
    mirado: Option<std::time::Instant>,
    /// El ultimo aviso del asistente visto: con uno nuevo se mira ya
    /// (`lector_tinta::turnos`).
    aviso: u64,
    /// Cuantas veces se releyo la capa del disco: va en la huella de la
    /// foto (lo releido puede traer los mismos ids y versiones).
    leida: u64,
}

/// Lo que se teselan como mucho por fotograma los trazos al cambiar de
/// escala (ver `Tinta::presupuesto`): con la capa en su foto mientras se
/// mueve, esto solo cuenta al pararse y al rehacer la foto.
const PRESUPUESTO_DE_TESELAR: std::time::Duration = std::time::Duration::from_millis(4);

/// El tiempo de teselar de un fotograma: nada moviendose (se pinta con lo
/// que haya, y nitido al pararse), [`PRESUPUESTO_DE_TESELAR`] quieto.
fn presupuesto_de(moviendose: bool, quieto: std::time::Duration) -> std::time::Duration {
    if moviendose {
        std::time::Duration::ZERO
    } else {
        quieto
    }
}

/// Cuanto puede estirarse o encogerse la foto mientras se mueve antes de
/// rehacerla: al doble se ve blanda; a la mitad deja mucho fuera (que se
/// pinta vivo).
const ESTIRAR_LA_FOTO: std::ops::RangeInclusive<f32> = 0.5..=2.0;

/// Como se pinta lo anotado en un fotograma ([`MotorDelPlano::pintar`]).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Modo {
    /// La capa entera, trazo a trazo (mientras se traza algo).
    Directo,
    /// La foto tal cual: hecha nitida con esta misma camara.
    Foto,
    /// La foto puesta con la camara de ahora, y vivo lo que no tiene.
    Movida,
}

/// **La huella de lo que hay en la capa**: si cambia, la foto ya no vale.
/// Cada elemento (su id, su version y si esta borrado), si se anota (fuera
/// de anotar no salen las cotas ni los marcos) y el papel.
fn huella_de_la_capa(capa: &Capa, leida: u64, anotando: bool, claro: bool) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ ((anotando as u64) << 1) ^ claro as u64;
    let mut mezclar = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mezclar(leida);
    mezclar(capa.escena.elementos.len() as u64);
    for e in &capa.escena.elementos {
        mezclar(e.id);
        mezclar(((e.version as u64) << 1) | e.borrado as u64);
    }
    h
}

/// Cada cuanto, como mucho, se mira si el disco cambio mientras se ve el plano.
const MIRAR_EL_DISCO: std::time::Duration = std::time::Duration::from_millis(700);

/// Los grosores que pone el motor (fino, medio y grueso de las formas, del
/// lapiz y del resaltador), al nacer algo o al elegirlos en el panel: solo
/// eso se pone a la escala del plano; lo pegado o duplicado ya la trae.
fn grosor_de_fabrica(g: f32) -> bool {
    use pixpin_motor2d::estilo::NivelGrosor as N;
    [N::Fino, N::Medio, N::Grueso].iter().any(|n| {
        [n.de_forma(), n.de_tinta(), n.de_resaltador()]
            .iter()
            .any(|v| (v - g).abs() < 1e-4)
    })
}

/// Lo mismo con la letra: los tamanos del panel y el de fabrica.
fn letra_de_fabrica(t: f32, la_elegida: f32) -> bool {
    (t - la_elegida).abs() < 1e-4
        || pixpin_ui::panel_lateral::TAMANOS_DE_LETRA
            .iter()
            .any(|v| (v - t).abs() < 1e-4)
}

/// El grosor y la letra de un elemento, para ver si cambiaron.
fn medidas_de(e: &pixpin_motor2d::Elemento) -> (f32, Option<f32>) {
    let letra = match &e.figura {
        pixpin_motor2d::Figura::Texto { tam, .. } => Some(*tam),
        _ => None,
    };
    (e.grosor, letra)
}

impl MotorDelPlano {
    pub fn nuevo(rutas: Rutas, m: &Modelo) -> MotorDelPlano {
        let (base, sello) = (
            pixpin_proyecto::anotado::leer(&rutas.tinta),
            sello_de(&rutas),
        );
        let (capa, memoria, fichero) = leer_capa(&rutas, m);
        let mut tinta = Tinta::nueva();
        tinta.hoja = Some(0);
        tinta.presupuesto = Some(PRESUPUESTO_DE_TESELAR);
        let visto = capa
            .escena
            .elementos
            .iter()
            .map(|e| (e.id, medidas_de(e)))
            .collect();
        let antigua = Encaje::de_unidad(pixpin_cad::anotado::unidad_antigua(m));
        MotorDelPlano {
            rutas,
            capa,
            tinta,
            caja: m.caja,
            memoria,
            fichero,
            motor: None,
            visto,
            foto: pixpin_render::FotoDeCapa::nueva(),
            ultima_camara: None,
            falta: false,
            presupuesto_quieto: PRESUPUESTO_DE_TESELAR,
            #[cfg(test)]
            como_antes: false,
            #[cfg(test)]
            medida: (Modo::Directo, std::time::Duration::ZERO),
            antigua,
            base,
            sello,
            mirado: None,
            aviso: 0,
            leida: 0,
        }
    }

    /// **Relee la capa del disco** (lo que dejo la sincronizacion), con su
    /// `.hoja`, y la toma como base. Lo elegido se suelta: los numeros de la
    /// escena son otros.
    fn releer(&mut self) {
        let fichero = encaje_del_fichero(&self.rutas, self.caja, &self.antigua, &self.memoria);
        self.base = pixpin_proyecto::anotado::leer(&self.rutas.tinta);
        self.sello = sello_de(&self.rutas);
        self.capa = Capa::leer_en(&self.rutas.tinta, unidades_entre(&self.memoria, &fichero));
        self.fichero = fichero;
        self.visto = self
            .capa
            .escena
            .elementos
            .iter()
            .map(|e| (e.id, medidas_de(e)))
            .collect();
        self.tinta.gesto.seleccion.limpiar();
        // Lo pintado de la capa de antes no vale: la foto se tira y la huella
        // cambia (`leida`), y lo teselado tambien (al releer en otras
        // unidades, mismos ids y versiones son otra geometria).
        self.leida += 1;
        self.foto.soltar();
        self.tinta.olvidar_lo_calculado();
        self.tinta.gesto.lazo = None;
    }

    /// **Lo que trajo la sincronizacion, a la vista** (13-oct-2026): si la
    /// capa cambio en el disco y aqui no hay nada sin guardar, se relee. Dice
    /// si releyo. Con algo a medias no: eso se junta al guardar.
    pub fn al_dia(&mut self) -> bool {
        if self.capa.sucia
            || self.tinta.gesto.esta_escribiendo()
            || sello_de(&self.rutas) == self.sello
        {
            return false;
        }
        self.releer();
        true
    }

    /// [`MotorDelPlano::al_dia`], sin mirar el disco mas de una vez cada
    /// [`MIRAR_EL_DISCO`] (se llama al pintar cada fotograma).
    fn mirar_el_disco(&mut self) {
        let avisado = lector_tinta::turnos::hay_aviso(&mut self.aviso);
        if !avisado && self.mirado.is_some_and(|t| t.elapsed() < MIRAR_EL_DISCO) {
            return;
        }
        self.mirado = Some(std::time::Instant::now());
        self.al_dia();
    }

    /// **Los tamanos del lienzo, a la escala del plano** (lo pidio el usuario
    /// el 9-oct-2026: «la tinta es muy grande para el plano»). Lo que nace
    /// se pone a la escala de lo que se ve al dibujarlo (uno entre el zoom):
    /// un trazo «fino» se ve fino en la pantalla, como al anotar en AutoCAD,
    /// y se puede acercar para detallar (`escalaDeLoNuevo` de Android).
    ///
    /// Vale tambien al **elegir otro tamano en el panel** (10-oct-2026, el
    /// usuario: «al seleccionar el tamano de la letra o el lapiz con las
    /// opciones disponibles se agrandan grandemente como antes y ya no hay
    /// forma de volver a ese tamano pequeno»): el panel pone el valor de una
    /// hoja tal cual, tanto a lo nuevo como a lo elegido; ahora, todo grosor
    /// o letra del panel que aparezca (al nacer o al cambiar) se pasa a la
    /// escala del plano. Lo que no viene del panel (estirar un texto, pegar,
    /// deshacer) se deja como esta.
    fn a_la_escala_del_plano(&mut self, zoom: f32) {
        let letra = self.tinta.gesto.estilo.tamano_letra;
        poner_a_escala(
            &mut self.capa.escena.elementos,
            &mut self.visto,
            1.0 / zoom.max(1e-6),
            letra,
        );
    }

    /// Escribe lo anotado si cambio: primero la `.hoja` (una capa sin su
    /// hoja al lado se leeria con otra escala, como en Android) y luego la
    /// capa, en las unidades del fichero. Un plano sin nada hecho no deja
    /// ficheros.
    ///
    /// **Si la capa cambio en el disco desde que se leyo** (la trajo la
    /// sincronizacion con el plano abierto), se junta con ella y no se pisa
    /// (ver [`MotorDelPlano::guardar_juntando`]).
    fn guardar(&mut self) {
        if !self.capa.sucia {
            return;
        }
        if self.capa.escena.elementos.is_empty() && !self.rutas.tinta.exists() {
            self.capa.sucia = false;
            return;
        }
        let hecho = if self.rutas.tinta.is_file() && sello_de(&self.rutas) != self.sello {
            self.guardar_juntando()
        } else {
            escribir_hoja(&self.rutas.hoja, self.fichero.hoja(self.caja)).and_then(|()| {
                if let Some(d) = self.rutas.tinta.parent() {
                    std::fs::create_dir_all(d)?;
                }
                self.capa.guardar(&self.rutas.tinta)
            })
        };
        match hecho {
            Ok(()) => {
                self.base = pixpin_proyecto::anotado::leer(&self.rutas.tinta);
                self.sello = sello_de(&self.rutas);
            }
            Err(e) => {
                tracing::warn!(?e, ruta = %self.rutas.tinta.display(), "no se pudo guardar lo anotado en el plano")
            }
        }
    }

    /// **Guardar sin pisar lo que llego** (13-oct-2026). Antes se escribia la
    /// capa leida al abrir mas lo de aqui: lo que el movil mando mientras
    /// tanto se perdia aqui y, en la vuelta siguiente, tambien alli (la base
    /// lo tenia y el PC ya no: borrado). Ahora lo de aqui se escribe aparte,
    /// se pasa a las unidades de la `.hoja` del disco si son otras, y se
    /// junta con lo del disco con la regla de la sincronizacion. Luego se
    /// relee lo junto: se sigue anotando sobre ello.
    fn guardar_juntando(&mut self) -> std::io::Result<()> {
        let aparte = self.rutas.tinta.with_extension("excalidraw.mio");
        self.capa.guardar(&aparte)?;
        let mio = std::fs::read_to_string(&aparte);
        let _ = std::fs::remove_file(&aparte);
        let mio = mio?;
        let fichero = encaje_del_fichero(&self.rutas, self.caja, &self.antigua, &self.memoria);
        let paso = unidades_entre(&self.fichero, &fichero);
        let (base, mio) = if paso == Unidades::DEL_PC {
            (self.base.clone(), Some(mio))
        } else {
            (
                self.base
                    .as_deref()
                    .and_then(|b| en_otras_unidades(b, paso)),
                en_otras_unidades(&mio, paso),
            )
        };
        let suyo = pixpin_proyecto::anotado::leer(&self.rutas.tinta).unwrap_or_default();
        let junto = mio
            .as_deref()
            .and_then(|m| juntar_con_el_disco(base.as_deref(), m, &suyo));
        let Some(junto) = junto else {
            // Algo no se entiende (el disco danado): lo de aqui, como antes.
            tracing::warn!(ruta = %self.rutas.tinta.display(), "la capa del disco no se entiende: se escribe la de aqui");
            escribir_hoja(&self.rutas.hoja, self.fichero.hoja(self.caja))?;
            self.capa.sucia = true;
            return self.capa.guardar(&self.rutas.tinta);
        };
        escribir_hoja(&self.rutas.hoja, fichero.hoja(self.caja))?;
        pixpin_sincro::disco::escribir_atomico(&self.rutas.tinta, junto.as_bytes())?;
        self.releer();
        Ok(())
    }

    /// Pinta lo anotado (y, anotando, la barra y el panel) con `p`, ya puesto
    /// sobre el fotograma del visor. Fuera de anotar no salen las cotas ni
    /// los marcos (los pinta el visor, con sus medidas, como en Android).
    ///
    /// `modo` dice si la capa va trazo a trazo o en su foto (ver
    /// [`MotorDelPlano::pintar`]).
    fn pintar_con(
        &mut self,
        p: &pixpin_render::Pintor,
        v: &VistaPlano,
        anotando: bool,
        modo: Modo,
    ) {
        let cam = camara_de(v);
        let ocultas = self.ocultar_cotas(anotando);
        let (capa, tinta, foto) = (&self.capa, &mut self.tinta, &self.foto);
        let parte = match modo {
            Modo::Directo => Parte::Toda,
            Modo::Foto => Parte::Encima,
            Modo::Movida => Parte::FueraDe(foto.cubre()),
        };
        if modo != Modo::Directo {
            // La foto, en pixeles de la ventana (la transformada aun es la
            // identidad): una sola llamada por toda la capa.
            foto.poner(p, (cam.x, cam.y, cam.zoom));
        }
        if parte != Parte::Encima || anotando {
            // Lo anotado, en las unidades de la capa con la camara del plano.
            p.poner_vista((0.0, 0.0), cam.zoom, (-cam.x * cam.zoom, -cam.y * cam.zoom));
            let vista = (
                cam.x,
                cam.y,
                cam.x + v.ancho as f32 / cam.zoom,
                cam.y + v.alto as f32 / cam.zoom,
            );
            // Moviendose, sin teselar nada a la escala nueva: lo vivo va con lo
            // que ya tiene (solo lo que no tiene nada se tesela); al pararse,
            // nitido.
            if modo != Modo::Directo {
                tinta.presupuesto = Some(presupuesto_de(
                    modo == Modo::Movida,
                    self.presupuesto_quieto,
                ));
            }
            tinta.pintar_capa_parte(p, 0, capa, vista, cam.zoom, papel(v.claro), anotando, parte);
            if tinta.pendientes() > 0 {
                self.falta = true;
            }
        }
        // La barra y el panel del lienzo, en pixeles de la ventana.
        p.desplazar(0.0, 0.0);
        if anotando {
            tinta.pintar_interfaz(p, Some(capa), area_de(v), v.escala_por_cien);
        }
        self.mostrar_cotas(&ocultas);
    }

    /// Las cotas y los marcos, fuera mientras se pinta sin anotar (sin tocar
    /// su version): los pinta el visor. Devuelve cuales se escondieron.
    fn ocultar_cotas(&mut self, anotando: bool) -> Vec<usize> {
        if anotando {
            return Vec::new();
        }
        let ocultas: Vec<usize> = self
            .capa
            .escena
            .elementos
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.borrado && es_del_plano(e))
            .map(|(i, _)| i)
            .collect();
        for &i in &ocultas {
            self.capa.escena.elementos[i].borrado = true;
        }
        ocultas
    }

    fn mostrar_cotas(&mut self, ocultas: &[usize]) {
        for &i in ocultas {
            self.capa.escena.elementos[i].borrado = false;
        }
    }

    /// **Que hacer con la capa en este fotograma.** Mientras se traza algo,
    /// trazo a trazo (cambia en cada fotograma). Si no: con la camara quieta
    /// y la foto nitida de esta camara, la foto tal cual; moviendose (acercar,
    /// alejar, arrastrar) con una foto de lo mismo que no haya que estirar de
    /// mas, la foto puesta con la camara de ahora; y si no, se rehace la foto.
    fn elegir_modo(
        &mut self,
        motor: &MotorRender,
        v: &VistaPlano,
        anotando: bool,
        moviendose: bool,
    ) -> Modo {
        if anotando && self.tinta.gesto.elemento_en_curso().is_some() {
            return Modo::Directo;
        }
        #[cfg(test)]
        if self.como_antes {
            self.tinta.presupuesto = None;
            return Modo::Directo;
        }
        let cam = camara_de(v);
        let camara = (cam.x, cam.y, cam.zoom);
        let tamano = (v.ancho, v.alto);
        let huella = huella_de_la_capa(&self.capa, self.leida, anotando, v.claro);
        if self.foto.vale_tal_cual(huella, tamano, camara) {
            return Modo::Foto;
        }
        if moviendose
            && self.foto.vale(huella, tamano)
            && ESTIRAR_LA_FOTO.contains(&(cam.zoom / self.foto.camara().2))
        {
            // Al pararse, otro fotograma nitido.
            self.falta = true;
            return Modo::Movida;
        }
        // La foto de nuevo, con lo anotado a esta camara.
        let ocultas = self.ocultar_cotas(anotando);
        // Moviendose (la foto se quedo corta), sin parar a teselar: lo que
        // falte se rehace al pararse.
        self.tinta.presupuesto = Some(presupuesto_de(moviendose, self.presupuesto_quieto));
        let (capa, tinta) = (&self.capa, &mut self.tinta);
        let vista = (
            cam.x,
            cam.y,
            cam.x + v.ancho as f32 / cam.zoom,
            cam.y + v.alto as f32 / cam.zoom,
        );
        let hecha = self.foto.hacer(motor, tamano, camara, huella, |p| {
            p.poner_vista((0.0, 0.0), cam.zoom, (-cam.x * cam.zoom, -cam.y * cam.zoom));
            tinta.pintar_capa_parte(
                p,
                0,
                capa,
                vista,
                cam.zoom,
                papel(v.claro),
                anotando,
                Parte::Dibujado,
            );
        });
        self.mostrar_cotas(&ocultas);
        match hecha {
            Ok(()) => {
                if self.tinta.pendientes() > 0 {
                    // Algo quedo de otra escala: se rehace en el siguiente.
                    self.foto.no_nitida();
                    self.falta = true;
                }
                Modo::Foto
            }
            Err(e) => {
                tracing::warn!(?e, "sin foto de lo anotado: trazo a trazo");
                self.foto.soltar();
                Modo::Directo
            }
        }
    }
}

/// Ver [`MotorDelPlano::a_la_escala_del_plano`]: `k` es uno entre el zoom y
/// `letra`, la letra elegida ahora en el motor.
fn poner_a_escala(
    elementos: &mut [pixpin_motor2d::Elemento],
    visto: &mut std::collections::HashMap<u64, (f32, Option<f32>)>,
    k: f32,
    letra: f32,
) {
    for e in elementos.iter_mut() {
        let antes = visto.get(&e.id).copied();
        let (g, t) = medidas_de(e);
        let grosor_nuevo = antes.is_none_or(|(g0, _)| (g0 - g).abs() > 1e-6);
        if grosor_nuevo && grosor_de_fabrica(g) {
            e.grosor *= k;
        }
        let letra_nueva = antes
            .is_none_or(|(_, t0)| t0.is_none_or(|t0| t.is_some_and(|t| (t0 - t).abs() > 1e-6)));
        if letra_nueva
            && let pixpin_motor2d::Figura::Texto { tam, .. } = &mut e.figura
            && letra_de_fabrica(*tam, letra)
        {
            *tam *= k;
            e.ancho *= k;
            e.alto *= k;
        }
        visto.insert(e.id, medidas_de(e));
    }
}

impl Anotador for MotorDelPlano {
    fn evento(&mut self, ev: EventoPlano, v: &VistaPlano) -> Respuesta {
        let p = |x: i32, y: i32| Punto { x, y };
        let ev = match ev {
            EventoPlano::Mover(x, y) => EventoOverlay::RatonMovido(p(x, y)),
            EventoPlano::Bajar(x, y) => EventoOverlay::BotonPulsado(p(x, y)),
            EventoPlano::Subir(x, y) => EventoOverlay::BotonSoltado(p(x, y)),
            EventoPlano::BajarCentral(x, y) => EventoOverlay::BotonCentralPulsado(p(x, y)),
            EventoPlano::SubirCentral(x, y) => EventoOverlay::BotonCentralSoltado(p(x, y)),
            EventoPlano::Rueda(d) => EventoOverlay::Rueda(d),
            EventoPlano::Tecla {
                vk,
                shift,
                ctrl,
                alt,
            } => EventoOverlay::Tecla {
                vk,
                shift,
                ctrl,
                alt,
            },
            EventoPlano::TeclaSoltada(vk) => EventoOverlay::TeclaSoltada(vk),
            EventoPlano::Caracter(c) => EventoOverlay::Caracter(c),
        };
        self.tinta.hoja = Some(0);
        let cam = camara_de(v);
        let h = self
            .tinta
            .evento(&ev, &mut self.capa, &cam, area_de(v), v.escala_por_cien);
        self.a_la_escala_del_plano(cam.zoom);
        if h.cambio {
            self.capa.sucia = true;
        }
        Respuesta {
            consumido: h.consumido,
            salir: h.salir,
            arrastra: h.arrastra,
        }
    }

    /// **Lo anotado, sin frenar el zoom** (13-oct-2026, el usuario: «en los
    /// CAD con anotaciones el zoom va lagueado»). Medido en este PC
    /// (`rendimiento.rs`, 378 trazos): repintar la capa trazo a trazo costaba
    /// 7 a 12 ms por fotograma (una llamada de Direct2D por trazo), mas que
    /// el plano entero; y al pasar de vez y media de zoom se teselaba todo de
    /// golpe, de 140 a 260 ms. Ahora, mientras la camara se mueve, la capa va
    /// en su foto (`FotoDeCapa`: una sola llamada, estirada con la camara de
    /// ahora) y vivo solo lo que la foto no tiene; al pararse se pinta nitida
    /// otra vez, y lo que hay que teselar a la escala nueva se rehace poco a
    /// poco (`Tinta::presupuesto`), pintando mientras lo de antes.
    fn pintar(
        &mut self,
        d3d: &ID3D11Device,
        destino: &ID3D11Texture2D,
        v: &VistaPlano,
        anotando: bool,
    ) {
        self.falta = false;
        // Lo que trajo la sincronizacion, sin cerrar el plano.
        if !anotando {
            self.mirar_el_disco();
        }
        if self.capa.vacia() && !anotando {
            // Nada que pintar: ni foto que guardar.
            self.foto.soltar();
            self.ultima_camara = None;
            return;
        }
        let clave = d3d.as_raw() as usize;
        if self.motor.as_ref().is_none_or(|(k, _)| *k != clave) {
            // Otro dispositivo (u otro Direct2D): la foto y lo teselado eran
            // del de antes, y mezclarlos falla al pintar (D2DERR_WRONG_FACTORY).
            self.foto.soltar();
            self.tinta.olvidar_lo_calculado();
            match MotorRender::nuevo(d3d) {
                Ok(m) => self.motor = Some((clave, m)),
                Err(e) => {
                    tracing::warn!(?e, "sin Direct2D para la tinta del plano");
                    return;
                }
            }
        }
        let Some((_, motor)) = self.motor.take() else {
            return;
        };
        let cam = camara_de(v);
        let camara = (cam.x, cam.y, cam.zoom);
        let moviendose = self.ultima_camara.is_some_and(|c| c != camara);
        self.ultima_camara = Some(camara);
        #[cfg(test)]
        let t_modo = std::time::Instant::now();
        let modo = self.elegir_modo(&motor, v, anotando, moviendose);
        #[cfg(test)]
        {
            self.medida = (modo, t_modo.elapsed());
        }
        if let Ok(bitmap) = motor.destino_backbuffer(destino)
            && let Err(e) = motor.dibujar(&bitmap, |p| self.pintar_con(p, v, anotando, modo))
        {
            tracing::warn!(?e, "no se pudo pintar la tinta del plano");
        }
        self.motor = Some((clave, motor));
    }

    fn falta_pintar(&self) -> bool {
        self.falta
    }

    fn terminar(&mut self) {
        // Lo que se escribia se queda escrito, y lo elegido se suelta.
        if self.tinta.gesto.esta_escribiendo() {
            self.tinta.gesto.cerrar_texto(&mut self.capa.escena);
            self.capa.sucia = true;
        }
        self.tinta.gesto.seleccion.limpiar();
        self.tinta.gesto.lazo = None;
        self.guardar();
    }

    fn quiere_escape(&self) -> bool {
        self.tinta.quiere_escape()
    }

    fn cotas_y_marcos(&self) -> (Vec<Vec<[f64; 2]>>, Vec<[f64; 4]>) {
        leer_cotas_y_marcos(&self.capa.escena.elementos, &self.memoria)
    }

    fn poner_cotas_y_marcos(&mut self, cotas: &[Vec<[f64; 2]>], marcos: &[[f64; 4]]) {
        if poner_en_la_escena(&mut self.capa.escena, &self.memoria, cotas, marcos) {
            // Lo puesto aqui ya va a su medida: no es «lo nuevo del panel».
            for e in &self.capa.escena.elementos {
                self.visto.insert(e.id, medidas_de(e));
            }
            self.capa.sucia = true;
            self.guardar();
        }
    }
}

/// **Lo anotado, en una hoja impresa**: la capa con el motor (el mismo
/// pintado que en pantalla), en papel blanco. Sin las cotas ni los marcos
/// de la capa: la hoja ya los dibuja a su manera.
pub struct ParaImprimir {
    capa: Capa,
    tinta: std::cell::RefCell<Tinta>,
    /// Unidades del plano por unidad de la capa.
    pub unidad: f64,
}

impl ParaImprimir {
    pub fn leer(rutas: &Rutas, m: &Modelo) -> Option<ParaImprimir> {
        let (mut capa, memoria, _) = leer_capa(rutas, m);
        for e in capa.escena.elementos.iter_mut().filter(|e| es_del_plano(e)) {
            e.borrado = true;
        }
        (!capa.vacia()).then(|| ParaImprimir {
            capa,
            tinta: std::cell::RefCell::new(Tinta::nueva()),
            unidad: memoria.u,
        })
    }

    /// Pinta la capa a `escala` (unidades del destino por unidad de la capa)
    /// con su (0, 0) en `cero`. `visible`: lo que se ve, en la capa.
    pub fn pintar(
        &self,
        p: &pixpin_render::Pintor,
        escala: f32,
        cero: (f32, f32),
        visible: (f32, f32, f32, f32),
    ) {
        p.poner_vista((0.0, 0.0), escala, cero);
        let blanco = lector_tinta::papel(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
        self.tinta
            .borrow_mut()
            .pintar_capa(p, 0, &self.capa, visible, escala, blanco, false);
    }
}

// ---------------------------------------------------------------------------
// Desde fuera del visor (el asistente, 14-oct-2026).

/// Las cotas y los marcos de la capa de `plano` (para exportarlo sin abrir
/// el visor), en coordenadas del plano.
pub(crate) fn cotas_y_marcos_de(rutas: &Rutas, m: &Modelo) -> (Vec<Vec<[f64; 2]>>, Vec<[f64; 4]>) {
    let (capa, memoria, _) = leer_capa(rutas, m);
    leer_cotas_y_marcos(&capa.escena.elementos, &memoria)
}

/// Un marco para imprimir (`plano-marco`), `[x0, y0, x1, y1]` del plano,
/// como lo pone el visor.
pub(crate) fn marco_del_plano(marco: &[f64; 4], encaje: &Encaje) -> Elemento {
    elemento_de_marco(marco, encaje)
}

/// **Anade figuras a la capa del plano sin el visor**: se lee la capa tal
/// cual esta en el disco (en sus unidades, las de su `.hoja`), `hacer`
/// construye las figuras con ese encaje (coordenadas del plano → capa) y se
/// escribe primero la `.hoja` y luego la capa, como al guardar desde el
/// visor. Lo que puso el movil se queda tal cual. Si el plano esta abierto,
/// el visor lo relee solo (`MotorDelPlano::al_dia`). Devuelve cuantas.
pub(crate) fn anadir_figuras(
    rutas: &Rutas,
    m: &Modelo,
    hacer: impl FnOnce(&Encaje) -> Vec<Elemento>,
) -> std::io::Result<usize> {
    let memoria = Encaje::de_unidad(pixpin_cad::anotado::unidad_de_la_capa(m));
    let antigua = Encaje::de_unidad(pixpin_cad::anotado::unidad_antigua(m));
    let fichero = encaje_del_fichero(rutas, m.caja, &antigua, &memoria);
    let mut capa = Capa::leer(&rutas.tinta);
    let nuevas = hacer(&fichero);
    let n = nuevas.len();
    if n == 0 {
        return Ok(0);
    }
    for e in nuevas {
        capa.escena.anadir(e);
    }
    capa.sucia = true;
    escribir_hoja(&rutas.hoja, fichero.hoja(m.caja))?;
    if let Some(d) = rutas.tinta.parent() {
        std::fs::create_dir_all(d)?;
    }
    capa.guardar(&rutas.tinta)?;
    Ok(n)
}

#[cfg(test)]
mod pruebas;

#[cfg(test)]
mod pruebas_con_el_movil;

#[cfg(test)]
mod rendimiento;

#[cfg(test)]
mod pruebas_foto;
