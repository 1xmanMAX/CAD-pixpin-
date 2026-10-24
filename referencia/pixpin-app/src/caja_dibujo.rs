//! Como se pinta la barra de herramientas del lienzo (D53), la misma en
//! todos los anfitriones de las herramientas de dibujo: el lienzo, el
//! lector, la paleta del pin y el anotador de pantalla (ver `dibujo`). La
//! geometria la da `pixpin-ui` y aqui solo se traduce a rectangulos, colores
//! y letras. Quien la pinta en una ventana que no empieza donde empieza su
//! geometria corre el pintor antes (`Pintor::desplazar`).

use pixpin_geom::Punto;
use pixpin_render::{Color, Pintor, RectF};
use pixpin_ui::{BotonCaja, CajaHerramientas, Herramienta};

/// El icono de Excalidraw de cada boton. Donde Excalidraw no tiene la
/// herramienta (resaltador, foco, cotas), el suyo mas parecido.
fn icono(b: BotonCaja) -> &'static pixpin_render::icono::Icono {
    use pixpin_render::iconos_excalidraw as i;
    match b {
        // En PixPin la mano ELIGE y mueve: es la Seleccion de Excalidraw,
        // no su mano de desplazar el lienzo.
        BotonCaja::Elegir(Herramienta::Mano) => &i::SELECTION_ICON,
        BotonCaja::Elegir(Herramienta::Lapiz) => &i::FREEDRAW_ICON,
        // Excalidraw no tiene grafito: el lapiz con tres cuadritos en su raya.
        BotonCaja::Elegir(Herramienta::Grafito) => &pixpin_render::grafito::ICONO,
        BotonCaja::Elegir(Herramienta::Resaltador) => &i::PEN_MODE_ICON,
        BotonCaja::Elegir(Herramienta::Linea) => &i::LINE_ICON,
        BotonCaja::Elegir(Herramienta::Flecha) => &i::ARROW_ICON,
        BotonCaja::Elegir(Herramienta::Rectangulo) => &i::RECTANGLE_ICON,
        BotonCaja::Elegir(Herramienta::Elipse) => &i::ELLIPSE_ICON,
        BotonCaja::Elegir(Herramienta::Texto) => &i::TEXT_ICON,
        BotonCaja::Elegir(Herramienta::Foco) => &i::PRESENTATION_ICON,
        BotonCaja::Elegir(Herramienta::Lupa) => &i::SEARCH_ICON,
        BotonCaja::Elegir(Herramienta::Borrador) => &i::ERASER_ICON,
        BotonCaja::Elegir(Herramienta::Cota) => &i::LINE_EDITOR_ICON,
        BotonCaja::Elegir(Herramienta::Escalar) => &i::RESIZE_ICON,
        BotonCaja::Elegir(Herramienta::EscalaGrafica) => &i::GRID_ICON,
        BotonCaja::Elegir(Herramienta::Marco) => &MARCO,
        // La caja de dibujo no ofrece el emoji (es del universo, que tendra
        // su propia caja); si alguna vez llega, se ve como texto.
        BotonCaja::Elegir(Herramienta::Emoji) => &i::TEXT_ICON,
        // Las que abre la tanda cero. Los iconos ya estaban descargados de
        // Excalidraw —el rombo, el lazo, la flecha de codos, el bote, el
        // cuentagotas— y no se usaban todavia: aqui se les da su boton para
        // que el dia que la caja los ofrezca no haya que volver por aqui.
        BotonCaja::Elegir(Herramienta::Rombo) => &i::DIAMOND_ICON,
        // El arco, la esquina redonda (un cuarto de vuelta); y la marca de
        // angulo, el angulo de Excalidraw, que es justo lo que dibuja.
        BotonCaja::Elegir(Herramienta::Arco) => &i::EDGE_ROUND_ICON,
        BotonCaja::Elegir(Herramienta::Angulo) => &i::ANGLE_ICON,
        BotonCaja::Elegir(Herramienta::FlechaCodos) => &i::ELBOW_ARROW_ICON,
        BotonCaja::Elegir(Herramienta::FlechaLibre) => &i::ROUND_ARROW_ICON,
        BotonCaja::Elegir(Herramienta::Lazo) => &i::LASSO_ICON,
        // El ojo cerrado y no una rejilla: lo que un mosaico hace es dejar
        // de enseñar, y la rejilla ya es la barra de escala.
        BotonCaja::Elegir(Herramienta::Mosaico) => &i::EYE_CLOSED_ICON,
        BotonCaja::Elegir(Herramienta::Serie) => &i::ABACUS_ICON,
        BotonCaja::Elegir(Herramienta::Relleno) => &i::BUCKET_FILL_ICON,
        BotonCaja::Elegir(Herramienta::Recortar) => &i::CUT_ICON,
        BotonCaja::Elegir(Herramienta::Extender) => &i::ARROW_RIGHT_ICON,
        BotonCaja::Elegir(Herramienta::Punto) => &i::DOTS_ICON,
        BotonCaja::Elegir(Herramienta::CopiarEstilo) => &i::EYE_DROPPER_ICON,
        // F8 y F14: los de Excalidraw para recortar, el laser, la imagen y
        // la biblioteca.
        BotonCaja::Elegir(Herramienta::Zona) => &i::CROP_ICON,
        BotonCaja::Elegir(Herramienta::Laser) => &i::LASER_POINTER_TOOL_ICON,
        // Soldar vertices: la chincheta de Excalidraw, que es un clavo.
        BotonCaja::Elegir(Herramienta::Nudo) => &i::PIN_ICON,
        // La bolita elige: el icono de elegir todo de Excalidraw.
        BotonCaja::Elegir(Herramienta::Bolita) => &i::SELECT_ALL_ICON,
        BotonCaja::Imagen => &i::IMAGE_ICON,
        BotonCaja::Figuras => &i::LIBRARY_ICON,
        BotonCaja::Imprimir => &IMPRESORA,
        // El `IosShare` del movil: el cuadro con la flecha que sale.
        BotonCaja::Compartir => &i::SHARE_IOS,
        // La mano, como el boton de atravesar de la pastilla del anotador.
        BotonCaja::Atravesar => &i::HAND_ICON,
        BotonCaja::Deshacer => &i::UNDO_ICON,
        BotonCaja::Rehacer => &i::REDO_ICON,
        BotonCaja::Color => &i::PALETTE,
        BotonCaja::Salir => &i::CLOSE_ICON,
        // Un grupo se pinta con su cara (`pintar_barra`); esto es solo si
        // alguna vez no tiene ninguna: los tres puntos de «mas».
        BotonCaja::Grupo(_) => &i::DOTS_ICON,
    }
}

/// Un color de la hoja de estilos de Excalidraw, `#rrggbb`.
pub(crate) const fn hex(rgb: u32) -> Color {
    Color {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

/// Los colores de la barra, del tema claro u oscuro de Excalidraw
/// (`docs/excalidraw/interfaz.md` §2.1 y §5.1, y la entrada de menu).
pub(crate) struct TemaBarra {
    pub isla: Color,
    pub icono: Color,
    pub hover: Color,
    pub activo_fondo: Color,
    pub activo_icono: Color,
    pub tecla: Color,
    pub separador: Color,
    /// La entrada de menu de la herramienta puesta: `--color-primary-light`.
    pub menu_elegida: Color,
    /// Su icono: `--color-primary-darker`.
    pub menu_icono_elegido: Color,
}

pub(crate) const CLARO: TemaBarra = TemaBarra {
    isla: hex(0xffffff),
    icono: hex(0x1b1b1f),
    hover: hex(0xf1f0ff),
    activo_fondo: hex(0xe0dfff),
    activo_icono: hex(0x030064),
    tecla: hex(0xb8b8b8),
    separador: hex(0xf1f0ff),
    menu_elegida: hex(0xe3e2fe),
    menu_icono_elegido: hex(0x5b57d1),
};

pub(crate) const OSCURO: TemaBarra = TemaBarra {
    isla: hex(0x232329),
    icono: hex(0xe3e3e8),
    hover: hex(0x2e2d39),
    activo_fondo: hex(0x403e6a),
    activo_icono: hex(0xe0dfff),
    tecla: hex(0x7a7a7a),
    separador: hex(0x2e2d39),
    menu_elegida: hex(0x4f4d6f),
    menu_icono_elegido: hex(0xb2aeff),
};

/// El tema de ahora: el oscuro sobre papel de noche (`dibujo::tema`).
pub(crate) fn tema_barra() -> &'static TemaBarra {
    if crate::dibujo::tema::de_noche() {
        &OSCURO
    } else {
        &CLARO
    }
}

/// La sombra de isla de Excalidraw (`--shadow-island`) aproximada con capas
/// redondeadas: tres sombras de CSS no existen en Direct2D, y un desenfoque
/// de verdad costaria un efecto por fotograma para algo casi invisible.
pub(crate) fn sombra_isla(p: &Pintor, r: RectF, radio: f32, e: f32) {
    let capa = |crece: f32, baja: f32, alfa: f32| {
        p.rellenar_redondeado(
            RectF {
                x: r.x - crece,
                y: r.y - crece + baja,
                ancho: r.ancho + 2.0 * crece,
                alto: r.alto + 2.0 * crece,
            },
            radio + crece,
            Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: alfa,
            },
        )
    };
    // 0 7px 14px .05: tres anillos que se van apagando hacia abajo.
    capa(6.0 * e, 7.0 * e, 0.012);
    capa(4.0 * e, 6.0 * e, 0.016);
    capa(2.0 * e, 4.0 * e, 0.02);
    // 0 0 3px .08 y 0 0 1px .17: el contorno fino que marca la isla.
    capa(1.5 * e, 0.0, 0.035);
    capa(0.75 * e, 0.0, 0.12);
}

fn rf(r: pixpin_geom::Rect) -> RectF {
    RectF {
        x: r.x as f32,
        y: r.y as f32,
        ancho: r.ancho as f32,
        alto: r.alto as f32,
    }
}

/// Lo que ensena un boton de la barra: el mismo, o la cara si es un grupo.
fn cara_de(barra: &CajaHerramientas, b: BotonCaja, activa: Herramienta) -> BotonCaja {
    match b {
        BotonCaja::Grupo(g) => crate::dibujo::grupos::cara(barra, g, activa).unwrap_or(b),
        otro => otro,
    }
}

/// La barra de herramientas de arriba del editor, con el aspecto de
/// Excalidraw: isla blanca (o la oscura sobre papel de noche), iconos
/// oscuros, fondo lila al pasar el raton y en la herramienta activa,
/// separadores entre grupos y la tecla del atajo en la esquina de cada boton
/// que la tiene.
///
/// Un grupo se pinta con su cara (la herramienta puesta del grupo, o la
/// ultima usada) y un piquito debajo que dice que tiene hermanas; si esta
/// desplegado (`barra.con_desplegado`), sus hermanas salen debajo en una
/// isla con su nombre y su tecla, como el desplegable de Excalidraw.
pub fn pintar_barra(
    p: &Pintor,
    barra: &CajaHerramientas,
    activa: Herramienta,
    escala_por_cien: u32,
    raton: Option<Punto>,
    tecla: impl Fn(BotonCaja) -> Option<char>,
) {
    let t = tema_barra();
    let e = escala_por_cien as f32 / 100.0;
    let marco = rf(barra.marco);
    sombra_isla(p, marco, 8.0 * e, e);
    p.rellenar_redondeado(marco, 8.0 * e, t.isla);

    for s in barra.separadores() {
        p.rellenar(rf(s), t.separador);
    }

    let sobre = raton.and_then(|r| barra.boton_en(r));
    for (i, boton) in barra.botones().iter().enumerate() {
        let caja = rf(barra.rect_de(i));
        let cara = cara_de(barra, *boton, activa);
        let elegido = cara == BotonCaja::Elegir(activa);
        let abierto = matches!(boton, BotonCaja::Grupo(g) if barra.desplegado() == Some(*g));
        if elegido || abierto {
            p.rellenar_redondeado(caja, 8.0 * e, t.activo_fondo);
        } else if sobre == Some(*boton) {
            p.rellenar_redondeado(caja, 8.0 * e, t.hover);
        }
        let color = if elegido || abierto {
            t.activo_icono
        } else {
            t.icono
        };
        let lado = 16.0 * e;
        p.icono(
            icono(cara),
            RectF {
                x: caja.x + (caja.ancho - lado) / 2.0,
                y: caja.y + (caja.alto - lado) / 2.0,
                ancho: lado,
                alto: lado,
            },
            color,
        );
        // El piquito de los grupos con hermanas: sin el, nada dice que ese
        // boton esconde mas herramientas (en el movil lo dice el toque).
        if let BotonCaja::Grupo(g) = boton
            && barra.miembros(*g).len() > 1
        {
            let (cx, y) = (caja.x + caja.ancho / 2.0, caja.y + caja.alto - 6.0 * e);
            let (w, h) = (3.0 * e, 2.5 * e);
            p.poligono(&[(cx - w, y), (cx + w, y), (cx, y + h)], color);
        }
        if let Some(c) = tecla(cara) {
            let texto = c.to_string();
            let tam = 10.0 * e;
            let (w, h) = p.medir_texto(&texto, tam);
            p.texto(
                &texto,
                caja.x + caja.ancho - 4.0 * e - w,
                caja.y + caja.alto - 2.0 * e - h,
                tam,
                if elegido || abierto {
                    t.activo_icono
                } else {
                    t.tecla
                },
            );
        }
    }

    // Las hermanas del grupo abierto, en su isla bajo el boton.
    if let Some(menu) = barra.menu() {
        let isla = rf(menu.marco);
        sombra_isla(p, isla, 8.0 * e, e);
        p.rellenar_redondeado(isla, 8.0 * e, t.isla);
        let textos = crate::ventana_editor::exportar::textos();
        for (b, r) in &menu.filas {
            let fila = rf(*r);
            let elegida = *b == BotonCaja::Elegir(activa);
            if elegida {
                p.rellenar_redondeado(fila, 6.0 * e, t.menu_elegida);
            } else if sobre == Some(*b) {
                p.rellenar_redondeado(fila, 6.0 * e, t.hover);
            }
            let lado = 16.0 * e;
            let x_icono = fila.x + 8.0 * e;
            p.icono(
                icono(*b),
                RectF {
                    x: x_icono,
                    y: fila.y + (fila.alto - lado) / 2.0,
                    ancho: lado,
                    alto: lado,
                },
                if elegida {
                    t.menu_icono_elegido
                } else {
                    t.icono
                },
            );
            let nombre = crate::dibujo::permitidas::nombre_visible(*b, textos);
            let tam = 14.0 * e;
            let (_, h) = p.medir_texto(&nombre, tam);
            p.texto(
                &nombre,
                x_icono + lado + 10.0 * e,
                fila.y + (fila.alto - h) / 2.0,
                tam,
                t.icono,
            );
            // La tecla a la derecha, a media tinta: `opacity .5` del menu.
            if let Some(c) = tecla(*b) {
                let texto = c.to_string();
                let tam = 12.0 * e;
                let (w, h) = p.medir_texto(&texto, tam);
                p.texto(
                    &texto,
                    fila.x + fila.ancho - 8.0 * e - w,
                    fila.y + (fila.alto - h) / 2.0,
                    tam,
                    t.tecla,
                );
            }
        }
    }
}

/// El icono del marco: «frame» de Tabler Icons (MIT, ver
/// THIRD-PARTY-NOTICES.md). Los de Excalidraw se generan y no se editan a
/// mano, y su juego no trae este.
const MARCO: pixpin_render::icono::Icono = pixpin_render::icono::Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[pixpin_render::icono::TrazoIcono {
        d: "M4 7l16 0M4 17l16 0M7 4l0 16M17 4l0 16",
        relleno: pixpin_render::icono::Pintura::Nada,
        trazo: pixpin_render::icono::Pintura::Actual,
        grosor: 1.75,
        extremo_redondo: true,
        union_redonda: true,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }],
};

/// **La impresora** (F11): «printer» de Tabler Icons (MIT, ver
/// THIRD-PARTY-NOTICES.md), del mismo juego que el marco. Excalidraw no
/// imprime y su juego no la trae.
const IMPRESORA: pixpin_render::icono::Icono = pixpin_render::icono::Icono {
    vista: (0.0, 0.0, 24.0, 24.0),
    trazos: &[pixpin_render::icono::TrazoIcono {
        d: "M17 17h2a2 2 0 0 0 2 -2v-4a2 2 0 0 0 -2 -2h-14a2 2 0 0 0 -2 2v4a2 2 0 0 0 2 2h2\
            M17 9v-4a2 2 0 0 0 -2 -2h-6a2 2 0 0 0 -2 2v4\
            M7 15a2 2 0 0 1 2 -2h6a2 2 0 0 1 2 2v4a2 2 0 0 1 -2 2h-6a2 2 0 0 1 -2 -2z",
        relleno: pixpin_render::icono::Pintura::Nada,
        trazo: pixpin_render::icono::Pintura::Actual,
        grosor: 1.75,
        extremo_redondo: true,
        union_redonda: true,
        opacidad: 1.0,
        par_impar: false,
        matriz: None,
        mascara: None,
    }],
};

/// El globo de Excalidraw (`.excalidraw-tooltip`): fondo casi negro, letra
/// blanca de 12 px, radio 6.
const GLOBO: Color = hex(0x1b1b1f);

/// **El rotulo del boton bajo el raton**, debajo de el, como el `title` de
/// los botones de Excalidraw: para los que no tienen tecla pintada y cuyo
/// icono no se explica solo (imprimir: «Imprimir… (Ctrl+P)»). `rotulo` dice
/// el texto de cada boton, o `None` si no lleva.
pub fn pintar_pista(
    p: &Pintor,
    barra: &CajaHerramientas,
    escala_por_cien: u32,
    raton: Option<Punto>,
    rotulo: impl Fn(BotonCaja) -> Option<String>,
) {
    let Some(boton) = raton.and_then(|r| barra.boton_en(r)) else {
        return;
    };
    // Con las hermanas a la vista el globo caeria encima de ellas, que ya
    // llevan su nombre escrito.
    if barra.desplegado().is_some() {
        return;
    }
    // Un grupo dice su nombre y el de la herramienta que ensena:
    // «Formas: Rombo». `Emoji` no es de ningun grupo: pide la cara sin
    // tocar la memoria de la ultima usada.
    let texto = match boton {
        BotonCaja::Grupo(g) => {
            let cara = crate::dibujo::grupos::cara(barra, g, Herramienta::Emoji);
            match (rotulo(boton), cara.and_then(&rotulo)) {
                (Some(grupo), Some(cara)) => Some(format!("{grupo}: {cara}")),
                (grupo, cara) => grupo.or(cara),
            }
        }
        otro => rotulo(otro),
    };
    let Some(texto) = texto else {
        return;
    };
    let Some(i) = barra.botones().iter().position(|b| *b == boton) else {
        return;
    };
    let r = barra.rect_de(i);
    let e = escala_por_cien as f32 / 100.0;
    let tam = 12.0 * e;
    let (w, h) = p.medir_texto(&texto, tam);
    let (px, py) = (8.0 * e, 5.0 * e);
    let (ancho, alto) = (w + 2.0 * px, h + 2.0 * py);
    // Centrado bajo el boton, pero sin salirse de la barra por la derecha
    // (Salir va al final, y el globo de los ultimos se cortaria).
    let centro = r.x as f32 + r.ancho as f32 / 2.0;
    let derecha = (barra.marco.x + barra.marco.ancho as i32) as f32;
    let x = (centro - ancho / 2.0)
        .min(derecha - ancho)
        .max(barra.marco.x as f32);
    let y = (barra.marco.y + barra.marco.alto as i32) as f32 + 6.0 * e;
    p.rellenar_redondeado(RectF { x, y, ancho, alto }, 6.0 * e, GLOBO);
    p.texto(&texto, x + px, y + py, tam, CLARO.isla);
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Rect;
    use pixpin_ui::{BOTONES_EDITOR, GrupoBarra};

    type Imagen = (u32, u32, Vec<u8>);

    /// Pinta la barra fuera de pantalla, con el papel que se diga (de noche
    /// o no), y devuelve los pixeles.
    fn pintar_fuera(
        barra: &CajaHerramientas,
        activa: Herramienta,
        escala: u32,
        raton: Option<Punto>,
        (w, h): (u32, u32),
        noche: bool,
    ) -> Imagen {
        let d = pixpin_capture::Dispositivo::nuevo().expect("GPU");
        let motor = pixpin_render::MotorRender::nuevo(d.d3d()).expect("motor");
        let destino =
            pixpin_render::fuera_de_pantalla::FueraDePantalla::nuevo(&motor, d.d3d(), w, h)
                .expect("textura");
        let papel = if noche {
            pixpin_motor2d::ColorRgba {
                r: 0.12,
                g: 0.13,
                b: 0.15,
                a: 1.0,
            }
        } else {
            pixpin_motor2d::ColorRgba {
                r: 0.97,
                g: 0.97,
                b: 0.98,
                a: 1.0,
            }
        };
        let t = crate::ventana_editor::exportar::textos();
        crate::dibujo::tema::con_papel(Some(papel), || {
            motor
                .dibujar(&destino.destino, |p| {
                    p.limpiar(Color {
                        r: papel.r,
                        g: papel.g,
                        b: papel.b,
                        a: 1.0,
                    });
                    pintar_barra(p, barra, activa, escala, raton, |b| match b {
                        BotonCaja::Elegir(h) => crate::dibujo::teclas::tecla_de(h),
                        _ => None,
                    });
                    pintar_pista(p, barra, escala, raton, |b| {
                        crate::dibujo::permitidas::rotulo_de_boton(b, t)
                    });
                })
                .expect("pinta")
        });
        destino.leer_rgba().expect("lee")
    }

    fn guardar(img: &Imagen, nombre: &str) {
        let dir = std::env::var_os("PIXPIN_MUESTRAS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&dir).expect("carpeta");
        let png = pixpin_codec::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: img.0,
            alto: img.1,
            pixeles: img.2.clone(),
        })
        .expect("png");
        std::fs::write(dir.join(nombre), png).expect("escribe");
    }

    fn pixel(img: &Imagen, x: i32, y: i32) -> [u8; 3] {
        let k = ((y as u32 * img.0 + x as u32) * 4) as usize;
        [img.2[k], img.2[k + 1], img.2[k + 2]]
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    /// Cuantos pixeles casi negros hay en la franja bajo la barra: el globo.
    fn oscuros_bajo(img: &Imagen, barra: &CajaHerramientas) -> usize {
        let y0 = (barra.marco.abajo() + 8) as u32;
        let mut n = 0;
        for y in y0..(y0 + 14).min(img.1) {
            for x in 0..img.0 {
                let k = ((y * img.0 + x) * 4) as usize;
                if img.2[k] < 60 && img.2[k + 1] < 60 && img.2[k + 2] < 60 {
                    n += 1;
                }
            }
        }
        n
    }

    /// **Imprimir y compartir van en su grupo, con las acciones, y el globo
    /// de cada boton sale al pasar el raton** (F11: quien no sabe `Ctrl+P`
    /// no lo encontraba). Sin raton no hay globo.
    #[test]
    fn imprimir_y_compartir_estan_en_su_grupo_y_el_globo_sale_al_pasar_el_raton() {
        let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let rotulo = |b| crate::dibujo::permitidas::rotulo_de_boton(b, &t).unwrap_or_default();
        assert!(rotulo(BotonCaja::Compartir).contains("Ctrl+May"));
        assert!(rotulo(BotonCaja::Imprimir).contains("Ctrl+P"));
        assert!(rotulo(BotonCaja::Elegir(Herramienta::Lupa)).contains("figura cerrada"));
        assert!(rotulo(BotonCaja::Elegir(Herramienta::Mosaico)).starts_with("Pixelar"));
        assert!(rotulo(BotonCaja::Elegir(Herramienta::Zona)).contains("arrastra"));
        // El lector y el pin no la ofrecen (su compartir es el suyo).
        use crate::dibujo::permitidas::Anfitrion;
        assert!(Anfitrion::Lienzo.admite_boton(BotonCaja::Compartir));
        assert!(!Anfitrion::Lector.admite_boton(BotonCaja::Compartir));
        assert!(!Anfitrion::Pin.admite_boton(BotonCaja::Compartir));

        let zona = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 200,
        };
        let barra = CajaHerramientas::barra_superior(zona, 100, &BOTONES_EDITOR);
        let sacar = barra
            .rect_de_boton(BotonCaja::Grupo(GrupoBarra::Sacar))
            .unwrap();
        // Justo antes de deshacer.
        let i = barra
            .botones()
            .iter()
            .position(|b| *b == BotonCaja::Grupo(GrupoBarra::Sacar))
            .unwrap();
        assert_eq!(barra.botones()[i + 1], BotonCaja::Deshacer);
        let abierta = barra.con_desplegado(Some(GrupoBarra::Sacar));
        assert!(abierta.rect_de_boton(BotonCaja::Imprimir).is_some());
        assert!(abierta.rect_de_boton(BotonCaja::Compartir).is_some());

        let sin = pintar_fuera(&barra, Herramienta::Mano, 100, None, (1920, 120), false);
        assert_eq!(oscuros_bajo(&sin, &barra), 0, "sin raton no hay globo");
        let con = pintar_fuera(
            &barra,
            Herramienta::Mano,
            100,
            Some(centro(sacar)),
            (1920, 120),
            false,
        );
        assert!(oscuros_bajo(&con, &barra) > 500, "no salio el globo");
        guardar(&con, "barra-globo-sacar.png");
        // Caso negativo: con las hermanas abiertas el globo no sale (caeria
        // encima de ellas, que ya llevan su nombre).
        let abierto = pintar_fuera(
            &abierta,
            Herramienta::Mano,
            100,
            Some(centro(sacar)),
            (1920, 200),
            false,
        );
        let bajo_menu = abierta.menu().unwrap().marco.derecha() + 20;
        let mut negros = 0;
        for y in (barra.marco.abajo() + 8)..(barra.marco.abajo() + 22) {
            for x in bajo_menu..1920 {
                let [r, g, b] = pixel(&abierto, x, y);
                if r < 60 && g < 60 && b < 60 {
                    negros += 1;
                }
            }
        }
        assert_eq!(negros, 0);
    }

    /// **El clic a traves esta en la barra del anotador de pantalla viva**
    /// (2026-09-29: el usuario no lo encontraba; en el movil va en la misma
    /// barra que las herramientas, `CapaPantalla.kt`). Suelto, con su globo,
    /// y solo alli: ni en la pantalla congelada (debajo solo hay una foto)
    /// ni en el lienzo. Deja `barra-anotador-atravesar.png` para mirarla.
    #[test]
    fn el_clic_a_traves_sale_en_la_barra_del_anotador_vivo_y_solo_alli() {
        use crate::dibujo::permitidas::{Anfitrion, botones_con, rotulo_de_boton};
        let todas = pixpin_store::herramientas::Herramientas::default();
        let viva = botones_con(Anfitrion::PantallaViva, &todas);
        assert!(viva.contains(&BotonCaja::Atravesar));
        for otro in [
            Anfitrion::PantallaCongelada,
            Anfitrion::Lienzo,
            Anfitrion::Lector,
            Anfitrion::Pin,
        ] {
            assert!(
                !botones_con(otro, &todas).contains(&BotonCaja::Atravesar),
                "{otro:?}"
            );
        }
        let zona = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 200,
        };
        let barra = CajaHerramientas::barra_superior(
            zona,
            100,
            crate::dibujo::permitidas::botones(Anfitrion::PantallaViva),
        );
        let boton = barra
            .rect_de_boton(BotonCaja::Atravesar)
            .expect("suelto en la barra, no en un grupo");
        let t = pixpin_store::Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let pista = rotulo_de_boton(BotonCaja::Atravesar, &t).unwrap();
        assert!(pista.starts_with("Clic a través"), "{pista}");
        let img = pintar_fuera(
            &barra,
            Herramienta::Lapiz,
            100,
            Some(centro(boton)),
            (1920, 120),
            false,
        );
        assert!(oscuros_bajo(&img, &barra) > 500, "sin globo");
        guardar(&img, "barra-anotador-atravesar.png");
    }

    /// **Las muestras de la barra agrupada**: clara y oscura (papel de
    /// noche), un grupo desplegado, y en un portatil de 1366 px al 125 %. Se
    /// dejan en `PIXPIN_MUESTRAS` (o la carpeta temporal) para mirarlas.
    #[test]
    fn la_barra_agrupada_se_pinta_clara_u_oscura_y_con_un_grupo_desplegado() {
        // 1366 x 768 al 125 %: lo que queda de ancho en un portatil pequeno.
        let zona = Rect {
            x: 0,
            y: 0,
            ancho: 1366,
            alto: 400,
        };
        let barra = CajaHerramientas::barra_superior(zona, 125, &BOTONES_EDITOR);
        assert_eq!(barra.filas(), 1);
        let tam = (1366, 110);
        let clara = pintar_fuera(&barra, Herramienta::Lapiz, 125, None, tam, false);
        let oscura = pintar_fuera(&barra, Herramienta::Lapiz, 125, None, tam, true);
        guardar(&clara, "barra-1366-125-clara.png");
        guardar(&oscura, "barra-1366-125-oscura.png");
        // En el hueco entre el primer boton y el segundo se ve la isla: blanca
        // en la clara, #232329 en la oscura.
        let r0 = barra.rect_de(0);
        let (x, y) = (r0.derecha() + 2, r0.y + 2);
        assert_eq!(pixel(&clara, x, y), [0xff, 0xff, 0xff]);
        let [r, g, b] = pixel(&oscura, x, y);
        assert!(
            (r as i32 - 0x23).abs() <= 2
                && (g as i32 - 0x23).abs() <= 2
                && (b as i32 - 0x29).abs() <= 2,
            "isla oscura: {r:x} {g:x} {b:x}"
        );

        // Las formas desplegadas, con el raton sobre el rombo.
        let abierta = barra.con_desplegado(Some(GrupoBarra::Formas));
        let menu = abierta.menu().expect("abierto");
        let rombo = abierta
            .rect_de_boton(BotonCaja::Elegir(Herramienta::Rombo))
            .unwrap();
        let alto = (menu.marco.abajo() + 20) as u32;
        let tam = (1366, alto);
        let desplegada = pintar_fuera(
            &abierta,
            Herramienta::Rectangulo,
            125,
            Some(centro(rombo)),
            tam,
            false,
        );
        let desplegada_noche = pintar_fuera(
            &abierta,
            Herramienta::Rectangulo,
            125,
            Some(centro(rombo)),
            tam,
            true,
        );
        guardar(&desplegada, "barra-formas-desplegadas-clara.png");
        guardar(&desplegada_noche, "barra-formas-desplegadas-oscura.png");
        // Dentro de la isla del desplegable hay isla, y hay texto (los
        // nombres): pixeles oscuros a la derecha de los iconos.
        let m = menu.marco;
        assert_eq!(
            pixel(&desplegada, m.x + m.ancho as i32 / 2, m.abajo() - 2),
            [0xff, 0xff, 0xff]
        );
        let fila = menu.filas[1].1; // la elipse, sin raton ni elegida
        let mut letra = 0;
        for yy in fila.y..fila.abajo() {
            for xx in (fila.x + 40)..(fila.x + 120) {
                let [r, _, _] = pixel(&desplegada, xx, yy);
                if r < 100 {
                    letra += 1;
                }
            }
        }
        assert!(letra > 20, "sin nombre en la fila: {letra}");
    }
}
