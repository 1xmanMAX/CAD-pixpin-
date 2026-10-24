//! **Imprimir con vista previa** (F11): el lienzo en el dialogo moderno de
//! imprimir de Windows (`pixpin_render::imprimir_moderno`), que ensena las
//! hojas tal como saldran y las rehace al cambiar el papel o la orientacion.
//!
//! Aqui va lo que es del lienzo: que hojas salen («cada marco en su hoja»,
//! «todo el lienzo», «solo lo elegido», la opcion propia del dialogo, como el
//! selector de marcos del movil) y como se pinta una hoja en su papel. Ese
//! pintado, [`pintar_en_papel`], es el mismo para la vista previa, para el
//! papel del dialogo moderno y para el del clasico: lo que se ve es lo que
//! sale.
//!
//! Como el movil (`guardados/Imprimir.kt`): se imprime sobre el papel de la
//! impresora, **sin** el color de fondo del lienzo ni su pauta de andamio
//! (su `Renderer` de exportar no los pinta); el papel pautado de un marco y
//! la pagina de un PDF debajo si salen, porque son parte del dibujo.

use std::collections::HashMap;

use anyhow::{Context, Result};
use pixpin_codec::ImagenRgba;
use pixpin_motor2d::Escena;
use pixpin_motor2d::exportar::{self as ex, Alcance, Hoja};
use pixpin_render::imprimir_moderno::{self as moderno, Documento, OpcionPropia, Pagina, Pedido};
use pixpin_render::{MotorRender, Pintor, RectF};
use pixpin_store::Catalogo;
use windows::Win32::Foundation::HWND;

use super::{Lienzo, MARGEN_IMPRESION, Subidas, pintar_hoja, subir};

/// Los puntos por pulgada de la miniatura. Fijos y no los del monitor: a
/// 144 se ve nitida en pantallas del 100 al 150 %, y una hoja de 400 DIP
/// son 600 x 850 pixeles, nada para la GPU mas floja (HD 4000).
const PPP_PREVIA: f32 = 144.0;

/// El id de la opcion propia y de sus elementos. No se traducen: son
/// llaves, lo que se lee es su rotulo.
const OPCION: &str = "pixpin-que-imprimir";
const MARCOS: &str = "marcos";
const TODO: &str = "todo";
const SELECCION: &str = "seleccion";

/// **Pinta la hoja en su papel**: encajada con los 28 puntos de margen del
/// movil y recortada a su caja, a `pagina.escala` unidades del destino por
/// DIP de papel (1 al imprimir; menos en la miniatura).
///
/// `numero` es `(esta, de cuantas)`: con marcos, cada hoja lleva su numero
/// abajo, centrado en el margen, para ordenar lo impreso. Sin marcos (una
/// hoja sola del lienzo entero, o lo elegido) no lleva ninguno.
pub(super) fn pintar_en_papel(
    p: &Pintor<'_>,
    hoja: &Hoja,
    pagina: Pagina,
    bitmaps: &Subidas,
    numero: Option<(usize, usize)>,
) {
    let (pw, ph) = pagina.papel;
    let Some((k, dx, dy)) = ex::encaje(hoja, pw, ph, MARGEN_IMPRESION) else {
        return;
    };
    let e = pagina.escala;
    p.poner_vista((0.0, 0.0), k * e, (dx * e, dy * e));
    let (x0, y0, x1, y1) = hoja.caja;
    p.con_recorte(
        RectF {
            x: x0,
            y: y0,
            ancho: x1 - x0,
            alto: y1 - y0,
        },
        |p| pintar_hoja(p, hoja, bitmaps),
    );
    if let Some(texto) = numero.and_then(|(n, de)| texto_de_pagina(n, de)) {
        // En DIP del papel: el numero no depende de lo grande que sea el
        // marco, siempre el mismo tamano y en el mismo sitio.
        p.poner_vista((0.0, 0.0), e, (0.0, 0.0));
        let (w, h) = p.medir_texto(&texto, TAM_NUMERO);
        let y = ph - (MARGEN_IMPRESION + h) / 2.0;
        p.texto(&texto, (pw - w) / 2.0, y.max(0.0), TAM_NUMERO, GRIS_NUMERO);
    }
}

/// 9 puntos: se lee sin llamar la atencion.
const TAM_NUMERO: f32 = 12.0;
const GRIS_NUMERO: pixpin_render::Color = pixpin_render::Color {
    r: 0.35,
    g: 0.35,
    b: 0.35,
    a: 1.0,
};

/// «2 / 5». `None` si no hay que numerar (fuera de rango).
fn texto_de_pagina(n: usize, de: usize) -> Option<String> {
    (n >= 1 && n <= de).then(|| format!("{n} / {de}"))
}

/// El alcance de un elemento de la opcion. Sin opcion o con uno que no se
/// conoce, por marcos: lo que hace el movil.
fn alcance_de(id: Option<&str>) -> Alcance {
    match id {
        Some(TODO) => Alcance::Todo,
        Some(SELECCION) => Alcance::Seleccion,
        _ => Alcance::Marcos,
    }
}

/// **La opcion «Que imprimir»**, solo si hay algo que elegir: con marcos,
/// cada marco o el lienzo entero; con algo elegido, ademas solo eso. Sin
/// marcos ni seleccion las dos cosas son la misma hoja y no se pregunta.
fn opcion_que_imprimir(
    t: &Catalogo,
    hay_marcos: bool,
    hay_seleccion: bool,
) -> Option<OpcionPropia> {
    if !hay_marcos && !hay_seleccion {
        return None;
    }
    let mut elementos = Vec::new();
    if hay_marcos {
        elementos.push((MARCOS.to_string(), t.t("imprimir-que-marcos")));
    }
    elementos.push((TODO.to_string(), t.t("imprimir-que-todo")));
    if hay_seleccion {
        elementos.push((SELECCION.to_string(), t.t("imprimir-que-seleccion")));
    }
    Some(OpcionPropia {
        id: OPCION.into(),
        titulo: t.t("imprimir-que"),
        elementos,
        // Por marcos, como el movil; sin marcos, todo.
        inicial: if hay_marcos { MARCOS } else { TODO }.into(),
    })
}

/// Una copia del lienzo que vive lo que viva el dialogo: el editor sigue
/// dibujando mientras tanto, y lo que se imprime es lo que habia al pulsar.
pub(super) struct LienzoImpreso {
    escena: Escena,
    seleccion: Vec<u64>,
    papel: Option<(ImagenRgba, f32, f32)>,
    fotos: HashMap<u64, ImagenRgba>,
    nombre: String,
    hojas: Vec<Hoja>,
    /// Si las hojas llevan su numero: por marcos, y solo si los hay.
    numerar: bool,
    bitmaps: Subidas,
}

impl LienzoImpreso {
    /// Copia la escena y solo las fotos que usa (no todas las del editor).
    pub(super) fn de(lienzo: &Lienzo<'_>) -> LienzoImpreso {
        let mut fotos = HashMap::new();
        for e in lienzo.escena.visibles() {
            if let pixpin_motor2d::Figura::Imagen { id_objeto } = e.figura
                && let Some(img) = (lienzo.fotos)(id_objeto)
            {
                fotos.entry(id_objeto).or_insert_with(|| img.clone());
            }
        }
        LienzoImpreso {
            escena: lienzo.escena.clone(),
            seleccion: lienzo.seleccion.to_vec(),
            papel: lienzo.papel.map(|(i, w, h)| (i.clone(), w, h)),
            fotos,
            nombre: lienzo.nombre.clone(),
            hojas: Vec::new(),
            numerar: false,
            bitmaps: HashMap::new(),
        }
    }
}

impl Documento for LienzoImpreso {
    fn paginar(&mut self, motor: &MotorRender, alcance: Option<&str>) -> usize {
        let fotos = |id: u64| self.fotos.get(&id);
        let vista = Lienzo {
            escena: &self.escena,
            seleccion: &self.seleccion,
            papel: self.papel.as_ref().map(|(i, w, h)| (i, *w, *h)),
            fotos: &fotos,
            nombre: self.nombre.clone(),
        };
        let alcance = alcance_de(alcance);
        let hojas = vista.hojas(alcance);
        self.numerar = matches!(alcance, Alcance::Marcos) && vista.hay_marcos();
        let bitmaps = subir(motor, &hojas, &vista);
        self.hojas = hojas;
        self.bitmaps = bitmaps;
        self.hojas.len()
    }

    fn pintar(&self, i: usize, pagina: Pagina, p: &Pintor) {
        if let Some(hoja) = self.hojas.get(i) {
            let numero = self.numerar.then_some((i + 1, self.hojas.len()));
            pintar_en_papel(p, hoja, pagina, &self.bitmaps, numero);
        }
    }
}

/// **Abre el dialogo moderno** con el lienzo. `Err` si no se pudo ni abrir:
/// quien llama tira del clasico.
pub(super) fn con_vista_previa(
    propietaria: HWND,
    lienzo: &Lienzo<'_>,
    primera: &Hoja,
    textos: &Catalogo,
) -> Result<()> {
    let (d, motor) = super::motor_propio()?;
    let pedido = Pedido {
        titulo: lienzo.nombre.clone(),
        apaisada: ex::apaisada(primera),
        opcion: opcion_que_imprimir(textos, lienzo.hay_marcos(), !lienzo.seleccion.is_empty()),
        ppp_previa: PPP_PREVIA,
    };
    moderno::mostrar(
        propietaria,
        pedido,
        d.d3d().clone(),
        motor,
        Box::new(LienzoImpreso::de(lienzo)),
    )
    .context("el dialogo moderno de imprimir no se abrio")
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::{ColorRgba, Elemento, Figura};

    fn catalogo() -> Catalogo {
        Catalogo::nuevo(pixpin_store::Idioma::Espanol)
    }

    #[test]
    fn se_pregunta_que_imprimir_solo_si_hay_donde_elegir() {
        let t = catalogo();
        // Sin marcos ni seleccion: una sola hoja posible, no se pregunta.
        assert_eq!(opcion_que_imprimir(&t, false, false), None);
        let o = opcion_que_imprimir(&t, true, false).expect("con marcos");
        let ids: Vec<&str> = o.elementos.iter().map(|(i, _)| i.as_str()).collect();
        assert_eq!(ids, [MARCOS, TODO]);
        assert_eq!(o.inicial, MARCOS, "por marcos, como el movil");
        assert!(
            o.elementos
                .iter()
                .all(|(_, r)| !r.is_empty() && !r.starts_with("imprimir-"))
        );
        let o = opcion_que_imprimir(&t, false, true).expect("con seleccion");
        let ids: Vec<&str> = o.elementos.iter().map(|(i, _)| i.as_str()).collect();
        // Caso negativo: sin marcos no se ofrece «cada marco».
        assert_eq!(ids, [TODO, SELECCION]);
        assert_eq!(o.inicial, TODO);
    }

    #[test]
    fn cada_elemento_de_la_opcion_es_su_alcance_y_lo_raro_va_por_marcos() {
        assert_eq!(alcance_de(Some(TODO)), Alcance::Todo);
        assert_eq!(alcance_de(Some(SELECCION)), Alcance::Seleccion);
        assert_eq!(alcance_de(Some(MARCOS)), Alcance::Marcos);
        assert_eq!(alcance_de(None), Alcance::Marcos);
        assert_eq!(alcance_de(Some("otra cosa")), Alcance::Marcos);
    }

    /// Dos marcos (uno apaisado) con algo dentro, y un texto fuera de ellos,
    /// sobre un lienzo de color crema.
    fn escena() -> Escena {
        let mut e = Escena::nueva();
        let crema = ColorRgba::opaco(253.0 / 255.0, 246.0 / 255.0, 227.0 / 255.0);
        assert!(e.poner_fondo(crema));
        for (y, nombre, ancho, alto) in [
            (0.0, "Planta", 300.0, 420.0),
            (600.0, "Alzado", 500.0, 260.0),
        ] {
            e.anadir(Elemento {
                figura: Figura::Marco {
                    nombre: nombre.into(),
                },
                x: 0.0,
                y,
                ancho,
                alto,
                ..Elemento::default()
            });
            e.anadir(Elemento {
                figura: Figura::Rectangulo,
                x: 30.0,
                y: y + 30.0,
                ancho: ancho - 60.0,
                alto: alto - 60.0,
                ..Elemento::default()
            });
            e.anadir(Elemento {
                figura: Figura::Elipse,
                x: 60.0,
                y: y + 60.0,
                ancho: 120.0,
                alto: 120.0,
                ..Elemento::default()
            });
        }
        e
    }

    fn miniatura(
        e: &Escena,
        papel: (f32, f32),
        alcance: Option<&str>,
        i: usize,
    ) -> (usize, (u32, u32, Vec<u8>)) {
        let lienzo = Lienzo {
            escena: e,
            seleccion: &[],
            papel: None,
            fotos: &|_| None,
            nombre: "casa".into(),
        };
        let (d, motor) = super::super::motor_propio().expect("GPU");
        moderno::miniatura_rgba(
            Box::new(LienzoImpreso::de(&lienzo)),
            motor,
            d.d3d().clone(),
            papel,
            alcance,
            i,
            400.0,
            PPP_PREVIA,
        )
        .expect("miniatura")
    }

    fn pixel(img: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 4] {
        let i = ((y * img.0 + x) * 4) as usize;
        [img.2[i], img.2[i + 1], img.2[i + 2], img.2[i + 3]]
    }

    fn tinta(img: &(u32, u32, Vec<u8>)) -> usize {
        img.2
            .chunks_exact(4)
            .filter(|p| p[0] < 128 && p[1] < 128 && p[2] < 128)
            .count()
    }

    /// **La vista previa ensena las hojas que salen**: por marcos, una por
    /// marco en su orden; todo, una. Sobre papel blanco y sin el crema del
    /// lienzo (como imprime el movil), con el dibujo dentro del margen.
    /// Deja las miniaturas en PNG para mirarlas.
    #[test]
    fn la_vista_previa_ensena_cada_hoja_tal_como_saldra_en_el_papel() {
        let dir = std::env::var_os("PIXPIN_MUESTRAS_IMPRIMIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("pixpin-imprimir-{}", std::process::id()))
            });
        std::fs::create_dir_all(&dir).expect("carpeta");
        let e = escena();
        let a4 = moderno::A4;
        let apaisado = (a4.1, a4.0);
        let casos = [
            ("marcos-1-a4", a4, Some(MARCOS), 0, 2),
            ("marcos-2-a4", a4, Some(MARCOS), 1, 2),
            ("marcos-2-a4-apaisado", apaisado, Some(MARCOS), 1, 2),
            ("todo-a4", a4, Some(TODO), 0, 1),
            ("todo-a3", (1122.5, 1587.4), Some(TODO), 0, 1),
        ];
        for (nombre, papel, alcance, i, total) in casos {
            let (n, img) = miniatura(&e, papel, alcance, i);
            assert_eq!(n, total, "{nombre}");
            // 400 DIP de miniatura a 144 ppp: 600 pixeles de ancho.
            assert_eq!(img.0, 600, "{nombre}");
            let png = pixpin_codec::codificar_png(&ImagenRgba {
                ancho: img.0,
                alto: img.1,
                pixeles: img.2.clone(),
            })
            .expect("png");
            std::fs::write(dir.join(format!("{nombre}.png")), png).expect("escribir");
            // El papel es blanco: ni el crema del lienzo ni transparente.
            for (x, y) in [(2, 2), (img.0 - 3, img.1 - 3)] {
                assert_eq!(
                    pixel(&img, x, y),
                    [255, 255, 255, 255],
                    "{nombre} ({x},{y})"
                );
            }
            // El margen de 28 pt (a esta escala, unos 28 px) queda limpio.
            let m = (28.0 / 72.0 * 96.0 * img.0 as f32 / papel.0) as u32 - 2;
            for x in 0..img.0 {
                assert_eq!(
                    pixel(&img, x, m)[0],
                    255,
                    "{nombre}: tinta en el margen de arriba"
                );
            }
            assert!(tinta(&img) > 500, "{nombre}: la hoja salio en blanco");
            // El numero de pagina: en el margen de abajo, solo por marcos.
            let abajo = (img.1 - m..img.1 - 2)
                .flat_map(|y| (0..img.0).map(move |x| (x, y)))
                .filter(|&(x, y)| pixel(&img, x, y)[0] < 200)
                .count();
            if alcance == Some(MARCOS) {
                assert!(abajo > 8, "{nombre}: sin numero de pagina ({abajo})");
            } else {
                assert_eq!(abajo, 0, "{nombre}: una hoja sola no se numera");
            }
        }
        // Caso negativo: una hoja que no existe se ve en blanco del todo.
        let (_, vacia) = miniatura(&e, a4, Some(TODO), 3);
        assert_eq!(tinta(&vacia), 0);
        eprintln!("muestras en {}", dir.display());
    }
}
