//! **Presentar** (G5; `ui/Presentacion.kt` del movil, 14-sep-2026) y el
//! **modo visualizacion** (`soloMirar` / `BarraDeVista`).
//!
//! Presentar: fuera todas las barras, cada diapositiva encajada en la
//! pantalla y abajo una pastilla para pasar y para anotar encima mientras se
//! habla. En el movil una diapositiva es la pagina del PDF de fondo o cada
//! hoja del cuaderno; aqui, **cada marco** en el orden del cuaderno
//! (`marco::hojas_en_orden`, el mismo de imprimir por marcos, F11), y sin
//! marcos, el lienzo entero. Lo que se anota encima se guarda como siempre.
//!
//! Lo que el movil hace con el dedo, aqui con el teclado de un mando de
//! presentaciones (que manda Av Pag / Re Pag o las flechas) y con el raton:
//!
//! - **F5** empieza; **Esc** sale y deja la vista y la herramienta como
//!   estaban.
//! - **→ ↓ Av Pag Espacio Intro** pasan; **← ↑ Re Pag Retroceso** vuelven;
//!   **Inicio/Fin**, a la primera y la ultima.
//! - **B** o **.** fundido a negro (como PowerPoint); otra vez, vuelve.
//! - En «Pasar», un clic en el tercio derecho pasa, en el izquierdo vuelve y
//!   en el medio esconde o ensena la pastilla (`ZonaDePasar`).
//! - La pastilla: anterior, «3 / 7», siguiente, y los modos Pasar, Laser,
//!   Lapiz, Resaltador y Goma (el movil tiene los cuatro sin laser: el laser
//!   es de escritorio, ver `puntero_laser.rs`), y salir. **J** laser, **L**
//!   lapiz, **R** resaltador, **E** goma, **M** pasar.
//!
//! El modo visualizacion (**Alt+R**, el atajo de Excalidraw): sin barra ni
//! panel, solo mirar —mover y acercar— y nada se edita; una etiqueta arriba
//! dice como salir. De ahi, F5 presenta.

use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::Herramienta;
use pixpin_render::{Color, Pintor, RectF};

/// Que hace el raton mientras se presenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Modo {
    Pasar,
    Laser,
    Lapiz,
    Resaltador,
    Goma,
}

impl Modo {
    pub const TODOS: [Modo; 5] = [
        Modo::Pasar,
        Modo::Laser,
        Modo::Lapiz,
        Modo::Resaltador,
        Modo::Goma,
    ];

    /// La herramienta del motor que hace el trabajo (`ponerModoDePresentacion`).
    pub fn herramienta(self) -> Herramienta {
        match self {
            Modo::Pasar => Herramienta::Mano,
            Modo::Laser => Herramienta::Laser,
            Modo::Lapiz => Herramienta::Lapiz,
            Modo::Resaltador => Herramienta::Resaltador,
            Modo::Goma => Herramienta::Borrador,
        }
    }
}

/// Lo que se pide con una tecla o un clic mientras se presenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Accion {
    /// Pasar tantas diapositivas (negativo, atras).
    Pasar(i32),
    Primera,
    Ultima,
    Negro,
    Modo(Modo),
    Pastilla,
    Salir,
}

/// Un boton de la pastilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Boton {
    Anterior,
    Siguiente,
    Modo(Modo),
    Salir,
}

/// La presentacion en curso.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Presentacion {
    /// Las diapositivas: la caja de cada marco, en orden; sin marcos, la del
    /// dibujo entero.
    pub hojas: Vec<(f32, f32, f32, f32)>,
    pub actual: usize,
    pub negro: bool,
    pub modo: Modo,
    /// La pastilla a la vista (en «Pasar», el clic del medio la esconde).
    pub pastilla: bool,
    /// Como estaba el lienzo al empezar, para dejarlo igual al salir.
    pub antes: (Camara, Herramienta),
}

/// El aire que se deja alrededor de la diapositiva: un 3 % por lado.
const AIRE: f32 = 0.94;

/// Las diapositivas de una escena: sus marcos en orden o, sin marcos, el
/// dibujo entero con un margen. Vacio si no hay nada que ensenar.
pub(crate) fn hojas_de(escena: &Escena) -> Vec<(f32, f32, f32, f32)> {
    let marcos: Vec<_> = pixpin_motor2d::marco::hojas_en_orden(&escena.elementos)
        .into_iter()
        .map(|m| m.caja())
        .filter(|c| c.2 > c.0 && c.3 > c.1)
        .collect();
    if !marcos.is_empty() {
        return marcos;
    }
    let caja = escena
        .visibles()
        .map(|e| e.caja())
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)));
    caja.filter(|c| c.2 > c.0 && c.3 > c.1)
        .map(|c| {
            let m = 20.0;
            vec![(c.0 - m, c.1 - m, c.2 + m, c.3 + m)]
        })
        .unwrap_or_default()
}

impl Presentacion {
    pub fn empezar(
        escena: &Escena,
        camara: Camara,
        herramienta: Herramienta,
    ) -> Option<Presentacion> {
        let hojas = hojas_de(escena);
        (!hojas.is_empty()).then_some(Presentacion {
            hojas,
            actual: 0,
            negro: false,
            modo: Modo::Pasar,
            pastilla: true,
            antes: (camara, herramienta),
        })
    }

    /// **La camara que encaja la diapositiva de ahora** en la ventana. La
    /// camara del editor va en pixeles logicos (la efectiva multiplica por
    /// la escala del monitor), asi que se encaja en el tamano logico.
    pub fn camara(&self, ancho_px: f32, alto_px: f32, escala_por_cien: u32) -> Camara {
        let k = escala_por_cien.max(1) as f32 / 100.0;
        let (ancho, alto) = (ancho_px / k, alto_px / k);
        let (x0, y0, x1, y1) = self.hojas[self.actual.min(self.hojas.len() - 1)];
        let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
        let zoom = (ancho / w).min(alto / h) * AIRE;
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        Camara {
            x: cx - ancho / 2.0 / zoom,
            y: cy - alto / 2.0 / zoom,
            zoom,
        }
    }

    /// Pasa `delta` diapositivas sin salirse. `true` si cambio de diapositiva.
    pub fn pasar(&mut self, delta: i32) -> bool {
        let n = self.hojas.len() as i32;
        let nueva = (self.actual as i32 + delta).clamp(0, n - 1) as usize;
        let cambio = nueva != self.actual;
        self.actual = nueva;
        cambio
    }

    /// Lo que hace una tecla (codigo virtual de Windows). `None`: no es suya.
    pub fn tecla(vk: u32) -> Option<Accion> {
        const VK_RETROCESO: u32 = 0x08;
        const VK_INTRO: u32 = 0x0D;
        const VK_ESCAPE: u32 = 0x1B;
        const VK_ESPACIO: u32 = 0x20;
        const VK_REPAG: u32 = 0x21;
        const VK_AVPAG: u32 = 0x22;
        const VK_FIN: u32 = 0x23;
        const VK_INICIO: u32 = 0x24;
        const VK_IZQUIERDA: u32 = 0x25;
        const VK_ARRIBA: u32 = 0x26;
        const VK_DERECHA: u32 = 0x27;
        const VK_ABAJO: u32 = 0x28;
        const VK_PUNTO: u32 = 0xBE;
        Some(match vk {
            VK_DERECHA | VK_ABAJO | VK_AVPAG | VK_ESPACIO | VK_INTRO => Accion::Pasar(1),
            VK_IZQUIERDA | VK_ARRIBA | VK_REPAG | VK_RETROCESO => Accion::Pasar(-1),
            VK_INICIO => Accion::Primera,
            VK_FIN => Accion::Ultima,
            VK_ESCAPE => Accion::Salir,
            VK_PUNTO => Accion::Negro,
            v if v == b'B' as u32 => Accion::Negro,
            v if v == b'J' as u32 => Accion::Modo(Modo::Laser),
            v if v == b'L' as u32 => Accion::Modo(Modo::Lapiz),
            v if v == b'R' as u32 => Accion::Modo(Modo::Resaltador),
            v if v == b'E' as u32 => Accion::Modo(Modo::Goma),
            v if v == b'M' as u32 => Accion::Modo(Modo::Pasar),
            _ => return None,
        })
    }

    /// Lo que hace un clic en «Pasar» fuera de la pastilla: el tercio
    /// derecho pasa, el izquierdo vuelve y el medio esconde o ensena la
    /// pastilla (`ZonaDePasar`).
    pub fn clic_para_pasar(x: f32, ancho_px: f32) -> Accion {
        let tercio = ancho_px / 3.0;
        if x < tercio {
            Accion::Pasar(-1)
        } else if x > tercio * 2.0 {
            Accion::Pasar(1)
        } else {
            Accion::Pastilla
        }
    }

    /// Aplica una accion. `true` si hay que salir de la presentacion.
    pub fn aplicar(&mut self, a: Accion) -> bool {
        match a {
            Accion::Pasar(d) => {
                self.pasar(d);
            }
            Accion::Primera => self.actual = 0,
            Accion::Ultima => self.actual = self.hojas.len() - 1,
            Accion::Negro => self.negro = !self.negro,
            Accion::Modo(m) => self.modo = m,
            Accion::Pastilla => self.pastilla = !self.pastilla,
            Accion::Salir => return true,
        }
        false
    }
}

/// Los botones de la pastilla y donde van, en pixeles de la ventana: una
/// isla abajo en el centro, como la del movil.
pub(crate) fn pastilla(
    ancho_px: f32,
    alto_px: f32,
    escala_por_cien: u32,
) -> (RectF, Vec<(Boton, RectF)>) {
    let k = escala_por_cien as f32 / 100.0;
    let lado = 36.0 * k;
    let hueco = 4.0 * k;
    let contador = 64.0 * k;
    let separador = 12.0 * k;
    let botones: Vec<Boton> = std::iter::once(Boton::Anterior)
        .chain(std::iter::once(Boton::Siguiente))
        .chain(Modo::TODOS.iter().map(|m| Boton::Modo(*m)))
        .chain(std::iter::once(Boton::Salir))
        .collect();
    let ancho = hueco * 2.0 + botones.len() as f32 * (lado + hueco) + contador + separador * 2.0;
    let alto = lado + hueco * 2.0;
    let isla = RectF {
        x: (ancho_px - ancho) / 2.0,
        y: alto_px - alto - 20.0 * k,
        ancho,
        alto,
    };
    let mut x = isla.x + hueco;
    let mut v = Vec::new();
    for b in botones {
        // El contador entre anterior y siguiente, y un respiro antes de los
        // modos y antes de salir.
        match b {
            Boton::Siguiente => x += contador,
            Boton::Modo(Modo::Pasar) | Boton::Salir => x += separador,
            _ => {}
        }
        v.push((
            b,
            RectF {
                x,
                y: isla.y + hueco,
                ancho: lado,
                alto: lado,
            },
        ));
        x += lado + hueco;
    }
    (isla, v)
}

/// El boton de la pastilla bajo un punto de la ventana.
pub(crate) fn boton_en(
    x: f32,
    y: f32,
    ancho_px: f32,
    alto_px: f32,
    escala_por_cien: u32,
) -> Option<Boton> {
    let (isla, v) = pastilla(ancho_px, alto_px, escala_por_cien);
    let dentro = |r: &RectF| x >= r.x && x <= r.x + r.ancho && y >= r.y && y <= r.y + r.alto;
    if !dentro(&isla) {
        return None;
    }
    v.into_iter().find(|(_, r)| dentro(r)).map(|(b, _)| b)
}

/// Si un punto cae en la isla de la pastilla (aunque no en un boton).
pub(crate) fn en_la_pastilla(
    x: f32,
    y: f32,
    ancho_px: f32,
    alto_px: f32,
    escala_por_cien: u32,
) -> bool {
    let (isla, _) = pastilla(ancho_px, alto_px, escala_por_cien);
    x >= isla.x && x <= isla.x + isla.ancho && y >= isla.y && y <= isla.y + isla.alto
}

fn icono_de(b: Boton) -> &'static pixpin_render::icono::Icono {
    use pixpin_render::iconos_excalidraw as i;
    match b {
        // Excalidraw no trae flecha a la izquierda: la de la derecha,
        // volteada al pintarla.
        Boton::Anterior | Boton::Siguiente => &i::ARROW_RIGHT_ICON,
        Boton::Modo(Modo::Pasar) => &i::HAND_ICON,
        Boton::Modo(Modo::Laser) => &i::LASER_POINTER_TOOL_ICON,
        Boton::Modo(Modo::Lapiz) => &i::FREEDRAW_ICON,
        Boton::Modo(Modo::Resaltador) => &i::PEN_MODE_ICON,
        Boton::Modo(Modo::Goma) => &i::ERASER_ICON,
        Boton::Salir => &i::CLOSE_ICON,
    }
}

/// **Pinta lo de la presentacion** encima de todo: el fundido a negro y la
/// pastilla. En coordenadas de la ventana (`base` es lo que pide sumar la
/// capa de la interfaz).
pub(crate) fn pintar(
    p: &Pintor<'_>,
    base: (f32, f32),
    pr: &Presentacion,
    ancho_px: f32,
    alto_px: f32,
    escala_por_cien: u32,
    ayuda: &str,
) {
    p.desplazar(base.0, base.1);
    if pr.negro {
        p.rellenar(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: ancho_px,
                alto: alto_px,
            },
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        );
        return;
    }
    if !pr.pastilla {
        return;
    }
    let k = escala_por_cien as f32 / 100.0;
    let (isla, botones) = pastilla(ancho_px, alto_px, escala_por_cien);
    p.rellenar_redondeado(
        isla,
        isla.alto / 2.0,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.62,
        },
    );
    let apagado = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.35,
    };
    for (b, r) in &botones {
        let puesto = matches!(b, Boton::Modo(m) if *m == pr.modo);
        if puesto {
            p.rellenar_redondeado(
                *r,
                r.alto / 2.0,
                Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.28,
                },
            );
        }
        let habil = match b {
            Boton::Anterior => pr.actual > 0,
            Boton::Siguiente => pr.actual + 1 < pr.hojas.len(),
            _ => true,
        };
        let m = 8.0 * k;
        let caja = RectF {
            x: r.x + m,
            y: r.y + m,
            ancho: r.ancho - 2.0 * m,
            alto: r.alto - 2.0 * m,
        };
        let color = if habil { Color::BLANCO } else { apagado };
        if *b == Boton::Anterior {
            p.icono_volteado(icono_de(*b), caja, color);
        } else {
            p.icono(icono_de(*b), caja, color);
        }
    }
    // El contador, entre anterior y siguiente.
    let texto = format!("{} / {}", pr.actual + 1, pr.hojas.len());
    let tam = 14.0 * k;
    let (tw, th) = p.medir_texto(&texto, tam);
    let anterior = botones[0].1;
    let hueco = botones[1].1.x - (anterior.x + anterior.ancho);
    p.texto(
        &texto,
        anterior.x + anterior.ancho + (hueco - tw) / 2.0,
        isla.y + (isla.alto - th) / 2.0,
        tam,
        Color::BLANCO,
    );
    // La ayuda, pequena, encima de la pastilla.
    let t2 = 11.0 * k;
    let (aw, ah) = p.medir_texto(ayuda, t2);
    p.texto(
        ayuda,
        (ancho_px - aw) / 2.0,
        isla.y - ah - 6.0 * k,
        t2,
        Color {
            r: 0.45,
            g: 0.45,
            b: 0.5,
            a: 0.9,
        },
    );
}

/// La etiqueta del modo visualizacion, arriba en el centro.
pub(crate) fn pintar_solo_mirar(
    p: &Pintor<'_>,
    base: (f32, f32),
    ancho_px: f32,
    escala_por_cien: u32,
    texto: &str,
) {
    p.desplazar(base.0, base.1);
    let k = escala_por_cien as f32 / 100.0;
    let tam = 13.0 * k;
    let (w, h) = p.medir_texto(texto, tam);
    let caja = RectF {
        x: (ancho_px - w) / 2.0 - 14.0 * k,
        y: 16.0 * k,
        ancho: w + 28.0 * k,
        alto: h + 14.0 * k,
    };
    p.rellenar_redondeado(
        caja,
        caja.alto / 2.0,
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.62,
        },
    );
    p.texto(
        texto,
        caja.x + 14.0 * k,
        caja.y + 7.0 * k,
        tam,
        Color::BLANCO,
    );
}

/// **Arrastrar para mover** en el modo visualizacion: la mano de Excalidraw
/// en su «view mode». Guarda donde se pulso y la camara de entonces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Arrastre {
    pub desde: (f32, f32),
    pub camara: Camara,
}

impl Arrastre {
    /// La camara tras llevar el raton a `(x, y)`: el dibujo sigue al raton.
    /// `zoom_efectivo` es el de la vista (pixeles de pantalla por unidad).
    pub fn camara_en(&self, x: f32, y: f32, zoom_efectivo: f32) -> Camara {
        let z = zoom_efectivo.max(1e-4);
        Camara {
            x: self.camara.x - (x - self.desde.0) / z,
            y: self.camara.y - (y - self.desde.1) / z,
            zoom: self.camara.zoom,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::elemento::{Elemento, Figura};

    fn marco(x: f32, y: f32, w: f32, h: f32) -> Elemento {
        Elemento {
            figura: Figura::Marco {
                nombre: String::new(),
            },
            x,
            y,
            ancho: w,
            alto: h,
            ..Default::default()
        }
    }

    fn escena_con(v: Vec<Elemento>) -> Escena {
        let mut e = Escena::nueva();
        for x in v {
            e.anadir(x);
        }
        e
    }

    #[test]
    fn cada_marco_es_una_diapositiva_en_el_orden_del_cuaderno() {
        let e = escena_con(vec![
            marco(0.0, 900.0, 800.0, 600.0),
            marco(0.0, 0.0, 800.0, 600.0),
        ]);
        let h = hojas_de(&e);
        assert_eq!(
            h,
            vec![(0.0, 0.0, 800.0, 600.0), (0.0, 900.0, 800.0, 1500.0)]
        );
    }

    #[test]
    fn sin_marcos_se_presenta_el_dibujo_entero_y_sin_nada_no_se_empieza() {
        let r = Elemento {
            x: 10.0,
            y: 10.0,
            ancho: 100.0,
            alto: 50.0,
            ..Default::default()
        };
        let h = hojas_de(&escena_con(vec![r]));
        assert_eq!(h.len(), 1);
        assert!(h[0].0 < 10.0 && h[0].2 > 110.0);
        assert!(
            Presentacion::empezar(&Escena::nueva(), Camara::nueva(), Herramienta::Lapiz).is_none()
        );
    }

    #[test]
    fn la_camara_encaja_la_diapositiva_centrada_y_con_aire() {
        let e = escena_con(vec![marco(0.0, 0.0, 800.0, 600.0)]);
        let pr = Presentacion::empezar(&e, Camara::nueva(), Herramienta::Lapiz).unwrap();
        // 1920 x 1080 al 100 %: manda el alto, 1080 / 600 * 0,94.
        let c = pr.camara(1920.0, 1080.0, 100);
        assert!((c.zoom - 1080.0 / 600.0 * AIRE).abs() < 1e-4);
        let centro = c.a_pantalla(pixpin_motor2d::vector::Punto2::nuevo(400.0, 300.0));
        assert!((centro.x - 960.0).abs() < 0.5 && (centro.y - 540.0).abs() < 0.5);
        // Al 150 % la camara logica es un 150 % menor: la efectiva es la misma.
        let c15 = pr.camara(1920.0, 1080.0, 150);
        assert!((c15.zoom * 1.5 - c.zoom).abs() < 1e-4);
    }

    #[test]
    fn pasar_no_se_sale_de_las_diapositivas() {
        let e = escena_con(vec![
            marco(0.0, 0.0, 10.0, 10.0),
            marco(0.0, 20.0, 10.0, 10.0),
        ]);
        let mut pr = Presentacion::empezar(&e, Camara::nueva(), Herramienta::Lapiz).unwrap();
        assert!(!pr.pasar(-1), "antes de la primera no hay nada");
        assert!(pr.pasar(1));
        assert!(!pr.pasar(1), "despues de la ultima tampoco");
        assert_eq!(pr.actual, 1);
        pr.aplicar(Accion::Primera);
        assert_eq!(pr.actual, 0);
    }

    #[test]
    fn el_mando_de_presentaciones_pasa_con_avpag_y_la_b_funde_a_negro() {
        assert_eq!(Presentacion::tecla(0x22), Some(Accion::Pasar(1)));
        assert_eq!(Presentacion::tecla(0x21), Some(Accion::Pasar(-1)));
        assert_eq!(Presentacion::tecla(0x27), Some(Accion::Pasar(1)));
        assert_eq!(Presentacion::tecla(b'B' as u32), Some(Accion::Negro));
        assert_eq!(Presentacion::tecla(0x1B), Some(Accion::Salir));
        assert_eq!(
            Presentacion::tecla(b'J' as u32),
            Some(Accion::Modo(Modo::Laser))
        );
        // Caso negativo: una letra sin papel no es de la presentacion.
        assert_eq!(Presentacion::tecla(b'Q' as u32), None);
        let e = escena_con(vec![marco(0.0, 0.0, 10.0, 10.0)]);
        let mut pr = Presentacion::empezar(&e, Camara::nueva(), Herramienta::Lapiz).unwrap();
        assert!(!pr.aplicar(Accion::Negro));
        assert!(pr.negro);
        assert!(pr.aplicar(Accion::Salir));
    }

    #[test]
    fn un_clic_en_los_tercios_pasa_o_vuelve_y_en_el_medio_esconde_la_pastilla() {
        assert_eq!(
            Presentacion::clic_para_pasar(1800.0, 1920.0),
            Accion::Pasar(1)
        );
        assert_eq!(
            Presentacion::clic_para_pasar(100.0, 1920.0),
            Accion::Pasar(-1)
        );
        assert_eq!(
            Presentacion::clic_para_pasar(960.0, 1920.0),
            Accion::Pastilla
        );
    }

    #[test]
    fn cada_boton_de_la_pastilla_se_encuentra_en_su_sitio_y_no_se_pisan() {
        let (isla, v) = pastilla(1920.0, 1080.0, 125);
        assert_eq!(v.len(), 8);
        for (i, (b, r)) in v.iter().enumerate() {
            let c = (r.x + r.ancho / 2.0, r.y + r.alto / 2.0);
            assert_eq!(boton_en(c.0, c.1, 1920.0, 1080.0, 125), Some(*b));
            assert!(r.x >= isla.x && r.x + r.ancho <= isla.x + isla.ancho + 0.01);
            if i > 0 {
                assert!(r.x >= v[i - 1].1.x + v[i - 1].1.ancho);
            }
        }
        // Caso negativo: fuera de la isla no hay boton.
        assert_eq!(boton_en(10.0, 10.0, 1920.0, 1080.0, 125), None);
    }

    #[test]
    fn arrastrar_en_solo_mirar_lleva_el_dibujo_con_el_raton() {
        let a = Arrastre {
            desde: (100.0, 100.0),
            camara: Camara {
                x: 0.0,
                y: 0.0,
                zoom: 1.0,
            },
        };
        let c = a.camara_en(150.0, 80.0, 2.0);
        assert_eq!((c.x, c.y), (-25.0, 10.0));
    }
}
