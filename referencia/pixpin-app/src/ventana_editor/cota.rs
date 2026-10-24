//! **El cajetin de la cota: cuanto mide y hacia donde va** (el `DialogoDeCota`
//! del movil, `DrawToolbar.kt`).
//!
//! Se abre en cuanto se traza una cota con «Pedir la medida al trazar»
//! encendido (de fabrica, como en el movil): es el momento en que uno sabe la
//! medida. Dos casillas, el largo y el angulo; se teclea en la que esta
//! encendida y **Tab** pasa a la otra (en el movil, tocandola); el menos
//! cambia el signo del angulo; **Intro** acepta y **Escape** deja la cota
//! como se trazo, que ya dice lo que mide. El principio de la raya no se
//! mueve: ver `medida::con_largo_y_angulo`.
//!
//! Abre su propio bucle de eventos, como el cajetin de calibrar
//! (`pedir_medida`), porque tiene que seguir pintando el lienzo mientras se
//! teclea. Es hijo de `ventana_editor` para usar su `pintar` sin repetirlo.

use super::*;

/// Lo que se esta tecleando en el cajetin.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Dictado {
    pub largo: String,
    pub angulo: String,
    /// Si se teclea en el angulo (si no, en el largo).
    pub en_el_angulo: bool,
}

impl Dictado {
    /// Nace con lo que mide la raya trazada, para corregir y no reescribir.
    pub fn de(
        e: &pixpin_motor2d::Elemento,
        escala: Option<&pixpin_motor2d::medida::Escala>,
    ) -> Dictado {
        Dictado {
            largo: numero(pixpin_motor2d::medida::largo_en_unidades(e, escala)),
            angulo: numero(pixpin_motor2d::medida::angulo_de(e)),
            en_el_angulo: false,
        }
    }

    /// Una tecla. `Some(true)` si hay que repintar.
    pub fn tecla(&mut self, c: char) -> bool {
        let campo = if self.en_el_angulo {
            &mut self.angulo
        } else {
            &mut self.largo
        };
        match c {
            '\t' => self.en_el_angulo = !self.en_el_angulo,
            '\u{8}' => {
                campo.pop();
            }
            // El menos, solo en el angulo: una distancia negativa no existe.
            '-' if self.en_el_angulo => {
                if let Some(resto) = campo.strip_prefix('-') {
                    *campo = resto.to_string();
                } else {
                    campo.insert(0, '-');
                }
            }
            c if c.is_ascii_digit() || c == ',' || c == '.' => campo.push(c),
            _ => return false,
        }
        true
    }

    /// El largo y el angulo tecleados, si valen: largo positivo y angulo
    /// numerico (`valida` del cajetin del movil).
    pub fn valores(&self) -> Option<(f32, f32)> {
        let leer = |t: &str| {
            t.trim()
                .replace(',', ".")
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
        };
        let largo = leer(&self.largo).filter(|v| *v > 0.0)?;
        let angulo = if self.angulo.trim().is_empty() {
            0.0
        } else {
            leer(&self.angulo)?
        };
        Some((largo, angulo))
    }
}

/// Un numero como se escribe a mano: sin ceros de mas.
fn numero(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// Abre el cajetin para la cota `id` y devuelve lo dictado (largo en las
/// unidades de la escala, angulo en grados como en un plano), o `None` si se
/// cancela. No toca la escena: lo aplica quien llama.
#[allow(clippy::too_many_arguments)]
pub(super) fn dictar(
    ventana: &VentanaOverlay,
    motor: &mut MotorRender,
    superficie: &Superficie,
    escena: &Escena,
    camara: &Camara,
    gesto: &Gesto,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    rejilla: &Rejilla,
    capa: &CapaEstatica,
    fondo: &mut Option<FondoLienzo>,
    imagenes: &mut ImagenesLienzo,
    caja: &CajaHerramientas,
    corrimiento_ui: (f32, f32),
    escala_por_cien: u32,
    ancho_px: f32,
    alto_px: f32,
    id: u64,
) -> Option<(f32, f32)> {
    let e = escena.buscar(id)?;
    let mut dictado = Dictado::de(e, escena.escala.as_ref());
    let unidad = escena
        .escala
        .as_ref()
        .filter(|e| e.valida())
        .map_or("px".to_string(), |e| e.unidad.clone());
    let textos = exportar::textos();
    let (t_titulo, t_largo, t_angulo) = (
        textos.t("lienzo-cota-titulo"),
        textos.t("lienzo-cota-largo"),
        textos.t("lienzo-cota-angulo"),
    );
    ventana.invalidar();
    loop {
        pixpin_shell::overlay::bombear_pendientes();
        for (hwnd, ev) in pixpin_shell::overlay::tomar_eventos_pendientes() {
            if hwnd != ventana.handle() {
                continue;
            }
            match ev {
                EventoOverlay::Caracter(c) => match c {
                    '\r' | '\n' => {
                        if let Some(v) = dictado.valores() {
                            return Some(v);
                        }
                    }
                    '\u{1b}' => return None,
                    c => {
                        if dictado.tecla(c) {
                            ventana.invalidar();
                        }
                    }
                },
                EventoOverlay::Cerrar => return None,
                EventoOverlay::Pintar => {
                    let d = dictado.clone();
                    pintar(
                        motor,
                        superficie,
                        escena,
                        camara,
                        gesto,
                        cache,
                        cache_tinta,
                        rejilla,
                        capa,
                        fondo,
                        imagenes,
                        caja,
                        None,
                        corrimiento_ui,
                        true,
                        escala_por_cien,
                        ancho_px,
                        alto_px,
                        None,
                        None,
                        None,
                        true,
                        FueraDeLaEscena::default(),
                        None,
                        |p, base| {
                            dibujar(
                                p,
                                base,
                                (ancho_px, alto_px),
                                &d,
                                &unidad,
                                (&t_titulo, &t_largo, &t_angulo),
                            )
                        },
                    );
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// La tarjeta, centrada y con el mismo aspecto que la de calibrar: el titulo
/// y las dos casillas, la encendida con su filo claro.
fn dibujar(
    p: &pixpin_render::Pintor<'_>,
    base: (f32, f32),
    (ancho_px, alto_px): (f32, f32),
    d: &Dictado,
    unidad: &str,
    (titulo, t_largo, t_angulo): (&str, &str, &str),
) {
    p.desplazar(base.0, base.1);
    let (ancho, alto) = (300.0, 118.0);
    let caja = RectF {
        x: (ancho_px - ancho) / 2.0,
        y: (alto_px - alto) / 2.0,
        ancho,
        alto,
    };
    p.rellenar_redondeado(
        caja,
        8.0,
        Color {
            r: 0.12,
            g: 0.12,
            b: 0.14,
            a: 0.92,
        },
    );
    p.texto(titulo, caja.x + 16.0, caja.y + 12.0, 13.0, Color::BLANCO);
    let casilla = |i: usize, etiqueta: &str, valor: &str, sufijo: &str, activa: bool| {
        let r = RectF {
            x: caja.x + 16.0 + i as f32 * 138.0,
            y: caja.y + 38.0,
            ancho: 130.0,
            alto: 64.0,
        };
        p.rellenar_redondeado(
            r,
            6.0,
            Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: if activa { 0.16 } else { 0.06 },
            },
        );
        p.texto(etiqueta, r.x + 8.0, r.y + 6.0, 11.0, Color::BLANCO);
        let v = if valor.is_empty() { "0" } else { valor };
        let texto = format!("{v} {sufijo}");
        let (_, th) = p.medir_texto(&texto, 20.0);
        p.texto(
            &texto,
            r.x + 8.0,
            r.y + r.alto - th - 8.0,
            20.0,
            Color::BLANCO,
        );
    };
    casilla(0, t_largo, &d.largo, unidad, !d.en_el_angulo);
    casilla(1, t_angulo, &d.angulo, "°", d.en_el_angulo);
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn tab_pasa_al_angulo_y_el_menos_solo_cambia_su_signo() {
        let mut d = Dictado {
            largo: String::new(),
            angulo: String::new(),
            en_el_angulo: false,
        };
        for c in "12,5".chars() {
            d.tecla(c);
        }
        // Caso negativo: una distancia negativa no existe.
        assert!(!d.tecla('-'));
        d.tecla('\t');
        for c in "30".chars() {
            d.tecla(c);
        }
        d.tecla('-');
        assert_eq!(d.valores(), Some((12.5, -30.0)));
        d.tecla('-');
        assert_eq!(d.valores(), Some((12.5, 30.0)));
    }

    #[test]
    fn sin_largo_o_con_largo_cero_no_se_puede_aceptar() {
        let d = Dictado {
            largo: "0".into(),
            angulo: "10".into(),
            en_el_angulo: false,
        };
        assert_eq!(d.valores(), None);
        let d = Dictado {
            largo: String::new(),
            angulo: String::new(),
            en_el_angulo: false,
        };
        assert_eq!(d.valores(), None);
    }

    #[test]
    fn nace_con_lo_que_mide_la_raya_trazada() {
        use pixpin_motor2d::elemento::Figura;
        let e = pixpin_motor2d::Elemento {
            figura: Figura::Cota {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(30.0, -40.0)],
            },
            ..Default::default()
        };
        let d = Dictado::de(&e, None);
        assert_eq!(d.largo, "50");
        assert_eq!(d.angulo, "53.13");
    }
}
