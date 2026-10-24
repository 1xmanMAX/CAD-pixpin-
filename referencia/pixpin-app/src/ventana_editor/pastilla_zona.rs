//! **La pastilla de la Zona** (F8; `PastillaDeLaZona` del movil).
//!
//! Con la herramienta Zona en la mano sale arriba en el centro, justo debajo
//! de la barra, una pastilla que dice que va a pasar al arrastrar: «Arrastra:
//! sale una copia» o «Arrastra: va al chat del proyecto», y a su derecha el
//! interruptor **«Al chat»**. Es lo mismo que el movil pone bajo su rotulo.
//!
//! El interruptor solo sale si el lienzo es una hoja de un proyecto (lo
//! abrio el chat: ver `zona_al_chat`). En el movil sale siempre y, sin
//! proyecto, pregunta a cual; aqui un lienzo sin proyecto saca la copia.
//!
//! Tras mandar una zona, la pastilla dice como fue («Zona mandada al chat de
//! «Casa»»): es el `Toast` del movil, que aqui no tiene donde salir. Se va al
//! tocar el interruptor o al sacar otra zona.

use pixpin_render::icono::material;
use pixpin_render::{Color, Pintor, RectF};
use std::cell::Cell;

use crate::caja_dibujo::{hex, sombra_isla};

const ISLA: Color = hex(0xffffff);
const TINTA: Color = hex(0x1b1b1f);
const APAGADO: Color = hex(0xc9c9d1);

/// Donde va cada pieza, en pixeles de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Colocada {
    pub isla: RectF,
    pub icono: RectF,
    /// Donde empieza el texto (x) y su linea de arriba (y).
    pub texto: (f32, f32),
    /// La etiqueta «Al chat» y el interruptor, si los hay.
    pub etiqueta: Option<(f32, f32)>,
    pub interruptor: Option<RectF>,
}

/// Los tamanos de la pastilla a `k` (escala de pantalla).
const ALTO: f32 = 36.0;
const TAM_TEXTO: f32 = 13.0;
const TAM_ETIQUETA: f32 = 12.0;

/// **Coloca la pastilla**: centrada en `ancho`, a 10 px bajo la barra.
/// `texto` y `etiqueta` son lo que miden esos textos (la etiqueta, solo si
/// hay interruptor). Pura, para probarla sin ventana.
pub(super) fn colocar(
    ancho: f32,
    bajo_la_barra: f32,
    k: f32,
    texto: (f32, f32),
    etiqueta: Option<(f32, f32)>,
) -> Colocada {
    let alto = ALTO * k;
    let icono = 18.0 * k;
    let (sw, sh) = (34.0 * k, 18.0 * k);
    let mut w = 12.0 * k + icono + 8.0 * k + texto.0 + 12.0 * k;
    if let Some((ew, _)) = etiqueta {
        // La etiqueta y el interruptor van tras el aire del texto, con 8 px
        // de borde al final.
        w += ew + 6.0 * k + sw + 8.0 * k;
    }
    let x = ((ancho - w) / 2.0).max(0.0);
    let y = bajo_la_barra + 10.0 * k;
    let medio = y + alto / 2.0;
    let icono_r = RectF {
        x: x + 12.0 * k,
        y: medio - icono / 2.0,
        ancho: icono,
        alto: icono,
    };
    let tx = icono_r.x + icono + 8.0 * k;
    let (etiqueta_pos, interruptor) = match etiqueta {
        Some((ew, eh)) => {
            let ex = tx + texto.0 + 12.0 * k;
            let s = RectF {
                x: ex + ew + 6.0 * k,
                y: medio - sh / 2.0,
                ancho: sw,
                alto: sh,
            };
            (Some((ex, medio - eh / 2.0)), Some(s))
        }
        None => (None, None),
    };
    Colocada {
        isla: RectF {
            x,
            y,
            ancho: w,
            alto,
        },
        icono: icono_r,
        texto: (tx, medio - texto.1 / 2.0),
        etiqueta: etiqueta_pos,
        interruptor,
    }
}

/// Lo que se hizo con un clic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Clic {
    /// No era suyo: sigue al lienzo.
    Fuera,
    /// Cayo en la pastilla pero no en el interruptor: no pinta una zona.
    Tragado,
    /// Cambio el interruptor.
    Cambiado,
}

/// El estado de la pastilla mientras dura el editor.
#[derive(Debug, Default)]
pub(super) struct PastillaZona {
    /// El interruptor «Al chat» (`zonaAlChat`): empieza apagado, como en el movil.
    pub al_chat: bool,
    /// Lo que se dice tras mandar una zona, en vez de la ayuda.
    pub dicho: Option<String>,
    /// Lo ultimo pintado: la isla y donde empieza lo del interruptor (la
    /// etiqueta, si habia), para saber
    /// donde cae un clic sin volver a medir los textos.
    ultima: Cell<Option<(RectF, Option<f32>)>>,
}

fn dentro(r: RectF, x: f32, y: f32) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.ancho && y < r.y + r.alto
}

impl PastillaZona {
    /// Un clic en `(x, y)`, pixeles de la ventana. La pastilla solo existe
    /// si se pinto (`visible`): con otra herramienta no se traga nada.
    pub fn pulsar(&mut self, x: f32, y: f32, visible: bool) -> Clic {
        let Some((isla, interruptor)) = self.ultima.get().filter(|_| visible) else {
            return Clic::Fuera;
        };
        if !dentro(isla, x, y) {
            return Clic::Fuera;
        }
        // La etiqueta cuenta como interruptor: el hueco del raton es mas
        // pequeno que el del dedo, y «Al chat» es lo que se lee.
        match interruptor {
            Some(desde) if x >= desde => {
                self.al_chat = !self.al_chat;
                self.dicho = None;
                Clic::Cambiado
            }
            _ => Clic::Tragado,
        }
    }

    /// Si la zona que se suelta va al chat: el interruptor puesto y un
    /// proyecto al que mandarla.
    pub fn manda_al_chat(&self, con_proyecto: bool) -> bool {
        self.al_chat && con_proyecto
    }

    /// Olvida donde estaba (otra herramienta: ya no esta en pantalla).
    pub fn esconder(&self) {
        self.ultima.set(None);
    }

    /// Pinta la pastilla. `con_proyecto` dice si hay interruptor.
    #[allow(clippy::too_many_arguments)]
    pub fn pintar(
        &self,
        p: &Pintor<'_>,
        base: (f32, f32),
        ancho: f32,
        bajo_la_barra: f32,
        escala_por_cien: u32,
        con_proyecto: bool,
        textos: &pixpin_store::Catalogo,
    ) {
        p.desplazar(base.0, base.1);
        let k = crate::navegacion::escala_de(escala_por_cien);
        let texto = self.dicho.clone().unwrap_or_else(|| {
            textos.t(if self.manda_al_chat(con_proyecto) {
                "zona-arrastra-chat"
            } else {
                "zona-arrastra-copia"
            })
        });
        let etiqueta = textos.t("zona-al-chat");
        let mt = p.medir_texto(&texto, TAM_TEXTO * k);
        let me = con_proyecto.then(|| p.medir_texto(&etiqueta, TAM_ETIQUETA * k));
        let c = colocar(ancho, bajo_la_barra, k, mt, me);
        self.ultima.set(Some((c.isla, c.etiqueta.map(|e| e.0))));
        sombra_isla(p, c.isla, c.isla.alto / 2.0, k);
        p.rellenar_redondeado(c.isla, c.isla.alto / 2.0, ISLA);
        p.icono(&material::CROP, c.icono, TINTA);
        p.texto(&texto, c.texto.0, c.texto.1, TAM_TEXTO * k, TINTA);
        if let (Some((ex, ey)), Some(s)) = (c.etiqueta, c.interruptor) {
            p.texto(&etiqueta, ex, ey, TAM_ETIQUETA * k, TINTA);
            pintar_interruptor(p, s, self.al_chat);
        }
    }
}

/// El interruptor, como el `Switch` de Material: carril redondo y bolita;
/// encendido, azul de la zona y la bolita a la derecha.
fn pintar_interruptor(p: &Pintor<'_>, s: RectF, encendido: bool) {
    let azul = {
        let c = pixpin_motor2d::zona::COLOR_DE_LA_ZONA;
        Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: 1.0,
        }
    };
    p.rellenar_redondeado(s, s.alto / 2.0, if encendido { azul } else { APAGADO });
    let r = s.alto / 2.0 - 2.5;
    let cx = if encendido {
        s.x + s.ancho - s.alto / 2.0
    } else {
        s.x + s.alto / 2.0
    };
    p.circulo((cx, s.y + s.alto / 2.0), r, Color::BLANCO);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_pastilla_va_centrada_bajo_la_barra_y_el_interruptor_a_su_derecha() {
        let c = colocar(1000.0, 60.0, 1.0, (200.0, 16.0), Some((50.0, 15.0)));
        assert!(
            (c.isla.x + c.isla.ancho / 2.0 - 500.0).abs() < 0.5,
            "centrada"
        );
        assert_eq!(c.isla.y, 70.0, "a diez pixeles bajo la barra");
        let s = c.interruptor.expect("hay interruptor");
        assert!(s.x > c.texto.0 + 200.0, "detras del texto");
        assert!(
            s.x + s.ancho <= c.isla.x + c.isla.ancho,
            "dentro de la isla"
        );
        // Caso negativo: sin proyecto no hay interruptor ni etiqueta, y es
        // mas estrecha.
        let sin = colocar(1000.0, 60.0, 1.0, (200.0, 16.0), None);
        assert!(sin.interruptor.is_none() && sin.etiqueta.is_none());
        assert!(sin.isla.ancho < c.isla.ancho);
    }

    #[test]
    fn el_interruptor_cambia_y_la_pastilla_se_traga_el_clic() {
        let mut p = PastillaZona::default();
        let c = colocar(1000.0, 60.0, 1.0, (200.0, 16.0), Some((50.0, 15.0)));
        p.ultima.set(Some((c.isla, c.etiqueta.map(|e| e.0))));
        let s = c.interruptor.unwrap();
        p.dicho = Some("Zona mandada".into());
        assert_eq!(p.pulsar(s.x + 5.0, s.y + 5.0, true), Clic::Cambiado);
        assert!(p.al_chat);
        assert_eq!(p.dicho, None, "al tocarlo se olvida lo dicho");
        assert!(p.manda_al_chat(true));
        // Sin proyecto, aunque este puesto, no manda.
        assert!(!p.manda_al_chat(false));
        // En el icono: suyo, pero no cambia nada.
        assert_eq!(
            p.pulsar(c.icono.x + 2.0, c.icono.y + 2.0, true),
            Clic::Tragado
        );
        assert!(p.al_chat);
        // Caso negativo: fuera, o con otra herramienta, sigue al lienzo.
        assert_eq!(p.pulsar(5.0, 500.0, true), Clic::Fuera);
        assert_eq!(p.pulsar(s.x + 5.0, s.y + 5.0, false), Clic::Fuera);
        assert!(p.al_chat);
    }

    #[test]
    fn sin_pintarse_no_se_traga_nada() {
        let mut p = PastillaZona::default();
        assert_eq!(p.pulsar(500.0, 80.0, true), Clic::Fuera);
    }

    /// **La pastilla en PNG**, en sus cuatro caras: sin proyecto, con el
    /// interruptor apagado, encendido y tras mandar. Necesita GPU:
    /// `cargo test -p pixpin --bin pixpinmax muestra_de_la_pastilla_de_zona
    /// -- --ignored --nocapture`. Deja los PNG en `target/muestras-zona`.
    #[test]
    #[ignore = "necesita GPU; genera PNG para mirarlos"]
    fn muestra_de_la_pastilla_de_zona() {
        use pixpin_render::MotorRender;
        use pixpin_render::fuera_de_pantalla::FueraDePantalla;
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
        let motor = MotorRender::nuevo(d.d3d()).expect("motor");
        let (ancho, alto) = (720u32, 130u32);
        let fuera = FueraDePantalla::nuevo(&motor, d.d3d(), ancho, alto).expect("superficie");
        let carpeta =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-zona");
        std::fs::create_dir_all(&carpeta).unwrap();
        let textos = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let casos = [
            ("sin-proyecto", false, false, None),
            ("apagado", true, false, None),
            ("al-chat", true, true, None),
            (
                "mandada",
                true,
                true,
                Some("Zona mandada al chat de «Casa Lima»"),
            ),
        ];
        for (nombre, con_proyecto, al_chat, dicho) in casos {
            let pz = PastillaZona {
                al_chat,
                dicho: dicho.map(str::to_string),
                ..Default::default()
            };
            motor
                .dibujar(&fuera.destino, |p| {
                    p.limpiar(hex(0xf4f4f6));
                    // La barra encima, para ver donde queda.
                    p.rellenar_redondeado(
                        RectF {
                            x: 160.0,
                            y: 8.0,
                            ancho: 400.0,
                            alto: 44.0,
                        },
                        8.0,
                        hex(0xffffff),
                    );
                    pz.pintar(
                        p,
                        (0.0, 0.0),
                        ancho as f32,
                        52.0,
                        100,
                        con_proyecto,
                        &textos,
                    );
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
            let ruta = carpeta.join(format!("pastilla-{nombre}.png"));
            std::fs::write(&ruta, png).expect("guardar");
            println!("{nombre}: {}", ruta.display());
        }
    }
}
