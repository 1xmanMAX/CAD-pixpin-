//! **Exportar un plano** (13-oct-2026, el usuario: «al visor de AutoCAD
//! implementale un exportador en formato HTML en el que se considere todo»).
//!
//! Los formatos del plano para el «Exportar» comun ([`crate::compartir::exportable`]):
//!
//! - **HTML**: la pagina que se mueve y acerca como el visor, con las capas
//!   que se encienden y apagan, los grosores, las imagenes, las cotas, los
//!   marcos y lo anotado con el motor del lienzo (`pixpin_cad::web`).
//! - **PDF**: en vectores, una hoja por marco (o el plano entero), con los
//!   grosores de sus capas y lo anotado encima en vectores
//!   (`pixpin_pdf::con_anotaciones`, lo mismo que en un PDF anotado).
//! - **PNG**: el plano pintado como al imprimir (`imprimir::dibujar`).
//! - **DXF** (de un DWG): lo escribe `opencadcodec`.
//! - Original y `.pixpin`: los pone el componente comun.
//!
//! El plano **por capas** (para el HTML y el PDF) y el DXF se hacen en el
//! proceso aparte (`pixpinmax.exe --cad-exportar`), como al abrir: un DWG
//! roto solo tumba a ese proceso. Lo leido por capas queda en la cache.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pixpin_cad::convertir::{GROSOR_NORMAL, Grupo, PlanoPorCapas};
use pixpin_cad::modelo::{CORTE, GLIFO_DE_RAYAS, Modelo, Tramo};
use pixpin_motor2d::pintado::Orden;
use pixpin_motor2d::vector::Punto2;

use super::anotar::{self, Rutas};

/// La orden con que PixPin se llama a si misma para exportar un plano.
pub const ORDEN: &str = "--cad-exportar";

/// En el proceso aparte: `--cad-exportar <capas|dxf> <plano> <salida> [letras]`.
pub fn en_este_proceso(args: &[std::ffi::OsString]) -> i32 {
    let (Some(tipo), Some(entrada), Some(salida)) = (args.first(), args.get(1), args.get(2)) else {
        return 2;
    };
    let (entrada, salida) = (PathBuf::from(entrada), PathBuf::from(salida));
    let temporal = salida.with_extension("tmp");
    let hecho: Result<(), String> = match tipo.to_str() {
        Some("capas") => {
            pixpin_cad::letras::poner_carpeta(args.get(3).map(PathBuf::from));
            pixpin_cad::convertir::convertir_fichero_por_capas(&entrada).and_then(|p| {
                std::fs::write(&temporal, pixpin_cad::web::a_bytes(&p)).map_err(|e| e.to_string())
            })
        }
        Some("dxf") => pixpin_cad::leer::a_dxf(&entrada, &temporal),
        _ => return 2,
    };
    match hecho.and_then(|()| std::fs::rename(&temporal, &salida).map_err(|e| e.to_string())) {
        Ok(()) => 0,
        Err(e) => {
            let _ = std::fs::remove_file(&temporal);
            let _ = std::fs::write(salida.with_extension("error"), e);
            1
        }
    }
}

/// Lanza el proceso aparte y espera.
fn aparte(tipo: &str, plano: &Path, salida: &Path, letras: Option<&Path>) -> Result<()> {
    let exe = std::env::current_exe().context("sin el ejecutable")?;
    let mut orden = std::process::Command::new(exe);
    orden
        .arg(super::ORDEN_EXPORTAR)
        .arg(tipo)
        .arg(plano)
        .arg(salida);
    if let Some(l) = letras {
        orden.arg(l);
    }
    {
        use std::os::windows::process::CommandExt;
        orden.creation_flags(0x0800_0000);
    }
    let estado = orden.status().context("no se pudo lanzar la exportacion")?;
    if !estado.success() {
        let motivo = std::fs::read_to_string(salida.with_extension("error")).unwrap_or_default();
        let _ = std::fs::remove_file(salida.with_extension("error"));
        anyhow::bail!("el plano no se pudo exportar: {motivo}");
    }
    Ok(())
}

/// **El plano por capas**, de la cache o del proceso aparte.
fn por_capas(raiz: &Path, plano: &Path) -> Result<PlanoPorCapas> {
    let carpeta = raiz.join("cache").join("cad");
    std::fs::create_dir_all(&carpeta)?;
    let clave = super::clave(plano).context("no se puede leer el plano")?;
    let letras = pixpin_cad::letras::carpeta_en(raiz);
    let destino = carpeta.join(format!(
        "{clave}-{}.pxcapas",
        pixpin_cad::letras::huella(&letras)
    ));
    if let Ok(b) = std::fs::read(&destino)
        && let Some(p) = pixpin_cad::web::de_bytes(&b)
    {
        return Ok(p);
    }
    // En las pruebas no hay `pixpinmax.exe` que llamar: aqui mismo.
    if cfg!(test) {
        let p = pixpin_cad::convertir::convertir_fichero_por_capas(plano)
            .map_err(|e| anyhow::anyhow!(e))?;
        std::fs::write(&destino, pixpin_cad::web::a_bytes(&p))?;
        return Ok(p);
    }
    aparte("capas", plano, &destino, Some(&letras))?;
    let b = std::fs::read(&destino)?;
    pixpin_cad::web::de_bytes(&b).context("lo leido por capas no se entiende")
}

/// **Lo que hace falta para exportar un plano**: el modelo del visor, sus
/// cotas y marcos y donde esta lo anotado.
#[derive(Clone)]
pub struct Plano {
    pub raiz: PathBuf,
    pub ruta: PathBuf,
    pub modelo: std::sync::Arc<Modelo>,
    pub cotas: Vec<Vec<[f64; 2]>>,
    pub marcos: Vec<[f64; 4]>,
    pub capa: Rutas,
}

impl Plano {
    pub fn nuevo(
        raiz: &Path,
        ruta: &Path,
        modelo: &Modelo,
        cotas: &[Vec<[f64; 2]>],
        marcos: &[[f64; 4]],
    ) -> Plano {
        Plano {
            raiz: raiz.to_path_buf(),
            ruta: ruta.to_path_buf(),
            modelo: std::sync::Arc::new(modelo.clone()),
            cotas: cotas.to_vec(),
            marcos: marcos.to_vec(),
            capa: anotar::rutas_de_la_capa(raiz, ruta),
        }
    }

    fn titulo(&self) -> String {
        self.ruta
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Los grupos por capas o, si no se pudo leer asi, el plano del visor
    /// entero como una sola capa.
    fn grupos(&self) -> Vec<Grupo> {
        match por_capas(&self.raiz, &self.ruta) {
            Ok(p) if !p.grupos.is_empty() => p.grupos,
            r => {
                if let Err(e) = r {
                    tracing::info!(?e, "el plano va sin capas");
                }
                vec![Grupo {
                    capa: "0".into(),
                    grosor: GROSOR_NORMAL,
                    encendida: true,
                    modelo: (*self.modelo).clone(),
                }]
            }
        }
    }

    /// Lo anotado (sin las cotas ni los marcos, que se pintan aparte).
    fn anotado(&self) -> Option<(pixpin_motor2d::Escena, pixpin_cad::anotado::Encaje)> {
        if !self.capa.tinta.is_file() {
            return None;
        }
        let (capa, memoria, _) = anotar::leer_capa(&self.capa, &self.modelo);
        let mut escena = capa.escena;
        escena.elementos.retain(|e| {
            !e.borrado
                && !e.grupos.iter().any(|g| {
                    g == pixpin_cad::anotado::GRUPO_COTA || g == pixpin_cad::anotado::GRUPO_MARCO
                })
        });
        (!escena.elementos.is_empty()).then_some((escena, memoria))
    }
}

// ---------------------------------------------------------------- HTML

/// **La pagina web del plano.**
pub fn html(p: &Plano, idioma: pixpin_store::Idioma) -> Result<String> {
    let m = &p.modelo;
    let capas = por_capas(&p.raiz, &p.ruta).ok();
    let grupos = match &capas {
        Some(c) if !c.grupos.is_empty() => c.grupos.clone(),
        _ => p.grupos(),
    };
    let anotado = p.anotado().and_then(|(escena, enc)| {
        let hoja = pixpin_motor2d::exportar::hojas(
            &escena,
            pixpin_motor2d::exportar::Alcance::Todo,
            &[],
            None,
        )
        .into_iter()
        .next()?;
        let svg = pixpin_motor2d::exportar_svg::svg(
            &hoja,
            pixpin_motor2d::exportar_svg::OpcionesSvg {
                fondo: None,
                marcos: false,
            },
            &|_| None,
        );
        let c = hoja.caja;
        Some(pixpin_cad::web::Anotado {
            svg,
            caja: [c.0 as f64, c.1 as f64, c.2 as f64, c.3 as f64],
            u: enc.u,
            cero: [enc.cero_x, enc.cero_y],
        })
    });
    let imagenes = capas
        .as_ref()
        .map(|c| imagenes(&p.ruta, &c.imagenes, m.origen))
        .unwrap_or_default();
    let u = m.caja_util();
    let textos = match idioma {
        pixpin_store::Idioma::Ingles => Some(pixpin_cad::web::TextosWeb {
            todo: "Fit all".into(),
            tema: "Light / dark".into(),
            capas: "Layers".into(),
            grosores: "Lineweights".into(),
            anotaciones: "Annotations".into(),
            todas: "All".into(),
            ninguna: "None".into(),
            sin_soporte: "This browser cannot unpack the drawing: open it with a recent Edge, Chrome, Firefox or Safari.".into(),
            pista: "Wheel: zoom · Drag: pan · Double click: fit · F, B, L".into(),
        }),
        _ => None,
    };
    let o = pixpin_cad::web::OpcionesWeb {
        titulo: p.titulo(),
        origen: m.origen,
        vista: [u[0] as f64, u[1] as f64, u[2] as f64, u[3] as f64],
        unidades: m.unidades,
        cotas: p.cotas.clone(),
        marcos: p.marcos.clone(),
        anotado,
        imagenes,
        claro: true,
        textos,
    };
    Ok(pixpin_cad::web::pagina(&grupos, &o))
}

/// Las imagenes del plano que se encuentran (su ruta tal cual, junto al
/// plano o por su nombre en su carpeta), en PNG o JPEG.
fn imagenes(
    plano: &Path,
    lista: &[pixpin_cad::convertir::ImagenDelPlano],
    origen: [f64; 2],
) -> Vec<pixpin_cad::web::ImagenWeb> {
    let carpeta = plano.parent().unwrap_or(Path::new("."));
    lista
        .iter()
        .filter_map(|i| {
            let r = PathBuf::from(&i.ruta);
            let nombre = r.file_name()?.to_owned();
            let ruta = [r.clone(), carpeta.join(&r), carpeta.join(&nombre)]
                .into_iter()
                .find(|x| x.is_file())?;
            let ext = ruta
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            let (mime, bytes) = match ext.as_str() {
                "png" => ("image/png", std::fs::read(&ruta).ok()?),
                "jpg" | "jpeg" => ("image/jpeg", std::fs::read(&ruta).ok()?),
                _ => (
                    "image/png",
                    pixpin_codec::codificar_png(&pixpin_codec::cargar(&ruta).ok()?).ok()?,
                ),
            };
            let rel = |p: [f64; 2]| [p[0] - origen[0], p[1] - origen[1]];
            Some(pixpin_cad::web::ImagenWeb {
                capa: i.capa.clone(),
                mime,
                bytes,
                esquina: rel(i.esquina),
                u: i.u,
                v: i.v,
            })
        })
        .collect()
}

// ---------------------------------------------------------------- PDF

/// El lado largo de una hoja del PDF: un A3 (puntos).
const LADO_PDF: f64 = 1190.55;
const MARGEN_PDF: f64 = 28.0;

/// El color en papel blanco (como al imprimir): el 7 en negro y lo muy
/// claro oscurecido. `tono`: mezclado con el blanco (los sombreados).
fn en_papel(c: u32, tono: f64) -> [f64; 3] {
    let a = (c >> 24) & 255;
    let mut v = if a == 0 {
        [0.0, 0.0, 0.0]
    } else {
        [
            (c & 255) as f64 / 255.0,
            ((c >> 8) & 255) as f64 / 255.0,
            ((c >> 16) & 255) as f64 / 255.0,
        ]
    };
    let l = 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
    if a != 0 && l > 0.62 {
        let k = 0.5 / l;
        v = v.map(|x| x * k);
    }
    v.map(|x| 1.0 - (1.0 - x) * tono)
}

fn n(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" || s == "-" {
        "0".into()
    } else {
        s.to_string()
    }
}

fn corta(t: &Tramo, caja: &[f64; 4]) -> bool {
    (t.caja[0] as f64) <= caja[2]
        && (t.caja[2] as f64) >= caja[0]
        && (t.caja[1] as f64) <= caja[3]
        && (t.caja[3] as f64) >= caja[1]
}

/// Una hoja del PDF: su tamano y como cae el plano en ella.
struct HojaPdf {
    marco: [f64; 4],
    ancho: f64,
    alto: f64,
    /// Puntos por unidad del plano.
    s: f64,
    /// Donde cae el (0, 0) del plano (respecto al origen del modelo).
    x0: f64,
    y0: f64,
}

impl HojaPdf {
    fn de(marco: [f64; 4]) -> HojaPdf {
        let (mw, mh) = (
            (marco[2] - marco[0]).max(1e-9),
            (marco[3] - marco[1]).max(1e-9),
        );
        let util = LADO_PDF - 2.0 * MARGEN_PDF;
        let s = util / mw.max(mh);
        let (ancho, alto) = (mw * s + 2.0 * MARGEN_PDF, mh * s + 2.0 * MARGEN_PDF);
        HojaPdf {
            marco,
            ancho,
            alto,
            s,
            x0: MARGEN_PDF - marco[0] * s,
            y0: MARGEN_PDF - marco[1] * s,
        }
    }
}

/// El contenido de una hoja: el plano en vectores, las cotas con su medida.
fn contenido(
    h: &HojaPdf,
    grupos: &[Grupo],
    origen: [f64; 2],
    cotas: &[Vec<[f64; 2]>],
    unidades: u32,
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "q {} {} {} {} re W n\n",
        n(MARGEN_PDF),
        n(MARGEN_PDF),
        n(h.ancho - 2.0 * MARGEN_PDF),
        n(h.alto - 2.0 * MARGEN_PDF)
    ));
    let minimo = 0.1 / h.s;
    let color = |s: &mut String, c: [f64; 3], relleno: bool| {
        s.push_str(&format!(
            "{} {} {} {}\n",
            n(c[0]),
            n(c[1]),
            n(c[2]),
            if relleno { "rg" } else { "RG" }
        ));
    };
    // Cinco pasadas, como el visor: rellenos, sombreados, rayas, arcos y letras.
    for pasada in 0..5 {
        for g in grupos.iter().filter(|g| g.encendida) {
            let m = &g.modelo;
            let (ox, oy) = (m.origen[0] - origen[0], m.origen[1] - origen[1]);
            let caja = [
                h.marco[0] - ox,
                h.marco[1] - oy,
                h.marco[2] - ox,
                h.marco[3] - oy,
            ];
            s.push_str(&format!(
                "q {} 0 0 {} {} {} cm\n",
                n(h.s),
                n(h.s),
                n(h.x0 + ox * h.s),
                n(h.y0 + oy * h.s)
            ));
            let mm = if g.grosor > 0 {
                g.grosor as f64 / 100.0 * 72.0 / 25.4
            } else {
                0.35
            };
            s.push_str(&format!("{} w 1 J 1 j\n", n(mm.max(0.25) / h.s)));
            let mut por_color: HashMap<u32, String> = HashMap::new();
            let tri = |v: [(f32, f32); 3], c: u32, por: &mut HashMap<u32, String>| {
                let t = por.entry(c).or_default();
                t.push_str(&format!(
                    "{} {} m {} {} l {} {} l h\n",
                    n(v[0].0 as f64),
                    n(v[0].1 as f64),
                    n(v[1].0 as f64),
                    n(v[1].1 as f64),
                    n(v[2].0 as f64),
                    n(v[2].1 as f64)
                ));
            };
            match pasada {
                0 | 1 => {
                    let (indices, tramos): (&[u32], &[Tramo]) = if pasada == 0 {
                        (&m.triangulos, &m.tramos_triangulos)
                    } else {
                        (&m.triangulos_trama, &m.tramos_trama)
                    };
                    for t in tramos
                        .iter()
                        .filter(|t| t.tamano as f64 >= minimo && corta(t, &caja))
                    {
                        let fin = ((t.desde + t.cuantos) as usize).min(indices.len());
                        for k in indices[t.desde as usize..fin].chunks_exact(3) {
                            let v = |i: u32| -> Option<(f32, f32, u32)> {
                                if pasada == 0 {
                                    m.vertices.get(i as usize).map(|q| (q.x, q.y, q.color))
                                } else {
                                    m.vertices_trama
                                        .get(i as usize)
                                        .map(|q| (q.x, q.y, q.color))
                                }
                            };
                            if let (Some(a), Some(b), Some(c)) = (v(k[0]), v(k[1]), v(k[2])) {
                                tri([(a.0, a.1), (b.0, b.1), (c.0, c.1)], a.2, &mut por_color);
                            }
                        }
                    }
                    for (c, caminos) in &por_color {
                        color(
                            &mut s,
                            en_papel(*c, if pasada == 0 { 1.0 } else { 0.27 }),
                            true,
                        );
                        s.push_str(caminos);
                        s.push_str("f\n");
                    }
                }
                2 => {
                    for t in m
                        .tramos_lineas
                        .iter()
                        .filter(|t| t.tamano as f64 >= minimo * 0.75 && corta(t, &caja))
                    {
                        let fin = ((t.desde + t.cuantos) as usize).min(m.lineas.len());
                        let mut primero = true;
                        let mut actual: Option<u32> = None;
                        let mut ult = (0.0f32, 0.0f32);
                        for &i in &m.lineas[t.desde as usize..fin] {
                            if i == CORTE {
                                primero = true;
                                continue;
                            }
                            let Some(q) = m.vertices.get(i as usize) else {
                                continue;
                            };
                            let camino = por_color.entry(q.color).or_default();
                            if primero || actual != Some(q.color) {
                                if !primero {
                                    camino.push_str(&format!(
                                        "{} {} m\n",
                                        n(ult.0 as f64),
                                        n(ult.1 as f64)
                                    ));
                                    camino.push_str(&format!(
                                        "{} {} l\n",
                                        n(q.x as f64),
                                        n(q.y as f64)
                                    ));
                                } else {
                                    camino.push_str(&format!(
                                        "{} {} m\n",
                                        n(q.x as f64),
                                        n(q.y as f64)
                                    ));
                                }
                                primero = false;
                            } else {
                                camino.push_str(&format!(
                                    "{} {} l\n",
                                    n(q.x as f64),
                                    n(q.y as f64)
                                ));
                            }
                            actual = Some(q.color);
                            ult = (q.x, q.y);
                        }
                    }
                    for (c, caminos) in &por_color {
                        color(&mut s, en_papel(*c, 1.0), false);
                        s.push_str(caminos);
                        s.push_str("S\n");
                    }
                }
                3 => {
                    for t in m
                        .tramos_arcos
                        .iter()
                        .filter(|t| t.tamano as f64 >= minimo * 0.75 && corta(t, &caja))
                    {
                        let fin = ((t.desde + t.cuantos) as usize).min(m.arcos.len());
                        for a in &m.arcos[t.desde as usize..fin] {
                            let pasos = ((a.barrido.abs() as f64 * a.radio as f64 * h.s / 2.0)
                                .ceil() as usize)
                                .clamp(8, 720);
                            let camino = por_color.entry(a.color).or_default();
                            for k in 0..=pasos {
                                let ang = a.inicio + a.barrido * k as f32 / pasos as f32;
                                let (x, y) = (
                                    a.centro[0] + a.radio * ang.cos(),
                                    a.centro[1] + a.radio * ang.sin(),
                                );
                                camino.push_str(&format!(
                                    "{} {} {}\n",
                                    n(x as f64),
                                    n(y as f64),
                                    if k == 0 { "m" } else { "l" }
                                ));
                            }
                        }
                    }
                    for (c, caminos) in &por_color {
                        color(&mut s, en_papel(*c, 1.0), false);
                        s.push_str(caminos);
                        s.push_str("S\n");
                    }
                }
                _ => {
                    let mut rayas: HashMap<u32, String> = HashMap::new();
                    for t in m
                        .tramos_letras
                        .iter()
                        .filter(|t| t.tamano as f64 >= 1.0 / h.s && corta(t, &caja))
                    {
                        let fin = ((t.desde + t.cuantos) as usize).min(m.letras.len());
                        for l in &m.letras[t.desde as usize..fin] {
                            let Some(gl) = m.glifos.get(l.glifo as usize) else {
                                continue;
                            };
                            let de_rayas = gl[1] & GLIFO_DE_RAYAS != 0;
                            let (desde, cuantos) =
                                (gl[0] as usize, (gl[1] & !GLIFO_DE_RAYAS) as usize);
                            let Some(malla) = m.malla_letras.get(desde..desde + cuantos) else {
                                continue;
                            };
                            let pt = |e: &[f32; 2]| {
                                (
                                    l.pos[0] + l.m[0] * e[0] + l.m[1] * e[1],
                                    l.pos[1] + l.m[2] * e[0] + l.m[3] * e[1],
                                )
                            };
                            if de_rayas {
                                let camino = rayas.entry(l.color).or_default();
                                for par in malla.chunks_exact(2) {
                                    let (a, b) = (pt(&par[0]), pt(&par[1]));
                                    camino.push_str(&format!(
                                        "{} {} m {} {} l\n",
                                        n(a.0 as f64),
                                        n(a.1 as f64),
                                        n(b.0 as f64),
                                        n(b.1 as f64)
                                    ));
                                }
                            } else {
                                for k in malla.chunks_exact(3) {
                                    tri([pt(&k[0]), pt(&k[1]), pt(&k[2])], l.color, &mut por_color);
                                }
                            }
                        }
                    }
                    for (c, caminos) in &por_color {
                        color(&mut s, en_papel(*c, 1.0), true);
                        s.push_str(caminos);
                        s.push_str("f\n");
                    }
                    s.push_str(&format!("{} w\n", n(0.3 / h.s)));
                    for (c, caminos) in &rayas {
                        color(&mut s, en_papel(*c, 1.0), false);
                        s.push_str(caminos);
                        s.push_str("S\n");
                    }
                }
            }
            s.push_str("Q\n");
        }
    }
    // Las cotas, en naranja con su medida (en el papel, sin escala).
    let a_papel = |p: [f64; 2]| (h.x0 + p[0] * h.s, h.y0 + p[1] * h.s);
    for cadena in cotas.iter().filter(|c| c.len() >= 2) {
        s.push_str("0.88 0.42 0 RG 0.88 0.42 0 rg 0.8 w\n");
        let mut total = 0.0;
        for par in cadena.windows(2) {
            let (a, b) = (a_papel(par[0]), a_papel(par[1]));
            s.push_str(&format!(
                "{} {} m {} {} l S\n",
                n(a.0),
                n(a.1),
                n(b.0),
                n(b.1)
            ));
            let d = (par[1][0] - par[0][0]).hypot(par[1][1] - par[0][1]);
            total += d;
            let texto = pixpin_cad::ventana::medida(d, unidades);
            s.push_str(&format!(
                "BT /F1 7 Tf {} {} Td ({}) Tj ET\n",
                n((a.0 + b.0) / 2.0 + 2.0),
                n((a.1 + b.1) / 2.0 + 2.0),
                texto_pdf(&texto)
            ));
        }
        if cadena.len() > 2
            && let Some(u) = cadena.last()
        {
            let q = a_papel(*u);
            s.push_str(&format!(
                "BT /F1 7 Tf {} {} Td (Total {}) Tj ET\n",
                n(q.0 + 4.0),
                n(q.1 - 9.0),
                texto_pdf(&pixpin_cad::ventana::medida(total, unidades))
            ));
        }
    }
    s.push_str("Q\n");
    s
}

/// Un texto entre parentesis de PDF (solo lo que entiende Helvetica).
fn texto_pdf(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_ascii() && !c.is_ascii_control())
        .flat_map(|c| match c {
            '(' | ')' | '\\' => vec!['\\', c],
            c => vec![c],
        })
        .collect()
}

/// Escribe el PDF: objetos, paginas, la Helvetica y su tabla.
fn pdf_de(paginas: &[(f64, f64, String)]) -> Vec<u8> {
    let mut objetos: Vec<Vec<u8>> = Vec::new();
    // 1 catalogo, 2 paginas, 3 letra; luego cada pagina y su contenido.
    let mut kids = Vec::new();
    let mut cuerpo: Vec<Vec<u8>> = Vec::new();
    for (i, (w, h, c)) in paginas.iter().enumerate() {
        let (np, nc) = (4 + 2 * i, 5 + 2 * i);
        kids.push(format!("{np} 0 R"));
        cuerpo.push(
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R >> >> /Contents {nc} 0 R >>", n(*w), n(*h))
                .into_bytes(),
        );
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(6));
        let _ = z.write_all(c.as_bytes());
        let datos = z.finish().unwrap_or_default();
        let mut o = format!(
            "<< /Length {} /Filter /FlateDecode >>\nstream\n",
            datos.len()
        )
        .into_bytes();
        o.extend_from_slice(&datos);
        o.extend_from_slice(b"\nendstream");
        cuerpo.push(o);
    }
    objetos.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objetos.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            paginas.len()
        )
        .into_bytes(),
    );
    objetos.push(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec(),
    );
    objetos.extend(cuerpo);
    let mut b = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut sitios = Vec::new();
    for (i, o) in objetos.iter().enumerate() {
        sitios.push(b.len());
        b.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        b.extend_from_slice(o);
        b.extend_from_slice(b"\nendobj\n");
    }
    let xref = b.len();
    b.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objetos.len() + 1).as_bytes());
    for s in sitios {
        b.extend_from_slice(format!("{s:010} 00000 n \n").as_bytes());
    }
    b.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objetos.len() + 1
        )
        .as_bytes(),
    );
    b
}

/// Mueve y escala una orden del motor (`k` y luego `d`).
fn mover(o: &Orden, k: f32, d: (f32, f32)) -> Orden {
    let p = |q: &Punto2| Punto2::nuevo(q.x * k + d.0, q.y * k + d.1);
    let v = |l: &[Punto2]| l.iter().map(p).collect::<Vec<_>>();
    let mut o = o.clone();
    match &mut o {
        Orden::Poligono { puntos, .. } | Orden::Relleno { puntos, .. } => *puntos = v(puntos),
        Orden::Tinta { contorno, .. } => *contorno = v(contorno),
        Orden::Velo { hueco, .. } => *hueco = v(hueco),
        Orden::Polilinea { puntos, grosor, .. } => {
            *puntos = v(puntos);
            *grosor *= k;
        }
        Orden::Texto {
            x,
            y,
            tam,
            ancho_max,
            ..
        } => {
            (*x, *y) = (*x * k + d.0, *y * k + d.1);
            *tam *= k;
            *ancho_max *= k;
        }
        Orden::Rotulo {
            x,
            y,
            tam,
            grosor_halo,
            centro,
            ..
        } => {
            (*x, *y) = (*x * k + d.0, *y * k + d.1);
            *tam *= k;
            *grosor_halo *= k;
            *centro = p(centro);
        }
        Orden::Imagen {
            x, y, ancho, alto, ..
        } => {
            (*x, *y) = (*x * k + d.0, *y * k + d.1);
            *ancho *= k;
            *alto *= k;
        }
    }
    o
}

/// **El PDF del plano**: una hoja por marco (sin marcos, el plano entero),
/// con lo anotado encima en vectores.
pub fn pdf(p: &Plano) -> Result<Vec<u8>> {
    let m = &p.modelo;
    anyhow::ensure!(!m.vacio(), "el plano no tiene nada que dibujar");
    let grupos = p.grupos();
    let marcos: Vec<[f64; 4]> = if p.marcos.is_empty() {
        let u = m.caja_util();
        vec![[u[0] as f64, u[1] as f64, u[2] as f64, u[3] as f64]]
    } else {
        p.marcos.clone()
    };
    let hojas: Vec<HojaPdf> = marcos.into_iter().map(HojaPdf::de).collect();
    let paginas: Vec<(f64, f64, String)> = hojas
        .iter()
        .map(|h| {
            (
                h.ancho,
                h.alto,
                contenido(h, &grupos, m.origen, &p.cotas, m.unidades),
            )
        })
        .collect();
    let base = pdf_de(&paginas);
    let Some((escena, enc)) = p.anotado() else {
        return Ok(base);
    };
    let ordenes = pixpin_motor2d::pintado::ordenes_de_escena(&escena);
    // De la capa (y hacia abajo) a las unidades de cada hoja (cero arriba a
    // la izquierda, `ANCHO_EN_UNIDADES` de ancho).
    let tinta = |i: usize| -> Option<Vec<Orden>> {
        let h = hojas.get(i)?;
        let kk = pixpin_pdf::con_anotaciones::ANCHO_EN_UNIDADES as f64 / h.ancho;
        let a = h.x0 + enc.cero_x * h.s;
        let b = h.alto - h.y0 - enc.cero_y * h.s;
        let k = (kk * enc.u * h.s) as f32;
        Some(
            ordenes
                .iter()
                .map(|o| mover(o, k, ((kk * a) as f32, (kk * b) as f32)))
                .collect(),
        )
    };
    let segoe = pixpin_pdf::letra::del_sistema("Segoe UI");
    let con = pixpin_pdf::con_anotaciones::hacer(
        &base,
        &pixpin_pdf::con_anotaciones::Anotaciones {
            tinta: &tinta,
            izquierda: 0.0,
            derecha: 0.0,
            marcadores: &[],
            imagenes: &|_| None,
            letra: segoe.as_ref(),
        },
    );
    Ok(con.unwrap_or(base))
}

// ---------------------------------------------------------------- PNG

/// El lado largo de la imagen del plano, en pixeles.
const LADO_PNG: f32 = 3200.0;

/// **El plano como imagen**, pintado como al imprimir, con lo anotado.
pub fn imagen(p: &Plano) -> Result<pixpin_codec::ImagenRgba> {
    let m = &p.modelo;
    anyhow::ensure!(!m.vacio(), "el plano no tiene nada que dibujar");
    let u = m.caja_util();
    let marco = [u[0] as f64, u[1] as f64, u[2] as f64, u[3] as f64];
    let (mw, mh) = (
        (marco[2] - marco[0]).max(1e-9) as f32,
        (marco[3] - marco[1]).max(1e-9) as f32,
    );
    let k = LADO_PNG / mw.max(mh);
    let margen = 24.0;
    let (w, h) = (
        (mw * k + 2.0 * margen).round() as u32,
        (mh * k + 2.0 * margen).round() as u32,
    );
    let d = pixpin_capture::Dispositivo::nuevo().context("sin dispositivo para exportar")?;
    let motor = pixpin_render::MotorRender::nuevo(d.d3d()).context("sin motor para exportar")?;
    let destino = pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
        .context("sin textura para exportar")?;
    let anotado = anotar::ParaImprimir::leer(&p.capa, m);
    motor
        .dibujar(&destino.destino, |pin| {
            pin.limpiar(pixpin_render::Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            });
            super::imprimir::dibujar(
                pin,
                m,
                &marco,
                &p.cotas,
                anotado.as_ref().map(|a| (a, 1.0)),
                (
                    margen,
                    margen,
                    w as f32 - 2.0 * margen,
                    h as f32 - 2.0 * margen,
                ),
                None,
            );
        })
        .context("no se pudo pintar el plano")?;
    let (ancho, alto, pixeles) = destino
        .leer_rgba()
        .context("no se pudo leer el plano pintado")?;
    Ok(pixpin_codec::ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
}

// ---------------------------------------------------------------- todo junto

fn es_dwg(ruta: &Path) -> bool {
    pixpin_docs::extension(&pixpin_docs::nombre(ruta)) == "dwg"
}

/// **Lo que el plano ofrece al «Exportar» comun**: HTML, PDF, PNG, JPG y
/// DXF (de un DWG) con su funcion cada uno; original y `.pixpin` los pone
/// el componente.
pub fn exportable(
    p: Plano,
    idioma: pixpin_store::Idioma,
) -> crate::compartir::exportable::Exportable {
    use crate::compartir::exportable::{Exportable, Propio};
    let base = crate::compartir::nombre_de_fichero(&p.titulo());
    let mut x = Exportable::nuevo(p.titulo());
    x.raiz = Some(p.raiz.clone());
    x.original = Some(p.ruta.clone());
    x.anotado = vec![p.capa.tinta.clone(), p.capa.hoja.clone()];
    let (p1, b1) = (p.clone(), base.clone());
    x.propios.push(Propio::nuevo(
        crate::compartir::WEB,
        "compartir-web",
        move |c| {
            let r = c.join(format!("{b1}.html"));
            std::fs::write(&r, html(&p1, idioma)?)?;
            Ok(vec![r])
        },
    ));
    let (p2, b2) = (p.clone(), base.clone());
    x.propios.push(Propio::nuevo(
        crate::compartir::PDF,
        "compartir-pdf",
        move |c| {
            let r = c.join(format!("{b2}.pdf"));
            std::fs::write(&r, pdf(&p2)?)?;
            Ok(vec![r])
        },
    ));
    for (id, clave) in [
        (crate::compartir::PNG, "compartir-png"),
        (crate::compartir::JPG, "compartir-jpg"),
    ] {
        let (p3, b3) = (p.clone(), base.clone());
        x.propios.push(Propio::nuevo(id, clave, move |c| {
            let img = imagen(&p3)?;
            let r = c.join(format!("{b3}.{id}"));
            let bytes = if id == crate::compartir::PNG {
                pixpin_codec::codificar_png(&img)?
            } else {
                pixpin_codec::imagen::codificar_jpg(&img, 90)?
            };
            std::fs::write(&r, bytes)?;
            Ok(vec![r])
        }));
    }
    if es_dwg(&p.ruta) {
        let (p4, b4) = (p.clone(), base);
        x.propios
            .push(Propio::nuevo("dxf", "compartir-dxf", move |c| {
                let r = c.join(format!("{b4}.dxf"));
                aparte("dxf", &p4.ruta, &r, None)?;
                Ok(vec![r])
            }));
    }
    x
}

#[cfg(test)]
mod pruebas;
