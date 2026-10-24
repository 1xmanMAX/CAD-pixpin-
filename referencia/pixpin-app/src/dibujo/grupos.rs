//! **Los grupos de la barra**: que cara ensena cada uno y que pasa al
//! pulsarlo. La geometria (donde va cada grupo y su desplegable) es de
//! `pixpin_ui::caja_herramientas`; aqui esta la memoria de la ultima usada
//! de cada grupo y la regla del clic, que copian las del movil
//! (`DrawToolbar.kt` y `caraDelGrupo` de `Barra.kt`).
//!
//! La memoria es una para toda la aplicacion y no una por ventana: es una
//! costumbre de quien dibuja («el rombo es mi forma»), no del sitio donde
//! dibuja, y asi el lector abre con la misma cara que dejo el lienzo. Se
//! pierde al cerrar PixPin, como en el movil.

use pixpin_motor2d::gesto::Herramienta;
use pixpin_ui::{BotonCaja, CajaHerramientas, GrupoBarra, cara_del_grupo};
use std::sync::Mutex;

/// La ultima que se vio puesta de cada grupo, por `GrupoBarra::indice`.
static ULTIMAS: Mutex<[Option<BotonCaja>; GrupoBarra::TODOS.len()]> =
    Mutex::new([None; GrupoBarra::TODOS.len()]);

/// La ultima usada de un grupo, si hay.
pub fn recordada(g: GrupoBarra) -> Option<BotonCaja> {
    ULTIMAS.lock().ok().and_then(|u| u[g.indice()])
}

fn recordar(g: GrupoBarra, b: BotonCaja) {
    if let Ok(mut u) = ULTIMAS.lock() {
        u[g.indice()] = Some(b);
    }
}

/// Se uso `b` desde el desplegable. Las herramientas se recuerdan solas al
/// quedar puestas (`cara`); esto es para las acciones (imprimir, las
/// figuras), que no quedan puestas y si deben quedar de cara.
pub fn usado(b: BotonCaja) {
    if let Some(g) = pixpin_ui::grupo_de_boton(b) {
        recordar(g, b);
    }
}

/// **La cara del grupo `g` en esta caja** con la herramienta `activa`
/// puesta. Si la puesta es del grupo, pasa a ser la recordada: da igual si
/// se eligio en el desplegable, con su letra o con el cuentagotas.
pub fn cara(caja: &CajaHerramientas, g: GrupoBarra, activa: Herramienta) -> Option<BotonCaja> {
    let miembros = caja.miembros(g);
    let c = cara_del_grupo(&miembros, activa, recordada(g))?;
    if c == BotonCaja::Elegir(activa) {
        recordar(g, c);
    }
    Some(c)
}

/// Lo que hace un clic en el boton de un grupo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClicGrupo {
    /// La herramienta que hay que coger, si hay que cambiar.
    pub elegir: Option<Herramienta>,
    /// Que grupo queda desplegado despues.
    pub desplegado: Option<GrupoBarra>,
}

/// **El clic en el boton de un grupo** (`DrawToolbar` del movil, con las
/// hermanas pintadas fuera de la barra): coge la cara si no estaba puesta
/// Y ensena las hermanas en el mismo clic, para seguir dibujando sin mas o
/// cambiar de hermana sin otro clic de vuelta. Pulsar el grupo ya abierto lo
/// cierra, y uno de una sola herramienta no se abre.
///
/// La cara de un grupo de acciones (compartir, imprimir) no se dispara al
/// pulsar el grupo: se abre el desplegable y se elige ahi. Dispararla sin
/// ver que se va a hacer seria imprimir por sorpresa.
pub fn pulsar(
    caja: &CajaHerramientas,
    g: GrupoBarra,
    activa: Herramienta,
    desplegado: Option<GrupoBarra>,
) -> ClicGrupo {
    let elegir = match cara(caja, g, activa) {
        Some(BotonCaja::Elegir(h)) if h != activa => Some(h),
        _ => None,
    };
    let desplegado = if desplegado == Some(g) || caja.miembros(g).len() <= 1 {
        None
    } else {
        Some(g)
    };
    ClicGrupo { elegir, desplegado }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Rect;

    fn caja() -> CajaHerramientas {
        CajaHerramientas::barra_superior(
            Rect {
                x: 0,
                y: 0,
                ancho: 1366,
                alto: 700,
            },
            125,
            &pixpin_ui::BOTONES_EDITOR,
        )
    }

    // Una prueba sola para todo lo que toca la memoria global: varias en
    // paralelo se pisarian la ultima de cada grupo.
    #[test]
    fn pulsar_un_grupo_coge_su_cara_y_despliega_y_la_ultima_usada_se_queda_de_cara() {
        let c = caja();
        // De fabrica, la cara de las formas es el rectangulo: pulsar el
        // grupo lo coge y lo abre.
        let r = pulsar(&c, GrupoBarra::Formas, Herramienta::Lapiz, None);
        assert_eq!(r.elegir, Some(Herramienta::Rectangulo));
        assert_eq!(r.desplegado, Some(GrupoBarra::Formas));
        // Pulsado otra vez, abierto y con su herramienta ya puesta: se
        // cierra y no cambia nada.
        let r = pulsar(
            &c,
            GrupoBarra::Formas,
            Herramienta::Rectangulo,
            Some(GrupoBarra::Formas),
        );
        assert_eq!(
            r,
            ClicGrupo {
                elegir: None,
                desplegado: None
            }
        );
        // Se usa el rombo (por el desplegable o su letra): se queda de cara
        // aunque luego se coja el lapiz.
        assert_eq!(
            cara(&c, GrupoBarra::Formas, Herramienta::Rombo),
            Some(BotonCaja::Elegir(Herramienta::Rombo))
        );
        assert_eq!(
            cara(&c, GrupoBarra::Formas, Herramienta::Lapiz),
            Some(BotonCaja::Elegir(Herramienta::Rombo))
        );
        assert_eq!(
            pulsar(&c, GrupoBarra::Formas, Herramienta::Lapiz, None).elegir,
            Some(Herramienta::Rombo)
        );
        // Caso negativo: el grupo de sacar no imprime ni comparte al pulsarlo,
        // solo se abre; y abrir otro grupo cierra el que hubiera.
        let r = pulsar(
            &c,
            GrupoBarra::Sacar,
            Herramienta::Lapiz,
            Some(GrupoBarra::Formas),
        );
        assert_eq!(
            r,
            ClicGrupo {
                elegir: None,
                desplegado: Some(GrupoBarra::Sacar)
            }
        );
    }
}
