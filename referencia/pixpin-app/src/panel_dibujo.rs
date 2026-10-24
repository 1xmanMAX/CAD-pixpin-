//! Como se pinta el panel lateral de Excalidraw (`pixpin_ui::panel_lateral`)
//! y como se decide que muestra en el editor.
//!
//! La geometria, que sale y que hace cada clic son de `pixpin-ui`; aqui van
//! los colores del tema claro de Excalidraw (`docs/excalidraw/interfaz.md`
//! §2 y §5), los iconos de cada opcion, los titulos traducidos y el unico
//! estado que el panel tiene: que desplegable esta abierto.

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use pixpin_geom::Rect;
use pixpin_motor2d::elemento::{ColorRgba, EstiloTrazo};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::estilo::{CambioEstilo, CambioForma, NivelGrosor, TipoFlecha};
use pixpin_motor2d::formas::TipoPunta;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::organizar::{self, Alineacion, Reparto};
use pixpin_motor2d::puntos_etiquetados::SerieDePunto;
use pixpin_motor2d::relleno::EstiloRelleno;
use pixpin_motor2d::seleccion::Seleccion;
use pixpin_motor2d::texto::{AlineacionTexto, AlineacionVertical};
use pixpin_motor2d::texto::{FUENTE_COMIC_SHANNS, FUENTE_NUNITO};
use pixpin_motor2d::tinta::{MaterialTinta, Variabilidad};
use pixpin_render::iconos_excalidraw as i;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_shell::overlay::EventoOverlay;
use pixpin_store::{Catalogo, Idioma};
use pixpin_ui::panel_lateral::{
    self as pl, AccionPanel, Capa, ContextoPanel, Control, Desplegable, EdicionHex, Mandos,
    PanelLateral, Seccion,
};

use crate::caja_dibujo::{hex, sombra_isla};

const ISLA: Color = hex(0xffffff);
const TEXTO: Color = hex(0x1b1b1f);
const TEXTO_SUAVE: Color = hex(0x999999);
const BOTON: Color = hex(0xf6f6f9);
const ACTIVO_FONDO: Color = hex(0xe0dfff);
const ACTIVO_ICONO: Color = hex(0x030064);
const CONTORNO_CLARO: Color = hex(0xebebeb);
const CONTORNO_ACTIVO: Color = hex(0x4a47b1);
const PISTA_RECORRIDA: Color = hex(0xccccff);
const PULGAR: Color = hex(0x3d3d3d);

/// **El panel quitado** (24-oct-2026, «pon un boton para mostrarla y
/// quitarla»): lo dice el boton de arriba a la izquierda y vale para todos
/// los lienzos de la sesion, no solo el de esta ventana.
static PLEGADO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Si el panel esta quitado.
pub fn plegado() -> bool {
    PLEGADO.load(std::sync::atomic::Ordering::Relaxed)
}

/// **El panel de este hilo lleva el boton de quitarlo** (o deja de
/// llevarlo). Lo pide el lienzo al abrir.
pub fn con_boton_plegar(si: bool) {
    CON_PLEGAR.with(|c| c.set(si));
}

/// Quita o saca el panel (y cierra su desplegable).
pub fn poner_plegado(si: bool) {
    PLEGADO.store(si, std::sync::atomic::Ordering::Relaxed);
    if si {
        cerrar_desplegable();
    }
}

thread_local! {
    /// Si el panel de este hilo lleva el boton de quitarlo: solo el del
    /// lienzo ([`con_boton_plegar`]); el de un pin, el lector o la hojita
    /// van en su sitio de siempre, sin el.
    static CON_PLEGAR: Cell<bool> = const { Cell::new(false) };
    /// El desplegable abierto (la paleta o las puntas).
    ///
    /// Es el unico estado del panel y vive aqui, por hilo, porque cada
    /// editor tiene su hilo y su bucle: asi dos editores abiertos no
    /// comparten la paleta abierta, y ni el gesto ni la escena —que se
    /// guardan y se deshacen— cargan con algo que solo es de la interfaz.
    static ABIERTO: Cell<Option<Desplegable>> = const { Cell::new(None) };
    /// Si el ultimo boton-abajo fue del panel: su boton-arriba tambien lo es
    /// (ver `atender_clic`). Por hilo por lo mismo que `ABIERTO`.
    static PULSADO_EN_PANEL: Cell<bool> = const { Cell::new(false) };
    /// Se pulso «Guardar en figuras»: lo recoge el editor
    /// ([`tomar_guardar_figura`]), que es quien abre el cajetin del nombre.
    static GUARDAR_FIGURA: Cell<bool> = const { Cell::new(false) };
    /// Lo que se va escribiendo en el campo del codigo, si se esta
    /// escribiendo. **Mientras hay algo aqui el teclado es del panel**
    /// (`tecla_del_hex`): una «r» es un digito que no vale y no la
    /// herramienta rectangulo. Por hilo, como `ABIERTO`, del que depende.
    static HEX: RefCell<Option<EdicionHex>> = const { RefCell::new(None) };
    /// Si se pidio elegir el papel del lienzo (el «Fondo del lienzo» del
    /// menu del clic derecho). Mientras vale, el panel es el del papel
    /// (`ContextoPanel::lienzo`). Por hilo, como `ABIERTO`: es interfaz,
    /// no documento.
    static PAPEL: Cell<bool> = const { Cell::new(false) };
}

/// **Ensena el selector del papel del lienzo**, con su paleta ya abierta:
/// quien lo pide desde el menu viene a elegir un color, y los cinco rapidos
/// de Excalidraw son pocos para eso. Pedirlo con el ya a la vista lo cierra,
/// como el mismo disparador cierra la paleta.
pub fn pedir_papel() {
    HEX.with(|h| h.borrow_mut().take());
    let ya = PAPEL.with(|p| p.replace(false));
    if !ya {
        PAPEL.with(|p| p.set(true));
        ABIERTO.with(|a| a.set(Some(Desplegable::ColorLienzo)));
    } else {
        ABIERTO.with(|a| a.set(None));
    }
}

/// Cierra el desplegable si habia alguno. Lo llama el editor con un clic
/// fuera del panel, como se cierra el `Popover` de Excalidraw. Devuelve si
/// habia algo que cerrar, para saber si hay que repintar.
///
/// Se lleva tambien lo que se estuviera escribiendo en su codigo: sin el
/// desplegable no hay campo, y un campo invisible que se queda el teclado
/// dejaria el editor sordo a sus atajos sin que se viera por que.
pub fn cerrar_desplegable() -> bool {
    HEX.with(|h| h.borrow_mut().take());
    // El selector del papel se cierra con el: es un menu, y un clic en el
    // lienzo es la forma de decir «ya esta».
    let papel = PAPEL.with(|p| p.replace(false));
    ABIERTO.with(|a| a.take().is_some()) || papel
}

/// Si se esta escribiendo un codigo de color: el editor lo mira para saber
/// de quien es el teclado.
pub fn escribiendo_hex() -> bool {
    HEX.with(|h| h.borrow().is_some())
}

/// El color que ensena el desplegable abierto: el de lo elegido, o el
/// «actual» si no hay nada elegido. Es el mismo que pinta la muestra.
fn color_del_desplegable(gesto: &Gesto, escena: &Escena) -> Option<ColorRgba> {
    let estilo = pl::estilo_de_seleccion(&elegidos(gesto, escena)).unwrap_or(gesto.estilo);
    match ABIERTO.with(Cell::get)? {
        Desplegable::ColorTrazo => Some(estilo.trazo),
        Desplegable::ColorFondo => estilo.relleno,
        Desplegable::ColorLienzo => Some(escena.fondo),
        Desplegable::PuntaInicio | Desplegable::PuntaFin => None,
    }
}

/// **El teclado mientras se escribe el codigo del color.** `None` si no se
/// esta escribiendo o el evento no es de teclado: entonces sigue su camino
/// normal. `Some(repintar)` si era suyo, y entonces no llega a nadie mas —ni
/// a los atajos de herramienta ni a los del lienzo—.
///
/// Intro aplica el color si lo escrito es un codigo entero (y si no, se
/// queda escribiendo, para corregirlo); Esc lo deja como estaba. Es lo
/// minimo de un campo de texto: no hay cursor que mover, porque seis digitos
/// se reescriben antes de lo que se tarda en apuntar.
pub fn tecla_del_hex(ev: &EventoOverlay, gesto: &mut Gesto, escena: &mut Escena) -> Option<bool> {
    const VK_RETROCESO: u32 = 0x08;
    const VK_ENTRAR: u32 = 0x0D;
    const VK_ESCAPE: u32 = 0x1B;
    if !escribiendo_hex() {
        return None;
    }
    let editar = |f: &mut dyn FnMut(&mut EdicionHex) -> bool| {
        HEX.with(|h| h.borrow_mut().as_mut().is_some_and(f))
    };
    Some(match *ev {
        // Los mandos (Intro, Esc, Retroceso) llegan tambien como caracter;
        // se atienden por su tecla, que es donde se distinguen bien.
        EventoOverlay::Caracter(c) if c >= ' ' => editar(&mut |e| e.escribir(c)),
        EventoOverlay::Tecla {
            vk: VK_RETROCESO, ..
        } => editar(&mut |e| e.borrar()),
        EventoOverlay::Tecla { vk: VK_ESCAPE, .. } => {
            HEX.with(|h| h.borrow_mut().take());
            true
        }
        EventoOverlay::Tecla { vk: VK_ENTRAR, .. } => {
            let color = HEX.with(|h| h.borrow().as_ref().and_then(EdicionHex::color));
            let accion = match (color, ABIERTO.with(Cell::get)) {
                (Some(c), Some(Desplegable::ColorTrazo)) => {
                    Some(AccionPanel::Estilo(CambioEstilo::Trazo(c)))
                }
                (Some(c), Some(Desplegable::ColorFondo)) => {
                    Some(AccionPanel::Estilo(CambioEstilo::Relleno(Some(c))))
                }
                (Some(c), Some(Desplegable::ColorLienzo)) => Some(AccionPanel::FondoLienzo(c)),
                _ => None,
            };
            match accion {
                Some(a) => {
                    HEX.with(|h| h.borrow_mut().take());
                    aplicar(a, gesto, escena)
                }
                // A medio escribir: no se aplica nada ni se pierde lo escrito.
                None => false,
            }
        }
        EventoOverlay::Caracter(_)
        | EventoOverlay::Tecla { .. }
        | EventoOverlay::TeclaSoltada(_) => false,
        _ => return None,
    })
}

static TEXTOS: OnceLock<Catalogo> = OnceLock::new();

/// Fija el idioma de los titulos. Si nadie lo llama antes del primer
/// pintado, se toma el de Windows: mejor el del sistema que dejar el panel
/// en un idioma fijo.
pub fn fijar_idioma(idioma: Idioma) {
    let _ = TEXTOS.set(Catalogo::nuevo(idioma));
}

fn textos() -> &'static Catalogo {
    TEXTOS.get_or_init(|| {
        Catalogo::nuevo(pixpin_store::resolver_idioma(
            &pixpin_shell::entorno::locale_del_sistema(),
            pixpin_store::ajustes::PreferenciaIdioma::Sistema,
        ))
    })
}

fn rf(r: Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

fn color_de(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn elegidos<'a>(gesto: &Gesto, escena: &'a Escena) -> Vec<&'a pixpin_motor2d::Elemento> {
    gesto
        .seleccion
        .ids()
        .iter()
        .filter_map(|id| escena.buscar(*id))
        .filter(|e| !e.borrado)
        .collect()
}

/// El panel que toca ahora: de lo elegido si hay algo, y si no de la
/// herramienta, marcando los valores actuales. Como en Excalidraw.
pub fn panel_para(
    gesto: &Gesto,
    escena: &Escena,
    area: Rect,
    escala_por_cien: u32,
) -> Option<PanelLateral> {
    let elegidos = elegidos(gesto, escena);
    // El modo guia y el ojo salen con cualquier herramienta que dibuje y con
    // lo elegido: cualquier herramienta traza guias. Con la mano y nada
    // elegido, solo si hay guias o el modo esta puesto (si no, ese panel no
    // existe y no tiene por que nacer por esto).
    let guias = (
        gesto.modo_guia(),
        escena
            .hay_referencias()
            .then_some(escena.referencias_visibles),
    );
    let con_guias = !elegidos.is_empty()
        || !pixpin_ui::propiedades::de_herramienta(gesto.herramienta).is_empty()
        || guias.0
        || guias.1.is_some();
    let guias = con_guias.then_some(guias);
    let (propiedades, estilo, mandos) = match pl::estilo_de_seleccion(&elegidos) {
        Some(estilo) => (
            pl::propiedades_de_seleccion(&elegidos),
            estilo,
            Mandos::de_seleccion(&elegidos).con_guias(guias),
        ),
        None => {
            let mut s = gesto.estilo;
            // El grafito es un lapiz mas (v0.75 del movil): su grosor es el
            // de la tinta (`grosor_tinta`, teclas 1-2-3), no el de las
            // figuras. Sin esto el panel marcaba un grosor que no era el que
            // iba a salir.
            if matches!(gesto.herramienta, Herramienta::Lapiz | Herramienta::Grafito) {
                s.grosor = NivelGrosor::de_elemento(
                    &pixpin_motor2d::Figura::Lapiz {
                        puntos: Vec::new(),
                        presiones: Vec::new(),
                        opciones: None,
                    },
                    gesto.grosor_tinta,
                );
            }
            (
                pixpin_ui::propiedades::de_herramienta(gesto.herramienta).to_vec(),
                s,
                Mandos::de_herramienta(gesto.herramienta, &s, gesto.variabilidad)
                    .con_serie_de_punto(gesto.herramienta, gesto.serie_de_punto)
                    .con_pedir_la_medida(gesto.herramienta, gesto.pedir_la_medida)
                    .con_dureza(gesto.herramienta, gesto.estilo.dureza)
                    .con_guias(guias),
            )
        }
    };
    let papel = PAPEL.with(Cell::get);
    let panel = PanelLateral::construir(
        area,
        escala_por_cien,
        ContextoPanel {
            propiedades: &propiedades,
            estilo,
            seleccionados: elegidos.len(),
            mandos,
            abierto: ABIERTO.with(Cell::get),
            lienzo: papel.then_some(escena.fondo),
        },
    )?;
    // Quitado, solo su boton; el papel pedido desde el menu sale siempre.
    Some(if !CON_PLEGAR.with(Cell::get) {
        panel
    } else if plegado() && !papel {
        PanelLateral::plegado(area, escala_por_cien)
    } else {
        panel.con_plegar(area, escala_por_cien)
    })
}

/// Lo que fue de un clic del raton frente al panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClicPanel {
    /// Era del panel (un control o su hueco): no llega al lienzo.
    /// `repintar` si cambio algo.
    Suyo { repintar: bool },
    /// Fuera del panel: sigue hacia el lienzo. `cerro` si cerro un
    /// desplegable, que hay que repintar.
    Fuera { cerro: bool },
}

/// **El camino entero de un clic sobre el panel**, el mismo que recorre la
/// ventana: monta el panel que se esta viendo (`panel_para`, lo mismo que
/// pinta), mira que control hay bajo el punto (`PanelLateral::destino`) y
/// aplica la accion. Esta aqui y no en `ventana_editor.rs` para que las
/// pruebas pulsen cada boton por donde lo pulsa el usuario y no por un atajo
/// que podria acertar donde la ventana falla.
///
/// `pulsado` distingue el boton-abajo del boton-arriba: la accion va con el
/// primero; el segundo solo se traga para que no llegue al lienzo.
/// Si se pulso «Guardar en figuras» desde la ultima vez (y lo olvida).
pub fn tomar_guardar_figura() -> bool {
    GUARDAR_FIGURA.with(|g| g.replace(false))
}

pub fn atender_clic(
    p: pixpin_geom::Punto,
    pulsado: bool,
    gesto: &mut Gesto,
    escena: &mut Escena,
    area: Rect,
    escala_por_cien: u32,
) -> ClicPanel {
    use pixpin_ui::panel_lateral::DestinoPanel;
    // **El boton-arriba es de quien se quedo el boton-abajo**, aunque el
    // panel haya cambiado entre medias. Pulsar «Borrar» deja sin nada
    // elegido y el panel desaparece: sin esto, el boton-arriba caia en el
    // hueco que dejo y seguia hacia el lienzo como un soltar huerfano.
    if !pulsado && PULSADO_EN_PANEL.with(|c| c.replace(false)) {
        return ClicPanel::Suyo { repintar: false };
    }
    let destino = panel_para(gesto, escena, area, escala_por_cien)
        .map_or(DestinoPanel::Fuera, |panel| panel.destino(p));
    if pulsado {
        PULSADO_EN_PANEL.with(|c| c.set(!matches!(destino, DestinoPanel::Fuera)));
        // Pulsar en cualquier otro sitio suelta el campo del codigo sin
        // aplicarlo, como Esc: lo aplicado es solo lo que se confirma con
        // Intro, y un clic en una muestra ya elige su propio color.
        if destino != DestinoPanel::Accion(AccionPanel::EditarHex) {
            HEX.with(|h| h.borrow_mut().take());
        }
    }
    match destino {
        DestinoPanel::Accion(a) => ClicPanel::Suyo {
            repintar: pulsado && aplicar(a, gesto, escena),
        },
        DestinoPanel::Panel => ClicPanel::Suyo { repintar: false },
        // Un clic fuera cierra el desplegable (la paleta, las puntas), como
        // el `Popover` de Excalidraw, y sigue su camino hacia el lienzo.
        DestinoPanel::Fuera => ClicPanel::Fuera {
            cerro: pulsado && cerrar_desplegable(),
        },
    }
}

/// De lo elegido, solo los que admiten la accion (`panel_lateral::
/// destinatarios`): el panel ensena lo que admite ALGUNO, asi que sin este
/// filtro ponerle fondo a un rectangulo y un texto se lo pondria al texto.
fn solo_quien_admite(accion: &AccionPanel, gesto: &Gesto, escena: &Escena) -> Seleccion {
    let mut s = Seleccion::nueva();
    s.poner_todos(pl::destinatarios(accion, &elegidos(gesto, escena)));
    s
}

/// Aplica lo que se pulso en el panel. Devuelve si cambio algo que haya que
/// repintar.
pub fn aplicar(accion: AccionPanel, gesto: &mut Gesto, escena: &mut Escena) -> bool {
    let sel = gesto.seleccion.clone();
    match accion {
        AccionPanel::Plegar => {
            poner_plegado(!plegado());
            return true;
        }
        AccionPanel::Abrir(d) => {
            // El mismo disparador abre y cierra, como en Excalidraw.
            ABIERTO.with(|a| a.set(if a.get() == Some(d) { None } else { Some(d) }));
        }
        AccionPanel::Estilo(cambio) => {
            // Siempre queda como el estilo de lo proximo que se dibuje; y si
            // hay algo elegido, se le aplica tambien.
            gesto.estilo.aplicar(cambio);
            if let CambioEstilo::Grosor(n) = cambio {
                gesto.grosor_tinta = n.de_tinta();
            }
            let destino = solo_quien_admite(&accion, gesto, escena);
            if !destino.esta_vacia() {
                pixpin_motor2d::estilo::aplicar_a(escena, &destino, cambio);
            }
        }
        AccionPanel::Forma(cambio) => {
            // Como el estilo: queda como «actual» para lo proximo que se
            // dibuje (`currentItemRoundness` y compania en Excalidraw), y si
            // hay algo elegido se le aplica. Sin esto, «Bordes: redondo» con
            // la herramienta puesta no hacia nada. La presion va en su
            // propio campo del gesto, que es anterior a esto.
            gesto.estilo.aplicar_forma(cambio);
            if let CambioForma::Presion(v) = cambio {
                gesto.variabilidad = v;
            }
            let destino = solo_quien_admite(&accion, gesto, escena);
            pixpin_motor2d::estilo::aplicar_forma(escena, &destino, cambio);
            // Elegir una punta cierra su desplegable, como el `IconPicker`
            // de Excalidraw; la paleta de color, en cambio, se queda abierta
            // para ir probando.
            if matches!(
                cambio,
                CambioForma::PuntaInicio(_) | CambioForma::PuntaFin(_)
            ) {
                cerrar_desplegable();
            }
        }
        AccionPanel::Capa(Capa::Fondo) => organizar::al_fondo(escena, &sel),
        AccionPanel::Capa(Capa::Atras) => organizar::atras(escena, &sel),
        AccionPanel::Capa(Capa::Adelante) => organizar::adelante(escena, &sel),
        AccionPanel::Capa(Capa::Frente) => organizar::al_frente(escena, &sel),
        AccionPanel::Alinear(a) => organizar::alinear(escena, &sel, a),
        AccionPanel::Repartir(r) => organizar::repartir(escena, &sel, r),
        AccionPanel::Agrupar => return organizar::agrupar(escena, &sel).is_some(),
        AccionPanel::Desagrupar => organizar::desagrupar(escena, &sel),
        // No toca la escena: lo apunta para el editor ([`tomar_guardar_figura`]).
        AccionPanel::GuardarFigura => {
            GUARDAR_FIGURA.with(|g| g.set(true));
            return false;
        }
        AccionPanel::Duplicar => {
            // Como Ctrl+D en Excalidraw: copias desplazadas 10 unidades, y
            // quedan elegidas las copias. Por `portapapeles::pegar` y no a
            // mano, porque ahi esta lo que la copia a mano olvidaba: la copia
            // de un grupo es OTRO grupo (si no, mover la copia arrastraba el
            // original) y sale en el orden de la escena.
            let mut copias = pixpin_motor2d::portapapeles::copiar(escena, &sel);
            if copias.is_empty() {
                return false;
            }
            for c in &mut copias {
                // La copia es un elemento nuevo: con el id de fichero del
                // original se guardarian dos elementos con el mismo `id` y el
                // movil se quedaria con uno. Las ataduras (flecha-caja,
                // rotulo-caja) eran del original, como al duplicar en
                // Excalidraw una sola de las dos piezas.
                c.extras.id_de_fichero = None;
                c.extras.enganche_inicio = None;
                c.extras.enganche_fin = None;
                c.extras.atados.clear();
                c.extras.contenedor = None;
            }
            let d = pixpin_motor2d::portapapeles::DESPLAZAMIENTO;
            let nuevos = pixpin_motor2d::portapapeles::pegar(escena, &copias, d, d);
            gesto.seleccion.poner_todos(nuevos);
        }
        AccionPanel::Borrar => {
            gesto.evento(EventoGesto::Suprimir, escena, 1.0);
        }
        AccionPanel::EditarHex => {
            // Solo con una paleta abierta hay campo; y el campo nace con el
            // codigo que ensena, elegido entero (ver `EdicionHex`).
            if !matches!(
                ABIERTO.with(Cell::get),
                Some(Desplegable::ColorTrazo | Desplegable::ColorFondo | Desplegable::ColorLienzo)
            ) {
                return false;
            }
            let actual = color_del_desplegable(gesto, escena);
            HEX.with(|h| *h.borrow_mut() = Some(EdicionHex::nueva(actual)));
        }
        // A la escena y a nada mas: ni a lo elegido ni al estilo de lo
        // proximo. La paleta se queda abierta para ir probando, como la de
        // los colores de las figuras.
        AccionPanel::FondoLienzo(color) => return escena.poner_fondo(color),
        // Numerar los puntos: A B C, a b c o 1 2 3 para los proximos. Del
        // gesto, no de lo elegido (ver `AccionPanel::SeriePunto`).
        AccionPanel::SeriePunto(s) => gesto.serie_de_punto = s,
        // Pedir la medida al trazar la cota: del gesto, como la serie.
        AccionPanel::PedirMedida(v) => gesto.pedir_la_medida = v,
        // El modo guia, al gesto (suelta lo elegido: cambia de mundo).
        AccionPanel::ModoGuia(v) => return gesto.poner_modo_guia(v),
        // Ver o esconder las guias, a la escena: viaja con el fichero.
        AccionPanel::VerGuias(v) => {
            if escena.referencias_visibles == v {
                return false;
            }
            gesto.alternar_referencias(escena);
        }
    }
    true
}

fn icono_de_punta(t: TipoPunta) -> &'static pixpin_render::icono::Icono {
    match t {
        TipoPunta::Ninguna => &i::ARROWHEAD_NONE_ICON,
        TipoPunta::Flecha => &i::ARROWHEAD_ARROW_ICON,
        TipoPunta::Barra => &i::ARROWHEAD_BAR_ICON,
        TipoPunta::Circulo => &i::ARROWHEAD_CIRCLE_ICON,
        TipoPunta::CirculoHueco => &i::ARROWHEAD_CIRCLE_OUTLINE_ICON,
        TipoPunta::Triangulo => &i::ARROWHEAD_TRIANGLE_ICON,
        TipoPunta::TrianguloHueco => &i::ARROWHEAD_TRIANGLE_OUTLINE_ICON,
        TipoPunta::Rombo => &i::ARROWHEAD_DIAMOND_ICON,
        TipoPunta::RomboHueco => &i::ARROWHEAD_DIAMOND_OUTLINE_ICON,
    }
}

/// **El rotulo de los mandos de imagen que van con letras** (lienzo-imagen):
/// el aumento de la lupa («x1,5», «x2»...) y desenfocar el mosaico («≈»,
/// lo borroso). `None` en todos los demas.
fn rotulo_de_imagen(accion: AccionPanel) -> Option<String> {
    match accion {
        AccionPanel::Forma(CambioForma::AumentoLupa(z)) => {
            Some(format!("x{}", format!("{z}").replace('.', ",")))
        }
        AccionPanel::Forma(CambioForma::Desenfoque(true)) => Some("\u{2248}".into()),
        // El foco: cuanto oscurece y cuanto ilumina, en por ciento.
        AccionPanel::Forma(CambioForma::Oscurecer(n)) => Some(format!("{n}%")),
        AccionPanel::Forma(CambioForma::ZonaFoco(z)) => Some(format!("{}%", (z * 100.0).round())),
        // La dureza, con su nombre: «4H», «HB», «8B», como en la caja de
        // lapices (`LaDurezaQueHay` del movil).
        AccionPanel::Estilo(CambioEstilo::Dureza(d)) => Some(d.rotulo().into()),
        // El modo guia: la palabra, que un icono de regla no dice que lo que
        // se trace sera andamio.
        AccionPanel::ModoGuia(_) => Some(textos().t("lienzo-guia-corto").to_string()),
        // Las de las laminas de geometria, con su palabra: no hay iconos de
        // Excalidraw para «sector» ni para «radial».
        AccionPanel::Forma(CambioForma::Degradado(t)) => Some(
            textos()
                .t(match t {
                    None => "lienzo-degradado-liso",
                    Some(pixpin_motor2d::geometria_exacta::TipoDegradado::Lineal) => {
                        "lienzo-degradado-lineal"
                    }
                    Some(pixpin_motor2d::geometria_exacta::TipoDegradado::Radial) => {
                        "lienzo-degradado-radial"
                    }
                })
                .to_string(),
        ),
        AccionPanel::Forma(CambioForma::CierreArco(c)) => Some(
            textos()
                .t(match c {
                    None => "lienzo-cierre-abierto",
                    Some(pixpin_motor2d::geometria_exacta::CierreArco::Sector) => {
                        "lienzo-cierre-sector"
                    }
                    Some(pixpin_motor2d::geometria_exacta::CierreArco::Segmento) => {
                        "lienzo-cierre-segmento"
                    }
                })
                .to_string(),
        ),
        AccionPanel::Forma(CambioForma::ModoArco(m)) => Some(
            textos()
                .t(match m {
                    pixpin_motor2d::geometria_exacta::ModoArco::Guia => "lienzo-arco-modo-guia",
                    pixpin_motor2d::geometria_exacta::ModoArco::CentroYPuntos => {
                        "lienzo-arco-modo-centro"
                    }
                    pixpin_motor2d::geometria_exacta::ModoArco::TresPuntos => {
                        "lienzo-arco-modo-tres"
                    }
                })
                .to_string(),
        ),
        AccionPanel::Forma(CambioForma::ArcosAngulo(n)) => Some(")".repeat(n as usize)),
        AccionPanel::Forma(CambioForma::ModoAngulo(m)) => Some(
            textos()
                .t(match m {
                    pixpin_motor2d::geometria_exacta::ModoAngulo::Griega => {
                        "lienzo-angulo-modo-griega"
                    }
                    pixpin_motor2d::geometria_exacta::ModoAngulo::Simple => {
                        "lienzo-angulo-modo-simple"
                    }
                    pixpin_motor2d::geometria_exacta::ModoAngulo::Recto => {
                        "lienzo-angulo-modo-recto"
                    }
                })
                .to_string(),
        ),
        // El raya-punto: el dibujo con letras, que Excalidraw no lo tiene.
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::RayaPunto)) => Some("— · —".into()),
        _ => None,
    }
}

fn icono_de(accion: AccionPanel) -> Option<&'static pixpin_render::icono::Icono> {
    Some(match accion {
        AccionPanel::Estilo(CambioEstilo::Grosor(NivelGrosor::Fino)) => &i::STROKE_WIDTH_BASE_ICON,
        AccionPanel::Estilo(CambioEstilo::Grosor(NivelGrosor::Medio)) => &i::STROKE_WIDTH_BOLD_ICON,
        AccionPanel::Estilo(CambioEstilo::Grosor(NivelGrosor::Grueso)) => {
            &i::STROKE_WIDTH_EXTRA_BOLD_ICON
        }
        AccionPanel::Estilo(CambioEstilo::EstiloRelleno(EstiloRelleno::Rayado)) => {
            &i::FILL_HACHURE_ICON
        }
        AccionPanel::Estilo(CambioEstilo::EstiloRelleno(EstiloRelleno::Cruzado)) => {
            &i::FILL_CROSS_HATCH_ICON
        }
        AccionPanel::Estilo(CambioEstilo::EstiloRelleno(EstiloRelleno::Solido)) => {
            &i::FILL_SOLID_ICON
        }
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::Solido)) => {
            &i::STROKE_STYLE_SOLID_ICON
        }
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::Discontinuo)) => {
            &i::STROKE_STYLE_DASHED_ICON
        }
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::Punteado)) => {
            &i::STROKE_STYLE_DOTTED_ICON
        }
        // **Los materiales de tinta** (v0.59 del movil). Excalidraw no tiene
        // esta fila, asi que cada uno lleva el icono suyo mas parecido: la
        // pluma para la lisa, el rayo para las encendidas, los rellenos para
        // las de raya y el lapiz para las porosas. Lo que de verdad distingue
        // una de otra es la muestra pintada, no el icono.
        AccionPanel::Estilo(CambioEstilo::Material(m)) => match m {
            MaterialTinta::Lisa => &i::STROKE_STYLE_SOLID_ICON,
            MaterialTinta::Luz | MaterialTinta::Hdr => &i::BOLT_ICON,
            MaterialTinta::Rayado => &i::FILL_HACHURE_ICON,
            MaterialTinta::Cruzado => &i::FILL_CROSS_HATCH_ICON,
            MaterialTinta::Puntos => &i::STROKE_STYLE_DOTTED_ICON,
            MaterialTinta::Tiza => &i::BUCKET_FILL_ICON,
            MaterialTinta::Lapiz2b => &i::PENCIL_ICON,
            MaterialTinta::Seco => &i::STROKE_STYLE_DASHED_ICON,
            MaterialTinta::Trama => &i::FILL_ZIG_ZAG_ICON,
            // El grafito: el panel no lo ofrece (es herramienta en el movil),
            // pero el `match` tiene que saber que icono le tocaria.
            MaterialTinta::Cuadritos => &i::PENCIL_ICON,
        },
        AccionPanel::Estilo(CambioEstilo::Rugosidad(r)) if r < 0.5 => &i::SLOPPINESS_ARCHITECT_ICON,
        AccionPanel::Estilo(CambioEstilo::Rugosidad(r)) if r < 1.5 => &i::SLOPPINESS_ARTIST_ICON,
        AccionPanel::Estilo(CambioEstilo::Rugosidad(_)) => &i::SLOPPINESS_CARTOONIST_ICON,
        AccionPanel::Forma(CambioForma::Presion(Variabilidad::Constante)) => {
            &i::STROKE_VARIABILITY_CONSTANT_ICON
        }
        AccionPanel::Forma(CambioForma::Presion(Variabilidad::Variable)) => {
            &i::STROKE_VARIABILITY_VARIABLE_ICON
        }
        AccionPanel::Forma(CambioForma::Bordes { redondo: false }) => &i::EDGE_SHARP_ICON,
        AccionPanel::Forma(CambioForma::Bordes { redondo: true }) => &i::EDGE_ROUND_ICON,
        AccionPanel::Forma(CambioForma::Codos(false)) => &i::SHARP_ARROW_ICON,
        AccionPanel::Forma(CambioForma::Codos(true)) => &i::ELBOW_ARROW_ICON,
        // Los tres de `changeArrowType`, con sus iconos.
        AccionPanel::Forma(CambioForma::TipoFlecha(TipoFlecha::Afilada)) => &i::SHARP_ARROW_ICON,
        AccionPanel::Forma(CambioForma::TipoFlecha(TipoFlecha::Curva)) => &i::ROUND_ARROW_ICON,
        AccionPanel::Forma(CambioForma::TipoFlecha(TipoFlecha::Codos)) => &i::ELBOW_ARROW_ICON,
        // Los de `changeTextAlign` y `changeVerticalAlign`.
        AccionPanel::Forma(CambioForma::Alineacion(AlineacionTexto::Izquierda)) => {
            &i::TEXT_ALIGN_LEFT_ICON
        }
        AccionPanel::Forma(CambioForma::Alineacion(AlineacionTexto::Centro)) => {
            &i::TEXT_ALIGN_CENTER_ICON
        }
        AccionPanel::Forma(CambioForma::Alineacion(AlineacionTexto::Derecha)) => {
            &i::TEXT_ALIGN_RIGHT_ICON
        }
        AccionPanel::Forma(CambioForma::AlineacionVertical(AlineacionVertical::Arriba)) => {
            &i::TEXT_ALIGN_TOP_ICON
        }
        AccionPanel::Forma(CambioForma::AlineacionVertical(AlineacionVertical::Medio)) => {
            &i::TEXT_ALIGN_MIDDLE_ICON
        }
        AccionPanel::Forma(CambioForma::AlineacionVertical(AlineacionVertical::Abajo)) => {
            &i::TEXT_ALIGN_BOTTOM_ICON
        }
        // Los tres de `FontPicker.tsx:23-42`: la pluma para la de a mano.
        AccionPanel::Forma(CambioForma::Familia(FUENTE_NUNITO)) => &i::FONT_FAMILY_NORMAL_ICON,
        AccionPanel::Forma(CambioForma::Familia(FUENTE_COMIC_SHANNS)) => &i::FONT_FAMILY_CODE_ICON,
        AccionPanel::Forma(CambioForma::Familia(_)) => &i::FREEDRAW_ICON,
        AccionPanel::Forma(CambioForma::TamanoLetra(t)) if t < 18.0 => &i::FONT_SIZE_SMALL_ICON,
        AccionPanel::Forma(CambioForma::TamanoLetra(t)) if t < 24.0 => &i::FONT_SIZE_MEDIUM_ICON,
        AccionPanel::Forma(CambioForma::TamanoLetra(t)) if t < 32.0 => &i::FONT_SIZE_LARGE_ICON,
        AccionPanel::Forma(CambioForma::TamanoLetra(_)) => &i::FONT_SIZE_EXTRA_LARGE_ICON,
        AccionPanel::Forma(CambioForma::PuntaInicio(t) | CambioForma::PuntaFin(t)) => {
            icono_de_punta(t)
        }
        AccionPanel::Capa(Capa::Fondo) => &i::SEND_TO_BACK_ICON,
        AccionPanel::Capa(Capa::Atras) => &i::SEND_BACKWARD_ICON,
        AccionPanel::Capa(Capa::Adelante) => &i::BRING_FORWARD_ICON,
        AccionPanel::Capa(Capa::Frente) => &i::BRING_TO_FRONT_ICON,
        AccionPanel::Alinear(Alineacion::Izquierda) => &i::ALIGN_LEFT_ICON,
        AccionPanel::Alinear(Alineacion::CentroHorizontal) => &i::CENTER_HORIZONTALLY_ICON,
        AccionPanel::Alinear(Alineacion::Derecha) => &i::ALIGN_RIGHT_ICON,
        AccionPanel::Alinear(Alineacion::Arriba) => &i::ALIGN_TOP_ICON,
        AccionPanel::Alinear(Alineacion::CentroVertical) => &i::CENTER_VERTICALLY_ICON,
        AccionPanel::Alinear(Alineacion::Abajo) => &i::ALIGN_BOTTOM_ICON,
        AccionPanel::Repartir(Reparto::Horizontal) => &i::DISTRIBUTE_HORIZONTALLY_ICON,
        AccionPanel::Repartir(Reparto::Vertical) => &i::DISTRIBUTE_VERTICALLY_ICON,
        // Pixelar: la rejilla de cuadros. Desenfocar lleva su rotulo (abajo).
        AccionPanel::Forma(CambioForma::Desenfoque(false)) => &i::GRID_ICON,
        // La guia de la lupa: sin nada, la flecha, el cono (el angulo que se
        // abre) y el punto.
        AccionPanel::Forma(CambioForma::GuiaLupa(g)) => match g {
            pixpin_motor2d::lupa_elemento::GuiaDeLupa::Ninguna => &i::ARROWHEAD_NONE_ICON,
            pixpin_motor2d::lupa_elemento::GuiaDeLupa::Flecha => &i::ARROW_ICON,
            pixpin_motor2d::lupa_elemento::GuiaDeLupa::DosLineas => &i::ANGLE_ICON,
            pixpin_motor2d::lupa_elemento::GuiaDeLupa::Punto => &i::ARROWHEAD_CIRCLE_ICON,
        },
        AccionPanel::Duplicar => &i::DUPLICATE_ICON,
        AccionPanel::Borrar => &i::TRASH_ICON,
        AccionPanel::Agrupar => &i::GROUP_ICON,
        AccionPanel::Desagrupar => &i::UNGROUP_ICON,
        AccionPanel::GuardarFigura => &i::LIBRARY_ICON,
        // Si o no: la marca y el aspa.
        AccionPanel::PedirMedida(true) => &i::TABLER_CHECK_ICON,
        AccionPanel::PedirMedida(false) => &i::CLOSE_ICON,
        // El modo guia se dice con su palabra (`rotulo_de_imagen`); el icono
        // es el de reserva, el del editor de lineas.
        AccionPanel::ModoGuia(_) => &i::LINE_EDITOR_ICON,
        // El ojo de las guias: abierto mientras se ven (pulsarlo las
        // esconde), cerrado y marcado mientras estan escondidas.
        AccionPanel::VerGuias(false) => &i::EYE_ICON,
        AccionPanel::VerGuias(true) => &i::EYE_CLOSED_ICON,
        // Las de las laminas de geometria van con su palabra
        // (`rotulo_de_imagen`); estos son los iconos de reserva.
        AccionPanel::Estilo(CambioEstilo::Estilo(EstiloTrazo::RayaPunto)) => {
            &i::STROKE_STYLE_DASHED_ICON
        }
        AccionPanel::Forma(CambioForma::Degradado(None)) => &i::FILL_SOLID_ICON,
        AccionPanel::Forma(CambioForma::Degradado(Some(_))) => &i::FILL_ZIG_ZAG_ICON,
        AccionPanel::Forma(CambioForma::CierreArco(None)) => &i::EDGE_ROUND_ICON,
        AccionPanel::Forma(CambioForma::CierreArco(Some(_))) => &i::ANGLE_ICON,
        AccionPanel::Forma(CambioForma::ModoArco(_)) => &i::EDGE_ROUND_ICON,
        AccionPanel::Forma(CambioForma::ArcosAngulo(_)) => &i::ANGLE_ICON,
        AccionPanel::Forma(CambioForma::ModoAngulo(_)) => &i::ANGLE_ICON,
        _ => return None,
    })
}

/// La clave del catalogo de cada titulo (`panel-*` en los `.ftl`). Los
/// textos son los de Excalidraw (`locales/es-ES.json`), salvo donde su
/// traduccion confunde: alli «sloppiness» y «stroke style» salen los dos
/// como «Estilo de trazo», y aqui el primero es «a mano».
fn clave(s: Seccion) -> &'static str {
    match s {
        Seccion::Trazo => "panel-trazo",
        Seccion::Fondo => "panel-fondo",
        Seccion::Relleno => "panel-relleno",
        Seccion::Grosor => "panel-grosor",
        Seccion::EstiloTrazo => "panel-estilo-trazo",
        Seccion::Presion => "panel-presion",
        Seccion::TrazoAMano => "panel-trazo-a-mano",
        Seccion::Bordes => "panel-bordes",
        Seccion::TipoFlecha => "panel-tipo-flecha",
        Seccion::Fuente => "panel-fuente",
        Seccion::TamanoFuente => "panel-tamano-fuente",
        Seccion::AlineacionTexto => "panel-alineacion-texto",
        Seccion::Puntas => "panel-puntas",
        Seccion::Opacidad => "panel-opacidad",
        Seccion::Capas => "panel-capas",
        Seccion::Alinear => "panel-alinear",
        Seccion::Acciones => "panel-acciones",
        Seccion::Colores => "panel-colores",
        Seccion::Tonos => "panel-tonos",
        Seccion::CodigoHex => "panel-codigo-hex",
        Seccion::FondoLienzo => "fondo-lienzo",
        Seccion::PapelesDelMovil => "fondo-papeles-del-movil",
        Seccion::SerieDePunto => "lienzo-punto-serie",
        Seccion::PedirMedida => "lienzo-cota-pedir",
        Seccion::Dureza => "lienzo-panel-dureza",
        Seccion::Guias => "lienzo-panel-guias",
        Seccion::Mosaico => "lienzo-panel-mosaico",
        Seccion::AumentoLupa => "lienzo-panel-aumento",
        Seccion::GuiaLupa => "lienzo-panel-guia",
        Seccion::Oscurecer => "lienzo-panel-oscurecer",
        Seccion::ZonaFoco => "lienzo-panel-zona-foco",
        Seccion::Degradado => "lienzo-panel-degradado",
        Seccion::CierreArco => "lienzo-panel-cierre-arco",
        Seccion::ModoArco => "lienzo-panel-modo-arco",
        Seccion::ArcosAngulo => "lienzo-panel-arcos-angulo",
        Seccion::ModoAngulo => "lienzo-panel-modo-angulo",
    }
}

/// Tablero de ajedrez del color transparente, como Excalidraw.
fn transparente(p: &Pintor, r: RectF) {
    let n = 4;
    let (w, h) = (r.ancho / n as f32, r.alto / n as f32);
    for fil in 0..n {
        for col in 0..n {
            let c = if (fil + col) % 2 == 0 {
                hex(0xffffff)
            } else {
                hex(0xd6d6d6)
            };
            p.rellenar(
                RectF {
                    x: r.x + col as f32 * w,
                    y: r.y + fil as f32 * h,
                    ancho: w,
                    alto: h,
                },
                c,
            );
        }
    }
}

/// `#rrggbb`, sin la almohadilla: la pinta aparte el campo, como Excalidraw.
fn codigo(c: Option<ColorRgba>) -> String {
    pl::codigo_hex(c).unwrap_or_else(|| "transparent".to_string())
}

fn pintar_isla(p: &Pintor, marco: Rect, e: f32) {
    let marco = rf(marco);
    sombra_isla(p, marco, 8.0 * e, e);
    p.rellenar_redondeado(marco, 8.0 * e, ISLA);
}

pub fn pintar(p: &Pintor, panel: &PanelLateral, escala_por_cien: u32) {
    let e = escala_por_cien as f32 / 100.0;
    // El boton de mostrar y quitar: una isla como la del panel, con la
    // paleta para sacarlo y la flecha para quitarlo.
    if let Some(b) = panel.plegar {
        pintar_isla(p, b, e);
        let lado = 20.0 * e;
        let r = rf(b);
        p.icono(
            if panel.plegado {
                &pixpin_render::icono::material::PALETTE
            } else {
                &pixpin_render::icono::material::CHEVRON_LEFT
            },
            RectF {
                x: r.x + (r.ancho - lado) / 2.0,
                y: r.y + (r.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            TEXTO,
        );
    }
    if panel.plegado {
        return;
    }
    pintar_isla(p, panel.marco, e);
    pintar_controles(p, &panel.controles, e);
    // El desplegable, encima de todo lo demas.
    if let Some(em) = &panel.emergente {
        pintar_isla(p, em.marco, e);
        pintar_controles(p, &em.controles, e);
    }
}

fn pintar_controles(p: &Pintor, controles: &[Control], e: f32) {
    for c in controles {
        match *c {
            Control::Titulo { seccion, rect } => {
                p.texto(
                    &textos().t(clave(seccion)),
                    rect.x as f32,
                    rect.y as f32,
                    12.0 * e,
                    TEXTO,
                );
            }
            Control::Muestra {
                rect,
                color,
                activa,
                grande,
                ..
            } => {
                let r = rf(rect);
                // 4 las rapidas, 5 la del color actual y 6 las de la paleta
                // (`ColorPicker.scss:89-103`, `:178-182`, `:198-202`).
                let radio = if grande {
                    5.0
                } else if r.ancho >= 28.0 * e {
                    6.0
                } else {
                    4.0
                } * e;
                if activa {
                    let fuera = RectF {
                        x: r.x - 2.0 * e,
                        y: r.y - 2.0 * e,
                        ancho: r.ancho + 4.0 * e,
                        alto: r.alto + 4.0 * e,
                    };
                    p.rellenar_redondeado(fuera, radio + 2.0 * e, CONTORNO_ACTIVO);
                    p.rellenar_redondeado(
                        RectF {
                            x: r.x - 1.0 * e,
                            y: r.y - 1.0 * e,
                            ancho: r.ancho + 2.0 * e,
                            alto: r.alto + 2.0 * e,
                        },
                        radio + 1.0 * e,
                        ISLA,
                    );
                }
                match color {
                    None => p.con_recorte(r, |p| transparente(p, r)),
                    Some(col) => {
                        // Los claros llevan contorno para no perderse en la
                        // isla blanca.
                        if col.r + col.g + col.b > 2.4 {
                            p.rellenar_redondeado(
                                RectF {
                                    x: r.x - 1.0,
                                    y: r.y - 1.0,
                                    ancho: r.ancho + 2.0,
                                    alto: r.alto + 2.0,
                                },
                                radio + 1.0,
                                CONTORNO_CLARO,
                            );
                        }
                        p.rellenar_redondeado(r, radio, color_de(col));
                    }
                }
            }
            Control::Opcion {
                rect,
                accion,
                activa,
            } => {
                let r = rf(rect);
                p.rellenar_redondeado(r, 8.0 * e, if activa { ACTIVO_FONDO } else { BOTON });
                // **Cada letra escrita en su propia letra**: «Aa» en
                // Excalifont, en Caveat... Con ocho familias, ocho iconos
                // parecidos no dicen cual es cual; la muestra si. Cuesta una
                // disposicion cacheada por boton.
                if let AccionPanel::Forma(CambioForma::Familia(n)) = accion
                    && let Some(f) = pixpin_motor2d::texto::fuente(n)
                {
                    let letra = crate::dibujo::pintar::letra_de(f.nombre, false, false);
                    let tam = 15.0 * e;
                    let (w, h) = p.medir_con_letra("Aa", tam, f32::MAX, &letra);
                    p.texto_con_letra(
                        "Aa",
                        r.x + (r.ancho - w) / 2.0,
                        r.y + (r.alto - h) / 2.0,
                        tam,
                        f32::MAX,
                        &letra,
                        if activa { ACTIVO_ICONO } else { TEXTO },
                    );
                } else if let Some(muestra) = rotulo_de_imagen(accion) {
                    // El aumento de la lupa y desenfocar se dicen con letras:
                    // «x2» se entiende sin icono, y no hay icono de «borroso».
                    // Un signo solo (el de borroso) va mas grande, como un icono.
                    let tam = if muestra.chars().count() == 1 {
                        18.0
                    } else {
                        11.0
                    } * e;
                    let (w, h) = p.medir_texto(&muestra, tam);
                    p.texto(
                        &muestra,
                        r.x + (r.ancho - w) / 2.0,
                        r.y + (r.alto - h) / 2.0,
                        tam,
                        if activa { ACTIVO_ICONO } else { TEXTO },
                    );
                } else if let AccionPanel::SeriePunto(s) = accion {
                    // La serie se dice con su propio principio, como en el
                    // movil («A B C»): se entiende sin leer ningun rotulo.
                    let muestra = match s {
                        SerieDePunto::Mayusculas => "ABC",
                        SerieDePunto::Minusculas => "abc",
                        SerieDePunto::Numeros => "123",
                    };
                    let tam = 11.0 * e;
                    let (w, h) = p.medir_texto(muestra, tam);
                    p.texto(
                        muestra,
                        r.x + (r.ancho - w) / 2.0,
                        r.y + (r.alto - h) / 2.0,
                        tam,
                        if activa { ACTIVO_ICONO } else { TEXTO },
                    );
                } else if let Some(ic) = icono_de(accion) {
                    let lado = 16.0 * e;
                    p.icono(
                        ic,
                        RectF {
                            x: r.x + (r.ancho - lado) / 2.0,
                            y: r.y + (r.alto - lado) / 2.0,
                            ancho: lado,
                            alto: lado,
                        },
                        if activa { ACTIVO_ICONO } else { TEXTO },
                    );
                }
            }
            Control::Punta {
                rect,
                punta,
                al_inicio,
                activa,
                ..
            } => {
                let r = rf(rect);
                p.rellenar_redondeado(r, 8.0 * e, if activa { ACTIVO_FONDO } else { BOTON });
                let tinta = if activa { ACTIVO_ICONO } else { TEXTO };
                // Los iconos de punta son de 40 × 20: se les da una caja con
                // esa proporcion, o saldrian la mitad de pequenos.
                let ancho = (r.ancho - 4.0 * e).min(36.0 * e);
                let caja = RectF {
                    x: r.x + (r.ancho - ancho) / 2.0,
                    y: r.y + (r.alto - ancho / 2.0) / 2.0,
                    ancho,
                    alto: ancho / 2.0,
                };
                match punta {
                    Some(t) if al_inicio => p.icono_volteado(icono_de_punta(t), caja, tinta),
                    Some(t) => p.icono(icono_de_punta(t), caja, tinta),
                    // Puntas distintas en lo elegido: los tres puntos, que
                    // dicen «hay varias» sin elegir ninguna.
                    None => {
                        let lado = 16.0 * e;
                        p.icono(
                            &i::DOTS_HORIZONTAL_ICON,
                            RectF {
                                x: r.x + (r.ancho - lado) / 2.0,
                                y: r.y + (r.alto - lado) / 2.0,
                                ancho: lado,
                                alto: lado,
                            },
                            tinta,
                        );
                    }
                }
            }
            Control::Hex { rect, color } => {
                let r = rf(rect);
                // Escribiendo: el contorno del foco, lo escrito y la barra
                // del cursor al final (o todo resaltado si aun esta elegido,
                // que es lo que dice que la primera tecla lo sustituye).
                let edicion = HEX.with(|h| h.borrow().clone());
                if let Some(ed) = edicion {
                    p.rellenar_redondeado(r, 8.0 * e, CONTORNO_ACTIVO);
                    p.rellenar_redondeado(
                        RectF {
                            x: r.x + 2.0,
                            y: r.y + 2.0,
                            ancho: r.ancho - 4.0,
                            alto: r.alto - 4.0,
                        },
                        6.0 * e,
                        ISLA,
                    );
                    let tam = 14.0 * e;
                    let y = r.y + (r.alto - tam * 1.3) / 2.0;
                    let x = r.x + 24.0 * e;
                    let (ancho_escrito, _) = p.medir_texto(ed.texto(), tam);
                    if ed.todo_elegido() && !ed.texto().is_empty() {
                        p.rellenar(
                            RectF {
                                x,
                                y,
                                ancho: ancho_escrito,
                                alto: tam * 1.3,
                            },
                            ACTIVO_FONDO,
                        );
                    }
                    p.texto("#", r.x + 10.0 * e, y, tam, TEXTO_SUAVE);
                    p.texto(ed.texto(), x, y, tam, TEXTO);
                    if !ed.todo_elegido() {
                        p.rellenar(
                            RectF {
                                x: x + ancho_escrito + 1.0,
                                y,
                                ancho: (1.0 * e).max(1.0),
                                alto: tam * 1.3,
                            },
                            TEXTO,
                        );
                    }
                    continue;
                }
                p.rellenar_redondeado(r, 8.0 * e, CONTORNO_CLARO);
                p.rellenar_redondeado(
                    RectF {
                        x: r.x + 1.0,
                        y: r.y + 1.0,
                        ancho: r.ancho - 2.0,
                        alto: r.alto - 2.0,
                    },
                    7.0 * e,
                    ISLA,
                );
                let tam = 14.0 * e;
                let y = r.y + (r.alto - tam * 1.3) / 2.0;
                p.texto("#", r.x + 10.0 * e, y, tam, TEXTO_SUAVE);
                p.texto(&codigo(color), r.x + 24.0 * e, y, tam, TEXTO);
            }
            Control::Deslizador { rect, valor } => {
                let r = rf(rect);
                let alto_pista = 4.0 * e;
                let pista = RectF {
                    x: r.x,
                    y: r.y + (r.alto - alto_pista) / 2.0,
                    ancho: r.ancho,
                    alto: alto_pista,
                };
                p.rellenar_redondeado(pista, 2.0 * e, BOTON);
                p.rellenar_redondeado(
                    RectF {
                        ancho: r.ancho * valor,
                        ..pista
                    },
                    2.0 * e,
                    PISTA_RECORRIDA,
                );
                let lado = 16.0 * e;
                p.rellenar_redondeado(
                    RectF {
                        x: (r.x + r.ancho * valor - lado / 2.0).clamp(r.x, r.x + r.ancho - lado),
                        y: r.y + (r.alto - lado) / 2.0,
                        ancho: lado,
                        alto: lado,
                    },
                    lado / 2.0,
                    PULGAR,
                );
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::Elemento;
    use pixpin_motor2d::elemento::Figura;

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1280,
            alto: 900,
        }
    }

    fn con(escena: &mut Escena, figura: Figura) -> u64 {
        escena.abrir_paso();
        let id = escena.anadir(Elemento {
            figura,
            ancho: 100.0,
            alto: 40.0,
            ..Default::default()
        });
        escena.cerrar_paso();
        id
    }

    fn texto() -> Figura {
        Figura::Texto {
            texto: "hola".into(),
            tam: 20.0,
            familia: "Excalifont".into(),
        }
    }

    #[test]
    fn el_fondo_pulsado_con_un_rectangulo_y_un_texto_solo_pinta_el_rectangulo() {
        let mut escena = Escena::nueva();
        let caja = con(&mut escena, Figura::Rectangulo);
        let rotulo = con(&mut escena, texto());
        let mut gesto = Gesto::default();
        gesto.seleccion.poner_todos([caja, rotulo]);
        let rojo = pl::COLORES_FONDO[1];
        assert!(aplicar(
            AccionPanel::Estilo(CambioEstilo::Relleno(rojo)),
            &mut gesto,
            &mut escena
        ));
        assert_eq!(escena.buscar(caja).unwrap().relleno, rojo);
        assert_eq!(escena.buscar(rotulo).unwrap().relleno, None);
        // Y queda para lo proximo que se dibuje.
        assert_eq!(gesto.estilo.relleno, rojo);
    }

    #[test]
    fn la_paleta_se_abre_y_se_cierra_con_el_mismo_disparador_y_con_un_clic_fuera() {
        let mut escena = Escena::nueva();
        let caja = con(&mut escena, Figura::Rectangulo);
        let mut gesto = Gesto::default();
        gesto.seleccion.poner(caja);
        let abrir = AccionPanel::Abrir(Desplegable::ColorTrazo);
        aplicar(abrir, &mut gesto, &mut escena);
        assert!(
            panel_para(&gesto, &escena, area(), 100)
                .unwrap()
                .emergente
                .is_some()
        );
        aplicar(abrir, &mut gesto, &mut escena);
        assert!(
            panel_para(&gesto, &escena, area(), 100)
                .unwrap()
                .emergente
                .is_none()
        );
        aplicar(abrir, &mut gesto, &mut escena);
        assert!(cerrar_desplegable());
        // Caso negativo: cerrar lo ya cerrado no pide repintar.
        assert!(!cerrar_desplegable());
    }

    #[test]
    fn elegir_una_punta_la_cambia_y_cierra_su_desplegable() {
        let mut escena = Escena::nueva();
        let flecha = con(
            &mut escena,
            Figura::Flecha {
                puntos: vec![
                    pixpin_motor2d::Punto2::nuevo(0.0, 0.0),
                    pixpin_motor2d::Punto2::nuevo(90.0, 0.0),
                ],
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
        );
        let mut gesto = Gesto::default();
        gesto.seleccion.poner(flecha);
        aplicar(
            AccionPanel::Abrir(Desplegable::PuntaInicio),
            &mut gesto,
            &mut escena,
        );
        aplicar(
            AccionPanel::Forma(CambioForma::PuntaInicio(TipoPunta::Rombo)),
            &mut gesto,
            &mut escena,
        );
        let Figura::Flecha { punta_inicio, .. } = &escena.buscar(flecha).unwrap().figura else {
            panic!("sigue siendo flecha");
        };
        assert_eq!(*punta_inicio, TipoPunta::Rombo);
        assert!(!cerrar_desplegable(), "ya estaba cerrado");
    }

    fn tecla(vk: u32) -> EventoOverlay {
        EventoOverlay::Tecla {
            vk,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    /// Escribe `s` como lo manda Windows: cada letra, su tecla y su caracter.
    fn teclear(s: &str, gesto: &mut Gesto, escena: &mut Escena) {
        for c in s.chars() {
            let vk = c.to_ascii_uppercase() as u32;
            assert_eq!(tecla_del_hex(&tecla(vk), gesto, escena), Some(false));
            assert!(tecla_del_hex(&EventoOverlay::Caracter(c), gesto, escena).is_some());
        }
    }

    #[test]
    fn el_codigo_se_escribe_con_el_teclado_intro_lo_aplica_y_esc_lo_deja() {
        cerrar_desplegable();
        let mut escena = Escena::nueva();
        let caja = con(&mut escena, Figura::Rectangulo);
        let de_fabrica = escena.buscar(caja).unwrap().trazo;
        let mut gesto = Gesto::default();
        gesto.herramienta = Herramienta::Mano;
        gesto.seleccion.poner(caja);
        // Sin escribir, el teclado no es del panel: sigue hacia el lienzo.
        assert_eq!(
            tecla_del_hex(&EventoOverlay::Caracter('r'), &mut gesto, &mut escena),
            None
        );

        // Se abre la paleta y se pulsa el campo, por donde lo pulsa el raton.
        aplicar(
            AccionPanel::Abrir(Desplegable::ColorTrazo),
            &mut gesto,
            &mut escena,
        );
        let panel = panel_para(&gesto, &escena, area(), 100).unwrap();
        let campo = panel
            .emergente
            .as_ref()
            .unwrap()
            .controles
            .iter()
            .find_map(|c| match *c {
                Control::Hex { rect, .. } => Some(rect),
                _ => None,
            })
            .unwrap();
        let clic = |g: &mut Gesto, e: &mut Escena| {
            atender_clic(centro(campo), true, g, e, area(), 100);
            atender_clic(centro(campo), false, g, e, area(), 100);
        };
        clic(&mut gesto, &mut escena);
        assert!(escribiendo_hex());

        // Con almohadilla, y una «r» por medio que no es un digito: no
        // cambia de herramienta ni se escribe.
        teclear("#e0r3131", &mut gesto, &mut escena);
        assert_eq!(
            gesto.herramienta,
            Herramienta::Mano,
            "la r no es la del rectangulo"
        );
        assert_eq!(escena.buscar(caja).unwrap().trazo, de_fabrica);
        assert_eq!(
            tecla_del_hex(&tecla(0x0D), &mut gesto, &mut escena),
            Some(true)
        );
        assert!(!escribiendo_hex(), "Intro termina");
        assert_eq!(escena.buscar(caja).unwrap().trazo, pl::COLORES_TRAZO[1]);
        assert_eq!(
            gesto.estilo.trazo,
            pl::COLORES_TRAZO[1],
            "y queda como actual"
        );
        assert!(
            escena.deshacer(),
            "un paso de deshacer, como cualquier color"
        );

        // Esc: lo escrito no se aplica.
        clic(&mut gesto, &mut escena);
        teclear("2f9e44", &mut gesto, &mut escena);
        assert_eq!(
            tecla_del_hex(&tecla(0x1B), &mut gesto, &mut escena),
            Some(true)
        );
        assert!(!escribiendo_hex());
        assert_eq!(escena.buscar(caja).unwrap().trazo, de_fabrica);

        // Caso negativo: Intro a medio escribir no aplica nada y deja seguir.
        clic(&mut gesto, &mut escena);
        teclear("2f9", &mut gesto, &mut escena);
        assert_eq!(
            tecla_del_hex(&tecla(0x0D), &mut gesto, &mut escena),
            Some(false)
        );
        assert!(escribiendo_hex(), "se sigue escribiendo para corregirlo");
        // Y un clic en el lienzo lo suelta sin aplicar.
        let lejos = Punto { x: 1200, y: 800 };
        atender_clic(lejos, true, &mut gesto, &mut escena, area(), 100);
        assert!(!escribiendo_hex());
        assert_eq!(escena.buscar(caja).unwrap().trazo, de_fabrica);
        cerrar_desplegable();
    }

    #[test]
    fn el_codigo_del_fondo_pone_el_fondo_y_no_el_trazo() {
        cerrar_desplegable();
        let mut escena = Escena::nueva();
        let caja = con(&mut escena, Figura::Rectangulo);
        let de_fabrica = escena.buscar(caja).unwrap().trazo;
        let mut gesto = Gesto::default();
        gesto.seleccion.poner(caja);
        aplicar(
            AccionPanel::Abrir(Desplegable::ColorFondo),
            &mut gesto,
            &mut escena,
        );
        aplicar(AccionPanel::EditarHex, &mut gesto, &mut escena);
        teclear("a5d8ff", &mut gesto, &mut escena);
        tecla_del_hex(&tecla(0x0D), &mut gesto, &mut escena);
        let e = escena.buscar(caja).unwrap();
        assert_eq!(e.relleno, pl::COLORES_FONDO[3]);
        assert_eq!(e.trazo, de_fabrica);
        // Caso negativo: sin paleta abierta no hay campo en el que escribir.
        cerrar_desplegable();
        assert!(!aplicar(AccionPanel::EditarHex, &mut gesto, &mut escena));
        assert!(!escribiendo_hex());
    }

    #[test]
    fn la_presion_sin_nada_elegido_queda_para_el_proximo_trazo() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::default();
        gesto.herramienta = Herramienta::Lapiz;
        aplicar(
            AccionPanel::Forma(CambioForma::Presion(Variabilidad::Constante)),
            &mut gesto,
            &mut escena,
        );
        assert_eq!(gesto.variabilidad, Variabilidad::Constante);
    }

    /// Las acciones de los botones del panel, en orden.
    fn acciones(panel: &PanelLateral) -> Vec<(AccionPanel, bool)> {
        panel
            .controles
            .iter()
            .filter_map(|c| match c {
                Control::Opcion { accion, activa, .. } => Some((*accion, *activa)),
                _ => None,
            })
            .collect()
    }

    /// 24-oct-2026: el boton de arriba a la izquierda quita el panel y lo
    /// vuelve a sacar, por el mismo camino del clic que el usuario.
    #[test]
    fn el_boton_quita_el_panel_y_lo_vuelve_a_sacar() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::default();
        gesto.tomar_herramienta(Herramienta::Rectangulo);
        poner_plegado(false);
        // Caso negativo: fuera del lienzo (un pin, el lector) no hay boton.
        assert!(
            panel_para(&gesto, &escena, area(), 100)
                .unwrap()
                .plegar
                .is_none()
        );
        con_boton_plegar(true);
        let entero = panel_para(&gesto, &escena, area(), 100).unwrap();
        let b = entero.plegar.expect("lleva el boton");
        let centro = pixpin_geom::Punto {
            x: b.x + b.ancho as i32 / 2,
            y: b.y + b.alto as i32 / 2,
        };
        let dentro = pixpin_geom::Punto {
            x: entero.marco.x + 20,
            y: entero.marco.y + 20,
        };
        assert_eq!(
            atender_clic(centro, true, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Suyo { repintar: true }
        );
        let _ = atender_clic(centro, false, &mut gesto, &mut escena, area(), 100);
        assert!(panel_para(&gesto, &escena, area(), 100).unwrap().plegado);
        // Caso negativo: quitado, donde estaba el panel el clic es del lienzo.
        assert!(matches!(
            atender_clic(dentro, true, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Fuera { .. }
        ));
        let _ = atender_clic(dentro, false, &mut gesto, &mut escena, area(), 100);
        assert_eq!(
            atender_clic(centro, true, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Suyo { repintar: true }
        );
        let _ = atender_clic(centro, false, &mut gesto, &mut escena, area(), 100);
        assert!(!panel_para(&gesto, &escena, area(), 100).unwrap().plegado);
        poner_plegado(false);
        con_boton_plegar(false);
    }

    #[test]
    fn con_el_grafito_salen_las_siete_durezas_y_elegir_una_queda_para_el_proximo() {
        use pixpin_motor2d::tinta::grafito::Dureza;
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::default();
        gesto.tomar_herramienta(Herramienta::Grafito);
        let panel = panel_para(&gesto, &escena, area(), 100).unwrap();
        let durezas: Vec<(Dureza, bool)> = acciones(&panel)
            .into_iter()
            .filter_map(|(a, activa)| match a {
                AccionPanel::Estilo(CambioEstilo::Dureza(d)) => Some((d, activa)),
                _ => None,
            })
            .collect();
        assert_eq!(
            durezas.iter().map(|d| d.0).collect::<Vec<_>>(),
            Dureza::TODAS.to_vec()
        );
        assert_eq!(
            durezas
                .iter()
                .filter(|d| d.1)
                .map(|d| d.0)
                .collect::<Vec<_>>(),
            [Dureza::Hb]
        );
        // Cada boton dice su nombre.
        for d in Dureza::TODAS {
            assert_eq!(
                rotulo_de_imagen(AccionPanel::Estilo(CambioEstilo::Dureza(d))).as_deref(),
                Some(d.rotulo())
            );
        }
        aplicar(
            AccionPanel::Estilo(CambioEstilo::Dureza(Dureza::B6)),
            &mut gesto,
            &mut escena,
        );
        assert_eq!(gesto.estilo.dureza, Dureza::B6);
        // Caso negativo: con el lapiz de tinta no sale.
        gesto.tomar_herramienta(Herramienta::Lapiz);
        let panel = panel_para(&gesto, &escena, area(), 100).unwrap();
        assert!(
            !acciones(&panel)
                .iter()
                .any(|(a, _)| matches!(a, AccionPanel::Estilo(CambioEstilo::Dureza(_))))
        );
    }

    #[test]
    fn el_modo_guia_sale_con_cualquier_herramienta_y_el_ojo_solo_si_hay_guias() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::default();
        gesto.tomar_herramienta(Herramienta::Rectangulo);
        let guias = |gesto: &Gesto, escena: &Escena| -> Vec<(AccionPanel, bool)> {
            acciones(&panel_para(gesto, escena, area(), 100).unwrap())
                .into_iter()
                .filter(|(a, _)| matches!(a, AccionPanel::ModoGuia(_) | AccionPanel::VerGuias(_)))
                .collect()
        };
        assert_eq!(
            guias(&gesto, &escena),
            [(AccionPanel::ModoGuia(true), false)]
        );
        assert!(aplicar(
            AccionPanel::ModoGuia(true),
            &mut gesto,
            &mut escena
        ));
        assert!(gesto.modo_guia());
        assert_eq!(
            guias(&gesto, &escena),
            [(AccionPanel::ModoGuia(false), true)]
        );
        // Una guia en la escena: sale el ojo, y pulsarlo la esconde.
        let id = con(&mut escena, Figura::Elipse);
        escena.buscar_mut(id).unwrap().extras.referencia = true;
        assert_eq!(
            guias(&gesto, &escena),
            [
                (AccionPanel::ModoGuia(false), true),
                (AccionPanel::VerGuias(false), false)
            ]
        );
        assert!(aplicar(
            AccionPanel::VerGuias(false),
            &mut gesto,
            &mut escena
        ));
        assert!(!escena.referencias_visibles);
        // Pulsar lo que ya esta no cambia nada.
        assert!(!aplicar(
            AccionPanel::VerGuias(false),
            &mut gesto,
            &mut escena
        ));
        assert!(!aplicar(
            AccionPanel::ModoGuia(true),
            &mut gesto,
            &mut escena
        ));
        // Con la mano y nada elegido sale por haber guias.
        gesto.tomar_herramienta(Herramienta::Mano);
        assert!(panel_para(&gesto, &escena, area(), 100).is_some());
    }

    #[test]
    fn agrupar_desde_el_panel_mete_a_los_dos_en_el_mismo_grupo() {
        let mut escena = Escena::nueva();
        let a = con(&mut escena, Figura::Rectangulo);
        let b = con(&mut escena, Figura::Elipse);
        let mut gesto = Gesto::default();
        gesto.seleccion.poner_todos([a, b]);
        assert!(aplicar(AccionPanel::Agrupar, &mut gesto, &mut escena));
        let g = |escena: &Escena, id| escena.buscar(id).unwrap().grupos.clone();
        assert_eq!(g(&escena, a).len(), 1);
        assert_eq!(g(&escena, a), g(&escena, b));
        aplicar(AccionPanel::Desagrupar, &mut gesto, &mut escena);
        assert!(g(&escena, a).is_empty() && g(&escena, b).is_empty());
    }

    // ---------------------------------------------------------------------
    // Todos los botones, pulsados por donde los pulsa el usuario
    // ---------------------------------------------------------------------

    use pixpin_geom::Punto;
    use pixpin_motor2d::estilo::EstiloDibujo;
    use pixpin_motor2d::vector::Punto2;

    /// La escena de las pruebas de botones: una figura de cada clase que
    /// tiene mandos propios, en sitios y tamanos distintos para que alinear
    /// y repartir tengan algo que mover. Siempre la misma, con los mismos ids.
    fn escena_con_de_todo() -> (Escena, Vec<u64>) {
        let mut escena = Escena::nueva();
        let figuras = [
            Figura::Rectangulo,
            Figura::Rombo,
            Figura::Elipse,
            texto(),
            Figura::Flecha {
                // En diagonal: una flecha horizontal de codos se pinta igual
                // que la recta y el boton no se veria cambiar nada. Y con un
                // vertice en medio, por lo mismo: con dos puntos la curva es
                // la recta.
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(30.0, 55.0),
                    Punto2::nuevo(90.0, 60.0),
                ],
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            },
            Figura::Lapiz {
                puntos: (0..12)
                    .map(|i| Punto2::nuevo(i as f32 * (4.0 + i as f32), (i as f32).sin() * 20.0))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            Figura::Linea {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(80.0, 30.0)],
            },
            // El rotulo de la caja: el unico que ofrece la alineacion
            // vertical. Se ata a ella despues del bucle.
            texto(),
        ];
        escena.abrir_paso();
        let ids: Vec<u64> = figuras
            .into_iter()
            .enumerate()
            .map(|(i, figura)| {
                let k = i as f32;
                escena.anadir(Elemento {
                    figura,
                    x: 300.0 + k * 130.0 + k * k * 3.0,
                    y: 100.0 + k * 45.0 + k * k * 2.0,
                    ancho: 80.0 + k * 9.0,
                    alto: 50.0 + k * 7.0,
                    semilla: 7 + i as u32,
                    relleno: if i == 0 { pl::COLORES_FONDO[1] } else { None },
                    ..Default::default()
                })
            })
            .collect();
        // Atado por los dos lados, como `texto_en_figuras::atar`, y metido
        // dentro del hueco de la caja.
        let (caja, rotulo) = (ids[0], ids[7]);
        {
            let c = escena.buscar_mut(caja).unwrap();
            c.extras.id_de_fichero = Some("caja".into());
            c.extras.atados.push(pixpin_motor2d::elemento::Atado {
                id: "rotulo".into(),
                tipo: "text".into(),
            });
        }
        {
            let t = escena.buscar_mut(rotulo).unwrap();
            t.extras.id_de_fichero = Some("rotulo".into());
            t.extras.contenedor = Some("caja".into());
            // Lejos del borde izquierdo: pinchar la caja por ahi (la prueba de
            // agrupar) tiene que coger la caja y no su rotulo.
            (t.x, t.y, t.ancho, t.alto) = (345.0, 115.0, 30.0, 20.0);
        }
        escena.cerrar_paso();
        (escena, ids)
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    /// Un clic entero (abajo y arriba) por el camino de la ventana.
    fn clic(p: Punto, gesto: &mut Gesto, escena: &mut Escena) -> ClicPanel {
        let abajo = atender_clic(p, true, gesto, escena, area(), 100);
        let arriba = atender_clic(p, false, gesto, escena, area(), 100);
        assert!(
            matches!(arriba, ClicPanel::Suyo { repintar: false }),
            "el boton-arriba del panel no puede llegar al lienzo"
        );
        abajo
    }

    /// Todos los sitios pulsables del panel que se ve ahora: cada control
    /// con accion (tambien los de un desplegable abierto) y la barra de
    /// opacidad por tres sitios.
    fn pulsables(panel: &PanelLateral) -> Vec<Punto> {
        let mut v = Vec::new();
        let controles = panel
            .controles
            .iter()
            .chain(panel.emergente.iter().flat_map(|e| e.controles.iter()));
        for c in controles {
            match *c {
                Control::Muestra {
                    rect,
                    accion: Some(_),
                    ..
                }
                | Control::Opcion { rect, .. }
                | Control::Punta { rect, .. }
                | Control::Hex { rect, .. } => v.push(centro(rect)),
                Control::Deslizador { rect, .. } => {
                    for f in [0.05, 0.3, 0.7] {
                        v.push(Punto {
                            x: rect.x + (rect.ancho as f32 * f) as i32,
                            y: rect.y + rect.alto as i32 / 2,
                        });
                    }
                }
                _ => {}
            }
        }
        v
    }

    fn sin_version(escena: &Escena) -> Vec<Elemento> {
        escena
            .visibles()
            .map(|e| Elemento {
                version: 0,
                ..e.clone()
            })
            .collect()
    }

    fn estilo_llego(e: &Elemento, c: CambioEstilo) -> bool {
        match c {
            CambioEstilo::Trazo(x) => e.trazo == x,
            CambioEstilo::Relleno(x) => e.relleno == x,
            CambioEstilo::EstiloRelleno(x) => e.estilo_relleno == x,
            CambioEstilo::Grosor(n) => NivelGrosor::de_elemento(&e.figura, e.grosor) == n,
            CambioEstilo::Estilo(s) => e.estilo == s,
            CambioEstilo::Rugosidad(r) => (e.rugosidad - r).abs() < 1e-3,
            CambioEstilo::Opacidad(o) => (e.opacidad - o).abs() < 1e-3,
            CambioEstilo::Material(m) => e.material == m,
            CambioEstilo::Dureza(d) => pixpin_motor2d::tinta::grafito::Dureza::de(e) == d,
        }
    }

    fn forma_llego(e: &Elemento, c: CambioForma) -> bool {
        match (c, &e.figura) {
            (CambioForma::Bordes { redondo }, _) => e.redondo == redondo,
            (CambioForma::Codos(v), Figura::Flecha { codos, .. }) => *codos == v,
            (CambioForma::PuntaInicio(t), Figura::Flecha { punta_inicio, .. }) => {
                *punta_inicio == t
            }
            (CambioForma::PuntaFin(t), Figura::Flecha { punta_fin, .. }) => *punta_fin == t,
            (CambioForma::Familia(n), Figura::Texto { familia, .. }) => {
                pixpin_motor2d::texto::numero_de_familia(familia) == n
            }
            (CambioForma::TamanoLetra(t), Figura::Texto { tam, .. }) => (*tam - t).abs() < 0.01,
            // Lo que rotula sin ser un texto: su letra en los extras.
            (CambioForma::Familia(n), _) => {
                e.extras.familia.as_deref()
                    == Some(pixpin_motor2d::texto::nombre_de_familia(Some(n)))
            }
            (CambioForma::TamanoLetra(t), Figura::Serie { .. }) => {
                (e.ancho - pixpin_motor2d::serie::diametro(t)).abs() < 0.01
            }
            (CambioForma::Presion(v), _) => pixpin_motor2d::estilo::variabilidad_de(e) == Some(v),
            (CambioForma::TipoFlecha(t), _) => TipoFlecha::de(e) == Some(t),
            // Lo que se ve: sin `textAlign` el texto va a la izquierda.
            (CambioForma::Alineacion(a), Figura::Texto { .. }) => {
                e.extras.alineacion.unwrap_or_default() == a
            }
            (CambioForma::AlineacionVertical(v), Figura::Texto { .. }) => {
                e.extras.alineacion_vertical.unwrap_or_default() == v
            }
            (CambioForma::Desenfoque(v), Figura::Mosaico { desenfoque }) => *desenfoque == v,
            (CambioForma::AumentoLupa(z), Figura::Lupa { cristal }) => {
                let caja = pixpin_motor2d::lupa_elemento::caja_de(e);
                (pixpin_motor2d::lupa_elemento::aumento_de(cristal, caja) - z).abs() < 0.05
            }
            (CambioForma::GuiaLupa(g), Figura::Lupa { cristal }) => cristal.guia == g,
            (CambioForma::Oscurecer(n), Figura::Foco { cristal }) => {
                pixpin_motor2d::lupa_elemento::oscurecimiento_de(cristal) == n
            }
            (CambioForma::ZonaFoco(z), Figura::Foco { cristal }) => {
                let caja = pixpin_motor2d::lupa_elemento::caja_de(e);
                (pixpin_motor2d::lupa_elemento::zona_de(cristal, caja) - z).abs() < 0.01
            }
            (CambioForma::Degradado(t), _) => e.extras.degradado.map(|d| d.tipo) == t,
            (CambioForma::CierreArco(c), Figura::Arco { .. }) => e.extras.cierre == c,
            (CambioForma::ArcosAngulo(n), Figura::Arco { .. }) => {
                e.extras.marca_angulo.as_ref().is_some_and(|m| m.arcos == n)
            }
            (CambioForma::ModoAngulo(m), Figura::Arco { .. }) => e
                .extras
                .marca_angulo
                .as_ref()
                .is_some_and(|x| pixpin_ui::panel_lateral::modo_de_marca(x) == m),
            _ => false,
        }
    }

    /// Lo que tiene que haber pasado tras pulsar `accion` sobre `elegidos`.
    /// Devuelve por que no, si no paso.
    fn comprobar(
        accion: AccionPanel,
        elegidos: &[u64],
        antes: &Escena,
        despues: &Escena,
        gesto: &Gesto,
        abierto_antes: Option<Desplegable>,
    ) -> Result<(), String> {
        let a = |id: u64| antes.buscar(id).unwrap();
        let d = |id: u64| despues.buscar(id).unwrap();
        let pos = |e: &Escena, id: u64| e.elementos.iter().position(|x| x.id == id).unwrap();
        let caja = |e: &Escena, id: u64| e.buscar(id).unwrap().caja();
        match accion {
            AccionPanel::Abrir(des) => {
                // El mismo disparador abre y cierra.
                let quiero = (abierto_antes != Some(des)).then_some(des);
                let abierto = panel_para(gesto, despues, area(), 100)
                    .and_then(|p| p.emergente)
                    .map(|e| e.cual);
                if abierto != quiero {
                    return Err(format!("quedo {abierto:?} y tocaba {quiero:?}"));
                }
            }
            AccionPanel::Estilo(c) => {
                for &id in elegidos {
                    let llega = pl::admite(&accion, a(id));
                    if llega && !estilo_llego(d(id), c) {
                        return Err(format!("no llego a {id}"));
                    }
                    if !llega && d(id) != a(id) {
                        return Err(format!("toco a {id}, que no lo admite"));
                    }
                }
                // Y queda como «actual»: el gesto de la prueba empieza de
                // fabrica, asi que es la fabrica con solo este cambio.
                let mut s = EstiloDibujo::default();
                s.aplicar(c);
                if s != gesto.estilo {
                    return Err("no quedo como actual".into());
                }
            }
            AccionPanel::Forma(c) if !matches!(c, CambioForma::Presion(_)) => {
                let mut s = EstiloDibujo::default();
                s.aplicar_forma(c);
                if s != gesto.estilo {
                    return Err("no quedo como actual".into());
                }
                for &id in elegidos {
                    let llega = pl::admite(&accion, a(id));
                    if llega && !forma_llego(d(id), c) {
                        return Err(format!("no llego a {id}"));
                    }
                    if !llega && d(id) != a(id) {
                        return Err(format!("toco a {id}, que no lo admite"));
                    }
                }
            }
            AccionPanel::Forma(c) => {
                for &id in elegidos {
                    let llega = pl::admite(&accion, a(id));
                    if llega && !forma_llego(d(id), c) {
                        return Err(format!("no llego a {id}"));
                    }
                    if !llega && d(id) != a(id) {
                        return Err(format!("toco a {id}, que no lo admite"));
                    }
                }
            }
            AccionPanel::FondoLienzo(color) => {
                // El papel cambia y ningun elemento se entera: no es un
                // estilo.
                if despues.fondo != (ColorRgba { a: 1.0, ..color }) {
                    return Err(format!("el papel quedo {:?}", despues.fondo));
                }
                if despues.elementos != antes.elementos {
                    return Err("el papel toco elementos".into());
                }
            }
            // La serie va al gesto, para los proximos puntos: ningun
            // elemento se entera.
            AccionPanel::PedirMedida(v) => {
                if gesto.pedir_la_medida != v {
                    return Err(format!("pedir la medida quedo {}", gesto.pedir_la_medida));
                }
                if despues.elementos != antes.elementos {
                    return Err("el interruptor toco elementos".into());
                }
            }
            // El modo guia va al gesto y suelta lo elegido; el ojo, a la
            // escena. Ninguno toca un elemento.
            AccionPanel::ModoGuia(v) => {
                if gesto.modo_guia() != v {
                    return Err(format!("el modo guia quedo {}", gesto.modo_guia()));
                }
                if !gesto.seleccion.ids().is_empty() {
                    return Err("cambiar de mundo no solto lo elegido".into());
                }
                if despues.elementos != antes.elementos {
                    return Err("el modo guia toco elementos".into());
                }
            }
            AccionPanel::VerGuias(v) => {
                if despues.referencias_visibles != v {
                    return Err(format!(
                        "las guias quedaron {}",
                        despues.referencias_visibles
                    ));
                }
                if despues.elementos != antes.elementos {
                    return Err("esconder las guias toco elementos".into());
                }
            }
            AccionPanel::SeriePunto(s) => {
                if gesto.serie_de_punto != s {
                    return Err(format!("la serie quedo {:?}", gesto.serie_de_punto));
                }
                if despues.elementos != antes.elementos {
                    return Err("la serie toco elementos".into());
                }
            }
            AccionPanel::Capa(capa) => {
                let n = despues.elementos.len();
                let k = elegidos.len();
                let posiciones: Vec<usize> = elegidos.iter().map(|id| pos(despues, *id)).collect();
                let ok = match capa {
                    // Si ya estaba donde se pide, no se mueve nada.
                    _ if ya_esta(capa, antes, elegidos) => despues.elementos == antes.elementos,
                    Capa::Frente => posiciones.iter().all(|p| *p >= n - k),
                    Capa::Fondo => posiciones.iter().all(|p| *p < k),
                    Capa::Adelante => elegidos
                        .iter()
                        .any(|id| pos(despues, *id) > pos(antes, *id)),
                    Capa::Atras => elegidos
                        .iter()
                        .any(|id| pos(despues, *id) < pos(antes, *id)),
                };
                if !ok {
                    return Err(format!("orden {posiciones:?}"));
                }
                // Y lo que pinta la ventana, que recorre la rejilla y no la
                // lista: tiene que salir en el mismo orden.
                let mut rejilla = pixpin_motor2d::indice::Rejilla::nueva();
                rejilla.sincronizar(despues);
                let pintado = rejilla.candidatos((-1e5, -1e5, 1e5, 1e5));
                let escena: Vec<u64> = despues.visibles().map(|e| e.id).collect();
                if pintado != escena {
                    return Err("la rejilla pinta en otro orden que la escena".into());
                }
            }
            AccionPanel::Alinear(al) => {
                let valor = |id: u64| {
                    let (x0, y0, x1, y1) = caja(despues, id);
                    match al {
                        Alineacion::Izquierda => x0,
                        Alineacion::Derecha => x1,
                        Alineacion::CentroHorizontal => (x0 + x1) / 2.0,
                        Alineacion::Arriba => y0,
                        Alineacion::Abajo => y1,
                        Alineacion::CentroVertical => (y0 + y1) / 2.0,
                    }
                };
                let v0 = valor(elegidos[0]);
                if elegidos.iter().any(|id| (valor(*id) - v0).abs() > 0.01) {
                    return Err("no quedaron alineados".into());
                }
            }
            AccionPanel::Repartir(r) => {
                let mut c: Vec<f32> = elegidos
                    .iter()
                    .map(|id| {
                        let (x0, y0, x1, y1) = caja(despues, *id);
                        match r {
                            Reparto::Horizontal => (x0 + x1) / 2.0,
                            Reparto::Vertical => (y0 + y1) / 2.0,
                        }
                    })
                    .collect();
                c.sort_by(f32::total_cmp);
                let h = c[1] - c[0];
                if c.windows(2).any(|w| (w[1] - w[0] - h).abs() > 0.01) {
                    return Err(format!("huecos distintos: {c:?}"));
                }
            }
            AccionPanel::Duplicar => {
                let nuevos = gesto.seleccion.ids().to_vec();
                if nuevos.len() != elegidos.len() || nuevos.iter().any(|id| elegidos.contains(id)) {
                    return Err("no quedaron elegidas las copias".into());
                }
                if despues.visibles().count() != antes.visibles().count() + elegidos.len() {
                    return Err("no hay tantas copias como elegidos".into());
                }
                let grupos_viejos: Vec<&String> = elegidos
                    .iter()
                    .flat_map(|id| a(*id).grupos.iter())
                    .collect();
                for id in &nuevos {
                    if d(*id).grupos.iter().any(|g| grupos_viejos.contains(&g)) {
                        return Err("la copia se metio en el grupo del original".into());
                    }
                }
            }
            AccionPanel::Borrar => {
                if elegidos
                    .iter()
                    .any(|id| despues.visibles().any(|e| e.id == *id))
                {
                    return Err("sigue ahi".into());
                }
            }
            AccionPanel::Agrupar => {
                let g = d(elegidos[0]).grupos.last().cloned();
                if g.is_none() || elegidos.iter().any(|id| d(*id).grupos.last() != g.as_ref()) {
                    return Err("no comparten grupo".into());
                }
                // Lo que lo hace visible: pinchar uno con la mano coge todos.
                let mut otra = despues.clone();
                let mut mano = Gesto::default();
                mano.herramienta = Herramienta::Mano;
                let (x0, y0, x1, y1) = caja(despues, elegidos[0]);
                let p = Punto2::nuevo(x0, (y0 + y1) / 2.0);
                let _ = x1;
                mano.evento(
                    EventoGesto::Pulsar {
                        p,
                        shift: false,
                        alt: false,
                        presion: None,
                    },
                    &mut otra,
                    1.0,
                );
                let mut cogidos = mano.seleccion.ids().to_vec();
                let mut todos = elegidos.to_vec();
                cogidos.sort_unstable();
                todos.sort_unstable();
                if cogidos != todos {
                    return Err(format!("pinchar uno del grupo coge {cogidos:?}"));
                }
            }
            AccionPanel::Desagrupar => {
                if elegidos
                    .iter()
                    .any(|id| d(*id).grupos.len() + 1 != a(*id).grupos.len())
                {
                    return Err("no se deshizo el grupo".into());
                }
            }
            // Quita el panel: no toca la escena; queda solo su boton.
            AccionPanel::Plegar => {
                if despues.elementos != antes.elementos {
                    return Err("quitar el panel cambio el dibujo".into());
                }
                con_boton_plegar(true);
                let quitado = panel_para(gesto, despues, area(), 100).is_some_and(|p| p.plegado);
                con_boton_plegar(false);
                poner_plegado(false);
                if !quitado {
                    return Err("el panel no se quito".into());
                }
            }
            // No toca la escena: deja apuntado el cajetin para el editor.
            AccionPanel::GuardarFigura => {
                if despues.elementos != antes.elementos {
                    return Err("guardar en figuras cambio el dibujo".into());
                }
                if !tomar_guardar_figura() {
                    return Err("no quedo pedido el cajetin del nombre".into());
                }
            }
            // El campo del codigo: no toca la escena, empieza a escribir con
            // el codigo que ensena, entero y elegido.
            AccionPanel::EditarHex => {
                let ed = HEX.with(|h| h.borrow().clone());
                let ensena = panel_para(gesto, despues, area(), 100)
                    .and_then(|p| p.emergente)
                    .and_then(|e| {
                        e.controles.iter().find_map(|c| match *c {
                            Control::Hex { color, .. } => Some(pl::codigo_hex(color)),
                            _ => None,
                        })
                    })
                    .flatten()
                    .unwrap_or_default();
                match ed {
                    Some(ed) if ed.texto() == ensena && despues.elementos == antes.elementos => {}
                    otra => return Err(format!("escribiendo {otra:?} y ensena {ensena}")),
                }
            }
        }
        Ok(())
    }

    /// Si lo elegido ya esta donde lo manda `capa`: arriba del todo para
    /// subir, abajo del todo para bajar. Entonces el boton no mueve nada, y
    /// no tiene por que.
    fn ya_esta(capa: Capa, escena: &Escena, elegidos: &[u64]) -> bool {
        let n = escena.elementos.len();
        let k = elegidos.len();
        let rango = match capa {
            Capa::Frente | Capa::Adelante => n - k..n,
            Capa::Fondo | Capa::Atras => 0..k,
        };
        escena.elementos[rango]
            .iter()
            .all(|e| elegidos.contains(&e.id))
    }

    /// Si pulsar esto sobre lo elegido cambia la escena (pulsar el color que
    /// ya tiene no la cambia, y no tiene por que).
    fn deberia_cambiar(accion: AccionPanel, escena: &Escena, ids: &[u64]) -> bool {
        let elegidos: Vec<&Elemento> = ids.iter().map(|id| escena.buscar(*id).unwrap()).collect();
        match accion {
            AccionPanel::Abrir(_) | AccionPanel::EditarHex | AccionPanel::GuardarFigura => false,
            // El modo guia y el ojo no dibujan ni van al historial, como en
            // el movil: los mira su propia prueba.
            AccionPanel::ModoGuia(_) | AccionPanel::VerGuias(_) => false,
            AccionPanel::Capa(c) => !ya_esta(c, escena, ids),
            AccionPanel::Estilo(c) => elegidos
                .iter()
                .any(|e| pl::admite(&accion, e) && !estilo_llego(e, c)),
            AccionPanel::Forma(c) => elegidos
                .iter()
                .any(|e| pl::admite(&accion, e) && !forma_llego(e, c)),
            _ => true,
        }
    }

    #[test]
    fn cada_boton_del_panel_pulsado_como_en_la_ventana_cambia_lo_que_debe_y_se_deshace() {
        let (base, ids) = escena_con_de_todo();
        let [
            caja,
            rombo,
            ovalo,
            rotulo,
            flecha,
            trazo,
            raya,
            rotulo_en_caja,
        ] = ids[..]
        else {
            panic!("ocho figuras");
        };
        // Las selecciones que sacan todas las secciones: todo junto, las dos
        // con esquinas, cada una con mandos propios sola, y un grupo.
        let mut agrupada = base.clone();
        {
            let mut s = Seleccion::nueva();
            s.poner_todos([caja, ovalo]);
            organizar::agrupar(&mut agrupada, &s);
        }
        let casos: Vec<(&str, &Escena, Vec<u64>)> = vec![
            ("todo", &base, ids.clone()),
            ("esquinas", &base, vec![caja, rombo]),
            ("flecha", &base, vec![flecha]),
            ("texto", &base, vec![rotulo]),
            ("rotulo de la caja", &base, vec![rotulo_en_caja]),
            ("lapiz", &base, vec![trazo]),
            ("linea", &base, vec![raya]),
            ("grupo", &agrupada, vec![caja, ovalo]),
        ];
        let mut fallos: Vec<String> = Vec::new();
        let mut pulsados = 0;
        for (nombre, escena0, elegidos) in casos {
            // Los desplegables se abren primero, pulsando su disparador: sus
            // controles solo existen con el abierto.
            for abierto in [
                None,
                Some(Desplegable::ColorTrazo),
                Some(Desplegable::ColorFondo),
                Some(Desplegable::PuntaInicio),
                Some(Desplegable::PuntaFin),
            ] {
                cerrar_desplegable();
                let mut gesto = Gesto::default();
                gesto.herramienta = Herramienta::Mano;
                gesto.seleccion.poner_todos(elegidos.clone());
                let mut escena = escena0.clone();
                if let Some(d) = abierto {
                    let Some(panel) = panel_para(&gesto, &escena, area(), 100) else {
                        continue;
                    };
                    let Some(disparador) = panel.controles.iter().find_map(|c| match *c {
                        Control::Muestra {
                            rect,
                            accion: Some(AccionPanel::Abrir(x)),
                            ..
                        }
                        | Control::Punta {
                            rect,
                            accion: AccionPanel::Abrir(x),
                            ..
                        } if x == d => Some(rect),
                        _ => None,
                    }) else {
                        continue;
                    };
                    clic(centro(disparador), &mut gesto, &mut escena);
                }
                let panel = panel_para(&gesto, &escena, area(), 100).expect("hay panel");
                for p in pulsables(&panel) {
                    // Cada boton, sobre una copia limpia: con el desplegable
                    // como estaba y la seleccion de partida.
                    let mut escena = escena.clone();
                    let mut g = Gesto::default();
                    g.herramienta = Herramienta::Mano;
                    g.seleccion.poner_todos(elegidos.clone());
                    cerrar_desplegable();
                    if let Some(d) = abierto {
                        ABIERTO.with(|a| a.set(Some(d)));
                    }
                    let pl::DestinoPanel::Accion(accion) = panel.destino(p) else {
                        fallos.push(format!("{nombre}: en {p:?} no hay boton"));
                        continue;
                    };
                    let antes = escena.clone();
                    let cambia = deberia_cambiar(accion, &antes, &elegidos);
                    pulsados += 1;
                    let r = clic(p, &mut g, &mut escena);
                    if !matches!(r, ClicPanel::Suyo { .. }) {
                        fallos.push(format!("{nombre}: {accion:?} no llego al panel"));
                        continue;
                    }
                    if let Err(e) = comprobar(accion, &elegidos, &antes, &escena, &g, abierto) {
                        fallos.push(format!("{nombre}: {accion:?}: {e}"));
                        continue;
                    }
                    if !cambia {
                        continue;
                    }
                    // Que se VEA: lo que pinta la escena tiene que cambiar.
                    // Agrupar no cambia el dibujo; lo suyo se ve al pinchar,
                    // y eso ya lo mira `comprobar`.
                    let dibujo = pixpin_motor2d::pintado::ordenes_de_escena;
                    if !matches!(accion, AccionPanel::Agrupar | AccionPanel::Desagrupar)
                        && dibujo(&antes) == dibujo(&escena)
                    {
                        fallos.push(format!("{nombre}: {accion:?}: no cambia nada que se vea"));
                    }
                    // Y figura a figura: que cambie otra no tapa que a esta
                    // el boton no le haga nada (un rombo redondo y un
                    // rectangulo que seguia en pico pasaban por buenos).
                    for id in &elegidos {
                        let (a, d) = (antes.buscar(*id).unwrap(), escena.buscar(*id).unwrap());
                        let tocaba = match accion {
                            // Sin fondo no hay nada que rayar: el estilo del
                            // relleno se guarda y no se ve, como en Excalidraw.
                            AccionPanel::Estilo(CambioEstilo::EstiloRelleno(_))
                                if a.relleno.is_none() =>
                            {
                                false
                            }
                            AccionPanel::Estilo(c) => pl::admite(&accion, a) && !estilo_llego(a, c),
                            AccionPanel::Forma(c) => pl::admite(&accion, a) && !forma_llego(a, c),
                            _ => false,
                        };
                        let pinta = pixpin_motor2d::pintado::ordenes;
                        if tocaba && pinta(a) == pinta(d) {
                            fallos.push(format!("{nombre}: {accion:?}: a {id} no se le ve"));
                        }
                    }
                    if !escena.deshacer() || sin_version(&escena) != sin_version(&antes) {
                        fallos.push(format!("{nombre}: {accion:?}: deshacer no lo revierte"));
                    }
                }
            }
        }
        cerrar_desplegable();
        assert!(pulsados > 150, "se pulsaron muy pocos: {pulsados}");
        assert!(
            fallos.is_empty(),
            "{} fallos:\n{}",
            fallos.len(),
            fallos.join("\n")
        );
    }

    /// Dibuja con la herramienta puesta de (400, 300) a (560, 420) y
    /// devuelve lo que nacio.
    fn dibujar(gesto: &mut Gesto, escena: &mut Escena) -> Elemento {
        let ev = |p: Punto2, fase: u8| match fase {
            0 => EventoGesto::Pulsar {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            1 => EventoGesto::Mover {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            _ => EventoGesto::Soltar { p },
        };
        gesto.evento(ev(Punto2::nuevo(400.0, 300.0), 0), escena, 1.0);
        if gesto.herramienta != Herramienta::Texto {
            gesto.evento(ev(Punto2::nuevo(480.0, 350.0), 1), escena, 1.0);
            gesto.evento(ev(Punto2::nuevo(560.0, 420.0), 1), escena, 1.0);
            gesto.evento(ev(Punto2::nuevo(560.0, 420.0), 2), escena, 1.0);
        }
        escena.elementos.last().cloned().expect("nacio algo")
    }

    #[test]
    fn cada_boton_sin_nada_elegido_se_queda_para_la_figura_que_se_dibuja_despues() {
        let mut fallos = Vec::new();
        let mut pulsados = 0;
        for h in [
            Herramienta::Rectangulo,
            Herramienta::Rombo,
            Herramienta::Elipse,
            Herramienta::Flecha,
            Herramienta::FlechaCodos,
            Herramienta::Linea,
            Herramienta::Texto,
            Herramienta::Lapiz,
        ] {
            for abierto in [
                None,
                Some(Desplegable::ColorTrazo),
                Some(Desplegable::ColorFondo),
                Some(Desplegable::PuntaInicio),
                Some(Desplegable::PuntaFin),
            ] {
                cerrar_desplegable();
                ABIERTO.with(|a| a.set(abierto));
                let mut gesto = Gesto::default();
                gesto.herramienta = h;
                // Con fondo, para que salga tambien la fila del relleno.
                gesto.estilo.relleno = pl::COLORES_FONDO[2];
                let escena = Escena::nueva();
                let Some(panel) = panel_para(&gesto, &escena, area(), 100) else {
                    continue;
                };
                if abierto.is_some() && panel.emergente.is_none() {
                    continue;
                }
                for p in pulsables(&panel) {
                    let mut escena = Escena::nueva();
                    let mut g = Gesto::default();
                    g.herramienta = h;
                    g.estilo.relleno = pl::COLORES_FONDO[2];
                    ABIERTO.with(|a| a.set(abierto));
                    let pl::DestinoPanel::Accion(accion) = panel.destino(p) else {
                        fallos.push(format!("{h:?}: en {p:?} no hay boton"));
                        continue;
                    };
                    pulsados += 1;
                    clic(p, &mut g, &mut escena);
                    if !escena.elementos.is_empty() {
                        fallos.push(format!("{h:?}: {accion:?} toco la escena sin nada elegido"));
                    }
                    cerrar_desplegable();
                    let nuevo = dibujar(&mut g, &mut escena);
                    if !pl::admite(&accion, &nuevo) {
                        continue;
                    }
                    let ok = match accion {
                        AccionPanel::Estilo(c) => estilo_llego(&nuevo, c),
                        AccionPanel::Forma(c) => forma_llego(&nuevo, c),
                        _ => true,
                    };
                    if !ok {
                        fallos.push(format!("{h:?}: {accion:?} no llego a la figura nueva"));
                    }
                }
            }
        }
        cerrar_desplegable();
        assert!(pulsados > 100, "se pulsaron muy pocos: {pulsados}");
        assert!(
            fallos.is_empty(),
            "{} fallos:\n{}",
            fallos.len(),
            fallos.join("\n")
        );
    }

    #[test]
    fn un_rectangulo_dibujado_con_bordes_redondos_elegidos_nace_redondo_y_una_elipse_no() {
        let mut gesto = Gesto::default();
        let mut escena = Escena::nueva();
        gesto.herramienta = Herramienta::Rectangulo;
        aplicar(
            AccionPanel::Forma(CambioForma::Bordes { redondo: true }),
            &mut gesto,
            &mut escena,
        );
        assert!(dibujar(&mut gesto, &mut escena).redondo);
        // Caso negativo: la elipse no tiene esquinas, y un `roundness` en
        // ella seria un dato que no se ve.
        gesto.herramienta = Herramienta::Elipse;
        assert!(!dibujar(&mut gesto, &mut escena).redondo);
    }

    #[test]
    fn un_clic_fuera_del_panel_sigue_hacia_el_lienzo_y_cierra_el_desplegable() {
        let (mut escena, ids) = escena_con_de_todo();
        let mut gesto = Gesto::default();
        gesto.seleccion.poner(ids[0]);
        aplicar(
            AccionPanel::Abrir(Desplegable::ColorTrazo),
            &mut gesto,
            &mut escena,
        );
        let lejos = Punto { x: 1200, y: 800 };
        assert_eq!(
            atender_clic(lejos, true, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Fuera { cerro: true }
        );
        // Caso negativo: el segundo ya no tiene nada que cerrar.
        assert_eq!(
            atender_clic(lejos, true, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Fuera { cerro: false }
        );
    }

    #[test]
    fn el_boton_arriba_de_borrar_no_llega_al_lienzo_aunque_el_panel_ya_no_este() {
        let (mut escena, ids) = escena_con_de_todo();
        let mut gesto = Gesto::default();
        gesto.herramienta = Herramienta::Mano;
        gesto.seleccion.poner(ids[0]);
        let panel = panel_para(&gesto, &escena, area(), 100).unwrap();
        let borrar = panel
            .controles
            .iter()
            .find_map(|c| match *c {
                Control::Opcion {
                    rect,
                    accion: AccionPanel::Borrar,
                    ..
                } => Some(rect),
                _ => None,
            })
            .unwrap();
        let p = centro(borrar);
        assert!(matches!(
            atender_clic(p, true, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Suyo { repintar: true }
        ));
        // Ya no hay nada elegido y con la mano no hay panel...
        assert!(panel_para(&gesto, &escena, area(), 100).is_none());
        // ...pero el boton-arriba sigue siendo del panel.
        assert_eq!(
            atender_clic(p, false, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Suyo { repintar: false }
        );
        // Caso negativo: el siguiente, sin boton-abajo en el panel, ya es
        // del lienzo.
        assert_eq!(
            atender_clic(p, false, &mut gesto, &mut escena, area(), 100),
            ClicPanel::Fuera { cerro: false }
        );
    }

    #[test]
    fn todas_las_claves_de_los_titulos_estan_traducidas_en_los_dos_idiomas() {
        use Seccion::*;
        for idioma in [Idioma::Espanol, Idioma::Ingles] {
            let c = Catalogo::nuevo(idioma);
            for s in [
                Trazo,
                Fondo,
                Relleno,
                Grosor,
                EstiloTrazo,
                Presion,
                TrazoAMano,
                Bordes,
                TipoFlecha,
                Fuente,
                TamanoFuente,
                AlineacionTexto,
                Puntas,
                Opacidad,
                Capas,
                Alinear,
                Acciones,
                Colores,
                Tonos,
                CodigoHex,
                FondoLienzo,
                PapelesDelMovil,
            ] {
                // `t` devuelve la propia clave cuando falta.
                assert_ne!(c.t(clave(s)), clave(s), "{idioma:?} sin {s:?}");
            }
        }
    }

    #[test]
    fn todo_boton_del_panel_tiene_icono() {
        // Un boton sin icono es un cuadrado gris que no dice que hace.
        let mut escena = Escena::nueva();
        let ids = [
            con(&mut escena, Figura::Rectangulo),
            con(&mut escena, texto()),
            con(
                &mut escena,
                Figura::Flecha {
                    puntos: Vec::new(),
                    punta_inicio: TipoPunta::Ninguna,
                    punta_fin: TipoPunta::Flecha,
                    codos: false,
                },
            ),
            con(
                &mut escena,
                Figura::Lapiz {
                    puntos: Vec::new(),
                    presiones: Vec::new(),
                    opciones: None,
                },
            ),
        ];
        let mut gesto = Gesto::default();
        gesto.seleccion.poner_todos(ids);
        let panel = panel_para(&gesto, &escena, area(), 100).unwrap();
        for c in &panel.controles {
            if let Control::Opcion { accion, .. } = c {
                assert!(icono_de(*accion).is_some(), "{accion:?} sin icono");
            }
        }
    }

    /// **El panel pintado de verdad**, en PNG, para mirarlo. Necesita GPU y
    /// sesion de escritorio: `cargo test -p pixpin --bin pixpinmax
    /// muestra_del_panel -- --ignored --nocapture`. Deja los PNG en
    /// `PIXPIN_MUESTRAS` o en la carpeta temporal.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_del_panel() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;

        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let (ancho, alto) = (520u32, 900u32);
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let area = Rect {
            x: 0,
            y: 0,
            ancho,
            alto,
        };

        let mut escena = Escena::nueva();
        let caja = con(&mut escena, Figura::Rectangulo);
        escena.buscar_mut(caja).unwrap().relleno = pl::COLORES_FONDO[3];
        escena.buscar_mut(caja).unwrap().redondo = true;
        let rotulo = con(&mut escena, texto());
        let flecha = con(
            &mut escena,
            Figura::Flecha {
                puntos: Vec::new(),
                punta_inicio: TipoPunta::Circulo,
                punta_fin: TipoPunta::Triangulo,
                codos: true,
            },
        );
        let trazo = con(
            &mut escena,
            Figura::Lapiz {
                puntos: Vec::new(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
        );

        // Un grafito 4B y una guia: el panel del grafito con sus durezas, el
        // modo guia y el ojo.
        let grafito = con(
            &mut escena,
            Figura::Lapiz {
                puntos: Vec::new(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
        );
        {
            let g = escena.buscar_mut(grafito).unwrap();
            g.material = pixpin_motor2d::tinta::MaterialTinta::Cuadritos;
            g.extras.dureza = Some(pixpin_motor2d::tinta::grafito::Dureza::B4);
        }
        let guia = con(&mut escena, Figura::Elipse);
        escena.buscar_mut(guia).unwrap().extras.referencia = true;

        let casos: [(&str, Vec<u64>, Option<Desplegable>); 7] = [
            ("grafito-y-guias", vec![grafito], None),
            ("rectangulo", vec![caja], None),
            (
                "rectangulo-paleta-fondo",
                vec![caja],
                Some(Desplegable::ColorFondo),
            ),
            ("texto", vec![rotulo], None),
            (
                "flecha-puntas-inicio",
                vec![flecha],
                Some(Desplegable::PuntaInicio),
            ),
            (
                "lapiz-paleta-trazo",
                vec![trazo],
                Some(Desplegable::ColorTrazo),
            ),
            ("todo", vec![caja, rotulo, flecha, trazo], None),
        ];
        for (nombre, ids, abierto) in casos {
            let mut gesto = Gesto::default();
            gesto.seleccion.poner_todos(ids);
            ABIERTO.with(|a| a.set(abierto));
            let panel = panel_para(&gesto, &escena, area, 100).expect("panel");
            motor
                .dibujar(&fuera.destino, |p| {
                    p.limpiar(hex(0xffffff));
                    pintar(p, &panel, 100);
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
            let ruta = carpeta.join(format!("panel-{nombre}.png"));
            std::fs::write(&ruta, png).expect("guardar");
            println!("{nombre}: {}", ruta.display());
        }
        ABIERTO.with(|a| a.set(None));
    }
}
