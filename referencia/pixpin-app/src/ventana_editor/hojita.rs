//! **La hojita** (F6), en el editor: una hoja pequena para garabatear un
//! recado y **pegarlo como pin** o **insertarlo en el lienzo**.
//!
//! En el movil sale con tres dedos hacia arriba (`c6df577`) y se garabatea
//! debajo de la barra (v0.67). En el PC no hay tres dedos: sale con el boton
//! del lapiz del riel de la derecha o con **Ctrl+Mayus+N** («N» de nota). Y
//! donde el movil la pega en el lienzo como una imagen, aqui se pega como
//! **pin**, que flota encima de todas las ventanas: es lo que es un recado
//! (el porque esta en `pixpin_motor2d::hojita`). Insertarla en el lienzo como
//! hoja vectorial agrupada tambien esta, como en el movil.
//!
//! Como el cajetin de calibrar (`pedir_medida`), abre su propio bucle de
//! eventos sobre la misma ventana y no vuelve hasta cerrarse: mientras esta
//! abierta, el raton y el teclado son suyos.
//!
//! Se dibuja con el lapiz y el estilo que tenga puesto el editor (el color,
//! el grosor, la pluma), como en el movil, que «coge la herramienta y el
//! estilo del lienzo»; con una herramienta que no es de trazar a mano, con el
//! lapiz. Lo garabateado **espera** mientras viva la aplicacion: cerrar la
//! hojita sin pegarla no lo tira, como en el movil (que la guarda aparte).
//!
//! # Por que se pinta sola en la capa de la interfaz
//!
//! Con capas (A3), cada movimiento del raton sobre el papel repinta SOLO la
//! capa de la interfaz: la escena no ha cambiado, y repintarla por cada
//! punto del recado costaria lo que cueste el dibujo entero (15 ms con 2.000
//! elementos). El primer fotograma si va entero, para que la escena este.

use super::marcas::Marcador;
use super::{FueraDeLaEscena, dibujar_orden, pintar, por_cada_orden, tecla_de};
use crate::fondo_lienzo::FondoLienzo;
use crate::imagenes_lienzo::ImagenesLienzo;
use pixpin_geom::Punto;
use pixpin_motor2d::cache::Cache;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::hojita::{PAPELES, papel};
use pixpin_motor2d::indice::Rejilla;
use pixpin_motor2d::vector::Punto2;
use pixpin_motor2d::{ColorRgba, Elemento};
use pixpin_render::icono::material;
use pixpin_render::{CapaEstatica, Color, MotorRender, Pintor, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin, VentanaOverlay};
use pixpin_ui::hojita::{DestinoHojita, Hojita as Colocada};
use pixpin_ui::{BotonCaja, CajaHerramientas};
use std::cell::RefCell;

use crate::caja_dibujo::{hex, sombra_isla};

thread_local! {
    /// Lo garabateado y el papel, entre una apertura y la siguiente.
    static GUARDADA: RefCell<Option<(Escena, usize)>> = const { RefCell::new(None) };
}

/// Lo que hay que meter en el lienzo al pulsar «Insertar».
pub(super) struct Insertar {
    pub anotado: Vec<Elemento>,
    /// El papel en coordenadas de la hojita (pixeles logicos).
    pub vista: (f32, f32, f32, f32),
    pub papel: ColorRgba,
}

/// Todo lo del editor que hace falta para seguir pintandolo debajo.
pub(super) struct Editor<'a> {
    pub ventana: &'a VentanaOverlay,
    pub dispositivo: &'a pixpin_capture::Dispositivo,
    pub motor: &'a mut MotorRender,
    pub superficie: &'a Superficie,
    pub escena: &'a Escena,
    pub camara: &'a Camara,
    pub gesto: &'a Gesto,
    pub cache: &'a mut Cache,
    pub cache_tinta: &'a mut pixpin_render::CacheTinta,
    pub rejilla: &'a Rejilla,
    pub capa: &'a CapaEstatica,
    pub fondo: &'a mut Option<FondoLienzo>,
    pub imagenes: &'a mut ImagenesLienzo,
    pub caja: &'a CajaHerramientas,
    pub marcador: &'a Marcador,
    pub area: pixpin_geom::Rect,
    pub escala_por_cien: u32,
    pub ancho_px: f32,
    pub alto_px: f32,
    pub bajo_la_barra: i32,
}

/// La herramienta con la que se escribe en la hojita: la del editor si es de
/// trazar a mano, y si no el lapiz. Una flecha o un marco no tienen sentido
/// en un recado, y el movil hace lo mismo con las que «piden un sitio que
/// aqui no hay».
pub(super) fn herramienta_de_la_hojita(del_editor: Herramienta) -> Herramienta {
    match del_editor {
        Herramienta::Lapiz | Herramienta::Grafito | Herramienta::Resaltador => del_editor,
        _ => Herramienta::Lapiz,
    }
}

/// Abre la hojita y no vuelve hasta cerrarla. Devuelve lo que haya que
/// insertar en el lienzo si se pulso «Insertar»; pegar como pin lo resuelve
/// ella sola.
pub(super) fn abrir(ed: Editor<'_>) -> Option<Insertar> {
    let Editor {
        ventana,
        dispositivo,
        motor,
        superficie,
        escena,
        camara,
        gesto: gesto_editor,
        cache,
        cache_tinta,
        rejilla,
        capa,
        fondo,
        imagenes,
        caja,
        marcador,
        area,
        escala_por_cien,
        ancho_px,
        alto_px,
        bajo_la_barra,
    } = ed;
    let e = crate::navegacion::escala_de(escala_por_cien);
    let hoja = Colocada::colocar(
        area.ancho,
        area.alto,
        escala_por_cien,
        PAPELES.len(),
        bajo_la_barra,
    );
    let (mut anotado, mut indice_papel) = GUARDADA
        .with(|g| g.borrow_mut().take())
        .unwrap_or_else(|| (Escena::nueva(), 0));
    let mut lapiz = Gesto::nuevo();
    lapiz.tomar_herramienta(herramienta_de_la_hojita(gesto_editor.herramienta));
    lapiz.estilo = gesto_editor.estilo;
    lapiz.grosor_tinta = gesto_editor.grosor_tinta;
    lapiz.variabilidad = gesto_editor.variabilidad;
    // La cache de la hojita es suya: sus ids se repiten con los del lienzo.
    let mut cache_hoja = Cache::nueva();
    // Un recado a mano no lleva fotos, pero `dibujar_orden` pide almacen; el
    // del lienzo esta prestado a `pintar` mientras se pinta la hojita.
    let sin_imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
    let mut dibujando = false;
    let mut encima = DestinoHojita::Fuera;
    // El primer fotograma va entero (la escena, la barra, el riel y la
    // hojita); los siguientes, solo la interfaz si hay capas.
    let mut entero = true;
    let al_papel = |p: Punto| {
        Punto2::nuevo(
            (p.x - area.x - hoja.papel.x) as f32 / e,
            (p.y - area.y - hoja.papel.y) as f32 / e,
        )
    };
    let local = |p: Punto| Punto {
        x: p.x - area.x,
        y: p.y - area.y,
    };
    let vista = (
        0.0,
        0.0,
        hoja.papel.ancho as f32 / e,
        hoja.papel.alto as f32 / e,
    );
    ventana.poner_cursor(FormaCursorWin::Cruz);
    ventana.invalidar();

    let salida = 'bucle: loop {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            let mut cambio = false;
            match ev {
                EventoOverlay::BotonPulsado(p) => match hoja.destino(local(p)) {
                    DestinoHojita::Papel => {
                        lapiz.evento(
                            EventoGesto::Pulsar {
                                p: al_papel(p),
                                shift: false,
                                alt: false,
                                presion: None,
                            },
                            &mut anotado,
                            1.0 / e,
                        );
                        dibujando = true;
                        cambio = true;
                    }
                    DestinoHojita::ColorPapel(i) => {
                        indice_papel = i;
                        cambio = true;
                    }
                    DestinoHojita::BorrarTodo => {
                        // Borrar se puede deshacer con Ctrl+Z mientras siga
                        // abierta: un toque sin querer no puede costar el
                        // recado entero.
                        anotado.abrir_paso();
                        let ids: Vec<u64> = anotado.visibles().map(|x| x.id).collect();
                        for id in ids {
                            anotado.borrar_apuntando(id);
                        }
                        anotado.cerrar_paso();
                        cambio = true;
                    }
                    DestinoHojita::Insertar => {
                        let vivos: Vec<Elemento> = anotado.visibles().cloned().collect();
                        if vivos.is_empty() {
                            continue;
                        }
                        // Lo insertado ya no espera en la hojita: vive en el
                        // lienzo.
                        break 'bucle Some(Insertar {
                            anotado: vivos,
                            vista,
                            papel: papel(indice_papel),
                        });
                    }
                    DestinoHojita::PegarComoPin => {
                        if anotado.cuantos_visibles() == 0 {
                            continue;
                        }
                        let (ancho, alto) = (hoja.papel.ancho, hoja.papel.alto);
                        match pegar_como_pin(
                            motor,
                            dispositivo,
                            &anotado,
                            indice_papel,
                            ancho,
                            alto,
                            e,
                        ) {
                            Ok(ruta) => {
                                tracing::info!(ruta = %ruta.display(), "hojita pegada como pin");
                                anotado = Escena::nueva();
                                break 'bucle None;
                            }
                            // Si no se pudo, la hojita se queda abierta con
                            // lo suyo: cerrarla perderia el recado sin decir
                            // por que.
                            Err(err) => tracing::warn!(?err, "no se pudo pegar la hojita"),
                        }
                    }
                    DestinoHojita::Cerrar | DestinoHojita::Fuera => break 'bucle None,
                    DestinoHojita::Hueco => {}
                },
                EventoOverlay::RatonMovido(p) => {
                    if dibujando {
                        lapiz.evento(
                            EventoGesto::Mover {
                                p: al_papel(p),
                                shift: false,
                                alt: false,
                                presion: None,
                            },
                            &mut anotado,
                            1.0 / e,
                        );
                        cambio = true;
                    } else {
                        let ahora = hoja.destino(local(p));
                        cambio = ahora != encima;
                        encima = ahora;
                        ventana.poner_cursor(if ahora == DestinoHojita::Papel {
                            FormaCursorWin::Cruz
                        } else {
                            FormaCursorWin::Flecha
                        });
                    }
                }
                EventoOverlay::BotonSoltado(p) if dibujando => {
                    lapiz.evento(
                        EventoGesto::Soltar { p: al_papel(p) },
                        &mut anotado,
                        1.0 / e,
                    );
                    dibujando = false;
                    cambio = true;
                }
                EventoOverlay::Tecla { vk, ctrl, .. } => {
                    const VK_ESCAPE: u32 = 0x1B;
                    match (vk, ctrl) {
                        (VK_ESCAPE, _) => break 'bucle None,
                        (v, true) if v == b'Z' as u32 => cambio = anotado.deshacer(),
                        (v, true) if v == b'Y' as u32 => cambio = anotado.rehacer(),
                        _ => {}
                    }
                }
                EventoOverlay::Cerrar => break 'bucle None,
                EventoOverlay::Pintar => {
                    let dibujar_hoja =
                        |p: &Pintor<'_>,
                         d: (f32, f32),
                         cache_hoja: &mut Cache,
                         imagenes: &ImagenesLienzo| {
                            pintar_hojita(
                                p,
                                d,
                                &hoja,
                                &anotado,
                                indice_papel,
                                encima,
                                cache_hoja,
                                imagenes,
                                e,
                                vista,
                            )
                        };
                    if entero || !superficie.tiene_capas() {
                        let panel = crate::panel_dibujo::panel_para(
                            gesto_editor,
                            escena,
                            area,
                            escala_por_cien,
                        );
                        pintar(
                            motor,
                            superficie,
                            escena,
                            camara,
                            gesto_editor,
                            cache,
                            cache_tinta,
                            rejilla,
                            capa,
                            fondo,
                            imagenes,
                            caja,
                            None,
                            (-(area.x as f32), -(area.y as f32)),
                            true,
                            escala_por_cien,
                            ancho_px,
                            alto_px,
                            None,
                            None,
                            panel.as_ref(),
                            true,
                            FueraDeLaEscena::default(),
                            Some(marcador),
                            |p, d| {
                                marcador.pintar_interfaz(
                                    p,
                                    area.ancho,
                                    area.alto,
                                    escala_por_cien,
                                    bajo_la_barra,
                                );
                                p.desplazar(d.0, d.1);
                                dibujar_hoja(p, d, &mut cache_hoja, &sin_imagenes);
                            },
                        );
                        entero = false;
                    } else {
                        let panel = crate::panel_dibujo::panel_para(
                            gesto_editor,
                            escena,
                            area,
                            escala_por_cien,
                        );
                        let _ = superficie.pintar_interfaz(motor, |p, d| {
                            crate::caja_dibujo::pintar_barra(
                                p,
                                caja,
                                gesto_editor.herramienta,
                                escala_por_cien,
                                None,
                                |b| match b {
                                    BotonCaja::Elegir(h) => tecla_de(h),
                                    _ => None,
                                },
                            );
                            if let Some(panel) = &panel {
                                crate::panel_dibujo::pintar(p, panel, escala_por_cien);
                            }
                            marcador.pintar_interfaz(
                                p,
                                area.ancho,
                                area.alto,
                                escala_por_cien,
                                bajo_la_barra,
                            );
                            p.desplazar(d.0, d.1);
                            dibujar_hoja(p, d, &mut cache_hoja, &sin_imagenes);
                        });
                    }
                }
                _ => {}
            }
            if cambio {
                ventana.invalidar();
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(4));
    };
    // Lo que no se inserto ni se pego espera a la proxima vez.
    if salida.is_none() {
        GUARDADA.with(|g| *g.borrow_mut() = Some((anotado, indice_papel)));
    }
    ventana.poner_cursor(FormaCursorWin::Flecha);
    salida
}

/// **Mete la hojita en el lienzo**, centrada en lo que se ve, y la deja
/// elegida para poder llevarla a su sitio de un tiron.
pub(super) fn insertar_en_el_lienzo(
    escena: &mut Escena,
    gesto: &mut Gesto,
    efectiva: &Camara,
    ancho_px: f32,
    alto_px: f32,
    ins: Insertar,
) {
    let (x0, y0, x1, y1) = efectiva.ventana(ancho_px, alto_px);
    let (w, h) = (ins.vista.2 - ins.vista.0, ins.vista.3 - ins.vista.1);
    let en = Punto2::nuevo((x0 + x1) / 2.0 - w / 2.0, (y0 + y1) / 2.0 - h / 2.0);
    let ids = pixpin_motor2d::hojita::insertar(escena, &ins.anotado, ins.vista, ins.papel, en);
    if !ids.is_empty() {
        gesto.seleccion.poner_todos(ids);
    }
}

fn a_color(c: ColorRgba) -> Color {
    Color {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn rf(r: pixpin_geom::Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

/// Lo anotado, con su vista ya puesta. Lo comparten la pantalla y el pin.
fn pintar_anotado(
    p: &Pintor<'_>,
    anotado: &Escena,
    cache: &mut Cache,
    imagenes: &ImagenesLienzo,
    zoom: f32,
    vista: (f32, f32, f32, f32),
) {
    // Su grafito, en su espacio del horno: los ids de la hojita se repiten
    // con los del lienzo, que se pinta en el mismo fotograma y el mismo
    // hilo; en el mismo espacio se echarian el uno al otro y se recocerian
    // enteros en cada fotograma.
    pixpin_motor2d::tinta::grafito::en_espacio(ESPACIO_DEL_HORNO, || {
        for el in anotado.visibles() {
            let grano = pixpin_motor2d::pintado::grano_de(el);
            por_cada_orden(cache, el, zoom, None, |orden| {
                dibujar_orden(p, orden, vista, None, imagenes, zoom, grano);
            });
        }
    });
}

/// El espacio del horno del grafito de la hojita (el lienzo va en el 0 y
/// los lectores a partir de `1 << 40`).
const ESPACIO_DEL_HORNO: u64 = 1 << 39;

/// La tarjeta de la hojita: el papel con lo garabateado y la fila de mandos.
/// El pintor llega con el desplazamiento `d` de la capa ya puesto.
#[allow(clippy::too_many_arguments)]
fn pintar_hojita(
    p: &Pintor<'_>,
    d: (f32, f32),
    hoja: &Colocada,
    anotado: &Escena,
    indice_papel: usize,
    encima: DestinoHojita,
    cache: &mut Cache,
    imagenes: &ImagenesLienzo,
    e: f32,
    vista: (f32, f32, f32, f32),
) {
    let tarjeta = rf(hoja.tarjeta);
    sombra_isla(p, tarjeta, 10.0 * e, e);
    p.rellenar_redondeado(tarjeta, 10.0 * e, hex(0xffffff));
    let papel_r = rf(hoja.papel);
    p.rellenar_redondeado(papel_r, 4.0 * e, a_color(papel(indice_papel)));
    // Lo garabateado, recortado al papel: una raya que se sale no puede
    // pintarse encima de los mandos.
    p.empujar_recorte(papel_r);
    p.poner_vista(d, e, (papel_r.x, papel_r.y));
    pintar_anotado(p, anotado, cache, imagenes, e, vista);
    p.desplazar(d.0, d.1);
    p.soltar_recorte();

    for (i, m) in hoja.muestras.iter().enumerate() {
        let r = rf(*m);
        let c = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
        if i == indice_papel {
            p.anillo(c, r.ancho / 2.0 + 2.0 * e, 2.0 * e, hex(0x6965db));
        }
        p.circulo(c, r.ancho / 2.0, a_color(papel(i)));
        p.anillo(c, r.ancho / 2.0, 1.0 * e, hex(0xb8b8b8));
    }
    let t = super::marcas::textos();
    for (r, icono, texto, que) in [
        (
            hoja.borrar,
            &material::DELETE,
            t.t("hojita-borrar"),
            DestinoHojita::BorrarTodo,
        ),
        (
            hoja.insertar,
            &material::LIBRARY_ADD,
            t.t("hojita-insertar"),
            DestinoHojita::Insertar,
        ),
        (
            hoja.pegar,
            &material::PUSH_PIN,
            t.t("hojita-pegar"),
            DestinoHojita::PegarComoPin,
        ),
    ] {
        let r = rf(r);
        if encima == que {
            p.rellenar_redondeado(r, 6.0 * e, hex(0xf1f0ff));
        }
        let lado = 18.0 * e;
        p.icono(
            icono,
            RectF {
                x: r.x + 8.0 * e,
                y: r.y + (r.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            hex(0x1b1b1f),
        );
        let tam = 12.0 * e;
        let (_, h) = p.medir_texto(&texto, tam);
        p.texto(
            &texto,
            r.x + 8.0 * e + lado + 6.0 * e,
            r.y + (r.alto - h) / 2.0,
            tam,
            hex(0x1b1b1f),
        );
    }
    let x = rf(hoja.cerrar);
    if encima == DestinoHojita::Cerrar {
        p.rellenar_redondeado(x, 6.0 * e, hex(0xf1f0ff));
    }
    let lado = 18.0 * e;
    p.icono(
        &material::CLOSE,
        RectF {
            x: x.x + (x.ancho - lado) / 2.0,
            y: x.y + (x.alto - lado) / 2.0,
            ancho: lado,
            alto: lado,
        },
        hex(0x1b1b1f),
    );
}

/// Pinta el recado solo (el papel y lo garabateado, sin mandos) en una
/// imagen del tamano en que se veia y la manda a la ventana principal, que
/// es quien tiene los pines: por el mismo camino que «Sacar a la pantalla»
/// del chat. El PNG va a la carpeta temporal; el pin se queda con su copia.
fn pegar_como_pin(
    motor: &MotorRender,
    dispositivo: &pixpin_capture::Dispositivo,
    anotado: &Escena,
    indice_papel: usize,
    ancho: u32,
    alto: u32,
    e: f32,
) -> anyhow::Result<std::path::PathBuf> {
    let imagen = pintar_recado(motor, dispositivo, anotado, indice_papel, ancho, alto, e)?;
    let png = pixpin_codec::imagen::codificar_png(&imagen)?;
    let carpeta = std::env::temp_dir().join("pixpin-hojitas");
    std::fs::create_dir_all(&carpeta)?;
    let sello = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let ruta = carpeta.join(format!("hojita-{sello}.png"));
    std::fs::write(&ruta, png)?;
    if !pixpin_shell::mensajero::enviar_ficheros(std::slice::from_ref(&ruta)) {
        anyhow::bail!("no contesta la ventana principal");
    }
    Ok(ruta)
}

/// El recado en pixeles: el papel entero con lo garabateado encima.
pub(super) fn pintar_recado(
    motor: &MotorRender,
    dispositivo: &pixpin_capture::Dispositivo,
    anotado: &Escena,
    indice_papel: usize,
    ancho: u32,
    alto: u32,
    e: f32,
) -> anyhow::Result<pixpin_codec::ImagenRgba> {
    use pixpin_render::fuera_de_pantalla::FueraDePantalla;
    let fuera = FueraDePantalla::nuevo(motor, dispositivo.d3d(), ancho.max(1), alto.max(1))?;
    let mut cache = Cache::nueva();
    let imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
    let vista = (0.0, 0.0, ancho as f32 / e, alto as f32 / e);
    motor.dibujar(&fuera.destino, |p| {
        p.limpiar(a_color(papel(indice_papel)));
        p.poner_vista((0.0, 0.0), e, (0.0, 0.0));
        pintar_anotado(p, anotado, &mut cache, &imagenes, e, vista);
    })?;
    fuera.esperar_gpu()?;
    let (ancho, alto, pixeles) = fuera.leer_rgba()?;
    Ok(pixpin_codec::ImagenRgba {
        ancho,
        alto,
        pixeles,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn en_la_hojita_se_escribe_con_el_lapiz_del_editor_o_con_el_lapiz() {
        assert_eq!(
            herramienta_de_la_hojita(Herramienta::Resaltador),
            Herramienta::Resaltador
        );
        assert_eq!(
            herramienta_de_la_hojita(Herramienta::Grafito),
            Herramienta::Grafito
        );
        // Lo que no es trazar a mano no entra: una flecha en un recado.
        assert_eq!(
            herramienta_de_la_hojita(Herramienta::Flecha),
            Herramienta::Lapiz
        );
        assert_eq!(
            herramienta_de_la_hojita(Herramienta::Mano),
            Herramienta::Lapiz
        );
        assert_eq!(
            herramienta_de_la_hojita(Herramienta::Marco),
            Herramienta::Lapiz
        );
    }

    #[test]
    fn insertar_la_centra_en_lo_que_se_ve_y_la_deja_elegida() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        let camara = Camara {
            x: 1000.0,
            y: 2000.0,
            zoom: 1.0,
        };
        let trazo = Elemento {
            figura: pixpin_motor2d::Figura::Lapiz {
                puntos: vec![Punto2::nuevo(10.0, 10.0), Punto2::nuevo(50.0, 40.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            ..Default::default()
        };
        insertar_en_el_lienzo(
            &mut escena,
            &mut gesto,
            &camara,
            800.0,
            600.0,
            Insertar {
                anotado: vec![trazo],
                vista: (0.0, 0.0, 480.0, 320.0),
                papel: papel(0),
            },
        );
        assert_eq!(gesto.seleccion.cuantos(), 2);
        let hoja = escena.visibles().next().unwrap();
        // La ventana ve de (1000, 2000) a (1800, 2600): centro (1400, 2300).
        assert_eq!((hoja.x, hoja.y), (1400.0 - 240.0, 2300.0 - 160.0));
    }

    /// **La muestra de la hojita en PNG**: la tarjeta como se ve en el
    /// editor y el recado como sale en el pin. Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_hojita --
    /// --ignored --nocapture`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_hojita() {
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let (ancho, alto) = (900u32, 520u32);
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
        let carpeta = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        // Un recado escrito con el lapiz de verdad, por el gesto.
        let mut anotado = Escena::nueva();
        let mut lapiz = Gesto::nuevo();
        lapiz.tomar_herramienta(Herramienta::Lapiz);
        for trazo in [
            &[
                (40.0, 60.0),
                (80.0, 40.0),
                (120.0, 70.0),
                (160.0, 45.0),
                (200.0, 65.0),
            ][..],
            &[(40.0, 120.0), (300.0, 110.0)][..],
            &[(60.0, 180.0), (90.0, 220.0), (150.0, 160.0)][..],
        ] {
            let (x, y) = trazo[0];
            let _ = lapiz.evento(
                EventoGesto::Pulsar {
                    p: Punto2::nuevo(x, y),
                    shift: false,
                    alt: false,
                    presion: None,
                },
                &mut anotado,
                1.0,
            );
            for (x, y) in &trazo[1..] {
                let _ = lapiz.evento(
                    EventoGesto::Mover {
                        p: Punto2::nuevo(*x, *y),
                        shift: false,
                        alt: false,
                        presion: None,
                    },
                    &mut anotado,
                    1.0,
                );
            }
            let (x, y) = *trazo.last().unwrap();
            let _ = lapiz.evento(
                EventoGesto::Soltar {
                    p: Punto2::nuevo(x, y),
                },
                &mut anotado,
                1.0,
            );
        }
        let hoja = Colocada::colocar(ancho, alto, 100, PAPELES.len(), 40);
        let vista = (0.0, 0.0, hoja.papel.ancho as f32, hoja.papel.alto as f32);
        let imagenes = ImagenesLienzo::nuevo(motor.lado_maximo_bitmap());
        let mut cache = Cache::nueva();
        motor
            .dibujar(&fuera.destino, |p| {
                p.limpiar(hex(0xf4f4f6));
                pintar_hojita(
                    p,
                    (0.0, 0.0),
                    &hoja,
                    &anotado,
                    0,
                    DestinoHojita::PegarComoPin,
                    &mut cache,
                    &imagenes,
                    1.0,
                    vista,
                );
            })
            .expect("pintar");
        fuera.esperar_gpu().expect("esperar");
        let (_, _, pixeles) = fuera.leer_rgba().expect("leer");
        let tarjeta = pixpin_codec::ImagenRgba {
            ancho,
            alto,
            pixeles,
        };
        let recado = pintar_recado(
            &motor,
            &d,
            &anotado,
            2,
            hoja.papel.ancho,
            hoja.papel.alto,
            1.0,
        )
        .expect("recado");
        for (nombre, img) in [("hojita-tarjeta", tarjeta), ("hojita-pin", recado)] {
            let ruta = carpeta.join(format!("{nombre}.png"));
            std::fs::write(
                &ruta,
                pixpin_codec::imagen::codificar_png(&img).expect("png"),
            )
            .expect("guardar");
            println!("{nombre}: {}", ruta.display());
        }
    }

    #[test]
    fn insertar_nada_no_elige_nada() {
        let mut escena = Escena::nueva();
        let mut gesto = Gesto::nuevo();
        insertar_en_el_lienzo(
            &mut escena,
            &mut gesto,
            &Camara::nueva(),
            800.0,
            600.0,
            Insertar {
                anotado: Vec::new(),
                vista: (0.0, 0.0, 10.0, 10.0),
                papel: papel(0),
            },
        );
        assert!(gesto.seleccion.esta_vacia());
        assert_eq!(escena.cuantos_visibles(), 0);
    }
}
