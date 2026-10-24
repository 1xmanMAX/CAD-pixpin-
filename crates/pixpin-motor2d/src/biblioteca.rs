//! **La biblioteca de figuras**: guardar una seleccion y volver a estamparla.
//!
//! Es la herramienta mas barata de toda la lista y la que mas tiempo ahorra a
//! quien dibuja lo mismo cada dia: una flecha de norte, un sello de «borrador»,
//! el bloque de titulo de una lamina.
//!
//! Tres decisiones, las tres del movil (`Biblioteca.kt`) y las tres con motivo:
//!
//! 1. **Lo guardado se normaliza al origen.** Una figura guardada no recuerda
//!    donde estaba, porque donde estaba no es parte de la figura. Sin esto,
//!    estampar tendria que restar la posicion antigua cada vez y cualquier
//!    error de redondeo se acumularia.
//! 2. **Al estampar nacen identificadores y semillas nuevos.** La semilla es
//!    lo que hace que el garabato sea el mismo cada vez que se abre; repetirla
//!    haria que dos sellos de la misma figura salieran calcados, y calcados se
//!    leen como una copia y no como dos cosas.
//! 3. **Lo estampado nace agrupado.** Se guardo como una cosa y tiene que
//!    volver como una cosa: si naciera suelto, el primer clic lo desharia.
//!
//! Lo que NO hace, y es a proposito: no viaja en el `.excalidraw`. Una figura
//! guardada es un contenedor aparte, igual que alli —no hay campo del elemento
//! que la mencione— y por eso abrir un `.pixpin` no trae la biblioteca del
//! otro ni se la lleva.
//!
//! ## El fichero, el del movil
//!
//! `figuras.json` es una lista de `{id, nombre, elementos, propia}` con las
//! piezas **como elementos de Excalidraw** (`BibliotecaStore.kt`, que las
//! escribe con `ExcalidrawJson`): el mismo fichero se abre en el telefono y
//! aqui. Antes el PC escribia sus piezas con el formato interno del motor y
//! el movil no las entendia; ese formato viejo se sigue leyendo
//! ([`desde_json`]) y al guardar sale ya en el del movil.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::elemento::Elemento;
use crate::vector::Punto2;

/// Una figura guardada: su nombre y las piezas de las que esta hecha, ya
/// normalizadas al origen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiguraGuardada {
    /// El `id` del movil (con el se quita una de la lista). Las del PC de
    /// antes no traian ninguno: se les da uno al leerlas.
    #[serde(default)]
    pub id: String,
    pub nombre: String,
    /// Las piezas, con su esquina superior izquierda comun en `(0, 0)`.
    pub elementos: Vec<Elemento>,
}

impl FiguraGuardada {
    /// Lo que mide, calculado y no guardado: guardarlo seria una segunda
    /// verdad sobre las piezas y quedaria desfasado en cuanto se editara una.
    pub fn tamano(&self) -> (f32, f32) {
        match caja_de(&self.elementos) {
            Some((x0, y0, x1, y1)) => (x1 - x0, y1 - y0),
            None => (0.0, 0.0),
        }
    }
}

/// La caja que ocupan varios elementos juntos. `None` si no hay ninguno vivo.
pub fn caja_de(elementos: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
    let mut caja: Option<(f32, f32, f32, f32)> = None;
    for e in elementos.iter().filter(|e| !e.borrado) {
        let (x0, y0, x1, y1) = e.caja();
        caja = Some(match caja {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }
    caja
}

/// Lleva las piezas a que su esquina comun caiga en `(0, 0)`.
pub fn normalizar(elementos: &[Elemento]) -> Vec<Elemento> {
    let mut piezas: Vec<Elemento> = elementos.iter().filter(|e| !e.borrado).cloned().collect();
    if let Some((x0, y0, _, _)) = caja_de(&piezas) {
        for e in &mut piezas {
            e.mover(-x0, -y0);
        }
    }
    piezas
}

/// Guarda lo elegido como figura. `None` si no habia nada que guardar.
pub fn de_la_seleccion(
    elementos: &[Elemento],
    elegidos: &[u64],
    nombre: &str,
) -> Option<FiguraGuardada> {
    let piezas: Vec<Elemento> = elementos
        .iter()
        .filter(|e| elegidos.contains(&e.id) && !e.borrado)
        .cloned()
        .collect();
    if piezas.is_empty() {
        return None;
    }
    Some(FiguraGuardada {
        id: id_nuevo(),
        nombre: nombre.to_string(),
        elementos: normalizar(&piezas),
    })
}

/// Un id que no se repite (`randomId` del movil): la hora y un contador.
pub fn id_nuevo() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let hora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    format!("pc{:x}{:x}", hora, N.fetch_add(1, Ordering::Relaxed))
}

// ---------------------------------------------------------------------------
// El fichero
// ---------------------------------------------------------------------------

/// Cuantas piezas como mucho se leen de una figura: una biblioteca con una
/// figura de un millon de piezas (rota, o de otro programa) no puede colgar
/// la apertura del lienzo.
pub const MAX_PIEZAS: usize = 20_000;

/// **La lista en el formato del movil**: piezas como elementos de Excalidraw.
pub fn a_json(figuras: &[FiguraGuardada]) -> String {
    let lista: Vec<Value> = figuras
        .iter()
        .map(|f| {
            json!({
                "id": if f.id.is_empty() { id_nuevo() } else { f.id.clone() },
                "nombre": f.nombre,
                "elementos": crate::excalidraw::elementos_a_json(&f.elementos),
                "propia": true,
            })
        })
        .collect();
    serde_json::to_string(&lista).unwrap_or_else(|_| "[]".into())
}

/// **Lee la lista**, en el formato del movil o en el viejo del PC. Nunca
/// falla: lo ilegible se salta figura a figura y pieza a pieza (perder una
/// figura es malo; no poder abrir el lienzo por ella es peor), y una figura
/// sin ninguna pieza que se entienda no entra.
pub fn desde_json(texto: &str) -> Vec<FiguraGuardada> {
    let Ok(Value::Array(lista)) = serde_json::from_str::<Value>(texto) else {
        return Vec::new();
    };
    lista
        .into_iter()
        .filter_map(|v| {
            let piezas = v.get("elementos")?.as_array()?;
            let piezas = &piezas[..piezas.len().min(MAX_PIEZAS)];
            // Del movil: elementos de Excalidraw, con su `type`. Del PC de
            // antes: el formato interno, con su `figura`.
            let elementos: Vec<Elemento> = if piezas.iter().any(|p| p.get("type").is_some()) {
                crate::excalidraw::elementos_de_json(piezas)
            } else {
                piezas
                    .iter()
                    .filter_map(|p| serde_json::from_value::<Elemento>(p.clone()).ok())
                    .collect()
            };
            if elementos.is_empty() {
                return None;
            }
            let nombre = v
                .get("nombre")
                .and_then(Value::as_str)
                .unwrap_or("Figura")
                .trim();
            Some(FiguraGuardada {
                id: v
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map_or_else(id_nuevo, str::to_string),
                nombre: if nombre.is_empty() {
                    "Figura".into()
                } else {
                    nombre.chars().take(80).collect()
                },
                elementos: normalizar(&elementos),
            })
        })
        .collect()
}

/// Las figuras guardadas en `ruta`, o ninguna.
pub fn cargar(ruta: &Path) -> Vec<FiguraGuardada> {
    std::fs::read_to_string(ruta)
        .map(|t| desde_json(&t))
        .unwrap_or_default()
}

/// Escribe la lista entera, a un temporal y renombrando: un corte a medias
/// no puede dejar la biblioteca rota (`BibliotecaStore.guardar`).
pub fn guardar(ruta: &Path, figuras: &[FiguraGuardada]) -> bool {
    if let Some(dir) = ruta.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let temporal = ruta.with_extension("json.tmp");
    std::fs::write(&temporal, a_json(figuras)).is_ok() && std::fs::rename(&temporal, ruta).is_ok()
}

/// **Anade una figura al principio** (lo ultimo guardado es lo que se esta
/// usando: `BibliotecaStore.anadir`). Si ya habia una con el mismo nombre,
/// la nueva la sustituye: «guarda esto como arbol» dos veces no deja dos
/// arboles que no se distinguen.
pub fn anadir(ruta: &Path, figura: FiguraGuardada) -> bool {
    let mut lista = cargar(ruta);
    lista.retain(|f| !f.nombre.eq_ignore_ascii_case(&figura.nombre));
    lista.insert(0, figura);
    guardar(ruta, &lista)
}

/// Estampa la figura **centrada** en `centro`.
///
/// `primer_id` es el identificador libre con el que empezar: la escena es
/// quien los reparte, y aqui solo se consumen correlativos desde ese. El
/// nombre del grupo lo pone el llamante por lo mismo que los ids —en el
/// fichero es una cadena que genera el movil y aqui viaja intacta—.
pub fn estampar(
    figura: &FiguraGuardada,
    centro: Punto2,
    primer_id: u64,
    grupo: &str,
) -> Vec<Elemento> {
    let mut piezas = figura.elementos.clone();
    let (ancho, alto) = figura.tamano();
    // Centrado y no con la esquina en el cursor: lo que el usuario apunta con
    // el raton es donde quiere que quede la figura, no donde quiere que
    // empiece su caja invisible.
    let (dx, dy) = (centro.x - ancho / 2.0, centro.y - alto / 2.0);
    for (i, e) in piezas.iter_mut().enumerate() {
        e.mover(dx, dy);
        e.id = primer_id + i as u64;
        // Semilla nueva **derivada del id**, no al azar: asi estampar dos
        // veces en el mismo sitio da dos garabatos distintos, y estampar en
        // una sesion y en la siguiente da el mismo si el id coincide, que es
        // lo que hace que el documento se reabra igual.
        e.semilla = semilla_de(e.id);
        e.version = 0;
        e.borrado = false;
        // El grupo de la figura va el ULTIMO de la pila: `groupIds` es una
        // pila donde el ultimo es el mas exterior, asi que desagrupar una vez
        // deshace la estampacion y deja intactos los grupos que la figura ya
        // traia dentro.
        e.grupos.push(grupo.to_string());
    }
    piezas
}

/// **Estampa la figura en la escena**, centrada en `centro`, en un solo paso
/// de deshacer, y devuelve los ids de lo estampado (para dejarlo elegido y
/// llevarlo a su sitio de un tiron, como lo pegado).
///
/// El nombre del grupo sale como en `organizar::agrupar` (`g` y el siguiente
/// id): unico en la escena sin llevar una cuenta aparte.
pub fn estampar_en_escena(
    escena: &mut crate::escena::Escena,
    figura: &FiguraGuardada,
    centro: Punto2,
) -> Vec<u64> {
    if figura.elementos.is_empty() {
        return Vec::new();
    }
    let primero = escena.siguiente_id.max(1);
    let grupo = format!("g{primero}");
    let piezas = estampar(figura, centro, primero, &grupo);
    escena.abrir_paso();
    let ids = piezas.into_iter().map(|e| escena.anadir(e)).collect();
    escena.cerrar_paso();
    ids
}

/// Una semilla nunca cero a partir de un identificador. Cero significaria
/// «sin sembrar» y el dibujo temblaria en cada apertura.
fn semilla_de(id: u64) -> u32 {
    // Mezcla barata (xorshift de una vuelta): ids correlativos tienen que dar
    // garabatos distintos, y `id as u32` daria semillas casi iguales.
    let mut x = (id as u32) ^ 0x9E37_79B9;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x.max(1)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::Figura;

    fn pieza(id: u64, x: f32, y: f32) -> Elemento {
        Elemento {
            id,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho: 10.0,
            alto: 10.0,
            semilla: 7,
            ..Default::default()
        }
    }

    fn guardada() -> FiguraGuardada {
        de_la_seleccion(
            &[pieza(1, 100.0, 200.0), pieza(2, 120.0, 200.0)],
            &[1, 2],
            "flecha",
        )
        .unwrap()
    }

    #[test]
    fn lo_guardado_olvida_donde_estaba() {
        let f = guardada();
        assert_eq!(caja_de(&f.elementos).unwrap().0, 0.0);
        assert_eq!(caja_de(&f.elementos).unwrap().1, 0.0);
        assert_eq!(f.tamano(), (30.0, 10.0));
    }

    #[test]
    fn guardar_sin_nada_elegido_no_crea_una_figura_vacia() {
        // Caso negativo: una biblioteca con figuras de cero piezas es una
        // lista de botones que no hacen nada.
        assert!(de_la_seleccion(&[pieza(1, 0.0, 0.0)], &[], "x").is_none());
        let mut borrado = pieza(1, 0.0, 0.0);
        borrado.borrado = true;
        assert!(de_la_seleccion(&[borrado], &[1], "x").is_none());
    }

    #[test]
    fn estampar_deja_la_figura_centrada_en_el_cursor() {
        let e = estampar(&guardada(), Punto2::nuevo(500.0, 500.0), 40, "g1");
        let (x0, y0, x1, y1) = caja_de(&e).unwrap();
        assert_eq!(((x0 + x1) / 2.0, (y0 + y1) / 2.0), (500.0, 500.0));
    }

    #[test]
    fn estampar_da_identificadores_y_semillas_nuevos() {
        let e = estampar(&guardada(), Punto2::nuevo(0.0, 0.0), 40, "g1");
        assert_eq!(e[0].id, 40);
        assert_eq!(e[1].id, 41);
        assert_ne!(
            e[0].semilla, e[1].semilla,
            "dos sellos calcados se leen mal"
        );
        assert_ne!(e[0].semilla, 7, "la semilla guardada no se reutiliza");
        assert!(e.iter().all(|x| x.semilla != 0), "cero seria sin sembrar");
    }

    #[test]
    fn estampar_dos_veces_en_el_mismo_sitio_no_da_dos_dibujos_identicos() {
        let a = estampar(&guardada(), Punto2::nuevo(0.0, 0.0), 40, "g1");
        let b = estampar(&guardada(), Punto2::nuevo(0.0, 0.0), 80, "g2");
        assert_ne!(a[0].semilla, b[0].semilla);
    }

    #[test]
    fn lo_estampado_nace_agrupado_y_el_grupo_de_la_figura_va_por_fuera() {
        let mut f = guardada();
        f.elementos[0].grupos = vec!["interno".into()];
        let e = estampar(&f, Punto2::nuevo(0.0, 0.0), 1, "sello");
        assert_eq!(
            e[0].grupos,
            vec!["interno".to_string(), "sello".to_string()]
        );
        assert_eq!(e[1].grupos, vec!["sello".to_string()]);
    }

    #[test]
    fn el_fichero_va_en_el_formato_del_movil_y_vuelve_igual() {
        let mut f = guardada();
        f.elementos[1].figura = Figura::Elipse;
        let texto = a_json(std::slice::from_ref(&f));
        let v: Value = serde_json::from_str(&texto).unwrap();
        // Lo que escribe `BibliotecaStore.kt`: piezas de Excalidraw.
        assert_eq!(v[0]["id"], f.id.as_str());
        assert_eq!(v[0]["propia"], true);
        assert_eq!(v[0]["elementos"][0]["type"], "rectangle");
        assert_eq!(v[0]["elementos"][1]["type"], "ellipse");
        let vuelta = desde_json(&texto);
        assert_eq!(vuelta.len(), 1);
        assert_eq!(vuelta[0].id, f.id);
        assert_eq!(vuelta[0].nombre, "flecha");
        assert_eq!(vuelta[0].tamano(), f.tamano());
        assert!(matches!(vuelta[0].elementos[1].figura, Figura::Elipse));
        // Y otra vuelta da lo mismo: ni se mueve ni cambia.
        let otra = desde_json(&a_json(&vuelta));
        assert_eq!(otra[0].tamano(), vuelta[0].tamano());
        assert_eq!(otra[0].elementos.len(), vuelta[0].elementos.len());
    }

    #[test]
    fn se_lee_una_figura_hecha_en_el_movil() {
        let movil = r##"[{"id":"abc","nombre":"Norte","propia":true,"elementos":[
            {"id":"e1","type":"arrow","x":10,"y":10,"width":0,"height":50,"points":[[0,0],[0,-50]],
             "strokeColor":"#1e1e1e","backgroundColor":"transparent","strokeWidth":2,"seed":3,"version":1,"isDeleted":false},
            {"id":"e2","type":"text","x":4,"y":-70,"width":12,"height":20,"text":"N","fontSize":20,"fontFamily":1,
             "strokeColor":"#1e1e1e","backgroundColor":"transparent","seed":4,"version":1,"isDeleted":false}]}]"##;
        let l = desde_json(movil);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].id, "abc");
        assert_eq!(l[0].elementos.len(), 2);
        // Normalizada al origen al leerla.
        let (x0, y0, _, _) = caja_de(&l[0].elementos).unwrap();
        assert!(x0.abs() < 0.01 && y0.abs() < 0.01, "{x0} {y0}");
    }

    #[test]
    fn una_biblioteca_rota_no_tumba_nada() {
        // Casos negativos: no es JSON, no es una lista, figuras sin piezas,
        // piezas que no se entienden. Y el formato viejo del PC se lee.
        assert!(desde_json("no es json").is_empty());
        assert!(desde_json(r#"{"a":1}"#).is_empty());
        assert!(
            desde_json(r#"[{"nombre":"vacia","elementos":[]}, 7, {"nombre":"sin"}]"#).is_empty()
        );
        assert!(
            desde_json(r#"[{"nombre":"rara","elementos":[{"type":"pixpin-solido-raro"}]}]"#)
                .is_empty()
        );
        let viejo = serde_json::to_string(&vec![guardada()]).unwrap();
        let l = desde_json(&viejo);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].elementos.len(), 2);
    }

    #[test]
    fn anadir_pone_la_nueva_primero_y_sustituye_la_del_mismo_nombre() {
        let dir = std::env::temp_dir().join(format!("pixpin-biblio-{}", id_nuevo()));
        let ruta = dir.join("figuras.json");
        assert!(cargar(&ruta).is_empty(), "sin fichero, lista vacia");
        let mut a = guardada();
        a.nombre = "arbol".into();
        let mut b = guardada();
        b.nombre = "casa".into();
        assert!(anadir(&ruta, a.clone()));
        assert!(anadir(&ruta, b));
        assert!(anadir(&ruta, a));
        let l = cargar(&ruta);
        assert_eq!(
            l.iter().map(|f| f.nombre.as_str()).collect::<Vec<_>>(),
            ["arbol", "casa"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_figura_guardada_va_y_vuelve_por_json() {
        // La biblioteca se guarda en su propio fichero, no en el
        // `.excalidraw`: esto es lo que promete que se puede.
        let f = guardada();
        let texto = serde_json::to_string(&f).unwrap();
        let vuelta: FiguraGuardada = serde_json::from_str(&texto).unwrap();
        assert_eq!(f, vuelta);
    }
}
