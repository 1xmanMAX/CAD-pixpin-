//! Un lector de XML minimo, del tamano justo para un `.docx` y un `.opf`.
//!
//! Se escribe aqui en vez de traer una libreria por dos razones. La primera
//! es el peso: el arbol entero cabe en un fichero y no anade nada al
//! `Cargo.lock`. La segunda es la seguridad, que es la que manda: un
//! documento que llega por el chat **no manda en el disco de nadie**, asi
//! que aqui el `DOCTYPE` se salta entero sin mirarlo y **no existen las
//! entidades declaradas por el documento**; solo se traducen las cinco de
//! siempre y las numericas. Eso cierra de raiz la «bomba de entidades» y la
//! lectura de ficheros por entidad externa, que es de lo poco que un XML
//! puede hacer por su cuenta.
//!
//! El movil hace lo mismo con el analizador de la plataforma y el
//! `disallow-doctype-decl` puesto (`DocxAHtml.kt`, cabecera de `Lector`).
//!
//! Lo que NO hace: validar, resolver espacios de nombres de verdad ni
//! conservar el orden de los atributos repetidos. Aqui todo se busca por
//! **nombre local** —`w:p` es `p`—, que es como esta escrito el Kotlin del
//! movil (`localName`) y lo unico que hace falta para leer un Word.

/// Un elemento con sus hijos. Los textos sueltos van entre los hijos para
/// que un `<w:t>hola<w:tab/>adios</w:t>` no pierda el orden.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Nodo {
    /// Nombre sin prefijo: de `w:p` queda `p`.
    pub nombre: String,
    /// El prefijo, si lo traia: de `w:p` queda `w`. Hace falta para
    /// distinguir `r:embed` de un `embed` a secas.
    pub prefijo: String,
    /// Atributos tal como venian escritos, con su prefijo si lo tenian.
    pub atributos: Vec<(String, String)>,
    pub hijos: Vec<Hijo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hijo {
    Elemento(Nodo),
    Texto(String),
}

impl Nodo {
    /// Los hijos que son elementos, en orden.
    pub fn elementos(&self) -> impl Iterator<Item = &Nodo> {
        self.hijos.iter().filter_map(|h| match h {
            Hijo::Elemento(e) => Some(e),
            Hijo::Texto(_) => None,
        })
    }

    /// El primer hijo directo que se llame asi (por nombre local).
    pub fn hijo(&self, nombre: &str) -> Option<&Nodo> {
        self.elementos().find(|e| e.nombre == nombre)
    }

    /// Todos los descendientes que se llamen asi, en orden de documento.
    ///
    /// Los `.opf` antiguos meten los metadatos dentro de `dc-metadata`, asi
    /// que buscar solo entre los hijos directos se los perderia.
    pub fn descendientes(&self, nombre: &str) -> Vec<&Nodo> {
        let mut salida = Vec::new();
        self.bajar(nombre, &mut salida);
        salida
    }

    fn bajar<'a>(&'a self, nombre: &str, salida: &mut Vec<&'a Nodo>) {
        for e in self.elementos() {
            if e.nombre == nombre {
                salida.push(e);
            }
            e.bajar(nombre, salida);
        }
    }

    /// Un atributo por su nombre local: `getAttribute` del movil.
    pub fn atributo(&self, nombre: &str) -> &str {
        self.atributos
            .iter()
            .find(|(k, _)| local(k) == nombre)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default()
    }

    /// Un atributo exigiendo tambien el prefijo: `r:embed` de una imagen no
    /// es lo mismo que el `embed` de otra cosa.
    pub fn atributo_con_prefijo(&self, prefijo: &str, nombre: &str) -> &str {
        self.atributos
            .iter()
            .find(|(k, _)| local(k) == nombre && prefijo_de(k) == prefijo)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default()
    }

    /// El `w:val`, venga con prefijo o sin el; vacio si no esta. Es como el
    /// movil lee los interruptores de estilo de Word.
    pub fn valor(&self) -> &str {
        self.atributo("val")
    }

    /// Todo el texto que cuelgue de aqui, concatenado.
    pub fn texto(&self) -> String {
        let mut salida = String::new();
        self.juntar_texto(&mut salida);
        salida
    }

    fn juntar_texto(&self, salida: &mut String) {
        for h in &self.hijos {
            match h {
                Hijo::Texto(t) => salida.push_str(t),
                Hijo::Elemento(e) => e.juntar_texto(salida),
            }
        }
    }
}

fn local(nombre: &str) -> &str {
    nombre.rsplit(':').next().unwrap_or(nombre)
}

fn prefijo_de(nombre: &str) -> &str {
    match nombre.split_once(':') {
        Some((p, _)) => p,
        None => "",
    }
}

/// Lee un XML entero. Devuelve `None` si no hay ni un elemento.
///
/// Es tolerante a proposito: una etiqueta que cierra la que no es cierra
/// hasta donde puede y sigue. Un documento de Office real nunca lo necesita,
/// pero uno roto es mejor leerlo a medias que no leerlo.
pub fn leer(texto: &str) -> Option<Nodo> {
    let cs: Vec<char> = texto.chars().collect();
    let mut i = 0;
    // Marca de orden de bytes al principio: no es contenido.
    if cs.first() == Some(&'\u{FEFF}') {
        i = 1;
    }
    let mut pila: Vec<Nodo> = Vec::new();
    let mut raiz: Option<Nodo> = None;

    while i < cs.len() {
        if cs[i] != '<' {
            let inicio = i;
            while i < cs.len() && cs[i] != '<' {
                i += 1;
            }
            if let Some(padre) = pila.last_mut() {
                let crudo: String = cs[inicio..i].iter().collect();
                let t = descodificar(&crudo);
                if !t.is_empty() {
                    padre.hijos.push(Hijo::Texto(t));
                }
            }
            continue;
        }
        // `<!...`: comentario, CDATA o DOCTYPE.
        if cs.get(i + 1) == Some(&'!') {
            if empieza(&cs, i, "<!--") {
                i = tras(&cs, i + 4, "-->").unwrap_or(cs.len());
            } else if empieza(&cs, i, "<![CDATA[") {
                let fin = buscar(&cs, i + 9, "]]>").unwrap_or(cs.len());
                if let Some(padre) = pila.last_mut() {
                    padre
                        .hijos
                        .push(Hijo::Texto(cs[i + 9..fin].iter().collect()));
                }
                i = (fin + 3).min(cs.len());
            } else {
                // DOCTYPE (y cualquier otra declaracion): fuera entera, con
                // su subconjunto interno si lo trae. Ni se mira: aqui no se
                // declaran entidades.
                i = saltar_doctype(&cs, i);
            }
            continue;
        }
        // `<?xml ...?>` y demas instrucciones de proceso.
        if cs.get(i + 1) == Some(&'?') {
            i = tras(&cs, i + 2, "?>").unwrap_or(cs.len());
            continue;
        }
        // `</nombre>`
        if cs.get(i + 1) == Some(&'/') {
            let fin = buscar(&cs, i + 2, ">").unwrap_or(cs.len());
            let nombre: String = cs[i + 2..fin].iter().collect();
            let nombre = local(nombre.trim()).to_string();
            cerrar(&mut pila, &mut raiz, &nombre);
            i = (fin + 1).min(cs.len());
            continue;
        }
        // Una etiqueta de apertura.
        let Some((nodo, suelta, siguiente)) = etiqueta(&cs, i) else {
            // Un `<` que no abre nada es texto: asi lo trata un navegador.
            if let Some(padre) = pila.last_mut() {
                padre.hijos.push(Hijo::Texto("<".into()));
            }
            i += 1;
            continue;
        };
        i = siguiente;
        if suelta {
            if let Some(padre) = pila.last_mut() {
                padre.hijos.push(Hijo::Elemento(nodo));
            } else if raiz.is_none() {
                raiz = Some(nodo);
            }
        } else {
            pila.push(nodo);
        }
    }
    // Lo que quedo abierto se cierra solo.
    while let Some(n) = pila.pop() {
        colocar(&mut pila, &mut raiz, n);
    }
    raiz
}

fn cerrar(pila: &mut Vec<Nodo>, raiz: &mut Option<Nodo>, nombre: &str) {
    // El cierre mas cercano que coincida. Si ninguno coincide, este cierre
    // sobra y se tira: cerrar «el de arriba» a ciegas desmonta el arbol.
    let Some(pos) = pila.iter().rposition(|n| n.nombre == nombre) else {
        return;
    };
    while pila.len() > pos {
        let n = pila.pop().expect("la pila tiene al menos `pos` elementos");
        colocar(pila, raiz, n);
    }
}

fn colocar(pila: &mut [Nodo], raiz: &mut Option<Nodo>, n: Nodo) {
    match pila.last_mut() {
        Some(padre) => padre.hijos.push(Hijo::Elemento(n)),
        None => {
            if raiz.is_none() {
                *raiz = Some(n);
            }
        }
    }
}

/// Lee `<nombre a="1" b='2'/>` desde `i` (que apunta al `<`).
/// Devuelve el nodo, si se cerraba en si misma, y por donde seguir.
fn etiqueta(cs: &[char], i: usize) -> Option<(Nodo, bool, usize)> {
    let mut j = i + 1;
    let inicio = j;
    while j < cs.len() && !cs[j].is_whitespace() && cs[j] != '>' && cs[j] != '/' {
        j += 1;
    }
    if j == inicio {
        return None;
    }
    let entero: String = cs[inicio..j].iter().collect();
    let mut nodo = Nodo {
        nombre: local(&entero).to_string(),
        prefijo: prefijo_de(&entero).to_string(),
        ..Default::default()
    };
    let mut suelta = false;
    loop {
        while j < cs.len() && cs[j].is_whitespace() {
            j += 1;
        }
        if j >= cs.len() {
            return Some((nodo, suelta, j));
        }
        if cs[j] == '>' {
            return Some((nodo, suelta, j + 1));
        }
        if cs[j] == '/' {
            suelta = true;
            j += 1;
            continue;
        }
        let ini = j;
        while j < cs.len() && !cs[j].is_whitespace() && cs[j] != '=' && cs[j] != '>' && cs[j] != '/'
        {
            j += 1;
        }
        let clave: String = cs[ini..j].iter().collect();
        if clave.is_empty() {
            j += 1;
            continue;
        }
        while j < cs.len() && cs[j].is_whitespace() {
            j += 1;
        }
        let mut valor = String::new();
        if cs.get(j) == Some(&'=') {
            j += 1;
            while j < cs.len() && cs[j].is_whitespace() {
                j += 1;
            }
            match cs.get(j) {
                Some(&c @ ('"' | '\'')) => {
                    j += 1;
                    let ini = j;
                    while j < cs.len() && cs[j] != c {
                        j += 1;
                    }
                    valor = descodificar(&cs[ini..j].iter().collect::<String>());
                    j = (j + 1).min(cs.len());
                }
                _ => {
                    let ini = j;
                    while j < cs.len() && !cs[j].is_whitespace() && cs[j] != '>' && cs[j] != '/' {
                        j += 1;
                    }
                    valor = descodificar(&cs[ini..j].iter().collect::<String>());
                }
            }
        }
        nodo.atributos.push((clave, valor));
    }
}

/// Un `<!DOCTYPE ...>` con su `[...]` opcional. Devuelve por donde seguir.
fn saltar_doctype(cs: &[char], i: usize) -> usize {
    let mut j = i + 2;
    let mut dentro = false;
    while j < cs.len() {
        match cs[j] {
            '[' => dentro = true,
            ']' => dentro = false,
            '>' if !dentro => return j + 1,
            _ => {}
        }
        j += 1;
    }
    cs.len()
}

fn empieza(cs: &[char], i: usize, que: &str) -> bool {
    que.chars()
        .enumerate()
        .all(|(k, c)| cs.get(i + k) == Some(&c))
}

fn buscar(cs: &[char], desde: usize, que: &str) -> Option<usize> {
    let n = que.chars().count();
    (desde..cs.len().saturating_sub(n.saturating_sub(1))).find(|&k| empieza(cs, k, que))
}

fn tras(cs: &[char], desde: usize, que: &str) -> Option<usize> {
    buscar(cs, desde, que).map(|k| k + que.chars().count())
}

/// Las cinco entidades de siempre y las numericas. Nada mas: lo que el
/// documento declare en su `DOCTYPE` no existe aqui.
///
/// Una entidad desconocida se deja **tal cual**. Es lo que mas se parece a
/// lo que querian decir, y un `&nbsp;` de un capitulo de EPUB —que llega por
/// el camino de HTML, no por este— ya se traduce en su sitio.
pub fn descodificar(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let cs: Vec<char> = s.chars().collect();
    let mut salida = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if cs[i] != '&' {
            salida.push(cs[i]);
            i += 1;
            continue;
        }
        // Una entidad nunca pasa de unas pocas letras: buscar el `;` hasta
        // el final del documento haria cuadratico un texto lleno de `&`.
        let tope = (i + 12).min(cs.len());
        let Some(fin) = (i + 1..tope).find(|&k| cs[k] == ';') else {
            salida.push('&');
            i += 1;
            continue;
        };
        let cuerpo: String = cs[i + 1..fin].iter().collect();
        match traducir(&cuerpo) {
            Some(c) => salida.push(c),
            None => {
                salida.push('&');
                salida.push_str(&cuerpo);
                salida.push(';');
            }
        }
        i = fin + 1;
    }
    salida
}

fn traducir(cuerpo: &str) -> Option<char> {
    match cuerpo {
        "amp" => return Some('&'),
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "quot" => return Some('"'),
        "apos" => return Some('\''),
        _ => {}
    }
    let numero = cuerpo.strip_prefix('#')?;
    let valor = match numero.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => numero.parse::<u32>().ok()?,
    };
    char::from_u32(valor)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_arbol_con_prefijos_se_busca_por_nombre_local() {
        let n = leer(r#"<w:body><w:p><w:r><w:t>hola</w:t></w:r></w:p></w:body>"#).unwrap();
        assert_eq!(n.nombre, "body");
        assert_eq!(n.prefijo, "w");
        assert_eq!(n.hijo("p").unwrap().texto(), "hola");
    }

    #[test]
    fn el_atributo_se_encuentra_con_prefijo_y_sin_el() {
        let n = leer(r#"<a:blip r:embed="rId7" cstate="print"/>"#).unwrap();
        assert_eq!(n.atributo("embed"), "rId7");
        assert_eq!(n.atributo_con_prefijo("r", "embed"), "rId7");
        assert_eq!(
            n.atributo_con_prefijo("w", "embed"),
            "",
            "el prefijo tiene que contar: `w:embed` no es `r:embed`"
        );
    }

    #[test]
    fn el_doctype_y_sus_entidades_se_saltan_enteros() {
        // La bomba clasica: si el subconjunto interno se leyera, `&lol;` se
        // expandiria. Aqui ni se mira, y la entidad se queda escrita.
        let x = leer(
            r#"<!DOCTYPE a [<!ENTITY lol "aaaa"> <!ENTITY x SYSTEM "file:///c:/secreto">]><a>&lol;</a>"#,
        )
        .unwrap();
        assert_eq!(x.nombre, "a");
        assert_eq!(x.texto(), "&lol;", "una entidad del documento no existe");
    }

    #[test]
    fn el_cdata_y_los_comentarios_no_son_etiquetas() {
        let n = leer(r#"<a><!-- <b>no</b> --><![CDATA[<c>si</c>]]></a>"#).unwrap();
        assert_eq!(n.texto(), "<c>si</c>");
        assert!(n.hijo("b").is_none(), "un comentario no crea nodos");
    }

    #[test]
    fn una_etiqueta_que_cierra_la_que_no_es_no_desmonta_el_arbol() {
        let n = leer("<a><b>uno</i></b><c>dos</c></a>").unwrap();
        assert_eq!(n.hijo("b").map(|e| e.texto()), Some("uno".to_string()));
        assert_eq!(n.hijo("c").map(|e| e.texto()), Some("dos".to_string()));
    }

    #[test]
    fn lo_que_queda_abierto_se_cierra_al_acabar() {
        let n = leer("<a><b>sin cerrar").unwrap();
        assert_eq!(n.texto(), "sin cerrar");
    }

    #[test]
    fn un_texto_que_no_es_xml_no_da_arbol() {
        assert!(leer("").is_none());
        assert!(leer("solo palabras, ni un signo de menor").is_none());
        assert!(leer("   \n  ").is_none());
    }

    #[test]
    fn las_entidades_numericas_y_las_cinco_de_siempre_se_traducen() {
        let n = leer(r#"<a>5 &lt; 6 &amp;&amp; &#65;&#x42; &loquesea;</a>"#).unwrap();
        assert_eq!(n.texto(), "5 < 6 && AB &loquesea;");
    }

    #[test]
    fn los_descendientes_bajan_mas_de_un_nivel() {
        let n =
            leer("<m><dc-metadata><title>Uno</title></dc-metadata><title>Dos</title></m>").unwrap();
        let t: Vec<String> = n.descendientes("title").iter().map(|e| e.texto()).collect();
        assert_eq!(t, vec!["Uno".to_string(), "Dos".to_string()]);
    }

    #[test]
    fn un_atributo_sin_comillas_tambien_se_lee() {
        let n = leer("<a href=uno.html id=x>t</a>").unwrap();
        assert_eq!(n.atributo("href"), "uno.html");
        assert_eq!(n.atributo("id"), "x");
    }
}
