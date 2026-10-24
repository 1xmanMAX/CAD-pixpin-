//! **El anotador de pantalla**: el lienzo entero, encima del escritorio.
//!
//! Lo pidio el usuario el 2026-09-24 («implementa una funcion para poder
//! anotar en pantalla, como un anotador de pantalla») junto con que las
//! herramientas de dibujo fueran las mismas en todas partes. Por eso no es
//! otra ventana con otra maquina (lo que era `capa.rs` con el `Anotador`
//! viejo, once herramientas y ni capa de tinta ni panel): **es el editor**,
//! con un anfitrion `Pantalla` que cambia solo lo que tiene que cambiar:
//!
//! - la ventana cubre el **escritorio virtual entero** (todos los monitores);
//!   las coordenadas son pixeles fisicos, y la camara se queda quieta a 1:1
//!   aunque cada monitor tenga su escala: lo dibujado cae donde esta el
//!   cursor en cualquiera de ellos. La barra y el panel van en el monitor
//!   principal, a su escala;
//! - **no se navega**: ni rueda ni espacio+arrastre mueven la pantalla;
//! - **viva**: el papel es transparente (la cadena de intercambio ya es
//!   premultiplicada) y debajo sigue todo moviendose. **Espacio** deja pasar
//!   los clics a lo de debajo y lo vuelve a coger; **Ctrl** mantenido los
//!   deja pasar mientras dure. Pasante, la barra y el panel se esconden;
//! - **congelada**: el fondo es la foto del escritorio tomada ANTES de abrir
//!   la ventana (para que no salga en ella);
//! - **Escape sale** (si no hay un texto abierto ni algo elegido: entonces
//!   hace lo de siempre). Desde el 2026-09-29, al salir (Escape, Salir o
//!   cerrar) con algo nuevo dibujado **se guarda solo en «Mensajes
//!   guardados»**: la captura de debajo con la tinta editable encima
//!   ([`guardar_en_el_chat`]), sin preguntar ni crear un pin. Asi Escape
//!   nunca tira lo dibujado, que es lo que protegia D54 (su pregunta y su
//!   pin quedan solo para un anotador sin almacen, que `main` no abre);
//! - sin marcas, sin F11 y sin exportar: son del lienzo, y aqui lo que se
//!   guarda es la foto.
//!
//! Todo lo demas —las herramientas, la capa de tinta de baja latencia, el
//! horneado, las zonas, el panel— es el editor tal cual.
//!
//! Desde el 2026-09-28 se abre tambien con **Alt + doble clic central** y
//! lleva abajo **la pastilla** de `CapaPantalla.kt` en su propia ventana
//! (`pastilla_pantalla.rs`): clic a traves, limpiar, copiar, guardar en
//! «Mensajes guardados» (la captura de debajo con la tinta editable encima,
//! `anotador_al_chat.rs`) y salir.
//!
//! Lo unico que el editor no tenia es **la lupa viva** (D52, D60) de la capa
//! vieja: con la lupa (Q) elegida, un cristal amplia lo que hay alrededor
//! del cursor y la rueda sube o baja el aumento. Va aqui, en [`LupaViva`].

use pixpin_geom::{Punto, Rect};

/// Viva (la pantalla se sigue moviendo debajo) o congelada (sobre una foto).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    Viva,
    Congelada,
}

/// Lo que el editor necesita para ser anotador de pantalla.
pub struct Pantalla<'a> {
    pub modo: Modo,
    /// El escritorio virtual: la ventana lo cubre entero.
    pub escritorio: Rect,
    /// El monitor principal: ahi van la barra y el panel.
    pub principal: pixpin_geom::Monitor,
    /// La foto del escritorio virtual (solo en congelada), del tamano de
    /// `escritorio`.
    pub foto: Option<pixpin_codec::ImagenRgba>,
    /// Hace la foto de lo que se ve ahora (el dibujo sobre lo de debajo,
    /// sin interfaz). La pone quien tiene con que capturar (`main`).
    pub capturar: &'a mut dyn FnMut() -> Option<pixpin_codec::ImagenRgba>,
    /// La foto que se hizo al salir, si habia algo dibujado.
    pub resultado: Option<pixpin_codec::ImagenRgba>,
    /// El almacen del chat, para «Guardar en Mensajes guardados» de la
    /// pastilla (`anotador_al_chat`). `None`: la pastilla no guarda.
    pub raiz: Option<std::path::PathBuf>,
    /// La ventana de mensajes de `main`, dueña del icono de la bandeja: los
    /// globos de guardado y copiado salen sobre el. 0 = sin globos.
    pub avisos: isize,
}

/// Los globos del anotador, ya traducidos.
pub fn globo(textos: &pixpin_store::Catalogo) -> crate::anotador_al_chat::Globo {
    crate::anotador_al_chat::Globo {
        titulo: textos.t("anotador-globo-titulo"),
        hecho: textos.t("anotador-globo-guardado"),
        fallo: textos.t("anotador-globo-fallo"),
    }
}

/// **La foto de lo que hay DEBAJO del anotador**, sin la tinta ni la barra
/// ni la pastilla: lo que va de fondo al guardar en el chat.
///
/// Congelada, es la foto que ya se tomo al abrir (el fondo). Viva, se
/// esconden las dos ventanas un instante, se espera a que el compositor lo
/// haya puesto en pantalla (sin esperar, la captura puede salir con ellas,
/// D59) y se fotografia; luego vuelven. Es lo que hace el movil al copiar
/// (`setVisible(false)`, 64 ms, `ProjectionSession.grab()`): se nota como un
/// parpadeo de la tinta, y es el precio de no fotografiarse a si mismo.
pub fn foto_de_debajo(
    pa: &mut Pantalla<'_>,
    fondo: Option<&crate::fondo_lienzo::FondoLienzo>,
    ventana: &pixpin_shell::overlay::VentanaOverlay,
    pastilla: Option<&super::pastilla_pantalla::Pastilla>,
    con_foco: bool,
) -> Option<pixpin_codec::ImagenRgba> {
    if pa.modo == Modo::Congelada {
        return fondo.and_then(|f| f.imagen()).cloned();
    }
    ventana.ocultar();
    if let Some(p) = pastilla {
        p.ocultar();
    }
    pixpin_shell::overlay::bombear_pendientes();
    pixpin_shell::esperar_composicion();
    let foto = (pa.capturar)();
    ventana.mostrar();
    ventana.traer_encima();
    if let Some(p) = pastilla {
        p.mostrar();
    }
    // Esconderla le quito el foco: dibujando, se le devuelve (con el clic a
    // traves, el teclado es de la aplicacion de debajo y alli se queda).
    if con_foco {
        ventana.enfocar();
    }
    ventana.invalidar();
    foto
}

/// **La foto de debajo al SALIR**: como [`foto_de_debajo`], pero la ventana
/// se queda escondida (se esta cerrando: ensenarla otra vez seria un
/// parpadeo de la tinta para nada). La pastilla ya no esta.
pub fn foto_al_salir(
    pa: &mut Pantalla<'_>,
    fondo: Option<&crate::fondo_lienzo::FondoLienzo>,
    ventana: &pixpin_shell::overlay::VentanaOverlay,
) -> Option<pixpin_codec::ImagenRgba> {
    if pa.modo == Modo::Congelada {
        return fondo.and_then(|f| f.imagen()).cloned();
    }
    ventana.ocultar();
    pixpin_shell::overlay::bombear_pendientes();
    pixpin_shell::esperar_composicion();
    (pa.capturar)()
}

/// **Guarda lo anotado en «Mensajes guardados»** segun la regla de
/// `anotador_al_chat::que_hacer`: la primera vez con tinta, la captura de
/// debajo (`foto`, que solo se pide entonces: en viva esconde las ventanas)
/// con la tinta como elementos editables; si ya se guardo en esta sesion,
/// el MISMO lienzo puesto al dia; sin nada nuevo, nada. Lo usan el boton
/// de la pastilla y la salida (Escape, Salir o cerrar), que guarda sola.
/// Devuelve lo que hizo; sin almacen (`raiz` = `None`), nada.
pub fn guardar_en_el_chat(
    chat: &mut crate::anotador_al_chat::EnElChat,
    pa: &mut Pantalla<'_>,
    tinta: Vec<pixpin_motor2d::Elemento>,
    pegadas: Vec<crate::anotador_al_chat::Pegada>,
    globo: crate::anotador_al_chat::Globo,
    foto: impl FnOnce(&mut Pantalla<'_>) -> Option<pixpin_codec::ImagenRgba>,
) -> crate::anotador_al_chat::Paso {
    use crate::anotador_al_chat::{Paso, Sesion};
    let Some(raiz) = pa.raiz.clone() else {
        return Paso::Nada;
    };
    let firma: Vec<_> = pegadas.iter().map(|p| p.elemento.clone()).collect();
    match chat.paso(&tinta, &firma) {
        Paso::Nada => Paso::Nada,
        Paso::Nuevo => match foto(pa) {
            Some(foto) => {
                tracing::info!(
                    trazos = tinta.len(),
                    ancho = foto.ancho,
                    alto = foto.alto,
                    "anotador: a Mensajes guardados"
                );
                // Solo los monitores con algo dibujado, a su tamano: la foto
                // del escritorio entero el movil la reducia hasta no leerse.
                let monitores: Vec<Rect> = pixpin_capture::enumerar_monitores()
                    .map(|d| {
                        d.monitores()
                            .iter()
                            .map(|m| Rect {
                                x: m.area.x - pa.escritorio.x,
                                y: m.area.y - pa.escritorio.y,
                                ..m.area
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let cajas: Vec<(f32, f32, f32, f32)> = tinta
                    .iter()
                    .chain(pegadas.iter().map(|p| &p.elemento))
                    .map(|e| e.caja())
                    .collect();
                let recorte = crate::anotador_al_chat::monitores_con_dibujo(
                    &monitores,
                    &cajas,
                    (foto.ancho, foto.alto),
                );
                chat.guardar_nuevo(
                    raiz,
                    Sesion {
                        foto,
                        tinta,
                        pegadas,
                    },
                    recorte,
                    pa.avisos,
                    globo,
                );
                Paso::Nuevo
            }
            None => {
                tracing::warn!("sin foto de debajo: la pantalla anotada no se pudo guardar");
                Paso::Nada
            }
        },
        Paso::AlDia => {
            tracing::info!(
                trazos = tinta.len(),
                "anotador: al dia en Mensajes guardados"
            );
            chat.poner_al_dia(raiz, tinta, pegadas, pa.avisos, globo);
            Paso::AlDia
        }
    }
}

impl Pantalla<'_> {
    pub fn anfitrion(&self) -> crate::dibujo::permitidas::Anfitrion {
        match self.modo {
            Modo::Viva => crate::dibujo::permitidas::Anfitrion::PantallaViva,
            Modo::Congelada => crate::dibujo::permitidas::Anfitrion::PantallaCongelada,
        }
    }
}

/// La camara del anotador: quieta y a 1:1 con los pixeles fisicos del
/// escritorio. `vista_efectiva` multiplica por la escala del monitor, asi
/// que aqui se divide: con el 150 % del principal, un trazo de 100 px
/// seguiria midiendo 100 px y no 150.
pub fn camara_quieta(escala_por_cien: u32) -> pixpin_motor2d::camara::Camara {
    pixpin_motor2d::camara::Camara {
        x: 0.0,
        y: 0.0,
        zoom: 100.0 / escala_por_cien.max(1) as f32,
    }
}

/// El papel de la escena: transparente en viva (se ve lo de debajo);
/// blanco en congelada, que la foto tapa entero.
pub fn papel(modo: Modo) -> pixpin_motor2d::ColorRgba {
    match modo {
        Modo::Viva => pixpin_motor2d::ColorRgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        },
        Modo::Congelada => pixpin_motor2d::ColorRgba::opaco(1.0, 1.0, 1.0),
    }
}

/// Lo que hace una tecla que es del anotador y no de las herramientas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeclaPantalla {
    /// Escape sin nada que soltar: salir (con foto si hay dibujo).
    Salir,
    /// Espacio: alternar el pasante fijo (solo viva).
    AlternarPasante,
    /// Ctrl pulsado o soltado: pasante mientras dure (solo viva).
    Ctrl(bool),
}

const VK_ESCAPE: u32 = 0x1B;
const VK_ESPACIO: u32 = 0x20;
const VK_CONTROL: u32 = 0x11;

/// **Pura**: si un evento es de las teclas propias del anotador.
///
/// `libre` es que no se esta escribiendo un texto (entonces el espacio es un
/// espacio y Escape cierra el texto) y `sin_nada` que no hay nada elegido ni
/// un gesto a medias (entonces Escape suelta eso, como en el lienzo).
pub fn tecla(
    ev: &pixpin_shell::overlay::EventoOverlay,
    modo: Modo,
    libre: bool,
    sin_nada: bool,
) -> Option<TeclaPantalla> {
    use pixpin_shell::overlay::EventoOverlay as E;
    match *ev {
        E::Tecla { vk: VK_ESCAPE, .. } if libre && sin_nada => Some(TeclaPantalla::Salir),
        E::Tecla {
            vk: VK_ESPACIO,
            ctrl: false,
            ..
        } if libre && modo == Modo::Viva => Some(TeclaPantalla::AlternarPasante),
        E::Tecla { vk: VK_CONTROL, .. } if modo == Modo::Viva => Some(TeclaPantalla::Ctrl(true)),
        E::TeclaSoltada(VK_CONTROL) if modo == Modo::Viva => Some(TeclaPantalla::Ctrl(false)),
        _ => None,
    }
}

/// **Pura**: une las fotos de cada monitor en una del escritorio virtual.
/// Cada monitor va a su sitio (`area` en coordenadas del escritorio); lo que
/// ningun monitor cubre (un escritorio en L) queda negro opaco.
pub fn unir_fotos(
    escritorio: Rect,
    fotos: &[(Rect, pixpin_codec::ImagenRgba)],
) -> pixpin_codec::ImagenRgba {
    let (w, h) = (escritorio.ancho as usize, escritorio.alto as usize);
    let mut pixeles = vec![0u8; w * h * 4];
    for px in pixeles.chunks_exact_mut(4) {
        px[3] = 255;
    }
    for (area, img) in fotos {
        let dx = (area.x - escritorio.x) as isize;
        let dy = (area.y - escritorio.y) as isize;
        let (iw, ih) = (img.ancho as usize, img.alto as usize);
        for fila in 0..ih {
            let y = dy + fila as isize;
            if y < 0 || y as usize >= h {
                continue;
            }
            // Recortada a lo que cae dentro, por si la foto y la disposicion
            // no coinciden (se cambio la resolucion entre medias).
            let x0 = dx.max(0) as usize;
            let x1 = ((dx + iw as isize).min(w as isize)).max(0) as usize;
            if x1 <= x0 {
                continue;
            }
            let desde = (x0 as isize - dx) as usize;
            let origen = &img.pixeles[(fila * iw + desde) * 4..(fila * iw + desde + (x1 - x0)) * 4];
            let destino = &mut pixeles[(y as usize * w + x0) * 4..(y as usize * w + x1) * 4];
            destino.copy_from_slice(origen);
        }
    }
    pixpin_codec::ImagenRgba {
        ancho: escritorio.ancho,
        alto: escritorio.alto,
        pixeles,
    }
}

// ------------------------------------------------------------ la lupa viva

/// Aumento de la lupa al abrir, y sus topes (los de la capa vieja).
pub const LUPA_POR_DEFECTO: f32 = 2.0;
pub const LUPA_MINIMA: f32 = 1.5;
pub const LUPA_MAXIMA: f32 = 8.0;

/// **Pura**: el aumento tras una muesca de la rueda. Por factor y no de uno
/// en uno, como la capa vieja: x1,25 hacia arriba, x0,8 hacia abajo.
pub fn siguiente_aumento(actual: f32, delta: i32) -> f32 {
    let paso = if delta > 0 { 1.25 } else { 0.8 };
    (actual * paso).clamp(LUPA_MINIMA, LUPA_MAXIMA)
}

/// **Pura**: donde mira la lupa y donde se pinta.
///
/// Devuelve la region que amplia, en pixeles del MONITOR (los de su
/// fotograma), y el cuadrado donde se pinta, en pixeles de la VENTANA (que
/// cubre el escritorio virtual entero). El cuadrado cae en el mismo monitor
/// y fuera de su propia region (`Lupa::colocar_fuera`): lo que se captura
/// es la pantalla CON la lupa pintada, y encima de su fuente se ampliaria a
/// si misma en bucle.
pub fn colocar_lupa(
    aumento: f32,
    monitor: &pixpin_geom::Monitor,
    cursor: Punto,
    escritorio: Rect,
) -> (Rect, Rect) {
    let lupa = pixpin_ui::Lupa::con_aumento(monitor.escala_por_cien, aumento);
    let local = Rect {
        x: 0,
        y: 0,
        ancho: monitor.area.ancho,
        alto: monitor.area.alto,
    };
    let en_el_monitor = Punto {
        x: cursor.x - monitor.area.x,
        y: cursor.y - monitor.area.y,
    };
    let fuente = lupa.region_fuente(en_el_monitor, local);
    let pos = lupa.colocar_fuera(en_el_monitor, local);
    let destino = Rect {
        x: pos.x + monitor.area.x - escritorio.x,
        y: pos.y + monitor.area.y - escritorio.y,
        ancho: lupa.diametro,
        alto: lupa.diametro,
    };
    (fuente, destino)
}

/// **La lupa viva** del anotador de pantalla (la de `CapaViva`, D52/D60).
///
/// Mira la pantalla de verdad con una sesion de captura (WGC) del monitor
/// que hay bajo el cursor, que **solo existe mientras la lupa esta puesta**:
/// capturar el monitor entero en cada movimiento del raton costaba un 28 %
/// de CPU. Cada fotograma nuevo despierta la ventana y se repinta con el,
/// asi que la lupa ensena un video que avanza aunque el raton este quieto.
/// El tope de fotogramas es el del nivel (D14): 30 por segundo en el ligero.
///
/// En congelada mira lo mismo: la ventana ya ensena la foto, y la sesion la
/// ve con lo dibujado encima (y con el mosaico ya puesto, que en la capa
/// vieja habia que volver a tapar a mano porque ampliaba la foto sin el).
///
/// Pasante no hay lupa: la barra se esconde y el cristal tambien, que ahi
/// el raton es de la aplicacion de abajo.
pub struct LupaViva {
    monitores: Vec<pixpin_geom::Monitor>,
    escritorio: Rect,
    aumento: f32,
    /// Donde esta el raton, en coordenadas del escritorio.
    cursor: Option<Punto>,
    sesion: Option<(u32, pixpin_capture::SesionViva)>,
    /// Donde se pinto la ultima vez (pixeles de ventana), para borrarla al
    /// moverse: sin ella quedaria un rastro de lupas en la zona parcial.
    pintada: Option<Rect>,
    /// Hay que repintarla (se movio, cambio el aumento o llego fotograma).
    cambio: bool,
}

impl LupaViva {
    pub fn nueva(monitores: Vec<pixpin_geom::Monitor>, escritorio: Rect) -> Self {
        Self {
            monitores,
            escritorio,
            aumento: LUPA_POR_DEFECTO,
            cursor: None,
            sesion: None,
            pintada: None,
            cambio: false,
        }
    }

    fn monitor_bajo_el_cursor(&self) -> Option<&pixpin_geom::Monitor> {
        let c = self.cursor?;
        self.monitores.iter().find(|m| m.area.contiene(c))
    }

    /// Un evento de la ventana. Devuelve si se lo queda (la rueda con la
    /// lupa puesta: en el anotador la rueda no hace nada mas).
    pub fn evento(&mut self, ev: &pixpin_shell::overlay::EventoOverlay, puesta: bool) -> bool {
        use pixpin_shell::overlay::EventoOverlay as E;
        match *ev {
            E::RatonMovido(p) => {
                self.cursor = Some(p);
                self.cambio |= puesta;
                false
            }
            E::Rueda(delta) if puesta => {
                self.aumento = siguiente_aumento(self.aumento, delta);
                self.cambio = true;
                true
            }
            E::Despierta if self.sesion.is_some() => {
                self.cambio = true;
                false
            }
            _ => false,
        }
    }

    /// Abre la sesion del monitor bajo el cursor si hace falta, y la cierra
    /// si ya no (otra herramienta, pasante, o el cursor cambio de monitor).
    pub fn al_dia(
        &mut self,
        puesta: bool,
        dispositivo: &pixpin_capture::Dispositivo,
        nivel: pixpin_nivel::Nivel,
        hwnd: isize,
    ) {
        let quiere = if puesta {
            self.monitor_bajo_el_cursor().map(|m| m.id)
        } else {
            None
        };
        if self.sesion.as_ref().map(|(id, _)| *id) == quiere {
            return;
        }
        self.cerrar();
        // Al quitarla hay que borrar lo pintado; al ponerla, pintarla.
        self.cambio = true;
        let Some(id) = quiere else {
            return;
        };
        let tope = match nivel {
            pixpin_nivel::Nivel::Completo => std::time::Duration::ZERO,
            pixpin_nivel::Nivel::Ligero => std::time::Duration::from_millis(33),
        };
        let aviso = Some((hwnd, pixpin_shell::overlay::MSG_DESPIERTA));
        match pixpin_capture::SesionViva::nueva(dispositivo, id, tope, aviso) {
            Ok(s) => self.sesion = Some((id, s)),
            Err(e) => tracing::warn!(?e, "sin sesion en vivo; la lupa no vera la pantalla"),
        }
    }

    /// La sesion se cierra pase lo que pase: una olvidada dejaria un hilo
    /// de captura vivo para siempre.
    pub fn cerrar(&mut self) {
        if let Some((_, s)) = self.sesion.take() {
            tracing::info!(aceptados = s.aceptados(), "sesion de la lupa cerrada");
            s.cerrar();
        }
    }

    /// Si hay que repintar, la zona que cambia (pixeles de ventana): donde
    /// estaba y donde va a estar. `None` si no cambio nada.
    pub fn tomar_zona(&mut self) -> Option<(f32, f32, f32, f32)> {
        if !std::mem::take(&mut self.cambio) {
            return None;
        }
        let ahora = self.donde().map(|(_, d)| d);
        let caja = |r: Rect| {
            (
                r.x as f32,
                r.y as f32,
                (r.x + r.ancho as i32) as f32,
                (r.y + r.alto as i32) as f32,
            )
        };
        let zonas: Vec<_> = self.pintada.into_iter().chain(ahora).map(caja).collect();
        self.pintada = ahora;
        zonas
            .into_iter()
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
            // Aunque no haya lupa ni antes ni ahora hay que repintar: el
            // fotograma puede traer algo que ensenar el proximo.
            .or(Some((0.0, 0.0, 1.0, 1.0)))
    }

    fn donde(&self) -> Option<(Rect, Rect)> {
        self.sesion.as_ref()?;
        let m = self.monitor_bajo_el_cursor()?;
        Some(colocar_lupa(self.aumento, m, self.cursor?, self.escritorio))
    }

    /// El ultimo fotograma, envuelto como bitmap sin copiar (el bitmap ES la
    /// textura). Va ANTES de pintar: dentro del fotograma el motor esta
    /// prestado.
    pub fn preparar(
        &self,
        motor: &pixpin_render::MotorRender,
    ) -> Option<windows::Win32::Graphics::Direct2D::ID2D1Bitmap1> {
        let (_, s) = self.sesion.as_ref()?;
        motor.bitmap_desde_textura(&s.ultimo()?).ok()
    }

    /// Pinta el cristal. No es un elemento: ni se guarda ni sale en la foto
    /// de al salir (esa se pinta sin `encima`).
    pub fn pintar(
        &self,
        p: &pixpin_render::Pintor<'_>,
        fotograma: &windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    ) {
        let Some((fuente, destino)) = self.donde() else {
            return;
        };
        let m = self
            .monitor_bajo_el_cursor()
            .map_or(100, |m| m.escala_por_cien);
        let d = pixpin_render::RectF {
            x: destino.x as f32,
            y: destino.y as f32,
            ancho: destino.ancho as f32,
            alto: destino.alto as f32,
        };
        p.desplazar(0.0, 0.0);
        p.bitmap(
            fotograma,
            d,
            Some(pixpin_render::RectF {
                x: fuente.x as f32,
                y: fuente.y as f32,
                ancho: fuente.ancho as f32,
                alto: fuente.alto as f32,
            }),
            true,
        );
        p.trazar(d, 2.0 * m as f32 / 100.0, pixpin_render::Color::ACENTO);
    }
}

impl Drop for LupaViva {
    fn drop(&mut self) {
        self.cerrar();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_shell::overlay::EventoOverlay;

    fn monitor(id: u32, x: i32, ancho: u32, alto: u32, escala: u32) -> pixpin_geom::Monitor {
        let area = Rect {
            x,
            y: 0,
            ancho,
            alto,
        };
        pixpin_geom::Monitor {
            id,
            area,
            area_trabajo: area,
            escala_por_cien: escala,
            principal: x == 0,
        }
    }

    fn se_solapan(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.ancho as i32
            && b.x < a.x + a.ancho as i32
            && a.y < b.y + b.alto as i32
            && b.y < a.y + a.alto as i32
    }

    /// **Banco: lo que cuesta ENTRAR en el anotador** (Alt + doble clic
    /// central), paso a paso y SIN ensenar nada: el dispositivo de GPU, el
    /// motor, la ventana del escritorio virtual (creada oculta), su
    /// superficie con capas y la pastilla con su primer pintado. Es lo que
    /// hace `abrir_en_modo` antes del primer fotograma; en viva no hay
    /// captura. Cinco vueltas: la primera paga lo que Windows carga una vez
    /// por proceso (DirectWrite, las DLL del controlador).
    /// `cargo test -p pixpin --release --bin pixpinmax banco_de_entrar_en_el_anotador -- --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU y escritorio (crea ventanas ocultas); ejecutar con --ignored --nocapture"]
    fn banco_de_entrar_en_el_anotador() {
        let d = pixpin_capture::enumerar_monitores().expect("monitores");
        let escritorio = d.escritorio_virtual();
        let principal = *d.principal().expect("principal");
        let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let rotulos = (
            t.t("anotador-rotulo-dibujando"),
            t.t("anotador-rotulo-atravesando"),
        );
        println!(
            "escritorio virtual {}x{} ({} monitores)",
            escritorio.ancho,
            escritorio.alto,
            d.monitores().len()
        );
        for vuelta in 0..5 {
            let t0 = std::time::Instant::now();
            let disp = pixpin_capture::Dispositivo::nuevo().expect("dispositivo");
            let t_disp = t0.elapsed();
            let motor = pixpin_render::MotorRender::nuevo(disp.d3d()).expect("motor");
            let t_motor = t0.elapsed();
            let ventana =
                pixpin_shell::overlay::VentanaOverlay::nueva(escritorio).expect("ventana");
            let t_ventana = t0.elapsed();
            let sup = pixpin_render::Superficie::nueva_con_capas(
                &motor,
                disp.d3d(),
                ventana.handle(),
                escritorio.ancho,
                escritorio.alto,
                1,
            )
            .expect("superficie");
            let t_sup = t0.elapsed();
            let mut ps = super::super::pastilla_pantalla::Pastilla::nueva(
                &motor,
                disp.d3d(),
                &principal,
                rotulos.clone(),
                ventana.handle(),
            )
            .expect("pastilla");
            ps.al_dia(&motor);
            let t_pastilla = t0.elapsed();
            println!(
                "vuelta {vuelta}: dispositivo {:.1} ms, motor {:.1}, ventana {:.1}, superficie {:.1}, pastilla {:.1} -> total {:.1} ms",
                t_disp.as_secs_f64() * 1e3,
                (t_motor - t_disp).as_secs_f64() * 1e3,
                (t_ventana - t_motor).as_secs_f64() * 1e3,
                (t_sup - t_ventana).as_secs_f64() * 1e3,
                (t_pastilla - t_sup).as_secs_f64() * 1e3,
                t_pastilla.as_secs_f64() * 1e3,
            );
            drop((ps, sup, ventana, motor, disp));
        }
    }

    #[test]
    fn la_rueda_sube_y_baja_el_aumento_sin_pasarse_de_los_topes() {
        let mas = siguiente_aumento(LUPA_POR_DEFECTO, 120);
        assert!(mas > LUPA_POR_DEFECTO);
        assert!(siguiente_aumento(mas, -120) < mas);
        let mut a = LUPA_POR_DEFECTO;
        for _ in 0..40 {
            a = siguiente_aumento(a, 120);
        }
        assert_eq!(a, LUPA_MAXIMA);
        for _ in 0..40 {
            a = siguiente_aumento(a, -120);
        }
        assert_eq!(a, LUPA_MINIMA);
    }

    #[test]
    fn la_lupa_cae_en_el_monitor_del_cursor_y_no_pisa_lo_que_amplia() {
        // Dos monitores: un portatil a 150 % a la izquierda (x negativa) y el
        // principal a 100 %. El escritorio empieza en el del portatil.
        let izq = monitor(1, -1920, 1920, 1080, 150);
        let der = monitor(2, 0, 2560, 1440, 100);
        let escritorio = Rect {
            x: -1920,
            y: 0,
            ancho: 4480,
            alto: 1440,
        };
        for (m, cursor) in [
            (&izq, Punto { x: -1000, y: 500 }),
            (&der, Punto { x: 2500, y: 1400 }),
            (&der, Punto { x: 10, y: 10 }),
        ] {
            let (fuente, destino) = colocar_lupa(3.0, m, cursor, escritorio);
            // La fuente, en pixeles del monitor, dentro de el.
            assert!(fuente.x >= 0 && fuente.y >= 0);
            assert!(fuente.x + fuente.ancho as i32 <= m.area.ancho as i32);
            // El cristal, en pixeles de ventana, dentro del mismo monitor.
            let en_ventana = Rect {
                x: m.area.x - escritorio.x,
                y: m.area.y - escritorio.y,
                ..m.area
            };
            assert!(destino.x >= en_ventana.x);
            assert!(destino.x + destino.ancho as i32 <= en_ventana.x + en_ventana.ancho as i32);
            assert!(destino.y + destino.alto as i32 <= en_ventana.y + en_ventana.alto as i32);
            // Y sin pisar su fuente, llevada a la ventana.
            let fuente_en_ventana = Rect {
                x: fuente.x + en_ventana.x,
                y: fuente.y + en_ventana.y,
                ..fuente
            };
            assert!(!se_solapan(destino, fuente_en_ventana), "{cursor:?}");
        }
        // El cristal crece con la escala del monitor, como la barra.
        let (_, chico) = colocar_lupa(2.0, &der, Punto { x: 100, y: 100 }, escritorio);
        let (_, grande) = colocar_lupa(2.0, &izq, Punto { x: -1800, y: 100 }, escritorio);
        assert!(grande.ancho > chico.ancho);
    }

    #[test]
    fn sin_la_lupa_puesta_la_rueda_no_es_suya_ni_moverse_repinta() {
        let mut l = LupaViva::nueva(
            vec![monitor(1, 0, 1920, 1080, 100)],
            Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080,
            },
        );
        assert!(!l.evento(&EventoOverlay::Rueda(120), false));
        assert!(!l.evento(&EventoOverlay::RatonMovido(Punto { x: 5, y: 5 }), false));
        assert_eq!(
            l.tomar_zona(),
            None,
            "sin lupa, mover el raton no cuesta nada"
        );
        // Puesta, la rueda es suya y pide repintar.
        assert!(l.evento(&EventoOverlay::Rueda(120), true));
        assert!(l.tomar_zona().is_some());
        assert_eq!(l.tomar_zona(), None, "lo pedido se entrega una vez");
    }

    fn tecla_ev(vk: u32, ctrl: bool) -> EventoOverlay {
        EventoOverlay::Tecla {
            vk,
            shift: false,
            ctrl,
            alt: false,
        }
    }

    #[test]
    fn escape_sale_solo_si_no_hay_texto_abierto_ni_nada_elegido() {
        let esc = tecla_ev(VK_ESCAPE, false);
        assert_eq!(
            tecla(&esc, Modo::Viva, true, true),
            Some(TeclaPantalla::Salir)
        );
        assert_eq!(
            tecla(&esc, Modo::Congelada, true, true),
            Some(TeclaPantalla::Salir)
        );
        // Casos negativos: con un texto abierto Escape lo cierra, y con algo
        // elegido lo suelta; no se sale con el dibujo a medias.
        assert_eq!(tecla(&esc, Modo::Viva, false, true), None);
        assert_eq!(tecla(&esc, Modo::Viva, true, false), None);
    }

    #[test]
    fn espacio_y_ctrl_dejan_pasar_los_clics_solo_con_la_pantalla_viva() {
        let esp = tecla_ev(VK_ESPACIO, false);
        assert_eq!(
            tecla(&esp, Modo::Viva, true, true),
            Some(TeclaPantalla::AlternarPasante)
        );
        assert_eq!(
            tecla(&tecla_ev(VK_CONTROL, true), Modo::Viva, true, true),
            Some(TeclaPantalla::Ctrl(true))
        );
        assert_eq!(
            tecla(
                &EventoOverlay::TeclaSoltada(VK_CONTROL),
                Modo::Viva,
                true,
                true
            ),
            Some(TeclaPantalla::Ctrl(false))
        );
        // Congelada debajo solo hay una foto: no hay a quien dejar pasar.
        assert_eq!(tecla(&esp, Modo::Congelada, true, true), None);
        assert_eq!(
            tecla(&tecla_ev(VK_CONTROL, true), Modo::Congelada, true, true),
            None
        );
        // Escribiendo, el espacio es un espacio.
        assert_eq!(tecla(&esp, Modo::Viva, false, true), None);
    }

    #[test]
    fn la_camara_quieta_deja_el_trazo_a_1_1_con_cualquier_escala() {
        for escala in [100, 125, 150, 200] {
            let c = crate::navegacion::vista_efectiva(&camara_quieta(escala), escala);
            assert!((c.zoom - 1.0).abs() < 1e-6, "{escala}: {}", c.zoom);
            let p = c.a_mundo(pixpin_motor2d::vector::Punto2::nuevo(1234.0, 567.0));
            assert!((p.x - 1234.0).abs() < 1e-3 && (p.y - 567.0).abs() < 1e-3);
        }
    }

    #[test]
    fn el_papel_vivo_es_transparente_y_no_se_toma_por_papel_de_noche() {
        assert_eq!(papel(Modo::Viva).a, 0.0);
        // Aunque sea «negro» transparente, la tinta no se adapta: el anotador
        // no fija papel (la pantalla de debajo es de cualquier color).
        assert_eq!(papel(Modo::Congelada).a, 1.0);
    }

    #[test]
    fn las_fotos_de_dos_monitores_con_distinta_escala_van_cada_una_a_su_sitio() {
        // Un portatil de 4x2 a la izquierda (x negativa) y el principal de
        // 6x3 a la derecha, alineados arriba: el escritorio mide 10x3 y la
        // esquina de abajo a la izquierda no la cubre nadie.
        let escritorio = Rect {
            x: -4,
            y: 0,
            ancho: 10,
            alto: 3,
        };
        let izq = pixpin_codec::ImagenRgba {
            ancho: 4,
            alto: 2,
            pixeles: [10u8, 20, 30, 255].repeat(8),
        };
        let der = pixpin_codec::ImagenRgba {
            ancho: 6,
            alto: 3,
            pixeles: [200u8, 100, 50, 255].repeat(18),
        };
        let u = unir_fotos(
            escritorio,
            &[
                (
                    Rect {
                        x: -4,
                        y: 0,
                        ancho: 4,
                        alto: 2,
                    },
                    izq,
                ),
                (
                    Rect {
                        x: 0,
                        y: 0,
                        ancho: 6,
                        alto: 3,
                    },
                    der,
                ),
            ],
        );
        let px = |x: usize, y: usize| &u.pixeles[(y * 10 + x) * 4..(y * 10 + x) * 4 + 4];
        assert_eq!(px(0, 0), [10, 20, 30, 255]);
        assert_eq!(px(3, 1), [10, 20, 30, 255]);
        assert_eq!(px(4, 0), [200, 100, 50, 255]);
        assert_eq!(px(9, 2), [200, 100, 50, 255]);
        // Caso negativo: lo que no cubre ningun monitor, negro opaco.
        assert_eq!(px(0, 2), [0, 0, 0, 255]);
    }
}
