//! **La ventana de las figuras** (22-oct-2026): el boton Figuras del lienzo
//! abre una ventana aparte, grande, en vez del menu con submenus.
//!
//! El usuario: «quiero la parte de las figuras aparezca en una ventana
//! separada de forma que lo pueda ver de forma mas grande y con una mejor
//! UI», y «que siga la linea de diseño de las demas ventanas». Por eso va
//! con las piezas del v2 (`crate::v2`, la cabecera comun de `crate::cabecera`,
//! los botones y cajas de `lecciones::ui`), como la galeria de capturas:
//!
//! - **a la izquierda**, las categorias con cuantas tiene cada una y, debajo,
//!   lo demas que traia el menu (grafica, tablas, instrumentos, coordenadas);
//! - **en medio**, las miniaturas grandes con su nombre, por categorias, y
//!   el buscador arriba;
//! - **a la derecha**, la elegida en grande: si es una plantilla con numeros,
//!   con su intervalo («del 1 al 40, de 10 en 10») para insertarla ya
//!   alargada; las propias, con renombrar y quitar.
//!
//! Doble clic o «Insertar» la pone en medio de lo que se ve y la ventana se
//! cierra. Mientras esta abierta, el lienzo espera (como un dialogo): vive en
//! el hilo del editor y usa su mismo motor de dibujo.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use pixpin_motor2d::biblioteca::FiguraGuardada;
use pixpin_motor2d::figuras_de_serie::{self as serie, Intervalo};
use pixpin_render::icono::material as mi;
use pixpin_render::{Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use super::Eleccion;
use crate::lecciones::ui::{Campo, boton_v2};
use crate::v2::color::blanco;
use crate::v2::geom::{cortar, encoger};
use crate::v2::{self as d};
use crate::ventanita::{Botones, centrado, dentro};

/// Lo que se pidio en la ventana.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::ventana_editor) enum Pedido {
    /// Estampar la figura `i` de la biblioteca, tal cual.
    Estampar(usize),
    /// Las hojas de una plantilla alargada, para ponerlas en fila.
    Hojas(Vec<FiguraGuardada>),
    /// Lo demas del menu de siempre (la grafica, una tabla...).
    Herramienta(Eleccion),
}

// ------------------------------------------------------------ las medidas

/// En pixeles logicos.
const ANCHO_LATERAL: f32 = 236.0;
const ANCHO_PANEL: f32 = 340.0;
const CELDA: f32 = 184.0;
/// La miniatura dentro de la celda (el nombre va debajo).
const MINI_ALTO: f32 = 196.0;
const NOMBRE_ALTO: f32 = 40.0;
const TITULO_ALTO: f32 = 46.0;
const FILA_LATERAL: f32 = 38.0;
/// El lado al que se pintan las miniaturas (pixeles fisicos, a escala 1).
const LADO_MINI: f32 = 200.0;
const DOBLE_CLIC: Duration = Duration::from_millis(450);

/// Lo que se puede pulsar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accion {
    Mover,
    Cerrar,
    Buscar,
    LimpiarBusqueda,
    Categoria(usize),
    Herramienta(u32),
    Celda(usize),
    Insertar,
    Campo(Dato),
    Quitar,
    Renombrar,
    GuardarSeleccion,
}

/// Los campos del panel de la derecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Dato {
    Desde,
    Hasta,
    DeA,
    SemanasDesde,
    SemanasHasta,
    Mes,
    Meses,
    Nombre,
}

// ----------------------------------------------------- lo que se decide

/// Las categorias de la barra de la izquierda: «Todas», las de serie en su
/// orden, las del usuario y «Mis figuras» (las que no llevan categoria).
fn categorias(figuras: &[FiguraGuardada], mis: &str) -> Vec<String> {
    let mut v: Vec<String> = serie::CATEGORIAS.iter().map(|c| c.to_string()).collect();
    for f in figuras {
        if let Some((c, _)) = serie::categoria_de(&f.nombre)
            && !v.iter().any(|o| o == c)
        {
            v.push(c.to_string());
        }
    }
    v.push(mis.to_string());
    v
}

/// La categoria de una figura (las sin categoria, en «Mis figuras») y su
/// nombre sin ella.
fn de_que_es<'a>(f: &'a FiguraGuardada, mis: &'a str) -> (&'a str, &'a str) {
    serie::categoria_de(&f.nombre).unwrap_or((mis, f.nombre.as_str()))
}

/// Sin tildes ni mayusculas, para buscar.
fn plegar(s: &str) -> String {
    pixpin_ia::acciones::normalizar(s)
}

/// **Las que se ven**: de la categoria elegida (`None`, todas) y que
/// contengan todas las palabras buscadas (en el nombre o la categoria), en
/// el orden de las categorias.
fn visibles(
    figuras: &[FiguraGuardada],
    cats: &[String],
    cat: Option<usize>,
    consulta: &str,
    mis: &str,
) -> Vec<usize> {
    let palabras: Vec<String> = plegar(consulta)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let mut v = Vec::new();
    for (k, c) in cats.iter().enumerate() {
        if cat.is_some_and(|x| x != k) {
            continue;
        }
        for (i, f) in figuras.iter().enumerate() {
            let (fc, _) = de_que_es(f, mis);
            if fc != c {
                continue;
            }
            let donde = plegar(&f.nombre);
            if palabras.iter().all(|p| donde.contains(p.as_str())) {
                v.push(i);
            }
        }
    }
    v
}

/// Una pieza de la rejilla: el titulo de una categoria o una figura.
#[derive(Debug, Clone, PartialEq)]
enum Pieza {
    Titulo(String),
    Celda(usize),
}

/// **La rejilla puesta**: cada pieza con su caja (en coordenadas de la
/// rejilla, sin desplazar), cuantas columnas caben y lo que mide de alto.
#[derive(Debug, Clone, Default)]
struct Disposicion {
    piezas: Vec<(RectF, Pieza)>,
    columnas: usize,
    alto: f32,
}

fn disponer(
    figuras: &[FiguraGuardada],
    lista: &[usize],
    con_titulos: bool,
    ancho: f32,
    s: f32,
    mis: &str,
) -> Disposicion {
    let (m, hueco) = (d::MARGEN * 1.25 * s, d::HUECO_GRANDE * 1.4 * s);
    let celda_w = CELDA * s;
    let celda_h = (MINI_ALTO + NOMBRE_ALTO) * s;
    let columnas = (((ancho - 2.0 * m + hueco) / (celda_w + hueco)).floor() as usize).max(1);
    // Las columnas se reparten el ancho: sin un hueco grande a la derecha.
    let celda_w =
        ((ancho - 2.0 * m - hueco * (columnas - 1) as f32) / columnas as f32).max(celda_w * 0.6);
    let mut piezas = Vec::new();
    let mut y = m;
    let mut col = 0;
    let mut cat_anterior: Option<&str> = None;
    for &i in lista {
        let (c, _) = de_que_es(&figuras[i], mis);
        if con_titulos && cat_anterior != Some(c) {
            if col > 0 {
                y += celda_h + hueco;
                col = 0;
            }
            if cat_anterior.is_some() {
                y += hueco;
            }
            piezas.push((
                RectF {
                    x: m,
                    y,
                    ancho: ancho - 2.0 * m,
                    alto: TITULO_ALTO * s,
                },
                Pieza::Titulo(c.to_string()),
            ));
            y += TITULO_ALTO * s;
            cat_anterior = Some(c);
        }
        let x = m + (celda_w + hueco) * col as f32;
        piezas.push((
            RectF {
                x,
                y,
                ancho: celda_w,
                alto: celda_h,
            },
            Pieza::Celda(i),
        ));
        col += 1;
        if col == columnas {
            col = 0;
            y += celda_h + hueco;
        }
    }
    if col > 0 {
        y += celda_h + hueco;
    }
    Disposicion {
        piezas,
        columnas,
        alto: y + m,
    }
}

/// Los campos que lleva el panel para esa plantilla.
fn campos_de(clave: &str) -> &'static [Dato] {
    use Dato::*;
    match clave {
        "planificador-mensual" => &[Mes, Meses],
        "habitos" => &[Desde, Hasta, DeA, Mes, Meses],
        "cronograma" => &[Desde, Hasta, DeA, SemanasDesde, SemanasHasta],
        _ => &[Desde, Hasta, DeA],
    }
}

/// **El intervalo de los campos** (`None` si estan todos vacios: la de
/// serie, tal cual). Error en palabras si algo no es un numero.
fn intervalo_de(campos: &HashMap<Dato, String>) -> Result<Option<Intervalo>, String> {
    let n = |c: Dato| -> Result<Option<u32>, String> {
        match campos.get(&c).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => Ok(None),
            Some(s) => s
                .parse()
                .map(Some)
                .map_err(|_| format!("«{s}» no es un número")),
        }
    };
    let mes = match campos
        .get(&Dato::Mes)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        None => None,
        Some(s) => Some(
            pixpin_ia::figuras::mes_de(s)
                .ok_or_else(|| format!("«{s}»: escribe el mes como 2026-11"))?,
        ),
    };
    let semanas = match (n(Dato::SemanasDesde)?, n(Dato::SemanasHasta)?) {
        (None, None) => None,
        (a, b) => Some((a.unwrap_or(1), b.or(a).unwrap_or(12))),
    };
    let iv = Intervalo {
        desde: n(Dato::Desde)?,
        hasta: n(Dato::Hasta)?,
        de_a: n(Dato::DeA)?,
        semanas,
        mes,
        meses: n(Dato::Meses)?,
    };
    Ok((iv != Intervalo::default()).then_some(iv))
}

// ------------------------------------------------------------ el estado

struct Estado {
    figuras: Vec<FiguraGuardada>,
    /// Si cambio la biblioteca (renombrar, quitar): se guarda al cerrar.
    cambiada: bool,
    cats: Vec<String>,
    mis: String,
    /// `None`: todas.
    cat: Option<usize>,
    consulta: Campo,
    /// Donde va lo que se teclea: el buscador o un campo del panel.
    foco: Option<Dato>,
    buscando: bool,
    campos: HashMap<Dato, Campo>,
    elegida: Option<usize>,
    lista: Vec<usize>,
    disp: Disposicion,
    scroll: f32,
    botones: Botones<Accion>,
    moviendo: Option<(pixpin_geom::Punto, pixpin_geom::Rect)>,
    ultimo_clic: Option<(Accion, Instant)>,
    /// Lo que dira el panel bajo el intervalo («4 hojas» o el error).
    previa: Option<Result<String, String>>,
    confirmar_quitar: bool,
    minis: HashMap<usize, (ID2D1Bitmap1, u32, u32)>,
    pedido: Option<Pedido>,
    herramientas: Vec<(u32, String)>,
    hay_seleccion: bool,
}

impl Estado {
    fn nuevo(
        figuras: Vec<FiguraGuardada>,
        herramientas: Vec<(u32, String)>,
        hay_seleccion: bool,
        mis: String,
    ) -> Estado {
        Estado {
            figuras,
            cambiada: false,
            cats: Vec::new(),
            mis,
            cat: None,
            consulta: Campo::default(),
            foco: None,
            buscando: false,
            campos: HashMap::new(),
            elegida: None,
            lista: Vec::new(),
            disp: Disposicion::default(),
            scroll: 0.0,
            botones: Botones::default(),
            moviendo: None,
            ultimo_clic: None,
            previa: None,
            confirmar_quitar: false,
            minis: HashMap::new(),
            pedido: None,
            herramientas,
            hay_seleccion,
        }
    }

    fn rehacer_lista(&mut self, ancho_rejilla: f32, s: f32) {
        self.cats = categorias(&self.figuras, &self.mis);
        let consulta = self.consulta.texto().to_string();
        self.lista = visibles(&self.figuras, &self.cats, self.cat, &consulta, &self.mis);
        let con_titulos = self.cat.is_none() && consulta.trim().is_empty();
        self.disp = disponer(
            &self.figuras,
            &self.lista,
            con_titulos,
            ancho_rejilla,
            s,
            &self.mis,
        );
        if self.elegida.is_none_or(|i| !self.lista.contains(&i)) {
            self.elegir(self.lista.first().copied());
        }
    }

    fn elegir(&mut self, i: Option<usize>) {
        if self.elegida == i {
            return;
        }
        self.elegida = i;
        self.campos.clear();
        self.confirmar_quitar = false;
        if self.foco != Some(Dato::Nombre) || i.is_none() {
            self.foco = None;
        }
        if let Some(f) = i.and_then(|i| self.figuras.get(i)) {
            self.campos.insert(Dato::Nombre, Campo::con(&f.nombre));
        }
        self.previa = None;
    }

    fn texto_de(&self, c: Dato) -> String {
        self.campos
            .get(&c)
            .map(|x| x.texto().to_string())
            .unwrap_or_default()
    }

    /// La clave de serie de la elegida, si se alarga.
    fn alargable(&self) -> Option<String> {
        let f = self.figuras.get(self.elegida?)?;
        serie::clave_de(f)
            .filter(|c| serie::se_alarga(c))
            .map(str::to_string)
    }

    fn rehacer_previa(&mut self, t: &Catalogo) {
        let Some(clave) = self.alargable() else {
            self.previa = None;
            return;
        };
        let textos: HashMap<Dato, String> = campos_de(&clave)
            .iter()
            .map(|c| (*c, self.texto_de(*c)))
            .collect();
        self.previa = match intervalo_de(&textos) {
            Ok(None) => None,
            Ok(Some(iv)) => Some(serie::alargar(&clave, &iv).map(|h| {
                if h.len() == 1 {
                    t.t("figuras-ventana-una-hoja")
                } else {
                    let mut a = fluent_bundle::FluentArgs::new();
                    a.set("n", h.len());
                    t.t_args("figuras-ventana-hojas", &a)
                }
            })),
            Err(e) => Some(Err(e)),
        };
    }

    /// Lo que hace «Insertar» con la elegida.
    fn insertar(&mut self) {
        let Some(i) = self.elegida else { return };
        if let Some(clave) = self.alargable() {
            let textos: HashMap<Dato, String> = campos_de(&clave)
                .iter()
                .map(|c| (*c, self.texto_de(*c)))
                .collect();
            match intervalo_de(&textos) {
                Ok(Some(iv)) => match serie::alargar(&clave, &iv) {
                    Ok(h) => self.pedido = Some(Pedido::Hojas(h)),
                    Err(e) => self.previa = Some(Err(e)),
                },
                Ok(None) => self.pedido = Some(Pedido::Estampar(i)),
                Err(e) => self.previa = Some(Err(e)),
            }
        } else {
            self.pedido = Some(Pedido::Estampar(i));
        }
    }

    fn renombrar(&mut self) {
        let Some(i) = self.elegida else { return };
        let nuevo = self.texto_de(Dato::Nombre).trim().to_string();
        if !nuevo.is_empty() && self.figuras[i].nombre != nuevo {
            self.figuras[i].nombre = nuevo;
            self.cambiada = true;
            self.minis.remove(&i);
        }
        self.foco = None;
    }

    fn quitar(&mut self) {
        let Some(i) = self.elegida else { return };
        if !self.confirmar_quitar {
            self.confirmar_quitar = true;
            return;
        }
        self.figuras.remove(i);
        self.cambiada = true;
        // Los indices de detras se corren uno.
        self.minis = std::mem::take(&mut self.minis)
            .into_iter()
            .filter(|(k, _)| *k != i)
            .map(|(k, v)| (if k > i { k - 1 } else { k }, v))
            .collect();
        self.elegida = None;
        self.confirmar_quitar = false;
    }

    /// Mueve la elegida por la rejilla (flechas).
    fn mover_eleccion(&mut self, paso: isize) {
        let Some(pos) = self
            .elegida
            .and_then(|i| self.lista.iter().position(|x| *x == i))
        else {
            self.elegir(self.lista.first().copied());
            return;
        };
        let nueva = (pos as isize + paso).clamp(0, self.lista.len() as isize - 1) as usize;
        self.elegir(Some(self.lista[nueva]));
    }
}

// ---------------------------------------------------------- las miniaturas

thread_local! {
    /// Las miniaturas pintadas, por figura (id, nombre, piezas) y lado.
    static PINTADAS: std::cell::RefCell<HashMap<(String, String, usize, u32), pixpin_codec::ImagenRgba>> = Default::default();
}

/// Las de todas, de la cache o pintadas de una vez las que falten.
fn miniaturas(figuras: &[FiguraGuardada], lado: u32) -> Vec<Option<pixpin_codec::ImagenRgba>> {
    let clave = |f: &FiguraGuardada| (f.id.clone(), f.nombre.clone(), f.elementos.len(), lado);
    let faltan: Vec<FiguraGuardada> = PINTADAS.with_borrow(|m| {
        figuras
            .iter()
            .filter(|f| !m.contains_key(&clave(f)))
            .cloned()
            .collect()
    });
    if !faltan.is_empty() {
        match crate::ventana_editor::exportar::miniaturas(&faltan, lado) {
            Ok(v) => PINTADAS.with_borrow_mut(|m| {
                for (f, img) in faltan.iter().zip(v) {
                    if let Some(img) = img {
                        m.insert(clave(f), img);
                    }
                }
            }),
            Err(e) => tracing::warn!(?e, "figuras: sin miniaturas"),
        }
    }
    PINTADAS.with_borrow(|m| figuras.iter().map(|f| m.get(&clave(f)).cloned()).collect())
}

fn subir_miniaturas(e: &mut Estado, motor: &MotorRender, lado: u32) {
    let faltan: Vec<usize> = (0..e.figuras.len())
        .filter(|i| !e.minis.contains_key(i))
        .collect();
    if faltan.is_empty() {
        return;
    }
    let imgs = miniaturas(&e.figuras, lado);
    for i in faltan {
        if let Some(Some(img)) = imgs.get(i)
            && let Ok(b) = motor.bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
        {
            e.minis.insert(i, (b, img.ancho, img.alto));
        }
    }
}

// --------------------------------------------------------------- el bucle

/// **Abre la ventana y espera** a que se elija algo o se cierre. Devuelve la
/// biblioteca como quedo (si se renombro o quito algo, ya guardada) y lo
/// pedido.
#[allow(clippy::too_many_arguments)]
pub(in crate::ventana_editor) fn elegir(
    dispositivo: &pixpin_capture::Dispositivo,
    motor: &MotorRender,
    escala_por_cien: u32,
    t: &Catalogo,
    figuras: Vec<FiguraGuardada>,
    herramientas: Vec<(u32, String)>,
    hay_seleccion: bool,
    guardar: impl Fn(&[FiguraGuardada]),
) -> (Vec<FiguraGuardada>, Option<Pedido>) {
    let s = escala_por_cien as f32 / 100.0;
    let monitor = pixpin_shell::pantalla_de::monitor_de_la_ventana_activa();
    let area = monitor
        .or_else(|| {
            pixpin_capture::enumerar_monitores()
                .ok()
                .and_then(|m| m.principal().map(|p| p.area_trabajo))
        })
        .unwrap_or(pixpin_geom::Rect {
            x: 0,
            y: 0,
            ancho: 1600,
            alto: 900,
        });
    // Grande: casi toda la pantalla de trabajo, hasta 1440 x 900 logicos.
    let ancho = ((area.ancho as f32 / s * 0.9) as u32).min(1440);
    let alto = ((area.alto as f32 / s * 0.9) as u32).min(900);
    let mut marco = centrado(area, ancho, alto, escala_por_cien);
    let mut e = Estado::nuevo(
        figuras,
        herramientas,
        hay_seleccion,
        t.t("figuras-mis-figuras"),
    );
    // Solo para probar: `PIXPIN_MENU_FIGURAS_AL_ABRIR=<categoria>` abre la
    // ventana ya en esa categoria.
    let de_prueba = std::env::var("PIXPIN_MENU_FIGURAS_AL_ABRIR").ok();
    let resultado = (|| -> anyhow::Result<()> {
        let mut ventana = VentanaOverlay::nueva_normal(marco, &t.t("figuras-ventana-titulo"))?;
        let superficie = Superficie::nueva(
            motor,
            dispositivo.d3d(),
            ventana.handle(),
            marco.ancho,
            marco.alto,
        )?;
        ventana.mostrar();
        ventana.enfocar();
        let (w, h) = (marco.ancho as f32, marco.alto as f32);
        let ancho_rejilla = w - (ANCHO_LATERAL + ANCHO_PANEL) * s;
        e.rehacer_lista(ancho_rejilla, s);
        if let Some(c) = de_prueba
            .as_deref()
            .and_then(|c| e.cats.iter().position(|x| x == c))
        {
            e.cat = Some(c);
            e.elegida = None;
            e.rehacer_lista(ancho_rejilla, s);
        }
        let lado = (LADO_MINI * s).round() as u32;
        let mut vivo = true;
        let mut pintar = true;
        let alto_rejilla = h - crate::cabecera::ALTO * s;
        while vivo && e.pedido.is_none() {
            pixpin_shell::overlay::bombear_pendientes();
            for (hw, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
                // Las del lienzo, mientras tanto, no: esta ventana es un
                // dialogo.
                if hw != ventana.handle() {
                    continue;
                }
                pintar = true;
                let local = |p: pixpin_geom::Punto, m: pixpin_geom::Rect| {
                    ((p.x - m.x) as f32, (p.y - m.y) as f32)
                };
                match ev {
                    EventoOverlay::Cerrar => vivo = false,
                    EventoOverlay::RatonMovido(p) => {
                        if let Some((desde, origen)) = e.moviendo {
                            marco = pixpin_geom::Rect {
                                x: origen.x + (p.x - desde.x),
                                y: origen.y + (p.y - desde.y),
                                ..origen
                            };
                            ventana.mover(marco);
                        }
                        e.botones.raton = local(p, marco);
                    }
                    EventoOverlay::BotonPulsado(p) => {
                        e.botones.raton = local(p, marco);
                        clic(&mut e, p, marco, &ventana, &mut vivo, ancho_rejilla, s, t);
                    }
                    EventoOverlay::BotonSoltado(_) => {
                        if e.moviendo.take().is_some() {
                            ventana.soltar_raton();
                        }
                    }
                    EventoOverlay::Rueda(m) => {
                        let (x, _) = e.botones.raton;
                        if x > ANCHO_LATERAL * s && x < w - ANCHO_PANEL * s {
                            e.scroll = (e.scroll
                                - m as f32 / 120.0 * (MINI_ALTO + NOMBRE_ALTO) * s * 0.5)
                                .clamp(0.0, (e.disp.alto - alto_rejilla).max(0.0));
                        }
                    }
                    EventoOverlay::Tecla {
                        vk, ctrl, shift, ..
                    } => tecla(&mut e, vk, ctrl, shift, &mut vivo, ancho_rejilla, s, t),
                    EventoOverlay::Caracter(c) => caracter(&mut e, c, ancho_rejilla, s, t),
                    _ => {}
                }
            }
            if !vivo || e.pedido.is_some() {
                break;
            }
            if pintar {
                subir_miniaturas(&mut e, motor, lado);
                e.scroll = e.scroll.clamp(0.0, (e.disp.alto - alto_rejilla).max(0.0));
                if let Ok(dd) = superficie.empezar(motor) {
                    let _ = motor.dibujar(&dd, |p: &Pintor| pintar_todo(&mut e, p, w, h, s, t));
                    let _ = superficie.presentar();
                }
                pintar = false;
            }
            pixpin_shell::overlay::esperar_eventos(Some(if e.foco.is_some() || e.buscando {
                250
            } else {
                1000
            }));
        }
        Ok(())
    })();
    if let Err(err) = resultado {
        tracing::warn!(?err, "no se pudo abrir la ventana de las figuras");
    }
    if e.cambiada {
        guardar(&e.figuras);
    }
    (e.figuras, e.pedido)
}

#[allow(clippy::too_many_arguments)]
fn clic(
    e: &mut Estado,
    p: pixpin_geom::Punto,
    marco: pixpin_geom::Rect,
    ventana: &VentanaOverlay,
    vivo: &mut bool,
    ancho_rejilla: f32,
    s: f32,
    t: &Catalogo,
) {
    let Some(a) = e.botones.bajo_el_raton() else {
        e.buscando = false;
        e.foco = None;
        return;
    };
    let doble = e
        .ultimo_clic
        .is_some_and(|(b, cuando)| b == a && cuando.elapsed() < DOBLE_CLIC);
    e.ultimo_clic = Some((a, Instant::now()));
    if !matches!(a, Accion::Buscar | Accion::LimpiarBusqueda) {
        e.buscando = false;
    }
    if !matches!(a, Accion::Campo(_)) {
        e.foco = None;
    }
    if a != Accion::Quitar {
        e.confirmar_quitar = false;
    }
    match a {
        Accion::Mover => {
            e.moviendo = Some((p, marco));
            ventana.capturar_raton();
        }
        Accion::Cerrar => *vivo = false,
        Accion::Buscar => e.buscando = true,
        Accion::LimpiarBusqueda => {
            e.consulta.poner("");
            e.rehacer_lista(ancho_rejilla, s);
        }
        Accion::Categoria(k) => {
            e.cat = (k > 0).then(|| k - 1);
            e.scroll = 0.0;
            e.rehacer_lista(ancho_rejilla, s);
        }
        Accion::Herramienta(id) => {
            if let Some(el) = super::eleccion_de(id, e.figuras.len()) {
                e.pedido = Some(Pedido::Herramienta(el));
            }
        }
        Accion::GuardarSeleccion => {
            e.pedido = Some(Pedido::Herramienta(Eleccion::GuardarSeleccion))
        }
        Accion::Celda(i) => {
            e.elegir(Some(i));
            if doble {
                e.insertar();
            }
        }
        Accion::Insertar => e.insertar(),
        Accion::Campo(c) => e.foco = Some(c),
        Accion::Quitar => {
            e.quitar();
            e.rehacer_lista(ancho_rejilla, s);
        }
        Accion::Renombrar => {
            e.renombrar();
            e.rehacer_lista(ancho_rejilla, s);
        }
    }
    e.rehacer_previa(t);
}

const VK_ESCAPE: u32 = 0x1B;
const VK_ENTRAR: u32 = 0x0D;
const VK_IZQ: u32 = 0x25;
const VK_ARRIBA: u32 = 0x26;
const VK_DER: u32 = 0x27;
const VK_ABAJO: u32 = 0x28;
const VK_TAB: u32 = 0x09;

#[allow(clippy::too_many_arguments)]
fn tecla(
    e: &mut Estado,
    vk: u32,
    ctrl: bool,
    shift: bool,
    vivo: &mut bool,
    ancho_rejilla: f32,
    s: f32,
    t: &Catalogo,
) {
    match vk {
        VK_ESCAPE if e.foco.is_some() || e.buscando => {
            e.foco = None;
            e.buscando = false;
        }
        VK_ESCAPE => *vivo = false,
        VK_ENTRAR if e.foco == Some(Dato::Nombre) => {
            e.renombrar();
            e.rehacer_lista(ancho_rejilla, s);
        }
        VK_ENTRAR => e.insertar(),
        // Ctrl+F: al buscador.
        0x46 if ctrl => {
            e.foco = None;
            e.buscando = true;
        }
        VK_TAB if e.foco.is_some() => {
            // Al siguiente campo del panel.
            if let (Some(clave), Some(f)) = (e.alargable(), e.foco) {
                let cs = campos_de(&clave);
                if let Some(k) = cs.iter().position(|c| *c == f) {
                    let n = cs.len() as isize;
                    let k = (k as isize + if shift { -1 } else { 1 }).rem_euclid(n) as usize;
                    e.foco = Some(cs[k]);
                }
            }
        }
        _ => {
            if let Some(f) = e.foco {
                if let Some(c) = e.campos.get_mut(&f) {
                    c.tecla(vk, ctrl, shift, false);
                }
            } else if e.buscando {
                if e.consulta.tecla(vk, ctrl, shift, false) {
                    e.scroll = 0.0;
                    e.rehacer_lista(ancho_rejilla, s);
                }
            } else {
                let cols = e.disp.columnas.max(1) as isize;
                match vk {
                    VK_IZQ => e.mover_eleccion(-1),
                    VK_DER => e.mover_eleccion(1),
                    VK_ARRIBA => e.mover_eleccion(-cols),
                    VK_ABAJO => e.mover_eleccion(cols),
                    _ => {}
                }
            }
        }
    }
    e.rehacer_previa(t);
}

fn caracter(e: &mut Estado, c: char, ancho_rejilla: f32, s: f32, t: &Catalogo) {
    if c.is_control() {
        return;
    }
    if let Some(f) = e.foco {
        // En los de numeros, solo numeros (y el guion del mes).
        let vale = matches!(f, Dato::Nombre)
            || c.is_ascii_digit()
            || (f == Dato::Mes && (c == '-' || c == '/' || c.is_alphabetic() || c == ' '));
        if vale {
            e.campos.entry(f).or_default().letra(c);
            e.rehacer_previa(t);
        }
        return;
    }
    // Escribir sin foco busca, como en la galeria.
    e.buscando = true;
    e.consulta.letra(c);
    e.scroll = 0.0;
    e.rehacer_lista(ancho_rejilla, s);
}

// --------------------------------------------------------------- pintar

fn pintar_todo(e: &mut Estado, p: &Pintor, w: f32, h: f32, s: f32, t: &Catalogo) {
    p.limpiar(d::FONDO);
    e.botones.vaciar();
    let cab = crate::cabecera::ALTO * s;
    let rejilla = RectF {
        x: ANCHO_LATERAL * s,
        y: cab,
        ancho: w - (ANCHO_LATERAL + ANCHO_PANEL) * s,
        alto: h - cab,
    };
    pintar_rejilla(e, p, rejilla, s, t);
    pintar_lateral(
        e,
        p,
        RectF {
            x: 0.0,
            y: cab,
            ancho: ANCHO_LATERAL * s,
            alto: h - cab,
        },
        s,
        t,
    );
    pintar_panel(
        e,
        p,
        RectF {
            x: w - ANCHO_PANEL * s,
            y: cab,
            ancho: ANCHO_PANEL * s,
            alto: h - cab,
        },
        s,
        t,
    );
    pintar_cabecera(e, p, w, s, t);
}

fn pintar_cabecera(e: &mut Estado, p: &Pintor, w: f32, s: f32, t: &Catalogo) {
    let titulo = t.t("figuras-ventana-titulo");
    let mut a = fluent_bundle::FluentArgs::new();
    a.set("n", e.figuras.len());
    let subtitulo = t.t_args("figuras-ventana-subtitulo", &a);
    let pista = t.t("figuras-ventana-buscar");
    let guardar = t.t("figuras-ventana-guardar-seleccion");
    let texto = e.consulta.texto().to_string();
    let mut botones = Vec::new();
    if e.hay_seleccion {
        botones.push(crate::cabecera::Boton {
            icono: Some(&mi::LIBRARY_ADD),
            rotulo: &guardar,
            chapa: None,
            activo: false,
            accion: Accion::GuardarSeleccion,
        });
    }
    let c = crate::cabecera::Cabecera {
        titulo: &titulo,
        subtitulo: &subtitulo,
        buscador: Some(crate::cabecera::Buscador {
            texto: &texto,
            pista: &pista,
            foco: e.buscando,
            cursor: Some(e.consulta.pos()),
            enfocar: Accion::Buscar,
            vaciar: Accion::LimpiarBusqueda,
        }),
        botones,
        mover: Accion::Mover,
        cerrar: Accion::Cerrar,
    };
    crate::cabecera::pintar(p, &mut e.botones, w, s, &c);
}

/// Una fila de la barra de la izquierda: el rotulo, su cuenta a la derecha
/// y, elegida, el fondo de tarjeta con la raya azul.
#[allow(clippy::too_many_arguments)]
fn fila_lateral(
    e: &mut Estado,
    p: &Pintor,
    caja: RectF,
    rotulo: &str,
    cuenta: Option<usize>,
    elegida: bool,
    a: Accion,
    s: f32,
) {
    let encima = dentro(caja, e.botones.raton);
    if elegida {
        p.rellenar_redondeado(caja, d::RADIO_BOTON * s, d::TARJETA_ENCIMA);
        p.rellenar_redondeado(
            RectF {
                x: caja.x,
                y: caja.y + 9.0 * s,
                ancho: 3.0 * s,
                alto: caja.alto - 18.0 * s,
            },
            1.5 * s,
            d::ACENTO,
        );
    } else if encima {
        p.rellenar_redondeado(caja, d::RADIO_BOTON * s, d::TARJETA);
    }
    let tam = d::LETRA_CUERPO * s;
    let (_, th) = p.medir_texto(rotulo, tam);
    p.texto(
        rotulo,
        caja.x + 14.0 * s,
        caja.y + (caja.alto - th) / 2.0,
        tam,
        if elegida { d::TEXTO } else { d::CUERPO },
    );
    if let Some(n) = cuenta {
        let n = n.to_string();
        let tc = d::LETRA_SECUNDARIO * s;
        let (nw, nh) = p.medir_texto(&n, tc);
        p.texto(
            &n,
            caja.x + caja.ancho - 12.0 * s - nw,
            caja.y + (caja.alto - nh) / 2.0,
            tc,
            d::GRIS,
        );
    }
    e.botones.zona(caja, a);
}

fn pintar_lateral(e: &mut Estado, p: &Pintor, z: RectF, s: f32, t: &Catalogo) {
    p.rellenar(z, hex_lateral());
    let m = d::MARGEN * s;
    // **Que quepa todo** (25-oct-2026: de 6 categorias de serie se paso a 18):
    // las filas se encogen lo justo para que las categorias y lo de insertar
    // entren enteras, sin bajar de un minimo que aun se pulse bien.
    let filas = 1
        + (0..e.cats.len())
            .filter(|k| {
                let c = &e.cats[*k];
                c == &e.mis || e.figuras.iter().any(|f| de_que_es(f, &e.mis).0 == *c)
            })
            .count()
        + e.herramientas.len();
    let fijo = m + m + m * 0.75 + 26.0 * s + m;
    let fh = ((z.alto - fijo) / filas.max(1) as f32 - 2.0 * s).clamp(24.0 * s, FILA_LATERAL * s);
    let x = z.x + m * 0.75;
    let ancho = z.ancho - m * 1.5;
    let mut y = z.y + m;
    let cuenta_de = |e: &Estado, c: &str| {
        e.figuras
            .iter()
            .filter(|f| de_que_es(f, &e.mis).0 == c)
            .count()
    };
    let todas = t.t("figuras-ventana-todas");
    let n = e.figuras.len();
    let elegida = e.cat.is_none();
    fila_lateral(
        e,
        p,
        RectF {
            x,
            y,
            ancho,
            alto: fh,
        },
        &todas,
        Some(n),
        elegida,
        Accion::Categoria(0),
        s,
    );
    y += fh + 2.0 * s;
    for k in 0..e.cats.len() {
        let c = e.cats[k].clone();
        let cuantas = cuenta_de(e, &c);
        if cuantas == 0 && c != e.mis {
            continue;
        }
        let elegida = e.cat == Some(k);
        fila_lateral(
            e,
            p,
            RectF {
                x,
                y,
                ancho,
                alto: fh,
            },
            &c,
            Some(cuantas),
            elegida,
            Accion::Categoria(k + 1),
            s,
        );
        y += fh + 2.0 * s;
    }
    // Lo demas que traia el menu.
    y += m;
    p.rellenar(
        RectF {
            x,
            y,
            ancho,
            alto: 1.0 * s.max(1.0),
        },
        blanco(0.07),
    );
    y += m * 0.75;
    let rotulo = t.t("figuras-ventana-insertar-seccion").to_uppercase();
    p.texto(&rotulo, x + 14.0 * s, y, d::LETRA_CHAPA * s, d::GRIS);
    y += 26.0 * s;
    let herramientas = e.herramientas.clone();
    for (id, texto) in herramientas {
        if y + fh > z.y + z.alto - m {
            break;
        }
        fila_lateral(
            e,
            p,
            RectF {
                x,
                y,
                ancho,
                alto: fh,
            },
            &texto,
            None,
            false,
            Accion::Herramienta(id),
            s,
        );
        y += fh + 2.0 * s;
    }
}

/// El fondo de la barra lateral: un poco mas claro que la ventana, como el
/// carril de la galeria.
fn hex_lateral() -> Color {
    crate::caja_dibujo::hex(0x202022)
}

fn pintar_rejilla(e: &mut Estado, p: &Pintor, z: RectF, s: f32, t: &Catalogo) {
    if e.lista.is_empty() {
        let msg = t.t("figuras-ventana-vacio");
        crate::ventanita::centrado_en(
            p,
            &msg,
            z,
            z.y + z.alto / 2.0 - 10.0 * s,
            d::LETRA_CUERPO * s,
            d::GRIS,
        );
        return;
    }
    let piezas = e.disp.piezas.clone();
    p.con_recorte(z, |p| {
        for (r, pieza) in &piezas {
            let c = RectF {
                x: z.x + r.x,
                y: z.y + r.y - e.scroll,
                ..*r
            };
            if c.y > z.y + z.alto || c.y + c.alto < z.y {
                continue;
            }
            match pieza {
                Pieza::Titulo(cat) => {
                    let tam = d::LETRA_TITULO_TARJETA * s;
                    p.texto(cat, c.x, c.y + 14.0 * s, tam, d::TEXTO);
                }
                Pieza::Celda(i) => pintar_celda(e, p, c, z, *i, s),
            }
        }
    });
}

/// Encaja `w x h` en `caja` sin deformar, centrado.
fn encajar(caja: RectF, w: f32, h: f32) -> RectF {
    let k = (caja.ancho / w.max(1.0)).min(caja.alto / h.max(1.0));
    let (ww, hh) = (w * k, h * k);
    RectF {
        x: caja.x + (caja.ancho - ww) / 2.0,
        y: caja.y + (caja.alto - hh) / 2.0,
        ancho: ww,
        alto: hh,
    }
}

fn pintar_celda(e: &mut Estado, p: &Pintor, c: RectF, zona: RectF, i: usize, s: f32) {
    let encima = dentro(c, e.botones.raton) && dentro(zona, e.botones.raton);
    let elegida = e.elegida == Some(i);
    if elegida {
        p.rellenar_redondeado(
            encoger(c, -3.0 * s),
            d::RADIO_TARJETA * s + 3.0 * s,
            d::ACENTO,
        );
        p.rellenar_redondeado(encoger(c, -s), d::RADIO_TARJETA * s + 1.0 * s, d::FONDO);
    }
    p.rellenar_redondeado(
        c,
        d::RADIO_TARJETA * s,
        if encima {
            d::TARJETA_ENCIMA
        } else {
            d::TARJETA
        },
    );
    // La miniatura sobre su papel claro: las plantillas son hojas blancas.
    let papel = encoger(
        RectF {
            alto: MINI_ALTO * s,
            ..c
        },
        10.0 * s,
    );
    p.rellenar_redondeado(papel, 8.0 * s, crate::caja_dibujo::hex(0xF2F2F5));
    if let Some((b, w, h)) = e.minis.get(&i) {
        let dentro_del_papel = encajar(encoger(papel, 8.0 * s), *w as f32, *h as f32);
        p.bitmap(b, dentro_del_papel, None, false);
    }
    let nombre = de_que_es(&e.figuras[i], &e.mis).1.to_string();
    let tam = d::LETRA_SECUNDARIO * s;
    let ancho = c.ancho - 20.0 * s;
    let corto = recortar(p, &nombre, tam, ancho);
    let (_, th) = p.medir_texto(&corto, tam);
    p.texto(
        &corto,
        c.x + 10.0 * s,
        c.y + MINI_ALTO * s + (NOMBRE_ALTO * s - th) / 2.0 - 4.0 * s,
        tam,
        if elegida { d::TEXTO } else { d::CUERPO },
    );
    if let Some(zc) = cortar(c, zona) {
        e.botones.zona(zc, Accion::Celda(i));
    }
}

/// El texto que cabe en `ancho`, con puntos suspensivos si no cabe.
fn recortar(p: &Pintor, s: &str, tam: f32, ancho: f32) -> String {
    if p.medir_texto(s, tam).0 <= ancho {
        return s.to_string();
    }
    let mut v: Vec<char> = s.chars().collect();
    while !v.is_empty() {
        v.pop();
        let t: String = v.iter().collect::<String>() + "…";
        if p.medir_texto(&t, tam).0 <= ancho {
            return t;
        }
    }
    "…".into()
}

fn pintar_panel(e: &mut Estado, p: &Pintor, z: RectF, s: f32, t: &Catalogo) {
    p.rellenar(z, hex_lateral());
    let m = d::MARGEN * s * 1.25;
    let x = z.x + m;
    let ancho = z.ancho - 2.0 * m;
    let Some(i) = e.elegida else {
        let msg = t.t("figuras-ventana-elige");
        crate::ventanita::centrado_en(
            p,
            &msg,
            z,
            z.y + z.alto / 2.0,
            d::LETRA_SECUNDARIO * s,
            d::GRIS,
        );
        return;
    };
    let f = e.figuras[i].clone();
    let (cat, nombre) = {
        let (c, n) = de_que_es(&f, &e.mis);
        (c.to_string(), n.to_string())
    };
    let mut y = z.y + m;
    // La vista grande, sobre su papel.
    // Lo que va debajo manda: el titulo, los campos del intervalo (64 cada
    // fila), la previa, el nombre de las propias y los dos botones. La vista
    // se queda con lo que sobre (en una pantalla al 150 % los campos
    // acababan debajo de «Insertar»).
    let filas_campos = e.alargable().map_or(0, |c| {
        let cs = campos_de(&c);
        let sueltos = cs.iter().filter(|c| matches!(c, Dato::DeA)).count();
        sueltos + (cs.len() - sueltos).div_ceil(2)
    }) as f32;
    let debajo = 56.0 * s
        + if filas_campos > 0.0 {
            22.0 * s + 64.0 * s * filas_campos + 30.0 * s
        } else {
            0.0
        }
        + if serie::es_de_serie(&f) {
            0.0
        } else {
            22.0 * s + d::BOTON * s + d::HUECO_GRANDE * s
        }
        + 2.0 * d::BOTON * s
        + d::HUECO * s;
    let alto_vista = (z.alto - 3.0 * m - debajo)
        .min(z.alto * 0.42)
        .min(ancho * 1.25)
        .max(80.0 * s);
    let papel = RectF {
        x,
        y,
        ancho,
        alto: alto_vista,
    };
    p.rellenar_redondeado(
        papel,
        d::RADIO_TARJETA * s,
        crate::caja_dibujo::hex(0xF2F2F5),
    );
    if let Some((b, w, h)) = e.minis.get(&i) {
        p.bitmap(
            b,
            encajar(encoger(papel, 12.0 * s), *w as f32, *h as f32),
            None,
            false,
        );
    }
    y += alto_vista + m;
    let tam_t = d::LETRA_TITULO_TARJETA * s * 1.15;
    p.texto(&recortar(p, &nombre, tam_t, ancho), x, y, tam_t, d::TEXTO);
    y += 26.0 * s;
    let mut a = fluent_bundle::FluentArgs::new();
    a.set("n", f.elementos.len());
    let sub = format!("{cat}  ·  {}", t.t_args("figuras-ventana-piezas", &a));
    p.texto(
        &recortar(p, &sub, d::LETRA_SECUNDARIO * s, ancho),
        x,
        y,
        d::LETRA_SECUNDARIO * s,
        d::GRIS,
    );
    y += 30.0 * s;

    let boton_h = d::BOTON * s;
    let fin = z.y + z.alto - m;
    if let Some(clave) = e.alargable() {
        p.texto(
            &t.t("figuras-ventana-intervalo").to_uppercase(),
            x,
            y,
            d::LETRA_CHAPA * s,
            d::GRIS,
        );
        y += 22.0 * s;
        let cs = campos_de(&clave);
        // De dos en dos por fila; el que queda solo, a lo ancho.
        let hueco = d::HUECO * s;
        let medio = (ancho - hueco) / 2.0;
        let mut k = 0;
        while k < cs.len() {
            let par =
                k + 1 < cs.len() && !matches!(cs[k], Dato::DeA) && !matches!(cs[k + 1], Dato::DeA);
            let n = if par { 2 } else { 1 };
            for j in 0..n {
                let c = cs[k + j];
                let ancho_c = if par { medio } else { ancho };
                let cx = x + (medio + hueco) * j as f32;
                pintar_campo(
                    e,
                    p,
                    RectF {
                        x: cx,
                        y,
                        ancho: ancho_c,
                        alto: 58.0 * s,
                    },
                    c,
                    &clave,
                    s,
                    t,
                );
            }
            y += 64.0 * s;
            k += n;
        }
        if let Some(pr) = &e.previa {
            let (texto, color) = match pr {
                Ok(t) => (format!("→ {t}"), d::ENLACE),
                Err(e) => (e.clone(), d::ROJO_TEXTO),
            };
            let tam = d::LETRA_SECUNDARIO * s;
            let (_, th) = p.medir_texto_ajustado(&texto, tam, ancho);
            p.texto_ajustado(&texto, x, y, tam, ancho, color);
            y += th + 10.0 * s;
        }
    }
    let propia = !serie::es_de_serie(&f);
    if propia {
        p.texto(
            &t.t("figuras-nombre").to_uppercase(),
            x,
            y,
            d::LETRA_CHAPA * s,
            d::GRIS,
        );
        y += 22.0 * s;
        let foco = e.foco == Some(Dato::Nombre);
        let caja = RectF {
            x,
            y,
            ancho: ancho - 108.0 * s,
            alto: boton_h,
        };
        e.campos
            .entry(Dato::Nombre)
            .or_insert_with(|| Campo::con(&f.nombre))
            .pintar(p, caja, d::LETRA_CUERPO * s, foco, "", 0.0, s);
        e.botones.zona(caja, Accion::Campo(Dato::Nombre));
        let rb = RectF {
            x: x + ancho - 100.0 * s,
            y,
            ancho: 100.0 * s,
            alto: boton_h,
        };
        boton_v2(
            p,
            &mut e.botones,
            rb,
            Accion::Renombrar,
            None,
            &t.t("figuras-ventana-renombrar"),
            None,
            Some(d::CAJA),
            d::TEXTO,
            s,
        );
        y += boton_h + d::HUECO_GRANDE * s;
    }
    // Abajo del todo: Insertar y, debajo, Quitar.
    let quitar_y = fin - boton_h;
    let insertar_y = quitar_y - boton_h - d::HUECO * s;
    let _ = y;
    boton_v2(
        p,
        &mut e.botones,
        RectF {
            x,
            y: insertar_y,
            ancho,
            alto: boton_h,
        },
        Accion::Insertar,
        Some(&mi::ADD),
        &t.t("figuras-ventana-insertar"),
        Some("Intro"),
        Some(d::AZUL_LLENO),
        Color::BLANCO,
        s,
    );
    let rotulo_quitar = if e.confirmar_quitar {
        t.t("figuras-ventana-quitar-seguro")
    } else {
        t.t("figuras-quitar")
    };
    boton_v2(
        p,
        &mut e.botones,
        RectF {
            x,
            y: quitar_y,
            ancho,
            alto: boton_h,
        },
        Accion::Quitar,
        Some(&mi::DELETE),
        &rotulo_quitar,
        None,
        if e.confirmar_quitar {
            Some(d::ROJO)
        } else {
            None
        },
        if e.confirmar_quitar {
            Color::BLANCO
        } else {
            d::ROJO_TEXTO
        },
        s,
    );
}

/// Un campo del intervalo: su rotulo encima y la caja con la pista de lo
/// que trae la de serie.
#[allow(clippy::too_many_arguments)]
fn pintar_campo(
    e: &mut Estado,
    p: &Pintor,
    caja: RectF,
    c: Dato,
    clave: &str,
    s: f32,
    t: &Catalogo,
) {
    let rotulo = t.t(match c {
        Dato::Desde => "figuras-ventana-desde",
        Dato::Hasta => "figuras-ventana-hasta",
        Dato::DeA => "figuras-ventana-de-a",
        Dato::SemanasDesde => "figuras-ventana-semanas-desde",
        Dato::SemanasHasta => "figuras-ventana-semanas-hasta",
        Dato::Mes => "figuras-ventana-mes",
        Dato::Meses => "figuras-ventana-meses",
        Dato::Nombre => "figuras-nombre",
    });
    p.texto(
        &rotulo,
        caja.x + 2.0 * s,
        caja.y,
        d::LETRA_SECUNDARIO * s * 0.92,
        d::CUERPO,
    );
    let pista = match c {
        Dato::Desde => serie::tramo_de_serie(clave)
            .map(|t| t.0.to_string())
            .unwrap_or_default(),
        Dato::Hasta => serie::tramo_de_serie(clave)
            .map(|t| t.1.to_string())
            .unwrap_or_default(),
        Dato::DeA => t.t("figuras-ventana-de-a-pista"),
        Dato::Mes => "2026-11".into(),
        Dato::Meses => "1".into(),
        Dato::SemanasDesde => "1".into(),
        Dato::SemanasHasta => "12".into(),
        Dato::Nombre => String::new(),
    };
    let r = RectF {
        x: caja.x,
        y: caja.y + 20.0 * s,
        ancho: caja.ancho,
        alto: d::BOTON * s * 0.95,
    };
    let foco = e.foco == Some(c);
    e.campos
        .entry(c)
        .or_default()
        .pintar(p, r, d::LETRA_CUERPO * s, foco, &pista, 0.0, s);
    e.botones.zona(r, Accion::Campo(c));
}

#[cfg(test)]
#[path = "ventana/pruebas.rs"]
mod pruebas;
