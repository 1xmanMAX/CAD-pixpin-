//! **Una hoja como SVG**, escrita desde las mismas ordenes que pinta la
//! pantalla.
//!
//! `pintado.rs` lo prometia desde el principio: «las mismas ordenes valen
//! para Direct2D hoy y para exportar a SVG manana sin tocar una linea de
//! geometria». Esto es ese manana. No hay un segundo dibujante que pueda
//! discrepar del primero: el garabato de un rectangulo, el contorno de un
//! trazo a mano y la punta de una flecha son exactamente los que se ven, y
//! lo unico que se decide aqui es como se escribe cada orden en SVG.
//!
//! El documento sigue el formato del movil (`Svg.documento`): `width` y
//! `height` en pixeles de escena, `viewBox` con el origen de la hoja y un
//! rectangulo de fondo del color del papel si no es transparente. Asi el
//! visor de la pagina web (que lee el `viewBox`) sirve igual para los SVG de
//! los dos aparatos.

use crate::elemento::ColorRgba;
use crate::exportar::{self, Hoja};
use crate::pintado::Orden;
use crate::vector::Punto2;
use std::fmt::Write as _;

/// Una imagen ya codificada, lista para ir dentro del SVG como `data:`.
pub struct Incrustada {
    /// `image/png`, `image/jpeg`...
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

/// Como se escribe el SVG.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpcionesSvg {
    /// El papel. `None` es transparente.
    pub fondo: Option<ColorRgba>,
    /// Los marcos a la vista, en su grupo `g.marcos`: solo para la pagina web
    /// del lienzo entero, que los usa para imprimir marco a marco.
    pub marcos: bool,
}

/// Un numero como lo escribe el movil (`Svg.num`): dos decimales, y los
/// enteros sin `.0`, que es la mitad de los numeros de un camino.
pub fn num(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let r = (v as f64 * 100.0).round() / 100.0;
    if r == r.floor() && r.abs() < 1e15 {
        return format!("{}", r as i64);
    }
    let s = format!("{r:.2}");
    s.trim_end_matches('0').to_string()
}

/// Lo que no puede ir tal cual dentro de un atributo o de un texto.
pub fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pintura(c: ColorRgba, atributo: &str) -> String {
    let mut s = format!("{atributo}=\"{}\"", exportar::hex(c));
    if c.a < 0.999 {
        let _ = write!(s, " {atributo}-opacity=\"{}\"", num(c.a.clamp(0.0, 1.0)));
    }
    s
}

fn poligonal(puntos: &[Punto2], cerrar: bool) -> String {
    let mut d = String::with_capacity(puntos.len() * 12);
    for (i, p) in puntos.iter().enumerate() {
        d.push(if i == 0 { 'M' } else { 'L' });
        let _ = write!(d, "{} {}", num(p.x), num(p.y));
    }
    if cerrar && !puntos.is_empty() {
        d.push('Z');
    }
    d
}

fn de_tinta(contorno: &[Punto2]) -> String {
    let Some((inicio, curvas)) = exportar::curvas_de_tinta(contorno) else {
        return String::new();
    };
    let mut d = format!("M{} {}", num(inicio.x), num(inicio.y));
    for (c, f) in curvas {
        let _ = write!(d, "Q{} {} {} {}", num(c.x), num(c.y), num(f.x), num(f.y));
    }
    let _ = write!(d, "L{} {}Z", num(inicio.x), num(inicio.y));
    d
}

/// Una orden como elemento SVG. `imagenes` resuelve las `Orden::Imagen`.
fn orden(
    o: &Orden,
    caja: (f32, f32, f32, f32),
    imagenes: &dyn Fn(u64) -> Option<Incrustada>,
    s: &mut String,
) {
    match o {
        Orden::Poligono { puntos, color } | Orden::Relleno { puntos, color } => {
            if puntos.len() >= 3 {
                let _ = writeln!(
                    s,
                    "<path d=\"{}\" {}/>",
                    poligonal(puntos, true),
                    pintura(*color, "fill")
                );
            }
        }
        // Regla *winding* (la de SVG por omision), como la pantalla: un
        // trazo que se cruza consigo mismo no deja agujeros.
        Orden::Tinta { contorno, color } => {
            if contorno.len() >= 3 {
                let _ = writeln!(
                    s,
                    "<path d=\"{}\" {}/>",
                    de_tinta(contorno),
                    pintura(*color, "fill")
                );
            }
        }
        Orden::Polilinea {
            puntos,
            color,
            grosor,
            estilo,
        } => {
            if puntos.len() < 2 {
                return;
            }
            let mut extra = String::new();
            if let Some((raya, hueco)) = exportar::rayas(*estilo, *grosor) {
                let _ = write!(extra, " stroke-dasharray=\"{} {}\"", num(raya), num(hueco));
            }
            let _ = writeln!(
                s,
                "<path d=\"{}\" fill=\"none\" {} stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{extra}/>",
                poligonal(puntos, false),
                pintura(*color, "stroke"),
                num(*grosor)
            );
        }
        // El foco: todo oscuro menos el hueco. Una sola ruta con el marco y el
        // hueco, rellena par-impar, que es como se agujerea en SVG.
        Orden::Velo { hueco, color } => {
            let (x0, y0, x1, y1) = caja;
            let marco = [
                Punto2::nuevo(x0, y0),
                Punto2::nuevo(x1, y0),
                Punto2::nuevo(x1, y1),
                Punto2::nuevo(x0, y1),
            ];
            let _ = writeln!(
                s,
                "<path d=\"{}{}\" fill-rule=\"evenodd\" {}/>",
                poligonal(&marco, true),
                poligonal(hueco, true),
                pintura(*color, "fill")
            );
        }
        Orden::Texto {
            texto,
            x,
            y,
            tam,
            familia,
            color,
            ancho_max,
            negrita,
            cursiva,
        } => {
            let lineas = exportar::partir_texto(texto, *tam, *ancho_max);
            // Las del catalogo van con su interlineado fijo y su linea base,
            // como en pantalla (`pixpin_render::letras`); las del sistema,
            // con los numeros de Segoe UI de siempre.
            let (base, paso) = match crate::texto::fuente_por_nombre(familia) {
                Some(f) => (f.linea_base() * tam, f.interlineado * tam),
                None => (tam * exportar::LINEA_BASE, tam * exportar::INTERLINEA),
            };
            let familia = if familia.trim().is_empty() {
                String::new()
            } else {
                format!("{}, ", escapar(familia))
            };
            let peso = if *negrita {
                " font-weight=\"bold\""
            } else {
                ""
            };
            let estilo = if *cursiva {
                " font-style=\"italic\""
            } else {
                ""
            };
            let _ = write!(
                s,
                "<text x=\"{}\" y=\"{}\" font-family=\"{familia}Segoe UI, Helvetica, Arial, sans-serif\" font-size=\"{}\"{peso}{estilo} {} xml:space=\"preserve\">",
                num(*x),
                num(*y + base),
                num(*tam),
                pintura(*color, "fill")
            );
            for (i, l) in lineas.iter().enumerate() {
                if i == 0 {
                    let _ = write!(s, "<tspan x=\"{}\">{}</tspan>", num(*x), escapar(l));
                } else {
                    let _ = write!(
                        s,
                        "<tspan x=\"{}\" dy=\"{}\">{}</tspan>",
                        num(*x),
                        num(paso),
                        escapar(l)
                    );
                }
            }
            s.push_str("</text>\n");
        }
        // El numero de una cota: girado con su raya y con halo. El halo es
        // el trazo de las letras con `paint-order="stroke"`, que lo pinta
        // debajo del relleno como el movil (primero el halo, luego la letra).
        Orden::Rotulo {
            texto,
            x,
            y,
            tam,
            familia,
            color,
            halo,
            grosor_halo,
            centro,
            angulo,
        } => {
            let base = match crate::texto::fuente_por_nombre(familia) {
                Some(f) => f.linea_base() * tam,
                None => tam * exportar::LINEA_BASE,
            };
            let familia = if familia.trim().is_empty() {
                String::new()
            } else {
                format!("{}, ", escapar(familia))
            };
            let _ = writeln!(
                s,
                "<g transform=\"rotate({} {} {})\"><text x=\"{}\" y=\"{}\" font-family=\"{familia}Segoe UI, Helvetica, Arial, sans-serif\" font-size=\"{}\" {} {} stroke-width=\"{}\" stroke-linejoin=\"round\" paint-order=\"stroke\" xml:space=\"preserve\">{}</text></g>",
                num(angulo.to_degrees()),
                num(centro.x),
                num(centro.y),
                num(*x),
                num(*y + base),
                num(*tam),
                pintura(*color, "fill"),
                pintura(*halo, "stroke"),
                num(*grosor_halo),
                escapar(texto)
            );
        }
        Orden::Imagen {
            id_objeto,
            x,
            y,
            ancho,
            alto,
            opacidad,
            recorte,
            angulo,
        } => {
            // Una imagen que no se encuentra no rompe el documento: se queda
            // su hueco, como en pantalla.
            let Some(img) = imagenes(*id_objeto) else {
                return;
            };
            let op = if *opacidad < 0.999 {
                format!(" opacity=\"{}\"", num(opacidad.clamp(0.0, 1.0)))
            } else {
                String::new()
            };
            // Girada, dentro de un `<g>` que la gira alrededor del centro de
            // su caja, como el movil. En un grupo y no en la propia etiqueta:
            // un `<svg>` anidado (la recortada) no admite `transform` en SVG 1.1.
            let girada = angulo.abs() > 1e-6;
            if girada {
                let _ = write!(
                    s,
                    "<g transform=\"rotate({} {} {})\">",
                    num(angulo.to_degrees()),
                    num(*x + *ancho / 2.0),
                    num(*y + *alto / 2.0)
                );
            }
            let cierre = if girada { "</g>\n" } else { "" };
            // **Recortada**: un `<svg>` anidado cuyo `viewBox` es el trozo, en
            // pixeles del original, y dentro la imagen entera a su tamano
            // natural. El `<svg>` anidado recorta solo lo que se sale, asi que
            // no hace falta ni un `clipPath` ni saber cuantos pixeles trae la
            // copia incrustada: el trozo sale exacto sea la que sea.
            let trozo = recorte.and_then(|r| {
                r.trozo_en(r.ancho_natural, r.alto_natural)
                    .map(|t| (t, r.ancho_natural, r.alto_natural))
            });
            if let Some(((tx0, ty0, tx1, ty1), nw, nh)) = trozo {
                let _ = writeln!(
                    s,
                    "<svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"none\"><image x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"{op} href=\"data:{};base64,{}\"/></svg>",
                    num(*x),
                    num(*y),
                    num(*ancho),
                    num(*alto),
                    num(tx0),
                    num(ty0),
                    num(tx1 - tx0),
                    num(ty1 - ty0),
                    num(nw),
                    num(nh),
                    img.mime,
                    exportar::base64(&img.bytes)
                );
                s.push_str(cierre);
                return;
            }
            let _ = writeln!(
                s,
                "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"{op} href=\"data:{};base64,{}\"/>",
                num(*x),
                num(*y),
                num(*ancho),
                num(*alto),
                img.mime,
                exportar::base64(&img.bytes)
            );
            s.push_str(cierre);
        }
    }
}

/// El nombre de la tela de un grano: sale de lo que la hace distinta
/// (material, color, paso, inclinacion) y no de un contador. Una pagina web
/// mete varios SVG en el mismo documento, y dos `id` iguales con dibujos
/// distintos harian que un trazo cogiera la tela de otro; con el nombre
/// sacado del contenido, dos iguales son la misma tela y da igual cual gane.
fn nombre_de_tela(g: &crate::pintado::Grano) -> String {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "grano-{:?}-{:02x}{:02x}{:02x}{:02x}-{}-{}",
        g.material,
        c(g.color.r),
        c(g.color.g),
        c(g.color.b),
        c(g.color.a),
        (g.paso * 100.0).round() as i64,
        u8::from(g.inclinada)
    )
    .to_lowercase()
}

/// **El grano de una tinta**: su tela como `<pattern>` y la silueta del trazo
/// rellena con ella, encima del cuerpo que ya se escribio.
///
/// Es lo que hace la pantalla (`Pintor::grano`) con otras palabras: la tela
/// es la misma imagen de 16x16, se repite clavada al origen del documento
/// —`patternUnits="userSpaceOnUse"`, sin correrla a la caja del trazo— y
/// gira con la misma matriz, asi que las rayas caen donde caen en pantalla.
/// Recortar al trazo lo hace el propio relleno: la tela solo se ve dentro
/// de la silueta.
fn grano(
    g: &crate::pintado::Grano,
    contorno: &[Punto2],
    telas: &mut std::collections::HashSet<String>,
    s: &mut String,
) {
    if contorno.len() < 3 || exportar::tela_vacia(g) || !(g.paso.is_finite() && g.paso > 0.0) {
        return;
    }
    let id = nombre_de_tela(g);
    if telas.insert(id.clone()) {
        let (lado, rgba) = exportar::tela_rgba(g);
        let [a, b, c, d] = exportar::matriz_de_la_tela(g);
        let _ = writeln!(
            s,
            "<defs><pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{lado}\" height=\"{lado}\" patternTransform=\"matrix({} {} {} {} 0 0)\"><image width=\"{lado}\" height=\"{lado}\" href=\"data:image/png;base64,{}\"/></pattern></defs>",
            num4(a),
            num4(b),
            num4(c),
            num4(d),
            exportar::base64(&exportar::png_rgba(lado, lado, &rgba))
        );
    }
    let _ = writeln!(
        s,
        "<path d=\"{}\" fill=\"url(#{id})\"/>",
        de_tinta(contorno)
    );
}

/// Un numero con cuatro decimales: la matriz de la tela es un giro por una
/// escala pequena, y con dos decimales el coseno de 45 grados torceria las
/// rayas a lo largo de un plano grande.
fn num4(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// **El grafito viaja como lo que es: su mapa de casillas** (`DrawSvg.grafito`
/// del movil). Como raya lisa dejaba de ser grafito. Va como imagen sin
/// perdida y con `image-rendering:pixelated`: al ampliar en el navegador se
/// ven las casillas, igual que en el lienzo. Solo el trozo con algo pintado:
/// el resto de la caja es aire y engordaria el fichero.
fn grafito(g: &crate::tinta::grafito::GrafitoSuelto, s: &mut String) {
    use crate::tinta::grafito;
    let m = &g.mapa;
    let Some(trozo) = grafito::lo_pintado(m) else {
        return;
    };
    let (tx, ty, tw, th) = trozo;
    let f = m.finura;
    let (x, y) = (m.x0 + tx as f32 / f, m.y0 + ty as f32 / f);
    let opacidad = g.opacidad.clamp(0.0, 1.0);
    let op = if opacidad < 1.0 {
        format!(" opacity=\"{}\"", num(opacidad))
    } else {
        String::new()
    };
    // Girado alrededor del centro de la figura, como en pantalla.
    let giro = if m.angulo != 0.0 {
        format!(
            " transform=\"rotate({} {} {})\"",
            num(m.angulo.to_degrees()),
            num(m.centro.x),
            num(m.centro.y)
        )
    } else {
        String::new()
    };
    let _ = writeln!(
        s,
        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" style=\"image-rendering:pixelated\"{op}{giro} href=\"data:image/png;base64,{}\"/>",
        num(x),
        num(y),
        num(tw as f32 / f),
        num(th as f32 / f),
        exportar::base64(&grafito::png_del_trozo(m, trozo))
    );
}

/// La hoja entera como documento SVG.
pub fn svg(
    hoja: &Hoja,
    opciones: OpcionesSvg,
    imagenes: &dyn Fn(u64) -> Option<Incrustada>,
) -> String {
    let (x0, y0, _, _) = hoja.caja;
    let (w, h) = (hoja.ancho(), hoja.alto());
    let mut s = String::with_capacity(4096 + hoja.ordenes.len() * 200);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\">",
        num(w),
        num(h),
        num(x0),
        num(y0),
        num(w),
        num(h)
    );
    if let Some(f) = opciones.fondo {
        let _ = writeln!(
            s,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {}/>",
            num(x0),
            num(y0),
            num(w),
            num(h),
            pintura(f, "fill")
        );
    }
    let mut telas = std::collections::HashSet::new();
    // El grafito, cada mapa justo antes de la orden que le toca.
    let mut grafitos = hoja.grafitos.iter().peekable();
    for (i, o) in hoja.ordenes.iter().enumerate() {
        while let Some(g) = grafitos.next_if(|g| g.antes_de <= i) {
            grafito(g, &mut s);
        }
        orden(o, hoja.caja, imagenes, &mut s);
        if let (Some(g), Orden::Tinta { contorno, .. }) = (exportar::grano_de_la_orden(hoja, i), o)
        {
            grano(&g, contorno, &mut telas, &mut s);
        }
    }
    // Lo que va despues de la ultima orden.
    for g in grafitos {
        grafito(g, &mut s);
    }
    if opciones.marcos && !hoja.marcos.is_empty() {
        // Igual que el movil (`DrawSvg.aTexto`, `marcosComoPaginas`): el visor
        // de la pagina web busca `g.marcos rect.marco` para imprimir marco a
        // marco, y su hoja de estilo los esconde en el papel.
        s.push_str("<g class=\"marcos\" fill=\"none\" stroke=\"#9a9aa6\" stroke-width=\"2\">\n");
        for m in &hoja.marcos {
            let (a, b, c, d) = m.caja;
            let _ = writeln!(
                s,
                "<rect class=\"marco\" data-n=\"{}\" data-nombre=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\"/>",
                m.n,
                escapar(&m.nombre),
                num(a),
                num(b),
                num(c - a),
                num(d - b)
            );
            let _ = writeln!(
                s,
                "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-family=\"sans-serif\" font-size=\"13\" fill=\"#9a9aa6\" stroke=\"none\">{}</text>",
                num(c - 10.0),
                num(d - 10.0),
                m.n
            );
        }
        s.push_str("</g>\n");
    }
    s.push_str("</svg>\n");
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::EstiloTrazo;
    use crate::exportar::MarcoALaVista;

    fn hoja(ordenes: Vec<Orden>) -> Hoja {
        Hoja {
            nombre: String::new(),
            caja: (-10.0, -10.0, 110.0, 60.0),
            ordenes,
            marcos: Vec::new(),
            granos: Vec::new(),
            grafitos: Vec::new(),
        }
    }

    fn negro() -> ColorRgba {
        ColorRgba::opaco(0.0, 0.0, 0.0)
    }

    fn ninguna(_: u64) -> Option<Incrustada> {
        None
    }

    #[test]
    fn los_numeros_se_escriben_como_en_el_movil() {
        assert_eq!(num(3.0), "3");
        assert_eq!(num(-10.0), "-10");
        assert_eq!(num(1.256), "1.26");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(f32::NAN), "0");
    }

    #[test]
    fn una_hoja_sale_con_su_viewbox_y_su_papel() {
        let s = svg(
            &hoja(Vec::new()),
            OpcionesSvg {
                fondo: Some(ColorRgba::opaco(1.0, 1.0, 1.0)),
                marcos: false,
            },
            &ninguna,
        );
        let esperado = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"120\" height=\"70\" viewBox=\"-10 -10 120 70\">\n\
<rect x=\"-10\" y=\"-10\" width=\"120\" height=\"70\" fill=\"#ffffff\"/>\n\
</svg>\n";
        assert_eq!(s, esperado);
    }

    #[test]
    fn transparente_no_lleva_rectangulo_de_fondo() {
        let s = svg(&hoja(Vec::new()), OpcionesSvg::default(), &ninguna);
        assert!(!s.contains("<rect"));
    }

    #[test]
    fn cada_orden_se_escribe_con_su_primitiva() {
        let p = |x: f32, y: f32| Punto2::nuevo(x, y);
        let s = svg(
            &hoja(vec![
                Orden::Relleno {
                    puntos: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
                    color: ColorRgba { a: 0.5, ..negro() },
                },
                Orden::Tinta {
                    contorno: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)],
                    color: negro(),
                },
                Orden::Polilinea {
                    puntos: vec![p(0.0, 0.0), p(50.0, 50.0)],
                    color: negro(),
                    grosor: 2.0,
                    estilo: EstiloTrazo::Discontinuo,
                },
                Orden::Texto {
                    texto: "a < b\nsegunda".into(),
                    x: 5.0,
                    y: 5.0,
                    tam: 20.0,
                    familia: String::new(),
                    color: negro(),
                    ancho_max: 500.0,
                    negrita: false,
                    cursiva: false,
                },
            ]),
            OpcionesSvg::default(),
            &ninguna,
        );
        assert!(s.contains("<path d=\"M0 0L10 0L10 10Z\" fill=\"#000000\" fill-opacity=\"0.5\"/>"));
        assert!(s.contains("<path d=\"M0 0Q0 0 2 0Q4 0 4 2Q4 4 2 2L0 0Z\" fill=\"#000000\"/>"));
        assert!(s.contains("stroke-dasharray=\"8 10\""));
        assert!(s.contains("a &lt; b</tspan>"), "el texto va escapado: {s}");
        assert!(s.contains("dy=\"26.6\">segunda</tspan>"));
    }

    #[test]
    fn el_numero_de_una_cota_va_girado_y_con_el_halo_debajo_de_la_letra() {
        let mut s = String::new();
        orden(
            &Orden::Rotulo {
                texto: "5,00 cm".into(),
                x: 10.0,
                y: 20.0,
                tam: 20.0,
                familia: String::new(),
                color: ColorRgba::opaco(0.0, 0.0, 0.0),
                halo: ColorRgba::opaco(1.0, 1.0, 1.0),
                grosor_halo: 4.4,
                centro: Punto2::nuevo(50.0, 30.0),
                angulo: std::f32::consts::FRAC_PI_2,
            },
            (0.0, 0.0, 100.0, 100.0),
            &|_| None,
            &mut s,
        );
        assert!(s.contains("transform=\"rotate(90 50 30)\""), "{s}");
        assert!(s.contains("stroke=\"#ffffff\""), "el halo: {s}");
        assert!(s.contains("stroke-width=\"4.4\""), "{s}");
        assert!(
            s.contains("paint-order=\"stroke\""),
            "debajo de la letra: {s}"
        );
        assert!(s.contains(">5,00 cm</text>"), "{s}");
    }

    #[test]
    fn una_imagen_que_no_se_encuentra_deja_su_hueco_y_no_rompe_nada() {
        let o = Orden::Imagen {
            id_objeto: 7,
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
            opacidad: 1.0,
            recorte: None,
            angulo: 0.0,
        };
        let sin = svg(&hoja(vec![o.clone()]), OpcionesSvg::default(), &ninguna);
        assert!(!sin.contains("<image"));
        let con = svg(&hoja(vec![o]), OpcionesSvg::default(), &|id| {
            (id == 7).then(|| Incrustada {
                mime: "image/png",
                bytes: b"foobar".to_vec(),
            })
        });
        assert!(con.contains("href=\"data:image/png;base64,Zm9vYmFy\""));
    }

    #[test]
    fn una_imagen_girada_sale_girada_alrededor_de_su_centro_y_una_derecha_sin_grupo() {
        let img = |_: u64| {
            Some(Incrustada {
                mime: "image/png",
                bytes: b"x".to_vec(),
            })
        };
        let orden = |angulo: f32| Orden::Imagen {
            id_objeto: 7,
            x: 10.0,
            y: 20.0,
            ancho: 40.0,
            alto: 20.0,
            opacidad: 1.0,
            recorte: None,
            angulo,
        };
        let girada = svg(
            &hoja(vec![orden(std::f32::consts::FRAC_PI_2)]),
            OpcionesSvg::default(),
            &img,
        );
        assert!(
            girada.contains("<g transform=\"rotate(90 30 30)\"><image"),
            "{girada}"
        );
        assert!(
            girada.contains(
                "/>
</g>"
            ),
            "el grupo se cierra: {girada}"
        );
        let derecha = svg(&hoja(vec![orden(0.0)]), OpcionesSvg::default(), &img);
        assert!(!derecha.contains("rotate("), "{derecha}");
    }

    #[test]
    fn los_marcos_solo_salen_cuando_se_piden() {
        let mut h = hoja(Vec::new());
        h.marcos = vec![MarcoALaVista {
            n: 1,
            nombre: "Planta \"A\"".into(),
            caja: (0.0, 0.0, 50.0, 40.0),
        }];
        let sin = svg(&h, OpcionesSvg::default(), &ninguna);
        assert!(!sin.contains("class=\"marcos\""));
        let con = svg(
            &h,
            OpcionesSvg {
                fondo: None,
                marcos: true,
            },
            &ninguna,
        );
        assert!(con.contains("<rect class=\"marco\" data-n=\"1\" data-nombre=\"Planta &quot;A&quot;\" x=\"0\" y=\"0\" width=\"50\" height=\"40\" rx=\"8\"/>"));
    }

    fn rayado(x: f32) -> crate::Elemento {
        crate::Elemento {
            figura: crate::Figura::Lapiz {
                puntos: (0..20)
                    .map(|i| Punto2::nuevo(x + i as f32 * 5.0, 10.0))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            grosor: 6.0,
            material: crate::tinta::MaterialTinta::Rayado,
            ..crate::Elemento::default()
        }
    }

    #[test]
    fn el_grano_sale_como_una_tela_clavada_al_documento_y_solo_una_vez() {
        let mut e = crate::Escena::nueva();
        e.anadir(rayado(0.0));
        e.anadir(rayado(0.0));
        let h = &exportar::hojas(&e, exportar::Alcance::Todo, &[], None)[0];
        let s = svg(h, OpcionesSvg::default(), &ninguna);
        // Dos trazos iguales, una sola tela y dos rellenos con ella.
        assert_eq!(s.matches("<pattern ").count(), 1, "{s}");
        assert_eq!(s.matches("fill=\"url(#grano-rayado-").count(), 2);
        assert!(s.contains("patternUnits=\"userSpaceOnUse\""));
        assert!(s.contains("href=\"data:image/png;base64,iVBORw0KGgo"));
        // Girada 45 grados: el coseno y el seno iguales.
        let m = s.split("matrix(").nth(1).expect("matriz");
        let v: Vec<f32> = m
            .split(' ')
            .take(4)
            .map(|x| x.parse().expect("numero"))
            .collect();
        assert!((v[0] - v[1]).abs() < 1e-3 && (v[0] - v[3]).abs() < 1e-3 && v[2] < 0.0);
        // Caso negativo: la tinta lisa no lleva tela.
        let mut lisa = crate::Escena::nueva();
        lisa.anadir(crate::Elemento {
            material: crate::tinta::MaterialTinta::Lisa,
            ..rayado(0.0)
        });
        let h2 = &exportar::hojas(&lisa, exportar::Alcance::Todo, &[], None)[0];
        assert!(!svg(h2, OpcionesSvg::default(), &ninguna).contains("<pattern"));
    }

    #[test]
    fn una_imagen_recortada_ensena_solo_su_trozo() {
        let o = Orden::Imagen {
            id_objeto: 7,
            x: 10.0,
            y: 20.0,
            ancho: 30.0,
            alto: 30.0,
            opacidad: 1.0,
            recorte: Some(crate::RecorteImagen {
                x: 50.0,
                y: 0.0,
                ancho: 50.0,
                alto: 50.0,
                ancho_natural: 100.0,
                alto_natural: 100.0,
            }),
            angulo: 0.0,
        };
        let img = |_: u64| {
            Some(Incrustada {
                mime: "image/png",
                bytes: b"x".to_vec(),
            })
        };
        let s = svg(&hoja(vec![o.clone()]), OpcionesSvg::default(), &img);
        assert!(
            s.contains("<svg x=\"10\" y=\"20\" width=\"30\" height=\"30\" viewBox=\"50 0 50 50\" preserveAspectRatio=\"none\"><image x=\"0\" y=\"0\" width=\"100\" height=\"100\""),
            "{s}"
        );
        // Caso negativo: un recorte roto pinta la imagen entera en su caja.
        let Orden::Imagen { recorte, .. } = &o else {
            unreachable!()
        };
        let roto = Orden::Imagen {
            id_objeto: 7,
            x: 10.0,
            y: 20.0,
            ancho: 30.0,
            alto: 30.0,
            opacidad: 1.0,
            recorte: recorte.map(|r| crate::RecorteImagen {
                ancho_natural: 0.0,
                ..r
            }),
            angulo: 0.0,
        };
        let s2 = svg(&hoja(vec![roto]), OpcionesSvg::default(), &img);
        assert!(
            s2.contains("<image x=\"10\" y=\"20\" width=\"30\" height=\"30\""),
            "{s2}"
        );
    }

    #[test]
    fn el_grafito_sale_como_su_mapa_de_casillas_y_no_como_raya_lisa() {
        use crate::elemento::{Elemento, Figura};
        use crate::tinta::MaterialTinta;
        let mut escena = crate::escena::Escena::nueva();
        let raya = |material| Elemento {
            figura: Figura::Lapiz {
                puntos: (0..60)
                    .map(|i| Punto2::nuevo(i as f32 * 3.0, 40.0 + (i % 7) as f32))
                    .collect(),
                presiones: Vec::new(),
                opciones: Some(Default::default()),
            },
            grosor: 2.0,
            trazo: negro(),
            material,
            ..Default::default()
        };
        escena.anadir(raya(MaterialTinta::Cuadritos));
        let hojas = exportar::hojas(&escena, exportar::Alcance::Todo, &[], None);
        assert_eq!(hojas[0].grafitos.len(), 1);
        let s = svg(&hojas[0], OpcionesSvg::default(), &ninguna);
        assert!(s.contains("style=\"image-rendering:pixelated\""), "{s}");
        assert!(s.contains("href=\"data:image/png;base64,"));
        // Sin la raya lisa: el grafito no deja una silueta de tinta.
        assert!(!s.contains("<path"), "salio tambien la raya lisa");
        // Y comprimido: una raya de 180 unidades no son megas.
        assert!(s.len() < 60_000, "pesa {}", s.len());

        // Caso negativo: el mismo trazo de tinta lisa sigue siendo un camino.
        let mut lisa = crate::escena::Escena::nueva();
        lisa.anadir(raya(MaterialTinta::Lisa));
        let hojas = exportar::hojas(&lisa, exportar::Alcance::Todo, &[], None);
        assert!(hojas[0].grafitos.is_empty());
        let s = svg(&hojas[0], OpcionesSvg::default(), &ninguna);
        assert!(s.contains("<path") && !s.contains("pixelated"));
    }
}
