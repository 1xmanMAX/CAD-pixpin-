//! **La mesa de trabajo**: el plano en el centro, la cinta de ordenes
//! arriba (por pestanas, como los grupos de un CAD), las capas y las
//! propiedades a la derecha, y abajo la linea de ordenes y la barra de
//! estado. Todo lo de editar lo decide `cad_nucleo::Sesion`; aqui se pinta
//! y se le pasan los clics y lo que se escribe.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cad_nucleo::enganche::{Enganche, Tipo};
use cad_nucleo::geo::{Forma, P};
use cad_nucleo::herramientas::capas;
use cad_nucleo::herramientas::vista::PedidoVista;
use cad_nucleo::opencadcodec::EntityType;
use cad_nucleo::ordenes::{self, Grupo, Id};
use cad_nucleo::{Dibujo, Sesion, formas, metrados};
use pixpin_render::{Color, Pintor, RectF};

use crate::autoguardado;
use crate::encima::{ClaveAjuste, Memo, Resalte};
use crate::plano::{Camara, Plano};
use crate::tema;
use crate::ui::{self, Accion, Estilo, Interruptor, Zona, rect};

/// Las pestanas que se ensenan: las que tienen alguna orden.
pub fn pestanas() -> Vec<Grupo> {
    ordenes::GRUPOS
        .iter()
        .copied()
        .filter(|g| !ordenes::de_grupo(*g).is_empty())
        .collect()
}

/// Donde va cada parte, en pixeles.
#[derive(Debug, Clone, Copy)]
pub struct Disposicion {
    pub cinta: RectF,
    pub panel: RectF,
    pub ordenes: RectF,
    pub estado: RectF,
    pub dibujo: RectF,
}

/// Con el historial de siempre (la mesa usa [`Mesa::disp`], que sabe si F2 lo alargo).
#[cfg_attr(not(test), allow(dead_code))]
pub fn disposicion(w: f32, h: f32, e: f32) -> Disposicion {
    disposicion_con(w, h, e, tema::LINEAS_HISTORIAL)
}

/// Lineas del historial con F2 (el alto).
pub const LINEAS_HISTORIAL_ALTO: usize = 12;
/// Cuantas vistas recuerda ZOOM Previo.
const VISTAS: usize = 20;

/// La disposicion con `lineas` de historial en la linea de ordenes.
pub fn disposicion_con(w: f32, h: f32, e: f32, lineas: usize) -> Disposicion {
    let alto_cinta = (tema::ALTO_PESTANAS + tema::ALTO_BOTONES) * e;
    let alto_estado = tema::ALTO_ESTADO * e;
    let alto_ordenes = (tema::ALTO_LINEA * lineas as f32 + tema::ALTO_ORDEN + 6.0) * e;
    let ancho_panel = (tema::ANCHO_PANEL * e).min(w * 0.4);
    let abajo = alto_estado + alto_ordenes;
    let medio = (h - alto_cinta - abajo).max(0.0);
    Disposicion {
        cinta: rect(0.0, 0.0, w, alto_cinta),
        panel: rect(w - ancho_panel, alto_cinta, ancho_panel, medio),
        ordenes: rect(0.0, h - abajo, w, alto_ordenes),
        estado: rect(0.0, h - alto_estado, w, alto_estado),
        dibujo: rect(0.0, alto_cinta, (w - ancho_panel).max(0.0), medio),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Abajo {
    x: f32,
    y: f32,
    arrastra: bool,
}

/// **La mesa** con su dibujo abierto.
pub struct Mesa {
    pub sesion: Sesion,
    pub camara: Camara,
    pub plano: Plano,
    pub pestana: Grupo,
    /// Lo que se va escribiendo en la linea de ordenes.
    pub escrito: String,
    /// El raton (pixeles de la ventana), si esta dentro.
    pub cursor: Option<(f32, f32)>,
    abajo: Option<Abajo>,
    /// Moviendo la vista con el boton central: de donde y con que camara.
    mano: Option<(f32, f32, Camara)>,
    /// Si ya se encuadro el plano la primera vez.
    encuadrado: bool,
    /// El enganche bajo el cursor (lo calcula el pintado).
    enganche: Option<Enganche>,
    /// Si el plano ya esta en la tarjeta y su caja (lo pone la ventana antes
    /// de pintar, que se lleva el plano mientras tanto).
    pub hay_plano: bool,
    pub caja_plano: Option<[f64; 4]>,
    /// Las vistas de antes (ZOOM Previo), la mas reciente al final.
    vistas: Vec<Camara>,
    /// Cuando giro la rueda por ultima vez (un giro seguido es una sola vista).
    ultima_rueda: Option<Instant>,
    /// F2: el historial de la linea de ordenes, alto.
    pub historial_alto: bool,
    /// F7: la rejilla pintada.
    pub rejilla_visible: bool,
    /// La primera capa que se ve en la lista (se mueve con la rueda).
    capas_desde: usize,
    /// Cuantas caben en la lista (lo apunta el pintado).
    capas_caben: Cell<usize>,
    /// Cuantos objetos hay en cada capa (en mayusculas), de una version del dibujo.
    conteo: Option<(u64, HashMap<String, usize>)>,
    pub autoguardado: autoguardado::Estado,
    /// Lo elegido listo para resaltar y el ultimo enganche buscado (ver
    /// [`crate::encima`]).
    resalte: Resalte,
    ajuste: Memo<ClaveAjuste, (P, Option<Enganche>)>,
}

impl Mesa {
    pub fn nueva(dibujo: Dibujo) -> Mesa {
        let mut sesion = Sesion::nueva(dibujo);
        let nombre = sesion
            .dibujo
            .ruta
            .as_ref()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        sesion.avisar(match nombre {
            Some(n) => format!("Abierto {n}. Escriba una orden (L, PL, MU, TR…) o use la cinta."),
            None => "Plano nuevo en milimetros. Escriba una orden (L, PL, MU, EJ…) o use la cinta."
                .to_string(),
        });
        Mesa {
            sesion,
            camara: Camara {
                centro: [0.0, 0.0],
                px: 10.0,
            },
            plano: Plano::nuevo(),
            pestana: Grupo::Dibujo,
            escrito: String::new(),
            cursor: None,
            abajo: None,
            mano: None,
            encuadrado: false,
            enganche: None,
            hay_plano: false,
            caja_plano: None,
            vistas: Vec::new(),
            ultima_rueda: None,
            historial_alto: false,
            rejilla_visible: false,
            capas_desde: 0,
            capas_caben: Cell::new(usize::MAX),
            conteo: None,
            autoguardado: autoguardado::Estado::nuevo(),
            resalte: Resalte::default(),
            ajuste: Memo::default(),
        }
    }

    fn lineas(&self) -> usize {
        if self.historial_alto {
            LINEAS_HISTORIAL_ALTO
        } else {
            tema::LINEAS_HISTORIAL
        }
    }

    /// Donde va cada parte, con el historial como este.
    pub fn disp(&self, w: f32, h: f32, e: f32) -> Disposicion {
        disposicion_con(w, h, e, self.lineas())
    }

    pub fn titulo(&self) -> String {
        let n = self
            .sesion
            .dibujo
            .ruta
            .as_ref()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sin nombre".into());
        format!(
            "{n}{} — CAD-pixpin",
            if self.sesion.dibujo.sucio() {
                " •"
            } else {
                ""
            }
        )
    }

    pub fn ruta(&self) -> Option<&Path> {
        self.sesion.dibujo.ruta.as_deref()
    }

    /// Pide dibujar lo que cambio (llamar cada vuelta del bucle).
    pub fn al_dia(&mut self) {
        self.plano.pedir(&self.sesion.dibujo);
    }

    /// El plano entero en la zona de dibujo.
    pub fn encuadrar(&mut self, w: f32, h: f32, e: f32) {
        if self.encuadrado {
            self.guardar_vista();
        }
        let d = self.disp(w, h, e).dibujo;
        let caja = self
            .sesion
            .dibujo
            .indice()
            .caja()
            .or(self.caja_plano)
            .unwrap_or([0.0, 0.0, 20_000.0, 12_000.0]);
        self.camara = Camara::encuadrar(
            caja,
            [d.x as f64, d.y as f64, d.ancho as f64, d.alto as f64],
            w as f64,
            h as f64,
        );
        self.encuadrado = true;
    }

    fn a_plano(&self, x: f32, y: f32, w: f32, h: f32) -> P {
        self.camara.plano(x as f64, y as f64, w as f64, h as f64)
    }

    /// Si el cursor esta sobre el plano (y se esconde la flecha).
    pub fn en_el_dibujo(&self, x: f32, y: f32, w: f32, h: f32, e: f32) -> bool {
        ui::dentro(&self.disp(w, h, e).dibujo, x, y)
    }

    // ------------------------------------------------------------- raton

    pub fn mover(&mut self, x: f32, y: f32, w: f32, h: f32, e: f32) {
        self.cursor = Some((x, y));
        if let Some((x0, y0, c)) = self.mano {
            self.camara = Camara {
                centro: [
                    c.centro[0] - (x - x0) as f64 * c.px,
                    c.centro[1] + (y - y0) as f64 * c.px,
                ],
                px: c.px,
            };
        }
        if let Some(a) = &mut self.abajo
            && !a.arrastra
            && ((x - a.x).abs() > 5.0 * e || (y - a.y).abs() > 5.0 * e)
        {
            a.arrastra = true;
        }
        let _ = (w, h);
    }

    /// Un boton baja dentro del plano (`central`: la mano).
    pub fn bajar(&mut self, x: f32, y: f32, central: bool) {
        if central {
            self.guardar_vista();
            self.mano = Some((x, y, self.camara));
        } else {
            self.abajo = Some(Abajo {
                x,
                y,
                arrastra: false,
            });
        }
    }

    pub fn subir(&mut self, x: f32, y: f32, central: bool, w: f32, h: f32, e: f32) {
        if central {
            self.mano = None;
            return;
        }
        let Some(a) = self.abajo.take() else { return };
        let tol = self.camara.px * (tema::CAJA_CURSOR * e) as f64 / 2.0 + self.camara.px;
        if a.arrastra && self.sesion.pide_objetos() {
            let (p0, p1) = (self.a_plano(a.x, a.y, w, h), self.a_plano(x, y, w, h));
            self.sesion.recuadro(p0, p1);
            return;
        }
        let crudo = self.a_plano(x, y, w, h);
        if self.sesion.activa().is_none() && self.sesion.tomar_pinzamiento(crudo, tol * 1.5) {
            return;
        }
        let p = if self.sesion.pide_objetos() {
            crudo
        } else {
            self.sesion.ajustar(crudo, self.radio_enganche(e)).0
        };
        self.sesion.clic(p, tol);
    }

    fn radio_enganche(&self, e: f32) -> f64 {
        self.camara.px * (tema::RADIO_ENGANCHE * e) as f64
    }

    pub fn rueda(&mut self, pasos: f32, x: f32, y: f32, w: f32, h: f32) {
        let ahora = Instant::now();
        if self
            .ultima_rueda
            .is_none_or(|t| ahora.duration_since(t) > Duration::from_millis(600))
        {
            self.guardar_vista();
        }
        self.ultima_rueda = Some(ahora);
        let f = 0.8_f64.powf(pasos as f64);
        self.camara = self.camara.zoom(f, x as f64, y as f64, w as f64, h as f64);
    }

    /// El boton derecho: Enter (como en AutoCAD).
    pub fn derecho(&mut self) {
        self.enter();
    }

    // ----------------------------------------------------------- teclado

    pub fn enter(&mut self) {
        let t = std::mem::take(&mut self.escrito);
        self.sesion.escribir(&t);
    }

    pub fn caracter(&mut self, c: char) {
        match c {
            '\r' | '\n' => self.enter(),
            ' ' if !self.sesion.pide_texto() => self.enter(),
            '\u{8}' => {
                self.escrito.pop();
            }
            c if !c.is_control() => self.escrito.push(c),
            _ => {}
        }
    }

    pub fn escape(&mut self) {
        if !self.escrito.is_empty() {
            self.escrito.clear();
        } else {
            self.sesion.escape();
        }
    }

    // ----------------------------------------------------------- acciones

    pub fn alternar(&mut self, i: Interruptor) {
        let a = &mut self.sesion.ajustes;
        let (nombre, encendido) = match i {
            Interruptor::Enganche => {
                a.enganche = !a.enganche;
                ("Enganche", a.enganche)
            }
            Interruptor::Orto => {
                a.orto = !a.orto;
                ("Orto", a.orto)
            }
            Interruptor::Polar => {
                a.polar = if a.polar.is_some() {
                    None
                } else {
                    Some(std::f64::consts::PI / 4.0)
                };
                ("Polar", a.polar.is_some())
            }
            Interruptor::Rejilla => {
                // Un paso redondo a la escala de lo que se ve.
                a.rejilla = if a.rejilla.is_some() {
                    None
                } else {
                    Some(paso_redondo(self.camara.px * 20.0))
                };
                ("Rejilla", a.rejilla.is_some())
            }
        };
        self.sesion.avisar(format!(
            "<{nombre} {}>",
            if encendido { "activado" } else { "desactivado" }
        ));
    }

    /// El metrado del plano (de lo elegido, si hay algo) en CSV.
    pub fn metrado_csv(&self) -> (String, usize) {
        let elegidos =
            (!self.sesion.seleccion.is_empty()).then_some(self.sesion.seleccion.as_slice());
        let filas = metrados::del_plano(&self.sesion.dibujo, elegidos);
        let k = metrados::unidades_por_metro(self.sesion.dibujo.doc.header.insertion_units);
        (metrados::csv_plano(&filas, k), filas.len())
    }

    /// Guarda en `ruta` (o en la suya).
    pub fn guardar(&mut self, ruta: Option<PathBuf>) -> Result<PathBuf, String> {
        let r = match ruta {
            Some(r) => {
                self.sesion.dibujo.guardar_como(&r)?;
                r
            }
            None => {
                self.sesion.dibujo.guardar()?;
                self.sesion.dibujo.ruta.clone().unwrap_or_default()
            }
        };
        self.autoguardado.olvidar();
        self.sesion.avisar(format!("Guardado en {}", r.display()));
        Ok(r)
    }

    // ----------------------------------------------------------- pintar

    /// **Pinta la mesa** encima del plano ya dibujado y da las zonas de los botones.
    pub fn pintar(&mut self, p: &Pintor, w: f32, h: f32, e: f32) -> Vec<Zona> {
        let d = self.disp(w, h, e);
        self.contar_capas();
        if !self.encuadrado && (self.hay_plano || !self.plano.esperando()) {
            self.encuadrar(w, h, e);
        }
        let mut zonas = Vec::new();
        // Lo que hay encima del plano, recortado a su zona.
        let cursor_dibujo = self.cursor.filter(|(x, y)| ui::dentro(&d.dibujo, *x, *y));
        let ajustado = cursor_dibujo.map(|(x, y)| {
            let crudo = self.a_plano(x, y, w, h);
            let radio = self.radio_enganche(e);
            let sin = self.sesion.pide_objetos() && self.sesion.activa().is_some();
            // Si el cursor y lo demas siguen igual, el enganche de antes.
            let clave = ClaveAjuste::de(&self.sesion, crudo, radio, sin);
            let sesion = &self.sesion;
            self.ajuste.o_calcular(clave, || {
                if sin {
                    (crudo, None)
                } else {
                    sesion.ajustar(crudo, radio)
                }
            })
        });
        self.enganche = ajustado.and_then(|a| a.1);
        let indice = self.sesion.dibujo.indice();
        let version = self.sesion.dibujo.version();
        self.resalte
            .al_dia(version, &self.sesion.seleccion, &indice);
        let vista = {
            let (a, b) = (
                self.a_plano(d.dibujo.x, d.dibujo.y + d.dibujo.alto, w, h),
                self.a_plano(d.dibujo.x + d.dibujo.ancho, d.dibujo.y, w, h),
            );
            [a[0], a[1], b[0], b[1]]
        };
        let (wd, hd) = (w as f64, h as f64);
        let a_pantalla = |q: P| self.camara.pantalla(q, wd, hd);
        p.con_recorte(d.dibujo, |p| {
            if !self.hay_plano {
                let t = match &self.plano.error {
                    Some(e) => format!("No se pudo dibujar: {e}"),
                    None if self.plano.esperando() => "Dibujando el plano…".into(),
                    None => String::new(),
                };
                p.texto(
                    &t,
                    d.dibujo.x + 24.0 * e,
                    d.dibujo.y + 20.0 * e,
                    14.0 * e,
                    tema::TEXTO_SUAVE,
                );
            }
            if self.rejilla_visible {
                self.pintar_rejilla(p, &d.dibujo, w, h, e);
            }
            self.pintar_pendiente(p, &a_pantalla, e, &vista);
            self.pintar_seleccion(p, &a_pantalla, e, &vista);
            if let Some((q, _)) = ajustado {
                let previa = self.sesion.previa(q);
                for f in &previa.formas {
                    trazar_forma(p, f, &a_pantalla, self.camara.px, 1.5 * e, tema::PREVIA);
                }
                for (a, b) in &previa.guias {
                    let (pa, pb) = (a_pantalla(*a), a_pantalla(*b));
                    p.polilinea_discontinua(&[pa, pb], 1.0 * e, tema::rgba(0xA1, 0xA1, 0xA8, 0.8));
                }
                for (q, t) in &previa.etiquetas {
                    let (x, y) = a_pantalla(*q);
                    p.texto_con_fondo(
                        t,
                        x + 18.0 * e,
                        y + 18.0 * e,
                        11.0 * e,
                        tema::TEXTO,
                        tema::rgba(0x0E, 0x0E, 0x10, 0.85),
                    );
                }
            }
            // El recuadro de eleccion.
            if let (Some(a), Some((x, y))) = (self.abajo, self.cursor)
                && a.arrastra
                && self.sesion.pide_objetos()
            {
                let captura = x < a.x;
                let r = rect(a.x.min(x), a.y.min(y), (x - a.x).abs(), (y - a.y).abs());
                let (relleno, borde) = if captura {
                    (tema::VERDE_SUAVE, tema::VERDE)
                } else {
                    (tema::ACENTO_SUAVE, tema::ACENTO)
                };
                p.rellenar(r, relleno);
                if captura {
                    p.trazar_discontinuo(r, 1.0 * e, borde);
                } else {
                    p.trazar(r, 1.0 * e, borde);
                }
            }
            if let Some(g) = self.enganche {
                pintar_enganche(p, &g, &a_pantalla, e);
            }
            if let Some((x, y)) = cursor_dibujo {
                pintar_cursor(p, x, y, &d.dibujo, e, self.sesion.pide_objetos());
            }
        });

        self.pintar_cinta(p, &d, e, &mut zonas);
        self.pintar_panel(p, &d, e, &mut zonas);
        self.pintar_ordenes(p, &d, e);
        self.pintar_estado(p, &d, e, ajustado.map(|a| a.0), &mut zonas);
        zonas
    }

    /// Lo elegido que se ve en `vista`: lo mayor primero y, con mucho
    /// elegido (Ctrl+A), hasta un tope y un recuadro de todo.
    /// **Lo recien dibujado que aun no esta en la tarjeta**: mientras su cubo
    /// se vuelve a convertir en otro hilo, se pinta aqui con el color de su
    /// capa. Sin esto, lo ultimo que se dibujo desaparecia un par de
    /// fotogramas (el parpadeo al dibujar una linea tras otra).
    fn pintar_pendiente(
        &self,
        p: &Pintor,
        a_pantalla: &impl Fn(P) -> (f32, f32),
        e: f32,
        vista: &[f64; 4],
    ) {
        const TOPE: usize = 5000;
        if !self.plano.esperando() {
            return;
        }
        let dib = &self.sesion.dibujo;
        let Some(hs) = dib.cambios_desde(self.plano.vista) else {
            return;
        };
        let ind = dib.indice();
        for h in hs.iter().take(TOPE) {
            let (Some(en), Some(ent)) = (ind.de_handle(*h), dib.entidad(*h)) else {
                continue;
            };
            if !cad_nucleo::geo::se_tocan(&en.caja, vista) {
                continue;
            }
            let color = color_de_entidad(dib, ent);
            for f in &en.formas {
                trazar_forma(p, f, a_pantalla, self.camara.px, 1.0 * e, color);
            }
        }
    }

    fn pintar_seleccion(
        &self,
        p: &Pintor,
        a_pantalla: &impl Fn(P) -> (f32, f32),
        e: f32,
        vista: &[f64; 4],
    ) {
        if self.sesion.seleccion.is_empty() {
            return;
        }
        const TOPE_FORMAS: usize = 3000;
        let ind = self.sesion.dibujo.indice();
        // Con mucho elegido, lo que en pantalla no llega a dos pixeles no se
        // resalta (el recuadro de todo dice que esta elegido).
        let minimo = if self.sesion.seleccion.len() > TOPE_FORMAS {
            self.camara.px * 2.0
        } else {
            0.0
        };
        let (cuales, mut quedan) = self.resalte.visibles(vista, minimo, TOPE_FORMAS);
        let mut trazadas = 0;
        for k in cuales {
            let Some(en) = ind.entradas.get(k as usize) else {
                continue;
            };
            trazadas += en.formas.len().max(1);
            if trazadas > TOPE_FORMAS {
                quedan = true;
                break;
            }
            if en.formas.is_empty() {
                let (x0, y0) = a_pantalla([en.caja[0], en.caja[3]]);
                let (x1, y1) = a_pantalla([en.caja[2], en.caja[1]]);
                p.trazar_discontinuo(rect(x0, y0, x1 - x0, y1 - y0), 1.5 * e, tema::ACENTO);
            }
            for f in &en.formas {
                trazar_forma(p, f, a_pantalla, self.camara.px, 2.0 * e, tema::ACENTO);
            }
        }
        if quedan && let Some(c) = self.resalte.total {
            let (x0, y0) = a_pantalla([c[0], c[3]]);
            let (x1, y1) = a_pantalla([c[2], c[1]]);
            p.trazar_discontinuo(rect(x0, y0, x1 - x0, y1 - y0), 1.5 * e, tema::ACENTO);
        }
        // Los pinzamientos.
        let lado = 7.0 * e;
        for (_, _, q) in self.sesion.pinzamientos() {
            let (x, y) = a_pantalla(q);
            let r = rect(x - lado / 2.0, y - lado / 2.0, lado, lado);
            p.rellenar(r, tema::ACENTO);
            p.trazar(r, 1.0 * e, tema::TEXTO);
        }
    }

    fn pintar_cinta(&self, p: &Pintor, d: &Disposicion, e: f32, zonas: &mut Vec<Zona>) {
        let c = d.cinta;
        p.rellenar(c, tema::PANEL);
        p.rellenar(rect(c.x, c.y + c.alto - 1.0, c.ancho, 1.0), tema::BORDE);
        let encima = self.cursor;
        let alto = tema::ALTO_PESTANAS * e;
        let tam = tema::LETRA * e;
        let mut x = 8.0 * e;
        let fila = |x: f32, ancho: f32| rect(x, 5.0 * e, ancho, alto - 10.0 * e);
        // Lo de siempre a la izquierda.
        for (texto, accion, ayuda) in [
            ("Inicio", Accion::Inicio, "Volver a la pantalla de inicio"),
            ("Guardar", Accion::Guardar, "Guardar  Ctrl+S"),
            (
                "Guardar como",
                Accion::GuardarComo,
                "Guardar como DWG o DXF  Ctrl+Mayus+S",
            ),
            ("Deshacer", Accion::Orden(Id::Deshacer), "Deshacer  Ctrl+Z"),
            ("Rehacer", Accion::Orden(Id::Rehacer), "Rehacer  Ctrl+Y"),
            (
                "Ver todo",
                Accion::Encuadre,
                "Ver todo el plano  doble clic central",
            ),
        ] {
            let ancho = p.medir_texto(texto, tam).0 + 20.0 * e;
            ui::boton(
                p,
                zonas,
                fila(x, ancho),
                texto,
                tam,
                Estilo::Plano,
                encima,
                accion,
                ayuda,
            );
            x += ancho + 2.0 * e;
        }
        x += 14.0 * e;
        p.rellenar(
            rect(x - 8.0 * e, 9.0 * e, 1.0, alto - 18.0 * e),
            tema::BORDE,
        );
        for g in pestanas() {
            let texto = g.nombre();
            let ancho = p.medir_texto(texto, tam).0 + 24.0 * e;
            let estilo = if g == self.pestana {
                Estilo::Elegido
            } else {
                Estilo::Plano
            };
            ui::boton(
                p,
                zonas,
                fila(x, ancho),
                texto,
                tam,
                estilo,
                encima,
                Accion::Pestana(g),
                "",
            );
            x += ancho + 2.0 * e;
        }
        // El nombre del archivo, a la derecha.
        let t = self.titulo();
        let (tw, _) = p.medir_texto(&t, tam);
        if c.ancho - tw - 16.0 * e > x {
            p.texto(&t, c.ancho - tw - 16.0 * e, 9.0 * e, tam, tema::TEXTO_SUAVE);
        }
        // Los botones de la pestana.
        let ids = ordenes::de_grupo(self.pestana);
        let mut x = 8.0 * e;
        let y = alto;
        let alto_b = tema::ALTO_BOTONES * e - 8.0 * e;
        if self.pestana == Grupo::Medir {
            // Lo que no es una orden del dibujo: el metrado.
            let texto = "Metrado BIM…";
            let ancho = p.medir_texto(texto, tam).0 + 22.0 * e;
            let r = rect(
                c.ancho - ancho - 12.0 * e,
                y + 8.0 * e,
                ancho,
                alto_b - 16.0 * e,
            );
            ui::boton(
                p,
                zonas,
                r,
                texto,
                tam,
                Estilo::Plano,
                encima,
                Accion::MetradoBim,
                "Metrado de un modelo IFC o Revit a CSV",
            );
            let texto = "Metrado del plano…";
            let ancho2 = p.medir_texto(texto, tam).0 + 22.0 * e;
            let r = rect(
                c.ancho - ancho - ancho2 - 16.0 * e,
                y + 8.0 * e,
                ancho2,
                alto_b - 16.0 * e,
            );
            ui::boton(
                p,
                zonas,
                r,
                texto,
                tam,
                Estilo::Plano,
                encima,
                Accion::MetradoPlano,
                "Cantidad, longitud y area por capa",
            );
        }
        for id in &ids {
            let o = ordenes::de(*id);
            let nombre = ordenes::etiqueta(*id);
            let alias = o.alias.first().copied().unwrap_or("");
            let ancho = p.medir_texto(nombre, tam).0.max(44.0 * e) + 22.0 * e;
            let r = rect(x, y, ancho, alto_b);
            let activa = self.sesion.activa() == Some(*id);
            let sobre = encima.is_some_and(|(cx, cy)| ui::dentro(&r, cx, cy));
            if activa {
                p.rellenar_redondeado(r, 7.0 * e, tema::ACENTO_SUAVE);
            } else if sobre {
                p.rellenar_redondeado(r, 7.0 * e, tema::PANEL_ALTO);
            }
            let (nw, _) = p.medir_texto(nombre, tam);
            p.texto(
                nombre,
                r.x + (r.ancho - nw) / 2.0,
                r.y + 8.0 * e,
                tam,
                if activa { tema::ACENTO } else { tema::TEXTO },
            );
            let ta = tema::LETRA_PEQUENA * e;
            let (aw, _) = p.medir_texto(alias, ta);
            p.texto(
                alias,
                r.x + (r.ancho - aw) / 2.0,
                r.y + 28.0 * e,
                ta,
                tema::TEXTO_APAGADO,
            );
            zonas.push(Zona {
                r,
                accion: Accion::Orden(*id),
                ayuda: o.ayuda.to_string(),
            });
            x += ancho + 4.0 * e;
        }
    }

    fn pintar_panel(&self, p: &Pintor, d: &Disposicion, e: f32, zonas: &mut Vec<Zona>) {
        let r = d.panel;
        if r.ancho <= 0.0 || r.alto <= 0.0 {
            return;
        }
        p.rellenar(r, tema::PANEL);
        p.rellenar(rect(r.x, r.y, 1.0, r.alto), tema::BORDE);
        let encima = self.cursor;
        let (x0, ancho) = (r.x + 12.0 * e, r.ancho - 24.0 * e);
        let mut y = r.y + 12.0 * e;
        let tam = 12.5 * e;
        p.texto("Capas", x0, y, 14.0 * e, tema::TEXTO);
        ui::boton(
            p,
            zonas,
            rect(x0 + ancho - 24.0 * e, y - 3.0 * e, 24.0 * e, 22.0 * e),
            "+",
            15.0 * e,
            Estilo::Plano,
            encima,
            Accion::Orden(Id::Otra(capas::CAPANUEVA)),
            "Capa nueva (escriba su nombre)",
        );
        y += 26.0 * e;
        let dib = &self.sesion.dibujo;
        let actual = dib.capa_actual();
        let alto_fila = 24.0 * e;
        // La mitad de arriba para las capas.
        let limite = r.y + r.alto * 0.5;
        let capas: Vec<_> = dib.doc.layers.iter().collect();
        let caben = (((limite - y) / alto_fila).floor().max(1.0)) as usize;
        self.capas_caben.set(caben);
        let desde = self.capas_desde.min(capas.len().saturating_sub(caben));
        let y_lista = y;
        let conteo = self.conteo.as_ref().map(|(_, c)| c);
        let tam_cuenta = 10.5 * e;
        for l in capas.iter().skip(desde).take(caben) {
            let fila = rect(x0 - 4.0 * e, y, ancho + 8.0 * e, alto_fila - 2.0 * e);
            let es_actual = l.name == actual;
            if es_actual {
                p.rellenar_redondeado(fila, 5.0 * e, tema::ACENTO_SUAVE);
            } else if encima.is_some_and(|(cx, cy)| ui::dentro(&fila, cx, cy)) {
                p.rellenar_redondeado(fila, 5.0 * e, tema::PANEL_ALTO);
            }
            let color = l
                .color
                .index()
                .map(|i| tema::de_aci(i as i16))
                .or_else(|| l.color.rgb().map(|(r, g, b)| tema::rgb(r, g, b)))
                .unwrap_or(tema::TEXTO);
            p.rellenar_redondeado(rect(x0, y + 6.0 * e, 11.0 * e, 11.0 * e), 3.0 * e, color);
            let visible = l.is_visible();
            let bloqueada = l.is_locked();
            // Cuantos objetos tiene, a la derecha (del recuento por version).
            let cuenta = conteo
                .and_then(|c| c.get(&l.name.to_uppercase()))
                .map(|n| n.to_string())
                .unwrap_or_default();
            let (cw, _) = p.medir_texto(&cuenta, tam_cuenta);
            p.texto(
                &cuenta,
                x0 + ancho - 52.0 * e - cw,
                y + 5.0 * e,
                tam_cuenta,
                tema::TEXTO_APAGADO,
            );
            let nombre_ancho = ancho - 78.0 * e - cw;
            let col_nombre = if visible {
                tema::TEXTO
            } else {
                tema::TEXTO_APAGADO
            };
            p.texto(
                &ui::recortar(p, &l.name, tam, nombre_ancho),
                x0 + 20.0 * e,
                y + 3.0 * e,
                tam,
                col_nombre,
            );
            zonas.push(Zona {
                r: rect(fila.x, fila.y, nombre_ancho + 24.0 * e, fila.alto),
                accion: Accion::CapaActual(l.name.clone()),
                ayuda: "Dibujar en esta capa".into(),
            });
            let ojo = rect(x0 + ancho - 46.0 * e, y, 22.0 * e, alto_fila - 2.0 * e);
            ui::boton(
                p,
                zonas,
                ojo,
                "",
                tam,
                Estilo::Plano,
                encima,
                Accion::CapaVisible(l.name.clone()),
                "Encender o apagar",
            );
            // El ojo: un circulo lleno si se ve, hueco si no.
            let (ox, oy) = (ojo.x + ojo.ancho / 2.0, ojo.y + ojo.alto / 2.0);
            if visible {
                p.circulo((ox, oy), 4.5 * e, tema::AMARILLO);
            } else {
                p.anillo((ox, oy), 4.5 * e, 1.2 * e, tema::TEXTO_APAGADO);
            }
            let candado = rect(x0 + ancho - 22.0 * e, y, 22.0 * e, alto_fila - 2.0 * e);
            ui::boton(
                p,
                zonas,
                candado,
                "",
                tam,
                Estilo::Plano,
                encima,
                Accion::CapaBloqueo(l.name.clone()),
                "Bloquear: se ve pero no se elige",
            );
            // El candado: cerrado (relleno) o abierto (el arco a un lado).
            let (kx, ky) = (
                candado.x + candado.ancho / 2.0,
                candado.y + candado.alto / 2.0,
            );
            let cuerpo = rect(kx - 4.5 * e, ky - 1.0 * e, 9.0 * e, 6.5 * e);
            let col = if bloqueada {
                tema::NARANJA
            } else {
                tema::TEXTO_APAGADO
            };
            if bloqueada {
                p.rellenar(cuerpo, col);
                p.polilinea(
                    &[
                        (kx - 3.0 * e, ky - 1.0 * e),
                        (kx - 3.0 * e, ky - 5.0 * e),
                        (kx + 3.0 * e, ky - 5.0 * e),
                        (kx + 3.0 * e, ky - 1.0 * e),
                    ],
                    1.3 * e,
                    col,
                );
            } else {
                p.trazar(cuerpo, 1.2 * e, col);
                p.polilinea(
                    &[
                        (kx - 3.0 * e, ky - 1.0 * e),
                        (kx - 3.0 * e, ky - 5.0 * e),
                        (kx + 3.0 * e, ky - 5.0 * e),
                        (kx + 3.0 * e, ky - 3.5 * e),
                    ],
                    1.2 * e,
                    col,
                );
            }
            y += alto_fila;
        }
        if capas.len() > caben {
            // La barra de desplazamiento (la lista se mueve con la rueda).
            let total = capas.len() as f32;
            let alto_lista = caben as f32 * alto_fila;
            let barra = rect(
                r.x + r.ancho - 6.0 * e,
                y_lista + alto_lista * desde as f32 / total,
                3.0 * e,
                (alto_lista * caben as f32 / total).max(12.0 * e),
            );
            p.rellenar_redondeado(barra, 1.5 * e, tema::TEXTO_APAGADO);
            p.texto(
                &format!(
                    "{}–{} de {} capas (rueda para ver mas)",
                    desde + 1,
                    (desde + caben).min(capas.len()),
                    capas.len()
                ),
                x0,
                y + 2.0 * e,
                11.0 * e,
                tema::TEXTO_APAGADO,
            );
        }

        // Propiedades de lo elegido.
        let mut y = limite + 10.0 * e;
        p.rellenar(
            rect(r.x + 12.0 * e, y - 8.0 * e, r.ancho - 24.0 * e, 1.0),
            tema::BORDE,
        );
        p.texto("Propiedades", x0, y, 14.0 * e, tema::TEXTO);
        y += 26.0 * e;
        let sel = &self.sesion.seleccion;
        let filas = propiedades(&self.sesion);
        if sel.is_empty() {
            p.texto_linea(
                "Nada elegido. Haz clic en un objeto o arrastra un recuadro (a la derecha: ventana; a la izquierda: captura).",
                x0,
                y,
                11.5 * e,
                ancho,
                tema::TEXTO_APAGADO,
            );
            y += 52.0 * e;
        } else {
            for (k, v) in filas {
                p.texto(&k, x0, y, 11.5 * e, tema::TEXTO_SUAVE);
                p.texto(
                    &ui::recortar(p, &v, 12.0 * e, ancho - 96.0 * e),
                    x0 + 96.0 * e,
                    y,
                    12.0 * e,
                    tema::TEXTO,
                );
                y += 20.0 * e;
            }
            y += 6.0 * e;
            let b = rect(x0, y, ancho, 28.0 * e);
            ui::boton(
                p,
                zonas,
                b,
                &format!(
                    "Pasar a la capa «{}»",
                    ui::recortar(p, &actual, 12.0 * e, ancho - 140.0 * e)
                ),
                12.0 * e,
                Estilo::Plano,
                encima,
                Accion::PasarACapa(actual.clone()),
                "Cambiar la capa de lo elegido a la actual",
            );
            y += 34.0 * e;
        }
        let b = rect(
            x0,
            (y + 6.0 * e).min(r.y + r.alto - 36.0 * e),
            ancho,
            30.0 * e,
        );
        let texto = if sel.is_empty() {
            "Metrado del plano (CSV)…"
        } else {
            "Metrado de lo elegido (CSV)…"
        };
        ui::boton(
            p,
            zonas,
            b,
            texto,
            12.0 * e,
            Estilo::Principal,
            encima,
            Accion::MetradoPlano,
            "Cantidad, longitud y area por capa",
        );
    }

    fn pintar_ordenes(&self, p: &Pintor, d: &Disposicion, e: f32) {
        let r = d.ordenes;
        p.rellenar(r, tema::rgb(0x18, 0x18, 0x1B));
        p.rellenar(rect(r.x, r.y, r.ancho, 1.0), tema::BORDE);
        let tam = 12.0 * e;
        let x = 12.0 * e;
        let mut y = r.y + 4.0 * e;
        let reg = &self.sesion.registro;
        let lineas = self.lineas();
        let desde = reg.len().saturating_sub(lineas);
        for _ in 0..lineas.saturating_sub(reg.len() - desde) {
            y += tema::ALTO_LINEA * e;
        }
        for l in &reg[desde..] {
            p.texto(
                &ui::recortar(p, l, tam, r.ancho - 24.0 * e),
                x,
                y,
                tam,
                tema::TEXTO_SUAVE,
            );
            y += tema::ALTO_LINEA * e;
        }
        // La linea donde se escribe.
        let caja = rect(
            6.0 * e,
            y + 1.0 * e,
            r.ancho - 12.0 * e,
            tema::ALTO_ORDEN * e - 2.0 * e,
        );
        p.rellenar_redondeado(caja, 6.0 * e, tema::PANEL);
        let indicacion = self.sesion.indicacion();
        let ty = caja.y + (caja.alto - 13.0 * e * 1.3) / 2.0;
        p.texto(&indicacion, caja.x + 10.0 * e, ty, 13.0 * e, tema::ACENTO);
        let (iw, _) = p.medir_texto(&indicacion, 13.0 * e);
        let xe = caja.x + 10.0 * e + iw + 8.0 * e;
        p.texto(&self.escrito, xe, ty, 13.0 * e, tema::TEXTO);
        let (ew, eh) = p.medir_texto(
            if self.escrito.is_empty() {
                "M"
            } else {
                &self.escrito
            },
            13.0 * e,
        );
        let xc = xe + if self.escrito.is_empty() { 0.0 } else { ew } + 1.0 * e;
        p.rellenar(rect(xc, ty + 1.0 * e, 1.5 * e, eh), tema::TEXTO);
        // Sin orden: lo que se puede querer decir.
        if self.sesion.activa().is_none() && !self.escrito.is_empty() {
            let s: Vec<String> = ordenes::sugerencias(&self.escrito)
                .into_iter()
                .take(6)
                .map(|o| format!("{} ({})", o.nombre, o.alias.first().copied().unwrap_or("")))
                .collect();
            if !s.is_empty() {
                p.texto(
                    &format!("→ {}", s.join("   ")),
                    xc + 18.0 * e,
                    ty + 1.0 * e,
                    11.5 * e,
                    tema::TEXTO_APAGADO,
                );
            }
        }
    }

    fn pintar_estado(
        &self,
        p: &Pintor,
        d: &Disposicion,
        e: f32,
        punto: Option<P>,
        zonas: &mut Vec<Zona>,
    ) {
        let r = d.estado;
        p.rellenar(r, tema::PANEL);
        let tam = 11.5 * e;
        let ins = self.sesion.dibujo.doc.header.insertion_units;
        let unidad = match ins {
            4 => "mm",
            5 => "cm",
            6 => "m",
            1 => "pulg",
            2 => "pies",
            _ => "unid.",
        };
        let coords = match punto {
            Some(q) => format!("{:.2}, {:.2}  {unidad}", q[0], q[1]),
            None => unidad.to_string(),
        };
        p.texto(&coords, 12.0 * e, r.y + 6.0 * e, tam, tema::TEXTO_SUAVE);
        let a = &self.sesion.ajustes;
        let mut x = 230.0 * e;
        let alto = r.alto - 6.0 * e;
        for (texto, encendido, i, ayuda) in [
            (
                "ENGANCHE",
                a.enganche,
                Interruptor::Enganche,
                "Referencias a objetos  F3",
            ),
            (
                "ORTO",
                a.orto,
                Interruptor::Orto,
                "Solo horizontal y vertical  F8",
            ),
            (
                "POLAR",
                a.polar.is_some(),
                Interruptor::Polar,
                "Rastreo polar cada 45°  F10",
            ),
            (
                "REJILLA",
                a.rejilla.is_some(),
                Interruptor::Rejilla,
                "Pegar a la rejilla  F9",
            ),
        ] {
            let ancho = p.medir_texto(texto, tam).0 + 18.0 * e;
            let estilo = if encendido {
                Estilo::Elegido
            } else {
                Estilo::Plano
            };
            ui::boton(
                p,
                zonas,
                rect(x, r.y + 3.0 * e, ancho, alto),
                texto,
                tam,
                estilo,
                self.cursor,
                Accion::Alternar(i),
                ayuda,
            );
            x += ancho + 3.0 * e;
        }
        x += 12.0 * e;
        // Los enganches, uno por uno.
        for t in Tipo::TODOS {
            let corto = match t {
                Tipo::Extremo => "Fin",
                Tipo::Medio => "Med",
                Tipo::Centro => "Cen",
                Tipo::Cuadrante => "Cua",
                Tipo::Interseccion => "Int",
                Tipo::Insercion => "Ins",
                Tipo::Perpendicular => "Per",
                Tipo::Cercano => "Cer",
            };
            let ancho = p.medir_texto(corto, tam).0 + 12.0 * e;
            let estilo = if a.enganche && a.modos.tiene(t) {
                Estilo::Elegido
            } else {
                Estilo::Plano
            };
            ui::boton(
                p,
                zonas,
                rect(x, r.y + 3.0 * e, ancho, alto),
                corto,
                tam,
                estilo,
                self.cursor,
                Accion::Enganche(t),
                t.nombre(),
            );
            x += ancho + 2.0 * e;
        }
        let n = self.sesion.seleccion.len();
        if n > 0 {
            let t = format!("{n} elegidos");
            let (tw, _) = p.medir_texto(&t, tam);
            p.texto(
                &t,
                r.ancho - tw - 12.0 * e,
                r.y + 6.0 * e,
                tam,
                tema::ACENTO,
            );
        }
    }
}

/// La zona de la lista de capas dentro del panel (para la rueda).
fn zona_capas(panel: RectF, e: f32) -> RectF {
    let arriba = panel.y + 38.0 * e;
    rect(
        panel.x,
        arriba,
        panel.ancho,
        (panel.y + panel.alto * 0.5 - arriba).max(0.0),
    )
}

/// La vista, las capas del panel, el doble clic y la rejilla.
impl Mesa {
    /// Apunta la vista de ahora para ZOOM Previo.
    pub fn guardar_vista(&mut self) {
        if self.vistas.last() != Some(&self.camara) {
            self.vistas.push(self.camara);
            if self.vistas.len() > VISTAS {
                self.vistas.remove(0);
            }
        }
    }

    /// **Atiende lo que pidio una orden a la vista** (ZOOM, ZE). Dice si cambio.
    pub fn atender_vista(&mut self, w: f32, h: f32, e: f32) -> bool {
        let Some(pedido) = self.sesion.memoria.vista.take() else {
            return false;
        };
        let d = self.disp(w, h, e).dibujo;
        let zona = [d.x as f64, d.y as f64, d.ancho as f64, d.alto as f64];
        let (wd, hd) = (w as f64, h as f64);
        match pedido {
            PedidoVista::Extension => self.encuadrar(w, h, e),
            PedidoVista::Todo => {
                let hd_ = &self.sesion.dibujo.doc.header;
                let limites = [
                    hd_.model_space_limits_min.x,
                    hd_.model_space_limits_min.y,
                    hd_.model_space_limits_max.x,
                    hd_.model_space_limits_max.y,
                ];
                let caja = [self.sesion.dibujo.indice().caja(), self.caja_plano]
                    .into_iter()
                    .flatten()
                    .chain(cad_nucleo::geo::caja_valida(&limites).then_some(limites))
                    .reduce(cad_nucleo::geo::unir_cajas);
                match caja {
                    Some(c) => {
                        self.guardar_vista();
                        self.camara = Camara::encuadrar(c, zona, wd, hd);
                    }
                    None => self.encuadrar(w, h, e),
                }
            }
            PedidoVista::Ventana(a, b) => {
                self.guardar_vista();
                let c = [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[0].max(b[0]),
                    a[1].max(b[1]),
                ];
                self.camara = Camara::encuadrar(c, zona, wd, hd);
            }
            PedidoVista::Previa => match self.vistas.pop() {
                Some(c) => self.camara = c,
                None => self.sesion.avisar("No hay una vista anterior"),
            },
            PedidoVista::Factor(k) => {
                self.guardar_vista();
                let (cx, cy) = (zona[0] + zona[2] / 2.0, zona[1] + zona[3] / 2.0);
                self.camara = self.camara.zoom(1.0 / k, cx, cy, wd, hd);
            }
        }
        true
    }

    /// Si el raton esta sobre la lista de capas (la rueda la mueve a ella).
    pub fn en_capas(&self, x: f32, y: f32, w: f32, h: f32, e: f32) -> bool {
        ui::dentro(&zona_capas(self.disp(w, h, e).panel, e), x, y)
    }

    /// La rueda sobre la lista de capas: tres filas por paso.
    pub fn rueda_capas(&mut self, pasos: f32) {
        let filas = (pasos.abs() * 3.0).round().max(1.0) as usize;
        let total = self.sesion.dibujo.doc.layers.len();
        let tope = total.saturating_sub(self.capas_caben.get().min(total));
        self.capas_desde = if pasos > 0.0 {
            self.capas_desde.min(tope).saturating_sub(filas)
        } else {
            (self.capas_desde + filas).min(tope)
        };
    }

    /// Vuelve a contar los objetos de cada capa si el dibujo cambio (no en
    /// cada fotograma: una vez por version).
    fn contar_capas(&mut self) {
        let v = self.sesion.dibujo.version();
        if self.conteo.as_ref().is_some_and(|(c, _)| *c == v) {
            return;
        }
        let mut m: HashMap<String, usize> = HashMap::new();
        for e in self.sesion.dibujo.doc.model_space_entities() {
            let capa = &e.common().layer;
            match m.get_mut(capa) {
                Some(n) => *n += 1,
                None => {
                    m.insert(capa.clone(), 1);
                }
            }
        }
        // Las capas no distinguen mayusculas.
        let mut juntas: HashMap<String, usize> = HashMap::with_capacity(m.len());
        for (k, n) in m {
            *juntas.entry(k.to_uppercase()).or_default() += n;
        }
        self.conteo = Some((v, juntas));
    }

    /// **Doble clic** en el plano: en un texto, EDITARTEXTO (si esta). Dice
    /// si lo atendio.
    pub fn doble_clic(&mut self, x: f32, y: f32, w: f32, h: f32, e: f32) -> bool {
        if self.sesion.activa().is_some() {
            return false;
        }
        let Some(ed) = ordenes::buscar("ED") else {
            return false;
        };
        let tol = self.camara.px * (tema::CAJA_CURSOR * e) as f64 / 2.0 + self.camara.px;
        let p = self.a_plano(x, y, w, h);
        let Some(hd) = cad_nucleo::seleccion::elegir(&self.sesion.dibujo.indice(), p, tol) else {
            return false;
        };
        if !matches!(
            self.sesion.dibujo.entidad(hd),
            Some(EntityType::Text(_) | EntityType::MText(_))
        ) {
            return false;
        }
        self.abajo = None;
        self.sesion.seleccion = vec![hd];
        self.sesion.orden(ed.id);
        true
    }

    /// La rejilla pintada (F7): rayas tenues cada paso de la rejilla (o uno
    /// redondo a la escala de lo que se ve), con una mas marcada cada cinco.
    fn pintar_rejilla(&self, p: &Pintor, zona: &RectF, w: f32, h: f32, e: f32) {
        let px = self.camara.px;
        let mut paso = self
            .sesion
            .ajustes
            .rejilla
            .unwrap_or_else(|| paso_redondo(px * 20.0));
        if !(paso > 0.0 && paso.is_finite()) {
            return;
        }
        // Nunca mas de una raya cada 8 pixeles.
        while paso / px < 8.0 * e as f64 {
            paso *= 5.0;
        }
        let a = self.a_plano(zona.x, zona.y + zona.alto, w, h);
        let b = self.a_plano(zona.x + zona.ancho, zona.y, w, h);
        let (wd, hd) = (w as f64, h as f64);
        let tenue = tema::rgba(0xF5, 0xF5, 0xF7, 0.05);
        let marcada = tema::rgba(0xF5, 0xF5, 0xF7, 0.11);
        let color = |k: f64| {
            if (k / paso).round().rem_euclid(5.0) == 0.0 {
                marcada
            } else {
                tenue
            }
        };
        let mut x = (a[0] / paso).floor() * paso;
        while x <= b[0] {
            let (sx, _) = self.camara.pantalla([x, 0.0], wd, hd);
            p.linea((sx, zona.y), (sx, zona.y + zona.alto), 1.0, color(x));
            x += paso;
        }
        let mut y = (a[1] / paso).floor() * paso;
        while y <= b[1] {
            let (_, sy) = self.camara.pantalla([0.0, y], wd, hd);
            p.linea((zona.x, sy), (zona.x + zona.ancho, sy), 1.0, color(y));
            y += paso;
        }
    }
}

/// La caja de un modelo ya convertido, en coordenadas absolutas.
pub fn caja_de_modelo(m: &pixpin_cad::modelo::Modelo) -> [f64; 4] {
    let c = m.caja_util();
    [
        c[0] as f64 + m.origen[0],
        c[1] as f64 + m.origen[1],
        c[2] as f64 + m.origen[0],
        c[3] as f64 + m.origen[1],
    ]
}

/// Un paso de rejilla redondo (1, 2 o 5 por una potencia de diez).
fn paso_redondo(x: f64) -> f64 {
    if !(x > 0.0 && x.is_finite()) {
        return 1.0;
    }
    let p = 10f64.powf(x.log10().floor());
    let m = x / p;
    p * if m < 1.5 {
        1.0
    } else if m < 3.5 {
        2.0
    } else if m < 7.5 {
        5.0
    } else {
        10.0
    }
}

/// El color con que se ve una entidad: el suyo o el de su capa (el 7,
/// blanco sobre el fondo oscuro).
fn color_de_entidad(d: &Dibujo, e: &EntityType) -> Color {
    use cad_nucleo::opencadcodec::Color as C;
    let de = |c: &C| match c {
        C::Index(i) => Some(tema::de_aci(*i as i16)),
        C::Rgb { r, g, b } => Some(tema::rgb(*r, *g, *b)),
        _ => None,
    };
    let comun = e.common();
    de(&comun.color)
        .or_else(|| d.doc.layers.get(&comun.layer).and_then(|l| de(&l.color)))
        .unwrap_or(tema::TEXTO)
}

/// Las propiedades de lo elegido: tipo, capa y lo que mide.
pub fn propiedades(s: &Sesion) -> Vec<(String, String)> {
    let sel = &s.seleccion;
    let mut v = Vec::new();
    if sel.len() != 1 {
        v.push(("Elegidos".into(), sel.len().to_string()));
        let mut largo = 0.0;
        let mut area = 0.0;
        for h in sel {
            if let Some(e) = s.dibujo.entidad(*h) {
                let (l, a) = formas::largo_y_area(e);
                largo += l;
                area += a.unwrap_or(0.0);
            }
        }
        v.push(("Longitud".into(), format!("{largo:.2}")));
        if area > 0.0 {
            v.push(("Area".into(), format!("{area:.2}")));
        }
        return v;
    }
    let Some(e) = s.dibujo.entidad(sel[0]) else {
        return v;
    };
    v.push(("Tipo".into(), nombre_de_tipo(e).into()));
    v.push(("Capa".into(), e.common().layer.clone()));
    match e {
        EntityType::Line(l) => {
            v.push(("Longitud".into(), format!("{:.2}", l.length())));
            let ang = (l.end.y - l.start.y)
                .atan2(l.end.x - l.start.x)
                .to_degrees()
                .rem_euclid(360.0);
            v.push(("Angulo".into(), format!("{ang:.2}°")));
        }
        EntityType::Circle(c) => {
            v.push(("Radio".into(), format!("{:.2}", c.radius)));
            v.push(("Area".into(), format!("{:.2}", c.area())));
        }
        EntityType::Arc(a) => {
            v.push(("Radio".into(), format!("{:.2}", a.radius)));
            v.push(("Longitud".into(), format!("{:.2}", a.arc_length())));
        }
        EntityType::Text(t) => {
            v.push(("Texto".into(), t.value.clone()));
            v.push(("Altura".into(), format!("{:.2}", t.height)));
        }
        otro => {
            let (l, a) = formas::largo_y_area(otro);
            if l > 0.0 {
                v.push(("Longitud".into(), format!("{l:.2}")));
            }
            if let Some(a) = a {
                v.push(("Area".into(), format!("{a:.2}")));
            }
        }
    }
    v
}

fn nombre_de_tipo(e: &EntityType) -> &'static str {
    match e {
        EntityType::Line(_) => "Línea",
        EntityType::Circle(_) => "Círculo",
        EntityType::Arc(_) => "Arco",
        EntityType::LwPolyline(_) => "Polilínea",
        EntityType::Text(_) => "Texto",
        EntityType::MText(_) => "Texto de párrafo",
        EntityType::Insert(_) => "Bloque",
        EntityType::Dimension(_) => "Cota",
        EntityType::Hatch(_) => "Sombreado",
        EntityType::Point(_) => "Punto",
        EntityType::Solid(_) => "Sólido 2D",
        EntityType::Spline(_) => "Spline",
        EntityType::Ellipse(_) => "Elipse",
        _ => "Objeto",
    }
}

/// Pinta una forma del dibujo en pantalla.
fn trazar_forma(
    p: &Pintor,
    f: &Forma,
    a_pantalla: &impl Fn(P) -> (f32, f32),
    px: f64,
    grosor: f32,
    color: Color,
) {
    match *f {
        Forma::Segmento { a, b } => p.linea(a_pantalla(a), a_pantalla(b), grosor, color),
        Forma::Arco { r, .. } | Forma::Circulo { r, .. } => {
            // Tantos tramos como pida el tamano en pantalla.
            let n = ((r / px).sqrt() * 4.0).clamp(12.0, 160.0) as usize;
            let pts: Vec<(f32, f32)> = f.muestras(n).into_iter().map(a_pantalla).collect();
            p.polilinea(&pts, grosor, color);
        }
    }
}

/// El simbolo de cada enganche (cuadrado, triangulo, circulo, aspa…).
fn pintar_enganche(p: &Pintor, g: &Enganche, a_pantalla: &impl Fn(P) -> (f32, f32), e: f32) {
    let (x, y) = a_pantalla(g.p);
    let s = 6.0 * e;
    let c = tema::AMARILLO;
    let gr = 1.8 * e;
    match g.tipo {
        Tipo::Extremo => p.trazar(rect(x - s, y - s, 2.0 * s, 2.0 * s), gr, c),
        Tipo::Medio => p.polilinea(
            &[(x, y - s), (x + s, y + s), (x - s, y + s), (x, y - s)],
            gr,
            c,
        ),
        Tipo::Centro => p.anillo((x, y), s, gr, c),
        Tipo::Cuadrante => p.polilinea(
            &[(x, y - s), (x + s, y), (x, y + s), (x - s, y), (x, y - s)],
            gr,
            c,
        ),
        Tipo::Interseccion => {
            p.linea((x - s, y - s), (x + s, y + s), gr, c);
            p.linea((x - s, y + s), (x + s, y - s), gr, c);
        }
        Tipo::Insercion => {
            p.trazar(rect(x - s, y - s, s, s), gr, c);
            p.trazar(rect(x, y, s, s), gr, c);
        }
        Tipo::Perpendicular => {
            p.polilinea(&[(x - s, y - s), (x - s, y + s), (x + s, y + s)], gr, c)
        }
        Tipo::Cercano => p.polilinea(
            &[
                (x - s, y - s),
                (x + s, y - s),
                (x - s, y + s),
                (x + s, y + s),
                (x - s, y - s),
            ],
            gr,
            c,
        ),
    }
    let t = g.tipo.nombre();
    p.texto_con_fondo(
        t,
        x + 12.0 * e,
        y - 26.0 * e,
        10.5 * e,
        tema::AMARILLO,
        tema::rgba(0x0E, 0x0E, 0x10, 0.85),
    );
}

/// La cruz del cursor de un CAD, con el cuadradito de eleccion.
fn pintar_cursor(p: &Pintor, x: f32, y: f32, zona: &RectF, e: f32, eligiendo: bool) {
    let c = tema::rgba(0xF5, 0xF5, 0xF7, 0.75);
    let g = 1.0 * e;
    p.linea(
        (zona.x, y),
        (zona.x + zona.ancho, y),
        g,
        tema::rgba(0xF5, 0xF5, 0xF7, 0.22),
    );
    p.linea(
        (x, zona.y),
        (x, zona.y + zona.alto),
        g,
        tema::rgba(0xF5, 0xF5, 0xF7, 0.22),
    );
    let brazo = 18.0 * e;
    p.linea((x - brazo, y), (x + brazo, y), g, c);
    p.linea((x, y - brazo), (x, y + brazo), g, c);
    let s = tema::CAJA_CURSOR * e / 2.0;
    p.trazar(
        rect(x - s, y - s, 2.0 * s, 2.0 * s),
        g,
        if eligiendo { tema::ACENTO } else { c },
    );
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_disposicion_reparte_toda_la_ventana() {
        let d = disposicion(1600.0, 900.0, 1.0);
        assert!((d.dibujo.ancho + d.panel.ancho - 1600.0).abs() < 1e-3);
        assert!(
            (d.cinta.alto + d.dibujo.alto + d.ordenes.alto + d.estado.alto - 900.0).abs() < 1e-3
        );
        // Ventana diminuta: nada negativo.
        let d = disposicion(100.0, 50.0, 2.0);
        assert!(d.dibujo.alto >= 0.0 && d.dibujo.ancho >= 0.0);
    }

    #[test]
    fn pasos_redondos() {
        assert_eq!(paso_redondo(13.0), 10.0);
        assert_eq!(paso_redondo(27.0), 20.0);
        assert_eq!(paso_redondo(480.0), 500.0);
        assert_eq!(paso_redondo(-1.0), 1.0);
    }

    #[test]
    fn todas_las_ordenes_de_la_cinta_tienen_nombre() {
        for g in pestanas() {
            for id in ordenes::de_grupo(g) {
                let id = &id;
                assert!(!ordenes::etiqueta(*id).is_empty());
                assert_eq!(ordenes::de(*id).id, *id);
            }
        }
    }
}
