//! **Lo que el movil coció dentro del PDF de un proyecto.**
//!
//! El movil devuelve lo anotado al documento del proyecto (`PdfDelProyecto.
//! rehacer`): cada hoja se pega a su pagina como una capa `PixPin — pagina N`
//! en una **revision incremental** (`PdfAnotado.anotar` + `PdfEscritura.
//! incremental`), es decir, anadida al final del fichero detras de un `%%EOF`.
//! Rehace desde la copia limpia, pero hay proyectos cuya «copia limpia» es el
//! mismo fichero que el documento (`PaquetePixpin.importar` pone las dos a la
//! misma ruta), y esos llevan lo anotado dentro, revision tras revision.
//!
//! Al abrir la pagina en el lienzo, lo anotado se pinta **encima, vivo**. Si
//! el fondo lo trae tambien, se ve dos veces: el recuadro de una zona
//! vinculada salia doble (queja del 28-sep, «dos cuadros de vinculacion
//! superpuestos cuando solo debe ser uno»), porque el cocido y el vivo no
//! coinciden al pixel. En el movil coinciden y no se nota; aqui no se puede
//! prometer eso.
//!
//! Como cada revision se anade al final, **el PDF de antes de la primera capa
//! de PixPin es un prefijo del fichero** que acaba en un `%%EOF`: es un PDF
//! entero y valido, el que habia antes de anotar. Se corta ahi y listo, sin
//! reescribir nada ni depender de que el lector de Windows respete las capas
//! apagadas.

/// Como empieza el nombre de una capa de PixPin, en las tres formas en que
/// puede ir escrito dentro del PDF: cadena hexadecimal UTF-16 (la del movil,
/// `<feff0050…>`), en mayusculas, y cadena literal.
const MARCAS: [&[u8]; 4] = [
    b"feff00500069007800500069006e",
    b"FEFF00500069007800500069006E",
    b"(PixPin",
    b"(\xFE\xFF\x00P\x00i\x00x\x00P\x00i\x00n",
];

/// **Cuanto mide el PDF de antes de lo cocido**: el largo del prefijo que
/// acaba en el `%%EOF` anterior a la primera capa de PixPin. `None` si no hay
/// ninguna capa de PixPin (el fichero ya esta limpio) o si esta en la primera
/// revision (no hay un antes al que volver).
pub fn largo_sin_lo_cocido(pdf: &[u8]) -> Option<usize> {
    let primera = primera_capa_de_pixpin(pdf)?;
    let fin = memchr::memmem::rfind(&pdf[..primera], b"%%EOF")?;
    let mut largo = fin + b"%%EOF".len();
    // El fin de linea del `%%EOF` es suyo.
    while largo < pdf.len() && matches!(pdf[largo], b'\r' | b'\n') && largo < primera {
        largo += 1;
    }
    Some(largo)
}

/// Donde empieza el primer nombre de capa de PixPin: detras de un `/Name`
/// (un `(PixPin` suelto en el `/Producer` de un PDF exportado no es una
/// capa), con los blancos y el `<` que abre la cadena hexadecimal.
fn primera_capa_de_pixpin(pdf: &[u8]) -> Option<usize> {
    memchr::memmem::find_iter(pdf, b"/Name").find_map(|i| {
        let mut j = i + b"/Name".len();
        while j < pdf.len() && matches!(pdf[j], b' ' | b'\r' | b'\n' | b'\t') {
            j += 1;
        }
        // El `(` es parte de la marca literal; el `<` no.
        let k = if pdf.get(j) == Some(&b'<') { j + 1 } else { j };
        MARCAS.iter().any(|m| pdf[k..].starts_with(m)).then_some(k)
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ORIGINAL: &[u8] = b"%PDF-1.7\n1 0 obj<<>>endobj\nstartxref\n9\n%%EOF\n";

    fn con_capa(nombre: &str) -> Vec<u8> {
        let mut v = ORIGINAL.to_vec();
        v.extend_from_slice(
            format!("9 0 obj<</Type /OCG /Name {nombre}>>endobj\nstartxref\n50\n%%EOF\n")
                .as_bytes(),
        );
        v
    }

    #[test]
    fn el_pdf_con_la_capa_del_movil_vuelve_a_lo_de_antes_de_anotar() {
        let v = con_capa("<feff00500069007800500069006e002000140020007000e10067>");
        assert_eq!(largo_sin_lo_cocido(&v), Some(ORIGINAL.len()));
        // Y con varias revisiones cocidas, a la de antes de la primera.
        let mut dos = v.clone();
        dos.extend_from_slice(b"10 0 obj<</Type /OCG /Name (PixPin 2)>>endobj\n%%EOF\n");
        assert_eq!(largo_sin_lo_cocido(&dos), Some(ORIGINAL.len()));
    }

    #[test]
    fn tambien_con_el_nombre_en_mayusculas_o_literal() {
        assert_eq!(
            largo_sin_lo_cocido(&con_capa("<FEFF00500069007800500069006E>")),
            Some(ORIGINAL.len())
        );
        assert_eq!(
            largo_sin_lo_cocido(&con_capa("(PixPin)")),
            Some(ORIGINAL.len())
        );
    }

    #[test]
    fn un_pdf_sin_capas_de_pixpin_no_se_toca() {
        // Caso negativo: una revision anadida que no es de PixPin (una pagina
        // pegada, las capas de AutoCAD) se queda.
        let mut v = ORIGINAL.to_vec();
        v.extend_from_slice(b"9 0 obj<</Type /OCG /Name (Muros)>>endobj\n%%EOF\n");
        assert_eq!(largo_sin_lo_cocido(&v), None);
        assert_eq!(largo_sin_lo_cocido(ORIGINAL), None);
        // Ni un «PixPin» que no es el nombre de una capa.
        let mut w = ORIGINAL.to_vec();
        w.extend_from_slice(b"9 0 obj<</Producer (PixPin Max)>>endobj\n%%EOF\n");
        assert_eq!(largo_sin_lo_cocido(&w), None);
    }

    #[test]
    fn si_la_capa_esta_en_la_primera_revision_no_hay_antes() {
        // Caso negativo: sin un `%%EOF` delante no se puede cortar.
        let v = b"%PDF-1.7\n9 0 obj<</Name (PixPin)>>endobj\n%%EOF\n";
        assert_eq!(largo_sin_lo_cocido(v), None);
    }
}
