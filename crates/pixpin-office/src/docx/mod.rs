//! **Un `.docx` paginado como en Word.**
//!
//! Abrir lee el ZIP, los estilos, la numeracion, el tema y el cuerpo (como
//! arbol, sin maquetar). Despues cada [`DocumentoWord::pagina`] maqueta lo
//! justo para tener esa pagina: los bloques se convierten en franjas
//! (`bloques.rs`) y el paginador de aqui las va dejando caer en columnas y
//! paginas, con sus encabezados, pies, notas y dibujos anclados.

pub mod bloques;
pub mod dibujo;
pub mod estilos;
pub mod mate;
pub mod numeracion;
pub mod rodear;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use pixpin_docs::xml::Nodo;

use crate::medidor::Medidor;
use crate::pagina::{Color, Elemento, Pagina, Recursos, Relleno, Trazo};
use crate::paquete::{ErrorOffice, Paquete, Relaciones, attr_num, twips};
use crate::tema::Tema;
use bloques::{Caja, Ctx, Franja, Salto};
use dibujo::{Flotante, Referencia};
use estilos::Estilos;
use numeracion::Numeracion;
use rodear::Obstaculo;

/// Una seccion de Word (`w:sectPr`): tamano de hoja, margenes, columnas,
/// encabezados y pies.
#[derive(Debug, Clone)]
pub struct Seccion {
    pub ancho: f32,
    pub alto: f32,
    /// Arriba, derecha, abajo, izquierda.
    pub margen: [f32; 4],
    pub cabecera: f32,
    pub pie: f32,
    /// (x desde el margen izquierdo, ancho) de cada columna.
    pub columnas: Vec<(f32, f32)>,
    /// `nextPage`, `continuous`, `evenPage`, `oddPage`, `nextColumn`.
    pub tipo: String,
    /// rId de encabezados: por defecto, primera pagina, pares.
    pub cabeceras: [Option<String>; 3],
    pub pies: [Option<String>; 3],
    pub titulo: bool,
    pub inicio: Option<u32>,
    /// Bordes de pagina (`w:pgBorders`): arriba, derecha, abajo, izquierda,
    /// con su distancia, y si se miden desde el borde de la hoja.
    pub bordes: [Option<(Trazo, f32)>; 4],
    pub bordes_desde_hoja: bool,
}

impl Seccion {
    fn leer(n: Option<&Nodo>) -> Seccion {
        let mut s = Seccion {
            ancho: 612.0,
            alto: 792.0,
            margen: [72.0; 4],
            cabecera: 36.0,
            pie: 36.0,
            columnas: Vec::new(),
            tipo: "nextPage".into(),
            cabeceras: [None, None, None],
            pies: [None, None, None],
            titulo: false,
            inicio: None,
            bordes: [None; 4],
            bordes_desde_hoja: false,
        };
        let mut cols = (1usize, 36.0f32, Vec::<(f32, f32)>::new());
        if let Some(n) = n {
            for e in n.elementos() {
                match e.nombre.as_str() {
                    "pgSz" => {
                        if let Some(w) = attr_num(e, "w", 20.0) {
                            s.ancho = twips(w);
                        }
                        if let Some(h) = attr_num(e, "h", 20.0) {
                            s.alto = twips(h);
                        }
                    }
                    "pgMar" => {
                        let m = |k: &str, d: f32| attr_num(e, k, 20.0).map(twips).unwrap_or(d);
                        let gutter = m("gutter", 0.0);
                        s.margen = [
                            m("top", 72.0).abs(),
                            m("right", 72.0),
                            m("bottom", 72.0).abs(),
                            m("left", 72.0) + gutter,
                        ];
                        s.cabecera = m("header", 36.0);
                        s.pie = m("footer", 36.0);
                    }
                    "cols" => {
                        cols.0 = e.atributo("num").parse().unwrap_or(1).max(1);
                        cols.1 = attr_num(e, "space", 20.0).map(twips).unwrap_or(36.0);
                        let iguales = !matches!(e.atributo("equalWidth"), "0" | "false");
                        if !iguales {
                            cols.2 = e
                                .elementos()
                                .filter(|c| c.nombre == "col")
                                .map(|c| {
                                    (
                                        attr_num(c, "w", 20.0).map(twips).unwrap_or(0.0),
                                        attr_num(c, "space", 20.0).map(twips).unwrap_or(0.0),
                                    )
                                })
                                .collect();
                        }
                    }
                    "type" => s.tipo = e.valor().to_string(),
                    "titlePg" => s.titulo = crate::paquete::interruptor(e),
                    "pgBorders" => {
                        s.bordes_desde_hoja = e.atributo("offsetFrom") == "page";
                        for b in e.elementos() {
                            let i = match b.nombre.as_str() {
                                "top" => 0,
                                "right" => 1,
                                "bottom" => 2,
                                "left" => 3,
                                _ => continue,
                            };
                            let espacio = attr_num(b, "space", 1.0).unwrap_or(0.0);
                            // Los bordes artisticos (`mapleMuffins`...) se
                            // dibujan como una raya: su `sz` va en puntos.
                            let arte = !matches!(
                                b.valor(),
                                "single"
                                    | "double"
                                    | "thick"
                                    | "dotted"
                                    | "dashed"
                                    | "dotDash"
                                    | "dotDotDash"
                                    | "triple"
                                    | "thinThickSmallGap"
                                    | "thickThinSmallGap"
                                    | "nil"
                                    | "none"
                            );
                            let t = estilos::borde(b, &Tema::default()).map(|mut t| {
                                if arte {
                                    t.grosor = 1.5;
                                    t.color = Color::rgb(0x60, 0x40, 0x20);
                                }
                                t
                            });
                            s.bordes[i] = t.map(|t| (t, espacio));
                        }
                    }
                    "pgNumType" => s.inicio = e.atributo("start").parse().ok(),
                    "headerReference" | "footerReference" => {
                        let i = match e.atributo("type") {
                            "first" => 1,
                            "even" => 2,
                            _ => 0,
                        };
                        let rid = Some(e.atributo_con_prefijo("r", "id").to_string());
                        if e.nombre == "headerReference" {
                            s.cabeceras[i] = rid;
                        } else {
                            s.pies[i] = rid;
                        }
                    }
                    _ => {}
                }
            }
        }
        let util = (s.ancho - s.margen[1] - s.margen[3]).max(36.0);
        if !cols.2.is_empty() {
            let mut x = 0.0;
            for (w, esp) in &cols.2 {
                s.columnas.push((x, *w));
                x += w + esp;
            }
        } else {
            let n = cols.0.min(20);
            let w = ((util - cols.1 * (n - 1) as f32) / n as f32).max(18.0);
            for i in 0..n {
                s.columnas.push((i as f32 * (w + cols.1), w));
            }
        }
        s
    }

    fn util(&self) -> f32 {
        (self.ancho - self.margen[1] - self.margen[3]).max(36.0)
    }
}

/// Un encabezado o pie leido.
struct Parte {
    bloques: Vec<Nodo>,
    rels: Arc<Relaciones>,
}

/// Lo leido del documento (sin estado de paginacion).
struct Datos {
    paquete: Paquete,
    estilos: Estilos,
    tema: Tema,
    numeracion: Numeracion,
    rels: Arc<Relaciones>,
    recursos: Arc<Recursos>,
    /// Los bloques del cuerpo ya leidos (se leen segun hacen falta).
    bloques: Vec<Nodo>,
    seccion_de: Vec<usize>,
    secciones: Vec<Seccion>,
    /// El `document.xml` en texto y por donde va la lectura del cuerpo.
    fuente: String,
    cursor: usize,
    cuerpo_acabado: bool,
    /// La seccion en que cae el siguiente bloque que se lea.
    seccion_lectura: usize,
    tope_defecto: f32,
    compat: u32,
    pares_impares: bool,
    total_estimado: usize,
    partes: HashMap<String, Arc<Parte>>,
    notas: HashMap<String, Arc<Vec<Nodo>>>,
    notas_rels: Arc<Relaciones>,
    notas_leidas: bool,
    contador_notas: u32,
    fondo: Option<Color>,
    alternas: HashMap<String, String>,
}

impl Datos {
    fn parte(&mut self, rid: &str) -> Option<Arc<Parte>> {
        if let Some(p) = self.partes.get(rid) {
            return Some(p.clone());
        }
        let rel = self.rels.get(rid)?.clone();
        let raiz = self.paquete.xml(&rel.destino)?;
        let rels = self.paquete.relaciones(&rel.destino);
        let bloques = raiz
            .hijos
            .into_iter()
            .filter_map(|h| {
                if let pixpin_docs::xml::Hijo::Elemento(e) = h {
                    Some(e)
                } else {
                    None
                }
            })
            .collect();
        let p = Arc::new(Parte {
            bloques,
            rels: Arc::new(rels),
        });
        self.partes.insert(rid.to_string(), p.clone());
        Some(p)
    }

    /// Lee del `document.xml` hasta tener `n` bloques (o acabar el
    /// cuerpo).
    fn leer_bloques(&mut self, n: usize) {
        while self.bloques.len() < n && !self.cuerpo_acabado {
            let mut hijos = crate::xml::Hijos::nuevo(&self.fuente, self.cursor);
            let siguiente = hijos.siguiente();
            self.cursor = hijos.pos;
            let Some((e, _)) = siguiente else {
                self.cuerpo_acabado = true;
                break;
            };
            if e.nombre == "sectPr" {
                continue;
            }
            let desde = self.bloques.len();
            aplanar_en(e, &mut self.bloques);
            let ultima = self.secciones.len().saturating_sub(1);
            for k in desde..self.bloques.len() {
                self.seccion_de.push(self.seccion_lectura.min(ultima));
                if self.bloques[k].nombre == "p"
                    && self.bloques[k]
                        .hijo("pPr")
                        .and_then(|p| p.hijo("sectPr"))
                        .is_some()
                {
                    self.seccion_lectura += 1;
                }
            }
        }
    }

    fn leer_notas(&mut self) {
        if self.notas_leidas {
            return;
        }
        self.notas_leidas = true;
        let Some(rel) = self.rels.values().find(|r| r.tipo == "footnotes").cloned() else {
            return;
        };
        let Some(raiz) = self.paquete.xml(&rel.destino) else {
            return;
        };
        self.notas_rels = Arc::new(self.paquete.relaciones(&rel.destino));
        for n in raiz.hijos {
            if let pixpin_docs::xml::Hijo::Elemento(e) = n
                && e.nombre == "footnote"
            {
                let id = e.atributo("id").to_string();
                let bloques = e
                    .hijos
                    .into_iter()
                    .filter_map(|h| {
                        if let pixpin_docs::xml::Hijo::Elemento(e) = h {
                            Some(e)
                        } else {
                            None
                        }
                    })
                    .collect();
                self.notas.insert(id, Arc::new(bloques));
            }
        }
    }
}

/// Construye un `Ctx` con los campos de `Datos` y las relaciones dadas.
macro_rules! ctx {
    ($d:expr, $rels:expr, $medidor:expr, $pagina:expr) => {
        Ctx {
            estilos: &$d.estilos,
            tema: &$d.tema,
            numeracion: &mut $d.numeracion,
            paquete: &mut $d.paquete,
            recursos: &$d.recursos,
            rels: $rels,
            medidor: $medidor,
            tope_defecto: $d.tope_defecto,
            compat: $d.compat,
            pagina: $pagina,
            total: $d.total_estimado as u32,
            base_tabla: None,
            notas: $d.contador_notas,
            notas_citadas: Vec::new(),
            alternas: &$d.alternas,
        }
    };
}

/// La pagina que se esta llenando.
struct EnCurso {
    pagina: Pagina,
    seccion: usize,
    /// (x absoluta, ancho) de cada columna de la zona actual.
    cols: Vec<(f32, f32)>,
    columna: usize,
    /// Arriba de la zona de columnas (una seccion continua abre otra).
    zona_y: f32,
    /// Lo mas bajo a que ha llegado alguna columna de la zona.
    zona_max: f32,
    y: f32,
    arriba: f32,
    abajo: f32,
    vacia: bool,
    /// La pagina no tiene nada del cuerpo aun.
    pagina_vacia: bool,
    detras: Vec<Elemento>,
    cuerpo: Vec<Elemento>,
    delante: Vec<Elemento>,
    ultimo: Option<(Option<String>, bool, f32)>,
    /// Las notas al pie de esta pagina, ya maquetadas.
    notas: Vec<Caja>,
    alto_notas: f32,
    /// Dibujos que apartan el texto en esta pagina: (arriba, abajo, x0, x1).
    obstaculos: Vec<Obstaculo>,
    /// La columna en curso empezo por un salto manual (`w:br`).
    tras_salto_manual: bool,
}

/// Un `.docx` abierto, que se va paginando segun se pide.
pub struct DocumentoWord {
    d: Datos,
    cola: VecDeque<Franja>,
    siguiente: usize,
    seccion: usize,
    en_curso: Option<EnCurso>,
    numero: u32,
    /// Primera pagina de la seccion en curso aun por abrir.
    seccion_nueva: bool,
    paginas: Vec<Arc<Pagina>>,
    terminado: bool,
    /// Lo que dejo la ultima franja colocada (para el espacio entre parrafos
    /// aunque haya un salto de pagina en medio).
    ultimo_global: bloques::Previa,
}

impl DocumentoWord {
    pub fn de_paquete(mut paquete: Paquete) -> Result<DocumentoWord, ErrorOffice> {
        let medir = std::env::var_os("PIXPIN_OFFICE_TIEMPOS").is_some();
        let t0 = std::time::Instant::now();
        let marca = |que: &str| {
            if medir {
                eprintln!(
                    "  [docx] {que}: {:.1} ms",
                    t0.elapsed().as_secs_f64() * 1000.0
                );
            }
        };
        let principal = paquete
            .principal()
            .unwrap_or_else(|| "word/document.xml".into());
        let fuente = paquete
            .texto(&principal)
            .ok_or(ErrorOffice::SinParte("word/document.xml"))?;
        marca("document.xml descomprimido");
        let rels = paquete.relaciones(&principal);
        let ruta_de = |tipo: &str| {
            rels.values()
                .find(|r| r.tipo == tipo && !r.externa)
                .map(|r| r.destino.clone())
        };
        let tema = ruta_de("theme")
            .and_then(|r| paquete.xml(&r))
            .map(|t| Tema::leer(&t))
            .unwrap_or_else(|| Tema::leer(&Nodo::default()));
        let x_estilos = ruta_de("styles").and_then(|r| paquete.xml(&r));
        marca("styles.xml leido");
        let estilos = Estilos::leer(x_estilos.as_ref(), &tema);
        marca("estilos");
        let numeracion = Numeracion::leer(
            ruta_de("numbering").and_then(|r| paquete.xml(&r)).as_ref(),
            &tema,
        );
        let mut tope_defecto = 36.0;
        let mut compat = 12;
        let mut pares_impares = false;
        if let Some(s) = ruta_de("settings").and_then(|r| paquete.xml(&r)) {
            if let Some(t) = s
                .hijo("defaultTabStop")
                .and_then(|t| attr_num(t, "val", 20.0))
            {
                tope_defecto = twips(t).max(1.0);
            }
            pares_impares = s
                .hijo("evenAndOddHeaders")
                .is_some_and(crate::paquete::interruptor);
            if let Some(c) = s.hijo("compat") {
                for cs in c.elementos().filter(|e| e.nombre == "compatSetting") {
                    if cs.atributo("name") == "compatibilityMode" {
                        compat = cs.atributo("val").parse().unwrap_or(12);
                    }
                }
            }
        }
        let total_estimado = paquete
            .xml("docProps/app.xml")
            .and_then(|a| a.hijo("Pages").and_then(|p| p.texto().trim().parse().ok()))
            .unwrap_or(1usize);

        // Las letras que el documento pide y sus alternativas.
        let mut alternas = HashMap::new();
        if let Some(ft) = ruta_de("fontTable").and_then(|r| paquete.xml(&r)) {
            for f in ft.elementos().filter(|f| f.nombre == "font") {
                let mut v: Vec<String> = Vec::new();
                if let Some(a) = f.hijo("altName") {
                    v.push(a.valor().to_string());
                }
                let generica = match f.hijo("family").map(|x| x.valor()).unwrap_or("") {
                    "roman" => "Times New Roman",
                    "swiss" => "Arial",
                    "modern" => "Courier New",
                    "script" => "Segoe Script",
                    _ => "",
                };
                if !generica.is_empty() {
                    v.push(generica.to_string());
                }
                if !v.is_empty() {
                    alternas.insert(f.atributo("name").to_string(), v.join(","));
                }
            }
        }
        // El cuerpo NO se lee aqui: solo se buscan las secciones (pocas y
        // pequenas) para saber el tamano de las hojas. Los bloques se leen
        // segun se van maquetando.
        if crate::xml::dentro_de(&fuente, "document", 0).is_none() {
            return Err(ErrorOffice::SinParte("word/document.xml"));
        }
        let cuerpo_en = crate::xml::dentro_de(&fuente, "body", 0);
        let fondo = crate::xml::aperturas(
            &fuente[..cuerpo_en.unwrap_or(0).min(fuente.len())],
            "background",
        )
        .first()
        .and_then(|&i| crate::xml::elemento_en(&fuente, i))
        .and_then(|(b, _)| Color::hex(b.atributo("color")));
        let mut secciones: Vec<Seccion> = crate::xml::aperturas(&fuente, "sectPr")
            .into_iter()
            .filter_map(|i| crate::xml::elemento_en(&fuente, i))
            .map(|(n, _)| Seccion::leer(Some(&n)))
            .collect();
        if secciones.is_empty() {
            secciones.push(Seccion::leer(None));
        }
        marca("secciones");
        let cursor = cuerpo_en;
        let cuerpo_acabado = cursor.is_none();
        let cursor = cursor.unwrap_or(0);
        // Los encabezados que una seccion no define los hereda de la
        // anterior.
        for i in 1..secciones.len() {
            for k in 0..3 {
                if secciones[i].cabeceras[k].is_none() {
                    secciones[i].cabeceras[k] = secciones[i - 1].cabeceras[k].clone();
                }
                if secciones[i].pies[k].is_none() {
                    secciones[i].pies[k] = secciones[i - 1].pies[k].clone();
                }
            }
        }
        // El `type` de una seccion dice como EMPIEZA ella.
        Ok(DocumentoWord {
            d: Datos {
                paquete,
                estilos,
                tema,
                numeracion,
                rels: Arc::new(rels),
                recursos: Recursos::nuevo(),
                bloques: Vec::new(),
                seccion_de: Vec::new(),
                secciones,
                fuente,
                cursor,
                cuerpo_acabado,
                seccion_lectura: 0,
                tope_defecto,
                compat,
                pares_impares,
                total_estimado,
                partes: HashMap::new(),
                notas: HashMap::new(),
                notas_rels: Arc::new(HashMap::new()),
                notas_leidas: false,
                contador_notas: 0,
                fondo,
                alternas,
            },
            cola: VecDeque::new(),
            siguiente: 0,
            seccion: 0,
            en_curso: None,
            numero: 0,
            seccion_nueva: true,
            paginas: Vec::new(),
            terminado: false,
            ultimo_global: None,
        })
    }

    pub fn recursos(&self) -> Arc<Recursos> {
        self.d.recursos.clone()
    }

    pub fn paginas_listas(&self) -> usize {
        self.paginas.len()
    }

    pub fn terminado(&self) -> bool {
        self.terminado
    }

    /// El total, o la estimacion de Word mientras no se acabe.
    pub fn total(&self) -> usize {
        if self.terminado {
            self.paginas.len()
        } else {
            self.d.total_estimado.max(self.paginas.len())
        }
    }

    /// Las secciones del documento.
    pub fn secciones(&self) -> &[Seccion] {
        &self.d.secciones
    }

    pub fn pagina(&mut self, n: usize, medidor: &dyn Medidor) -> Option<Arc<Pagina>> {
        while self.paginas.len() <= n && !self.terminado {
            self.avanzar(medidor);
        }
        self.paginas.get(n).cloned()
    }

    pub fn maquetar_todo(&mut self, medidor: &dyn Medidor) {
        while !self.terminado {
            self.avanzar(medidor);
        }
    }

    /// Coloca franjas hasta cerrar al menos una pagina (o acabar).
    fn avanzar(&mut self, medidor: &dyn Medidor) {
        let antes = self.paginas.len();
        while self.paginas.len() == antes {
            if self.cola.is_empty() && !self.cargar(medidor) {
                if let Some(ec) = self.en_curso.take() {
                    self.cerrar(ec, medidor);
                } else if self.paginas.is_empty() {
                    self.abrir(medidor);
                    if let Some(ec) = self.en_curso.take() {
                        self.cerrar(ec, medidor);
                    }
                }
                self.terminado = true;
                return;
            }
            if let Some(f) = self.cola.pop_front() {
                self.colocar(f, medidor);
            }
        }
    }

    /// Convierte el siguiente bloque en franjas. `false` si no quedan.
    fn cargar(&mut self, medidor: &dyn Medidor) -> bool {
        let i = self.siguiente;
        // El bloque y el siguiente (para juntar bordes de parrafo).
        self.d.leer_bloques(i + 2);
        if i >= self.d.bloques.len() {
            return false;
        }
        self.siguiente += 1;
        let s = self.d.seccion_de[i];
        if i == 0 || self.d.seccion_de[i - 1] != s {
            self.cola.push_back(Franja {
                seccion: Some(s),
                ..Default::default()
            });
        }
        let ancho = self.d.secciones[s]
            .columnas
            .first()
            .map(|c| c.1)
            .unwrap_or(self.d.secciones[s].util());
        let mut salida = Vec::new();
        let bloques = std::mem::take(&mut self.d.bloques);
        {
            let rels = self.d.rels.clone();
            let numero = self.numero.max(1);
            let mut ctx = ctx!(self.d, &rels, medidor, numero);
            let previo = if i > 0 { bloques.get(i - 1) } else { None };
            bloques::bloque(
                &mut ctx,
                &bloques[i],
                ancho,
                previo,
                bloques.get(i + 1),
                &mut salida,
            );
            self.d.contador_notas = ctx.notas;
        }
        self.d.bloques = bloques;
        self.cola.extend(salida);
        true
    }

    fn abrir(&mut self, medidor: &dyn Medidor) {
        let s = self.seccion;
        let sec = self.d.secciones[s].clone();
        let primera_de_seccion = self.seccion_nueva;
        self.seccion_nueva = false;
        self.numero = match (primera_de_seccion, sec.inicio) {
            (true, Some(n)) => n,
            _ => self.numero + 1,
        };
        let mut ec = EnCurso {
            pagina: Pagina {
                ancho: sec.ancho,
                alto: sec.alto,
                fondo: self.d.fondo.map(Relleno::Solido),
                elementos: Vec::new(),
                numero: self.numero,
            },
            seccion: s,
            cols: sec
                .columnas
                .iter()
                .map(|(x, w)| (sec.margen[3] + x, *w))
                .collect(),
            columna: 0,
            zona_y: sec.margen[0],
            zona_max: sec.margen[0],
            y: sec.margen[0],
            arriba: sec.margen[0],
            abajo: sec.alto - sec.margen[2],
            vacia: true,
            pagina_vacia: true,
            detras: Vec::new(),
            cuerpo: Vec::new(),
            delante: Vec::new(),
            ultimo: self.ultimo_global.clone(),
            notas: Vec::new(),
            alto_notas: 0.0,
            obstaculos: Vec::new(),
            tras_salto_manual: false,
        };
        // Los bordes de pagina.
        if sec.bordes.iter().any(|b| b.is_some()) {
            let m = sec.margen;
            let pos = |i: usize| -> f32 {
                let esp = sec.bordes[i].map(|b| b.1).unwrap_or(0.0);
                if sec.bordes_desde_hoja {
                    esp
                } else {
                    // Desde el texto: el margen menos la distancia.
                    (m[i] - esp).max(0.0)
                }
            };
            let (arriba, der, abajo, izq) = (pos(0), sec.ancho - pos(1), sec.alto - pos(2), pos(3));
            let lados = [
                ((izq, arriba), (der, arriba)),
                ((der, arriba), (der, abajo)),
                ((izq, abajo), (der, abajo)),
                ((izq, arriba), (izq, abajo)),
            ];
            for (i, (a, b)) in lados.into_iter().enumerate() {
                if let Some((t, _)) = sec.bordes[i] {
                    ec.detras.push(Elemento::Linea {
                        desde: a,
                        hasta: b,
                        trazo: t,
                    });
                }
            }
        }
        // Encabezado y pie.
        let cual = if primera_de_seccion && sec.titulo {
            1
        } else if self.d.pares_impares && self.numero % 2 == 0 {
            2
        } else {
            0
        };
        // Con «primera pagina distinta» y sin encabezado de primera pagina,
        // la primera va sin encabezado.
        let rid_cab = sec.cabeceras[cual].clone();
        let rid_pie = sec.pies[cual].clone();
        let util = sec.util();
        if let Some(c) = rid_cab.and_then(|r| self.caja_parte(&r, util, medidor)) {
            let y0 = sec.cabecera;
            ec.arriba = ec.arriba.max(y0 + c.alto);
            self.volcar_caja(&mut ec, c, sec.margen[3], y0);
        }
        if let Some(c) = rid_pie.and_then(|r| self.caja_parte(&r, util, medidor)) {
            let y0 = sec.alto - sec.pie - c.alto;
            ec.abajo = ec.abajo.min(y0);
            self.volcar_caja(&mut ec, c, sec.margen[3], y0);
        }
        ec.zona_y = ec.arriba;
        ec.zona_max = ec.arriba;
        ec.y = ec.arriba;
        self.en_curso = Some(ec);
    }

    /// Un encabezado o pie maquetado para la pagina en curso.
    fn caja_parte(&mut self, rid: &str, ancho: f32, medidor: &dyn Medidor) -> Option<Caja> {
        let parte = self.d.parte(rid)?;
        let numero = self.numero;
        let mut ctx = ctx!(self.d, &parte.rels, medidor, numero);
        // Los numeros de lista de un encabezado no cuentan en el cuerpo.
        Some(bloques::caja(&mut ctx, parte.bloques.iter(), ancho))
    }

    fn volcar_caja(&self, ec: &mut EnCurso, c: Caja, x: f32, y: f32) {
        for mut e in c.elementos {
            e.trasladar(x, y);
            ec.detras.push(e);
        }
        for (dy, f) in c.flotantes {
            // Un dibujo del encabezado que aparta el texto tambien aparta el
            // del cuerpo (un logotipo que baja hasta el texto).
            if let (_, Some(o)) = self.poner_flotante(ec, f, y + dy, true) {
                ec.obstaculos.push(o);
            }
        }
    }

    /// Coloca un dibujo anclado en la pagina. Devuelve su caja y, si aparta
    /// el texto, lo que le estorba a este.
    fn poner_flotante(
        &self,
        ec: &mut EnCurso,
        f: Flotante,
        y_parrafo: f32,
        de_cabecera: bool,
    ) -> ((f32, f32, f32, f32), Option<Obstaculo>) {
        let sec = &self.d.secciones[ec.seccion];
        let (cx, cw) = ec
            .cols
            .get(ec.columna)
            .copied()
            .unwrap_or((sec.margen[3], sec.util()));
        let (o_ancho, o_alto) = (f.objeto.ancho, f.objeto.alto);
        let (base_x, ancho_ref) = match f.h.referencia {
            Referencia::Pagina => (0.0, sec.ancho),
            Referencia::Margen => (sec.margen[3], sec.util()),
            Referencia::MargenIzq => (0.0, sec.margen[3]),
            Referencia::MargenDer => (sec.ancho - sec.margen[1], sec.margen[1]),
            _ => (cx, cw),
        };
        let x = match f.h.alineacion.as_deref() {
            Some("center") => base_x + (ancho_ref - o_ancho) / 2.0,
            Some("right") | Some("outside") => base_x + ancho_ref - o_ancho,
            Some("left") | Some("inside") => base_x,
            _ => base_x + f.h.desplazamiento,
        };
        let (base_y, alto_ref) = match f.v.referencia {
            Referencia::Pagina => (0.0, sec.alto),
            Referencia::Margen => (sec.margen[0], sec.alto - sec.margen[0] - sec.margen[2]),
            Referencia::MargenArriba => (0.0, sec.margen[0]),
            Referencia::MargenAbajo => (sec.alto - sec.margen[2], sec.margen[2]),
            _ => (y_parrafo, 0.0),
        };
        let y = match f.v.alineacion.as_deref() {
            Some("center") => base_y + (alto_ref - o_alto) / 2.0,
            Some("bottom") | Some("outside") => base_y + alto_ref - o_alto,
            Some("top") | Some("inside") => base_y,
            _ => base_y + f.v.desplazamiento,
        };
        let caja = (x, y, x + o_ancho, y + o_alto);
        let obstaculo = Obstaculo::de_flotante(caja, &f);
        let destino = if f.detras || de_cabecera {
            &mut ec.detras
        } else {
            &mut ec.delante
        };
        for mut e in f.objeto.elementos {
            e.trasladar(x, y);
            destino.push(e);
        }
        (caja, obstaculo)
    }

    /// Donde cae la franja `f` si empezara en `y` en la columna en curso:
    /// la altura (mas abajo si salta un dibujo) y, para un renglon de
    /// parrafo, los trozos en que tiene que ir (x de la caja; `None` = a lo
    /// ancho).
    fn sitio_de(ec: &EnCurso, f: &Franja, y: f32) -> (f32, Option<Vec<(f32, f32)>>) {
        if ec.obstaculos.is_empty() || f.alto <= 0.0 {
            return (y, None);
        }
        let (cx, cw) = ec.cols.get(ec.columna).copied().unwrap_or((72.0, 468.0));
        let Some(rh) = &f.rehacer else {
            return (rodear::debajo(&ec.obstaculos, y, f.alto, cx, cx + cw), None);
        };
        let cfg = &rh.cfg;
        let lo = cx + cfg.sangria_izq + if f.linea == 0 { cfg.primera } else { 0.0 };
        let hi = cx + cfg.ancho - cfg.sangria_der;
        if hi - lo < 1.0 {
            return (y, None);
        }
        let (y, t) = rodear::sitio(&ec.obstaculos, y, f.alto, lo, hi);
        let entero = t.len() == 1 && (t[0].0 - lo).abs() < 0.01 && (t[0].1 - hi).abs() < 0.01;
        (
            y,
            (!entero).then(|| t.into_iter().map(|(a, b)| (a - cx, b - cx)).collect()),
        )
    }

    /// Si el renglon `f` se partio para otros trozos que los que le tocan
    /// (`tramos`), lo vuelve a partir: deja en `f` el renglon nuevo y en la
    /// cola los que le siguen, en lugar de los viejos de su parrafo.
    fn rehacer_si_hace_falta(
        &mut self,
        f: &mut Franja,
        tramos: &Option<Vec<(f32, f32)>>,
        medidor: &dyn Medidor,
    ) -> bool {
        let Some(rh) = f.rehacer.clone() else {
            return false;
        };
        let iguales = match tramos {
            None => f.limites.is_empty(),
            Some(t) => {
                t.len() == f.limites.len()
                    && t.iter()
                        .zip(&f.limites)
                        .all(|(a, b)| (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5)
            }
        };
        if iguales {
            return false;
        }
        let mut nuevas = bloques::rehacer(&rh, f.linea, f.unidad, tramos.as_deref(), medidor);
        if nuevas.is_empty() {
            return false;
        }
        // Los renglones viejos que seguian a este.
        while self
            .cola
            .front()
            .is_some_and(|g| g.rehacer.as_ref().is_some_and(|r| Arc::ptr_eq(r, &rh)))
        {
            self.cola.pop_front();
        }
        let mut primera = nuevas.remove(0);
        // Lo que ya llevaba la franja y no sale de partir.
        primera.flotantes = std::mem::take(&mut f.flotantes);
        primera.inicio_bloque = f.inicio_bloque;
        primera.salto = f.salto;
        primera.salto_manual = f.salto_manual;
        for g in nuevas.into_iter().rev() {
            self.cola.push_front(g);
        }
        *f = primera;
        true
    }

    fn siguiente_columna(&mut self, medidor: &dyn Medidor) {
        let Some(ec) = self.en_curso.as_mut() else {
            self.abrir(medidor);
            return;
        };
        ec.zona_max = ec.zona_max.max(ec.y);
        if ec.columna + 1 < ec.cols.len() {
            ec.columna += 1;
            ec.y = ec.zona_y;
            ec.vacia = true;
            ec.tras_salto_manual = false;
            return;
        }
        let ec = self.en_curso.take().expect("hay pagina");
        self.cerrar(ec, medidor);
        self.abrir(medidor);
    }

    fn cerrar(&mut self, mut ec: EnCurso, medidor: &dyn Medidor) {
        // Las notas al pie, encima del pie de pagina.
        if !ec.notas.is_empty() {
            let sec = &self.d.secciones[ec.seccion];
            let mut y = ec.abajo + 6.0;
            ec.cuerpo.push(Elemento::Linea {
                desde: (sec.margen[3], y - 3.0),
                hasta: (sec.margen[3] + 144.0, y - 3.0),
                trazo: Trazo::solido(Color::NEGRO, 0.5),
            });
            for c in std::mem::take(&mut ec.notas) {
                for mut e in c.elementos {
                    e.trasladar(sec.margen[3], y);
                    ec.cuerpo.push(e);
                }
                y += c.alto;
            }
        }
        let _ = medidor;
        let mut p = ec.pagina;
        p.elementos = ec.detras;
        p.elementos.append(&mut ec.cuerpo);
        p.elementos.append(&mut ec.delante);
        self.paginas.push(Arc::new(p));
    }

    fn cambiar_seccion(&mut self, s: usize, medidor: &dyn Medidor) {
        let tipo = self.d.secciones[s].tipo.clone();
        let anterior = self.seccion;
        self.seccion = s;
        if s == 0 && self.en_curso.is_none() {
            self.seccion_nueva = true;
            return;
        }
        let Some(ec) = self.en_curso.as_mut() else {
            self.seccion_nueva = true;
            return;
        };
        let misma_hoja = {
            let (a, b) = (&self.d.secciones[anterior], &self.d.secciones[s]);
            (a.ancho - b.ancho).abs() < 1.0 && (a.alto - b.alto).abs() < 1.0
        };
        match tipo.as_str() {
            "continuous" if misma_hoja => {
                // Otra zona de columnas en la misma pagina.
                let sec = &self.d.secciones[s];
                let y = ec.zona_max.max(ec.y);
                ec.zona_y = y;
                ec.zona_max = y;
                ec.y = y;
                ec.columna = 0;
                ec.cols = sec
                    .columnas
                    .iter()
                    .map(|(x, w)| (sec.margen[3] + x, *w))
                    .collect();
                ec.seccion = s;
            }
            "nextColumn" if ec.columna + 1 < ec.cols.len() => {
                self.siguiente_columna(medidor);
            }
            _ => {
                let ec = self.en_curso.take().expect("hay pagina");
                self.cerrar(ec, medidor);
                self.seccion_nueva = true;
                let par = (self.numero + 1) % 2 == 0;
                if (tipo == "evenPage" && !par) || (tipo == "oddPage" && par) {
                    // Una pagina en blanco para caer en la paridad pedida.
                    self.abrir(medidor);
                    if let Some(ec) = self.en_curso.take() {
                        self.cerrar(ec, medidor);
                    }
                    self.seccion_nueva = true;
                }
            }
        }
    }

    /// Cuanto miden juntas `f` y las que la siguen pegadas.
    fn alto_cadena(&mut self, f: &Franja, maximo: f32, medidor: &dyn Medidor) -> f32 {
        // Como Word: «conservar con el siguiente» pide el principio del
        // siguiente parrafo (su primer renglon, o los dos de la huerfana),
        // no el parrafo entero aunque este tenga «lineas juntas». El espacio
        // de mas del interlineado de la ultima puede salirse del margen.
        let mut alto = f.antes + f.alto;
        let mut holgura = f.holgura;
        let mut sigue = f.pegada || f.pegada_lineas;
        let mut en_el_primero = true;
        let mut k = 0;
        while sigue {
            while self.cola.len() <= k {
                if !self.cargar(medidor) {
                    return alto - holgura;
                }
            }
            let g = &self.cola[k];
            if g.seccion.is_some() {
                break;
            }
            if g.inicio_bloque {
                en_el_primero = false;
                // Word no arrastra un parrafo a la pagina siguiente por pegarlo
                // a una tabla que alli no cabe: la tabla empieza donde pueda.
                if g.fila.is_some() {
                    break;
                }
            }
            alto += g.antes + g.alto;
            holgura = g.holgura;
            if alto - holgura > maximo {
                break;
            }
            sigue = g.pegada || (g.pegada_lineas && en_el_primero);
            k += 1;
            if k > 60 {
                break;
            }
        }
        alto - holgura
    }

    fn colocar(&mut self, mut f: Franja, medidor: &dyn Medidor) {
        if let Some(s) = f.seccion {
            self.cambiar_seccion(s, medidor);
            return;
        }
        if self.en_curso.is_none() {
            self.abrir(medidor);
        }
        match f.salto {
            Salto::Pagina => {
                if self.en_curso.as_ref().is_some_and(|ec| !ec.pagina_vacia) {
                    let ec = self.en_curso.take().expect("hay pagina");
                    self.cerrar(ec, medidor);
                    self.abrir(medidor);
                }
                f.salto = Salto::Ninguno;
                if f.salto_manual
                    && let Some(ec) = self.en_curso.as_mut()
                {
                    ec.tras_salto_manual = true;
                }
            }
            Salto::Columna => {
                self.siguiente_columna(medidor);
                f.salto = Salto::Ninguno;
                if let Some(ec) = self.en_curso.as_mut() {
                    ec.tras_salto_manual = true;
                }
            }
            Salto::Ninguno => {}
        }
        let mut intentos = 0;
        let mut cabecera_puesta = false;
        loop {
            intentos += 1;
            let ec = self.en_curso.as_mut().expect("hay pagina");
            // Las filas de cabecera de una tabla que sigue en otra pagina.
            if ec.vacia
                && !f.es_cabecera
                && let Some(cab) = f.cabecera.clone()
                && !cabecera_puesta
            {
                for c in cab.iter() {
                    self.poner(c.clone(), medidor);
                }
                cabecera_puesta = true;
            }
            let primera_del_todo = self.paginas.is_empty();
            let ec = self.en_curso.as_mut().expect("hay pagina");
            let (retroceso, antes) = espacio(ec, &f, primera_del_todo);
            // Los dibujos que apartan el texto: el renglon se estrecha (se
            // vuelve a partir) o baja.
            let mut y_franja = ec.y - retroceso + antes;
            if !ec.obstaculos.is_empty() || !f.limites.is_empty() {
                for _ in 0..3 {
                    let ec = self.en_curso.as_ref().expect("hay pagina");
                    let (y, tramos) = Self::sitio_de(ec, &f, ec.y - retroceso + antes);
                    y_franja = y;
                    if !self.rehacer_si_hace_falta(&mut f, &tramos, medidor) {
                        break;
                    }
                }
            }
            let ec = self.en_curso.as_mut().expect("hay pagina");
            let antes = y_franja - (ec.y - retroceso);
            let disponible = ec.abajo - (ec.y - retroceso);
            let alto_col = ec.abajo - ec.zona_y;
            let vacia = ec.vacia;
            let cadena = self.alto_cadena(&f, alto_col, medidor) - f.antes + antes;
            // (Una fila suelta que se puede partir no salta entera: se parte abajo.)
            let partible_suelta = f.fila.as_ref().is_some_and(|x| !x.no_partir) && !f.pegada;
            if !vacia
                && !partible_suelta
                && cadena > disponible + 0.5
                && cadena <= alto_col
                && intentos < 4
            {
                self.siguiente_columna(medidor);
                continue;
            }
            if antes + f.alto - f.holgura > disponible + 0.5 && intentos < 4 {
                // Una fila de tabla se parte si deja algo util arriba.
                if let Some(fila) = &f.fila
                    && disponible - antes > 12.0
                    && let Some((a, alto_a, b, alto_b)) = fila.partir(disponible - antes)
                {
                    let resto = Franja {
                        alto: alto_b,
                        fila: Some(Box::new(b)),
                        cabecera: f.cabecera.clone(),
                        despues: f.despues,
                        ..Default::default()
                    };
                    let primera = Franja {
                        alto: alto_a,
                        fila: Some(Box::new(a)),
                        antes: f.antes,
                        ..Default::default()
                    };
                    self.poner(primera, medidor);
                    self.cola.push_front(resto);
                    self.siguiente_columna(medidor);
                    return;
                }
                if !vacia {
                    self.siguiente_columna(medidor);
                    continue;
                }
            }
            break;
        }
        self.poner(f, medidor);
    }

    /// Deja la franja en la columna actual, donde vaya el cursor.
    fn poner(&mut self, f: Franja, medidor: &dyn Medidor) {
        let primera_del_todo = self.paginas.is_empty();
        let mut ec = self.en_curso.take().expect("hay pagina");
        let (retroceso, antes) = espacio(&ec, &f, primera_del_todo);
        ec.y -= retroceso;
        let (cx, cw) = ec.cols.get(ec.columna).copied().unwrap_or((72.0, 468.0));
        let _ = cw;
        let y_parrafo = ec.y + antes;
        // Los dibujos que apartan el texto: el renglon baja si hace falta (ya
        // se partio a su medida al colocarlo).
        let mut y = Self::sitio_de(&ec, &f, y_parrafo).0;
        let mut f = f;
        if !f.flotantes.is_empty() {
            let n = ec.obstaculos.len();
            for fl in std::mem::take(&mut f.flotantes) {
                if let (_, Some(o)) = self.poner_flotante(&mut ec, fl, y_parrafo, false) {
                    ec.obstaculos.push(o);
                }
            }
            if ec.obstaculos.len() > n {
                // Los dibujos de este mismo parrafo ya le estorban a su
                // primer renglon.
                let tramos = Self::sitio_de(&ec, &f, y_parrafo).1;
                self.en_curso = Some(ec);
                self.rehacer_si_hace_falta(&mut f, &tramos, medidor);
                ec = self.en_curso.take().expect("hay pagina");
                let y3 = Self::sitio_de(&ec, &f, y_parrafo).0;
                if y3 + f.alto - f.holgura > ec.abajo + 0.5 && y3 > y + 0.5 {
                    // Ya no cabe bajo sus dibujos: sigue en la columna
                    // siguiente (los dibujos se quedan en esta pagina).
                    ec.vacia = false;
                    ec.pagina_vacia = false;
                    self.en_curso = Some(ec);
                    self.cola.push_front(f);
                    self.siguiente_columna(medidor);
                    return;
                }
                y = y3;
            }
        }
        for mut e in bloques::materializar(&f) {
            e.trasladar(cx, y);
            ec.cuerpo.push(e);
        }
        // Las notas al pie citadas aqui.
        if !f.notas.is_empty() {
            self.d.leer_notas();
            let sec_ancho = self.d.secciones[ec.seccion].util();
            for (id, num) in &f.notas {
                let Some(bloques) = self.d.notas.get(id).cloned() else {
                    continue;
                };
                let rels = self.d.notas_rels.clone();
                let numero = self.numero;
                let previo = self.d.contador_notas;
                let mut ctx = ctx!(self.d, &rels, medidor, numero);
                ctx.notas = *num;
                let c = bloques::caja(&mut ctx, bloques.iter(), sec_ancho);
                self.d.contador_notas = previo;
                if ec.notas.is_empty() {
                    ec.alto_notas += 12.0;
                    ec.abajo -= 12.0;
                }
                ec.alto_notas += c.alto;
                ec.abajo -= c.alto;
                ec.notas.push(c);
            }
        }
        // Una franja sin alto ni nada (lo que deja un salto de pagina al
        // final de un parrafo) no ocupa la columna.
        if f.alto <= 0.0
            && f.despues <= 0.0
            && f.elementos.is_empty()
            && f.fila.is_none()
            && f.flotantes.is_empty()
        {
            self.en_curso = Some(ec);
            return;
        }
        ec.y = y + f.alto + f.despues;
        ec.zona_max = ec.zona_max.max(y + f.alto);
        ec.vacia = false;
        ec.pagina_vacia = false;
        ec.ultimo = Some((f.estilo.clone(), f.contextual, f.despues));
        // Word descuenta el espacio despues del ultimo parrafo de una pagina
        // del espacio antes del primero de la siguiente.
        self.ultimo_global = ec.ultimo.clone();
        self.en_curso = Some(ec);
    }
}

/// El espacio antes de una franja en la pagina en curso. Arriba de una
/// columna (salvo en la primera pagina) Word no deja el espacio antes.
fn espacio(ec: &EnCurso, f: &Franja, primera_del_todo: bool) -> (f32, f32) {
    let _ = primera_del_todo;
    if ec.vacia && ec.tras_salto_manual {
        return (0.0, 0.0);
    }
    bloques::espacio_entre(&ec.ultimo, f)
}

/// Mete en `v` los bloques de verdad de un hijo del cuerpo.
fn aplanar_en(e: Nodo, v: &mut Vec<Nodo>) {
    match e.nombre.as_str() {
        "p" | "tbl" => v.push(e),
        "sdt" => {
            if let Some(c) = e.hijos.into_iter().find_map(|h| match h {
                pixpin_docs::xml::Hijo::Elemento(x) if x.nombre == "sdtContent" => Some(x),
                _ => None,
            }) {
                for h in c.hijos {
                    if let pixpin_docs::xml::Hijo::Elemento(x) = h {
                        aplanar_en(x, v);
                    }
                }
            }
        }
        "customXml" | "ins" | "smartTag" => {
            for h in e.hijos {
                if let pixpin_docs::xml::Hijo::Elemento(x) = h {
                    aplanar_en(x, v);
                }
            }
        }
        "AlternateContent" => {
            if let Some(c) = e.hijos.into_iter().find_map(|h| match h {
                pixpin_docs::xml::Hijo::Elemento(x)
                    if x.nombre == "Choice" || x.nombre == "Fallback" =>
                {
                    Some(x)
                }
                _ => None,
            }) {
                for h in c.hijos {
                    if let pixpin_docs::xml::Hijo::Elemento(x) = h {
                        aplanar_en(x, v);
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
pub(crate) mod pruebas;
