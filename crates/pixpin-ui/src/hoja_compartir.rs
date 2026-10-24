//! **La hoja de compartir** (G3) como logica pura: que formatos se ven, que
//! paginas van, donde cae cada mando y que hay bajo el raton.
//!
//! Es `ui/HojaDeCompartir.kt` del movil. Alli es una hoja que sube desde
//! abajo con tres partes, y aqui son las mismas tres, de arriba abajo:
//!
//! - **Los formatos**, en botones redondos que se reconocen sin leer.
//! - **Que paginas**, cuando hay para elegir. En los formatos de una sola
//!   pagina (una imagen, un SVG) se elige una; en los demas, las que se
//!   quiera. **Con mas de una marcada, los de una sola desaparecen** —salvo
//!   el que este puesto—, que es la regla del movil: ofrecer «PNG» con doce
//!   paginas marcadas invita a un clic que no puede hacer lo que promete.
//! - **El peso y las salidas**: cuanto ocupa el fichero de verdad (lo
//!   prepara quien abre la hoja, en segundo plano) y los botones de
//!   terminar. En el movil son «Enlace» y «Compartir»; aqui el panel
//!   Compartir de Windows, «Guardar como», copiar y la wifi, que es lo que un
//!   PC hace con un fichero.
//!
//! Que formatos hay y como se genera cada uno lo decide quien abre la hoja
//! ([`Compartible`]): la hoja no sabe de lienzos, PDF ni proyectos.
//!
//! Pixeles de VENTANA (la ventana es la tarjeta), ya multiplicados por la
//! escala.

use std::collections::BTreeSet;

/// Cuantas paginas admite un formato (`Compartible.NINGUNA/UNA/VARIAS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cuantas {
    /// Va entero: el original, el `.pixpin`, el texto de una nota.
    Ninguna,
    /// Una sola: una imagen o un SVG no tienen paginas.
    Una,
    /// Las que se marquen: un PDF, una pagina web.
    Varias,
}

/// Una pagina que se puede elegir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pagina {
    pub clave: String,
    pub nombre: String,
    /// Debajo del nombre: de que proyecto, que tipo.
    pub detalle: String,
    /// Sangria: un marco va debajo de su lienzo.
    pub nivel: u8,
}

/// Un formato que se ofrece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formato {
    /// Lo que entiende quien genera (`"web"`, `"pdf"`...).
    pub id: String,
    pub nombre: String,
    pub cuantas: Cuantas,
    /// Las paginas que admite, si no son todas (una nota no tiene SVG si el
    /// que genera no sabe hacerlo). `None`: todas.
    pub admite: Option<BTreeSet<String>>,
    /// Un interruptor que cambia lo que sale (`Compartible.Interruptor` del
    /// movil: «Con anotaciones» en el PDF de un documento). Nace puesto.
    pub interruptor: Option<Interruptor>,
}

/// **Un interruptor de un formato**: puesto, se genera el formato; quitado,
/// `id_apagado`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interruptor {
    pub nombre: String,
    /// Debajo del nombre: que hace.
    pub detalle: String,
    pub id_apagado: String,
}

impl Formato {
    pub fn admite(&self, clave: &str) -> bool {
        self.admite.as_ref().is_none_or(|a| a.contains(clave))
    }
}

/// Lo que se comparte, visto desde la hoja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compartible {
    pub titulo: String,
    pub paginas: Vec<Pagina>,
    pub formatos: Vec<Formato>,
    /// Las marcadas al abrir; `None`, todas.
    pub marcadas: Option<BTreeSet<String>>,
}

/// Lo que se ha elegido en la hoja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estado {
    /// El formato puesto, por su posicion en `Compartible::formatos`.
    pub formato: usize,
    pub marcadas: BTreeSet<String>,
    /// La primera fila de la lista que se ve (la lista se corre con la
    /// rueda).
    pub primera_fila: usize,
    /// Los formatos (por su posicion) cuyo interruptor se quito.
    pub apagados: BTreeSet<usize>,
}

impl Estado {
    /// Al abrir: el primer formato y lo que diga `marcadas` (o todo).
    pub fn nuevo(c: &Compartible) -> Estado {
        Estado {
            formato: 0,
            marcadas: c
                .marcadas
                .clone()
                .unwrap_or_else(|| c.paginas.iter().map(|p| p.clave.clone()).collect()),
            primera_fila: 0,
            apagados: BTreeSet::new(),
        }
    }

    /// Si el formato puesto tiene interruptor, si esta puesto.
    pub fn interruptor(&self, c: &Compartible) -> Option<bool> {
        self.formato(c)?.interruptor.as_ref()?;
        Some(!self.apagados.contains(&self.formato))
    }

    /// Pone o quita el interruptor del formato puesto; sin interruptor, nada.
    pub fn alternar_interruptor(&mut self, c: &Compartible) {
        if self.interruptor(c).is_some() && !self.apagados.remove(&self.formato) {
            self.apagados.insert(self.formato);
        }
    }

    /// **Lo que hay que generar**: el formato puesto, o lo que diga su
    /// interruptor si esta quitado.
    pub fn id_a_generar(&self, c: &Compartible) -> Option<String> {
        let f = self.formato(c)?;
        Some(match (&f.interruptor, self.interruptor(c)) {
            (Some(i), Some(false)) => i.id_apagado.clone(),
            _ => f.id.clone(),
        })
    }

    pub fn formato<'a>(&self, c: &'a Compartible) -> Option<&'a Formato> {
        c.formatos.get(self.formato)
    }

    /// Los formatos que se ven, por su posicion. **Con mas de una pagina
    /// marcada, los de una sola desaparecen**, menos el que este puesto.
    pub fn visibles(&self, c: &Compartible) -> Vec<usize> {
        (0..c.formatos.len())
            .filter(|&i| {
                !(c.formatos[i].cuantas == Cuantas::Una
                    && self.marcadas.len() > 1
                    && i != self.formato)
            })
            .collect()
    }

    /// Lo que va, en el orden de la lista y solo lo que el formato admite.
    /// Vacio con un formato que va entero.
    pub fn elegidas(&self, c: &Compartible) -> Vec<String> {
        let Some(f) = self.formato(c) else {
            return Vec::new();
        };
        if f.cuantas == Cuantas::Ninguna {
            return Vec::new();
        }
        c.paginas
            .iter()
            .filter(|p| self.marcadas.contains(&p.clave) && f.admite(&p.clave))
            .map(|p| p.clave.clone())
            .collect()
    }

    /// Si con lo elegido hay algo que preparar: un formato de paginas sin
    /// ninguna marcada no lo tiene («Marca alguna pagina»).
    pub fn hay_que_preparar(&self, c: &Compartible) -> bool {
        match self.formato(c) {
            None => false,
            Some(f) if f.cuantas == Cuantas::Ninguna || c.paginas.is_empty() => true,
            Some(_) => !self.elegidas(c).is_empty(),
        }
    }

    /// Pone el formato `i`. Si es de una sola pagina y habia varias
    /// marcadas, se queda la primera que ese formato admite.
    pub fn elegir_formato(&mut self, c: &Compartible, i: usize) {
        let Some(f) = c.formatos.get(i) else {
            return;
        };
        self.formato = i;
        if f.cuantas == Cuantas::Una && self.marcadas.len() > 1 {
            let primera = c
                .paginas
                .iter()
                .find(|p| self.marcadas.contains(&p.clave) && f.admite(&p.clave))
                .map(|p| p.clave.clone());
            self.marcadas = primera.into_iter().collect();
        }
    }

    /// Un clic en una pagina: con un formato de una sola, pasa a ser la
    /// unica; con uno de varias, se pone o se quita. Lo que el formato no
    /// admite no se toca.
    pub fn tocar_pagina(&mut self, c: &Compartible, clave: &str) {
        let Some(f) = self.formato(c) else {
            return;
        };
        if !f.admite(clave) || !c.paginas.iter().any(|p| p.clave == clave) {
            return;
        }
        match f.cuantas {
            Cuantas::Ninguna => {}
            Cuantas::Una => {
                self.marcadas = std::iter::once(clave.to_string()).collect();
            }
            Cuantas::Varias => {
                if !self.marcadas.remove(clave) {
                    self.marcadas.insert(clave.to_string());
                }
            }
        }
    }

    pub fn todas(&mut self, c: &Compartible) {
        self.marcadas = c.paginas.iter().map(|p| p.clave.clone()).collect();
    }

    pub fn ninguna(&mut self) {
        self.marcadas.clear();
    }

    /// Corre la lista `filas` hacia abajo (negativo, arriba), sin pasarse.
    pub fn correr(&mut self, c: &Compartible, filas: i32) {
        let tope = c.paginas.len().saturating_sub(FILAS_VISIBLES);
        let nueva = self.primera_fila as i64 + filas as i64;
        self.primera_fila = nueva.clamp(0, tope as i64) as usize;
    }
}

// ---------------------------------------------------------------------------
// La colocacion

/// Un rectangulo en pixeles de ventana.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Caja {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

impl Caja {
    pub fn contiene(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.ancho && y >= self.y && y < self.y + self.alto
    }
}

/// Las salidas del pie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Salida {
    /// El panel Compartir de Windows. Es el boton principal, como en el
    /// movil.
    Compartir,
    /// El «Guardar como» de siempre.
    Guardar,
    /// Al portapapeles: la imagen si es una imagen, el fichero si no.
    Copiar,
    /// A otro aparato por la wifi (el «Enviar por Wi-Fi» del movil).
    Wifi,
    /// Subirlo a un servicio de archivos temporales y pasar el enlace (el
    /// «Enlace» del movil, 8-oct-2026). Hubo un «Arrastrar» aqui: se quito,
    /// porque compartir ya acaba en la Salida, que es donde se arrastra.
    Enlace,
}

/// Lo que hay bajo un punto de la hoja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destino {
    Nada,
    Cerrar,
    Formato(usize),
    Pagina(String),
    Todas,
    Ninguna,
    Salida(Salida),
    Interruptor,
}

// Medidas en pixeles logicos, las del movil donde las hay (dp).
pub const ANCHO_MINIMO: f32 = 760.0;
pub const MARGEN: f32 = 20.0;
pub const CABECERA: f32 = 56.0;
/// Cada formato: el redondel de 56 y su nombre debajo, en 78 de ancho.
pub const ANCHO_FORMATO: f32 = 78.0;
pub const DIAMETRO: f32 = 56.0;
pub const ALTO_FORMATOS: f32 = 104.0;
pub const ALTO_TITULO_PAGINAS: f32 = 40.0;
pub const ALTO_FILA: f32 = 40.0;
/// Las filas que se ven a la vez; el resto, con la rueda. Las del
/// `heightIn(max = 280.dp)` del movil.
pub const FILAS_VISIBLES: usize = 7;
pub const ALTO_PIE: f32 = 68.0;
const ANCHO_BOTON_PRINCIPAL: f32 = 124.0;
const ANCHO_BOTON: f32 = 112.0;
const ANCHO_BOTON_CORTO: f32 = 96.0;
const ALTO_BOTON: f32 = 40.0;
const HUECO_BOTON: f32 = 8.0;

/// Donde cae cada cosa.
#[derive(Debug, Clone, PartialEq)]
pub struct Disposicion {
    pub ancho: f32,
    pub alto: f32,
    pub cerrar: Caja,
    pub titulo: Caja,
    /// `(formato, caja del redondel, caja del nombre)`, solo los visibles.
    pub formatos: Vec<(usize, Caja, Caja)>,
    /// La franja de las paginas, si hay mas de una.
    pub paginas: Option<Caja>,
    pub rotulo_paginas: Caja,
    pub todas: Option<Caja>,
    pub ninguna: Option<Caja>,
    /// `(clave, fila)` de las filas que se ven.
    pub filas: Vec<(String, Caja)>,
    /// Donde se escribe el peso.
    pub peso: Caja,
    /// El interruptor del formato puesto, si lo tiene: encima del peso.
    pub interruptor: Option<Caja>,
    pub salidas: Vec<(Salida, Caja)>,
}

/// Si la hoja tiene franja de paginas: con una sola no hay que elegir.
pub fn con_paginas(c: &Compartible) -> bool {
    c.paginas.len() > 1
}

/// **El tamano de la ventana**, en pixeles: no cambia al cambiar de formato
/// para que la hoja no de saltos. La franja de paginas se reserva si alguna
/// vez hace falta.
pub fn tamano(c: &Compartible, escala: f32) -> (f32, f32) {
    let ancho = ANCHO_MINIMO.max(c.formatos.len() as f32 * ANCHO_FORMATO + 2.0 * MARGEN);
    let mut alto = CABECERA + ALTO_FORMATOS + ALTO_PIE;
    if con_paginas(c) {
        alto += ALTO_TITULO_PAGINAS + ALTO_FILA * c.paginas.len().min(FILAS_VISIBLES) as f32;
    }
    (ancho * escala, alto * escala)
}

/// **Coloca la hoja** para ese estado.
pub fn disponer(c: &Compartible, e: &Estado, escala: f32) -> Disposicion {
    let k = escala;
    let (ancho, alto) = tamano(c, escala);
    let cerrar = Caja {
        x: ancho - (MARGEN + 32.0) * k,
        y: 12.0 * k,
        ancho: 32.0 * k,
        alto: 32.0 * k,
    };
    let titulo = Caja {
        x: MARGEN * k,
        y: 0.0,
        ancho: cerrar.x - MARGEN * k,
        alto: CABECERA * k,
    };

    // Los formatos, centrados en su fila.
    let visibles = e.visibles(c);
    let fila_ancho = visibles.len() as f32 * ANCHO_FORMATO * k;
    let mut x = (ancho - fila_ancho) / 2.0;
    let y0 = CABECERA * k;
    let formatos = visibles
        .into_iter()
        .map(|i| {
            let redondel = Caja {
                x: x + (ANCHO_FORMATO - DIAMETRO) / 2.0 * k,
                y: y0 + 4.0 * k,
                ancho: DIAMETRO * k,
                alto: DIAMETRO * k,
            };
            let nombre = Caja {
                x,
                y: y0 + (DIAMETRO + 10.0) * k,
                ancho: ANCHO_FORMATO * k,
                alto: 32.0 * k,
            };
            x += ANCHO_FORMATO * k;
            (i, redondel, nombre)
        })
        .collect();

    // Las paginas.
    let mut y = y0 + ALTO_FORMATOS * k;
    let mut paginas = None;
    let mut todas = None;
    let mut ninguna = None;
    let mut filas = Vec::new();
    let mut rotulo_paginas = Caja::default();
    if con_paginas(c) {
        let visibles = c.paginas.len().min(FILAS_VISIBLES);
        let franja = Caja {
            x: 0.0,
            y,
            ancho,
            alto: (ALTO_TITULO_PAGINAS + ALTO_FILA * visibles as f32) * k,
        };
        paginas = Some(franja);
        rotulo_paginas = Caja {
            x: MARGEN * k,
            y,
            ancho: ancho - 2.0 * MARGEN * k,
            alto: ALTO_TITULO_PAGINAS * k,
        };
        let con_lista = e.formato(c).is_some_and(|f| f.cuantas != Cuantas::Ninguna);
        if e.formato(c).is_some_and(|f| f.cuantas == Cuantas::Varias) {
            let w = 84.0 * k;
            ninguna = Some(Caja {
                x: ancho - MARGEN * k - w,
                y: y + 4.0 * k,
                ancho: w,
                alto: 32.0 * k,
            });
            todas = Some(Caja {
                x: ancho - MARGEN * k - 2.0 * w,
                y: y + 4.0 * k,
                ancho: w,
                alto: 32.0 * k,
            });
        }
        if con_lista {
            let mut fy = y + ALTO_TITULO_PAGINAS * k;
            let desde = e.primera_fila.min(c.paginas.len().saturating_sub(visibles));
            for p in c.paginas.iter().skip(desde).take(visibles) {
                filas.push((
                    p.clave.clone(),
                    Caja {
                        x: 0.0,
                        y: fy,
                        ancho,
                        alto: ALTO_FILA * k,
                    },
                ));
                fy += ALTO_FILA * k;
            }
        }
        y += franja.alto;
    }

    // El pie: el peso a la izquierda y las salidas a la derecha, la
    // principal la ultima, que es donde se busca en Windows.
    let by = y + (ALTO_PIE - ALTO_BOTON) / 2.0 * k;
    let mut bx = ancho - MARGEN * k;
    let mut salidas = Vec::new();
    for (s, w) in [
        (Salida::Compartir, ANCHO_BOTON_PRINCIPAL),
        (Salida::Guardar, ANCHO_BOTON),
        // Todas con su nombre, en fila (8-oct-2026: el Wi-Fi era un icono
        // suelto y el usuario lo queria «junto a los demas»).
        (Salida::Copiar, ANCHO_BOTON_CORTO),
        (Salida::Wifi, ANCHO_BOTON_CORTO),
        (Salida::Enlace, ANCHO_BOTON_CORTO),
    ] {
        bx -= w * k;
        salidas.push((
            s,
            Caja {
                x: bx,
                y: by,
                ancho: w * k,
                alto: ALTO_BOTON * k,
            },
        ));
        bx -= HUECO_BOTON * k;
    }
    let mut peso = Caja {
        x: MARGEN * k,
        y,
        ancho: (bx - MARGEN * k).max(0.0),
        alto: ALTO_PIE * k,
    };
    // Con interruptor el pie se parte: el interruptor arriba y el peso
    // debajo. La ventana no cambia de tamano.
    let interruptor = e.interruptor(c).map(|_| {
        let arriba = Caja {
            x: peso.x,
            y: y + 6.0 * k,
            ancho: peso.ancho,
            alto: 28.0 * k,
        };
        peso.y = y + 34.0 * k;
        peso.alto = (ALTO_PIE - 34.0) * k;
        arriba
    });
    Disposicion {
        ancho,
        alto,
        cerrar,
        titulo,
        formatos,
        paginas,
        rotulo_paginas,
        todas,
        ninguna,
        filas,
        peso,
        interruptor,
        salidas,
    }
}

/// **Que hay bajo `(x, y)`.**
pub fn destino_en(d: &Disposicion, x: f32, y: f32) -> Destino {
    if d.cerrar.contiene(x, y) {
        return Destino::Cerrar;
    }
    for (i, redondel, nombre) in &d.formatos {
        if redondel.contiene(x, y) || nombre.contiene(x, y) {
            return Destino::Formato(*i);
        }
    }
    if d.todas.is_some_and(|c| c.contiene(x, y)) {
        return Destino::Todas;
    }
    if d.ninguna.is_some_and(|c| c.contiene(x, y)) {
        return Destino::Ninguna;
    }
    for (clave, fila) in &d.filas {
        if fila.contiene(x, y) {
            return Destino::Pagina(clave.clone());
        }
    }
    for (s, caja) in &d.salidas {
        if caja.contiene(x, y) {
            return Destino::Salida(*s);
        }
    }
    if d.interruptor.is_some_and(|c| c.contiene(x, y)) {
        return Destino::Interruptor;
    }
    Destino::Nada
}

/// **El peso, para leerlo**: `812 B`, `12 KB`, `1,2 MB`. Con un decimal
/// solo por debajo de diez, que «12,3 MB» no dice nada que «12 MB» no diga.
/// `coma` es el separador decimal del idioma.
pub fn peso_legible(bytes: u64, coma: char) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    let (valor, unidad) = if b < K {
        return format!("{bytes} B");
    } else if b < K * K {
        (b / K, "KB")
    } else if b < K * K * K {
        (b / (K * K), "MB")
    } else {
        (b / (K * K * K), "GB")
    };
    if valor < 10.0 {
        let s = format!("{valor:.1}");
        let s = s.strip_suffix(".0").map(str::to_string).unwrap_or(s);
        format!("{} {unidad}", s.replace('.', &coma.to_string()))
    } else {
        format!("{} {unidad}", valor.round() as u64)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn pagina(clave: &str) -> Pagina {
        Pagina {
            clave: clave.into(),
            nombre: clave.into(),
            detalle: String::new(),
            nivel: 0,
        }
    }

    fn formato(id: &str, cuantas: Cuantas) -> Formato {
        Formato {
            id: id.into(),
            nombre: id.into(),
            cuantas,
            admite: None,
            interruptor: None,
        }
    }

    #[test]
    fn el_interruptor_nace_puesto_y_quitado_se_genera_lo_otro() {
        let mut c = compartible(2);
        c.formatos[1].interruptor = Some(Interruptor {
            nombre: "Con anotaciones".into(),
            detalle: String::new(),
            id_apagado: "pdf-limpio".into(),
        });
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 1);
        assert_eq!(e.interruptor(&c), Some(true));
        assert_eq!(e.id_a_generar(&c).as_deref(), Some("pdf"));
        let d = disponer(&c, &e, 1.0);
        let caja = d.interruptor.expect("se ve");
        assert_eq!(
            destino_en(&d, caja.x + 10.0, caja.y + 10.0),
            Destino::Interruptor
        );
        assert!(d.peso.y >= caja.y + caja.alto, "el peso va debajo");
        e.alternar_interruptor(&c);
        assert_eq!(e.id_a_generar(&c).as_deref(), Some("pdf-limpio"));
        e.alternar_interruptor(&c);
        assert_eq!(e.id_a_generar(&c).as_deref(), Some("pdf"));
        // Caso negativo: un formato sin interruptor ni lo ensena ni cambia.
        e.elegir_formato(&c, 2);
        e.alternar_interruptor(&c);
        assert_eq!(e.interruptor(&c), None);
        assert_eq!(e.id_a_generar(&c).as_deref(), Some("web"));
        assert!(disponer(&c, &e, 1.0).interruptor.is_none());
    }

    fn compartible(n: usize) -> Compartible {
        Compartible {
            titulo: "Casa".into(),
            paginas: (1..=n).map(|i| pagina(&format!("p{i}"))).collect(),
            formatos: vec![
                formato("png", Cuantas::Una),
                formato("pdf", Cuantas::Varias),
                formato("web", Cuantas::Varias),
                formato("original", Cuantas::Ninguna),
            ],
            marcadas: None,
        }
    }

    #[test]
    fn al_abrir_va_todo_marcado_y_el_primer_formato_puesto() {
        let c = compartible(3);
        let e = Estado::nuevo(&c);
        assert_eq!(e.formato, 0);
        assert_eq!(e.marcadas.len(), 3);
    }

    #[test]
    fn con_varias_marcadas_los_formatos_de_una_pagina_desaparecen_menos_el_puesto() {
        let c = compartible(3);
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 1);
        assert_eq!(e.visibles(&c), vec![1, 2, 3], "sin el PNG");
        // El puesto no se esconde aunque haya varias.
        e.formato = 0;
        assert!(e.visibles(&c).contains(&0));
    }

    #[test]
    fn elegir_una_imagen_con_varias_marcadas_deja_solo_la_primera() {
        let c = compartible(3);
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 1);
        e.elegir_formato(&c, 0);
        assert_eq!(e.elegidas(&c), vec!["p1".to_string()]);
    }

    #[test]
    fn con_un_formato_de_una_pagina_tocar_otra_la_cambia_y_con_uno_de_varias_la_suma() {
        let c = compartible(3);
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 0);
        e.tocar_pagina(&c, "p3");
        assert_eq!(e.elegidas(&c), vec!["p3".to_string()]);
        e.elegir_formato(&c, 1);
        e.tocar_pagina(&c, "p1");
        assert_eq!(e.elegidas(&c), vec!["p1".to_string(), "p3".to_string()]);
        e.tocar_pagina(&c, "p1");
        assert_eq!(e.elegidas(&c), vec!["p3".to_string()]);
    }

    #[test]
    fn sin_paginas_marcadas_no_hay_nada_que_preparar_pero_el_original_si() {
        let c = compartible(3);
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 1);
        e.ninguna();
        assert!(!e.hay_que_preparar(&c));
        e.elegir_formato(&c, 3);
        assert!(e.hay_que_preparar(&c), "el original va entero");
        assert!(e.elegidas(&c).is_empty());
    }

    #[test]
    fn lo_que_un_formato_no_admite_ni_se_elige_ni_se_toca() {
        let mut c = compartible(2);
        c.formatos[1].admite = Some(["p2".to_string()].into_iter().collect());
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 1);
        assert_eq!(e.elegidas(&c), vec!["p2".to_string()]);
        e.ninguna();
        e.tocar_pagina(&c, "p1");
        assert!(e.elegidas(&c).is_empty());
        // Una clave que no esta tampoco entra.
        e.tocar_pagina(&c, "inventada");
        assert!(e.marcadas.is_empty());
    }

    #[test]
    fn la_lista_se_corre_sin_pasarse_de_ningun_lado() {
        let c = compartible(20);
        let mut e = Estado::nuevo(&c);
        e.correr(&c, -3);
        assert_eq!(e.primera_fila, 0);
        e.correr(&c, 100);
        assert_eq!(e.primera_fila, 20 - FILAS_VISIBLES);
    }

    #[test]
    fn cada_mando_se_encuentra_donde_se_pinto() {
        let c = compartible(3);
        let e = Estado::nuevo(&c);
        let d = disponer(&c, &e, 1.5);
        let centro = |k: Caja| (k.x + k.ancho / 2.0, k.y + k.alto / 2.0);
        let (x, y) = centro(d.cerrar);
        assert_eq!(destino_en(&d, x, y), Destino::Cerrar);
        for (i, redondel, _) in &d.formatos {
            let (x, y) = centro(*redondel);
            assert_eq!(destino_en(&d, x, y), Destino::Formato(*i));
        }
        for (clave, fila) in &d.filas {
            let (x, y) = centro(*fila);
            assert_eq!(destino_en(&d, x, y), Destino::Pagina(clave.clone()));
        }
        for (s, caja) in &d.salidas {
            let (x, y) = centro(*caja);
            assert_eq!(destino_en(&d, x, y), Destino::Salida(*s));
        }
        // El peso no es un boton.
        let (x, y) = centro(d.peso);
        assert_eq!(destino_en(&d, x, y), Destino::Nada);
        // Y todo cabe en la ventana.
        let (w, h) = tamano(&c, 1.5);
        for (_, caja) in &d.salidas {
            assert!(caja.x >= 0.0 && caja.x + caja.ancho <= w + 0.01);
            assert!(caja.y + caja.alto <= h + 0.01);
        }
        assert!(d.peso.ancho > 100.0, "queda sitio para el peso");
    }

    #[test]
    fn con_una_sola_pagina_no_hay_franja_de_paginas() {
        let c = compartible(1);
        let e = Estado::nuevo(&c);
        let d = disponer(&c, &e, 1.0);
        assert!(d.paginas.is_none() && d.filas.is_empty());
        assert_eq!(tamano(&c, 1.0).1, CABECERA + ALTO_FORMATOS + ALTO_PIE);
    }

    #[test]
    fn un_formato_que_va_entero_no_ensena_la_lista_ni_todas_ni_ninguna() {
        let c = compartible(4);
        let mut e = Estado::nuevo(&c);
        e.elegir_formato(&c, 3);
        let d = disponer(&c, &e, 1.0);
        assert!(d.filas.is_empty() && d.todas.is_none() && d.ninguna.is_none());
        // Pero la ventana no cambia de tamano.
        assert_eq!(d.alto, tamano(&c, 1.0).1);
    }

    #[test]
    fn el_peso_se_lee_como_en_el_movil() {
        assert_eq!(peso_legible(0, ','), "0 B");
        assert_eq!(peso_legible(812, ','), "812 B");
        assert_eq!(peso_legible(2048, ','), "2 KB");
        assert_eq!(peso_legible(1_258_291, ','), "1,2 MB");
        assert_eq!(peso_legible(1_258_291, '.'), "1.2 MB");
        assert_eq!(peso_legible(52_428_800, ','), "50 MB");
    }
}
