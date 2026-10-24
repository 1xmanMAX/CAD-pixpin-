//! **El arbol del modelo**: un panel a la izquierda con los niveles y las
//! categorias, cada uno con cuantos elementos tiene y su volumen, para
//! mostrarlos u ocultarlos. Clic: mostrar u ocultar; Ctrl + clic: solo ese
//! (de su lista). Lo pidio el usuario el 9-oct-2026 («el punto 2 y el 3»).

use std::collections::HashSet;

use crate::convertir::rgba;
use crate::modelo::Constructor;
use crate::modelo3d::{Elemento, Medidas, Modelo3d, tipo_legible};
use crate::texto::Textos;
use crate::ventana::{CAPA_ENCIMA, raya_en, rect_en};

// El pintor pone primero lo de capa mayor: el fondo, debajo; la casilla y su
// marca, encima (y todo el panel por encima de la caja de seccion).
const CAPA_FONDO: f64 = CAPA_ENCIMA * 0.5;
const CAPA_CASILLA: f64 = CAPA_ENCIMA * 0.2;
const CAPA_MARCA: f64 = CAPA_ENCIMA * 0.1;

use super::TextosUi3d;

/// Que agrupa una fila: un nivel (o «sin nivel») o una categoria.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum Clave {
    Nivel(Option<u32>),
    Tipo(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Grupo {
    pub clave: Clave,
    pub nombre: String,
    pub cuenta: usize,
    pub volumen: f64,
}

/// Una fila del panel, ya puesta: donde esta y que es.
#[derive(Debug, Clone, PartialEq)]
enum Que {
    Cabecera(String),
    Todo,
    Grupo(usize, bool),
}

#[derive(Debug, Default)]
pub(super) struct Arbol {
    pub niveles: Vec<Grupo>,
    pub tipos: Vec<Grupo>,
    pub ocultos: HashSet<Clave>,
    /// Cuanto se ha bajado la lista (pixeles).
    pub desplazar: f64,
}

const FILA: f64 = 24.0;
const ANCHO: f64 = 300.0;

/// Lo que vale una medida en la pastilla: con su unidad al cubo o al
/// cuadrado si se sabe.
pub(super) fn con_unidad(v: f64, unidad: &str, potencia: u8) -> String {
    let u = unidad.trim();
    let marca = if potencia == 3 { "³" } else { "²" };
    if u.is_empty() {
        format!("{v:.2}")
    } else {
        format!("{v:.2} {u}{marca}")
    }
}

/// El area que importa de un elemento: la de una cara en los muros (y lo
/// que se mide en alzado), la de planta en losas, cubiertas y zapatas.
pub(super) fn area_que_toca(tipo: &str, md: &Medidas) -> Option<f32> {
    let t = tipo.to_ascii_lowercase();
    if ["wall", "plate", "curtain", "window", "door"]
        .iter()
        .any(|k| t.contains(k))
    {
        Some(md.alzado)
    } else if ["slab", "roof", "covering", "footing", "ramp", "stair"]
        .iter()
        .any(|k| t.contains(k))
    {
        Some(md.planta)
    } else {
        None
    }
}

/// «Muro · nombre · 1ER PISO · 3.82 m³ · 21.80 m²».
pub(super) fn describir(el: &Elemento, m: &Modelo3d, unidad: &str) -> String {
    let tipo = tipo_legible(&el.tipo);
    let mut t = if el.nombre.is_empty() {
        tipo
    } else {
        format!("{tipo} · {}", el.nombre)
    };
    if let Some(n) = el.nivel.and_then(|k| m.niveles.get(k as usize)) {
        t.push_str(" · ");
        t.push_str(&n.nombre);
    }
    let md = el.medidas;
    if md.volumen > 0.0 {
        let aprox = if md.cerrado { "" } else { "≈ " };
        t.push_str(&format!(
            " · {aprox}{}",
            con_unidad(md.volumen as f64, unidad, 3)
        ));
    }
    if let Some(a) = area_que_toca(&el.tipo, &md).filter(|a| *a > 0.0) {
        t.push_str(&format!(" · {}", con_unidad(a as f64, unidad, 2)));
    }
    t
}

impl Arbol {
    pub fn de(m: &Modelo3d, ui: &TextosUi3d) -> Arbol {
        let mut niveles: Vec<Grupo> = m
            .niveles
            .iter()
            .enumerate()
            .map(|(k, n)| Grupo {
                clave: Clave::Nivel(Some(k as u32)),
                nombre: n.nombre.clone(),
                cuenta: 0,
                volumen: 0.0,
            })
            .collect();
        let mut sin = Grupo {
            clave: Clave::Nivel(None),
            nombre: ui.sin_nivel.clone(),
            cuenta: 0,
            volumen: 0.0,
        };
        let mut tipos: Vec<Grupo> = Vec::new();
        for el in &m.elementos {
            let v = el.medidas.volumen as f64;
            let g = match el.nivel.and_then(|k| niveles.get_mut(k as usize)) {
                Some(g) => g,
                None => &mut sin,
            };
            g.cuenta += 1;
            g.volumen += v;
            let nombre = tipo_legible(&el.tipo);
            match tipos.iter_mut().find(|g| g.nombre == nombre) {
                Some(g) => {
                    g.cuenta += 1;
                    g.volumen += v;
                }
                None => tipos.push(Grupo {
                    clave: Clave::Tipo(nombre.clone()),
                    nombre,
                    cuenta: 1,
                    volumen: v,
                }),
            }
        }
        // Los niveles vacios no dicen nada; «sin nivel», solo si hay niveles.
        niveles.retain(|g| g.cuenta > 0);
        if sin.cuenta > 0 && !niveles.is_empty() {
            niveles.push(sin);
        }
        tipos.sort_by(|a, b| a.nombre.cmp(&b.nombre));
        Arbol {
            niveles,
            tipos,
            ocultos: HashSet::new(),
            desplazar: 0.0,
        }
    }

    /// **Lo oculto, como texto** (para el estado de un marcador):
    /// `n:=<clave del nivel>` (la estable de [`Modelo3d::clave_de_nivel`],
    /// igual en otro PC y en el movil), `n:-` (sin nivel) o
    /// `t:<categoria>`, ordenado.
    pub fn ocultos_en_texto(&self, m: &Modelo3d) -> Vec<String> {
        let mut v: Vec<String> = self
            .ocultos
            .iter()
            .map(|c| match c {
                Clave::Nivel(Some(n)) => m
                    .clave_de_nivel(*n)
                    .map_or_else(|| format!("n:{n}"), |k| format!("n:={k}")),
                Clave::Nivel(None) => "n:-".to_string(),
                Clave::Tipo(t) => format!("t:{t}"),
            })
            .collect();
        v.sort();
        v
    }

    /// Y al reves: deja oculto justo eso. Dice si cambio algo. `n:<k>` (el
    /// numero de nivel de antes del 20-oct-2026) se sigue entendiendo.
    pub fn poner_ocultos(&mut self, v: &[String], m: &Modelo3d) -> bool {
        let nuevos: HashSet<Clave> = v
            .iter()
            .filter_map(|t| match t.split_once(':')? {
                ("n", "-") => Some(Clave::Nivel(None)),
                ("n", n) => match n.strip_prefix('=') {
                    Some(k) => m.nivel_de_clave(k).map(|n| Clave::Nivel(Some(n))),
                    None => n
                        .parse()
                        .ok()
                        .filter(|n: &u32| (*n as usize) < m.niveles.len())
                        .map(|n| Clave::Nivel(Some(n))),
                },
                ("t", t) => Some(Clave::Tipo(t.to_string())),
                _ => None,
            })
            .collect();
        let cambio = nuevos != self.ocultos;
        self.ocultos = nuevos;
        cambio
    }

    /// Si un elemento se ve con lo que esta oculto.
    pub fn visible(&self, el: &Elemento) -> bool {
        if self.ocultos.is_empty() {
            return true;
        }
        let nivel = if self.niveles.is_empty() {
            None
        } else {
            Some(Clave::Nivel(el.nivel))
        };
        !nivel.is_some_and(|n| self.ocultos.contains(&n))
            && !self.ocultos.contains(&Clave::Tipo(tipo_legible(&el.tipo)))
    }

    /// El rectangulo del panel (x0, y0, x1, y1).
    pub fn marco(&self, h: u32, e: f64) -> (f64, f64, f64, f64) {
        let alto_lista = self.contenido(e);
        let y1 = (12.0 * e + alto_lista + 8.0 * e)
            .min(h as f64 - 52.0 * e)
            .max(60.0 * e);
        (12.0 * e, 12.0 * e, 12.0 * e + ANCHO * e, y1)
    }

    pub fn dentro(&self, x: i32, y: i32, h: u32, e: f64) -> bool {
        let (x0, y0, x1, y1) = self.marco(h, e);
        (x as f64) >= x0 && (x as f64) <= x1 && (y as f64) >= y0 && (y as f64) <= y1
    }

    fn filas(&self, ui: &TextosUi3d) -> Vec<Que> {
        let mut v = vec![Que::Todo];
        if !self.niveles.is_empty() {
            v.push(Que::Cabecera(ui.niveles.clone()));
            v.extend((0..self.niveles.len()).map(|k| Que::Grupo(k, true)));
        }
        v.push(Que::Cabecera(ui.categorias.clone()));
        v.extend((0..self.tipos.len()).map(|k| Que::Grupo(k, false)));
        v
    }

    fn contenido(&self, e: f64) -> f64 {
        let n =
            1 + if self.niveles.is_empty() {
                0
            } else {
                1 + self.niveles.len()
            } + 1
                + self.tipos.len();
        n as f64 * FILA * e
    }

    /// La lista no baja mas de lo que tiene.
    pub fn rueda(&mut self, d: i32, h: u32, e: f64) {
        let (_, y0, _, y1) = self.marco(h, e);
        let sobra = (self.contenido(e) + 8.0 * e - (y1 - y0)).max(0.0);
        self.desplazar = (self.desplazar - d as f64 / 120.0 * 3.0 * FILA * e).clamp(0.0, sobra);
    }

    fn grupo(&self, k: usize, de_niveles: bool) -> &Grupo {
        if de_niveles {
            &self.niveles[k]
        } else {
            &self.tipos[k]
        }
    }

    /// Un clic en el panel: `true` si cambio lo que se ve.
    pub fn tocar(&mut self, x: i32, y: i32, ctrl: bool, ui: &TextosUi3d, h: u32, e: f64) -> bool {
        let (x0, y0, x1, y1) = self.marco(h, e);
        if (x as f64) < x0 || (x as f64) > x1 || (y as f64) < y0 || (y as f64) > y1 {
            return false;
        }
        let k = ((y as f64 - y0 - 4.0 * e + self.desplazar) / (FILA * e)).floor();
        let Some(que) = (k >= 0.0)
            .then(|| self.filas(ui).get(k as usize).cloned())
            .flatten()
        else {
            return false;
        };
        match que {
            Que::Cabecera(_) => false,
            Que::Todo => {
                let habia = !self.ocultos.is_empty();
                self.ocultos.clear();
                habia
            }
            Que::Grupo(k, de_niveles) => {
                let clave = self.grupo(k, de_niveles).clave.clone();
                if ctrl {
                    // Solo este, de su lista.
                    let lista = if de_niveles {
                        &self.niveles
                    } else {
                        &self.tipos
                    };
                    let otros: Vec<Clave> = lista
                        .iter()
                        .map(|g| g.clave.clone())
                        .filter(|c| *c != clave)
                        .collect();
                    self.ocultos
                        .retain(|c| matches!(c, Clave::Nivel(_)) != de_niveles);
                    self.ocultos.extend(otros);
                } else if !self.ocultos.remove(&clave) {
                    self.ocultos.insert(clave);
                }
                true
            }
        }
    }

    /// Pinta el panel (va lo ultimo, encima de todo).
    #[allow(clippy::too_many_arguments)]
    pub fn dibujar(
        &self,
        c: &mut Constructor,
        textos: &mut Textos,
        ui: &TextosUi3d,
        unidad: &str,
        h: u32,
        e: f64,
        claro: bool,
    ) {
        let (fondo, letra, tenue) = if claro {
            (0xF01C1C1Eu32, rgba(255, 255, 255), rgba(0xA0, 0xA0, 0xA8))
        } else {
            (
                0xF0F5F5F7u32,
                rgba(0x1C, 0x1C, 0x1E),
                rgba(0x6E, 0x6E, 0x73),
            )
        };
        let azul = rgba(0x00, 0x60, 0xDF);
        let (x0, y0, x1, y1) = self.marco(h, e);
        rect_en(c, x0, y0, x1, y1, fondo, CAPA_FONDO);
        let tam = 12.5 * e;
        for (k, que) in self.filas(ui).iter().enumerate() {
            let y = y0 + 4.0 * e + k as f64 * FILA * e - self.desplazar;
            if y < y0 || y + FILA * e > y1 + 0.5 {
                continue;
            }
            let base = y + FILA * e * 0.68;
            match que {
                Que::Cabecera(t) => {
                    textos.en_pantalla(c, t, x0 + 10.0 * e, base, tam, tenue, x1 - 10.0 * e);
                }
                Que::Todo => {
                    textos.en_pantalla(
                        c,
                        &ui.mostrar_todo,
                        x0 + 10.0 * e,
                        base,
                        tam,
                        if self.ocultos.is_empty() { tenue } else { azul },
                        x1 - 10.0 * e,
                    );
                }
                Que::Grupo(i, de_niveles) => {
                    let g = self.grupo(*i, *de_niveles);
                    let ve = !self.ocultos.contains(&g.clave);
                    // La casilla.
                    let (cx0, cy0) = (x0 + 12.0 * e, y + FILA * e / 2.0 - 6.0 * e);
                    let (cx1, cy1) = (cx0 + 12.0 * e, cy0 + 12.0 * e);
                    if ve {
                        rect_en(c, cx0, cy0, cx1, cy1, azul, CAPA_CASILLA);
                        let b = rgba(255, 255, 255);
                        raya_en(
                            c,
                            [cx0 + 2.5 * e, cy0 + 6.5 * e],
                            [cx0 + 5.0 * e, cy0 + 9.0 * e],
                            1.6 * e,
                            b,
                            CAPA_MARCA,
                        );
                        raya_en(
                            c,
                            [cx0 + 5.0 * e, cy0 + 9.0 * e],
                            [cx0 + 9.5 * e, cy0 + 3.0 * e],
                            1.6 * e,
                            b,
                            CAPA_MARCA,
                        );
                    } else {
                        for (a, b) in [
                            ([cx0, cy0], [cx1, cy0]),
                            ([cx1, cy0], [cx1, cy1]),
                            ([cx1, cy1], [cx0, cy1]),
                            ([cx0, cy1], [cx0, cy0]),
                        ] {
                            raya_en(c, a, b, 1.3 * e, tenue, CAPA_MARCA);
                        }
                    }
                    // A la derecha: cuantos y cuanto volumen.
                    let dato = if g.volumen > 0.0 {
                        format!("{} · {}", g.cuenta, con_unidad(g.volumen, unidad, 3))
                    } else {
                        g.cuenta.to_string()
                    };
                    let ancho_dato = textos.medir_pantalla(&dato, tam * 0.92);
                    textos.en_pantalla(
                        c,
                        &dato,
                        x1 - 10.0 * e - ancho_dato,
                        base,
                        tam * 0.92,
                        tenue,
                        x1,
                    );
                    let col = if ve { letra } else { tenue };
                    textos.en_pantalla(
                        c,
                        &g.nombre,
                        cx1 + 8.0 * e,
                        base,
                        tam,
                        col,
                        x1 - 18.0 * e - ancho_dato,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::modelo3d::{Constructor3d, Nivel};

    fn modelo() -> Modelo3d {
        let mut c = Constructor3d::nuevo();
        let a = c.elemento("IfcWall", "a");
        let b = c.elemento("IfcSlab", "b");
        let d = c.elemento("IfcWallStandardCase", "c");
        c.niveles(
            vec![
                Nivel {
                    nombre: "1ER PISO".into(),
                    cota: 3.4,
                    ..Default::default()
                },
                Nivel {
                    nombre: "VACIO".into(),
                    cota: 9.0,
                    ..Default::default()
                },
            ],
            &[(a, 0), (b, 0)],
        );
        let _ = d;
        c.terminar()
    }

    #[test]
    fn agrupa_por_nivel_y_categoria_sin_los_vacios() {
        let ui = TextosUi3d::default();
        let ar = Arbol::de(&modelo(), &ui);
        // «VACIO» no sale; el muro sin nivel va a «Sin nivel».
        assert_eq!(
            ar.niveles
                .iter()
                .map(|g| (g.nombre.as_str(), g.cuenta))
                .collect::<Vec<_>>(),
            vec![("1ER PISO", 2), (ui.sin_nivel.as_str(), 1)]
        );
        // IfcWall e IfcWallStandardCase son los dos «Muro».
        assert_eq!(
            ar.tipos
                .iter()
                .map(|g| (g.nombre.as_str(), g.cuenta))
                .collect::<Vec<_>>(),
            vec![("Losa", 1), ("Muro", 2)]
        );
    }

    #[test]
    fn clic_oculta_y_ctrl_clic_deja_solo_ese() {
        let ui = TextosUi3d::default();
        let m = modelo();
        let mut ar = Arbol::de(&m, &ui);
        let (e, h) = (1.0, 800);
        // Filas: Todo, «Niveles», 1ER PISO, Sin nivel, «Categorias», Losa, Muro.
        let y_de = |k: usize| (12.0 + 4.0 + (k as f64 + 0.5) * FILA) as i32;
        assert!(ar.tocar(40, y_de(6), false, &ui, h, e));
        assert!(!ar.visible(&m.elementos[0]) && ar.visible(&m.elementos[1]));
        // Ctrl en «1ER PISO»: fuera el «sin nivel» (el tercer elemento).
        assert!(ar.tocar(40, y_de(2), true, &ui, h, e));
        assert!(!ar.visible(&m.elementos[2]));
        // «Mostrar todo».
        assert!(ar.tocar(40, y_de(0), false, &ui, h, e));
        assert!(m.elementos.iter().all(|el| ar.visible(el)));
        // Caso negativo: una cabecera o fuera del panel no cambian nada.
        assert!(!ar.tocar(40, y_de(1), false, &ui, h, e));
        assert!(!ar.tocar(900, y_de(2), false, &ui, h, e));
    }

    /// Lo oculto va en el estado de un marcador como texto y vuelve igual.
    #[test]
    fn lo_oculto_va_y_vuelve_como_texto() {
        let ui = TextosUi3d::default();
        let m = modelo();
        let mut ar = Arbol::de(&m, &ui);
        ar.ocultos.insert(Clave::Nivel(None));
        ar.ocultos.insert(Clave::Nivel(Some(0)));
        ar.ocultos.insert(Clave::Tipo("Muro".into()));
        let t = ar.ocultos_en_texto(&m);
        // El nivel, por su clave estable (sin GlobalId: nombre y cota).
        assert_eq!(t, vec!["n:-", "n:=1ER PISO@3400", "t:Muro"]);
        let mut otro = Arbol::de(&m, &ui);
        assert!(otro.poner_ocultos(&t, &m));
        assert_eq!(otro.ocultos, ar.ocultos);
        // Lo mismo otra vez no es un cambio; lo que no se entiende se deja.
        assert!(!otro.poner_ocultos(&t, &m));
        assert!(otro.poner_ocultos(
            &[
                "x:1".into(),
                "n:abc".into(),
                "n:=NO-ESTA".into(),
                "n:7".into()
            ],
            &m
        ));
        assert!(otro.ocultos.is_empty());
        // El de antes, por numero, se sigue entendiendo.
        assert!(otro.poner_ocultos(&["n:0".into()], &m));
        assert!(otro.ocultos.contains(&Clave::Nivel(Some(0))));
    }

    /// Un marcador hecho con el modelo leido en un orden vuelve a lo mismo
    /// aunque en otro PC (u otra version del lector) los niveles y los
    /// elementos salgan en otro orden.
    #[test]
    fn lo_oculto_y_lo_aislado_aguantan_otro_orden() {
        use crate::modelo3d::{Constructor3d, Nivel};
        let leer = |al_reves: bool| {
            let mut c = Constructor3d::nuevo();
            let mut nombres = [
                ("IfcWall", "Muro A", "GA"),
                ("IfcSlab", "Losa", ""),
                ("IfcSlab", "Losa", ""),
            ];
            let mut niveles = vec![
                Nivel {
                    nombre: "1ER PISO".into(),
                    cota: 3.4,
                    clave: "S1".into(),
                },
                Nivel {
                    nombre: "2DO PISO".into(),
                    cota: 6.65,
                    ..Default::default()
                },
            ];
            if al_reves {
                nombres.reverse();
                niveles.reverse();
            }
            let es: Vec<u32> = nombres
                .iter()
                .map(|(t, n, g)| {
                    let e = c.elemento(t, n);
                    if !g.is_empty() {
                        c.poner_clave(e, g);
                    }
                    e
                })
                .collect();
            c.niveles(niveles, &[(es[0], 0), (es[1], 1)]);
            c.terminar()
        };
        let (a, b) = (leer(false), leer(true));
        let ui = TextosUi3d::default();
        let mut ar = Arbol::de(&a, &ui);
        let k = a
            .niveles
            .iter()
            .position(|n| n.nombre == "2DO PISO")
            .unwrap() as u32;
        ar.ocultos.insert(Clave::Nivel(Some(k)));
        let t = ar.ocultos_en_texto(&a);
        let mut otro = Arbol::de(&b, &ui);
        otro.poner_ocultos(&t, &b);
        let kb = b
            .niveles
            .iter()
            .position(|n| n.nombre == "2DO PISO")
            .unwrap() as u32;
        assert_eq!(otro.ocultos, [Clave::Nivel(Some(kb))].into_iter().collect());
        // El muro, por su GlobalId; la losa sin el, por tipo, nombre y orden.
        let muro_a = a
            .elementos
            .iter()
            .position(|e| e.nombre == "Muro A")
            .unwrap() as u32;
        let muro_b = b.elemento_de_clave(&a.clave_de_elemento(muro_a).unwrap());
        assert_eq!(
            muro_b.map(|i| b.elementos[i as usize].nombre.as_str()),
            Some("Muro A")
        );
        let losa = a.elementos.iter().position(|e| e.nombre == "Losa").unwrap() as u32;
        assert_eq!(
            a.clave_de_elemento(losa).as_deref(),
            Some("~IfcSlab|Losa|0")
        );
        assert!(b.elemento_de_clave("~IfcSlab|Losa|0").is_some());
        // Caso negativo: una clave que no esta.
        assert_eq!(b.elemento_de_clave("NO-ESTA"), None);
    }

    #[test]
    fn la_pastilla_dice_nivel_volumen_y_area() {
        let mut c = Constructor3d::nuevo();
        let a = c.elemento("IfcSlab", "Losa 1");
        c.niveles(
            vec![Nivel {
                nombre: "2DO PISO".into(),
                cota: 6.65,
                ..Default::default()
            }],
            &[(a, 0)],
        );
        let p: Vec<[f64; 3]> = (0..8)
            .map(|i| {
                [
                    (i & 1) as f64 * 2.0,
                    ((i >> 1) & 1) as f64,
                    ((i >> 2) & 1) as f64,
                ]
            })
            .collect();
        let mut idx = Vec::new();
        for f in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            idx.extend([f[0], f[1], f[2], f[0], f[2], f[3]]);
        }
        c.malla(a, &p, None, &idx, [0.8, 0.8, 0.8, 1.0]);
        let m = c.terminar();
        assert_eq!(
            describir(&m.elementos[0], &m, " m"),
            "Losa · Losa 1 · 2DO PISO · 2.00 m³ · 2.00 m²"
        );
        // Sin unidad conocida (un DWG), los numeros solos.
        assert!(describir(&m.elementos[0], &m, "").ends_with("2.00 · 2.00"));
    }
}
