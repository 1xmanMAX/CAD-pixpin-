//! **Buscar dentro del documento** (D9): la caja de Ctrl+F de cualquier
//! navegador, sobre el texto que el lector ya tiene.
//!
//! El movil no tiene esto todavia (es el punto 12 de su hoja de ruta), asi
//! que se copia lo que hace el buscador de un navegador, que es lo que el
//! usuario ya conoce: **sin distinguir mayusculas ni tildes** («arbol»
//! encuentra «Árbol»), todas las coincidencias marcadas, la actual mas
//! fuerte, «3 de 12», Intro/F3 a la siguiente y Mayus a la anterior, dando
//! la vuelta al llegar al final.
//!
//! Es puro a proposito: recibe textos y devuelve donde esta cada
//! coincidencia. El lector de Word y libros le pasa sus bloques ya
//! colocados y el de PDF el texto de cada pagina (`pixpin_pdf::texto`);
//! pintar y saltar es cosa de ellos.
//!
//! **Las posiciones van en letras (`char`), no en bytes**: un bloque con
//! acentos no puede partir una letra por la mitad. Quien pinta con
//! DirectWrite las pasa a UTF-16 con [`a_utf16`].

/// Donde esta una coincidencia: en que texto de la lista, y desde que letra
/// cuantas letras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coincidencia {
    pub texto: usize,
    pub inicio: usize,
    pub largo: usize,
}

/// Una letra tal como se compara: en minuscula y sin su tilde. Una letra da
/// siempre UNA letra, para que las posiciones del texto plegado sean las del
/// original sin llevar ninguna cuenta aparte.
///
/// La ñ se pliega a n como en el buscador de Chrome y Edge: quien teclea
/// «espana» en un teclado sin ñ espera encontrar «España».
pub fn plegar(c: char) -> char {
    if c.is_whitespace() {
        // El espacio duro de un Word y el tabulador cuentan como espacio: se
        // teclea un espacio y se espera encontrar la frase.
        return ' ';
    }
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ę' | 'ė' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'ī' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'ū' => 'u',
        'ñ' | 'ń' => 'n',
        'ç' | 'ć' | 'č' => 'c',
        'ý' | 'ÿ' => 'y',
        'š' | 'ś' => 's',
        'ž' | 'ź' | 'ż' => 'z',
        // Las comillas y los guiones «tipograficos» de un Word frente a los
        // del teclado: se buscan con los del teclado.
        '\u{2018}' | '\u{2019}' | '\u{00B4}' => '\'',
        '\u{201C}' | '\u{201D}' | '\u{00AB}' | '\u{00BB}' => '"',
        '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' => '-',
        otra => otra,
    }
}

fn plegado(texto: &str) -> Vec<char> {
    texto.chars().map(plegar).collect()
}

/// **Todas las coincidencias** de `consulta` en `textos`, en orden de
/// lectura. Sin solaparse: «aa» en «aaaa» son dos, como en el navegador.
///
/// Una consulta vacia o solo de espacios no encuentra nada: marcar cada
/// espacio del libro no le sirve a nadie.
pub fn buscar<'a>(textos: impl IntoIterator<Item = &'a str>, consulta: &str) -> Vec<Coincidencia> {
    let aguja = plegado(consulta.trim());
    let mut salida = Vec::new();
    if aguja.is_empty() {
        return salida;
    }
    for (t, texto) in textos.into_iter().enumerate() {
        let pajar = plegado(texto);
        if pajar.len() < aguja.len() {
            continue;
        }
        let mut i = 0;
        while i + aguja.len() <= pajar.len() {
            if pajar[i..i + aguja.len()] == aguja[..] {
                salida.push(Coincidencia {
                    texto: t,
                    inicio: i,
                    largo: aguja.len(),
                });
                i += aguja.len();
            } else {
                i += 1;
            }
        }
    }
    salida
}

/// De una posicion en letras a una en unidades UTF-16 (lo que cuenta
/// DirectWrite). Mas alla del final da el largo entero.
pub fn a_utf16(texto: &str, letras: usize) -> usize {
    texto.chars().take(letras).map(char::len_utf16).sum()
}

/// **La busqueda en curso**: lo escrito, lo encontrado y cual es la actual.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Busqueda {
    pub consulta: String,
    pub coincidencias: Vec<Coincidencia>,
    pub actual: Option<usize>,
}

impl Busqueda {
    /// Vuelve a buscar con lo escrito ahora. La actual pasa a ser **la
    /// primera desde donde se esta leyendo** (`desde_texto`), como en el
    /// navegador: escribir no te manda al principio del libro.
    pub fn rehacer<'a>(&mut self, textos: impl IntoIterator<Item = &'a str>, desde_texto: usize) {
        self.coincidencias = buscar(textos, &self.consulta);
        self.actual = if self.coincidencias.is_empty() {
            None
        } else {
            Some(
                self.coincidencias
                    .iter()
                    .position(|c| c.texto >= desde_texto)
                    .unwrap_or(0),
            )
        };
    }

    /// A la siguiente, y del final vuelta a la primera.
    pub fn siguiente(&mut self) -> Option<Coincidencia> {
        let n = self.coincidencias.len();
        if n == 0 {
            return None;
        }
        let i = self.actual.map_or(0, |i| (i + 1) % n);
        self.actual = Some(i);
        self.coincidencias.get(i).copied()
    }

    /// A la anterior, y de la primera vuelta a la ultima.
    pub fn anterior(&mut self) -> Option<Coincidencia> {
        let n = self.coincidencias.len();
        if n == 0 {
            return None;
        }
        let i = self.actual.map_or(n - 1, |i| (i + n - 1) % n);
        self.actual = Some(i);
        self.coincidencias.get(i).copied()
    }

    pub fn la_actual(&self) -> Option<Coincidencia> {
        self.actual.and_then(|i| self.coincidencias.get(i).copied())
    }

    /// «3 de 12», contando desde uno. `None` sin coincidencias.
    pub fn cuenta(&self) -> Option<(usize, usize)> {
        Some((self.actual? + 1, self.coincidencias.len()))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_encuentra_sin_mirar_mayusculas_ni_tildes() {
        let textos = ["El Árbol de la ciencia", "un arbol", "ARBOLEDA"];
        let c = buscar(textos, "árbol");
        assert_eq!(c.len(), 3);
        assert_eq!(
            c[0],
            Coincidencia {
                texto: 0,
                inicio: 3,
                largo: 5
            }
        );
        assert_eq!(c[1].texto, 1);
        assert_eq!(
            c[2],
            Coincidencia {
                texto: 2,
                inicio: 0,
                largo: 5
            }
        );
        // Y al reves: escribir con tilde encuentra lo escrito sin ella.
        assert_eq!(buscar(["arbol"], "ÁRBOL").len(), 1);
    }

    #[test]
    fn la_enie_y_las_comillas_de_word_se_encuentran_con_el_teclado() {
        assert_eq!(buscar(["Viva España"], "espana").len(), 1);
        assert_eq!(buscar(["dijo \u{201C}hola\u{201D}"], "\"hola\"").len(), 1);
        assert_eq!(buscar(["2010\u{2013}2020"], "2010-2020").len(), 1);
        assert_eq!(buscar(["cien\u{00A0}euros"], "cien euros").len(), 1);
    }

    #[test]
    fn las_posiciones_son_letras_y_no_bytes() {
        // «ñ» son dos bytes: una posicion en bytes pondria la marca una
        // letra mas alla.
        let c = buscar(["año nuevo"], "nuevo");
        assert_eq!(c[0].inicio, 4);
        assert_eq!(a_utf16("año nuevo", 4), 4);
        // Un emoticono son dos unidades UTF-16: lo que va detras se corre.
        assert_eq!(a_utf16("🖼 imagen", 2), 3);
        assert_eq!(a_utf16("ab", 99), 2, "mas alla del final, el largo entero");
    }

    #[test]
    fn las_coincidencias_no_se_pisan() {
        assert_eq!(buscar(["aaaa"], "aa").len(), 2);
        assert_eq!(buscar(["aaa"], "aa").len(), 1);
    }

    #[test]
    fn nada_escrito_o_nada_parecido_no_encuentra_nada() {
        assert!(buscar(["hola"], "").is_empty());
        assert!(buscar(["hola que tal"], "   ").is_empty());
        assert!(buscar(["hola"], "adios").is_empty());
        assert!(
            buscar(["ho"], "hola").is_empty(),
            "una aguja mas larga que el texto"
        );
        let vacio: [&str; 0] = [];
        assert!(buscar(vacio, "hola").is_empty());
    }

    #[test]
    fn siguiente_y_anterior_dan_la_vuelta() {
        let textos = ["uno x", "dos x", "tres x"];
        let mut b = Busqueda {
            consulta: "x".into(),
            ..Default::default()
        };
        b.rehacer(textos, 0);
        assert_eq!(b.cuenta(), Some((1, 3)));
        assert_eq!(b.siguiente().map(|c| c.texto), Some(1));
        assert_eq!(b.siguiente().map(|c| c.texto), Some(2));
        assert_eq!(
            b.siguiente().map(|c| c.texto),
            Some(0),
            "del final a la primera"
        );
        assert_eq!(
            b.anterior().map(|c| c.texto),
            Some(2),
            "de la primera a la ultima"
        );
        assert_eq!(b.cuenta(), Some((3, 3)));
    }

    #[test]
    fn al_escribir_la_actual_es_la_primera_desde_donde_se_lee() {
        let textos = ["x", "nada", "x", "x"];
        let mut b = Busqueda {
            consulta: "x".into(),
            ..Default::default()
        };
        b.rehacer(textos, 1);
        assert_eq!(b.la_actual().map(|c| c.texto), Some(2));
        // Leyendo pasada la ultima, se vuelve a la primera.
        b.rehacer(textos, 9);
        assert_eq!(b.la_actual().map(|c| c.texto), Some(0));
    }

    #[test]
    fn sin_coincidencias_no_hay_actual_ni_cuenta_ni_salto() {
        let mut b = Busqueda {
            consulta: "zzz".into(),
            ..Default::default()
        };
        b.rehacer(["hola"], 0);
        assert_eq!(b.cuenta(), None);
        assert_eq!(b.siguiente(), None);
        assert_eq!(b.anterior(), None);
    }

    #[test]
    fn buscar_en_un_libro_grande_es_rapido() {
        // Un libro de unas 400 paginas: dos millones de letras.
        let parrafo = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho tiempo que vivía un hidalgo. ";
        let textos: Vec<String> = (0..20_000).map(|_| parrafo.to_string()).collect();
        let t0 = std::time::Instant::now();
        let c = buscar(textos.iter().map(String::as_str), "hidalgo");
        let ms = t0.elapsed().as_millis();
        assert_eq!(c.len(), 20_000);
        eprintln!("buscar en {} letras: {ms} ms", parrafo.len() * 20_000);
        // En modo de pruebas (sin optimizar) y en la maquina suelo sigue
        // siendo una pulsacion: se comprueba holgado.
        assert!(ms < 3000, "{ms} ms");
    }
}
