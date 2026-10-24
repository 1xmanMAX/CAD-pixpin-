//! La ventana del editor de grabaciones (P5b.4).
//!
//! Al parar de grabar no se guarda a lo bruto. Se abre esto, que ensena lo
//! grabado dando vueltas, deja recorrerlo con la linea de tiempo y cambiar
//! la velocidad, y solo entonces pregunta que hacer con ello. Media
//! grabacion sale mal a la primera, y guardarlas sin mirar llena la
//! carpeta de ficheros que hay que borrar despues.
//!
//! El reloj y la geometria estan en `reproductor`, aparte y comprobables.
//! Aqui esta lo que necesita una pantalla.

use anyhow::{Context, Result};
use pixpin_geom::{Punto, Rect};
use pixpin_render::{Color, MotorRender, RectF, Superficie};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use std::time::Instant;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;

use crate::grabador::Grabacion;
use crate::overlay::Recursos;
use crate::reproductor::{
    Formato, MANDOS_ALTO, MARGEN, Mando, Reproductor, Salida, linea_tiempo, mando_en, mandos,
    medida_ventana,
};

const FONDO: Color = Color {
    r: 0.11,
    g: 0.11,
    b: 0.12,
    a: 0.98,
};
const TINTA: Color = Color {
    r: 0.95,
    g: 0.95,
    b: 0.96,
    a: 1.0,
};
const TENUE: Color = Color {
    r: 0.55,
    g: 0.56,
    b: 0.60,
    a: 1.0,
};
const PISTA: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.16,
};
const AVANCE: Color = Color {
    r: 0.16,
    g: 0.51,
    b: 0.96,
    a: 1.0,
};
const RESALTE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.12,
};
/// El tablero de cuadros que se ve detras de lo transparente, como en
/// cualquier programa de imagen: sin el, un fotograma con zonas
/// transparentes se confundiria con uno de fondo negro.
const CUADRO_CLARO: Color = Color {
    r: 0.22,
    g: 0.22,
    b: 0.24,
    a: 1.0,
};
const CUADRO_OSCURO: Color = Color {
    r: 0.17,
    g: 0.17,
    b: 0.19,
    a: 1.0,
};
const CUADRO_LADO: f32 = 8.0;

/// Abre el editor y no vuelve hasta que se decide que hacer.
///
/// Devuelve `Descartar` si no se pudo abrir la ventana: mas vale perder la
/// grabacion que dejar al usuario sin manera de contestar.
pub fn abrir(
    recursos: &Recursos,
    grabacion: &Grabacion,
    monitor: &pixpin_geom::Monitor,
    textos: &Catalogo,
) -> Result<(Salida, Formato)> {
    let Some(primero) = grabacion.fotogramas.first() else {
        return Ok((Salida::Descartar, Formato::Gif));
    };
    let escala_pantalla = monitor.escala_por_cien as f32 / 100.0;
    // Se mide en pixeles logicos y se lleva a fisicos al final: mezclarlos
    // es lo que hace que en una pantalla al 150% los botones queden a dos
    // tercios de donde se ven.
    let (ancho, alto, escala_imagen) = medida_ventana(
        primero.ancho,
        primero.alto,
        (monitor.area.ancho as f32 / escala_pantalla) as u32,
        (monitor.area.alto as f32 / escala_pantalla) as u32,
    );
    let fisico_ancho = (ancho as f32 * escala_pantalla) as u32;
    let fisico_alto = (alto as f32 * escala_pantalla) as u32;
    let marco = Rect {
        x: monitor.area.x + (monitor.area.ancho as i32 - fisico_ancho as i32) / 2,
        y: monitor.area.y + (monitor.area.alto as i32 - fisico_alto as i32) / 2,
        ancho: fisico_ancho,
        alto: fisico_alto,
    };

    let ventana = match VentanaOverlay::nueva(marco) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(?e, "no se pudo abrir el editor de la grabacion");
            return Ok((Salida::Descartar, Formato::Gif));
        }
    };
    let motor = recursos.motor();
    let superficie = Superficie::nueva(
        &motor,
        &recursos.d3d(),
        ventana.handle(),
        fisico_ancho,
        fisico_alto,
    )
    .context("sin superficie para el editor")?;
    ventana.mostrar();
    ventana.enfocar();

    let mut reproductor = Reproductor::nuevo(grabacion.fotogramas.len(), grabacion.por_segundo);
    let mut resaltado: Option<Mando> = None;
    // El GIF de partida: es lo que se pega en cualquier sitio y se ve
    // solo, que es para lo que se graba casi siempre.
    let mut formato = Formato::Gif;
    let mut arrastrando_tiempo = false;
    // El fotograma que hay subido a la tarjeta grafica, y cual es. Se sube
    // solo al cambiar: subir el mismo en cada vuelta serian megabytes por
    // segundo de trabajo tirado.
    let mut bitmap: Option<(usize, ID2D1Bitmap1)> = None;
    let mut ultimo = Instant::now();
    let mut pintado: Option<(usize, bool, usize, Option<Mando>, Formato)> = None;

    let salida = loop {
        pixpin_shell::overlay::bombear_pendientes();
        let ahora = Instant::now();
        reproductor.avanzar(ahora.duration_since(ultimo));
        ultimo = ahora;

        let mut decidido = None;
        for (hwnd, evento) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            // De coordenadas del escritorio a pixeles logicos de la
            // ventana, que es en lo que hablan `mandos` y `linea_tiempo`.
            let local = |p: Punto| {
                (
                    (p.x - marco.x) as f32 / escala_pantalla,
                    (p.y - marco.y) as f32 / escala_pantalla,
                )
            };
            match evento {
                EventoOverlay::BotonPulsado(p) => {
                    let (x, y) = local(p);
                    if let Some(m) = mando_en(ancho, alto, x, y) {
                        match m {
                            Mando::Reproducir => {
                                reproductor.reproduciendo = !reproductor.reproduciendo
                            }
                            Mando::Anterior => reproductor.paso(false),
                            Mando::Siguiente => reproductor.paso(true),
                            Mando::Velocidad => reproductor.siguiente_velocidad(),
                            Mando::Formato => formato = formato.siguiente(),
                            Mando::Guardar => decidido = Some(Salida::Guardar),
                            Mando::GuardadoRapido => decidido = Some(Salida::GuardadoRapido),
                            Mando::Copiar => decidido = Some(Salida::Copiar),
                            Mando::Descartar => decidido = Some(Salida::Descartar),
                        }
                        continue;
                    }
                    // Pinchar la linea de tiempo lleva ahi y para la
                    // reproduccion, para poder mirar el fotograma sin que
                    // se escape.
                    let pista = linea_tiempo(ancho, alto);
                    if y >= pista.y - 8.0 && y <= pista.y + pista.alto + 8.0 {
                        reproductor.reproduciendo = false;
                        reproductor.ir_a((x - pista.x) / pista.ancho);
                        arrastrando_tiempo = true;
                    }
                }
                EventoOverlay::RatonMovido(p) => {
                    let (x, y) = local(p);
                    if arrastrando_tiempo {
                        let pista = linea_tiempo(ancho, alto);
                        reproductor.ir_a((x - pista.x) / pista.ancho);
                    } else {
                        resaltado = mando_en(ancho, alto, x, y);
                    }
                }
                EventoOverlay::BotonSoltado(_) => arrastrando_tiempo = false,
                EventoOverlay::Cerrar => decidido = Some(Salida::Descartar),
                // Espacio reproduce y para; las flechas van fotograma a
                // fotograma; Intro guarda rapido y Escape descarta.
                EventoOverlay::Tecla { vk, .. } => match vk {
                    0x20 => reproductor.reproduciendo = !reproductor.reproduciendo,
                    0x25 => reproductor.paso(false),
                    0x27 => reproductor.paso(true),
                    0x0D => decidido = Some(Salida::GuardadoRapido),
                    0x1B => decidido = Some(Salida::Descartar),
                    _ => {}
                },
                _ => {}
            }
        }
        if let Some(s) = decidido {
            break s;
        }

        let fotograma = reproductor.fotograma();
        let estado = (
            fotograma,
            reproductor.reproduciendo,
            reproductor.indice_velocidad,
            resaltado,
            formato,
        );
        if pintado != Some(estado) {
            pintado = Some(estado);
            // Sube el fotograma solo si cambio el que toca.
            if bitmap.as_ref().map(|(i, _)| *i) != Some(fotograma) {
                if let Some(f) = grabacion.fotogramas.get(fotograma) {
                    match motor.bitmap_desde_pixeles(f.ancho, f.alto, &f.pixeles) {
                        Ok(b) => bitmap = Some((fotograma, b)),
                        Err(e) => tracing::warn!(?e, "fotograma que no se pudo ensenar"),
                    }
                }
            }
            pintar(
                &motor,
                &superficie,
                ancho,
                alto,
                escala_pantalla,
                escala_imagen,
                primero.ancho,
                primero.alto,
                bitmap.as_ref().map(|(_, b)| b),
                &reproductor,
                resaltado,
                formato,
                textos,
            );
        }
        // Un latido corto: el raton tiene que responder al instante, y a
        // cuatro veces la velocidad los fotogramas se suceden rapido.
        std::thread::sleep(std::time::Duration::from_millis(5));
    };

    ventana.ocultar();
    pixpin_shell::overlay::bombear_pendientes();
    Ok((salida, formato))
}

/// Pinta la ventana entera. Se le pasa todo lo que necesita en vez de
/// sacarlo de un estado compartido: asi se lee de arriba abajo.
#[allow(clippy::too_many_arguments)]
fn pintar(
    motor: &MotorRender,
    superficie: &Superficie,
    ancho: u32,
    alto: u32,
    e: f32,
    escala_imagen: f32,
    imagen_ancho: u32,
    imagen_alto: u32,
    bitmap: Option<&ID2D1Bitmap1>,
    reproductor: &Reproductor,
    resaltado: Option<Mando>,
    formato: Formato,
    textos: &Catalogo,
) {
    let Ok(destino) = superficie.empezar(motor) else {
        return;
    };
    let (ancho_f, alto_f) = (ancho as f32 * e, alto as f32 * e);
    // La imagen va centrada en el hueco de arriba: la ventana puede ser
    // mas ancha que ella si los mandos pedian mas sitio.
    let vista_ancho = imagen_ancho as f32 * escala_imagen;
    let vista_alto = imagen_alto as f32 * escala_imagen;
    let vista = RectF {
        x: ((ancho as f32 - vista_ancho) / 2.0) * e,
        y: MARGEN as f32 * e,
        ancho: vista_ancho * e,
        alto: vista_alto * e,
    };
    let _ = motor.dibujar(&destino, |p| {
        p.limpiar_transparente();
        p.rellenar_redondeado(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: ancho_f,
                alto: alto_f,
            },
            10.0 * e,
            FONDO,
        );
        // El tablero de cuadros de debajo de la imagen.
        let lado = CUADRO_LADO * e;
        let filas = (vista.alto / lado).ceil() as i32;
        let columnas = (vista.ancho / lado).ceil() as i32;
        for fila in 0..filas {
            for columna in 0..columnas {
                let x = vista.x + columna as f32 * lado;
                let y = vista.y + fila as f32 * lado;
                p.rellenar_redondeado(
                    RectF {
                        x,
                        y,
                        // El ultimo cuadro de cada borde se recorta para no
                        // salirse de la imagen.
                        ancho: lado.min(vista.x + vista.ancho - x),
                        alto: lado.min(vista.y + vista.alto - y),
                    },
                    0.0,
                    if (fila + columna) % 2 == 0 {
                        CUADRO_CLARO
                    } else {
                        CUADRO_OSCURO
                    },
                );
            }
        }
        if let Some(b) = bitmap {
            // Sin suavizar cuando se ve a tamano real: una captura de
            // pantalla con texto se emborrona al interpolarla.
            p.bitmap(b, vista, None, escala_imagen >= 1.0);
        }

        // La linea de tiempo.
        let pista = linea_tiempo(ancho, alto);
        let pista_f = RectF {
            x: pista.x * e,
            y: pista.y * e,
            ancho: pista.ancho * e,
            alto: pista.alto * e,
        };
        p.rellenar_redondeado(pista_f, pista_f.alto / 2.0, PISTA);
        let hecho = reproductor.avance();
        if hecho > 0.0 {
            p.rellenar_redondeado(
                RectF {
                    ancho: pista_f.ancho * hecho,
                    ..pista_f
                },
                pista_f.alto / 2.0,
                AVANCE,
            );
        }
        // El agarrador, para saber donde pinchar sin adivinar.
        let radio = 7.0 * e;
        p.rellenar_redondeado(
            RectF {
                x: pista_f.x + pista_f.ancho * hecho - radio,
                y: pista_f.y + pista_f.alto / 2.0 - radio,
                ancho: radio * 2.0,
                alto: radio * 2.0,
            },
            radio,
            TINTA,
        );

        for (mando, r) in mandos(ancho, alto) {
            let caja = RectF {
                x: r.x * e,
                y: r.y * e,
                ancho: r.ancho * e,
                alto: r.alto * e,
            };
            if resaltado == Some(mando) {
                p.rellenar_redondeado(caja, 5.0 * e, RESALTE);
            }
            let etiqueta = rotulo(mando, reproductor, formato, textos);
            let color = if mando == Mando::Descartar {
                TENUE
            } else {
                TINTA
            };
            let tam = 12.0 * e;
            let (w, h) = p.medir_texto(&etiqueta, tam);
            p.texto(
                &etiqueta,
                caja.x + (caja.ancho - w) / 2.0,
                caja.y + (caja.alto - h) / 2.0,
                tam,
                color,
            );
        }
        // El tiempo, en medio y a la misma altura que los mandos: por
        // donde vas y cuanto dura. Es lo primero que se mira para saber
        // si la grabacion cogio lo que tenia que coger.
        let segundo = reproductor.fotograma() as u64 / reproductor.por_segundo.max(1) as u64;
        let tiempo = format!(
            "{} / {}",
            crate::grabador::reloj(segundo),
            crate::grabador::reloj(reproductor.duracion().as_secs())
        );
        let tam = 12.0 * e;
        let (w, h) = p.medir_texto(&tiempo, tam);
        p.texto(
            &tiempo,
            (ancho_f - w) / 2.0,
            (alto as f32 - MANDOS_ALTO as f32 + 18.0 + (24.0 - h / e) / 2.0) * e,
            tam,
            TENUE,
        );
    });
    let _ = superficie.presentar();
}

/// El rotulo de cada mando, ya traducido.
fn rotulo(mando: Mando, reproductor: &Reproductor, formato: Formato, textos: &Catalogo) -> String {
    match mando {
        Mando::Reproducir => {
            // El boton dice lo que VA A HACER, no en que estado esta: es la
            // unica lectura que no se presta a dudas.
            if reproductor.reproduciendo {
                textos.t("editor-pausar")
            } else {
                textos.t("editor-ver")
            }
        }
        Mando::Anterior => textos.t("editor-anterior"),
        Mando::Siguiente => textos.t("editor-siguiente"),
        Mando::Velocidad => {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("veces", format!("{}", reproductor.velocidad()));
            textos.t_args("editor-velocidad", &args)
        }
        Mando::Formato => formato.rotulo().to_string(),
        Mando::Guardar => textos.t("editor-guardar"),
        Mando::GuardadoRapido => textos.t("editor-guardado-rapido"),
        Mando::Copiar => textos.t("editor-copiar"),
        Mando::Descartar => textos.t("editor-descartar"),
    }
}
