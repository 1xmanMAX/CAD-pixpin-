//! **La pantalla de inicio**: lo que se ve al abrir CAD-pixpin sin archivo.
//! No es el chat de PixPin: tres tarjetas para empezar (plano nuevo, abrir
//! un plano, abrir un modelo BIM), los recientes y las ordenes de siempre.

use std::path::PathBuf;

use pixpin_render::Pintor;

use crate::autoguardado::{self, Recuperable};
use crate::recientes;
use crate::tema;
use crate::ui::{self, Accion, Zona, rect};

/// Las ordenes que se ensenan en la tarjeta de atajos.
const ATAJOS: &[(&str, &str)] = &[
    ("L", "Linea"),
    ("PL", "Polilinea"),
    ("MU", "Muro con espesor"),
    ("EJ", "Malla de ejes"),
    ("PU / VE", "Puerta / ventana"),
    ("COT", "Cota alineada"),
    ("O", "Desfase"),
    ("TR / EX", "Recortar / alargar"),
    ("F", "Empalme"),
    ("M / CO", "Mover / copiar"),
    ("LA", "Lamina con membrete"),
    ("Z / ZE", "Zoom / ver todo"),
    ("Ctrl C / V", "Copiar / pegar"),
    ("Ctrl Z", "Deshacer"),
];

/// Pinta el inicio en una ventana de `w` x `h` (escala `e`) y da sus zonas.
pub fn pintar(
    p: &Pintor,
    w: f32,
    h: f32,
    e: f32,
    recientes: &[PathBuf],
    recuperables: &[Recuperable],
    encima: Option<(f32, f32)>,
) -> Vec<Zona> {
    let mut zonas = Vec::new();
    p.rellenar(rect(0.0, 0.0, w, h), tema::FONDO);
    let margen = (48.0 * e).min(w * 0.06);
    let ancho = (w - margen * 2.0).min(1180.0 * e);
    let x0 = (w - ancho) / 2.0;
    let mut y = (56.0 * e).min(h * 0.08);

    // La cabecera: el nombre y lo que es.
    p.rellenar_redondeado(rect(x0, y, 40.0 * e, 40.0 * e), 10.0 * e, tema::ACENTO);
    icono(p, '+', x0, y, 40.0 * e, e);
    p.texto(
        "CAD-pixpin",
        x0 + 54.0 * e,
        y - 2.0 * e,
        26.0 * e,
        tema::TEXTO,
    );
    p.texto(
        "Planos DWG y DXF y modelos BIM: ver, editar y anotar, sin AutoCAD ni Revit",
        x0 + 54.0 * e,
        y + 28.0 * e,
        13.0 * e,
        tema::TEXTO_SUAVE,
    );
    y += 80.0 * e;

    // Las tres tarjetas.
    let sep = 16.0 * e;
    let tw = (ancho - sep * 2.0) / 3.0;
    let th = 132.0 * e;
    let tarjetas = [
        (
            '+',
            "Plano nuevo",
            "En milimetros, con las capas de arquitectura, estructuras e instalaciones",
            Accion::PlanoNuevo,
            tema::ACENTO,
        ),
        (
            'p',
            "Abrir plano",
            "DWG (R13 a 2018+) o DXF. Se edita y se guarda en el mismo formato",
            Accion::AbrirPlano,
            tema::VERDE,
        ),
        (
            'b',
            "Abrir modelo BIM",
            "IFC, Revit (.rvt), LandXML o puntos: en 3D, con niveles y metrados",
            Accion::AbrirModelo,
            tema::NARANJA,
        ),
    ];
    for (i, (simbolo, titulo, texto, accion, color)) in tarjetas.into_iter().enumerate() {
        let r = rect(x0 + i as f32 * (tw + sep), y, tw, th);
        let sobre = encima.is_some_and(|(x, yy)| ui::dentro(&r, x, yy));
        p.rellenar_redondeado(
            r,
            12.0 * e,
            if sobre { tema::PANEL_ALTO } else { tema::PANEL },
        );
        if sobre {
            p.trazar(r, 1.5 * e, color);
        }
        p.rellenar_redondeado(
            rect(r.x + 18.0 * e, r.y + 18.0 * e, 36.0 * e, 36.0 * e),
            9.0 * e,
            color,
        );
        icono(p, simbolo, r.x + 18.0 * e, r.y + 18.0 * e, 36.0 * e, e);
        p.texto(
            titulo,
            r.x + 18.0 * e,
            r.y + 66.0 * e,
            17.0 * e,
            tema::TEXTO,
        );
        p.texto_linea(
            texto,
            r.x + 18.0 * e,
            r.y + 92.0 * e,
            12.0 * e,
            r.ancho - 36.0 * e,
            tema::TEXTO_SUAVE,
        );
        zonas.push(Zona {
            r,
            accion,
            ayuda: String::new(),
        });
    }
    y += th + 10.0 * e;
    let enlace = "Metrado de un modelo BIM a CSV (por nivel y partida)…";
    let ancho_enlace = p.medir_texto(enlace, 12.0 * e).0 + 24.0 * e;
    ui::boton(
        p,
        &mut zonas,
        rect(
            x0 + 2.0 * tw + 2.0 * sep + tw - ancho_enlace,
            y,
            ancho_enlace,
            26.0 * e,
        ),
        enlace,
        12.0 * e,
        ui::Estilo::Plano,
        encima,
        Accion::MetradoBim,
        "Volumen, areas y cantidad de cada partida del IFC o Revit",
    );
    y += 44.0 * e;

    if !recuperables.is_empty() {
        y = pintar_recuperar(p, x0, y, ancho, e, recuperables, encima, &mut zonas);
    }

    // Recientes a la izquierda, atajos a la derecha.
    let ancho_atajos = (ancho * 0.34).max(260.0 * e);
    let ancho_rec = ancho - ancho_atajos - sep;
    p.texto("Recientes", x0, y, 15.0 * e, tema::TEXTO);
    p.texto(
        "Ordenes de siempre",
        x0 + ancho_rec + sep,
        y,
        15.0 * e,
        tema::TEXTO,
    );
    y += 28.0 * e;
    let alto_fila = 46.0 * e;
    let alto_lista = (h - y - 32.0 * e).max(alto_fila);
    p.rellenar_redondeado(rect(x0, y, ancho_rec, alto_lista), 12.0 * e, tema::PANEL);
    if recientes.is_empty() {
        p.texto(
            "Aun no hay nada: los planos y modelos que abras saldran aqui.",
            x0 + 18.0 * e,
            y + 16.0 * e,
            12.5 * e,
            tema::TEXTO_APAGADO,
        );
    }
    let filas = ((alto_lista - 8.0 * e) / alto_fila).floor().max(0.0) as usize;
    for (i, ruta) in recientes.iter().take(filas).enumerate() {
        let r = rect(
            x0 + 6.0 * e,
            y + 4.0 * e + i as f32 * alto_fila,
            ancho_rec - 12.0 * e,
            alto_fila - 2.0 * e,
        );
        if encima.is_some_and(|(x, yy)| ui::dentro(&r, x, yy)) {
            p.rellenar_redondeado(r, 8.0 * e, tema::PANEL_ALTO);
        }
        let tipo = recientes::tipo(ruta);
        let color = match tipo {
            "DWG" | "DXF" => tema::VERDE,
            "IFC" | "RVT" => tema::NARANJA,
            _ => tema::TEXTO_SUAVE,
        };
        let pastilla = rect(
            r.x + 10.0 * e,
            r.y + (r.alto - 20.0 * e) / 2.0,
            40.0 * e,
            20.0 * e,
        );
        p.rellenar_redondeado(pastilla, 5.0 * e, tema::rgba(0x3A, 0x3A, 0x42, 1.0));
        let (pw, ph) = p.medir_texto(tipo, 10.5 * e);
        p.texto(
            tipo,
            pastilla.x + (pastilla.ancho - pw) / 2.0,
            pastilla.y + (pastilla.alto - ph) / 2.0,
            10.5 * e,
            color,
        );
        let nombre = ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let carpeta = ruta
            .parent()
            .map(|c| c.display().to_string())
            .unwrap_or_default();
        let ancho_texto = r.ancho - 70.0 * e;
        p.texto(
            &ui::recortar(p, &nombre, 13.0 * e, ancho_texto),
            r.x + 62.0 * e,
            r.y + 5.0 * e,
            13.0 * e,
            tema::TEXTO,
        );
        p.texto(
            &ui::recortar(p, &carpeta, 11.0 * e, ancho_texto),
            r.x + 62.0 * e,
            r.y + 24.0 * e,
            11.0 * e,
            tema::TEXTO_APAGADO,
        );
        zonas.push(Zona {
            r,
            accion: Accion::AbrirReciente(ruta.clone()),
            ayuda: ruta.display().to_string(),
        });
    }

    let xa = x0 + ancho_rec + sep;
    p.rellenar_redondeado(rect(xa, y, ancho_atajos, alto_lista), 12.0 * e, tema::PANEL);
    let mut ya = y + 14.0 * e;
    for (tecla, que) in ATAJOS {
        if ya + 22.0 * e > y + alto_lista {
            break;
        }
        let (kw, kh) = p.medir_texto(tecla, 11.5 * e);
        p.rellenar_redondeado(
            rect(xa + 16.0 * e, ya, kw + 12.0 * e, kh + 6.0 * e),
            5.0 * e,
            tema::PANEL_ALTO,
        );
        p.texto(tecla, xa + 22.0 * e, ya + 3.0 * e, 11.5 * e, tema::ACENTO);
        p.texto(
            que,
            xa + 104.0 * e,
            ya + 2.0 * e,
            12.5 * e,
            tema::TEXTO_SUAVE,
        );
        ya += 25.0 * e;
    }
    p.texto(
        "Escribe la orden en cualquier momento y pulsa Enter o Espacio, como en AutoCAD.",
        x0,
        h - 24.0 * e,
        11.0 * e,
        tema::TEXTO_APAGADO,
    );
    zonas
}

/// **Lo que se puede recuperar del autoguardado** (como mucho tres), con
/// Recuperar y Descartar. Da donde sigue lo de abajo.
#[allow(clippy::too_many_arguments)]
fn pintar_recuperar(
    p: &Pintor,
    x0: f32,
    mut y: f32,
    ancho: f32,
    e: f32,
    recuperables: &[Recuperable],
    encima: Option<(f32, f32)>,
    zonas: &mut Vec<Zona>,
) -> f32 {
    p.texto(
        "Recuperar: planos con cambios que no se guardaron",
        x0,
        y,
        15.0 * e,
        tema::NARANJA,
    );
    y += 28.0 * e;
    let ahora = std::time::SystemTime::now();
    for r in recuperables.iter().take(3) {
        let fila = rect(x0, y, ancho, 40.0 * e);
        p.rellenar_redondeado(fila, 10.0 * e, tema::PANEL);
        p.trazar(fila, 1.0 * e, tema::NARANJA);
        let hace = autoguardado::hace(ahora.duration_since(r.cuando).unwrap_or_default());
        let donde = r
            .original
            .as_ref()
            .and_then(|o| o.parent())
            .map(|c| c.display().to_string())
            .unwrap_or_else(|| "sin guardar nunca".into());
        let ancho_botones = 210.0 * e;
        p.texto(
            &ui::recortar(p, &r.nombre(), 13.0 * e, ancho - ancho_botones - 220.0 * e),
            fila.x + 16.0 * e,
            fila.y + 11.0 * e,
            13.0 * e,
            tema::TEXTO,
        );
        p.texto(
            &ui::recortar(p, &format!("copia {hace}  ·  {donde}"), 11.0 * e, 200.0 * e),
            fila.x + ancho - ancho_botones - 210.0 * e,
            fila.y + 13.0 * e,
            11.0 * e,
            tema::TEXTO_APAGADO,
        );
        let b = rect(
            fila.x + ancho - ancho_botones,
            fila.y + 6.0 * e,
            100.0 * e,
            28.0 * e,
        );
        ui::boton(
            p,
            zonas,
            b,
            "Recuperar",
            12.0 * e,
            ui::Estilo::Principal,
            encima,
            Accion::Recuperar(r.copia.clone()),
            "Abrir la copia del autoguardado (luego Ctrl+S la guarda en su sitio)",
        );
        ui::boton(
            p,
            zonas,
            rect(b.x + 106.0 * e, b.y, 94.0 * e, b.alto),
            "Descartar",
            12.0 * e,
            ui::Estilo::Plano,
            encima,
            Accion::DescartarCopia(r.copia.clone()),
            "Borrar esta copia del autoguardado",
        );
        y += 46.0 * e;
    }
    y + 10.0 * e
}

/// **Un icono dibujado con rayas** (no con un caracter: no todas las letras
/// del equipo traen esos simbolos y salian cuadrados): `+` nuevo, `p` un
/// plano (hoja con su esquina doblada y una cota), `b` un modelo (un cubo).
fn icono(p: &Pintor, simbolo: char, x: f32, y: f32, lado: f32, e: f32) {
    let c = tema::TEXTO;
    let g = 2.0 * e;
    let (cx, cy, m) = (x + lado / 2.0, y + lado / 2.0, lado * 0.26);
    match simbolo {
        '+' => {
            p.linea((cx - m, cy), (cx + m, cy), g, c);
            p.linea((cx, cy - m), (cx, cy + m), g, c);
        }
        'p' => {
            let (x0, y0, x1, y1) = (cx - m, cy - m * 1.2, cx + m, cy + m * 1.2);
            let d = m * 0.6;
            p.polilinea(
                &[
                    (x0, y0),
                    (x1 - d, y0),
                    (x1, y0 + d),
                    (x1, y1),
                    (x0, y1),
                    (x0, y0),
                ],
                g,
                c,
            );
            p.linea(
                (x0 + m * 0.4, cy + m * 0.5),
                (x1 - m * 0.4, cy + m * 0.5),
                g * 0.7,
                c,
            );
        }
        _ => {
            // Un cubo en perspectiva.
            let (a, b) = (m * 0.9, m * 0.5);
            let arriba = [
                (cx, cy - a),
                (cx + a, cy - a + b),
                (cx, cy - a + 2.0 * b),
                (cx - a, cy - a + b),
            ];
            p.polilinea(
                &[arriba[0], arriba[1], arriba[2], arriba[3], arriba[0]],
                g,
                c,
            );
            p.linea(arriba[3], (cx - a, cy + a - b), g, c);
            p.linea(arriba[1], (cx + a, cy + a - b), g, c);
            p.linea(arriba[2], (cx, cy + a), g, c);
            p.polilinea(
                &[(cx - a, cy + a - b), (cx, cy + a), (cx + a, cy + a - b)],
                g,
                c,
            );
        }
    }
}
