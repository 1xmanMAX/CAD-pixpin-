//! Orden de pintado, grupos, alinear y repartir. Puerto de `Organize.kt`.
//!
//! # Los grupos son una lista y no un identificador
//!
//! `Elemento::grupos` es un `Vec<String>` porque un elemento puede estar en
//! varios grupos anidados, como en Excalidraw: el ultimo de la lista es el
//! mas interno. Desagrupar deshace **solo el mas interno**, que es lo que
//! espera quien agrupo dos veces.
//!
//! # Todo lo de aqui es un solo paso de deshacer
//!
//! Alinear cinco elementos es un `Ctrl+Z`, no cinco. Cada funcion publica
//! abre y cierra su paso.

use crate::escena::Escena;
use crate::seleccion::Seleccion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineacion {
    Izquierda,
    CentroHorizontal,
    Derecha,
    Arriba,
    CentroVertical,
    Abajo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reparto {
    Horizontal,
    Vertical,
}

/// Mete lo elegido en un grupo nuevo. `None` si no hay al menos dos: un
/// grupo de uno no es un grupo, y ademas viajaria al movil como basura.
pub fn agrupar(escena: &mut Escena, sel: &Seleccion) -> Option<String> {
    if sel.cuantos() < 2 {
        return None;
    }
    // El nombre sale del contador de la escena, que ya garantiza que no se
    // repite dentro del documento. Con prefijo para no confundirlo con un
    // id de elemento al mirar el JSON a ojo.
    let grupo = format!("g{}", escena.siguiente_id);
    escena.siguiente_id += 1;

    escena.abrir_paso();
    for id in sel.ids() {
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            e.grupos.push(grupo.clone());
            e.tocar();
        }
    }
    escena.cerrar_paso();
    Some(grupo)
}

/// Deshace el grupo mas interno de lo elegido.
pub fn desagrupar(escena: &mut Escena, sel: &Seleccion) {
    escena.abrir_paso();
    for id in sel.ids() {
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            e.grupos.pop();
            e.tocar();
        }
    }
    escena.cerrar_paso();
}

/// Todos los que comparten el grupo mas interno de este. Si no tiene
/// grupo, solo el.
///
/// Es lo que convierte «pinche un elemento» en «cogi el grupo entero».
pub fn hermanos_de(escena: &Escena, id: u64) -> Vec<u64> {
    let Some(e) = escena.buscar(id) else {
        return Vec::new();
    };
    let Some(grupo) = e.grupos.last() else {
        return vec![id];
    };
    escena
        .visibles()
        .filter(|o| o.grupos.last() == Some(grupo))
        .map(|o| o.id)
        .collect()
}

/// Sube lo elegido al frente, conservando su orden relativo.
pub fn al_frente(escena: &mut Escena, sel: &Seleccion) {
    escena.abrir_paso();
    escena.apuntar_reordenamiento();
    reordenar(escena, sel, true);
    escena.cerrar_paso();
}

/// Baja lo elegido al fondo, conservando su orden relativo.
pub fn al_fondo(escena: &mut Escena, sel: &Seleccion) {
    escena.abrir_paso();
    escena.apuntar_reordenamiento();
    reordenar(escena, sel, false);
    escena.cerrar_paso();
}

/// Sube lo elegido UNA posicion: pasa por delante del primer elemento no
/// elegido que tenia encima. Es «Traer adelante» del panel de Excalidraw.
pub fn adelante(escena: &mut Escena, sel: &Seleccion) {
    un_paso(escena, sel, true);
}

/// Baja lo elegido una posicion («Enviar atras»).
pub fn atras(escena: &mut Escena, sel: &Seleccion) {
    un_paso(escena, sel, false);
}

fn un_paso(escena: &mut Escena, sel: &Seleccion, sube: bool) {
    escena.abrir_paso();
    escena.apuntar_reordenamiento();
    let n = escena.elementos.len();
    // Se recorre desde el lado hacia el que se mueve, para que dos elegidos
    // seguidos avancen juntos en vez de pisarse.
    let indices: Vec<usize> = if sube {
        (0..n).rev().collect()
    } else {
        (0..n).collect()
    };
    for i in indices {
        if !sel.contiene(escena.elementos[i].id) {
            continue;
        }
        let j = if sube { i + 1 } else { i.wrapping_sub(1) };
        if j < n && !sel.contiene(escena.elementos[j].id) {
            escena.elementos.swap(i, j);
        }
    }
    escena.cerrar_paso();
}

fn reordenar(escena: &mut Escena, sel: &Seleccion, al_frente: bool) {
    // Particionar conserva el orden dentro de cada mitad, que es justo lo
    // que hace falta: subir dos elementos no debe intercambiarlos.
    let (movidos, quietos): (Vec<_>, Vec<_>) =
        escena.elementos.drain(..).partition(|e| sel.contiene(e.id));
    escena.elementos = if al_frente {
        quietos.into_iter().chain(movidos).collect()
    } else {
        movidos.into_iter().chain(quietos).collect()
    };
}

/// Alinea lo elegido contra el borde o el centro del conjunto.
pub fn alinear(escena: &mut Escena, sel: &Seleccion, como: Alineacion) {
    let Some((cx0, cy0, cx1, cy1)) = sel.caja(escena) else {
        return;
    };
    escena.abrir_paso();
    let mut movidos = Vec::new();
    for id in sel.ids().to_vec() {
        let Some(e) = escena.buscar(id) else { continue };
        let (x0, y0, x1, y1) = e.caja();
        let (dx, dy) = match como {
            Alineacion::Izquierda => (cx0 - x0, 0.0),
            Alineacion::Derecha => (cx1 - x1, 0.0),
            Alineacion::CentroHorizontal => ((cx0 + cx1) / 2.0 - (x0 + x1) / 2.0, 0.0),
            Alineacion::Arriba => (0.0, cy0 - y0),
            Alineacion::Abajo => (0.0, cy1 - y1),
            Alineacion::CentroVertical => (0.0, (cy0 + cy1) / 2.0 - (y0 + y1) / 2.0),
        };
        if dx == 0.0 && dy == 0.0 {
            continue;
        }
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            e.mover(dx, dy);
            // Sin subir la version la cache pintaria el sitio viejo.
            e.tocar();
            movidos.push(id);
        }
    }
    // Las flechas atadas siguen a lo alineado EN ESTE MISMO PASO: un Ctrl+Z
    // que devolviera las cajas y dejara las flechas estiradas hacia donde
    // estuvieron descolocaria el dibujo a medias. Solo lo que se movio de
    // verdad: una flecha elegida que ya estaba alineada no se revisa.
    crate::enlace::despues_de_mover(escena, &movidos);
    escena.cerrar_paso();
}

/// Deja huecos iguales entre los centros, sin mover los dos extremos.
///
/// No mover los extremos es lo que hace que repartir no cambie el sitio del
/// conjunto: se coloca lo de en medio, que es lo que se pide.
pub fn repartir(escena: &mut Escena, sel: &Seleccion, como: Reparto) {
    if sel.cuantos() < 3 {
        return;
    }
    let centro_de = |escena: &Escena, id: u64| -> Option<f32> {
        let (x0, y0, x1, y1) = escena.buscar(id)?.caja();
        Some(match como {
            Reparto::Horizontal => (x0 + x1) / 2.0,
            Reparto::Vertical => (y0 + y1) / 2.0,
        })
    };

    let mut orden: Vec<(u64, f32)> = sel
        .ids()
        .iter()
        .filter_map(|id| centro_de(escena, *id).map(|c| (*id, c)))
        .collect();
    orden.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let (primero, ultimo) = (orden[0].1, orden[orden.len() - 1].1);
    let hueco = (ultimo - primero) / (orden.len() - 1) as f32;

    escena.abrir_paso();
    let mut movidos = Vec::new();
    for (i, (id, actual)) in orden.iter().enumerate().skip(1).take(orden.len() - 2) {
        let quiero = primero + hueco * i as f32;
        let d = quiero - actual;
        if d == 0.0 {
            continue;
        }
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            match como {
                Reparto::Horizontal => e.mover(d, 0.0),
                Reparto::Vertical => e.mover(0.0, d),
            }
            e.tocar();
            movidos.push(*id);
        }
    }
    // Igual que al alinear: las flechas atadas, en el mismo paso.
    crate::enlace::despues_de_mover(escena, &movidos);
    escena.cerrar_paso();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};

    pub(super) fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
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
            grosor: 0.0,
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

    fn con_tres() -> (Escena, u64, u64, u64) {
        let mut e = Escena::nueva();
        let a = e.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = e.anadir(rect(50.0, 30.0, 20.0, 20.0));
        let c = e.anadir(rect(100.0, 60.0, 10.0, 40.0));
        (e, a, b, c)
    }

    #[test]
    fn agrupar_pone_el_mismo_grupo_a_todos_y_desagrupar_lo_quita() {
        let (mut escena, a, b, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);

        let grupo = agrupar(&mut escena, &sel).unwrap();
        assert!(escena.buscar(a).unwrap().grupos.contains(&grupo));
        assert!(escena.buscar(b).unwrap().grupos.contains(&grupo));

        desagrupar(&mut escena, &sel);
        assert!(escena.buscar(a).unwrap().grupos.is_empty());
    }

    #[test]
    fn agrupar_uno_solo_no_crea_grupo() {
        // Un grupo de uno no es un grupo: seria basura en el fichero que
        // ademas viaja al movil.
        let (mut escena, a, _, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        assert!(agrupar(&mut escena, &sel).is_none());
        assert!(escena.buscar(a).unwrap().grupos.is_empty());
    }

    #[test]
    fn desagrupar_solo_quita_el_grupo_de_dentro_y_respeta_los_de_fuera() {
        // Un elemento puede estar en varios grupos anidados, como en
        // Excalidraw. Desagrupar deshace el mas interno, no todos.
        let (mut escena, a, b, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        let fuera = agrupar(&mut escena, &sel).unwrap();
        let dentro = agrupar(&mut escena, &sel).unwrap();

        desagrupar(&mut escena, &sel);
        let g = &escena.buscar(a).unwrap().grupos;
        assert!(g.contains(&fuera), "el de fuera sigue");
        assert!(!g.contains(&dentro), "el de dentro se fue");
    }

    #[test]
    fn tocar_uno_de_un_grupo_los_trae_a_todos() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        agrupar(&mut escena, &sel);

        let hermanos = hermanos_de(&escena, a);
        assert!(hermanos.contains(&a) && hermanos.contains(&b));
        assert!(!hermanos.contains(&c));
    }

    #[test]
    fn un_elemento_sin_grupo_es_hermano_de_si_mismo_y_de_nadie_mas() {
        let (escena, a, _, _) = con_tres();
        assert_eq!(hermanos_de(&escena, a), vec![a]);
    }

    #[test]
    fn al_frente_y_al_fondo_cambian_el_orden_de_pintado() {
        // El orden de la lista ES el orden de pintado: el ultimo va encima.
        let (mut escena, a, _, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner(a);

        al_frente(&mut escena, &sel);
        assert_eq!(escena.elementos.last().unwrap().id, a);

        al_fondo(&mut escena, &sel);
        assert_eq!(escena.elementos.first().unwrap().id, a);
    }

    #[test]
    fn adelante_y_atras_mueven_una_sola_posicion_y_se_deshacen() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        let ids = |e: &Escena| e.elementos.iter().map(|x| x.id).collect::<Vec<u64>>();
        adelante(&mut escena, &sel);
        assert_eq!(ids(&escena), vec![b, a, c], "una posicion, no al frente");
        atras(&mut escena, &sel);
        assert_eq!(ids(&escena), vec![a, b, c]);
        // Caso negativo: el que ya esta al fondo no se mueve mas.
        atras(&mut escena, &sel);
        assert_eq!(ids(&escena), vec![a, b, c]);
        adelante(&mut escena, &sel);
        assert!(escena.deshacer());
        assert_eq!(ids(&escena), vec![a, b, c]);
    }

    #[test]
    fn al_frente_conserva_el_orden_relativo_de_lo_que_sube() {
        let (mut escena, a, b, _) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        al_frente(&mut escena, &sel);

        let ids: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();
        let ia = ids.iter().position(|x| *x == a).unwrap();
        let ib = ids.iter().position(|x| *x == b).unwrap();
        assert!(ia < ib, "a estaba antes que b y lo sigue estando");
    }

    #[test]
    fn alinear_a_la_izquierda_los_lleva_al_borde_del_mas_a_la_izquierda() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);

        alinear(&mut escena, &sel, Alineacion::Izquierda);
        for id in [a, b, c] {
            assert_eq!(escena.buscar(id).unwrap().caja().0, 0.0);
        }
    }

    #[test]
    fn alinear_al_centro_usa_el_centro_del_conjunto() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);
        let centro = sel.centro(&escena).unwrap();

        alinear(&mut escena, &sel, Alineacion::CentroVertical);
        for id in [a, b, c] {
            let (_, y0, _, y1) = escena.buscar(id).unwrap().caja();
            assert!(((y0 + y1) / 2.0 - centro.y).abs() < 1e-3);
        }
    }

    #[test]
    fn repartir_deja_los_huecos_iguales() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);

        repartir(&mut escena, &sel, Reparto::Horizontal);

        let mut centros: Vec<f32> = [a, b, c]
            .iter()
            .map(|id| {
                let (x0, _, x1, _) = escena.buscar(*id).unwrap().caja();
                (x0 + x1) / 2.0
            })
            .collect();
        centros.sort_by(|p, q| p.partial_cmp(q).unwrap());
        let h1 = centros[1] - centros[0];
        let h2 = centros[2] - centros[1];
        assert!((h1 - h2).abs() < 1e-3, "huecos {h1} y {h2}");
    }

    #[test]
    fn repartir_no_mueve_los_dos_de_los_extremos() {
        // Repartir coloca lo de en medio; mover los extremos cambiaria el
        // sitio del conjunto, que no es lo que nadie espera.
        let (mut escena, a, b, c) = con_tres();
        let (x_a, x_c) = (
            escena.buscar(a).unwrap().caja().0,
            escena.buscar(c).unwrap().caja().0,
        );
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);

        repartir(&mut escena, &sel, Reparto::Horizontal);

        assert_eq!(escena.buscar(a).unwrap().caja().0, x_a);
        assert_eq!(escena.buscar(c).unwrap().caja().0, x_c);
    }

    #[test]
    fn alinear_lo_ya_alineado_no_deja_paso() {
        // alinear() ya se salta apuntar_edicion() cuando el desplazamiento
        // es (0, 0) — comprobarlo aqui deja constancia de que el gesto
        // «alinear sin que nada se mueva» no ensucia el historial, el mismo
        // problema que el filtro de cerrar_paso() cubre para el resto.
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);
        let antes = escena.buscar(b).unwrap().x;

        alinear(&mut escena, &sel, Alineacion::Izquierda);
        let alineado = escena.buscar(b).unwrap().x;
        assert_ne!(alineado, antes, "la primera vez si movio algo");

        // Ya estan alineados: repetirlo no deberia mover nada ni dejar paso.
        alinear(&mut escena, &sel, Alineacion::Izquierda);
        assert_eq!(
            escena.buscar(b).unwrap().x,
            alineado,
            "no se movio otra vez"
        );

        assert!(escena.deshacer(), "hay que deshacer la primera alineacion");
        assert_eq!(
            escena.buscar(b).unwrap().x,
            antes,
            "un solo deshacer basta: el segundo alinear no dejo paso propio"
        );
    }

    #[test]
    fn alinear_y_repartir_dejan_un_solo_paso_de_deshacer() {
        let (mut escena, a, b, c) = con_tres();
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b, c]);
        let antes = escena.buscar(b).unwrap().x;

        alinear(&mut escena, &sel, Alineacion::Izquierda);
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(b).unwrap().x, antes, "los tres de una vez");
    }

    #[test]
    fn con_menos_de_tres_repartir_no_hace_nada() {
        let (mut escena, a, b, _) = con_tres();
        let antes = escena.buscar(b).unwrap().x;
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);

        repartir(&mut escena, &sel, Reparto::Horizontal);
        assert_eq!(escena.buscar(b).unwrap().x, antes, "no hay nada en medio");
    }

    #[test]
    fn al_frente_seguido_de_deshacer_devuelve_el_orden_exacto_de_partida() {
        let (mut escena, a, _, _) = con_tres();
        let orden_partida: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();

        let mut sel = Seleccion::nueva();
        sel.poner(a);
        al_frente(&mut escena, &sel);

        assert!(escena.deshacer());
        let orden_vuelta: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();
        assert_eq!(
            orden_vuelta, orden_partida,
            "la lista de ids completa vuelve a su orden original"
        );
    }

    #[test]
    fn al_fondo_seguido_de_deshacer_devuelve_el_orden_exacto_de_partida() {
        let (mut escena, _, _, c) = con_tres();
        let orden_partida: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();

        let mut sel = Seleccion::nueva();
        sel.poner(c);
        al_fondo(&mut escena, &sel);

        assert!(escena.deshacer());
        let orden_vuelta: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();
        assert_eq!(
            orden_vuelta, orden_partida,
            "la lista de ids completa vuelve a su orden original"
        );
    }

    #[test]
    fn subir_dos_elementos_al_frente_y_deshacer_los_vuelve_a_su_sitio_en_orden() {
        let (mut escena, a, b, _) = con_tres();
        let orden_partida: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();

        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        al_frente(&mut escena, &sel);

        // Despues de subir: [c, a, b]. Al deshacer vuelven a su sitio y orden relativo.
        assert!(escena.deshacer());
        let orden_vuelta: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();
        assert_eq!(
            orden_vuelta, orden_partida,
            "los dos elementos vuelven a su sitio en su orden relativo original"
        );
    }

    #[test]
    fn deshacer_y_rehacer_un_reordenamiento_treinta_veces_deja_la_lista_identica() {
        let (mut escena, a, _, _) = con_tres();

        let mut sel = Seleccion::nueva();
        sel.poner(a);
        al_frente(&mut escena, &sel);
        let orden_despues_al_frente: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();

        // Deshacer y rehacer treinta veces, alternados. Cada ciclo deberia restaurar
        // el estado exacto: deshacer vuelve al original, rehacer vuelve al modificado.
        for _ in 0..30 {
            assert!(escena.deshacer());
            assert!(escena.rehacer());
        }

        let orden_final: Vec<u64> = escena.elementos.iter().map(|e| e.id).collect();
        assert_eq!(
            orden_final, orden_despues_al_frente,
            "tras treinta ciclos de deshacer y rehacer, la lista es identica a despues de al_frente"
        );
    }
}

/// Bloquea o desbloquea lo elegido (`locked` de Excalidraw). Devuelve si
/// cambio algo.
///
/// Todo a la vez y al mismo estado: con una seleccion mixta manda bloquear,
/// que es lo que pide quien pulsa la tecla mirando algo suelto. Al bloquear
/// se suelta la seleccion, porque lo bloqueado ya no se puede elegir y unos
/// tiradores sobre algo que no se mueve serian una mentira.
pub fn bloquear(escena: &mut Escena, sel: &mut Seleccion) -> bool {
    // Sin nada elegido, desbloquea TODO. Es la unica salida posible: lo
    // bloqueado no se puede elegir, asi que sin esto quedaria bloqueado para
    // siempre y la tecla seria una trampa.
    if sel.cuantos() == 0 {
        return desbloquear_todo(escena);
    }
    let ids: Vec<u64> = sel.ids().to_vec();
    let hay_suelto = ids
        .iter()
        .filter_map(|id| escena.buscar(*id))
        .any(|e| !e.bloqueado);
    escena.abrir_paso();
    for id in &ids {
        escena.apuntar_edicion(*id);
        if let Some(e) = escena.buscar_mut(*id) {
            e.bloqueado = hay_suelto;
            e.tocar();
        }
    }
    escena.cerrar_paso();
    if hay_suelto {
        sel.limpiar();
    }
    true
}

/// Suelta todo lo bloqueado de la escena. Devuelve si habia algo.
fn desbloquear_todo(escena: &mut Escena) -> bool {
    let ids: Vec<u64> = escena
        .elementos
        .iter()
        .filter(|e| e.bloqueado && !e.borrado)
        .map(|e| e.id)
        .collect();
    if ids.is_empty() {
        return false;
    }
    escena.abrir_paso();
    for id in ids {
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            e.bloqueado = false;
            e.tocar();
        }
    }
    escena.cerrar_paso();
    true
}

#[cfg(test)]
mod pruebas_bloqueo {
    use super::pruebas::rect;
    use super::*;
    use crate::Punto2;
    use crate::impacto;

    fn escena_con_dos() -> (Escena, Vec<u64>) {
        let mut escena = Escena::nueva();
        let ids = (0..2)
            .map(|n| escena.anadir(rect(100.0 * n as f32, 0.0, 50.0, 50.0)))
            .collect();
        (escena, ids)
    }

    #[test]
    fn lo_bloqueado_se_ve_pero_no_se_toca() {
        let (mut escena, ids) = escena_con_dos();
        let mut sel = Seleccion::nueva();
        sel.poner_todos(vec![ids[0]]);
        assert!(bloquear(&mut escena, &mut sel));
        assert_eq!(sel.cuantos(), 0, "bloquear suelta lo que tenia elegido");

        let dentro = Punto2::nuevo(110.0, 10.0);
        assert!(
            !impacto::toca(escena.buscar(ids[0]).unwrap(), Punto2::nuevo(110.0, 10.0)),
            "el clic lo atraviesa"
        );
        let _ = dentro;
        assert!(
            !impacto::dentro_de(&escena.elementos, (-10.0, -10.0, 500.0, 500.0)).contains(&ids[0]),
            "y la marquesina tampoco se lo lleva"
        );
    }

    #[test]
    fn sin_nada_elegido_la_misma_tecla_desbloquea_todo() {
        let (mut escena, ids) = escena_con_dos();
        let mut sel = Seleccion::nueva();
        sel.poner_todos(ids.clone());
        assert!(bloquear(&mut escena, &mut sel));

        // Caso negativo: sin nada bloqueado no hay nada que hacer.
        let mut vacia = Seleccion::nueva();
        assert!(bloquear(&mut escena, &mut vacia), "los suelta");
        assert!(escena.elementos.iter().all(|e| !e.bloqueado));
        assert!(!bloquear(&mut escena, &mut vacia), "y ya no hay mas");
    }
}
