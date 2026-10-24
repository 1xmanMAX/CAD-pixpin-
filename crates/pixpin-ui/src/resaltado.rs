//! Buscar dentro de una conversacion, y decir DONDE esta lo encontrado.
//!
//! Logica pura, como el resto del crate: aqui no se pinta nada, solo se
//! calculan los tramos que quien pinta tiene que resaltar. Es el mismo
//! reparto que en PixPin Android (`Resaltado.kt`), y por la misma razon:
//! buscar es lo que mas se nota cuando falla, asi que tiene que poder
//! probarse entero sin ventana.
//!
//! Tres decisiones que no son evidentes:
//!
//! - **Se busca sin mirar mayusculas NI acentos.** En espanol escribir
//!   «cemento» y no encontrar «Cemento», o escribir «arana» y no encontrar
//!   «araña», es lo que hunde una busqueda. Se normaliza a mano, con una
//!   tabla pequena, porque no se anaden dependencias por esto.
//! - **Los indices son del texto ORIGINAL**, con sus tildes, para que quien
//!   pinta pueda cortar la cadena tal cual la ensena. Normalizar cambia el
//!   largo en bytes (`á` ocupa dos y `a` uno), asi que los tramos NO se
//!   calculan sobre el normalizado: se lleva aparte donde empieza cada
//!   caracter del original.
//! - **La aguja en blanco no coincide con nada**, no con todo. Al empezar a
//!   escribir, la caja esta vacia un instante; resaltar la conversacion
//!   entera en ese instante es un parpadeo feo y ademas no dice nada.

/// Un trozo del texto que hay que resaltar.
///
/// `desde` y `hasta` van en indices de BYTE dentro de la cadena original,
/// que es lo que necesita quien pinta para hacer `&texto[desde..hasta]` sin
/// recorrerla entera. Siempre caen en limite de caracter UTF-8, de modo que
/// ese corte nunca puede entrar en panico ni partir un emoji por la mitad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tramo {
    pub desde: usize,
    pub hasta: usize,
}

impl Tramo {
    /// Lo que ocupa en bytes del original.
    pub fn largo(&self) -> usize {
        self.hasta - self.desde
    }

    /// El mismo tramo contado en unidades UTF-16: `(inicio, longitud)`.
    ///
    /// Hace falta porque quien pinta es DirectWrite, y DirectWrite cuenta en
    /// UTF-16. Los dos numeros coinciden mientras todo sea ASCII, asi que un
    /// error aqui no se ve hasta que aparece una tilde —o peor, un emoji,
    /// que ocupa DOS unidades UTF-16 y cuatro bytes—: por eso se convierte
    /// de verdad en vez de pasar los bytes y confiar.
    ///
    /// Si el tramo no cae dentro del texto devuelve longitud cero: resaltar
    /// nada es preferible a resaltar un trozo que no es.
    pub fn en_utf16(&self, texto: &str) -> (u32, u32) {
        if self.hasta > texto.len()
            || self.desde > self.hasta
            || !texto.is_char_boundary(self.desde)
            || !texto.is_char_boundary(self.hasta)
        {
            return (0, 0);
        }
        let inicio = texto[..self.desde].encode_utf16().count() as u32;
        let largo = texto[self.desde..self.hasta].encode_utf16().count() as u32;
        (inicio, largo)
    }
}

/// Deja un caracter en su forma «para comparar»: minuscula y sin tilde.
///
/// Lo que cubre: minusculas de cualquier alfabeto (por `to_lowercase`), las
/// cinco vocales espanolas con tilde, dieresis, acento grave y circunflejo,
/// la enne, la ce con cedilla y algun vecino del portugues y el catalan, que
/// salen en nombres de obra y de persona a diario.
///
/// Lo que NO cubre, a proposito: los acentos escritos aparte (la `a` seguida
/// del acento combinante U+0301) no se juntan, porque hacerlo bien es
/// normalizacion Unicode entera y eso son dependencias y tablas enormes;
/// Windows entrega el texto ya compuesto. Tampoco se deshacen ligaduras
/// (`æ`, `ß`), ni se traducen alfabetos entre si.
///
/// Efecto secundario asumido: si `ñ` vale `n`, buscar «cana» tambien saca
/// «caña». Preferimos ese exceso al fallo contrario, que es que quien no
/// tiene la enne a mano no encuentre nada.
fn normalizar(caracter: char) -> char {
    // Primero a minuscula. Se coge solo el primer caracter porque esta tabla
    // tiene que ser de uno a uno: si una minuscula ocupase dos caracteres se
    // descuadrarian los indices del original.
    let bajo = caracter.to_lowercase().next().unwrap_or(caracter);
    match bajo {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        otro => otro,
    }
}

/// El texto partido en caracteres normalizados, y donde empieza cada uno en
/// el original. `limites` tiene un elemento de mas: el final de la cadena,
/// para poder cerrar un tramo que llegue hasta el ultimo caracter.
fn desmenuzar(texto: &str) -> (Vec<char>, Vec<usize>) {
    let mut letras = Vec::with_capacity(texto.len());
    let mut limites = Vec::with_capacity(texto.len() + 1);
    for (posicion, caracter) in texto.char_indices() {
        letras.push(normalizar(caracter));
        limites.push(posicion);
    }
    limites.push(texto.len());
    (letras, limites)
}

fn buscar(texto: &str, aguja: &str, parar_en_la_primera: bool) -> Vec<Tramo> {
    // La aguja en blanco no coincide con nada; ver la cabecera del modulo.
    if aguja.trim().is_empty() {
        return Vec::new();
    }
    let (letras, limites) = desmenuzar(texto);
    let buscada: Vec<char> = aguja.chars().map(normalizar).collect();
    let cuantas = buscada.len();
    if cuantas > letras.len() {
        return Vec::new();
    }

    let mut tramos = Vec::new();
    let mut i = 0;
    while i + cuantas <= letras.len() {
        if letras[i..i + cuantas] == buscada[..] {
            tramos.push(Tramo {
                desde: limites[i],
                hasta: limites[i + cuantas],
            });
            if parar_en_la_primera {
                return tramos;
            }
            // Se salta lo encontrado entero: dos resaltados que se pisan se
            // pintarian uno encima del otro y se veria mas oscuro un trozo.
            i += cuantas;
        } else {
            i += 1;
        }
    }
    tramos
}

/// Todas las apariciones de `aguja` en `texto`, de izquierda a derecha y sin
/// solaparse, en indices de byte del original.
pub fn coincidencias(texto: &str, aguja: &str) -> Vec<Tramo> {
    buscar(texto, aguja, false)
}

/// Si `aguja` aparece en `texto`. Para filtrar una lista larga, donde solo
/// hace falta el si o el no y calcular todos los tramos seria tirar trabajo.
pub fn hay_coincidencia(texto: &str, aguja: &str) -> bool {
    !buscar(texto, aguja, true).is_empty()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Lo que quien pinta va a recortar de verdad.
    fn recortes<'a>(texto: &'a str, aguja: &str) -> Vec<&'a str> {
        coincidencias(texto, aguja)
            .into_iter()
            .map(|t| &texto[t.desde..t.hasta])
            .collect()
    }

    #[test]
    fn da_igual_como_se_escriban_las_mayusculas() {
        assert_eq!(recortes("Cemento para el muro", "cemento"), ["Cemento"]);
        assert_eq!(recortes("cemento para el muro", "CEMENTO"), ["cemento"]);
        assert_eq!(recortes("CeMeNtO", "cEmEnTo"), ["CeMeNtO"]);
    }

    #[test]
    fn da_igual_si_las_tildes_se_escriben_o_no() {
        // Sin tilde en la caja, con tilde en el texto.
        assert_eq!(recortes("viaje a Perú en mayo", "peru"), ["Perú"]);
        // Con tilde en la caja, sin tilde en el texto.
        assert_eq!(recortes("viaje a Peru en mayo", "perú"), ["Peru"]);
        // Y la enne y la dieresis, que es lo que nadie escribe con prisa.
        assert_eq!(recortes("una araña en la viga", "arana"), ["araña"]);
        assert_eq!(recortes("pinguino", "pingüino"), ["pinguino"]);
        assert_eq!(recortes("cigüeñal roto", "ciguenal"), ["cigüeñal"]);
    }

    #[test]
    fn el_tramo_recortado_del_original_sale_con_sus_tildes_puestas() {
        // Esta es la prueba del requisito: normalizar acorta la cadena en
        // bytes, y aun asi el tramo tiene que valer sobre la original.
        let texto = "el niño midió la fachada";
        let tramos = coincidencias(texto, "midio");
        assert_eq!(tramos.len(), 1);
        assert_eq!(&texto[tramos[0].desde..tramos[0].hasta], "midió");
        assert_eq!(tramos[0].largo(), "midió".len(), "seis bytes, cinco letras");
        // Y con la palabra al final del todo, que es donde se sale uno.
        let al_final = "la pared esta al revés";
        let ultimo = coincidencias(al_final, "reves");
        assert_eq!(&al_final[ultimo[0].desde..ultimo[0].hasta], "revés");
        assert_eq!(ultimo[0].hasta, al_final.len());
    }

    #[test]
    fn normalizar_no_cambia_el_numero_de_caracteres() {
        // De esto depende que los indices del original cuadren. Si algun dia
        // alguien mete en la tabla algo que se abre en dos caracteres, esta
        // prueba se pone roja antes de que se vean resaltados torcidos.
        for texto in ["áéíóúüñçÁÉÍÓÚÜÑÇ", "hola", "👋🏽ß\u{130}", ""] {
            let normalizado: String = texto.chars().map(normalizar).collect();
            assert_eq!(
                normalizado.chars().count(),
                texto.chars().count(),
                "{texto:?}"
            );
        }
    }

    #[test]
    fn salen_todas_las_apariciones_y_no_se_pisan_entre_ellas() {
        assert_eq!(
            recortes("cemento, mas cemento y otro CEMENTO", "cemento"),
            ["cemento", "cemento", "CEMENTO"]
        );
        // El caso que se solaparia: «aaaa» tiene tres sitios donde empieza
        // «aa», pero solo caben dos resaltados sin pisarse.
        let tramos = coincidencias("aaaa", "aa");
        assert_eq!(tramos.len(), 2);
        assert_eq!(tramos[0].hasta, tramos[1].desde);
        // Y en orden, sin huecos raros.
        for par in tramos.windows(2) {
            assert!(par[0].hasta <= par[1].desde);
        }
    }

    #[test]
    fn lo_que_no_esta_no_se_encuentra() {
        assert!(coincidencias("hormigon armado", "ladrillo").is_empty());
        assert!(!hay_coincidencia("hormigon armado", "ladrillo"));
        // Una aguja mas larga que el texto tampoco, aunque empiece igual.
        assert!(!hay_coincidencia("hormi", "hormigon"));
        // Y las letras sueltas no valen: se busca la cadena entera seguida.
        assert!(!hay_coincidencia("h o r m i g o n", "hormigon"));
    }

    #[test]
    fn una_aguja_vacia_o_en_blanco_no_coincide_con_nada() {
        for aguja in ["", " ", "   ", "\t", "\n "] {
            assert!(
                coincidencias("cualquier cosa", aguja).is_empty(),
                "{aguja:?}"
            );
            assert!(!hay_coincidencia("cualquier cosa", aguja), "{aguja:?}");
        }
        // Ni siquiera contra el texto vacio, donde «todo» y «nada» se
        // confunden facil.
        assert!(coincidencias("", "").is_empty());
        assert!(coincidencias("", "algo").is_empty());
    }

    #[test]
    fn los_emojis_no_se_parten_por_la_mitad() {
        let texto = "obra 👷🏽‍♀️ acabada 🧱 hoy";
        for aguja in ["acabada", "hoy", "obra", "🧱", "a"] {
            for tramo in coincidencias(texto, aguja) {
                assert!(texto.is_char_boundary(tramo.desde), "{aguja} {tramo:?}");
                assert!(texto.is_char_boundary(tramo.hasta), "{aguja} {tramo:?}");
            }
        }
        // Buscar el ladrillo saca el ladrillo entero, no medio.
        assert_eq!(recortes(texto, "🧱"), ["🧱"]);
        // Y una palabra pegada a un emoji se recorta sin llevarselo.
        assert_eq!(recortes(texto, "ACABADA"), ["acabada"]);
    }

    #[test]
    fn hay_coincidencia_dice_lo_mismo_que_coincidencias() {
        let casos = [
            ("Cemento y arena", "cemento"),
            ("Cemento y arena", "cal"),
            ("araña", "arana"),
            ("araña", ""),
            ("", "a"),
        ];
        for (texto, aguja) in casos {
            assert_eq!(
                hay_coincidencia(texto, aguja),
                !coincidencias(texto, aguja).is_empty(),
                "{texto:?} / {aguja:?}"
            );
        }
    }

    #[test]
    fn se_puede_buscar_un_trozo_con_espacios_dentro() {
        // La aguja solo se rechaza si esta en blanco ENTERA; una con espacios
        // en medio es una busqueda legitima de dos palabras seguidas.
        assert_eq!(
            recortes("hay que llamar al Aparejador manana", "al aparejador"),
            ["al Aparejador"]
        );
        // Pero los espacios cuentan: no se buscan las palabras por separado.
        assert!(!hay_coincidencia("aparejador al fin", "al aparejador"));
    }

    #[test]
    fn los_tramos_se_cuentan_tambien_en_utf16_para_quien_pinta() {
        // En ASCII los dos numeros coinciden, y ahi no se ve nada raro.
        let texto = "muro norte";
        let t = coincidencias(texto, "norte")[0];
        assert_eq!(t.en_utf16(texto), (5, 5));

        // Con tildes ya no: «Cimentación» ocupa doce bytes y once unidades
        // UTF-16, asi que pasar bytes moveria el resaltado.
        let texto = "Cimentación del muro";
        let t = coincidencias(texto, "del")[0];
        assert_eq!(t.desde, 13, "en bytes, despues de la o acentuada");
        assert_eq!(t.en_utf16(texto), (12, 3));

        // Y un emoji ocupa dos unidades UTF-16 aunque sea un solo caracter.
        let texto = "🧱 ladrillo";
        let t = coincidencias(texto, "ladrillo")[0];
        assert_eq!(t.en_utf16(texto), (3, 8), "el emoji cuenta por dos");
    }

    #[test]
    fn un_tramo_que_no_es_de_ese_texto_no_resalta_nada() {
        // Caso negativo: lo que cae fuera devuelve largo cero en vez de
        // resaltar un trozo cualquiera.
        let fuera = Tramo {
            desde: 3,
            hasta: 99,
        };
        assert_eq!(fuera.en_utf16("corto"), (0, 0));
        // Y uno que parte un caracter por la mitad, tampoco.
        let partido = Tramo { desde: 1, hasta: 2 };
        assert_eq!(partido.en_utf16("ñu"), (0, 0), "la ñ ocupa dos bytes");
    }
}
