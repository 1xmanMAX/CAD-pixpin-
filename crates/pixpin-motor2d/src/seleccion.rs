//! Que elementos estan elegidos.
//!
//! # Por que un `Vec` y no un `HashSet` (D26)
//!
//! Una seleccion de trabajo son de uno a veinte elementos. Buscar
//! linealmente en veinte es mas rapido que calcular un hash, y sobre todo:
//! el `Vec` se reutiliza con `clear()`, que conserva la memoria ya pedida.
//! Arrastrar una marquesina reconstruye la seleccion en cada aviso del
//! raton —camino caliente— y ahi la regla es cero asignaciones.
//!
//! El orden de los ids no significa nada, pero es estable a proposito: un
//! `HashSet` lo cambiaria de un recorrido a otro, y con el cambiaria el
//! orden en que se pintan los marcos.
//!
//! **Y ademas un `HashSet` al lado (2026-09-22).** «De uno a veinte» dejo de
//! ser verdad con la marquesina y `Ctrl+A`: con mil elegidos, `contiene`
//! recorria mil ids, y el arrastre y el pintado lo preguntan por cada
//! elemento en cada aviso del raton (2,2 ms por aviso medidos moviendo mil
//! de dos mil, `tests/puertas.rs`). Excalidraw pregunta a un mapa. El `Vec`
//! se queda para el orden estable; el conjunto contesta `contiene`. Los dos
//! se vacian con `clear()`, que conserva la memoria: la regla de cero
//! asignaciones en el camino caliente sigue en pie.

use std::collections::HashSet;

use crate::elemento::Elemento;
use crate::escena::Escena;
use crate::vector::Punto2;

#[derive(Debug, Clone, Default)]
pub struct Seleccion {
    ids: Vec<u64>,
    /// Los mismos ids que `ids`, para contestar `contiene` sin recorrerlos.
    conjunto: HashSet<u64>,
}

/// Dos selecciones son iguales si eligen lo mismo en el mismo orden; el
/// conjunto es un indice y no cuenta.
impl PartialEq for Seleccion {
    fn eq(&self, otra: &Self) -> bool {
        self.ids == otra.ids
    }
}

impl Seleccion {
    pub fn nueva() -> Self {
        Self::default()
    }

    pub fn ids(&self) -> &[u64] {
        &self.ids
    }

    pub fn esta_vacia(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn cuantos(&self) -> usize {
        self.ids.len()
    }

    pub fn contiene(&self, id: u64) -> bool {
        self.conjunto.contains(&id)
    }

    /// Lo que hace un clic normal: sustituye la seleccion entera.
    pub fn poner(&mut self, id: u64) {
        self.limpiar();
        self.ids.push(id);
        self.conjunto.insert(id);
    }

    /// Lo que hace `Shift` + clic: lo anade si no estaba, lo quita si estaba.
    pub fn alternar(&mut self, id: u64) {
        if self.conjunto.remove(&id) {
            self.ids.retain(|x| *x != id);
        } else {
            self.ids.push(id);
            self.conjunto.insert(id);
        }
    }

    /// Lo que hace la marquesina o `Ctrl+A`: sustituye por todos estos.
    pub fn poner_todos(&mut self, ids: impl IntoIterator<Item = u64>) {
        self.limpiar();
        for id in ids {
            // `insert` dice si era nuevo: sin esto, poner mil costaba
            // medio millon de comparaciones.
            if self.conjunto.insert(id) {
                self.ids.push(id);
            }
        }
    }

    /// Vacia la seleccion **conservando la memoria pedida** (D26).
    pub fn limpiar(&mut self) {
        self.ids.clear();
        self.conjunto.clear();
    }

    /// Para la prueba de que `limpiar` no reasigna.
    pub fn capacidad(&self) -> usize {
        self.ids.capacity()
    }

    /// Los elementos elegidos que siguen vivos.
    ///
    /// Filtra los borrados porque deshacer es borrado logico: el elemento
    /// sigue en la lista, y si contara, el marco abarcaria cosas que el
    /// usuario no ve en la pantalla.
    fn vivos<'a>(&'a self, escena: &'a Escena) -> impl Iterator<Item = &'a Elemento> + 'a {
        // Una pasada por la escena preguntando al conjunto, y no una
        // busqueda en la escena por cada elegido: con mil elegidos eso eran
        // mil recorridos, y la caja se pide en cada fotograma para el marco.
        escena
            .elementos
            .iter()
            .filter(move |e| !e.borrado && self.conjunto.contains(&e.id))
    }

    /// La caja que abarca todo lo elegido, paralela a los ejes.
    ///
    /// Paralela a los ejes aunque los elementos esten girados: es lo que
    /// hacen Excalidraw y el Android. Cada elemento conserva su propio
    /// angulo; la caja es solo el marco desde el que se tira.
    pub fn caja(&self, escena: &Escena) -> Option<(f32, f32, f32, f32)> {
        let mut caja: Option<(f32, f32, f32, f32)> = None;
        for e in self.vivos(escena) {
            let (x0, y0, x1, y1) = e.caja();
            caja = Some(match caja {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            });
        }
        caja
    }

    /// El centro de la caja: alrededor de el se gira y se escala.
    pub fn centro(&self, escena: &Escena) -> Option<Punto2> {
        let (x0, y0, x1, y1) = self.caja(escena)?;
        Some(Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0))
    }
}

// -------------------------------------------------------------------------
// Los atajos de teclado
// -------------------------------------------------------------------------

/// Una tecla, ya despojada de en que teclado se pulso.
///
/// Solo estan las que esta tabla usa. Es a proposito: un enumerado con las
/// ciento cinco teclas de un teclado obligaria a un `match` gigante por cada
/// atajo nuevo, y quien anade un atajo lo que quiere es anadir una linea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecla {
    /// Una letra, **siempre en minuscula**: quien la traduce del sistema ya
    /// ha decidido si habia Mayus o no, y esa decision viaja en `shift`.
    Letra(char),
    /// `[`
    CorcheteAbre,
    /// `]`
    CorcheteCierra,
}

/// Lo que un atajo pide que se haga.
///
/// Es **una orden y no una funcion que se llama**: quien tiene la escena, la
/// seleccion y el historial es la ventana, y esta tabla solo tiene teclas.
/// Devolviendo la orden, decidir el atajo se prueba sin escritorio y sin
/// sintetizar una sola pulsacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrdenEditor {
    /// Coger la herramienta de lazo.
    Lazo,
    /// Coger el cuentagotas de estilo.
    CopiarEstilo,
    /// Llevarse el estilo de lo que hay elegido.
    TomarEstilo,
    /// Pegar el estilo tomado sobre lo elegido.
    SoltarEstilo,
    VoltearHorizontal,
    VoltearVertical,
    Agrupar,
    Desagrupar,
    AlFrente,
    AlFondo,
    Subir,
    Bajar,
    /// Encender o apagar el iman entero.
    AlternarIman,
}

/// **La tabla de atajos del escritorio.**
///
/// Sigue a Excalidraw donde Excalidraw tiene atajo, porque quien llega aqui
/// llega de ahi y las dos manos ya se saben esos gestos: `Ctrl+G` agrupar,
/// `Ctrl+[` y `Ctrl+]` el orden de pintado, `Ctrl+Alt+C`/`Ctrl+Alt+V` el
/// estilo. Los dos que Excalidraw no tiene —voltear y el iman— van en
/// `Mayus+H`/`Mayus+V` y `Alt+S`, que son los de Figma.
///
/// **Donde se aparta de Excalidraw, y por que.** El lazo es `S` y no `Q`.
/// `Q` ya es la lupa en el escritorio, y no es una eleccion que se pueda
/// cambiar por detras: la letra va **pintada en el boton** de la caja de
/// herramientas (`ventana_editor::tecla_de`), asi que moverla seria mentirle
/// al usuario en la unica pantalla donde este atajo se anuncia. Entre seguir
/// a Excalidraw y no contradecir lo que se ve, manda lo que se ve.
///
/// **Lo que no esta.** El pautado del fondo no tiene atajo porque el editor
/// no tiene pautado del fondo: no hay nada que encender. Un atajo a nada es
/// justo el fallo que esta tanda viene a cerrar.
///
/// `alt` entra en la decision aunque hoy solo lo mire un atajo: sin el, un
/// `Ctrl+Alt+C` de un teclado con AltGr —donde AltGr **es** Ctrl+Alt— se
/// colaria como el `Ctrl+C` de copiar.
pub fn atajo_de(tecla: Tecla, ctrl: bool, shift: bool, alt: bool) -> Option<OrdenEditor> {
    use OrdenEditor::*;
    match (tecla, ctrl, shift, alt) {
        // Herramientas: una tecla pelada, como todas las de Excalidraw.
        // `S` y no `Q`: ver la cabecera.
        (Tecla::Letra('s'), false, false, false) => Some(Lazo),
        (Tecla::Letra('k'), false, false, false) => Some(CopiarEstilo),

        // El estilo, con la misma pareja que Excalidraw.
        (Tecla::Letra('c'), true, false, true) => Some(TomarEstilo),
        (Tecla::Letra('v'), true, false, true) => Some(SoltarEstilo),

        // Voltear. Sin Ctrl a proposito: `Ctrl+Mayus+V` ya es pegar sin
        // formato en medio mundo y `Ctrl+H` es buscar y reemplazar.
        (Tecla::Letra('h'), false, true, false) => Some(VoltearHorizontal),
        (Tecla::Letra('v'), false, true, false) => Some(VoltearVertical),

        (Tecla::Letra('g'), true, false, false) => Some(Agrupar),
        (Tecla::Letra('g'), true, true, false) => Some(Desagrupar),

        // El orden de pintado. Con Mayus, hasta el final; sin el, un paso.
        (Tecla::CorcheteCierra, true, true, false) => Some(AlFrente),
        (Tecla::CorcheteAbre, true, true, false) => Some(AlFondo),
        (Tecla::CorcheteCierra, true, false, false) => Some(Subir),
        (Tecla::CorcheteAbre, true, false, false) => Some(Bajar),

        (Tecla::Letra('s'), false, false, true) => Some(AlternarIman),

        _ => None,
    }
}

impl OrdenEditor {
    /// Si la orden necesita que haya algo elegido.
    ///
    /// Las que no —coger una herramienta, encender el iman— tienen que
    /// funcionar con el lienzo vacio, que es justo cuando se encienden.
    pub fn necesita_seleccion(self) -> bool {
        !matches!(
            self,
            OrdenEditor::Lazo | OrdenEditor::CopiarEstilo | OrdenEditor::AlternarIman
        )
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
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

    #[test]
    fn poner_sustituye_y_alternar_anade_o_quita() {
        let mut s = Seleccion::nueva();
        s.poner(1);
        assert_eq!(s.ids(), &[1]);

        // Clic normal sustituye.
        s.poner(2);
        assert_eq!(s.ids(), &[2]);

        // Shift+clic anade.
        s.alternar(3);
        assert_eq!(s.ids(), &[2, 3]);

        // Shift+clic sobre lo ya elegido lo quita.
        s.alternar(2);
        assert_eq!(s.ids(), &[3]);
    }

    #[test]
    fn poner_dos_veces_el_mismo_no_lo_duplica() {
        let mut s = Seleccion::nueva();
        s.alternar(7);
        s.poner(7);
        assert_eq!(s.ids(), &[7], "una sola vez");
    }

    #[test]
    fn poner_todos_elimina_duplicados_en_la_entrada() {
        // El metodo acepta cualquier iterador y no puede fiarse de que quien
        // lo llama le de ids unicos. La rama que evita duplicados es
        // justamente la que lo hace cuadratico, y sin esta prueba no se ejecuta.
        let mut s = Seleccion::nueva();
        s.poner_todos([1, 1, 2]);
        assert_eq!(s.ids(), &[1, 2]);
    }

    #[test]
    fn la_caja_de_varios_abarca_a_todos() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(100.0, 50.0, 20.0, 20.0));

        let mut s = Seleccion::nueva();
        s.poner_todos([a, b]);

        let (x0, y0, x1, y1) = s.caja(&escena).unwrap();
        assert_eq!((x0, y0), (0.0, 0.0));
        assert_eq!((x1, y1), (120.0, 70.0));
        assert_eq!(s.centro(&escena).unwrap(), Punto2::nuevo(60.0, 35.0));
    }

    #[test]
    fn un_elemento_borrado_no_cuenta_para_la_caja() {
        // Deshacer es borrado logico: el elemento sigue en la lista. Si la
        // caja lo contara, el marco abarcaria cosas que el usuario no ve.
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(500.0, 500.0, 10.0, 10.0));
        escena.borrar(b);

        let mut s = Seleccion::nueva();
        s.poner_todos([a, b]);

        let (_, _, x1, y1) = s.caja(&escena).unwrap();
        assert_eq!((x1, y1), (10.0, 10.0), "el borrado no estira la caja");
    }

    #[test]
    fn una_seleccion_de_solo_borrados_no_tiene_caja() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        escena.borrar(a);

        let mut s = Seleccion::nueva();
        s.poner(a);
        assert!(s.caja(&escena).is_none());
    }

    #[test]
    fn limpiar_conserva_la_capacidad_para_no_reasignar() {
        // D26: arrastrar una marquesina es camino caliente. El Vec se
        // reutiliza con clear(), que no devuelve la memoria al sistema.
        let mut s = Seleccion::nueva();
        s.poner_todos(1..=50);
        let capacidad = s.capacidad();
        s.limpiar();
        assert!(s.esta_vacia());
        assert_eq!(s.capacidad(), capacidad, "clear() no reasigna");
    }

    // --- Atajos ---

    #[test]
    fn los_atajos_de_excalidraw_son_los_mismos_aqui() {
        // Quien llega aqui llega de ahi y las dos manos ya se saben estos
        // gestos: cambiarlos seria pedirle que los desaprenda.
        //
        // Con una excepcion, y esta prueba la fija para que no se deshaga
        // sola: el lazo es `S`. `Q` ya es la lupa y la letra va **pintada en
        // su boton** de la caja de herramientas, asi que aqui habia dos
        // tablas de atajos diciendo cosas distintas sobre la misma tecla.
        assert_eq!(
            atajo_de(Tecla::Letra('s'), false, false, false),
            Some(OrdenEditor::Lazo)
        );
        assert_eq!(
            atajo_de(Tecla::Letra('q'), false, false, false),
            None,
            "`Q` es la lupa del editor: el lazo no puede reclamarla"
        );
        assert_eq!(
            atajo_de(Tecla::Letra('g'), true, false, false),
            Some(OrdenEditor::Agrupar)
        );
        assert_eq!(
            atajo_de(Tecla::Letra('g'), true, true, false),
            Some(OrdenEditor::Desagrupar)
        );
        assert_eq!(
            atajo_de(Tecla::CorcheteCierra, true, false, false),
            Some(OrdenEditor::Subir)
        );
        assert_eq!(
            atajo_de(Tecla::CorcheteAbre, true, true, false),
            Some(OrdenEditor::AlFondo)
        );
    }

    #[test]
    fn el_estilo_se_toma_y_se_suelta_con_la_pareja_de_siempre() {
        assert_eq!(
            atajo_de(Tecla::Letra('c'), true, false, true),
            Some(OrdenEditor::TomarEstilo)
        );
        assert_eq!(
            atajo_de(Tecla::Letra('v'), true, false, true),
            Some(OrdenEditor::SoltarEstilo)
        );
    }

    #[test]
    fn un_ctrl_c_pelado_no_se_confunde_con_tomar_el_estilo() {
        // La razon de que `alt` entre en la decision: en un teclado con
        // AltGr —donde AltGr ES Ctrl+Alt— mirar solo Ctrl haria que copiar
        // se llevase ademas el estilo, o al reves.
        assert_eq!(atajo_de(Tecla::Letra('c'), true, false, false), None);
        assert_eq!(atajo_de(Tecla::Letra('v'), true, false, false), None);
    }

    #[test]
    fn voltear_no_pisa_a_pegar_sin_formato_ni_a_buscar_y_reemplazar() {
        // `Ctrl+Mayus+V` ya es pegar sin formato en medio mundo y `Ctrl+H`
        // es buscar y reemplazar: voltear va sin Ctrl.
        assert_eq!(
            atajo_de(Tecla::Letra('h'), false, true, false),
            Some(OrdenEditor::VoltearHorizontal)
        );
        assert_eq!(
            atajo_de(Tecla::Letra('v'), false, true, false),
            Some(OrdenEditor::VoltearVertical)
        );
        assert_eq!(atajo_de(Tecla::Letra('v'), true, true, false), None);
        assert_eq!(atajo_de(Tecla::Letra('h'), true, false, false), None);
    }

    #[test]
    fn una_tecla_suelta_que_no_esta_en_la_tabla_no_hace_nada() {
        // Caso negativo: escribir una `z` con el lienzo enfocado no puede
        // disparar media interfaz.
        assert_eq!(atajo_de(Tecla::Letra('z'), false, false, false), None);
        assert_eq!(atajo_de(Tecla::CorcheteAbre, false, false, false), None);
    }

    #[test]
    fn lo_que_no_toca_el_dibujo_funciona_con_el_lienzo_vacio() {
        // Encender el iman es justo lo que se hace ANTES del primer
        // trazo: pedirle una seleccion seria pedirle lo que aun no hay.
        assert!(!OrdenEditor::AlternarIman.necesita_seleccion());
        assert!(!OrdenEditor::Lazo.necesita_seleccion());
        assert!(OrdenEditor::VoltearHorizontal.necesita_seleccion());
        assert!(
            OrdenEditor::SoltarEstilo.necesita_seleccion(),
            "pegar el estilo sin nada elegido no tiene destino"
        );
    }
}
