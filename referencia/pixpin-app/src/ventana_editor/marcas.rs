//! **Los marcadores del lienzo y su riel** (F5), en el editor.
//!
//! Las cuentas estan en `pixpin_motor2d::marcas` (el orden, la linea que se
//! guarda, adonde se lleva la camara) y la colocacion del riel en
//! `pixpin_ui::riel_marcas`; aqui solo queda lo que toca la ventana: atender
//! el raton y el teclado, pintar, guardar junto al dibujo y el viaje de la
//! camara.
//!
//! # Como se usa
//!
//! - **Poner**: el boton del marcador del riel (arriba a la derecha) o
//!   **Ctrl+M** abren la tira de emoticonos bajo la barra; se elige uno (o su
//!   numero, 1-9 y 0) y el siguiente clic en el lienzo la planta ahi. Escape
//!   deja de ponerla.
//! - **Ir**: un clic en su punto del riel, o **Ctrl+1…9** para las nueve
//!   primeras. La camara viaja hasta dejarla arriba y centrada, como en el
//!   movil. Un clic en la marca misma, con cualquier herramienta menos la
//!   mano, hace lo mismo.
//! - **Mover**: solo con la mano (la herramienta de seleccion). En el movil
//!   el dedo que pasaba dibujando se llevaba la marca puesta (v0.81.2), y
//!   aqui pasaria lo mismo con el raton.
//! - **Quitar**: clic derecho encima de la marca o de su punto del riel. En
//!   el movil es el toque largo, que un raton no tiene.
//!
//! # Por que la marca se pinta con la escena y el riel con la interfaz
//!
//! La marca esta pegada a un punto del dibujo: tiene que moverse con el
//! lienzo **mientras** se mueve (lo pidio el usuario, v0.81.1: en el movil se
//! quedaba clavada en la pantalla hasta soltar). Con A3 un paneo no repinta:
//! corre el visual de la escena. Si la marca fuera en la capa de la interfaz
//! se quedaria quieta mientras el dibujo se va. Pintada dentro de la escena,
//! va con el. El riel es lo contrario: va fijo a la derecha, asi que va en la
//! interfaz, que no se mueve.
//!
//! # Donde se guardan
//!
//! En un fichero hermano del dibujo, `<nombre>.pixpin-marcas`, con la misma
//! linea que el movil guarda en sus ajustes (`id:x:y:emoji|…`, ver
//! `pixpin_motor2d::marcas`). Es lo que ya hace el visor con sus marcadores
//! (`pixpin_docs::lectura`). Quien abre el editor dice de que dibujo se trata
//! con [`junto_a`]; sin eso (el lienzo en blanco de la bandeja) las marcas
//! valen mientras dura la ventana.

use pixpin_geom::{Punto, Rect};
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::marcas::{self, EMOJIS, Marca, Vuelo};
use pixpin_motor2d::vector::Punto2;
use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin};
use pixpin_store::Catalogo;
use pixpin_ui::riel_marcas::{DestinoRiel, PUNTO_CERCA, PUNTO_ELEGIDO, PUNTO_REPOSO, Riel, SALTO};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::caja_dibujo::{hex, sombra_isla};

/// El radio del redondel de una marca sobre el lienzo, en pixeles logicos:
/// el de 36 dp del movil.
const RADIO_MARCA: f32 = 18.0;
/// Por debajo de la barra: la marca a la que se va queda a esto del borde de
/// abajo de la barra, que es donde se lee sin que nada la tape.
const AIRE_BAJO_LA_BARRA: f32 = 48.0;

const VK_ESCAPE: u32 = 0x1B;
const VK_M: u32 = b'M' as u32;
const VK_N: u32 = b'N' as u32;

/// El fondo oscuro de los redondeles y de la tira, el `#14182B` del movil.
const OSCURO: Color = hex(0x14182b);
const ISLA: Color = hex(0xffffff);
const ICONO: Color = hex(0x1b1b1f);
const ACTIVO_FONDO: Color = hex(0xe0dfff);
const ACTIVO_ICONO: Color = hex(0x030064);
const HOVER: Color = hex(0xf1f0ff);

fn con_alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

thread_local! {
    /// El dibujo que tiene abierto el editor de este hilo. El editor corre en
    /// el hilo de quien lo llama y no vuelve hasta cerrarse, asi que un
    /// valor por hilo es exactamente «el dibujo de esta ventana».
    static DIBUJO: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Dice de que dibujo son las marcas mientras el valor vive. Se llama justo
/// antes de abrir el editor y se suelta al volver:
///
/// ```ignore
/// let _marcas = crate::ventana_editor::marcas::junto_a(&ruta);
/// ventana_editor::abrir(...)
/// ```
///
/// Es una guarda y no un parametro mas de `abrir` para no cambiar a los
/// cinco sitios que ya lo llaman: quien no la pone sigue funcionando igual,
/// solo que sus marcas no se guardan.
pub(crate) fn junto_a(dibujo: &Path) -> Junto {
    DIBUJO.with(|d| *d.borrow_mut() = Some(dibujo.to_path_buf()));
    Junto(())
}

pub(crate) struct Junto(());

impl Drop for Junto {
    fn drop(&mut self) {
        DIBUJO.with(|d| *d.borrow_mut() = None);
    }
}

/// El fichero hermano donde viven las marcas de un dibujo.
pub(crate) fn ruta_de_marcas(dibujo: &Path) -> PathBuf {
    let mut nombre = dibujo
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "dibujo".into());
    nombre.push_str(".pixpin-marcas");
    // Un lienzo de un chat las lleva junto a su dibujo del movil
    // (`<dibujo>.marcas`, v0.96), que viaja con el al sincronizar.
    crate::anotado_del_adjunto::marcas_del_lienzo(dibujo, dibujo.with_file_name(nombre))
}

/// Las marcas de un dibujo. Sin fichero, o roto, ninguna: no tener marcas es
/// lo normal.
pub(crate) fn leer(dibujo: &Path) -> Vec<Marca> {
    std::fs::read_to_string(ruta_de_marcas(dibujo))
        .map(|t| marcas::de_texto(t.trim()))
        .unwrap_or_default()
}

/// Las guarda junto al dibujo. Sin marcas se borra el fichero: dejar uno
/// vacio al lado de cada dibujo que alguna vez tuvo una seria basura.
pub(crate) fn escribir(dibujo: &Path, lista: &[Marca]) -> std::io::Result<()> {
    let ruta = ruta_de_marcas(dibujo);
    if !ruta.to_string_lossy().ends_with(".pixpin-marcas") {
        // La que viaja no se borra al vaciarla: se deja vacia, como el movil
        // (un fichero que desaparece no le dice nada al otro aparato).
        return pixpin_proyecto::anotado::escribir(&ruta, &marcas::a_texto(lista));
    }
    if lista.is_empty() {
        return match std::fs::remove_file(&ruta) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    // A uno de al lado y luego se cambia el nombre, como el dibujo: si se va
    // la luz a media escritura quedan las marcas de antes, no media linea.
    let temporal = ruta.with_extension("pixpin-marcas.tmp");
    std::fs::write(&temporal, marcas::a_texto(lista))?;
    std::fs::rename(&temporal, &ruta)
}

/// Los textos de la tira. El editor no recibe el catalogo de la aplicacion;
/// se toma el idioma del sistema, como el panel lateral.
pub(super) fn textos() -> &'static Catalogo {
    static TEXTOS: OnceLock<Catalogo> = OnceLock::new();
    TEXTOS.get_or_init(|| {
        Catalogo::nuevo(pixpin_store::resolver_idioma(
            &pixpin_shell::entorno::locale_del_sistema(),
            pixpin_store::ajustes::PreferenciaIdioma::Sistema,
        ))
    })
}

/// En que anda la puesta de una marca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Poniendo {
    /// Nada: el riel en reposo.
    No,
    /// La tira abierta, esperando emoticono.
    Eligiendo,
    /// Emoticono elegido (su indice): el siguiente clic en el lienzo la
    /// planta.
    Plantando(usize),
}

/// Lo que necesita saber el riel del editor en cada evento.
pub(super) struct Contexto<'a> {
    /// El rectangulo de la ventana en el escritorio: los eventos llegan en
    /// coordenadas del escritorio y el riel vive en las de la ventana.
    pub area: Rect,
    pub escala_por_cien: u32,
    /// La camara que pinta (con la escala de Windows) y la del usuario.
    pub efectiva: &'a Camara,
    pub camara: &'a Camara,
    /// Si la herramienta es la mano: solo con ella se arrastran las marcas.
    pub mano: bool,
    /// Escribiendo un texto las teclas son del texto.
    pub escribiendo: bool,
    /// El gesto esta a medias (un trazo, un arrastre): el raton que pasa
    /// por encima del riel sigue siendo del trazo, o saldria cortado.
    pub ocupado: bool,
    /// La y de ventana del borde de abajo de la barra de herramientas.
    pub bajo_la_barra: i32,
}

/// Lo que el editor tiene que hacer despues.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Respuesta {
    /// El evento era del riel o de una marca: no sigue hacia el gesto.
    pub consumido: bool,
    /// Algo de lo pintado cambio (una marca, el riel o la tira).
    pub repintar: bool,
    pub cursor: Option<FormaCursorWin>,
    /// Se pidio el marco para imprimir (`marco_imprimir`).
    pub imprimir: bool,
    /// Se pidio la hojita (F6).
    pub hojita: bool,
    /// Se pulso el lapiz del riel: de mirar a editar y vuelta (24-oct).
    pub editar: bool,
    /// Se pulso la ficha del marco `i`: ir a el.
    pub ir_a_marco: Option<usize>,
}

impl Respuesta {
    fn suya(repintar: bool) -> Self {
        Self {
            consumido: true,
            repintar,
            ..Self::default()
        }
    }
}

pub(super) struct Marcador {
    pub marcas: Vec<Marca>,
    dibujo: Option<PathBuf>,
    pub poniendo: Poniendo,
    /// La marca que se arrastra y donde va, en pixeles de ventana.
    arrastre: Option<(i64, Punto2)>,
    /// Que hay bajo el raton en el riel, para agrandarlo.
    encima: DestinoRiel,
    vuelo: Option<(Vuelo, std::time::Instant)>,
    /// Si se estan poniendo marcos (la herramienta Marco): el boton del
    /// marco del riel, pulsado. Lo pone el editor en cada vuelta.
    pub enmarcando: bool,
    /// Si se esta editando: el lapiz del riel, pulsado. Tambien lo pone el
    /// editor en cada vuelta.
    pub editando: bool,
    /// Los marcos del dibujo en su orden (x, y, ancho, alto del mundo): las
    /// fichas 1, 2, 3... de abajo, con la herramienta Marco.
    pub marcos: Vec<[f32; 4]>,
    /// El nombre de cada uno de `marcos` (vacio si no tiene).
    pub nombres: Vec<String>,
}

impl Marcador {
    /// Las marcas del dibujo que se abre (ver [`junto_a`]).
    pub fn cargar() -> Self {
        let dibujo = DIBUJO.with(|d| d.borrow().clone());
        let marcas = dibujo.as_deref().map(leer).unwrap_or_default();
        Self {
            marcas,
            dibujo,
            poniendo: Poniendo::No,
            arrastre: None,
            encima: DestinoRiel::Fuera,
            vuelo: None,
            enmarcando: false,
            editando: false,
            marcos: Vec::new(),
            nombres: Vec::new(),
        }
    }

    fn guardar(&self) {
        if let Some(d) = &self.dibujo
            && let Err(e) = escribir(d, &self.marcas)
        {
            // Un dibujo en un sitio de solo lectura no tiene por que
            // estropear el lienzo: se apunta y las marcas siguen en memoria.
            tracing::warn!(?e, ruta = %d.display(), "no se pudieron guardar las marcas");
        }
    }

    pub fn riel(&self, ancho: u32, alto: u32, escala_por_cien: u32, bajo_la_barra: i32) -> Riel {
        Riel::colocar(
            ancho,
            alto,
            escala_por_cien,
            self.marcas.len(),
            self.poniendo == Poniendo::Eligiendo,
            EMOJIS.len(),
            bajo_la_barra,
        )
    }

    /// Echa a volar la camara hacia la marca `i` del riel.
    fn ir_a(&mut self, i: usize, cx: &Contexto) {
        let Some(m) = self.marcas.get(i) else {
            return;
        };
        let e = pixpin_escala(cx.escala_por_cien);
        let destino = marcas::camara_con_la_marca_arriba(
            m,
            cx.camara.zoom,
            cx.area.ancho as f32 / e,
            cx.bajo_la_barra as f32 / e + AIRE_BAJO_LA_BARRA,
        );
        self.vuelo = Some((Vuelo::hacia(destino), std::time::Instant::now()));
    }

    /// Mueve la camara un paso del viaje. `true` si la movio.
    pub fn avanzar(&mut self, camara: &mut Camara) -> bool {
        let Some((v, reloj)) = self.vuelo.as_mut() else {
            return false;
        };
        // Si alguien cambio el aumento a mitad de viaje (la rueda), manda
        // quien toca: seguir volando le devolveria el zoom que acaba de
        // quitar.
        if (camara.zoom - v.destino.zoom).abs() > 1e-4 {
            self.vuelo = None;
            return false;
        }
        let dt = reloj.elapsed().as_secs_f32();
        *reloj = std::time::Instant::now();
        if !v.avanzar(camara, dt) {
            self.vuelo = None;
        }
        true
    }

    /// Cada cuanto hay que despertar el bucle: cada fotograma mientras vuela.
    pub fn tope_ms(&self) -> Option<u32> {
        self.vuelo.is_some().then_some(8)
    }

    /// Atiende un evento de la ventana. Lo que no es suyo sale con
    /// `consumido: false` y sigue su camino.
    pub fn evento(&mut self, ev: &EventoOverlay, cx: &Contexto) -> Respuesta {
        let local = |p: &Punto| Punto {
            x: p.x - cx.area.x,
            y: p.y - cx.area.y,
        };
        let riel = self.riel(
            cx.area.ancho,
            cx.area.alto,
            cx.escala_por_cien,
            cx.bajo_la_barra,
        );
        let radio = RADIO_MARCA * pixpin_escala(cx.escala_por_cien);
        let marca_bajo = |marcas: &[Marca], p: Punto| {
            marcas::marca_en(
                marcas,
                cx.efectiva,
                Punto2::nuevo(p.x as f32, p.y as f32),
                radio,
            )
        };
        match ev {
            EventoOverlay::Tecla {
                vk,
                ctrl,
                shift,
                alt,
                ..
            } if !cx.escribiendo => self.tecla(*vk, *ctrl, *shift, *alt, cx),
            // Con la tira abierta los numeros eligen emoticono (y no el
            // grosor de la pluma, que es lo que hacen fuera de ella).
            EventoOverlay::Caracter(c)
                if self.poniendo == Poniendo::Eligiendo && !cx.escribiendo =>
            {
                match c.to_digit(10) {
                    Some(d) => {
                        let i = if d == 0 { 9 } else { d as usize - 1 };
                        self.poniendo = Poniendo::Plantando(i.min(EMOJIS.len() - 1));
                        Respuesta::suya(true)
                    }
                    None => Respuesta::default(),
                }
            }
            EventoOverlay::BotonPulsado(p) => {
                // Un clic, en donde sea, para el viaje: la mano vuelve a
                // mandar.
                self.vuelo = None;
                let q = local(p);
                // Una ficha de los marcos (con la herramienta Marco): a ese.
                if self.enmarcando {
                    let e = cx.escala_por_cien as f32 / 100.0;
                    let fichas =
                        crate::marcos_doc::fichas_de(self.marcos.len(), cx.area.alto as f32, e);
                    if let Some(i) =
                        crate::marcos_doc::ficha_bajo(&fichas, (q.x as f32, q.y as f32))
                    {
                        return Respuesta {
                            ir_a_marco: Some(i),
                            ..Respuesta::suya(true)
                        };
                    }
                }
                match riel.destino(q) {
                    DestinoRiel::PonerMarca => {
                        self.poniendo = if self.poniendo == Poniendo::No {
                            Poniendo::Eligiendo
                        } else {
                            Poniendo::No
                        };
                        Respuesta::suya(true)
                    }
                    DestinoRiel::Imprimir => Respuesta {
                        imprimir: true,
                        ..Respuesta::suya(true)
                    },
                    // El lapiz del riel edita (24-oct-2026, «con el lapiz de
                    // la barra lateral pueda activar el modo edicion»), como
                    // el «Anotar» del mismo sitio en los visores. La hojita
                    // sigue en Ctrl+Mayus+N.
                    DestinoRiel::Hojita => Respuesta {
                        editar: true,
                        ..Respuesta::suya(true)
                    },
                    DestinoRiel::Marca(i) => {
                        self.ir_a(i, cx);
                        Respuesta::suya(true)
                    }
                    DestinoRiel::Emoji(i) => {
                        self.poniendo = Poniendo::Plantando(i);
                        Respuesta::suya(true)
                    }
                    DestinoRiel::CerrarTira => {
                        self.poniendo = Poniendo::No;
                        Respuesta::suya(true)
                    }
                    DestinoRiel::Hueco => Respuesta::suya(false),
                    DestinoRiel::Fuera => {
                        if let Poniendo::Plantando(i) = self.poniendo {
                            let w = cx.efectiva.a_mundo(Punto2::nuevo(q.x as f32, q.y as f32));
                            self.marcas = marcas::con(
                                &self.marcas,
                                w.x as f64,
                                w.y as f64,
                                EMOJIS[i.min(EMOJIS.len() - 1)],
                                ahora_ms(),
                            );
                            self.poniendo = Poniendo::No;
                            self.guardar();
                            return Respuesta::suya(true);
                        }
                        match marca_bajo(&self.marcas, q) {
                            Some(id) if cx.mano => {
                                let s = self.en_pantalla(id, cx.efectiva);
                                self.arrastre = s.map(|s| (id, s));
                                Respuesta {
                                    cursor: Some(FormaCursorWin::Mover),
                                    ..Respuesta::suya(false)
                                }
                            }
                            Some(id) => {
                                if let Some(i) = self.marcas.iter().position(|m| m.id == id) {
                                    self.ir_a(i, cx);
                                }
                                Respuesta::suya(false)
                            }
                            None => Respuesta::default(),
                        }
                    }
                }
            }
            EventoOverlay::BotonDerechoPulsado(p) => {
                let q = local(p);
                let id = match riel.destino(q) {
                    DestinoRiel::Marca(i) => self.marcas.get(i).map(|m| m.id),
                    DestinoRiel::Fuera => marca_bajo(&self.marcas, q),
                    // Sobre la isla o la tira el menu de exportar no pinta
                    // nada: se queda aqui.
                    _ => return Respuesta::suya(false),
                };
                match id {
                    Some(id) => {
                        self.marcas = marcas::sin(&self.marcas, id);
                        self.guardar();
                        Respuesta::suya(true)
                    }
                    None => Respuesta::default(),
                }
            }
            EventoOverlay::RatonMovido(_) if cx.ocupado && self.arrastre.is_none() => {
                Respuesta::default()
            }
            EventoOverlay::RatonMovido(p) => {
                let q = local(p);
                if let Some((id, _)) = self.arrastre {
                    self.arrastre = Some((id, Punto2::nuevo(q.x as f32, q.y as f32)));
                    return Respuesta {
                        cursor: Some(FormaCursorWin::Mover),
                        ..Respuesta::suya(true)
                    };
                }
                let ahora = riel.destino(q);
                let cambio = ahora != self.encima;
                self.encima = ahora;
                if ahora != DestinoRiel::Fuera {
                    return Respuesta {
                        cursor: Some(FormaCursorWin::Flecha),
                        ..Respuesta::suya(cambio)
                    };
                }
                // Encima de una marca del lienzo con la mano: se ve que se
                // puede coger antes de cogerla.
                if cx.mano && marca_bajo(&self.marcas, q).is_some() {
                    return Respuesta {
                        cursor: Some(FormaCursorWin::Mover),
                        ..Respuesta::suya(cambio)
                    };
                }
                Respuesta {
                    repintar: cambio,
                    cursor: matches!(self.poniendo, Poniendo::Plantando(_))
                        .then_some(FormaCursorWin::Cruz),
                    ..Respuesta::default()
                }
            }
            EventoOverlay::BotonSoltado(p) => {
                let Some((id, _)) = self.arrastre.take() else {
                    return Respuesta::default();
                };
                let q = local(p);
                let w = cx.efectiva.a_mundo(Punto2::nuevo(q.x as f32, q.y as f32));
                self.marcas = marcas::movida(&self.marcas, id, w.x as f64, w.y as f64);
                self.guardar();
                Respuesta::suya(true)
            }
            _ => Respuesta::default(),
        }
    }

    fn tecla(&mut self, vk: u32, ctrl: bool, shift: bool, alt: bool, cx: &Contexto) -> Respuesta {
        match (vk, ctrl, shift, alt) {
            (VK_M, true, false, false) => {
                self.poniendo = if self.poniendo == Poniendo::No {
                    Poniendo::Eligiendo
                } else {
                    Poniendo::No
                };
                Respuesta::suya(true)
            }
            // «N» de nota: Ctrl+Mayus+N saca la hojita.
            (VK_N, true, true, false) => Respuesta {
                hojita: true,
                ..Respuesta::suya(true)
            },
            (v, true, false, false) if (b'1' as u32..=b'9' as u32).contains(&v) => {
                let i = (v - b'1' as u32) as usize;
                if i < self.marcas.len() {
                    self.ir_a(i, cx);
                }
                // Consumida aunque no haya tantas marcas: un Ctrl+7 sin
                // septima marca no puede ir a parar a otra cosa.
                Respuesta::suya(false)
            }
            (VK_ESCAPE, _, _, _) if self.poniendo != Poniendo::No => {
                self.poniendo = Poniendo::No;
                Respuesta::suya(true)
            }
            _ => Respuesta::default(),
        }
    }

    /// Donde cae una marca en la pantalla: la que se arrastra, donde va el
    /// raton; las demas, donde dice su punto.
    fn en_pantalla(&self, id: i64, efectiva: &Camara) -> Option<Punto2> {
        if let Some((a, s)) = self.arrastre
            && a == id
        {
            return Some(s);
        }
        self.marcas
            .iter()
            .find(|m| m.id == id)
            .map(|m| efectiva.a_pantalla(Punto2::nuevo(m.x as f32, m.y as f32)))
    }

    /// Si el redondel de alguna marca cae en `zona` (pixeles de VENTANA:
    /// x0, y0, x1, y1). Lo pregunta `hornear_trazo` antes de pintar un trazo
    /// suelto encima de lo ya pintado: la marca va encima de todo lo
    /// dibujado, y el trazo horneado la taparia hasta el siguiente fotograma
    /// entero. Se mira la caja del redondel, que sobra un poco en las
    /// esquinas: equivocarse hacia «si toca» solo cuesta un fotograma entero.
    pub fn alguna_toca(
        &self,
        efectiva: &Camara,
        zona: (f32, f32, f32, f32),
        escala_por_cien: u32,
    ) -> bool {
        let radio = RADIO_MARCA * pixpin_escala(escala_por_cien);
        self.marcas.iter().any(|m| {
            self.en_pantalla(m.id, efectiva).is_some_and(|s| {
                s.x + radio >= zona.0
                    && s.x - radio <= zona.2
                    && s.y + radio >= zona.1
                    && s.y - radio <= zona.3
            })
        })
    }

    /// **Las marcas sobre el lienzo**, dentro del fotograma de la escena y
    /// despues de todo lo dibujado. `margen` es el colchon de la superficie
    /// de la escena (A3): lo que se pinta ahi va corrido esos pixeles, y lo
    /// que cae en el colchon tambien se pinta, para que asome al desplazar.
    pub fn pintar_en_la_escena(
        &self,
        p: &Pintor<'_>,
        efectiva: &Camara,
        margen: f32,
        ancho_px: f32,
        alto_px: f32,
        escala_por_cien: u32,
    ) {
        let e = pixpin_escala(escala_por_cien);
        pintar_marcos(
            p,
            &self.marcos,
            &self.nombres,
            efectiva,
            margen,
            ancho_px,
            alto_px,
            e,
        );
        if self.marcas.is_empty() {
            return;
        }
        // La vista del mundo que dejo puesta la escena se cambia por pixeles
        // de ventana corridos el colchon: la marca mide lo mismo a cualquier
        // aumento, como el redondel del movil.
        p.desplazar(margen, margen);
        let radio = RADIO_MARCA * e;
        for m in &self.marcas {
            let Some(s) = self.en_pantalla(m.id, efectiva) else {
                continue;
            };
            // Lo que no se ve ni asoma por el colchon no se compone.
            let fuera = margen + radio * 2.0;
            if s.x < -fuera || s.y < -fuera || s.x > ancho_px + fuera || s.y > alto_px + fuera {
                continue;
            }
            pintar_redondel(p, &m.emoji, (s.x, s.y), radio, 0.82);
        }
    }

    /// **El riel y la tira**, en la capa de la interfaz. `ancho` y `alto` son
    /// los de la ventana.
    pub fn pintar_interfaz(
        &self,
        p: &Pintor<'_>,
        ancho: u32,
        alto: u32,
        escala_por_cien: u32,
        bajo_la_barra: i32,
    ) {
        let riel = self.riel(ancho, alto, escala_por_cien, bajo_la_barra);
        pintar_riel(
            p,
            &riel,
            ancho,
            bajo_la_barra,
            &self.marcas,
            self.encima,
            self.poniendo,
            self.enmarcando,
            self.editando,
        );
        if self.enmarcando {
            let e = escala_por_cien as f32 / 100.0;
            let fichas = crate::marcos_doc::fichas_de(self.marcos.len(), alto as f32, e);
            crate::marcos_doc::pintar_fichas(p, &fichas, e);
        }
    }
}

fn pixpin_escala(escala_por_cien: u32) -> f32 {
    crate::navegacion::escala_de(escala_por_cien)
}

fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn rf(r: Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

/// Un emoticono sobre su redondel oscuro, centrado en `c`.
fn pintar_redondel(p: &Pintor<'_>, emoji: &str, c: (f32, f32), radio: f32, alfa: f32) {
    p.circulo(c, radio, con_alfa(OSCURO, alfa));
    let tam = radio * 1.1;
    let (w, h) = p.medir_texto(emoji, tam);
    // `texto_color` y no `texto`: con el otro el emoticono sale como una
    // silueta del color del pincel.
    p.texto_color(emoji, c.0 - w / 2.0, c.1 - h / 2.0, tam, Color::BLANCO);
}

/// Pinta el riel ya colocado. Aparte de `Marcador` para poder sacar su
/// muestra en PNG sin ventana (ver las pruebas).
pub(super) fn pintar_riel(
    p: &Pintor<'_>,
    riel: &Riel,
    ancho: u32,
    bajo_la_barra: i32,
    lista: &[Marca],
    encima: DestinoRiel,
    poniendo: Poniendo,
    enmarcando: bool,
    editando: bool,
) {
    let e = pixpin_escala(riel.escala_por_cien);
    // La isla de los dos botones, con la misma cara que la barra.
    let isla = rf(riel.isla);
    sombra_isla(p, isla, 8.0 * e, e);
    p.rellenar_redondeado(isla, 8.0 * e, ISLA);
    for (caja, icono, activo, que) in [
        (
            riel.poner_marca,
            &material::BOOKMARK_ADD,
            poniendo != Poniendo::No,
            DestinoRiel::PonerMarca,
        ),
        // El mismo lapiz que en los visores (`lector.rs`).
        (riel.hojita, &material::EDIT, editando, DestinoRiel::Hojita),
        // El marco: como en los visores y en el plano.
        (
            riel.imprimir,
            &material::CROP_FREE,
            enmarcando,
            DestinoRiel::Imprimir,
        ),
    ] {
        let r = rf(caja);
        if activo {
            p.rellenar_redondeado(r, 6.0 * e, ACTIVO_FONDO);
        } else if encima == que {
            p.rellenar_redondeado(r, 6.0 * e, HOVER);
        }
        let lado = 20.0 * e;
        p.icono(
            icono,
            RectF {
                x: r.x + (r.ancho - lado) / 2.0,
                y: r.y + (r.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            if activo { ACTIVO_ICONO } else { ICONO },
        );
    }

    // El globo del boton bajo el raton: su nombre y su tecla.
    if let Some((caja, clave)) = match encima {
        DestinoRiel::PonerMarca => Some((riel.poner_marca, "riel-marcador")),
        DestinoRiel::Hojita => Some((
            riel.hojita,
            if editando {
                "riel-mirar"
            } else {
                "riel-editar"
            },
        )),
        DestinoRiel::Imprimir => Some((riel.imprimir, "riel-marcos")),
        _ => None,
    } {
        crate::lector::globo_a_la_izquierda(p, rf(caja), &textos().t(clave), e);
    }

    // Los puntos. En reposo pequenos y medio transparentes, para no tapar el
    // dibujo; con el raton en el riel crecen, y el de debajo se adelanta.
    let con_raton = matches!(encima, DestinoRiel::Marca(_));
    for (i, (m, c)) in lista.iter().zip(&riel.puntos).enumerate() {
        let elegido = encima == DestinoRiel::Marca(i);
        let (diametro, alfa, x) = if elegido {
            (PUNTO_ELEGIDO, 0.9, c.x as f32 - SALTO as f32 * e)
        } else if con_raton {
            (PUNTO_CERCA, 0.4, c.x as f32)
        } else {
            (PUNTO_REPOSO, 0.4, c.x as f32)
        };
        let radio = diametro as f32 * e / 2.0;
        pintar_redondel(p, &m.emoji, (x, c.y as f32), radio, alfa);
        // Las nueve primeras llevan su atajo al lado cuando se miran.
        if elegido && i < 9 {
            let tecla = format!("Ctrl+{}", i + 1);
            let tam = 11.0 * e;
            let (w, h) = p.medir_texto(&tecla, tam);
            p.texto(
                &tecla,
                x - radio - w - 6.0 * e,
                c.y as f32 - h / 2.0,
                tam,
                con_alfa(OSCURO, 0.7),
            );
        }
    }

    // La tira de emoticonos, o el aviso de donde se va a plantar.
    let t = textos();
    if let Some(tira) = &riel.tira {
        let marco = rf(tira.marco);
        p.rellenar_redondeado(marco, marco.alto / 2.0, con_alfa(OSCURO, 0.85));
        for (i, celda) in tira.celdas.iter().enumerate() {
            let r = rf(*celda);
            if encima == DestinoRiel::Emoji(i) {
                p.circulo(
                    (r.x + r.ancho / 2.0, r.y + r.alto / 2.0),
                    r.ancho / 2.0,
                    con_alfa(Color::BLANCO, 0.18),
                );
            }
            let tam = 22.0 * e;
            let (w, h) = p.medir_texto(EMOJIS[i], tam);
            p.texto_color(
                EMOJIS[i],
                r.x + (r.ancho - w) / 2.0,
                r.y + (r.alto - h) / 2.0,
                tam,
                Color::BLANCO,
            );
        }
        let x = rf(tira.cerrar);
        let lado = 18.0 * e;
        p.icono(
            &material::CLOSE,
            RectF {
                x: x.x + (x.ancho - lado) / 2.0,
                y: x.y + (x.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            Color::BLANCO,
        );
        aviso(
            p,
            &t.t("marca-elige"),
            None,
            marco.x + marco.ancho / 2.0,
            marco.y + marco.alto + 8.0 * e,
            e,
        );
    } else if let Poniendo::Plantando(i) = poniendo {
        // Bajo la barra, donde estaba la tira: el emoticono elegido y que
        // falta un clic.
        aviso(
            p,
            &t.t("marca-plantar"),
            Some(EMOJIS[i.min(EMOJIS.len() - 1)]),
            ancho as f32 / 2.0,
            bajo_la_barra as f32 + 8.0 * e,
            e,
        );
    }
}

/// Una pastilla oscura con una linea de texto, centrada en `cx`.
fn aviso(p: &Pintor<'_>, texto: &str, emoji: Option<&str>, cx: f32, y: f32, e: f32) {
    let tam = 13.0 * e;
    let (w, h) = p.medir_texto(texto, tam);
    let (we, _) = emoji.map_or((0.0, 0.0), |em| p.medir_texto(em, tam * 1.3));
    let hueco = if emoji.is_some() { 6.0 * e } else { 0.0 };
    let caja = RectF {
        x: cx - (w + we + hueco) / 2.0 - 12.0 * e,
        y,
        ancho: w + we + hueco + 24.0 * e,
        alto: h + 12.0 * e,
    };
    p.rellenar_redondeado(caja, caja.alto / 2.0, con_alfa(OSCURO, 0.85));
    if let Some(em) = emoji {
        p.texto_color(
            em,
            caja.x + 12.0 * e,
            caja.y + 6.0 * e - 1.5 * e,
            tam * 1.3,
            Color::BLANCO,
        );
    }
    p.texto(
        texto,
        caja.x + 12.0 * e + we + hueco,
        caja.y + 6.0 * e,
        tam,
        Color::BLANCO,
    );
}

/// **Los marcos del lienzo** (22-oct), con la escena: su raya azul a trazos
/// y su numero en el cuadrito azul de la esquina, como los de los visores
/// (`marcos_doc`). En pixeles de la ventana, asi la raya mide lo mismo a
/// cualquier aumento (el usuario: «que a pesar de agrandar mantenga su
/// grosor constante»), y con la escena y no con la interfaz, para que el
/// numero se vaya con el lienzo mientras se mueve y no llegue tarde.
#[allow(clippy::too_many_arguments)]
fn pintar_marcos(
    p: &Pintor<'_>,
    marcos: &[[f32; 4]],
    nombres: &[String],
    efectiva: &Camara,
    margen: f32,
    ancho_px: f32,
    alto_px: f32,
    e: f32,
) {
    if marcos.is_empty() {
        return;
    }
    p.desplazar(margen, margen);
    for (i, m) in marcos.iter().enumerate() {
        let a = efectiva.a_pantalla(Punto2::nuevo(m[0], m[1]));
        let b = efectiva.a_pantalla(Punto2::nuevo(m[0] + m[2], m[1] + m[3]));
        let r = RectF {
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            ancho: (b.x - a.x).abs(),
            alto: (b.y - a.y).abs(),
        };
        let fuera = margen + 40.0 * e;
        if r.x > ancho_px + fuera
            || r.y > alto_px + fuera
            || r.x + r.ancho < -fuera
            || r.y + r.alto < -fuera
        {
            continue;
        }
        p.trazar_discontinuo(r, 1.5 * e, crate::marcos_doc::AZUL);
        crate::marcos_doc::insignia(p, r.x, r.y, i + 1, e);
        if let Some(n) = nombres.get(i).filter(|n| !n.is_empty()) {
            let tam = 12.0 * e;
            let (_, th) = p.medir_texto(n, tam);
            p.texto(n, r.x, r.y - th - 4.0 * e, tam, crate::marcos_doc::AZUL);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-marcas-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn contexto<'a>(efectiva: &'a Camara, mano: bool) -> Contexto<'a> {
        Contexto {
            area: Rect {
                x: 0,
                y: 0,
                ancho: 1600,
                alto: 900,
            },
            escala_por_cien: 100,
            efectiva,
            camara: efectiva,
            mano,
            escribiendo: false,
            ocupado: false,
            bajo_la_barra: 64,
        }
    }

    fn marcador(lista: Vec<Marca>) -> Marcador {
        Marcador {
            marcas: lista,
            dibujo: None,
            poniendo: Poniendo::No,
            arrastre: None,
            encima: DestinoRiel::Fuera,
            vuelo: None,
            enmarcando: false,
            editando: false,
            marcos: Vec::new(),
            nombres: Vec::new(),
        }
    }

    fn tecla(vk: u32, ctrl: bool, shift: bool) -> EventoOverlay {
        EventoOverlay::Tecla {
            vk,
            shift,
            ctrl,
            alt: false,
        }
    }

    #[test]
    fn los_emoticonos_del_lienzo_son_los_de_los_lectores() {
        assert_eq!(EMOJIS, pixpin_docs::lectura::EMOJIS);
    }

    #[test]
    fn una_marca_bajo_el_trazo_soltado_toca_su_zona_y_una_lejana_no() {
        // `hornear_trazo` pinta el trazo encima de lo que ya hay: si debajo
        // hay el redondel de una marca, el trazo lo taparia hasta el
        // siguiente fotograma entero. Por eso pregunta antes.
        let camara = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };
        let m = marcador(marcas::con(&[], 100.0, 100.0, "⭐", 1));
        // Zona que pasa por encima del punto de la marca.
        assert!(m.alguna_toca(&camara, (80.0, 90.0, 300.0, 110.0), 100));
        // Rozando solo el borde del redondel (radio 18 px al 100 %).
        assert!(m.alguna_toca(&camara, (115.0, 60.0, 200.0, 95.0), 100));
        // Caso negativo: lejos, no.
        assert!(!m.alguna_toca(&camara, (400.0, 400.0, 600.0, 500.0), 100));
        // Con la camara corrida, lo que cuenta es donde se VE la marca.
        let corrida = Camara {
            x: 300.0,
            y: 0.0,
            zoom: 1.0,
        };
        assert!(!m.alguna_toca(&corrida, (80.0, 90.0, 120.0, 110.0), 100));
        assert!(!marcador(Vec::new()).alguna_toca(&camara, (0.0, 0.0, 9e3, 9e3), 100));
    }

    #[test]
    fn las_marcas_se_guardan_junto_al_dibujo_y_vuelven_igual() {
        let d = carpeta_de_prueba("guardar");
        let dibujo = d.join("dib-1.excalidraw");
        let lista = marcas::con(&marcas::con(&[], 10.0, 20.0, "⭐", 1), -5.5, 0.0, "🔖", 2);
        escribir(&dibujo, &lista).unwrap();
        assert!(d.join("dib-1.excalidraw.pixpin-marcas").exists());
        assert_eq!(leer(&dibujo), lista);
        // Sin marcas no queda un fichero vacio al lado del dibujo.
        escribir(&dibujo, &[]).unwrap();
        assert!(!d.join("dib-1.excalidraw.pixpin-marcas").exists());
        assert!(leer(&dibujo).is_empty());
        // Borrar lo que ya no esta no es un error.
        escribir(&dibujo, &[]).unwrap();
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_guarda_dice_de_que_dibujo_son_y_al_soltarla_se_olvida() {
        let d = carpeta_de_prueba("guarda");
        let dibujo = d.join("hoja.excalidraw");
        escribir(&dibujo, &marcas::con(&[], 1.0, 2.0, "⭐", 5)).unwrap();
        {
            let _g = junto_a(&dibujo);
            assert_eq!(Marcador::cargar().marcas.len(), 1);
        }
        assert!(
            Marcador::cargar().marcas.is_empty(),
            "sin guarda no hay dibujo"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ctrl_m_abre_la_tira_un_emoticono_y_un_clic_plantan_la_marca() {
        let c = Camara {
            x: 100.0,
            y: 50.0,
            zoom: 2.0,
        };
        let cx = contexto(&c, false);
        let mut m = marcador(Vec::new());
        assert!(m.evento(&tecla(VK_M, true, false), &cx).consumido);
        assert_eq!(m.poniendo, Poniendo::Eligiendo);
        // El «2» elige el segundo emoticono, no el grosor de la pluma.
        assert!(m.evento(&EventoOverlay::Caracter('2'), &cx).consumido);
        assert_eq!(m.poniendo, Poniendo::Plantando(1));
        let r = m.evento(&EventoOverlay::BotonPulsado(Punto { x: 400, y: 300 }), &cx);
        assert!(r.consumido, "el clic de plantar no dibuja");
        assert_eq!(m.marcas.len(), 1);
        assert_eq!(m.marcas[0].emoji, EMOJIS[1]);
        // (400, 300) de pantalla con esa camara es (300, 200) del mundo.
        assert_eq!((m.marcas[0].x, m.marcas[0].y), (300.0, 200.0));
        assert_eq!(m.poniendo, Poniendo::No);
    }

    #[test]
    fn escape_deja_de_poner_y_sin_poner_no_se_lo_come() {
        let c = Camara::nueva();
        let cx = contexto(&c, false);
        let mut m = marcador(Vec::new());
        m.evento(&tecla(VK_M, true, false), &cx);
        assert!(m.evento(&tecla(VK_ESCAPE, false, false), &cx).consumido);
        assert_eq!(m.poniendo, Poniendo::No);
        // Ahora Escape es del editor, que es quien sabe que hacer con el.
        assert!(!m.evento(&tecla(VK_ESCAPE, false, false), &cx).consumido);
        // Y un numero suelto vuelve a ser de la pluma.
        assert!(!m.evento(&EventoOverlay::Caracter('2'), &cx).consumido);
    }

    #[test]
    fn escribiendo_un_texto_las_teclas_no_son_del_riel() {
        let c = Camara::nueva();
        let mut cx = contexto(&c, false);
        cx.escribiendo = true;
        let mut m = marcador(Vec::new());
        assert!(!m.evento(&tecla(VK_M, true, false), &cx).consumido);
        assert_eq!(m.poniendo, Poniendo::No);
    }

    #[test]
    fn un_trazo_que_pasa_por_encima_del_riel_sigue_siendo_del_trazo() {
        let c = Camara::nueva();
        let mut cx = contexto(&c, false);
        cx.ocupado = true;
        let mut m = marcador(marcas::con(&[], 10.0, 10.0, "⭐", 1));
        let riel = m.riel(1600, 900, 100, 64);
        let encima_del_riel = riel.puntos[0];
        assert!(
            !m.evento(&EventoOverlay::RatonMovido(encima_del_riel), &cx)
                .consumido
        );
    }

    #[test]
    fn un_clic_en_el_lienzo_sin_poner_marca_sigue_siendo_del_lienzo() {
        let c = Camara::nueva();
        let cx = contexto(&c, false);
        let mut m = marcador(marcas::con(&[], 10.0, 10.0, "⭐", 1));
        let r = m.evento(&EventoOverlay::BotonPulsado(Punto { x: 700, y: 500 }), &cx);
        assert!(!r.consumido);
        assert_eq!(m.marcas.len(), 1);
    }

    #[test]
    fn con_la_mano_la_marca_se_arrastra_y_se_queda_donde_se_suelta() {
        let c = Camara::nueva();
        let cx = contexto(&c, true);
        let mut m = marcador(marcas::con(&[], 200.0, 200.0, "⭐", 1));
        assert!(
            m.evento(&EventoOverlay::BotonPulsado(Punto { x: 205, y: 195 }), &cx)
                .consumido
        );
        assert!(
            m.evento(&EventoOverlay::RatonMovido(Punto { x: 500, y: 400 }), &cx)
                .repintar
        );
        assert!(
            m.evento(&EventoOverlay::BotonSoltado(Punto { x: 500, y: 400 }), &cx)
                .consumido
        );
        assert_eq!((m.marcas[0].x, m.marcas[0].y), (500.0, 400.0));
        // Soltar sin arrastre no es suyo.
        assert!(
            !m.evento(&EventoOverlay::BotonSoltado(Punto { x: 1, y: 1 }), &cx)
                .consumido
        );
    }

    #[test]
    fn sin_la_mano_la_marca_no_se_mueve_y_el_clic_viaja_a_ella() {
        let c = Camara::nueva();
        let cx = contexto(&c, false);
        let mut m = marcador(marcas::con(&[], 200.0, 200.0, "⭐", 1));
        assert!(
            m.evento(&EventoOverlay::BotonPulsado(Punto { x: 200, y: 200 }), &cx)
                .consumido
        );
        m.evento(&EventoOverlay::RatonMovido(Punto { x: 500, y: 400 }), &cx);
        m.evento(&EventoOverlay::BotonSoltado(Punto { x: 500, y: 400 }), &cx);
        assert_eq!((m.marcas[0].x, m.marcas[0].y), (200.0, 200.0));
        assert!(m.tope_ms().is_some(), "la camara sale hacia la marca");
    }

    #[test]
    fn ctrl_numero_lleva_la_camara_a_esa_marca_y_la_deja_arriba() {
        let mut c = Camara::nueva();
        let cx_camara = c;
        let cx = contexto(&cx_camara, false);
        let mut m = marcador(marcas::con(
            &marcas::con(&[], 0.0, 0.0, "⭐", 1),
            3000.0,
            2000.0,
            "🔖",
            2,
        ));
        assert!(m.evento(&tecla(b'2' as u32, true, false), &cx).consumido);
        let mut vueltas = 0;
        while m.avanzar(&mut c) {
            vueltas += 1;
            std::thread::sleep(std::time::Duration::from_millis(4));
            assert!(vueltas < 1000);
        }
        let s = c.a_pantalla(Punto2::nuevo(3000.0, 2000.0));
        assert!((s.x - 800.0).abs() < 1.0, "centrada a lo ancho: {}", s.x);
        assert!(
            (s.y - (64.0 + AIRE_BAJO_LA_BARRA)).abs() < 1.0,
            "bajo la barra: {}",
            s.y
        );
        // Una marca que no existe no mueve nada, pero la tecla es suya.
        assert!(m.evento(&tecla(b'9' as u32, true, false), &cx).consumido);
        assert!(m.tope_ms().is_none());
    }

    #[test]
    fn la_rueda_a_mitad_de_viaje_para_el_viaje() {
        let mut c = Camara::nueva();
        let quieta = c;
        let cx = contexto(&quieta, false);
        let mut m = marcador(marcas::con(&[], 5000.0, 0.0, "⭐", 1));
        m.evento(&tecla(b'1' as u32, true, false), &cx);
        c.zoom = 1.5;
        assert!(!m.avanzar(&mut c));
        assert!(m.tope_ms().is_none());
    }

    #[test]
    fn clic_derecho_en_una_marca_la_quita_y_en_el_vacio_no() {
        let c = Camara::nueva();
        let cx = contexto(&c, false);
        let mut m = marcador(marcas::con(&[], 300.0, 300.0, "⭐", 1));
        assert!(
            !m.evento(
                &EventoOverlay::BotonDerechoPulsado(Punto { x: 700, y: 700 }),
                &cx
            )
            .consumido
        );
        assert_eq!(m.marcas.len(), 1);
        assert!(
            m.evento(
                &EventoOverlay::BotonDerechoPulsado(Punto { x: 300, y: 300 }),
                &cx
            )
            .consumido
        );
        assert!(m.marcas.is_empty());
    }

    /// 24-oct-2026: el lapiz del riel pasa a editar (y vuelve); la hojita
    /// queda en su atajo.
    #[test]
    fn el_lapiz_del_riel_edita_y_el_atajo_saca_la_hojita() {
        let c = Camara::nueva();
        let cx = contexto(&c, false);
        let mut m = marcador(Vec::new());
        let riel = m.riel(1600, 900, 100, 64);
        let centro = Punto {
            x: riel.hojita.x + 5,
            y: riel.hojita.y + 5,
        };
        let r = m.evento(&EventoOverlay::BotonPulsado(centro), &cx);
        assert!(r.editar && r.consumido && !r.hojita);
        assert!(m.evento(&tecla(VK_N, true, true), &cx).hojita);
        assert!(
            !m.evento(&tecla(VK_N, true, false), &cx).hojita,
            "Ctrl+N solo no es la hojita"
        );
    }

    #[test]
    fn el_riel_atiende_en_coordenadas_de_la_ventana_aunque_no_empiece_en_cero() {
        let c = Camara::nueva();
        let mut cx = contexto(&c, false);
        cx.area.x = 200;
        cx.area.y = 100;
        let mut m = marcador(Vec::new());
        let riel = m.riel(1600, 900, 100, 64);
        let en_el_escritorio = Punto {
            x: riel.poner_marca.x + 5 + 200,
            y: riel.poner_marca.y + 5 + 100,
        };
        m.evento(&EventoOverlay::BotonPulsado(en_el_escritorio), &cx);
        assert_eq!(m.poniendo, Poniendo::Eligiendo);
    }

    /// **La muestra del riel en PNG**, para mirarla: necesita GPU.
    /// `cargo test -p pixpin --bin pixpinmax muestra_del_riel -- --ignored
    /// --nocapture`. Deja los PNG en `PIXPIN_MUESTRAS` o en la temporal.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_riel() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let (ancho, alto) = (900u32, 640u32);
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let mut lista = Vec::new();
        for (i, (x, y)) in [
            (100.0, 80.0),
            (400.0, 120.0),
            (220.0, 300.0),
            (600.0, 420.0),
            (50.0, 520.0),
        ]
        .into_iter()
        .enumerate()
        {
            lista = marcas::con(&lista, x, y, EMOJIS[i * 2], i as i64);
        }
        let casos = [
            ("reposo", DestinoRiel::Fuera, Poniendo::No),
            ("raton-encima", DestinoRiel::Marca(2), Poniendo::No),
            ("tira", DestinoRiel::Emoji(3), Poniendo::Eligiendo),
            ("plantando", DestinoRiel::Fuera, Poniendo::Plantando(5)),
        ];
        let camara = Camara::nueva();
        for (nombre, encima, poniendo) in casos {
            let m = Marcador {
                marcas: lista.clone(),
                dibujo: None,
                poniendo,
                arrastre: None,
                encima,
                vuelo: None,
                enmarcando: false,
                editando: false,
                marcos: Vec::new(),
                nombres: Vec::new(),
            };
            motor
                .dibujar(&fuera.destino, |p| {
                    p.limpiar(hex(0xffffff));
                    // Un poco de lienzo detras, para ver que se lee encima.
                    for i in 0..12 {
                        p.linea(
                            (0.0, 40.0 + i as f32 * 50.0),
                            (ancho as f32, 10.0 + i as f32 * 50.0),
                            2.0,
                            hex(0x1e1e1e),
                        );
                    }
                    m.pintar_en_la_escena(p, &camara, 0.0, ancho as f32, alto as f32, 100);
                    p.desplazar(0.0, 0.0);
                    m.pintar_interfaz(p, ancho, alto, 100, 64);
                })
                .expect("pintar");
            fuera.esperar_gpu().expect("esperar");
            let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
            let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::imagen::ImagenRgba {
                ancho,
                alto,
                pixeles,
            })
            .expect("codificar");
            let ruta = carpeta.join(format!("riel-{nombre}.png"));
            std::fs::write(&ruta, png).expect("guardar");
            println!("{nombre}: {}", ruta.display());
        }
    }
}
