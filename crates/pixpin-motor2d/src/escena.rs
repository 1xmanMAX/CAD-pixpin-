//! La escena: la lista de elementos y su historia.
//!
//! El orden de la lista ES el orden de dibujo: el ultimo se pinta encima. No
//! hay campo de "profundidad" a proposito — un indice fraccionario o un
//! z-order paralelo son dos verdades sobre lo mismo, y acaban discrepando.
//!
//! Deshacer es **logico**: el elemento se marca `borrado` y sigue en la lista.
//! Rehacer es quitar la marca. Nada toca el disco hasta que se guarda, y el
//! coste de guardar un elemento borrado (unas decenas de bytes) es mucho menor
//! que el de perder un trazo.

use std::mem::size_of;

use serde::{Deserialize, Serialize};

use crate::elemento::{ColorRgba, Elemento};
use crate::medida::Escala;
use crate::vector::Punto2;

/// Lo que se guarda en el fichero: version, contador y elementos. La
/// herramienta activa, el zoom o la seleccion NO estan aqui: son estado de la
/// interfaz, no del documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Escena {
    #[serde(default = "version_uno")]
    pub version: u32,
    #[serde(default = "uno_u64")]
    pub siguiente_id: u64,
    #[serde(default)]
    pub elementos: Vec<Elemento>,
    /// Que mide un pixel de este lienzo (D31). `None` mientras no se calibre.
    #[serde(default)]
    pub escala: Option<Escala>,
    /// **El color del papel**: el `appState.viewBackgroundColor` del
    /// fichero de Excalidraw, que es tambien el `backgroundColor` de la
    /// escena del movil (`ExcalidrawStore.kt`).
    ///
    /// Vive en la escena y no en la ventana porque es del documento: viaja
    /// con el `.excalidraw`, sale al exportar y se ve en la vista previa del
    /// chat y en el pin. Se cambia con `poner_fondo`, que lo apunta en el
    /// historial como hace Excalidraw (`changeViewBackgroundColor` captura
    /// el cambio): elegir un papel y arrepentirse se arregla con `Ctrl+Z`.
    #[serde(default = "fondo_de_fabrica")]
    pub fondo: ColorRgba,
    /// **Los clavos de soldar vertices** (`Scene.alfileres` del movil, ver
    /// `nudos`). Son de la escena y no de ningun elemento: un clavo ata dos
    /// figuras y no pertenece a ninguna de las dos.
    #[serde(default)]
    pub alfileres: Vec<crate::nudos::Alfiler>,
    /// **Si las lineas de referencia estan a la vista**
    /// (`Scene.referenciasVisibles` del movil, que viaja en el fichero).
    ///
    /// Esconderlas **no las borra**: es para mirar el dibujo limpio un
    /// momento y volver. Escondidas no se pintan, no salen al exportar y no se
    /// eligen ([`Escena::a_la_vista`], `impacto::Mundo`). No va al historial,
    /// como en el movil: es mirar, no dibujar.
    #[serde(default = "verdadero")]
    pub referencias_visibles: bool,
    /// **Las tablas de coordenadas** (`Scene.tablas` del movil): series de
    /// puntos tecleados, de la escena y no de ningun elemento. Ver
    /// `instrumento::tabla`.
    #[serde(default)]
    pub tablas: Vec<crate::instrumento::tabla::TablaDeCoordenadas>,
    /// **El eje comun de las coordenadas** (`Scene.origenCoordenadas`): uno
    /// para todo el dibujo, para que dos series se puedan comparar.
    #[serde(default)]
    pub origen_coordenadas: Option<Punto2>,
    /// Lo que se ha hecho en ESTA sesion, para deshacerlo. No se guarda: el
    /// historial es estado de la interfaz, no del documento, y deshacer al
    /// abrir un dibujo de ayer un trazo que no se ve hacer confunde mas de
    /// lo que ayuda.
    #[serde(skip)]
    historia: Vec<Paso>,
    #[serde(skip)]
    rehacer: Vec<Paso>,
    /// El paso que se esta construyendo, entre `abrir_paso` y `cerrar_paso`.
    #[serde(skip)]
    en_curso: Option<Paso>,
    /// Lo que ocupan `historia` y `rehacer`, para no tener que recorrerlos.
    #[serde(skip)]
    bytes_historial: usize,
    /// Cuantos pasos nuevos ha cerrado el usuario. Sube y nunca baja: es
    /// una senal para quien lleva otra pila encima de esta (D234).
    #[serde(skip)]
    cerrados: u64,
}

/// Un cambio suelto. Cada uno sabe invertirse.
#[derive(Debug, Clone, PartialEq)]
enum Cambio {
    Anadido(u64),
    Borrado(u64),
    /// El elemento entero **antes** del cambio.
    ///
    /// Guardar el estado anterior y no la operacion inversa es D24, y no es
    /// preferencia de estilo: en coma flotante `(a * 1.5) / 1.5` no siempre
    /// devuelve `a`. Con la operacion inversa, treinta ciclos de deshacer y
    /// rehacer deforman el dibujo poco a poco — un fallo que aparece en
    /// casa del usuario y no en las pruebas.
    Editado {
        id: u64,
        antes: Box<Elemento>,
    },
    /// El orden completo de los ids **antes** del cambio.
    ///
    /// Reordenar no toca ningun elemento, asi que no se puede expresar con
    /// `Editado`: lo que cambia es la lista, no su contenido. Y el orden de
    /// la lista ES el orden de pintado, asi que perderlo se ve.
    Reordenado(Vec<u64>),
    /// La escala **antes** del cambio.
    ///
    /// Calibrar no toca ningun elemento, asi que no se puede expresar con
    /// `Editado`: lo que cambia es la escena.
    Escala(Option<Escala>),
    /// El color del papel **antes** del cambio. Por lo mismo que `Escala`:
    /// no es de ningun elemento.
    Fondo(ColorRgba),
    /// Los clavos **antes** del cambio (`nudos`): tampoco son de ningun
    /// elemento.
    Alfileres(Vec<crate::nudos::Alfiler>),
    /// Las tablas de coordenadas y su eje **antes** del cambio: tampoco son
    /// de ningun elemento.
    Tablas(
        Vec<crate::instrumento::tabla::TablaDeCoordenadas>,
        Option<Punto2>,
    ),
}

/// Todo lo que hizo un gesto. Un arrastre que mueve cuarenta elementos es
/// **un** paso de deshacer, no cuarenta.
#[derive(Debug, Clone, PartialEq, Default)]
struct Paso {
    cambios: Vec<Cambio>,
    /// Los ids que ya tienen su `Cambio::Editado` en este paso. Sin esto,
    /// `apuntar_edicion` recorria `cambios` entero para no apuntar dos
    /// veces, y al arrastrar mil elementos eso era un millon de
    /// comparaciones por aviso del raton.
    editados: std::collections::HashSet<u64>,
}

impl Cambio {
    /// Lo que ocupa de cara al techo del historial.
    fn bytes(&self) -> usize {
        match self {
            Cambio::Anadido(_) | Cambio::Borrado(_) | Cambio::Fondo(_) => size_of::<Cambio>(),
            Cambio::Editado { antes, .. } => size_of::<Cambio>() + antes.bytes(),
            Cambio::Reordenado(orden) => size_of::<Cambio>() + orden.len() * size_of::<u64>(),
            Cambio::Escala(escala) => {
                size_of::<Cambio>() + escala.as_ref().map_or(0, |e| e.unidad.len())
            }
            Cambio::Tablas(lista, _) => {
                size_of::<Cambio>()
                    + lista
                        .iter()
                        .map(|t| t.id.len() + t.nombre.len() + t.color.len() + t.puntos.len() * 16)
                        .sum::<usize>()
            }
            Cambio::Alfileres(lista) => {
                size_of::<Cambio>()
                    + lista
                        .iter()
                        .map(|a| {
                            size_of::<crate::nudos::Alfiler>()
                                + a.agarres.len() * size_of::<crate::nudos::Agarre>()
                        })
                        .sum::<usize>()
            }
        }
    }
}

/// Techo del historial (D25). Ocho megas son quinientos arrastres de un
/// trazo de 492 puntos —mas de lo que nadie deshace de una sentada— y una
/// fraccion asumible de los ~1,5 GB que Windows 10 deja libres en la
/// maquina suelo de 4 GB.
const TECHO_HISTORIAL: usize = 8 * 1024 * 1024;

fn bytes_de(paso: &Paso) -> usize {
    paso.cambios.iter().map(Cambio::bytes).sum()
}

fn version_uno() -> u32 {
    1
}

fn uno_u64() -> u64 {
    1
}

fn verdadero() -> bool {
    true
}

/// El papel de un lienzo que no dice el suyo: blanco, el `#ffffff` que
/// escriben por defecto Excalidraw (`COLOR_PALETTE.white`) y el movil
/// (`DrawTheme.FONDO_DIA`).
pub const FONDO_DE_FABRICA: ColorRgba = ColorRgba::opaco(1.0, 1.0, 1.0);

fn fondo_de_fabrica() -> ColorRgba {
    FONDO_DE_FABRICA
}

impl Default for Escena {
    fn default() -> Self {
        Self {
            version: 1,
            siguiente_id: 1,
            elementos: Vec::new(),
            escala: None,
            fondo: FONDO_DE_FABRICA,
            alfileres: Vec::new(),
            referencias_visibles: true,
            tablas: Vec::new(),
            origen_coordenadas: None,
            historia: Vec::new(),
            rehacer: Vec::new(),
            en_curso: None,
            bytes_historial: 0,
            cerrados: 0,
        }
    }
}

impl Escena {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// Anade el elemento encima de todo y le asigna su id.
    pub fn anadir(&mut self, mut e: Elemento) -> u64 {
        let id = self.siguiente_id.max(1);
        self.siguiente_id = id + 1;
        e.id = id;
        self.elementos.push(e);
        self.apuntar_anadido(id);
        id
    }

    /// Mete un cambio suelto en el paso en curso, o le crea uno propio si no
    /// hay ninguno abierto. Hacer algo nuevo fuera de un paso corta la rama
    /// de rehacer, como en cualquier editor.
    ///
    /// Este es el camino que usan `anadir`, `borrar_apuntando` y
    /// `apuntar_movimiento` cuando se llaman sin `abrir_paso` alrededor —
    /// que es como los usa la app de verdad al arrastrar (ve
    /// `apps/pixpin/src/capa.rs` y `pines.rs`). Tiene que podar igual que
    /// `cerrar_paso`: si no, el techo de D25 solo se cumple mientras se
    /// dibuje a traves de `abrir_paso`/`cerrar_paso`.
    fn empujar_cambio(&mut self, cambio: Cambio) {
        match &mut self.en_curso {
            Some(paso) => {
                // Lo que entra por aqui tambien cuenta como apuntado: antes
                // `apuntar_edicion` lo veia recorriendo `cambios`.
                if let Cambio::Editado { id, .. } = &cambio {
                    paso.editados.insert(*id);
                }
                paso.cambios.push(cambio);
            }
            None => {
                let paso = Paso {
                    cambios: vec![cambio],
                    ..Paso::default()
                };
                self.bytes_historial += bytes_de(&paso);
                self.historia.push(paso);
                self.limpiar_rehacer();
                self.podar();
            }
        }
    }

    /// Vacia la rama de rehacer y descuenta lo que ocupaba. `Vec::clear` sin
    /// esto dejaria `bytes_historial` contando bytes de pasos que ya no
    /// existen, y el techo de D25 se aplicaria sobre un numero mentiroso.
    fn limpiar_rehacer(&mut self) {
        for p in self.rehacer.drain(..) {
            self.bytes_historial -= bytes_de(&p);
        }
        // Todo paso nuevo del usuario pasa por aqui (`cerrar_paso` y el
        // cambio suelto sin paso abierto); deshacer y rehacer, no. Por eso
        // el contador vive aqui y no en cada `historia.push`.
        self.cerrados += 1;
    }

    /// Cuantos pasos nuevos se han cerrado desde que se abrio: sube con cada
    /// uno y no baja al deshacer. Lo usa quien tiene su propia pila y tiene
    /// que saber que el usuario hizo algo aqui (el universo, D234).
    pub fn pasos_cerrados(&self) -> u64 {
        self.cerrados
    }

    fn apuntar_anadido(&mut self, id: u64) {
        self.empujar_cambio(Cambio::Anadido(id));
    }

    /// Desplaza un elemento SIN apuntarlo: es cada pixel de un arrastre en
    /// curso. Lo que se deshace es el arrastre entero, y ese se apunta al
    /// soltar con `apuntar_movimiento`.
    pub fn mover(&mut self, id: u64, dx: f32, dy: f32) -> bool {
        match self.buscar_mut(id) {
            Some(e) => {
                e.mover(dx, dy);
                e.tocar();
                true
            }
            None => false,
        }
    }

    /// Cierra un arrastre: apunta el desplazamiento total como UN paso. Un
    /// arrastre que acaba donde empezo no deja rastro en el historial.
    ///
    /// El elemento ya esta en su posicion final cuando se llama: el "antes"
    /// se reconstruye desplazandolo al reves, que para una traslacion (una
    /// suma) es exacto, a diferencia de una escala (D24).
    pub fn apuntar_movimiento(&mut self, id: u64, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let Some(e) = self.buscar(id) else { return };
        let mut antes = e.clone();
        antes.mover(-dx, -dy);
        self.empujar_cambio(Cambio::Editado {
            id,
            antes: Box::new(antes),
        });
    }

    /// Borrado a peticion del usuario (el borrador, la tecla Suprimir): se
    /// apunta para poder deshacerlo.
    ///
    /// **Suelta tambien las flechas atadas** (ver `enlace::soltar_lo_borrado`)
    /// y lo hace aqui, y no en cada sitio que borra, porque se borra desde
    /// cinco sitios —Suprimir, el borrador, el panel, los pines, la capa—, y
    /// el que se olvidara dejaria en el fichero una flecha atada a nada.
    ///
    /// Todo va a un solo paso: si no habia ninguno abierto se abre uno
    /// para esto, porque deshacer el borrado y dejar la flecha suelta
    /// dejaria la flecha sin su caja justo cuando la caja vuelve.
    pub fn borrar_apuntando(&mut self, id: u64) -> bool {
        if !self.borrar(id) {
            return false;
        }
        let abierto_aqui = self.en_curso.is_none();
        self.abrir_paso();
        self.empujar_cambio(Cambio::Borrado(id));
        crate::enlace::soltar_lo_borrado(self, id);
        if abierto_aqui {
            self.cerrar_paso();
        }
        true
    }

    pub fn buscar(&self, id: u64) -> Option<&Elemento> {
        self.elementos.iter().find(|e| e.id == id)
    }

    pub fn buscar_mut(&mut self, id: u64) -> Option<&mut Elemento> {
        self.elementos.iter_mut().find(|e| e.id == id)
    }

    /// Los que se ven, en orden de dibujo.
    pub fn visibles(&self) -> impl Iterator<Item = &Elemento> {
        self.elementos.iter().filter(|e| !e.borrado)
    }

    pub fn cuantos_visibles(&self) -> usize {
        self.visibles().count()
    }

    /// **Lo que se ve de verdad**: sin borrar y, si las lineas de referencia
    /// estan escondidas, sin ellas (`visibleConReferencias` del movil). Es lo
    /// que se pinta y lo que se exporta. `visibles` sigue contando las
    /// escondidas a proposito: guardar las tiene que llevar igual.
    pub fn a_la_vista(&self) -> impl Iterator<Item = &Elemento> {
        let todas = self.referencias_visibles;
        self.visibles()
            .filter(move |e| todas || !e.extras.referencia)
    }

    /// Si hay alguna linea de referencia (`hayReferencias` del movil): sin
    /// ninguna, el ojo de esconderlas no dice nada y no sale.
    pub fn hay_referencias(&self) -> bool {
        self.visibles().any(|e| e.extras.referencia)
    }

    /// Borrado logico. `true` si habia algo que borrar.
    pub fn borrar(&mut self, id: u64) -> bool {
        match self.buscar_mut(id) {
            Some(e) if !e.borrado => {
                e.borrado = true;
                e.tocar();
                true
            }
            _ => false,
        }
    }

    pub fn restaurar(&mut self, id: u64) -> bool {
        match self.buscar_mut(id) {
            Some(e) if e.borrado => {
                e.borrado = false;
                e.tocar();
                true
            }
            _ => false,
        }
    }

    /// Empieza un gesto. Todo lo que pase hasta `cerrar_paso` sera un solo
    /// paso de deshacer.
    ///
    /// Abrir dos veces sin cerrar no es error: el segundo abrir no hace
    /// nada. La ventana puede recibir un `WM_LBUTTONDOWN` sin su
    /// `WM_LBUTTONUP` si el usuario suelta fuera, y perder el gesto entero
    /// por eso seria peor que ignorarlo.
    pub fn abrir_paso(&mut self) {
        if self.en_curso.is_none() {
            self.en_curso = Some(Paso::default());
        }
    }

    /// Guarda el estado actual de un elemento para poder volver a el.
    ///
    /// Apuntarlo dos veces dentro del mismo paso guarda **solo la primera**:
    /// mover el raton produce cien avisos por gesto, y cien instantaneas de
    /// un trazo largo se comerian el techo en un solo arrastre.
    pub fn apuntar_edicion(&mut self, id: u64) {
        let Some(paso) = &self.en_curso else { return };
        if paso.editados.contains(&id) {
            return;
        }
        let Some(e) = self.buscar(id) else { return };
        let cambio = Cambio::Editado {
            id,
            antes: Box::new(e.clone()),
        };
        if let Some(paso) = &mut self.en_curso {
            paso.cambios.push(cambio);
            paso.editados.insert(id);
        }
    }

    /// Guarda el orden actual de los ids para poder volver a el.
    ///
    /// Apuntarlo dos veces dentro del mismo paso guarda **solo la primera**:
    /// igual que `apuntar_edicion`.
    pub fn apuntar_reordenamiento(&mut self) {
        let Some(paso) = &self.en_curso else { return };
        let ya_esta = paso
            .cambios
            .iter()
            .any(|c| matches!(c, Cambio::Reordenado(_)));
        if ya_esta {
            return;
        }
        let orden: Vec<u64> = self.elementos.iter().map(|e| e.id).collect();
        let cambio = Cambio::Reordenado(orden);
        if let Some(paso) = &mut self.en_curso {
            paso.cambios.push(cambio);
        }
    }

    /// **Pone las tablas de coordenadas y su eje**, apuntandolo para
    /// deshacerlo (aceptar el editor de una tabla, quitarla, abrir una serie
    /// nueva: `updateTabla`, `removeTabla`, `addTabla` del movil). Devuelve
    /// si cambio algo: aceptar sin tocar nada no gasta un `Ctrl+Z`.
    pub fn poner_tablas(
        &mut self,
        tablas: Vec<crate::instrumento::tabla::TablaDeCoordenadas>,
        origen: Option<Punto2>,
    ) -> bool {
        if self.tablas == tablas && self.origen_coordenadas == origen {
            return false;
        }
        let ya_apuntado = self
            .en_curso
            .as_ref()
            .is_some_and(|p| p.cambios.iter().any(|c| matches!(c, Cambio::Tablas(..))));
        if !ya_apuntado {
            let antes = Cambio::Tablas(self.tablas.clone(), self.origen_coordenadas);
            self.empujar_cambio(antes);
        }
        self.tablas = tablas;
        self.origen_coordenadas = origen;
        true
    }

    /// **Cambia el color del papel**, apuntandolo para deshacerlo. Devuelve
    /// si cambio algo: elegir el papel que ya estaba no gasta un `Ctrl+Z`.
    ///
    /// Solo papel opaco: el lienzo no tiene nada detras que se pueda ver, y
    /// «sin papel» ya lo pide quien exporta con su «fondo transparente». Un
    /// alfa aqui llegaria al movil como `#rrggbbaa`, que el lee quitandole
    /// el alfa (`DrawTheme.colorDe`): se veria distinto en cada lado.
    pub fn poner_fondo(&mut self, color: ColorRgba) -> bool {
        let color = ColorRgba { a: 1.0, ..color };
        if self.fondo == color {
            return false;
        }
        // Dentro de un paso abierto, solo el primero: deshacer vuelve al
        // papel de antes del gesto, no al penultimo que se probo.
        let ya_apuntado = self
            .en_curso
            .as_ref()
            .is_some_and(|p| p.cambios.iter().any(|c| matches!(c, Cambio::Fondo(_))));
        if !ya_apuntado {
            self.empujar_cambio(Cambio::Fondo(self.fondo));
        }
        self.fondo = color;
        true
    }

    /// Guarda la escala actual para poder volver a ella.
    ///
    /// Apuntarlo dos veces dentro del mismo paso guarda **solo la primera**:
    /// igual que `apuntar_edicion` y `apuntar_reordenamiento`.
    pub fn apuntar_escala(&mut self) {
        let Some(paso) = &self.en_curso else { return };
        let ya_esta = paso.cambios.iter().any(|c| matches!(c, Cambio::Escala(_)));
        if ya_esta {
            return;
        }
        let cambio = Cambio::Escala(self.escala.clone());
        if let Some(paso) = &mut self.en_curso {
            paso.cambios.push(cambio);
        }
    }

    /// Guarda los clavos actuales para poder volver a ellos (`nudos`).
    ///
    /// Apuntarlo dos veces dentro del mismo paso guarda **solo la primera**,
    /// como `apuntar_escala`. Fuera de un paso no hace nada.
    pub fn apuntar_alfileres(&mut self) {
        let Some(paso) = &self.en_curso else { return };
        if paso
            .cambios
            .iter()
            .any(|c| matches!(c, Cambio::Alfileres(_)))
        {
            return;
        }
        let cambio = Cambio::Alfileres(self.alfileres.clone());
        if let Some(paso) = &mut self.en_curso {
            paso.cambios.push(cambio);
        }
    }

    /// Cierra el gesto. Un paso sin cambios no entra en el historial: hacer
    /// clic sin arrastrar no debe consumir un `Ctrl+Z`.
    pub fn cerrar_paso(&mut self) {
        let Some(mut paso) = self.en_curso.take() else {
            return;
        };
        // `apuntar_edicion` toma la instantanea sin saber si el elemento va
        // a cambiar de verdad (lo hace al pulsar, para que el arrastre que
        // sigue no asigne memoria). Un clic de seleccion sobre algo ya
        // elegido, sin arrastre, deja entonces un `Editado` cuyo `antes` es
        // igual al elemento actual: no es un cambio, y si se colara en el
        // historial, `Ctrl+Z` no deshaceria nada visible. Se filtra aqui,
        // antes de mirar si el paso quedo vacio.
        //
        // `Anadido`, `Borrado`, `Reordenado` y `Escala` no se filtran: esos
        // solo se empujan cuando la accion ocurrio de verdad.
        paso.cambios.retain(|c| match c {
            Cambio::Editado { id, antes } => self.buscar(*id) != Some(antes.as_ref()),
            // Los clavos se apuntan por si acaso (todo gesto con clavos los
            // apunta): solo cuentan si de verdad cambiaron.
            Cambio::Alfileres(antes) => *antes != self.alfileres,
            _ => true,
        });
        if paso.cambios.is_empty() {
            return;
        }
        self.bytes_historial += bytes_de(&paso);
        self.historia.push(paso);
        self.limpiar_rehacer();
        self.podar();
    }

    /// Deshace el gesto en curso sin apuntarlo. Es lo que hace `Escape` a
    /// mitad de un arrastre, y sale gratis porque el paso ya guarda el
    /// estado anterior de lo que se estaba tocando.
    pub fn cancelar_paso(&mut self) {
        let Some(paso) = self.en_curso.take() else {
            return;
        };
        self.aplicar_inverso(&paso);
    }

    /// Deshace el ultimo gesto. Devuelve si habia algo que deshacer.
    pub fn deshacer(&mut self) -> bool {
        self.viajar_por_historial(true)
    }

    /// Rehace el ultimo gesto deshecho.
    pub fn rehacer(&mut self) -> bool {
        self.viajar_por_historial(false)
    }

    /// Cuerpo comun de `deshacer` y `rehacer`: solo cambia cual pila es el
    /// origen y cual el destino. Es el mismo argumento que ya lleva el
    /// comentario de `aplicar_inverso`: dos funciones separadas que hacen lo
    /// mismo acaban discrepando en cuanto se anada una variante a `Cambio`.
    fn viajar_por_historial(&mut self, deshaciendo: bool) -> bool {
        let origen = if deshaciendo {
            &mut self.historia
        } else {
            &mut self.rehacer
        };
        let Some(paso) = origen.pop() else {
            return false;
        };
        self.bytes_historial -= bytes_de(&paso);
        let inverso = self.aplicar_inverso(&paso);
        self.bytes_historial += bytes_de(&inverso);
        let destino = if deshaciendo {
            &mut self.rehacer
        } else {
            &mut self.historia
        };
        destino.push(inverso);
        // Podar aqui tambien: sin esto, un usuario que solo deshace (nunca
        // cierra un paso nuevo) podria crecer el historial sin techo.
        self.podar();
        true
    }

    /// Aplica el paso al reves y devuelve el paso que lo desharia.
    ///
    /// Que devuelva su propio inverso es lo que hace que deshacer y rehacer
    /// sean la misma funcion. Con dos funciones distintas, acabarian
    /// discrepando en cuanto se anadiera una variante a `Cambio`.
    fn aplicar_inverso(&mut self, paso: &Paso) -> Paso {
        let mut inverso = Paso::default();
        // Al reves: si un gesto borro y luego anadio, deshacerlo tiene que
        // quitar lo anadido antes de restaurar lo borrado.
        for c in paso.cambios.iter().rev() {
            match c {
                Cambio::Anadido(id) => {
                    self.borrar(*id);
                    inverso.cambios.push(Cambio::Borrado(*id));
                }
                Cambio::Borrado(id) => {
                    self.restaurar(*id);
                    inverso.cambios.push(Cambio::Anadido(*id));
                }
                Cambio::Editado { id, antes } => {
                    if let Some(i) = self.elementos.iter().position(|e| e.id == *id) {
                        let ahora = self.elementos[i].clone();
                        self.elementos[i] = (**antes).clone();
                        inverso.cambios.push(Cambio::Editado {
                            id: *id,
                            antes: Box::new(ahora),
                        });
                    }
                }
                Cambio::Reordenado(orden_anterior) => {
                    // Guarda el orden actual y restaura el anterior.
                    let orden_actual: Vec<u64> = self.elementos.iter().map(|e| e.id).collect();
                    // Reordena segun la lista guardada, ignorando ids que ya no existen.
                    let mut nuevos_elementos = Vec::new();
                    for id in orden_anterior {
                        if let Some(e) = self.elementos.iter().find(|e| e.id == *id) {
                            nuevos_elementos.push(e.clone());
                        }
                    }
                    self.elementos = nuevos_elementos;
                    inverso.cambios.push(Cambio::Reordenado(orden_actual));
                }
                Cambio::Escala(anterior) => {
                    let actual = self.escala.clone();
                    self.escala = anterior.clone();
                    inverso.cambios.push(Cambio::Escala(actual));
                }
                Cambio::Fondo(anterior) => {
                    let actual = self.fondo;
                    self.fondo = *anterior;
                    inverso.cambios.push(Cambio::Fondo(actual));
                }
                Cambio::Alfileres(anterior) => {
                    let actual = std::mem::replace(&mut self.alfileres, anterior.clone());
                    inverso.cambios.push(Cambio::Alfileres(actual));
                }
                Cambio::Tablas(tablas, origen) => {
                    let actual = std::mem::replace(&mut self.tablas, tablas.clone());
                    let eje = std::mem::replace(&mut self.origen_coordenadas, *origen);
                    inverso.cambios.push(Cambio::Tablas(actual, eje));
                }
            }
        }
        inverso
    }

    /// **Junta en un solo paso de deshacer todo lo cerrado desde `desde`**
    /// (un valor de [`Escena::pasos_cerrados`] tomado antes).
    ///
    /// Lo usa quien hace varias operaciones del usuario seguidas por una sola
    /// orden —el asistente: «corta la recta en el cruce» es pulsar, recortar
    /// y quiza volver a pulsar, pero el usuario pidio UNA cosa y la quiere
    /// deshacer con un solo `Ctrl+Z`—. Los cambios se encadenan en su orden,
    /// que es justo lo que `aplicar_inverso` sabe recorrer al reves.
    ///
    /// Si la poda tiro alguno de esos pasos (historial lleno) se juntan los
    /// que quedan: nunca mas de los que hay.
    pub fn juntar_pasos(&mut self, desde: u64) {
        let nuevos = self.cerrados.saturating_sub(desde) as usize;
        let n = nuevos.min(self.historia.len());
        if n < 2 {
            return;
        }
        let corte = self.historia.len() - n;
        let mut junto = Paso::default();
        let viejos: Vec<Paso> = self.historia.drain(corte..).collect();
        for paso in viejos {
            self.bytes_historial -= bytes_de(&paso);
            junto.editados.extend(paso.editados);
            junto.cambios.extend(paso.cambios);
        }
        self.bytes_historial += bytes_de(&junto);
        self.historia.push(junto);
    }

    /// **Vuelve a una copia de sus elementos** (un `elementos.clone()`
    /// tomado antes) como UN paso nuevo de deshacer, no deshaciendo.
    ///
    /// Lo usa la revision del asistente (22-oct): si una correccion deja el
    /// dibujo peor que la version guardada, se vuelve a esa. Deshacer los
    /// pasos no vale: `pasos_cerrados` no baja al deshacer, y el paso que
    /// junta todo el turno en un solo `Ctrl+Z` (`juntar_pasos`) se comeria
    /// pasos del usuario de antes del turno. Como paso nuevo, el `Ctrl+Z`
    /// del turno lo deshace todo junto y el fichero se guarda como siempre.
    ///
    /// Lo que no estaba en la copia (anadido despues) se borra; lo que
    /// estaba vuelve tal cual (con su version subida, para que la
    /// sincronizacion lo tome por nuevo), y el orden vuelve al de la copia.
    /// `siguiente_id` no baja: los ids nunca se reusan. Devuelve si cambio
    /// algo.
    pub fn volver_a(&mut self, copia: &[Elemento]) -> bool {
        let abierto_aqui = self.en_curso.is_none();
        self.abrir_paso();
        // El orden: el de la copia, y detras lo que ella no tenia.
        let mut orden: Vec<u64> = copia
            .iter()
            .map(|c| c.id)
            .filter(|id| self.buscar(*id).is_some())
            .collect();
        orden.extend(
            self.elementos
                .iter()
                .map(|e| e.id)
                .filter(|id| !copia.iter().any(|c| c.id == *id)),
        );
        let ahora: Vec<u64> = self.elementos.iter().map(|e| e.id).collect();
        let mut cambio = false;
        if orden != ahora {
            self.apuntar_reordenamiento();
            let mut viejos = std::mem::take(&mut self.elementos);
            for id in &orden {
                if let Some(i) = viejos.iter().position(|e| e.id == *id) {
                    self.elementos.push(viejos.remove(i));
                }
            }
            cambio = true;
        }
        for id in orden {
            let Some(actual) = self.buscar(id) else {
                continue;
            };
            let objetivo = match copia.iter().find(|c| c.id == id) {
                Some(c) => c.clone(),
                None if actual.borrado => continue,
                None => Elemento {
                    borrado: true,
                    ..actual.clone()
                },
            };
            if *actual == objetivo {
                continue;
            }
            let version = actual.version.max(objetivo.version);
            self.apuntar_edicion(id);
            if let Some(e) = self.buscar_mut(id) {
                *e = objetivo;
                e.version = version;
                e.tocar();
            }
            cambio = true;
        }
        if abierto_aqui {
            self.cerrar_paso();
        }
        cambio
    }

    /// Tira los pasos mas antiguos hasta caber en el techo (D25).
    ///
    /// El techo va en memoria y no en numero de pasos porque quinientos
    /// pasos son ocho kilobytes o doscientos megas segun lo que se haya
    /// tocado, y en un equipo de 4 GB eso no se puede prometer.
    fn podar(&mut self) {
        while self.bytes_historial > TECHO_HISTORIAL && self.historia.len() > 1 {
            let viejo = self.historia.remove(0);
            self.bytes_historial -= bytes_de(&viejo);
        }
    }

    /// Lo que ocupa el historial ahora mismo.
    pub fn bytes_de_historial(&self) -> usize {
        self.bytes_historial
    }

    /// Si hay algo que deshacer en esta sesion.
    pub fn hay_que_deshacer(&self) -> bool {
        !self.historia.is_empty()
    }

    /// Cuantos pasos hay para deshacer y para rehacer ahora mismo.
    pub fn pasos_en_historial(&self) -> (usize, usize) {
        (self.historia.len(), self.rehacer.len())
    }

    /// **Apunta como un solo paso nuevo que se anadieron `ids`**, que ya
    /// estan en la escena. Es lo que llego de fuera de una vez (lo que el
    /// asistente escribio en la capa de un documento, releida del disco):
    /// un `Ctrl+Z` lo quita entero, como si se hubiera dibujado aqui. Los
    /// que no esten se ignoran; sin ninguno no hay paso.
    pub fn apuntar_anadidos(&mut self, ids: &[u64]) {
        let ids: Vec<u64> = ids
            .iter()
            .copied()
            .filter(|id| self.buscar(*id).is_some())
            .collect();
        if ids.is_empty() {
            return;
        }
        self.abrir_paso();
        for id in ids {
            self.apuntar_anadido(id);
        }
        self.cerrar_paso();
    }

    /// Sube el elemento al frente sin cambiar el resto del orden.
    pub fn traer_al_frente(&mut self, id: u64) {
        if let Some(i) = self.elementos.iter().position(|e| e.id == id) {
            let e = self.elementos.remove(i);
            self.elementos.push(e);
        }
    }

    /// Saca de verdad los elementos borrados. Se llama al guardar, no al
    /// deshacer: hasta entonces, deshacer tiene que poder revertirse.
    pub fn compactar(&mut self) {
        self.elementos.retain(|e| !e.borrado);
    }

    /// La caja que abarca todo lo visible, o `None` si no hay nada.
    pub fn caja(&self) -> Option<(f32, f32, f32, f32)> {
        let mut iter = self.visibles();
        let primera = iter.next()?.caja();
        Some(iter.fold(primera, |(x0, y0, x1, y1), e| {
            let (a, b, c, d) = e.caja();
            (x0.min(a), y0.min(b), x1.max(c), y1.max(d))
        }))
    }

    /// El elemento visible de mas arriba bajo el punto.
    pub fn elemento_en(&self, p: Punto2) -> Option<u64> {
        self.elementos
            .iter()
            .rev()
            .find(|e| crate::impacto::toca(e, p))
            .map(|e| e.id)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn base() -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    fn rect(x: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y: 0.0,
            ancho: 50.0,
            alto: 50.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: Some(ColorRgba::opaco(1.0, 1.0, 1.0)),
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    #[test]
    fn deshacer_un_arrastre_devuelve_el_elemento_a_su_sitio() {
        // Antes solo se deshacian altas y bajas: mover algo no se podia
        // deshacer, que es justo lo que se nota al poder seleccionar.
        let mut e = Escena::nueva();
        let id = e.anadir(rect(10.0));
        // Un arrastre son muchos pasos y UN solo paso de historial.
        e.mover(id, 5.0, 0.0);
        e.mover(id, 25.0, 10.0);
        e.apuntar_movimiento(id, 30.0, 10.0);
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (40.0, 10.0)
        );

        assert!(e.deshacer());
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (10.0, 0.0),
            "el arrastre entero se deshace de una vez"
        );
        assert!(!e.buscar(id).unwrap().borrado, "deshacer mover no borra");

        assert!(e.rehacer());
        assert_eq!(
            (e.buscar(id).unwrap().x, e.buscar(id).unwrap().y),
            (40.0, 10.0)
        );

        // Y el siguiente deshacer se lleva el alta, que es el paso anterior.
        e.deshacer();
        assert!(e.deshacer());
        assert!(e.buscar(id).unwrap().borrado);
    }

    #[test]
    fn un_arrastre_que_acaba_donde_empezo_no_deja_paso() {
        let mut e = Escena::nueva();
        let id = e.anadir(rect(10.0));
        e.apuntar_movimiento(id, 0.0, 0.0);
        // Solo esta el alta: deshacer una vez y ya no queda nada.
        assert!(e.hay_que_deshacer());
        e.deshacer();
        assert!(!e.hay_que_deshacer());
    }

    #[test]
    fn hacer_algo_nuevo_corta_la_rama_de_rehacer() {
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        e.deshacer();
        e.anadir(rect(100.0));
        assert!(!e.rehacer(), "el rehacer viejo ya no vale");
        assert!(
            e.buscar(uno).unwrap().borrado,
            "y lo deshecho sigue deshecho"
        );
    }

    #[test]
    fn el_borrador_se_deshace() {
        let mut e = Escena::nueva();
        let id = e.anadir(rect(0.0));
        assert!(e.borrar_apuntando(id));
        assert_eq!(e.cuantos_visibles(), 0);
        e.deshacer();
        assert_eq!(e.cuantos_visibles(), 1, "vuelve lo borrado a mano");
    }

    #[test]
    fn el_historial_no_viaja_en_el_fichero() {
        // Caso negativo: al abrir un dibujo de ayer no hay nada que
        // deshacer, porque no se hizo nada en esta sesion.
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let texto = serde_json::to_string(&e).unwrap();
        let vuelta: Escena = serde_json::from_str(&texto).unwrap();
        assert_eq!(vuelta.cuantos_visibles(), 1);
        assert!(!vuelta.hay_que_deshacer());
        assert_eq!(vuelta.elementos, e.elementos);
    }

    #[test]
    fn los_ids_no_se_repiten_ni_empiezan_en_cero() {
        // El cero es un id valido en el mundo, pero aqui es la señal de
        // "sin asignar" del elemento recien construido.
        let mut e = Escena::nueva();
        let a = e.anadir(rect(0.0));
        let b = e.anadir(rect(100.0));
        assert_eq!((a, b), (1, 2));
        assert_ne!(a, b);
    }

    #[test]
    fn deshacer_y_rehacer_vuelven_al_mismo_sitio() {
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let segundo = e.anadir(rect(100.0));

        assert!(e.deshacer(), "deshace el ultimo");
        assert_eq!(e.cuantos_visibles(), 1);
        assert!(e.buscar(segundo).unwrap().borrado);
        assert!(e.rehacer());
        assert_eq!(e.cuantos_visibles(), 2);
        assert!(!e.buscar(segundo).unwrap().borrado);
    }

    #[test]
    fn deshacer_sobre_una_escena_vacia_no_entra_en_panico() {
        let mut e = Escena::nueva();
        assert!(!e.deshacer());
        assert!(!e.rehacer());
    }

    #[test]
    fn deshacer_dos_veces_deshace_dos_elementos_distintos() {
        // Caso negativo del anterior: una implementacion que solo mirase el
        // ultimo de la lista deshareria siempre el mismo.
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        let dos = e.anadir(rect(100.0));
        assert!(e.deshacer());
        assert!(e.buscar(dos).unwrap().borrado);
        assert!(e.deshacer());
        assert!(e.buscar(uno).unwrap().borrado);
        assert_eq!(e.cuantos_visibles(), 0);
    }

    #[test]
    fn compactar_saca_los_borrados_y_deja_los_demas() {
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let dos = e.anadir(rect(100.0));
        e.borrar(dos);
        assert_eq!(e.elementos.len(), 2, "antes de compactar siguen los dos");
        e.compactar();
        assert_eq!(e.elementos.len(), 1);
        assert_eq!(e.elementos[0].x, 0.0);
    }

    #[test]
    fn traer_al_frente_cambia_quien_recibe_el_clic() {
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        let dos = e.anadir(rect(0.0));
        let p = Punto2::nuevo(25.0, 25.0);
        assert_eq!(e.elemento_en(p), Some(dos));
        e.traer_al_frente(uno);
        assert_eq!(e.elemento_en(p), Some(uno));
    }

    #[test]
    fn la_caja_abarca_todo_lo_visible_y_solo_eso() {
        let mut e = Escena::nueva();
        e.anadir(rect(0.0));
        let lejos = e.anadir(rect(1000.0));
        assert_eq!(e.caja(), Some((0.0, 0.0, 1050.0, 50.0)));
        // Un elemento borrado no cuenta: si contara, la vista "ajustar a la
        // ventana" se alejaria hasta un trazo que ya no existe.
        e.borrar(lejos);
        assert_eq!(e.caja(), Some((0.0, 0.0, 50.0, 50.0)));
    }

    #[test]
    fn una_escena_vacia_no_tiene_caja() {
        assert_eq!(Escena::nueva().caja(), None);
    }

    #[test]
    fn borrar_dos_veces_lo_mismo_no_cuenta_dos_veces() {
        let mut e = Escena::nueva();
        let uno = e.anadir(rect(0.0));
        assert!(e.borrar(uno));
        assert!(!e.borrar(uno), "ya estaba borrado");
        assert!(!e.borrar(999), "no existe");
    }

    #[test]
    fn un_paso_agrupa_todos_los_cambios_del_gesto() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(base());
        let b = escena.anadir(base());
        let c = escena.anadir(base());

        escena.abrir_paso();
        for id in [a, b, c] {
            escena.apuntar_edicion(id);
            escena.buscar_mut(id).unwrap().mover(10.0, 0.0);
        }
        escena.cerrar_paso();

        assert!(escena.deshacer(), "tiene que haber algo que deshacer");
        assert_eq!(escena.buscar(a).unwrap().x, 0.0, "el primero vuelve");
        assert_eq!(escena.buscar(c).unwrap().x, 0.0, "y el tercero tambien");
        assert!(escena.deshacer(), "queda deshacer el haber anadido");
    }

    #[test]
    fn muchos_ciclos_de_deshacer_y_rehacer_alternados_no_deforman_el_dibujo() {
        // El motivo de D24. Con la operacion inversa en vez del estado
        // anterior, escalar por 1,5 y dividir por 1,5 acumularia error en coma
        // flotante hasta que el dibujo se nota torcido.
        //
        // Deshacer y volver a deshacer en cada vuelta (sin rehacer nunca) es
        // trivialmente verde: cada vuelta parte del mismo estado y no hay
        // forma de que se acumule nada. Esta version atraviesa las dos pilas
        // repetidas veces por cada paso —deshacer, rehacer, deshacer— antes
        // de pasar al siguiente, y solo compara contra `partida` al final.
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());
        let partida = escena.buscar(id).unwrap().clone();

        for _ in 0..30 {
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            let e = escena.buscar_mut(id).unwrap();
            e.ancho *= 1.5;
            e.alto *= 1.5;
            e.angulo += 0.37;
            e.tocar();
            escena.cerrar_paso();
        }

        // Un paso deshecho tres veces y rehecho dos por cada uno de los
        // treinta cambios: net, se deshace exactamente uno por vuelta, sin
        // tocar nunca el paso de "anadir" que queda debajo.
        for _ in 0..30 {
            assert!(escena.deshacer());
            assert!(escena.rehacer());
            assert!(escena.deshacer());
        }

        assert_eq!(
            escena.buscar(id).unwrap(),
            &partida,
            "identico bit a bit, no aproximado"
        );
    }

    #[test]
    fn cancelar_un_paso_deja_todo_como_estaba_y_no_ensucia_el_historial() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());

        escena.abrir_paso();
        escena.apuntar_edicion(id);
        escena.buscar_mut(id).unwrap().mover(500.0, 500.0);
        escena.cancelar_paso();

        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "vuelve a su sitio");
        assert!(escena.deshacer(), "queda el paso de haberlo anadido");
        assert!(
            !escena.deshacer(),
            "y nada mas: el arrastre cancelado no cuenta"
        );
    }

    #[test]
    fn un_gesto_sin_cambios_no_consume_un_ctrl_zeta() {
        let mut escena = Escena::nueva();
        escena.anadir(base());
        escena.abrir_paso();
        escena.cerrar_paso();

        assert!(escena.deshacer(), "el anadido");
        assert!(!escena.deshacer(), "el clic sin arrastrar no dejo paso");
    }

    #[test]
    fn apuntar_dos_veces_el_mismo_elemento_guarda_solo_el_estado_original() {
        // Mover el raton produce cien avisos por gesto. Si cada uno guardara
        // una instantanea, arrastrar un trazo largo se comeria el techo de
        // memoria en un solo arrastre.
        let mut escena = Escena::nueva();
        let id = escena.anadir(base());

        escena.abrir_paso();
        for _ in 0..100 {
            escena.apuntar_edicion(id);
            escena.buscar_mut(id).unwrap().mover(1.0, 0.0);
        }
        escena.cerrar_paso();

        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "vuelve al origen entero");
    }

    #[test]
    fn el_historial_no_pasa_de_ocho_megas() {
        // D25: el techo va en memoria, no en numero de pasos.
        let mut escena = Escena::nueva();
        let puntos: Vec<Punto2> = (0..492)
            .map(|i| Punto2::nuevo(i as f32, (i * 2) as f32))
            .collect();
        let id = escena.anadir(Elemento {
            figura: Figura::Lapiz {
                puntos,
                presiones: Vec::new(),
                opciones: None,
            },
            ..base()
        });

        for _ in 0..500 {
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            escena.buscar_mut(id).unwrap().mover(1.0, 0.0);
            escena.cerrar_paso();
        }

        assert!(
            escena.bytes_de_historial() <= 8 * 1024 * 1024,
            "el historial ocupa {} bytes",
            escena.bytes_de_historial()
        );
        assert!(escena.deshacer(), "y aun asi se deshace lo reciente");
    }

    #[test]
    fn un_editado_que_no_cambio_nada_se_descarta_y_uno_que_si_cambio_se_conserva() {
        // apuntar_edicion() guarda la instantanea sin saber si el elemento
        // va a cambiar de verdad; cerrar_paso() es quien tiene que
        // descartar los que no cambiaron, o un clic sin efecto ensuciaria
        // el historial. Las dos mitades juntas, para que el filtro no pase
        // por descartar de mas.
        let mut e = Escena::nueva();
        let id = e.anadir(base());

        // Mitad 1: se apunta y no se toca nada.
        e.abrir_paso();
        e.apuntar_edicion(id);
        e.cerrar_paso();
        assert!(e.deshacer(), "solo queda el paso de haberlo anadido");
        assert!(e.buscar(id).unwrap().borrado);
        assert!(!e.deshacer(), "el editado sin cambios no dejo paso propio");

        // Mitad 2: se rehace el alta y esta vez si se toca el elemento.
        e.rehacer();
        e.abrir_paso();
        e.apuntar_edicion(id);
        e.buscar_mut(id).unwrap().mover(10.0, 0.0);
        e.cerrar_paso();
        assert!(e.deshacer(), "este si cambio: hay paso que deshacer");
        assert_eq!(e.buscar(id).unwrap().x, 0.0, "y deshacerlo vuelve al sitio");
    }

    #[test]
    fn el_historial_no_pasa_de_ocho_megas_fuera_de_un_paso() {
        // Misma prueba que la de arriba, pero por el camino que usa la app
        // de verdad al arrastrar: apuntar_movimiento() sin abrir_paso ni
        // cerrar_paso alrededor (ve apps/pixpin/src/capa.rs:332 y
        // pines.rs:1060). empujar_cambio() tiene que podar tambien cuando
        // se crea su propio paso, no solo cuando lo cierra cerrar_paso.
        let mut escena = Escena::nueva();
        let puntos: Vec<Punto2> = (0..492)
            .map(|i| Punto2::nuevo(i as f32, (i * 2) as f32))
            .collect();
        let id = escena.anadir(Elemento {
            figura: Figura::Lapiz {
                puntos,
                presiones: Vec::new(),
                opciones: None,
            },
            ..base()
        });

        for _ in 0..500 {
            escena.mover(id, 1.0, 0.0);
            escena.apuntar_movimiento(id, 1.0, 0.0);
        }

        assert!(
            escena.bytes_de_historial() <= 8 * 1024 * 1024,
            "el historial ocupa {} bytes",
            escena.bytes_de_historial()
        );
        assert!(escena.deshacer(), "y aun asi se deshace lo reciente");
    }

    #[test]
    fn pasos_cerrados_sube_con_cada_paso_nuevo_y_no_al_deshacer() {
        let mut e = Escena::nueva();
        assert_eq!(e.pasos_cerrados(), 0);
        let id = e.anadir(rect(0.0));
        e.abrir_paso();
        e.borrar_apuntando(id);
        e.cerrar_paso();
        let tras_uno = e.pasos_cerrados();
        assert!(tras_uno >= 1);
        e.deshacer();
        assert_eq!(e.pasos_cerrados(), tras_uno);
    }

    #[test]
    fn cambiar_el_papel_se_deshace_y_se_rehace_como_en_excalidraw() {
        let mut e = Escena::nueva();
        assert_eq!(e.fondo, FONDO_DE_FABRICA);
        let crema = ColorRgba::opaco(0.99, 0.96, 0.89);
        assert!(e.poner_fondo(crema));
        assert_eq!(e.fondo, crema);
        assert!(e.deshacer());
        assert_eq!(e.fondo, FONDO_DE_FABRICA);
        assert!(e.rehacer());
        assert_eq!(e.fondo, crema);
    }

    #[test]
    fn elegir_el_papel_que_ya_estaba_no_gasta_un_deshacer() {
        // Caso negativo: nada cambia, nada se apunta.
        let mut e = Escena::nueva();
        assert!(!e.poner_fondo(FONDO_DE_FABRICA));
        assert!(!e.hay_que_deshacer());
    }

    #[test]
    fn el_papel_es_siempre_opaco() {
        let mut e = Escena::nueva();
        e.poner_fondo(ColorRgba {
            a: 0.3,
            ..ColorRgba::opaco(0.0, 0.0, 0.0)
        });
        assert_eq!(e.fondo.a, 1.0);
    }

    #[test]
    fn un_fichero_propio_sin_papel_se_abre_en_blanco_y_uno_con_papel_lo_conserva() {
        // Los `.json` de antes de este campo no lo traen.
        let viejo: Escena = serde_json::from_str(r#"{"version":1,"elementos":[]}"#).unwrap();
        assert_eq!(viejo.fondo, FONDO_DE_FABRICA);
        let mut e = Escena::nueva();
        e.poner_fondo(ColorRgba::opaco(0.1, 0.2, 0.3));
        let vuelta: Escena = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(vuelta.fondo, e.fondo);
    }

    #[test]
    fn varios_pasos_juntados_se_deshacen_de_una_vez() {
        let mut e = Escena::nueva();
        let a = e.anadir(base());
        let desde = e.pasos_cerrados();
        // Tres gestos: anadir, mover y borrar.
        let b = e.anadir(base());
        e.abrir_paso();
        e.apuntar_edicion(a);
        e.buscar_mut(a).unwrap().mover(10.0, 0.0);
        e.cerrar_paso();
        e.abrir_paso();
        e.borrar_apuntando(b);
        e.cerrar_paso();
        e.juntar_pasos(desde);
        assert!(e.deshacer());
        assert_eq!(e.buscar(a).unwrap().x, 0.0, "el movimiento se deshizo");
        assert!(e.buscar(b).is_none_or(|x| x.borrado), "lo anadido se quito");
        // Lo de antes de `desde` sigue en su propio paso.
        assert!(e.hay_que_deshacer());
        assert!(e.rehacer());
        assert_eq!(e.buscar(a).unwrap().x, 10.0);
    }

    #[test]
    fn juntar_un_solo_paso_nuevo_no_toca_el_de_antes() {
        // Caso negativo: un solo paso nuevo no se junta con el anterior.
        let mut e = Escena::nueva();
        e.anadir(base());
        let desde = e.pasos_cerrados();
        e.anadir(base());
        e.juntar_pasos(desde);
        assert!(e.deshacer());
        assert_eq!(e.visibles().count(), 1);
    }

    #[test]
    fn volver_a_una_copia_es_un_paso_nuevo_que_se_deshace_con_el_turno() {
        let mut e = Escena::nueva();
        let a = e.anadir(Elemento {
            x: 10.0,
            ancho: 5.0,
            alto: 5.0,
            ..Default::default()
        });
        let b = e.anadir(Elemento {
            x: 50.0,
            ancho: 5.0,
            alto: 5.0,
            ..Default::default()
        });
        let desde = e.pasos_cerrados();
        // Lo que dibujo el turno antes de la revision.
        let d = e.anadir(Elemento {
            x: 120.0,
            ancho: 5.0,
            alto: 5.0,
            ..Default::default()
        });
        let copia = e.elementos.clone();
        // La «correccion»: mueve a, borra b y anade c, en pasos sueltos.
        e.abrir_paso();
        e.apuntar_edicion(a);
        e.buscar_mut(a).unwrap().x = 99.0;
        e.cerrar_paso();
        e.borrar_apuntando(b);
        let c = e.anadir(Elemento {
            x: 200.0,
            ancho: 5.0,
            alto: 5.0,
            ..Default::default()
        });
        assert!(e.volver_a(&copia));
        assert_eq!(e.buscar(a).unwrap().x, 10.0);
        assert!(!e.buscar(b).unwrap().borrado);
        assert!(e.buscar(c).unwrap().borrado);
        assert!(!e.buscar(d).unwrap().borrado);
        assert!(e.siguiente_id > c, "los ids no se reusan");
        // Todo el turno (con la vuelta) en un Ctrl+Z: queda como antes del turno.
        e.juntar_pasos(desde);
        assert!(e.deshacer());
        assert_eq!(e.visibles().count(), 2);
        assert!(e.buscar(d).unwrap().borrado);
        assert_eq!(e.buscar(a).unwrap().x, 10.0);
        // Caso negativo: volver a lo que ya hay no cambia nada ni gasta un paso.
        let n = e.pasos_cerrados();
        let igual = e.elementos.clone();
        assert!(!e.volver_a(&igual));
        assert_eq!(e.pasos_cerrados(), n);
    }
}
