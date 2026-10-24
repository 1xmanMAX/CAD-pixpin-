//! **Texto dentro de una figura**: el rectangulo con su palabra, el circulo
//! con su nombre.
//!
//! Porte de `TextoEnFiguras.kt` del movil (211 lineas), que a su vez copia las
//! cuentas de `packages/element/src/textElement.ts` de Excalidraw.
//!
//! Es lo que convierte el motor en algo con lo que se hacen diagramas. Un
//! organigrama, un esquema de flujo o un mapa conceptual no son formas y
//! textos sueltos puestos uno encima de otro: son **cajas que dicen algo**, y
//! la diferencia se nota en cuanto mueves una. Un texto suelto se queda donde
//! estaba; el texto de la caja se va con ella.
//!
//! # Las cuentas raras tienen su porque geometrico
//!
//! | Figura | Cuanto cabe de ancho |
//! |---|---|
//! | Rectangulo | `ancho - 2*relleno` |
//! | Elipse | `redondear(ancho/2 * raiz(2)) - 2*relleno` |
//! | Rombo | `redondear(ancho/2) - 2*relleno` |
//!
//! El rectangulo mas grande que cabe dentro de una elipse mide
//! `ancho/2*raiz(2)`, y dentro de un rombo, `ancho/2`. Puesto de otro modo: en
//! un circulo del mismo ancho cabe un 70 % del texto, y en un rombo, la mitad.
//! Quien escriba en un rombo vera que se le queda pequeno antes, y eso no es
//! un fallo: es que un rombo tiene menos sitio dentro.
//!
//! # Por que `mide` llega de fuera
//!
//! Porque medir letras necesita DirectWrite y este crate es puro: se prueba
//! sin escritorio y sin GPU. Quien llame pasa su propio medidor —el de la
//! ventana, o el promedio de `texto::ancho_estimado` cuando no hay ninguno—.
//!
//! # Lo que este modulo NO hace todavia
//!
//! Atar y colocar esta aqui y probado; **pintar el rotulo en su sitio** es de
//! `pintado.rs`, que hoy pinta todo texto en la esquina de su caja porque no
//! mira `extras.contenedor`. Ver el informe del grupo A.

use crate::elemento::{Atado, Elemento, Figura};
use crate::texto::{AlineacionTexto, AlineacionVertical};
use crate::vector::Punto2;

/// El aire entre el texto y el borde de su figura. El de Excalidraw.
pub const RELLENO_DEL_TEXTO: f32 = 5.0;

/// Que figuras admiten texto dentro: las tres cerradas de caja del original.
///
/// El resto no: una flecha con un texto «dentro» no tiene dentro, y un marco
/// ya lleva su nombre por fuera.
pub fn admite_texto_dentro(f: &Figura) -> bool {
    matches!(f, Figura::Rectangulo | Figura::Elipse | Figura::Rombo)
}

/// La esquina de arriba a la izquierda del hueco donde va el texto.
///
/// En una elipse y en un rombo el hueco no empieza en la esquina de la caja:
/// hay que meterse hacia dentro hasta donde la figura deja sitio de verdad.
pub fn esquina_del_hueco(contenedor: &Elemento) -> Punto2 {
    let mut dx = RELLENO_DEL_TEXTO;
    let mut dy = RELLENO_DEL_TEXTO;
    match contenedor.figura {
        Figura::Elipse => {
            let k = 1.0 - std::f32::consts::SQRT_2 / 2.0;
            dx += (contenedor.ancho / 2.0) * k;
            dy += (contenedor.alto / 2.0) * k;
        }
        Figura::Rombo => {
            dx += contenedor.ancho / 4.0;
            dy += contenedor.alto / 4.0;
        }
        _ => {}
    }
    Punto2::nuevo(contenedor.x + dx, contenedor.y + dy)
}

/// Lo ancho que puede ser el texto dentro de `contenedor`.
pub fn ancho_que_cabe(contenedor: &Elemento) -> f32 {
    match contenedor.figura {
        Figura::Elipse => {
            (contenedor.ancho / 2.0 * std::f32::consts::SQRT_2).round() - RELLENO_DEL_TEXTO * 2.0
        }
        Figura::Rombo => (contenedor.ancho / 2.0).round() - RELLENO_DEL_TEXTO * 2.0,
        _ => contenedor.ancho - RELLENO_DEL_TEXTO * 2.0,
    }
}

/// Y lo alto.
pub fn alto_que_cabe(contenedor: &Elemento) -> f32 {
    match contenedor.figura {
        Figura::Elipse => {
            (contenedor.alto / 2.0 * std::f32::consts::SQRT_2).round() - RELLENO_DEL_TEXTO * 2.0
        }
        Figura::Rombo => (contenedor.alto / 2.0).round() - RELLENO_DEL_TEXTO * 2.0,
        _ => contenedor.alto - RELLENO_DEL_TEXTO * 2.0,
    }
}

/// Lo que tiene que medir la figura para que quepa un texto de `medida`.
///
/// Es la vuelta de [`ancho_que_cabe`], y es lo que hace que **la caja crezca
/// sola** cuando se escribe mas de lo que cabe. Sin esto, al llegar al borde
/// el texto seguiria saliendo por fuera del rectangulo y habria que estirarlo
/// a mano cada vez, que es justo el trabajo que un diagrama no deberia dar.
pub fn figura_que_lo_contiene(medida: f32, f: &Figura) -> f32 {
    let d = medida.ceil();
    let relleno = RELLENO_DEL_TEXTO * 2.0;
    match f {
        Figura::Elipse => ((d + relleno) / std::f32::consts::SQRT_2 * 2.0).round(),
        Figura::Rombo => 2.0 * (d + relleno),
        _ => d + relleno,
    }
}

/// Donde va el texto dentro de su figura: **centrado**.
///
/// Centrado en los dos ejes y no arriba a la izquierda, que es lo que hace que
/// una caja de diagrama se lea como una etiqueta y no como un parrafo metido
/// en un marco. Excalidraw permite las nueve posiciones; aqui va una sola,
/// como en el movil, porque las otras ocho son ocho opciones mas para algo que
/// casi nadie cambia.
///
/// `medida_del_texto` es `(ancho, alto)` de lo ya compuesto.
pub fn sitio_del_texto_dentro(contenedor: &Elemento, medida_del_texto: (f32, f32)) -> Punto2 {
    sitio_alineado_dentro(
        contenedor,
        medida_del_texto,
        AlineacionTexto::Centro,
        AlineacionVertical::Medio,
    )
}

/// **Donde va el texto dentro de su figura con su alineacion**: arriba,
/// en medio o abajo, y a la izquierda, al centro o a la derecha.
///
/// Es `computeBoundTextPosition` de Excalidraw (`textElement.ts`): el hueco
/// empieza en [`esquina_del_hueco`] y lo que sobra de [`ancho_que_cabe`] y
/// [`alto_que_cabe`] se reparte segun la alineacion —nada arriba, la mitad
/// en medio, todo abajo—. Con la misma cuenta que el centrado, las nueve
/// posiciones quedan dentro del mismo hueco y ninguna pisa el borde.
pub fn sitio_alineado_dentro(
    contenedor: &Elemento,
    medida_del_texto: (f32, f32),
    horizontal: AlineacionTexto,
    vertical: AlineacionVertical,
) -> Punto2 {
    let hueco = esquina_del_hueco(contenedor);
    let (ancho, alto) = medida_del_texto;
    Punto2::nuevo(
        hueco.x + (ancho_que_cabe(contenedor) - ancho) * horizontal.fraccion(),
        hueco.y + (alto_que_cabe(contenedor) - alto) * vertical.fraccion(),
    )
}

/// **Donde tiene que ir `texto` ahora**, si vive dentro de una figura de
/// `elementos`: el sitio que le dan su alineacion y la de su figura. `None`
/// si es un texto suelto, que no tiene hueco en el que moverse.
///
/// Sin alineacion en el fichero se toman las de fabrica —izquierda y
/// arriba—, las mismas que pone `restore` de Excalidraw a un texto que no las
/// trae y las mismas que marca el panel: si aqui se supusiera otra, el boton
/// marcado y el sitio del rotulo dirian cosas distintas.
pub fn sitio_en_su_contenedor(texto: &Elemento, elementos: &[Elemento]) -> Option<Punto2> {
    let contenedor = contenedor_de(texto, elementos, &crate::enlace::id_de_texto_de)?;
    Some(sitio_alineado_dentro(
        contenedor,
        (texto.ancho, texto.alto),
        texto.extras.alineacion.unwrap_or_default(),
        texto.extras.alineacion_vertical.unwrap_or_default(),
    ))
}

/// Parte el texto en lineas que quepan en `ancho`.
///
/// Se corta **por palabras**, y solo se parte una palabra cuando ella sola no
/// cabe: un nombre largo en una caja estrecha tiene que verse aunque quede
/// partido, y desbordar seria peor. Los saltos de linea que haya escrito el
/// usuario se respetan tal cual: son suyos.
///
/// Con `ancho` cero o negativo no se reparte nada —no hay sitio que repartir—
/// y salen los parrafos tal cual, que es lo que hace el movil.
pub fn repartir_en_lineas(texto: &str, ancho: f32, mide: &dyn Fn(&str) -> f32) -> Vec<String> {
    if texto.is_empty() {
        return vec![String::new()];
    }
    if ancho <= 0.0 {
        return texto.split('\n').map(str::to_string).collect();
    }

    let mut salida: Vec<String> = Vec::new();
    for parrafo in texto.split('\n') {
        if parrafo.is_empty() {
            salida.push(String::new());
            continue;
        }
        let mut linea = String::new();
        for palabra in parrafo.split(' ') {
            let probar = if linea.is_empty() {
                palabra.to_string()
            } else {
                format!("{linea} {palabra}")
            };
            if mide(&probar) <= ancho || linea.is_empty() {
                // Cabe, o es la primera palabra de la linea y no queda otra
                // que ponerla aunque se pase: partirla se hace mas abajo.
                linea = probar;
            } else {
                salida.push(std::mem::take(&mut linea));
                linea = palabra.to_string();
            }
        }
        salida.extend(partir_si_no_cabe(&linea, ancho, mide));
    }
    if salida.is_empty() {
        vec![String::new()]
    } else {
        salida
    }
}

/// Parte una palabra que no cabe entera, letra a letra.
///
/// Por CARACTERES y no por bytes, la misma regla que `texto.rs`: cortar una
/// cadena UTF-8 por un byte de en medio entra en panico.
fn partir_si_no_cabe(linea: &str, ancho: f32, mide: &dyn Fn(&str) -> f32) -> Vec<String> {
    if linea.is_empty() || mide(linea) <= ancho {
        return vec![linea.to_string()];
    }
    let mut trozos = Vec::new();
    let mut actual = String::new();
    for c in linea.chars() {
        let mut probar = actual.clone();
        probar.push(c);
        if mide(&probar) > ancho && !actual.is_empty() {
            trozos.push(std::mem::take(&mut actual));
            actual.push(c);
        } else {
            actual = probar;
        }
    }
    if !actual.is_empty() {
        trozos.push(actual);
    }
    trozos
}

// -------------------------------------------------------------------------
// El vinculo entre la figura y su texto
// -------------------------------------------------------------------------

/// El identificador con el que el movil ata este elemento: el del fichero.
///
/// El `Elemento` de aqui lleva un `id` numerico derivado del de texto
/// (`excalidraw::id_estable`), asi que para atar hace falta el de texto, que
/// solo conoce quien tiene el JSON. Va como parametro por eso.
pub type IdDelFichero<'a> = &'a str;

/// El texto que vive dentro de `contenedor`, si lo hay.
///
/// `id_de` traduce un elemento a su identificador de fichero, que es el que se
/// compara: `extras.contenedor` guarda el del movil y no el numerico de aqui.
pub fn texto_de<'a>(
    id_contenedor: IdDelFichero<'_>,
    elementos: &'a [Elemento],
) -> Option<&'a Elemento> {
    elementos.iter().find(|e| {
        matches!(e.figura, Figura::Texto { .. })
            && !e.borrado
            && e.extras.contenedor.as_deref() == Some(id_contenedor)
    })
}

/// Y al reves: la figura en la que vive `texto`.
pub fn contenedor_de<'a>(
    texto: &Elemento,
    elementos: &'a [Elemento],
    id_de: &dyn Fn(&Elemento) -> String,
) -> Option<&'a Elemento> {
    let id = texto.extras.contenedor.as_deref()?;
    elementos
        .iter()
        .find(|e| !e.borrado && id_de(e) == id && admite_texto_dentro(&e.figura))
}

/// Ata un texto a una figura, **por los dos lados**.
///
/// Los dos lados hacen falta. El texto guarda de quien es, para saber donde
/// colocarse; la figura guarda que lleva dentro, para poder arrastrarlo,
/// borrarlo con ella y crecer cuando haga falta. Con un solo lado, mover la
/// figura dejaria el texto atras, o borrarla dejaria un texto huerfano
/// flotando.
///
/// Es idempotente: atar dos veces no duplica la entrada de `boundElements`,
/// que en el movil se lee como dos rotulos dentro de la misma caja.
pub fn atar(
    contenedor: &mut Elemento,
    id_contenedor: IdDelFichero<'_>,
    texto: &mut Elemento,
    id_texto: IdDelFichero<'_>,
) {
    if !contenedor.extras.atados.iter().any(|a| a.id == id_texto) {
        contenedor.extras.atados.push(Atado {
            id: id_texto.to_string(),
            tipo: "text".to_string(),
        });
        contenedor.tocar();
    }
    if texto.extras.contenedor.as_deref() != Some(id_contenedor) {
        texto.extras.contenedor = Some(id_contenedor.to_string());
        texto.tocar();
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::formas::TipoPunta;

    /// Un medidor de prueba: cada caracter mide diez. No hace falta
    /// DirectWrite para comprobar donde se parte una linea.
    fn diez(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    fn caja(figura: Figura, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            figura,
            x: 100.0,
            y: 50.0,
            ancho,
            alto,
            ..Default::default()
        }
    }

    #[test]
    fn solo_las_tres_figuras_cerradas_admiten_texto_dentro() {
        assert!(admite_texto_dentro(&Figura::Rectangulo));
        assert!(admite_texto_dentro(&Figura::Elipse));
        assert!(admite_texto_dentro(&Figura::Rombo));
        // Caso negativo: una flecha no tiene dentro, y un marco ya lleva su
        // nombre por fuera.
        assert!(!admite_texto_dentro(&Figura::Flecha {
            puntos: Vec::new(),
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        }));
        assert!(!admite_texto_dentro(&Figura::Marco {
            nombre: String::new()
        }));
    }

    #[test]
    fn en_un_rombo_cabe_la_mitad_que_en_un_rectangulo_y_en_una_elipse_el_setenta_por_ciento() {
        // Es la razon de ser de las tres cuentas: reinventarlas daria un texto
        // que cabe en un rectangulo y se sale de un rombo.
        let r = ancho_que_cabe(&caja(Figura::Rectangulo, 200.0, 100.0));
        let e = ancho_que_cabe(&caja(Figura::Elipse, 200.0, 100.0));
        let d = ancho_que_cabe(&caja(Figura::Rombo, 200.0, 100.0));
        assert_eq!(r, 190.0);
        assert_eq!(e, 131.0, "ancho/2 * raiz(2) redondeado, menos el aire");
        assert_eq!(d, 90.0, "ancho/2, menos el aire");
        assert!(d < e && e < r);
    }

    #[test]
    fn el_hueco_de_una_elipse_y_de_un_rombo_empieza_dentro_y_no_en_la_esquina() {
        // Caso negativo: con la esquina de la caja, el rotulo de un rombo
        // saldria por fuera de sus lados inclinados.
        let rect = esquina_del_hueco(&caja(Figura::Rectangulo, 200.0, 100.0));
        assert_eq!(rect, Punto2::nuevo(105.0, 55.0));
        let rombo = esquina_del_hueco(&caja(Figura::Rombo, 200.0, 100.0));
        assert_eq!(rombo, Punto2::nuevo(155.0, 80.0));
        let elipse = esquina_del_hueco(&caja(Figura::Elipse, 200.0, 100.0));
        assert!(elipse.x > rect.x && elipse.x < rombo.x, "{elipse:?}");
    }

    #[test]
    fn el_texto_queda_centrado_en_los_dos_ejes() {
        let c = caja(Figura::Rectangulo, 200.0, 100.0);
        let p = sitio_del_texto_dentro(&c, (40.0, 20.0));
        // Centro de la caja menos medio texto.
        assert!((p.x - (100.0 + 100.0 - 20.0)).abs() < 0.01, "{p:?}");
        assert!((p.y - (50.0 + 50.0 - 10.0)).abs() < 0.01, "{p:?}");
    }

    #[test]
    fn arriba_a_la_izquierda_y_abajo_a_la_derecha_se_quedan_dentro_del_hueco() {
        // Caja 100,50 de 200 x 100; hueco de 190 x 90 desde 105,55.
        let c = caja(Figura::Rectangulo, 200.0, 100.0);
        let medida = (40.0, 20.0);
        let arriba = sitio_alineado_dentro(
            &c,
            medida,
            AlineacionTexto::Izquierda,
            AlineacionVertical::Arriba,
        );
        assert_eq!(arriba, Punto2::nuevo(105.0, 55.0));
        let abajo = sitio_alineado_dentro(
            &c,
            medida,
            AlineacionTexto::Derecha,
            AlineacionVertical::Abajo,
        );
        // Su esquina de abajo a la derecha, pegada al borde del hueco.
        assert!((abajo.x + 40.0 - 295.0).abs() < 0.01, "{abajo:?}");
        assert!((abajo.y + 20.0 - 145.0).abs() < 0.01, "{abajo:?}");
        // Caso negativo: el centrado de siempre no se ha movido.
        assert_eq!(
            sitio_alineado_dentro(
                &c,
                medida,
                AlineacionTexto::Centro,
                AlineacionVertical::Medio
            ),
            sitio_del_texto_dentro(&c, medida)
        );
    }

    #[test]
    fn un_rotulo_se_recoloca_en_su_caja_y_un_texto_suelto_no() {
        let mut c = caja(Figura::Rectangulo, 200.0, 100.0);
        c.extras.id_de_fichero = Some("caja".into());
        let mut t = Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Excalifont".into(),
            },
            ancho: 40.0,
            alto: 20.0,
            ..Default::default()
        };
        t.extras.contenedor = Some("caja".into());
        t.extras.alineacion_vertical = Some(AlineacionVertical::Abajo);
        let elementos = vec![c.clone(), t.clone()];
        let p = sitio_en_su_contenedor(&t, &elementos).expect("vive en la caja");
        // Sin `textAlign` va a la izquierda, la de fabrica; abajo, porque
        // lo pide.
        assert!(
            (p.x - 105.0).abs() < 0.01 && (p.y - 125.0).abs() < 0.01,
            "{p:?}"
        );
        // Caso negativo: suelto no tiene donde ir.
        t.extras.contenedor = None;
        assert!(sitio_en_su_contenedor(&t, &elementos).is_none());
    }

    #[test]
    fn la_caja_crece_lo_justo_para_que_quepa_lo_escrito() {
        // Y la vuelta es consistente: lo que se pide para un ancho tiene que
        // dejar al menos ese ancho de sitio.
        for f in [Figura::Rectangulo, Figura::Elipse, Figura::Rombo] {
            let quiero = 137.0;
            let ancho = figura_que_lo_contiene(quiero, &f);
            let cabe = ancho_que_cabe(&caja(f.clone(), ancho, 100.0));
            assert!(cabe >= quiero, "{f:?}: pide {ancho} y solo caben {cabe}");
        }
    }

    #[test]
    fn el_texto_se_parte_por_palabras_y_respeta_los_saltos_del_usuario() {
        let l = repartir_en_lineas("uno dos tres\ncuatro", 80.0, &diez);
        assert_eq!(l, vec!["uno dos", "tres", "cuatro"]);
    }

    #[test]
    fn una_palabra_que_no_cabe_ella_sola_se_parte_en_vez_de_desbordar() {
        // Un nombre largo en una caja estrecha tiene que verse aunque quede
        // partido: desbordar seria peor.
        let l = repartir_en_lineas("electroencefalografista", 50.0, &diez);
        assert!(l.len() > 1, "no se partio: {l:?}");
        for trozo in &l {
            assert!(diez(trozo) <= 50.0, "«{trozo}» se sale");
        }
        assert_eq!(l.concat(), "electroencefalografista");
    }

    #[test]
    fn una_palabra_con_enes_se_parte_por_caracteres_y_no_por_bytes() {
        // Caso negativo del panico clasico: una «n» con virgulilla ocupa dos
        // bytes, y cortar por el de en medio revienta la cadena.
        let l = repartir_en_lineas("ñañañañaña", 30.0, &diez);
        assert_eq!(l.concat(), "ñañañañaña");
        for trozo in &l {
            assert!(diez(trozo) <= 30.0, "«{trozo}» se sale");
        }
    }

    #[test]
    fn un_texto_vacio_o_sin_sitio_no_se_pierde() {
        assert_eq!(repartir_en_lineas("", 100.0, &diez), vec![""]);
        // Sin sitio no hay nada que repartir: salen los parrafos tal cual, y
        // no una lista vacia que borraria lo escrito.
        assert_eq!(
            repartir_en_lineas("uno\ndos", 0.0, &diez),
            vec!["uno", "dos"]
        );
        assert_eq!(repartir_en_lineas("\n\n", 100.0, &diez), vec!["", "", ""]);
    }

    fn texto_suelto() -> Elemento {
        Elemento {
            figura: Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn atar_deja_el_vinculo_por_los_dos_lados_y_no_lo_duplica() {
        let mut c = caja(Figura::Rectangulo, 200.0, 100.0);
        let mut t = texto_suelto();
        atar(&mut c, "caja-1", &mut t, "rotulo-1");
        assert_eq!(t.extras.contenedor.as_deref(), Some("caja-1"));
        assert_eq!(c.extras.atados.len(), 1);
        assert_eq!(c.extras.atados[0].tipo, "text");
        // Caso negativo: atar dos veces dejaria en el movil dos rotulos
        // dentro de la misma caja.
        atar(&mut c, "caja-1", &mut t, "rotulo-1");
        assert_eq!(c.extras.atados.len(), 1);
    }

    #[test]
    fn se_encuentra_el_rotulo_de_una_caja_y_no_el_de_la_de_al_lado() {
        let mut t = texto_suelto();
        t.extras.contenedor = Some("caja-1".into());
        let mut otro = texto_suelto();
        otro.extras.contenedor = Some("caja-2".into());
        let lista = vec![otro.clone(), t.clone()];
        assert_eq!(
            texto_de("caja-1", &lista).map(|e| &e.extras.contenedor),
            Some(&t.extras.contenedor)
        );
        // Caso negativo: una caja sin rotulo no se queda con el del vecino.
        assert!(texto_de("caja-3", &lista).is_none());
    }

    #[test]
    fn un_rotulo_borrado_no_cuenta_como_rotulo_de_la_caja() {
        // El borrado es logico: si contara, la caja no dejaria escribir otro.
        let mut t = texto_suelto();
        t.extras.contenedor = Some("caja-1".into());
        t.borrado = true;
        assert!(texto_de("caja-1", &[t]).is_none());
    }
}
