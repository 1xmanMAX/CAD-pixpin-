//! Las pruebas del exportador: que el `.docx` es un paquete sano (ZIP, XML
//! bien formado con sus espacios de nombres declarados, relaciones que
//! apuntan a partes que estan, tipos para todas) y que el propio lector de
//! Word de PixPin lo vuelve a leer con el mismo texto, tablas e imagenes.

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::documento::{Clase, texto_y_tramos};
use crate::md_comentarios::{Comentarios, Hilo, Respuesta};
use crate::{Paquete, docx};

// ---------------------------------------------------------------------------
// Un comprobador de XML estricto (el lector del crate es tolerante a
// proposito; este no perdona nada de lo que Word no perdona).

fn nombre_valido(n: &str) -> bool {
    let mut c = n.chars();
    matches!(c.next(), Some(x) if x.is_alphabetic() || x == '_' || x == ':')
        && c.all(|x| x.is_alphanumeric() || matches!(x, '_' | ':' | '.' | '-'))
}

fn letra_valida(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || ((c as u32) >= 0x20 && c != '\u{FFFE}' && c != '\u{FFFF}')
}

fn entidades_bien(t: &str) -> Result<(), String> {
    let mut resto = t;
    while let Some(i) = resto.find('&') {
        let tras = &resto[i + 1..];
        let fin = tras
            .find(';')
            .ok_or_else(|| format!("& sin cerrar en {t:?}"))?;
        let e = &tras[..fin];
        let bien = matches!(e, "amp" | "lt" | "gt" | "quot" | "apos")
            || (e.starts_with("#x") && u32::from_str_radix(&e[2..], 16).is_ok())
            || (e.starts_with('#') && e[1..].parse::<u32>().is_ok());
        if !bien {
            return Err(format!("entidad desconocida &{e};"));
        }
        resto = &tras[fin + 1..];
    }
    if let Some(c) = t.chars().find(|c| !letra_valida(*c)) {
        return Err(format!("letra prohibida en XML: U+{:04X}", c as u32));
    }
    Ok(())
}

/// **XML bien formado**, con los prefijos declarados y sin atributos
/// repetidos. `Err` dice que falla.
pub(super) fn bien_formado(xml: &str) -> Result<(), String> {
    let mut s = xml;
    if let Some(r) = s.strip_prefix("<?xml") {
        s = &r[r.find("?>").ok_or("declaracion sin cerrar")? + 2..];
    }
    let mut pila: Vec<(String, BTreeSet<String>)> = Vec::new();
    let mut raices = 0;
    let declarado = |pila: &Vec<(String, BTreeSet<String>)>, prefijo: &str| {
        prefijo == "xml" || prefijo == "xmlns" || pila.iter().any(|(_, ps)| ps.contains(prefijo))
    };
    while !s.is_empty() {
        let Some(i) = s.find('<') else {
            if !s.trim().is_empty() && pila.is_empty() {
                return Err("texto fuera de la raiz".into());
            }
            entidades_bien(s)?;
            break;
        };
        let texto = &s[..i];
        if pila.is_empty() && !texto.trim().is_empty() {
            return Err(format!("texto fuera de la raiz: {texto:?}"));
        }
        entidades_bien(texto)?;
        s = &s[i..];
        if let Some(r) = s.strip_prefix("<!--") {
            s = &r[r.find("-->").ok_or("comentario sin cerrar")? + 3..];
            continue;
        }
        if s.starts_with("<!") || s.starts_with("<?") {
            return Err("DOCTYPE, CDATA o PI donde no tocan".into());
        }
        let fin = s.find('>').ok_or("etiqueta sin cerrar")?;
        let etiqueta = &s[1..fin];
        s = &s[fin + 1..];
        if let Some(n) = etiqueta.strip_prefix('/') {
            let (abierta, _) = pila.pop().ok_or_else(|| format!("cierre de mas: </{n}>"))?;
            if abierta != n.trim() {
                return Err(format!("<{abierta}> cerrada con </{n}>"));
            }
            continue;
        }
        let sola = etiqueta.ends_with('/');
        let cuerpo = etiqueta.strip_suffix('/').unwrap_or(etiqueta);
        let nombre: String = cuerpo.chars().take_while(|c| !c.is_whitespace()).collect();
        if !nombre_valido(&nombre) {
            return Err(format!("nombre de elemento invalido: {nombre:?}"));
        }
        if pila.is_empty() {
            raices += 1;
            if raices > 1 {
                return Err("dos raices".into());
            }
        }
        // Atributos.
        let mut resto = cuerpo[nombre.len()..].trim_start();
        let mut vistos = BTreeSet::new();
        let mut prefijos = BTreeSet::new();
        let mut usados: Vec<String> = Vec::new();
        while !resto.is_empty() {
            let igual = resto
                .find('=')
                .ok_or_else(|| format!("atributo sin valor en <{nombre}>"))?;
            let an = resto[..igual].trim();
            if !nombre_valido(an) {
                return Err(format!("atributo invalido {an:?} en <{nombre}>"));
            }
            if !vistos.insert(an.to_string()) {
                return Err(format!("atributo repetido {an} en <{nombre}>"));
            }
            let tras = resto[igual + 1..].trim_start();
            let comilla = tras.chars().next().ok_or("valor vacio")?;
            if comilla != '"' && comilla != '\'' {
                return Err(format!("valor sin comillas en <{nombre}>"));
            }
            let cierre = tras[1..].find(comilla).ok_or("comillas sin cerrar")? + 1;
            let valor = &tras[1..cierre];
            if valor.contains('<') {
                return Err(format!("< dentro de un atributo de <{nombre}>"));
            }
            entidades_bien(valor)?;
            if let Some(p) = an.strip_prefix("xmlns:") {
                prefijos.insert(p.to_string());
            } else if an.contains(':') {
                usados.push(an.split(':').next().unwrap().to_string());
            }
            resto = tras[cierre + 1..].trim_start();
        }
        if nombre.contains(':') {
            usados.push(nombre.split(':').next().unwrap().to_string());
        }
        pila.push((nombre.clone(), prefijos));
        for p in usados {
            if !declarado(&pila, &p) {
                return Err(format!("prefijo sin declarar {p}: en <{nombre}>"));
            }
        }
        if sola {
            pila.pop();
        }
    }
    if let Some((n, _)) = pila.last() {
        return Err(format!("<{n}> sin cerrar"));
    }
    if raices != 1 {
        return Err("sin raiz".into());
    }
    Ok(())
}

#[test]
fn el_comprobador_de_xml_pilla_lo_que_word_no_perdona() {
    assert!(
        bien_formado(r#"<?xml version="1.0"?><a xmlns:w="x"><w:b c="1">t &amp; u</w:b></a>"#)
            .is_ok()
    );
    // Casos negativos: cada uno es un Word que no abre.
    assert!(bien_formado("<a><b></a></b>").is_err(), "cruzadas");
    assert!(bien_formado("<a>x & y</a>").is_err(), "& suelto");
    assert!(
        bien_formado(r#"<a b="1" b="2"/>"#).is_err(),
        "atributo repetido"
    );
    assert!(bien_formado("<w:a/>").is_err(), "prefijo sin declarar");
    assert!(bien_formado("<a>\u{1}</a>").is_err(), "letra de control");
    assert!(bien_formado("<a/><b/>").is_err(), "dos raices");
    assert!(bien_formado("<a>").is_err(), "sin cerrar");
}

// ---------------------------------------------------------------------------
// Fotos de juguete: PNG de verdad (con su CRC y su zlib sin comprimir), que
// Word y el lector pintan.

fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for b in datos {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

pub(super) fn png(ancho: u32, alto: u32, color: [u8; 3]) -> Vec<u8> {
    let mut crudo = Vec::new();
    for y in 0..alto {
        crudo.push(0u8);
        for x in 0..ancho {
            // Un marco oscuro para que se vea donde acaba.
            let borde = x < 3 || y < 3 || x + 3 >= ancho || y + 3 >= alto;
            crudo.extend_from_slice(if borde { &[40, 40, 40] } else { &color });
        }
    }
    let mut z = vec![0x78, 0x01];
    for (i, trozo) in crudo.chunks(65_535).enumerate() {
        let ultimo = (i + 1) * 65_535 >= crudo.len();
        z.push(ultimo as u8);
        z.extend_from_slice(&(trozo.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(trozo.len() as u16)).to_le_bytes());
        z.extend_from_slice(trozo);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for x in &crudo {
        a = (a + *x as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut s = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut trozo = |tipo: &[u8], datos: &[u8]| {
        s.extend_from_slice(&(datos.len() as u32).to_be_bytes());
        let mut c = tipo.to_vec();
        c.extend_from_slice(datos);
        s.extend_from_slice(&c);
        s.extend_from_slice(&crc32(&c).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&ancho.to_be_bytes());
    ihdr.extend_from_slice(&alto.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    trozo(b"IHDR", &ihdr);
    trozo(b"IDAT", &z);
    trozo(b"IEND", &[]);
    s
}

// ---------------------------------------------------------------------------
// La nota con todo

pub(super) const NOTA: &str = r#"# Informe de obra «Casa Lima»

Texto con **negrita**, *cursiva*, ~~tachado~~, `codigo()` y la formula $x^2+1$; especiales: <a> & "b" 'c' ✓ 😀.
Un enlace a [la web](https://example.com/a?b=1&c=2), a la hoja [Planta baja](pixpin:hoja=P1/H1) y a [un mensaje](pixpin:mensaje=P1/K7Q2).

## Listas

- uno
  - uno punto uno
    - tres niveles
- dos

3. tercero
4. cuarto
   1. anidado

- [ ] por hacer
- [x] hecho

> Una cita que se lee en cursiva
> y sigue en otro renglon

---

### Codigo

```rust
fn main() {
	println!("<hola> & adios");
}
```

![Planta|360](pixpin:files/guardados/pc/p1/notas/1-planta.png)
![Hoja viva](pixpin:files/guardados/pc/p1/notas/vivo-K7Q2ABCDEF.png)
![No esta](pixpin:files/no/esta.png)
![Plano.pdf](pixpin:files/guardados/pc/p1/notas/9-Plano.pdf)

| Objetivo | Tecnica | Coste |
|:---|:---:|---:|
| OE1 | **Pareto** | 10 |
| OE2 | Ishikawa | 20 |

<table>
  <tr>
    <th colspan="2" style="background:#FFF3C4;color:#7A4B00">Combinada</th>
    <th>C</th>
  </tr>
  <tr>
    <td rowspan="2" valign="middle">Alta</td>
    <td>b1</td>
    <td align="right">c1</td>
  </tr>
  <tr>
    <td>b2</td>
    <td>c2</td>
  </tr>
</table>

| Presupuesto | Contabilidad | Mediciones | Calendario | Subcontrata | Materiales | Seguridad | Documentos |
|---|---|---|---|---|---|---|---|
| Cimentacion | Estructuras | Albanileria | Carpinteria | Instalacion | Acabados01 | Revisiones | Entregables |

Fin del informe."#;

fn fotos(ruta: &str) -> Option<Vec<u8>> {
    if ruta.ends_with("1-planta.png") {
        Some(png(400, 300, [90, 160, 230]))
    } else if ruta.ends_with("vivo-K7Q2ABCDEF.png") {
        Some(png(900, 600, [240, 200, 120]))
    } else {
        None
    }
}

/// Un hilo anclado en la primera vez que sale `cita` en la nota.
fn hilo(md: &str, id: &str, cita: &str, texto: &str) -> Hilo {
    let u: Vec<u16> = md.encode_utf16().collect();
    let i = md.find(cita).expect("la cita esta en la nota");
    let a = md[..i].encode_utf16().count();
    let b = a + cita.encode_utf16().count();
    Hilo {
        id: id.into(),
        ancla: md_comentarios::ancla_de(&u, a, b).expect("ancla"),
        autor: "PC de Max".into(),
        cuando: 1_790_000_000_000,
        texto: texto.into(),
        ..Default::default()
    }
}

pub(super) fn comentarios_de_prueba() -> Comentarios {
    let mut c = Comentarios::default();
    let mut uno = hilo(
        NOTA,
        "h1",
        "negrita",
        "¿Esto va en negrita?\nSegundo renglon & <cosas>",
    );
    uno.respuestas.push(Respuesta {
        id: "r1".into(),
        autor: "Movil".into(),
        cuando: 1_790_000_100_000,
        texto: "Si, que se vea".into(),
        ..Default::default()
    });
    c.comentarios.push(uno);
    c.comentarios
        .push(hilo(NOTA, "h2", "Pareto", "Comentario en una celda"));
    let mut resuelto = hilo(NOTA, "h3", "Una cita", "Ya esta");
    resuelto.resuelto = true;
    c.comentarios.push(resuelto);
    let mut perdido = hilo(NOTA, "h4", "Fin del informe", "Este ya no esta");
    perdido.ancla.cita = "un texto que se borro hace tiempo".into();
    perdido.ancla.antes = "nada parecido aqui".into();
    perdido.ancla.despues = "ni aqui tampoco".into();
    c.comentarios.push(perdido);
    c
}

pub(super) fn exportar_de_prueba(md: &str, c: Option<&Comentarios>) -> Vec<u8> {
    exportar(&Nota {
        markdown: md,
        titulo: "",
        letra: Letra::default(),
        comentarios: c,
        imagen: &fotos,
        autor: "PC de Max",
        ahora_ms: 1_790_000_000_000,
    })
    .expect("se exporta")
}

/// Todas las entradas del ZIP como texto (las XML) o bytes.
fn entradas(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut p = Paquete::de_bytes(bytes).expect("es un ZIP");
    let mut m = BTreeMap::new();
    for n in p.nombres() {
        let b = p.bytes(&n, 64 * 1024 * 1024).expect("se lee");
        m.insert(n, b);
    }
    m
}

fn texto(m: &BTreeMap<String, Vec<u8>>, n: &str) -> String {
    String::from_utf8(m.get(n).unwrap_or_else(|| panic!("falta {n}")).clone()).expect("UTF-8")
}

/// **El paquete esta sano**: cada XML bien formado, cada relacion apunta a
/// una parte que esta, y `[Content_Types]` da tipo a todas.
pub(super) fn comprobar_paquete(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let m = entradas(bytes);
    for (n, b) in &m {
        if n.ends_with(".xml") || n.ends_with(".rels") {
            let t = std::str::from_utf8(b).unwrap_or_else(|_| panic!("{n} no es UTF-8"));
            if let Err(e) = bien_formado(t) {
                panic!("{n} no esta bien formado: {e}");
            }
        }
    }
    for obligatoria in [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
    ] {
        assert!(m.contains_key(obligatoria), "falta {obligatoria}");
    }
    // Relaciones: las internas, a partes que estan.
    for (rels, base) in [
        ("_rels/.rels", ""),
        ("word/_rels/document.xml.rels", "word/"),
    ] {
        let raiz = crate::xml::leer(&texto(&m, rels)).unwrap();
        let mut ids = BTreeSet::new();
        for r in raiz.elementos() {
            assert!(
                ids.insert(r.atributo("Id").to_string()),
                "Id repetido en {rels}"
            );
            if r.atributo("TargetMode") == "External" {
                assert!(r.atributo("Target").starts_with("http"), "externa rara");
                continue;
            }
            let destino = format!("{base}{}", r.atributo("Target"));
            assert!(
                m.contains_key(&destino),
                "{rels} apunta a {destino}, que no esta"
            );
        }
    }
    // Tipos: cada parte, por su nombre o por su extension.
    let tipos = crate::xml::leer(&texto(&m, "[Content_Types].xml")).unwrap();
    let defaults: BTreeSet<String> = tipos
        .elementos()
        .filter(|e| e.nombre == "Default")
        .map(|e| e.atributo("Extension").to_ascii_lowercase())
        .collect();
    let overrides: BTreeSet<String> = tipos
        .elementos()
        .filter(|e| e.nombre == "Override")
        .map(|e| e.atributo("PartName").to_string())
        .collect();
    for n in m.keys().filter(|n| *n != "[Content_Types].xml") {
        let ext = n.rsplit('.').next().unwrap().to_ascii_lowercase();
        assert!(
            overrides.contains(&format!("/{n}")) || defaults.contains(&ext),
            "{n} no tiene tipo en [Content_Types].xml"
        );
    }
    for o in &overrides {
        assert!(
            m.contains_key(&o[1..]),
            "Override de una parte que no esta: {o}"
        );
    }
    m
}

fn leer_con_pixpin(bytes: &[u8]) -> crate::Documento {
    let mut p = Paquete::de_bytes(bytes).unwrap();
    docx::de_paquete(&mut p, "x").expect("el lector de PixPin lo abre")
}

#[test]
fn la_nota_con_todo_sale_en_un_paquete_sano() {
    let c = comentarios_de_prueba();
    let bytes = exportar_de_prueba(NOTA, Some(&c));
    let m = comprobar_paquete(&bytes);
    assert!(m.contains_key("word/comments.xml") && m.contains_key("word/commentsExtended.xml"));
    let medios: Vec<&String> = m.keys().filter(|n| n.starts_with("word/media/")).collect();
    assert_eq!(
        medios.len(),
        2,
        "las dos fotos que estan, y no la que falta: {medios:?}"
    );
}

#[test]
fn el_lector_de_word_de_pixpin_lee_lo_mismo_que_dice_la_nota() {
    let d = leer_con_pixpin(&exportar_de_prueba(NOTA, None));
    let textos: Vec<(Clase, String)> = d.bloques.iter().map(|b| (b.clase, b.texto())).collect();
    let tiene = |clase: Clase, t: &str| textos.iter().any(|(c, x)| *c == clase && x == t);
    assert!(
        tiene(Clase::Titulo(1), "Informe de obra «Casa Lima»"),
        "{textos:#?}"
    );
    assert!(tiene(Clase::Titulo(2), "Listas"));
    assert!(tiene(Clase::Titulo(3), "Codigo"));
    // El primer parrafo, sin una sola marca y con lo especial tal cual.
    let p = d
        .bloques
        .iter()
        .find(|b| b.texto().starts_with("Texto con"))
        .expect("el parrafo");
    assert_eq!(
        p.texto(),
        "Texto con negrita, cursiva, tachado, codigo() y la formula x^2+1; especiales: <a> & \"b\" 'c' ✓ 😀."
    );
    let (t, tramos) = texto_y_tramos(p);
    let de = |palabra: &str| {
        let i = t[..t.find(palabra).unwrap()].encode_utf16().count() as u32;
        tramos
            .iter()
            .find(|x| x.inicio == i)
            .map(|x| x.estilo)
            .unwrap_or_default()
    };
    assert!(de("negrita").negrita && !de("negrita").cursiva);
    assert!(de("cursiva").cursiva);
    assert!(de("tachado").tachado);
    // Enlaces: el web es hipervinculo; los de PixPin, su nombre.
    let enlace = d
        .bloques
        .iter()
        .find(|b| b.texto().starts_with("Un enlace"))
        .unwrap();
    assert_eq!(
        enlace.texto(),
        "Un enlace a la web, a la hoja Planta baja y a un mensaje."
    );
    assert!(
        enlace
            .trozos
            .iter()
            .any(|x| x.texto == "la web" && x.estilo.enlace)
    );
    assert!(
        !enlace
            .trozos
            .iter()
            .any(|x| x.texto.contains("Planta") && x.estilo.enlace)
    );
    // Listas: siete puntos con numeracion de Word.
    let lista: Vec<String> = d
        .bloques
        .iter()
        .filter(|b| b.clase == Clase::Lista)
        .map(|b| b.texto())
        .collect();
    assert_eq!(
        lista,
        [
            "uno",
            "uno punto uno",
            "tres niveles",
            "dos",
            "tercero",
            "cuarto",
            "anidado"
        ]
    );
    assert!(
        tiene(Clase::Parrafo, "\u{2610}\tpor hacer") && tiene(Clase::Parrafo, "\u{2612}\thecho")
    );
    // El codigo, con su tabulador y sus signos.
    assert!(
        textos
            .iter()
            .any(|(_, x)| x == "\tprintln!(\"<hola> & adios\");")
    );
    // Las fotos que estan, y un aviso por la que no.
    assert_eq!(d.imagenes.len(), 2);
    assert!(d.imagenes.iter().all(|i| i.mime == "image/png"));
    assert!(textos.iter().any(|(_, x)| x.contains("No esta")));
    assert!(
        textos.iter().any(|(_, x)| x.contains("Plano.pdf")),
        "el adjunto, por su nombre"
    );
    assert!(tiene(Clase::Parrafo, "Fin del informe."));
}

#[test]
fn las_tablas_vuelven_con_sus_celdas_combinadas() {
    let d = leer_con_pixpin(&exportar_de_prueba(NOTA, None));
    let filas: Vec<&crate::Bloque> = d
        .bloques
        .iter()
        .filter(|b| b.clase == Clase::Fila)
        .collect();
    // GFM (3) + HTML (3) + la ancha (2).
    assert_eq!(filas.len(), 8);
    let tablas: BTreeSet<u32> = filas
        .iter()
        .map(|b| b.fila.as_ref().unwrap().tabla)
        .collect();
    assert_eq!(tablas.len(), 3, "tres tablas, sin fundirse");
    let celdas = |i: usize| filas[i].fila.as_ref().unwrap().celdas.clone();
    assert_eq!(
        celdas(0).iter().map(|c| c.texto()).collect::<Vec<_>>(),
        ["Objetivo", "Tecnica", "Coste"]
    );
    assert_eq!(celdas(1)[1].texto(), "Pareto");
    // La combinada de la cabecera ocupa dos columnas y tiene fondo.
    let c3 = celdas(3);
    assert_eq!(c3.len(), 2);
    assert_eq!(
        (c3[0].texto().as_str(), c3[0].columnas, c3[0].relleno),
        ("Combinada", 2, true)
    );
    // «Alta» baja dos filas: en la tercera, su continuacion.
    let c5 = celdas(5);
    assert!(c5[0].sigue && c5[0].texto().is_empty());
    assert_eq!(c5[1].texto(), "b2");
    assert_eq!(celdas(4)[0].texto(), "Alta");
}

#[test]
fn el_documento_lleva_los_estilos_y_la_estructura_de_word() {
    let m = comprobar_paquete(&exportar_de_prueba(NOTA, Some(&comentarios_de_prueba())));
    let doc = texto(&m, "word/document.xml");
    for esperado in [
        r#"<w:pStyle w:val="Heading1"/>"#,
        r#"<w:pStyle w:val="Heading3"/>"#,
        r#"<w:ilvl w:val="2"/><w:numId w:val="1"/>"#,
        r#"<w:gridSpan w:val="2"/>"#,
        r#"<w:vMerge w:val="restart"/>"#,
        "<w:vMerge/>",
        r#"w:fill="FFF3C4""#,
        r#"<w:color w:val="7A4B00"/>"#,
        "<w:tblHeader/>",
        r#"<w:jc w:val="center"/>"#,
        r#"<w:vAlign w:val="center"/>"#,
        r#"w:orient="landscape""#,
        r#"<w:pStyle w:val="CodigoBloque"/>"#,
        r#"<w:pStyle w:val="Cita"/>"#,
        r#"<w:rStyle w:val="CodigoEnLinea"/>"#,
        "Cambria Math",
        "<w:hyperlink r:id=",
        "<wp:inline",
    ] {
        assert!(doc.contains(esperado), "falta {esperado}");
    }
    // Las fotos a su tamano: la de 360 px del editor es la mitad de la
    // columna (720), luego la mitad del ancho de texto.
    let ancho = |cx: u64| doc.contains(&format!(r#"<wp:extent cx="{cx}""#));
    assert!(
        ancho(super::cuerpo::TEXTO_DE_PIE as u64 / 2 * 635),
        "la de 360 px, a media pagina"
    );
    assert!(
        ancho(super::cuerpo::TEXTO_DE_PIE as u64 * 635),
        "la viva de 900 px, a la columna entera"
    );
    // La ancha va tumbada y lo de despues vuelve de pie: dos saltos.
    assert_eq!(doc.matches("<w:sectPr>").count(), 3);
    let rels = texto(&m, "word/_rels/document.xml.rels");
    assert!(rels.contains(r#"Target="https://example.com/a?b=1&amp;c=2" TargetMode="External""#));
    assert!(
        !rels.contains("pixpin:"),
        "los enlaces de PixPin no son hipervinculos"
    );
    let num = texto(&m, "word/numbering.xml");
    assert!(
        num.contains(r#"<w:startOverride w:val="3"/>"#),
        "la lista empieza en el 3 como en la nota"
    );
}

#[test]
fn los_comentarios_van_anclados_a_su_texto_con_sus_respuestas() {
    let m = comprobar_paquete(&exportar_de_prueba(NOTA, Some(&comentarios_de_prueba())));
    let doc = texto(&m, "word/document.xml");
    // Cuatro hilos y una respuesta: cinco comentarios, cada uno con su
    // principio, su final y su referencia.
    for id in 0..5 {
        for marca in ["commentRangeStart", "commentRangeEnd", "commentReference"] {
            assert_eq!(
                doc.matches(&format!(r#"<w:{marca} w:id="{id}"/>"#)).count(),
                1,
                "{marca} {id}"
            );
        }
    }
    // El de «negrita» abraza justo esa palabra.
    let i = doc.find(r#"<w:commentRangeStart w:id="0"/>"#).unwrap();
    let f = doc.find(r#"<w:commentRangeEnd w:id="0"/>"#).unwrap();
    assert!(doc[i..f].contains(">negrita</w:t>") && !doc[i..f].contains("cursiva"));
    // El de la celda cae dentro de la tabla.
    let i = doc.find(r#"<w:commentRangeStart w:id="2"/>"#).unwrap();
    assert!(
        doc[..i].rfind("<w:tbl>") > doc[..i].rfind("</w:tbl>"),
        "dentro de una tabla"
    );
    let com = texto(&m, "word/comments.xml");
    assert!(com.contains(r#"w:author="PC de Max""#) && com.contains(r#"w:author="Movil""#));
    assert!(com.contains("2026-09-21T"), "con su fecha");
    assert!(com.contains("Segundo renglon &amp; &lt;cosas&gt;"));
    assert!(
        com.contains("«un texto que se borro hace tiempo»"),
        "el perdido lleva su cita"
    );
    let ex = texto(&m, "word/commentsExtended.xml");
    assert_eq!(ex.matches("w15:paraIdParent=").count(), 1, "una respuesta");
    assert_eq!(ex.matches(r#"w15:done="1""#).count(), 1, "un resuelto");
}

#[test]
fn una_nota_vacia_es_un_word_vacio_pero_sano() {
    for md in ["", "\n\n", "   "] {
        let m = comprobar_paquete(&exportar_de_prueba(md, None));
        assert!(
            !m.contains_key("word/comments.xml"),
            "sin comentarios no hay parte"
        );
        assert!(texto(&m, "word/document.xml").contains("<w:body><w:p/>"));
    }
}

#[test]
fn un_comentario_en_una_nota_sin_letras_no_se_pierde() {
    let mut c = Comentarios::default();
    let mut h = hilo("hola", "h1", "hola", "suelto");
    h.ancla.cita = "otra cosa".into();
    c.comentarios.push(h);
    let m = comprobar_paquete(&exportar_de_prueba("", Some(&c)));
    let doc = texto(&m, "word/document.xml");
    assert!(doc.contains(r#"<w:commentReference w:id="0"/>"#));
}

#[test]
fn las_letras_que_xml_no_admite_se_quitan_y_no_rompen_el_documento() {
    let md = "a\u{1}b\u{FFFF}c\u{B}d\u{0}e";
    let m = comprobar_paquete(&exportar_de_prueba(md, None));
    let d = leer_con_pixpin(&exportar_de_prueba(md, None));
    assert_eq!(d.bloques[0].texto(), "abc\nde", "{:?}", d.bloques);
    assert!(texto(&m, "word/document.xml").contains("<w:br/>"));
}

#[test]
fn lo_que_no_es_tabla_se_queda_como_texto() {
    // Una tabla dentro de codigo, una fila sin la de guiones y un HTML con
    // titulo (que el editor tampoco sabe ensenar).
    let md = "```\n| a | b |\n|---|---|\n```\n| suelta | sin guiones |\n<table><caption>T</caption><tr><td>x</td></tr></table>";
    let d = leer_con_pixpin(&exportar_de_prueba(md, None));
    assert!(
        d.bloques.iter().all(|b| b.clase != Clase::Fila),
        "{:?}",
        d.bloques
    );
    assert!(d.bloques.iter().any(|b| b.texto() == "| a | b |"));
}

#[test]
fn una_foto_que_word_no_pinta_deja_un_aviso_y_no_un_fichero_roto() {
    let webp = |_: &str| Some(b"RIFF\0\0\0\0WEBPVP8 ".to_vec());
    let bytes = exportar(&Nota {
        markdown: "![Foto](a.webp)",
        titulo: "x",
        letra: Letra::default(),
        comentarios: None,
        imagen: &webp,
        autor: "",
        ahora_ms: 0,
    })
    .unwrap();
    let m = comprobar_paquete(&bytes);
    assert!(!m.keys().any(|n| n.starts_with("word/media/")));
    assert!(texto(&m, "word/document.xml").contains("Foto"));
}

#[test]
fn la_letra_de_la_nota_va_con_su_sustituta_de_windows() {
    let bytes = exportar(&Nota {
        markdown: "# T\n\nhola",
        titulo: "",
        letra: Letra {
            cuerpo: "Caveat".into(),
            titulos: "Courier New".into(),
            px: 21,
        },
        comentarios: None,
        imagen: &|_| None,
        autor: "",
        ahora_ms: 0,
    })
    .unwrap();
    let m = comprobar_paquete(&bytes);
    let estilos = texto(&m, "word/styles.xml");
    assert!(
        estilos.contains(r#"w:ascii="Caveat""#) && estilos.contains(r#"w:ascii="Courier New""#)
    );
    // 21 px son 15,75 pt: 31 medios puntos.
    assert!(estilos.contains(r#"<w:sz w:val="31"/>"#));
    let letras = texto(&m, "word/fontTable.xml");
    assert!(letras.contains(r#"<w:font w:name="Caveat"><w:altName w:val="Segoe Print"/>"#));
    assert!(
        !letras.contains(r#"<w:font w:name="Courier New"><w:altName"#),
        "Courier ya es de Windows"
    );
    let core = texto(&m, "docProps/core.xml");
    assert!(core.contains("<dc:title>T</dc:title>") && core.contains("1970-01-01T00:00:00Z"));
}

#[test]
fn el_nombre_de_fichero_es_seguro_en_windows() {
    assert_eq!(
        nombre_de_fichero("Informe: obra/2026?"),
        "Informe_ obra_2026_.docx"
    );
    assert_eq!(nombre_de_fichero("  "), "Nota.docx");
    assert_eq!(
        nombre_de_fichero("con punto final. "),
        "con punto final.docx"
    );
    assert_eq!(nombre_de_fichero("CON"), "CON_.docx");
    assert_eq!(nombre_de_fichero("com1.txt"), "com1.txt_.docx");
    assert_eq!(
        nombre_de_fichero("Concierto"),
        "Concierto.docx",
        "empezar por CON no es reservado"
    );
    assert_eq!(nombre_de_fichero("a\u{7}b"), "a_b.docx");
    assert!(nombre_de_fichero(&"x".repeat(500)).chars().count() <= 125);
}

#[test]
fn una_tabla_ancha_achica_la_letra_y_si_no_basta_se_tumba() {
    let tabla = |columnas: usize, palabra: &str| {
        let fila = |t: &str| format!("|{}|", vec![t; columnas].join("|"));
        format!("{}\n{}\n{}", fila("Cabecera"), fila("---"), fila(palabra))
    };
    let reparto = |md: &str| {
        let m = modelo(md);
        match &m.bloques[0] {
            Bloque1::Tabla { tabla, celdas } => cuerpo::repartir(tabla, celdas, 16),
            otro => panic!("no es tabla: {otro:?}"),
        }
    };
    let estrecha = reparto(&tabla(3, "corto"));
    assert!(!estrecha.tumbada && estrecha.tam.is_none());
    assert_eq!(
        estrecha.anchos.iter().sum::<u32>(),
        cuerpo::TEXTO_DE_PIE,
        "llena el ancho de texto"
    );
    // Seis columnas de palabras largas: de pie, con la letra a 9 pt.
    let algo = reparto(&tabla(6, "Cuantitativo"));
    assert!(!algo.tumbada && algo.tam == Some(18), "{algo:?}");
    // Siete ya no caben de pie ni a 9 pt: tumbada, con la letra de siempre.
    let ancha = reparto(&tabla(7, "Contabilidad"));
    assert!(ancha.tumbada && ancha.tam.is_none(), "{ancha:?}");
    assert_eq!(ancha.anchos.iter().sum::<u32>(), cuerpo::TEXTO_TUMBADO);
    let enorme = reparto(&tabla(14, "Responsabilidades"));
    assert!(enorme.tumbada && enorme.tam == Some(16));
    // Las columnas piden en proporcion a lo que llevan.
    let desigual = reparto("| a | una cabecera bastante mas larga |\n|---|---|\n| 1 | 2 |");
    assert!(
        desigual.anchos[1] > desigual.anchos[0] * 3,
        "{:?}",
        desigual.anchos
    );
}

#[test]
fn las_imagenes_se_reconocen_por_sus_bytes_y_no_por_el_nombre() {
    assert_eq!(
        imagen::leer(&png(7, 5, [0, 0, 0])),
        Some((imagen::Formato::Png, 7, 5))
    );
    let jpeg = [
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x20,
        0x00, 0x40, 0x01, 0x01, 0x11, 0x00,
    ];
    assert_eq!(imagen::leer(&jpeg), Some((imagen::Formato::Jpeg, 64, 32)));
    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&[10, 0, 20, 0]);
    assert_eq!(imagen::leer(&gif), Some((imagen::Formato::Gif, 10, 20)));
    // Casos negativos: cortado, de un formato que Word no pinta, nada.
    assert_eq!(imagen::leer(&png(7, 5, [0, 0, 0])[..20]), None);
    assert!(
        imagen::es_webp(b"RIFF\0\0\0\0WEBPVP8 ") && imagen::leer(b"RIFF\0\0\0\0WEBPVP8 ").is_none()
    );
    assert_eq!(imagen::leer(b""), None);
    assert_eq!(imagen::leer(&[0xFF, 0xD8, 0xFF]), None);
}

#[test]
fn las_fechas_salen_en_utc_como_las_quiere_word() {
    assert_eq!(partes::fecha(0), "1970-01-01T00:00:00Z");
    assert_eq!(partes::fecha(1_790_000_000_000), "2026-09-21T14:13:20Z");
    assert_eq!(
        partes::fecha(951_782_400_000),
        "2000-02-29T00:00:00Z",
        "bisiesto"
    );
}

/// **La muestra para abrir con Word**: deja el `.docx` de la nota con todo
/// en `target/muestras-docx/`. Con `PIXPIN_NOTA_REAL=<ruta .md>` deja
/// tambien el de esa nota (fotos sin resolver: avisos en su sitio).
#[test]
#[ignore]
fn muestra_para_word() {
    let carpeta =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/muestras-docx");
    std::fs::create_dir_all(&carpeta).unwrap();
    let bytes = exportar_de_prueba(NOTA, Some(&comentarios_de_prueba()));
    std::fs::write(carpeta.join("nota-completa.docx"), &bytes).unwrap();
    if let Ok(ruta) = std::env::var("PIXPIN_NOTA_REAL") {
        let md = std::fs::read_to_string(&ruta).unwrap();
        let junto = std::path::Path::new(&ruta).parent().unwrap().to_path_buf();
        let fotos = move |r: &str| {
            let nombre = r.rsplit('/').next()?;
            std::fs::read(junto.join(nombre)).ok()
        };
        let c = std::env::var("PIXPIN_NOTA_REAL_COMENTARIOS")
            .ok()
            .and_then(|r| std::fs::read_to_string(r).ok())
            .and_then(|t| md_comentarios::leer(&t).ok());
        let bytes = exportar(&Nota {
            markdown: &md,
            titulo: "",
            letra: Letra::default(),
            comentarios: c.as_ref(),
            imagen: &fotos,
            autor: "PixPin",
            ahora_ms: 1_790_000_000_000,
        })
        .unwrap();
        comprobar_paquete(&bytes);
        leer_con_pixpin(&bytes);
        std::fs::write(carpeta.join("nota-real.docx"), &bytes).unwrap();
    }
}

#[test]
fn el_titulo_de_una_tabla_tumbada_se_va_con_ella_a_su_pagina() {
    let fila = |t: &str| format!("|{}|", [t; 7].join("|"));
    let md = format!(
        "Intro\n\n## Tabla grande\n\n{}\n{}\n{}\n\nFin",
        fila("Contabilidad"),
        fila("---"),
        fila("x")
    );
    let m = comprobar_paquete(&exportar_de_prueba(&md, None));
    let doc = texto(&m, "word/document.xml");
    let salto = doc
        .find("<w:p><w:pPr><w:sectPr>")
        .expect("salto de seccion");
    let titulo = doc.find(r#"<w:pStyle w:val="Heading2"/>"#).unwrap();
    let intro = doc.find(">Intro<").unwrap();
    assert!(
        intro < salto && salto < titulo,
        "el salto va entre la intro y el titulo"
    );
    // Caso negativo: una tabla que cabe de pie no parte el documento.
    let corta = "## T\n\n| a | b |\n|---|---|\n| 1 | 2 |";
    let doc = texto(
        &comprobar_paquete(&exportar_de_prueba(corta, None)),
        "word/document.xml",
    );
    assert_eq!(doc.matches("<w:sectPr>").count(), 1);
}
