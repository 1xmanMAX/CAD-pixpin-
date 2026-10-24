//! Mover, escalar y girar. Puerto de `Transform.kt` del Android.
//!
//! # El problema del ancla
//!
//! Redimensionar tirando de una esquina significa que **la de enfrente no
//! se mueve**. Sin giro es aritmetica de colegio. Con el elemento girado,
//! la forma evidente esta mal:
//!
//! > Cambio `ancho` y `alto` -> el centro se desplaza -> el elemento se
//! > dibuja girado **alrededor de un centro nuevo** -> la esquina anclada
//! > se va.
//!
//! El usuario lo ve como que la figura «se escapa» al redimensionarla.
//!
//! # La formula
//!
//! Un elemento se dibuja girado `ang` alrededor de su centro, asi que un
//! punto del mundo `q` es el punto local `q.girar(centro, -ang)`. Escalar
//! es multiplicar el desplazamiento respecto al ancla y devolverlo:
//!
//! ```text
//! nuevo(q) = ancla_mundo + R(ang) . ( (q_local - ancla_local) . (sx, sy) )
//! ```
//!
//! Si `q` es el ancla, el desplazamiento es cero y el resultado es el ancla:
//! **queda quieta por construccion, no por cuidado**.
//!
//! # La regla de los puntos
//!
//! `Elemento::caja()` calcula la caja **de los puntos** para lapiz,
//! resaltador, linea y flecha. Por eso: toda transformacion que toque una
//! figura con puntos, toca los puntos. Si solo se cambiara `ancho`, el
//! trazo no escalaria y el marco se despegaria del dibujo. Es el mismo
//! motivo por el que `Elemento::mover` ya mueve los puntos.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::vector::Punto2;

/// Por debajo de esto no se aplasta: se voltea (D30).
pub const MINIMO: f32 = 1.0;

/// Quince grados, el salto de giro con `Shift`.
pub const SALTO_GIRO: f32 = std::f32::consts::FRAC_PI_2 / 6.0;

/// El centro de la caja de un elemento.
fn centro_de(e: &Elemento) -> Punto2 {
    let (x0, y0, x1, y1) = e.caja();
    Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

/// Que ejes mueve cada tirador, y donde queda su ancla dentro de la caja
/// en tantos por uno: `(0,0)` es la esquina noroeste, `(1,1)` la sureste.
fn ancla_y_ejes(t: Tirador) -> (f32, f32, bool, bool) {
    match t {
        Tirador::NoroesteEsquina => (1.0, 1.0, true, true),
        Tirador::NorteBorde => (0.5, 1.0, false, true),
        Tirador::NoresteEsquina => (0.0, 1.0, true, true),
        Tirador::EsteBorde => (0.0, 0.5, true, false),
        Tirador::SuresteEsquina => (0.0, 0.0, true, true),
        Tirador::SurBorde => (0.5, 0.0, false, true),
        Tirador::SuroesteEsquina => (1.0, 0.0, true, true),
        Tirador::OesteBorde => (1.0, 0.5, true, false),
    }
}

/// Escala `e` arrastrando `tirador` hasta el punto `p` del mundo.
///
/// - `proporcional` (`Shift`): conserva la razon entre ancho y alto.
/// - `desde_centro` (`Alt`): el ancla pasa a ser el centro.
pub fn escalar(
    e: &mut Elemento,
    tirador: Tirador,
    p: Punto2,
    proporcional: bool,
    desde_centro: bool,
) {
    let (x0, y0, x1, y1) = e.caja();
    let (ancho, alto) = (x1 - x0, y1 - y0);
    // Una caja degenerada no se puede escalar: dividir por cero daria NaN,
    // y un NaN en la geometria borra el elemento de la pantalla sin error.
    if ancho.abs() < f32::EPSILON || alto.abs() < f32::EPSILON {
        return;
    }
    let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let ang = e.angulo;

    let (ax, ay, mueve_x, mueve_y) = ancla_y_ejes(tirador);
    let ancla_local = if desde_centro {
        centro
    } else {
        Punto2::nuevo(x0 + ancho * ax, y0 + alto * ay)
    };

    // El cursor, en el marco propio del elemento.
    let p_local = p.girar(centro, -ang);

    // Cuanto se estira cada eje. El tirador que no mueve un eje lo deja a 1.
    //
    // Dos sutilezas. Con `desde_centro`, la referencia es media caja y no la
    // caja entera: el cursor se aleja del centro, no del borde de enfrente.
    // Y el `signo` existe porque el ancla puede estar a la derecha o abajo
    // (tiradores del oeste y del norte): ahi, alejarse del ancla es ir hacia
    // los negativos.
    let mut sx = if mueve_x {
        let ancla_a_borde = if desde_centro { ancho / 2.0 } else { ancho };
        let signo = if ax > 0.5 { -1.0 } else { 1.0 };
        (p_local.x - ancla_local.x) * signo / ancla_a_borde
    } else {
        1.0
    };
    let mut sy = if mueve_y {
        let ancla_a_borde = if desde_centro { alto / 2.0 } else { alto };
        let signo = if ay > 0.5 { -1.0 } else { 1.0 };
        (p_local.y - ancla_local.y) * signo / ancla_a_borde
    } else {
        1.0
    };

    if proporcional && mueve_x && mueve_y {
        // El que mas se ha movido manda, para que la figura siga al cursor
        // por el eje en que el usuario esta tirando de verdad.
        let k = if sx.abs() > sy.abs() {
            sx.abs()
        } else {
            sy.abs()
        };
        sx = k * sx.signum();
        sy = k * sy.signum();
    }

    // No se aplasta: se voltea (D30). El minimo se aplica al tamano final,
    // conservando el signo, que es lo que produce el volteo.
    let tope = |s: f32, largo: f32| -> f32 {
        if (s * largo).abs() < MINIMO {
            MINIMO / largo * if s < 0.0 { -1.0 } else { 1.0 }
        } else {
            s
        }
    };
    let sx = tope(sx, ancho);
    let sy = tope(sy, alto);

    // Las figuras SIN puntos (rectangulo, elipse, foco, texto, imagen) no
    // guardan geometria propia: su caja sale de `x/y/ancho/alto`, que se
    // dibujan girados `ang` alrededor de su centro. Para ellas, `mapear` es
    // la formula del encabezado tal cual: recibe un punto DEL MUNDO.
    //
    // Las figuras CON puntos (lapiz, resaltador, linea, flecha) son
    // distintas: sus `puntos` se guardan en marco LOCAL (sin girar), y es
    // quien dibuja el que aplica `ang` alrededor del centro de su caja para
    // verlos en el mundo (asi lo trata ya `impacto::toca`). Pasarles esos
    // puntos locales a `mapear` -que espera un punto de mundo y por eso
    // empieza deshaciendo el giro- los des-gira una vez de mas: con
    // `ang == 0` no se nota (deshacer un giro nulo no hace nada), pero con
    // `ang != 0` manda el trazo a otro sitio.
    // Este `match` lleva comodin: una figura nueva con puntos no rompe la
    // compilacion al añadirse, asi que hay que acordarse de venir aqui.
    // `Figura::Cota` cayo en el `_` hasta que se noto: la caja se
    // recalculaba de sus puntos (que no se habian movido) y pisaba
    // cualquier cambio de x/y/ancho/alto, asi que escalar una cota no hacia
    // absolutamente nada.
    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => {
            // A(u): el escalado puro en marco local, respecto al ancla
            // -tambien local-. Sin ningun giro de por medio: `u` ya esta en
            // el marco en el que vive el ancla.
            for q in puntos.iter_mut() {
                q.x = ancla_local.x + (q.x - ancla_local.x) * sx;
                q.y = ancla_local.y + (q.y - ancla_local.y) * sy;
            }

            // Ese escalado puro ya deja el ancla quieta en marco local, pero
            // el centro de la caja de los puntos se ha desplazado -de `centro`
            // a `c_A`-, y quien dibuja gira alrededor del centro ACTUAL. Sin
            // corregir eso, lo que se ve queda girado alrededor del centro
            // equivocado (el mismo problema de fondo que resuelve toda esta
            // tarea, aqui otra vez en marco local).
            //
            // La correccion es una traslacion fija `k`, la misma para todos
            // los puntos: sale de exigir que lo que se vea despues -los
            // puntos finales girados `ang` alrededor de su propio centro
            // nuevo- sea lo que la formula del encabezado manda ver. Con
            // `ang == 0` la rotacion es la identidad y `k` sale cero, asi
            // que no cambia nada de lo que ya funcionaba sin giro.
            if !puntos.is_empty() {
                let cx = (puntos.iter().map(|q| q.x).fold(f32::MAX, f32::min)
                    + puntos.iter().map(|q| q.x).fold(f32::MIN, f32::max))
                    / 2.0;
                let cy = (puntos.iter().map(|q| q.y).fold(f32::MAX, f32::min)
                    + puntos.iter().map(|q| q.y).fold(f32::MIN, f32::max))
                    / 2.0;
                let c_a = Punto2::nuevo(cx, cy);
                let d = c_a.restar(centro);
                let origen = Punto2::nuevo(0.0, 0.0);
                let k = d.girar(origen, ang).restar(d);
                for q in puntos.iter_mut() {
                    *q = q.sumar(k);
                }
            }
        }
        _ => {
            let ancla_mundo = ancla_local.girar(centro, ang);
            let origen = Punto2::nuevo(0.0, 0.0);

            // La formula del encabezado, en una sola funcion.
            let mapear = |q: Punto2| -> Punto2 {
                let ql = q.girar(centro, -ang);
                let d = Punto2::nuevo((ql.x - ancla_local.x) * sx, (ql.y - ancla_local.y) * sy);
                ancla_mundo.sumar(d.girar(origen, ang))
            };

            let centro_nuevo = mapear(centro);
            e.ancho = (ancho * sx).abs();
            e.alto = (alto * sy).abs();
            e.x = centro_nuevo.x - e.ancho / 2.0;
            e.y = centro_nuevo.y - e.alto / 2.0;
        }
    }

    // Las figuras con puntos sacan `x/y/ancho/alto` de su propia caja, ya
    // con los puntos movidos: es exacto por construccion. Multiplicar
    // `ancho` por `sx` no sirve porque el grosor -que `caja()` suma como
    // margen- no escala con el trazo: la extension se estira pero el
    // margen no, y `ancho * sx` arrastra ese margen sin estirar tambien.
    if e.puntos().is_some() {
        let (nx0, ny0, nx1, ny1) = e.caja();
        e.x = nx0;
        e.y = ny0;
        e.ancho = nx1 - nx0;
        e.alto = ny1 - ny0;
    }

    // **El instrumento se estira de dos maneras** (`resizeSingleElement`
    // del movil): por un lado crece la caja y la unidad se queda, asi que
    // salen mas numeros; por una esquina crece tambien la unidad, con la
    // menor de las dos escalas, y se ven los mismos numeros mas grandes.
    // Las cifras siguen al alto, como el `fontSize` de alli.
    if let Figura::Instrumento(i) = &mut e.figura {
        if mueve_x && mueve_y {
            let k = sx.abs().min(sy.abs()) as f64;
            let k = if k.is_finite() && k > 0.0 { k } else { 1.0 };
            i.unidad = Some(i.unidad_util() * k);
        }
        if let Some(t) = e.extras.tam_letra.as_mut() {
            *t *= sy.abs();
        }
    }

    e.tocar();
}

/// **El tirador de giro sobre el espacio de tres ejes** (`girarElEspacio`
/// del movil): le da la vuelta al espacio y deja la caja derecha, para que
/// los numeros se sigan leyendo. El angulo es el del puntero respecto del
/// centro con el tirador arriba como cero, en grados de -180 a 180, a saltos
/// de quince con `discreto`. Devuelve si `e` era un espacio.
pub fn girar_el_espacio(e: &mut Elemento, p: Punto2, discreto: bool) -> bool {
    if !matches!(&e.figura, Figura::Instrumento(i) if i.ejes == 3) {
        return false;
    }
    let (x0, y0, x1, y1) = e.caja();
    let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let mut grados = (angulo_hacia(c, p) as f64).to_degrees();
    if discreto {
        grados = (grados / 15.0).round() * 15.0;
    }
    grados = ((grados % 360.0) + 540.0) % 360.0 - 180.0;
    if let Figura::Instrumento(i) = &mut e.figura {
        i.azimut = Some(grados);
    }
    e.tocar();
    true
}

/// Gira `e` en `delta` radianes alrededor de `centro`.
///
/// Si `centro` es el suyo, gira sobre si mismo. Si es ajeno -el centro de
/// una seleccion de varios- ademas orbita: es lo que hace que girar cinco
/// elementos a la vez se vea como girar el conjunto.
///
/// Los `puntos` de un trazo estan en marco LOCAL: quien dibuja ya les
/// aplica `e.angulo` alrededor de su centro. Girarlos aqui **ademas** de
/// subir `e.angulo` cuenta la rotacion dos veces -al dibujar se deshace la
/// mitad de lo que se penso que se habia girado-, asi que aqui los puntos
/// solo se TRASLADAN por la orbita (`nuevo - propio`, que es un
/// desplazamiento, no un giro); el giro entero lo aporta `e.angulo`. Con
/// centro propio la orbita es cero y solo cambia el angulo, que es lo
/// correcto.
pub fn girar(e: &mut Elemento, centro: Punto2, delta: f32) {
    let propio = centro_de(e);
    let nuevo = propio.girar(centro, delta);
    let t = nuevo.restar(propio);

    // Mismo aviso que en `escalar_elemento`: este `match` lleva comodin, asi
    // que una figura nueva con puntos compila igual sin pasar por aqui.
    // Con centro propio la traslacion es cero y el fallo no se nota; en una
    // seleccion multiple (centro ajeno) los puntos se quedarian quietos
    // mientras x/y orbitan.
    match &mut e.figura {
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => {
            for q in puntos.iter_mut() {
                *q = q.sumar(t);
            }
        }
        _ => {}
    }

    e.x += t.x;
    e.y += t.y;
    e.angulo += delta;
    e.tocar();
}

/// El angulo del centro al punto, con **cero apuntando hacia arriba**.
///
/// Hacia arriba y no hacia la derecha porque el tirador de giro esta encima
/// del elemento: con el cursor ahi mismo, el giro tiene que ser cero.
pub fn angulo_hacia(centro: Punto2, p: Punto2) -> f32 {
    let d = p.restar(centro);
    d.x.atan2(-d.y)
}

/// Redondea a saltos de quince grados. Es lo que hace `Shift` al girar.
pub fn a_saltos(angulo: f32) -> f32 {
    (angulo / SALTO_GIRO).round() * SALTO_GIRO
}

// -------------------------------------------------------------------------
// Voltear
// -------------------------------------------------------------------------

/// Por que espejo se refleja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EjeVolteo {
    /// Izquierda por derecha.
    Horizontal,
    /// Arriba por abajo.
    Vertical,
}

/// Voltea `elementos` **sobre la caja comun** (`flip` de `Transform.kt`).
///
/// # Por que sobre la caja comun y no uno a uno
///
/// Volteando cada elemento por su cuenta, la composicion se queda igual y no
/// parece que haya pasado nada: cada figura se refleja dentro de su propio
/// sitio y el conjunto no se mueve. Lo que se pide al voltear una seleccion
/// es que **la izquierda pase a la derecha**, y eso solo ocurre reflejando
/// sobre un espejo comun.
///
/// # Por que no basta con el efecto que ya habia
///
/// Cruzar el ancla al escalar ya volteaba (D30, prueba
/// `cruzar_el_ancla_voltea_en_vez_de_aplastar`), pero eso es un accidente del
/// arrastre: exige pasar la figura entera por encima de si misma con el raton
/// apretado, no reflejar varios a la vez, y no es un paso de deshacer con
/// nombre. Voltear tiene que ser una orden.
///
/// # Lo que se voltea ademas de la geometria
///
/// - **El angulo se invierte**: reflejar invierte el sentido de giro, y sin
///   esto una figura inclinada a la derecha se quedaria inclinada a la
///   derecha dentro de un dibujo que ya mira al otro lado.
/// - **Las puntas de flecha se intercambian** al voltear en horizontal: si no,
///   una flecha que apuntaba a la izquierda acaba apuntando a la izquierda
///   despues de haber cruzado el dibujo entero.
/// - **Lo bloqueado no se toca**, igual que en el movil y que en el resto del
///   editor.
///
/// # En que se aparta del movil
///
/// Alla, ademas, se invierte el signo de `scale` en las imagenes, que es como
/// Excalidraw guarda que una foto esta del reves. Aqui `Figura::Imagen` no
/// tiene `scale` —solo `id_objeto`—, asi que una imagen se voltea de sitio
/// pero no de contenido. Es del grupo D, que es quien trae `crop` y `scale`.
pub fn voltear(elementos: &mut [Elemento], eje: EjeVolteo) {
    let Some((x0, y0, x1, y1)) = caja_comun(elementos) else {
        return;
    };
    let espejo_x = (x0 + x1) / 2.0;
    let espejo_y = (y0 + y1) / 2.0;
    let reflejar = |p: Punto2| match eje {
        EjeVolteo::Horizontal => Punto2::nuevo(2.0 * espejo_x - p.x, p.y),
        EjeVolteo::Vertical => Punto2::nuevo(p.x, 2.0 * espejo_y - p.y),
    };

    for e in elementos.iter_mut() {
        if e.bloqueado || e.borrado {
            continue;
        }
        let (ex0, ey0, ex1, ey1) = e.caja();

        match &mut e.figura {
            // **Las rayas y el lapiz se reflejan por sus puntos, no por su
            // caja.** Su caja SALE de los puntos, asi que espejar la caja por
            // un lado y los puntos por otro los descoloca entre si: un
            // garabato trazado hacia la izquierda salia disparado en vez de
            // quedarse donde estaba. Aqui los puntos son absolutos, asi que
            // basta con reflejarlos y recolocar `x`/`y` detras.
            Figura::Lapiz { puntos, .. }
            | Figura::Resaltador { puntos }
            | Figura::Linea { puntos }
            | Figura::Flecha { puntos, .. }
            | Figura::Cota { puntos } => {
                for q in puntos.iter_mut() {
                    *q = reflejar(*q);
                }
            }
            // El relleno encontrado guarda contorno y agujeros, y los dos son
            // absolutos aqui: **los agujeros se reflejan con el contorno o el
            // anillo se abre** y el relleno acaba tapando justo lo que se
            // queria dejar ver.
            Figura::Region { contorno, huecos } => {
                for q in contorno.iter_mut().chain(huecos.iter_mut().flatten()) {
                    *q = reflejar(*q);
                }
            }
            // La flecha intercambia sus puntas: hay que hacerlo aqui dentro
            // porque `Figura::Flecha` ya se llevo el prestamo arriba, asi que
            // se resuelve despues, fuera del `match`.
            _ => {}
        }

        match &e.figura {
            Figura::Lapiz { .. }
            | Figura::Resaltador { .. }
            | Figura::Linea { .. }
            | Figura::Flecha { .. }
            | Figura::Cota { .. }
            | Figura::Region { .. } => {
                // `x`/`y` se recolocan a la esquina de la caja nueva: es lo
                // que se escribe en el fichero como origen de los puntos.
                let (nx0, ny0, _, _) = e.caja();
                e.x = nx0;
                e.y = ny0;
            }
            // Las que se pintan por su caja se mueven de caja: la esquina que
            // estaba a la izquierda pasa a ser la de la derecha.
            _ => match eje {
                EjeVolteo::Horizontal => e.x = 2.0 * espejo_x - ex1,
                EjeVolteo::Vertical => e.y = 2.0 * espejo_y - ey1,
            },
        }
        let _ = (ex0, ey0, ey1);

        if let Figura::Flecha {
            punta_inicio,
            punta_fin,
            ..
        } = &mut e.figura
        {
            if eje == EjeVolteo::Horizontal {
                std::mem::swap(punta_inicio, punta_fin);
            }
        }

        // Reflejar invierte el sentido de giro.
        if e.angulo != 0.0 {
            e.angulo = -e.angulo;
        }
        e.tocar();
    }
}

/// **Las flechas atadas, despues de voltear** `volteados` en una escena. Se
/// llama dentro del paso de deshacer que volteo.
///
/// [`voltear`] trabaja sobre un trozo de elementos y no ve la escena, asi que
/// esto va aparte. Hace dos cosas, como `actionFlip` de Excalidraw:
///
/// - las flechas que cuelgan de lo volteado y no se voltearon, lo siguen;
/// - las flechas volteadas **se vuelven a atar por las dos puntas**, aunque
///   su figura se volteara con ellas. Reflejar cambia de lado el sitio al que
///   apuntan —el `foco` y el `punto_fijo` guardados son del lado de antes—,
///   y sin re-atarlas la proxima vez que se moviera la caja la punta saltaria
///   al lado opuesto. Cada punta conserva su modo (borde o dentro).
pub fn atar_tras_voltear(escena: &mut crate::escena::Escena, volteados: &[u64]) {
    use crate::enlace::{self, Extremo};
    enlace::seguir(escena, volteados);
    // Solo las puntas cuya figura se volteo con la flecha. Lo que no
    // resuelve no se toca: viaja intacto al movil.
    let elementos = &escena.elementos;
    let puntas: Vec<(u64, Extremo)> = elementos
        .iter()
        .filter(|e| {
            !e.borrado && volteados.contains(&e.id) && matches!(e.figura, Figura::Flecha { .. })
        })
        .flat_map(|e| {
            [
                (&e.extras.enganche_inicio, Extremo::Inicio),
                (&e.extras.enganche_fin, Extremo::Fin),
            ]
            .into_iter()
            .filter_map(|(b, x)| {
                let o = enlace::resolver(elementos, b.as_ref()?)?;
                volteados.contains(&o.id).then_some((e.id, x))
            })
            .collect::<Vec<_>>()
        })
        .collect();
    for (id, extremo) in puntas {
        enlace::reatar_a_su_figura(escena, id, extremo);
    }
    // Y la punta cuya figura se quedo quieta se revisa como al soltar una
    // flecha movida sola: sigue atada si aun cae encima, si no se suelta.
    enlace::revisar_flechas_movidas(escena, |id| volteados.contains(&id), 1.0);
}

/// La caja que abarca a todos los que cuentan.
fn caja_comun(elementos: &[Elemento]) -> Option<(f32, f32, f32, f32)> {
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

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use crate::formas::TipoPunta;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_6, PI};

    /// Un rectangulo de 100x50 con la esquina en el origen.
    fn rect() -> Elemento {
        Elemento {
            id: 1,
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

    /// La esquina superior izquierda, ya en el mundo (girada).
    fn esquina_no(e: &Elemento) -> Punto2 {
        let (x0, y0, x1, y1) = e.caja();
        let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        Punto2::nuevo(x0, y0).girar(c, e.angulo)
    }

    fn cerca(a: Punto2, b: Punto2, que: &str) {
        assert!(
            (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3,
            "{que}: esperaba ({}, {}), es ({}, {})",
            b.x,
            b.y,
            a.x,
            a.y
        );
    }

    #[test]
    fn escalar_por_una_esquina_deja_quieta_la_de_enfrente() {
        let mut e = rect();
        let ancla = esquina_no(&e);

        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 100.0),
            false,
            false,
        );

        assert_eq!(e.ancho, 200.0);
        assert_eq!(e.alto, 100.0);
        cerca(esquina_no(&e), ancla, "la esquina anclada");
    }

    /// El punto del mundo al que hay que arrastrar para hacer, en un
    /// elemento girado `angulo`, **el mismo gesto** que `local` haria en uno
    /// sin girar.
    ///
    /// Sin esto, las pruebas de abajo arrastrarian al mismo punto del mundo
    /// para todos los angulos, que no es el mismo gesto: a 90 grados ese
    /// punto cae al otro lado del ancla y lo que sale es un volteo, no un
    /// escalado. Y tras un volteo la esquina que se queda quieta ya no es la
    /// noroeste, asi que la comprobacion dejaria de tener sentido.
    fn arrastre_equivalente(local: Punto2, centro: Punto2, angulo: f32) -> Punto2 {
        local.girar(centro, angulo)
    }

    #[test]
    fn con_el_elemento_girado_la_esquina_anclada_sigue_sin_moverse() {
        // ESTA es la prueba que justifica la tarea entera. La forma
        // evidente —cambiar ancho/alto y volver a girar— la falla, porque
        // el centro se desplaza y el elemento acaba girado alrededor de un
        // centro nuevo.
        for angulo in [0.0, FRAC_PI_6, FRAC_PI_2, PI, 2.6] {
            let mut e = rect();
            e.angulo = angulo;
            let centro = Punto2::nuevo(50.0, 25.0);
            let ancla = esquina_no(&e);
            let destino = arrastre_equivalente(Punto2::nuevo(180.0, 90.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, destino, false, false);

            cerca(esquina_no(&e), ancla, &format!("a {angulo} radianes"));
            assert!(
                (e.ancho - 180.0).abs() < 1e-2,
                "ancho a {angulo}: {}",
                e.ancho
            );
            assert!((e.alto - 90.0).abs() < 1e-2, "alto a {angulo}: {}", e.alto);
        }
    }

    #[test]
    fn escalar_y_devolver_deja_el_elemento_donde_estaba() {
        for angulo in [0.0, FRAC_PI_6, FRAC_PI_2, PI] {
            let mut e = rect();
            e.angulo = angulo;
            let centro = Punto2::nuevo(50.0, 25.0);
            let se_original = arrastre_equivalente(Punto2::nuevo(100.0, 50.0), centro, angulo);
            let lejos = arrastre_equivalente(Punto2::nuevo(300.0, 150.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, lejos, false, false);
            escalar(&mut e, Tirador::SuresteEsquina, se_original, false, false);

            assert!(
                (e.ancho - 100.0).abs() < 1e-2,
                "ancho a {angulo}: {}",
                e.ancho
            );
            assert!((e.alto - 50.0).abs() < 1e-2, "alto a {angulo}: {}", e.alto);
        }
    }

    #[test]
    fn cruzar_el_ancla_con_el_elemento_girado_voltea_sin_degenerar() {
        // El caso que las dos pruebas de arriba evitan a proposito, aqui
        // comprobado de frente: arrastrar al otro lado del ancla en un
        // elemento girado voltea. Lo que se exige es que el resultado siga
        // siendo un elemento valido —sin cero, sin NaN— y no que la esquina
        // noroeste siga quieta, porque tras un volteo esa ya no es el ancla.
        for angulo in [0.0, FRAC_PI_2, 2.6] {
            let mut e = rect();
            e.angulo = angulo;
            let centro = Punto2::nuevo(50.0, 25.0);
            let detras = arrastre_equivalente(Punto2::nuevo(-120.0, 90.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, detras, false, false);

            assert!(e.ancho >= MINIMO, "no se aplasta a {angulo}: {}", e.ancho);
            assert!(e.alto >= MINIMO, "ni de alto a {angulo}: {}", e.alto);
            assert!(e.x.is_finite() && e.y.is_finite(), "sin NaN a {angulo}");
        }
    }

    #[test]
    fn un_tirador_de_lado_solo_mueve_su_eje() {
        let mut e = rect();
        escalar(
            &mut e,
            Tirador::EsteBorde,
            Punto2::nuevo(300.0, 999.0),
            false,
            false,
        );
        assert_eq!(e.ancho, 300.0);
        assert_eq!(e.alto, 50.0, "el lado este no toca el alto");
    }

    #[test]
    fn escalar_un_trazo_mueve_sus_puntos() {
        // Elemento::caja() calcula la caja DE LOS PUNTOS para las figuras
        // con puntos. Si solo se cambiara ancho/alto, el trazo no escalaria
        // y el marco de seleccion se despegaria del dibujo.
        //
        // Los numeros de abajo salen a mano de la formula del encabezado de
        // este fichero, NO de volver a leer `e.caja()`: comparar la caja
        // final consigo misma es circular, solo falla si alguien vuelve a
        // la formula vieja `ancho * sx`.
        //
        // Con grosor 4.0, la caja de partida lleva un margen de 2.0 por
        // lado: (-2,-2,102,52), es decir ancho=104, alto=54. El tirador
        // SuresteEsquina ancla la esquina opuesta (noroeste), (-2,-2).
        //   sx = (200 - (-2)) / 104 = 202/104
        //   sy = (100 - (-2)) /  54 = 102/54
        // Sin giro, cada punto q sale de ancla + (q - ancla) * (sx, sy):
        //   (0,0)     -> (-2 + 2*sx,   -2 + 2*sy)   = (49/26, 16/9)
        //   (50,25)   -> (-2 + 52*sx,  -2 + 27*sy)  = (99, 49)
        //   (100,50)  -> (-2 + 102*sx, -2 + 52*sy)  = (5099/26, 866/9)
        // La caja final vuelve a sumar el margen de 2.0 a los extremos de
        // esos puntos.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(50.0, 25.0),
                    Punto2::nuevo(100.0, 50.0),
                ],
                presiones: Vec::new(),
                opciones: None,
            },
            grosor: 4.0,
            ..rect()
        };

        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 100.0),
            false,
            false,
        );

        let Figura::Lapiz { puntos, .. } = &e.figura else {
            panic!("sigue siendo un lapiz");
        };
        assert_eq!(puntos.len(), 3, "sigue teniendo sus tres puntos");
        cerca(puntos[0], Punto2::nuevo(49.0 / 26.0, 16.0 / 9.0), "primero");
        cerca(puntos[1], Punto2::nuevo(99.0, 49.0), "segundo");
        cerca(
            puntos[2],
            Punto2::nuevo(5099.0 / 26.0, 866.0 / 9.0),
            "tercero",
        );

        assert!(
            (e.ancho - 198.230_77).abs() < 1e-2,
            "ancho: esperaba 198.23, es {}",
            e.ancho
        );
        assert!(
            (e.alto - 98.444_44).abs() < 1e-2,
            "alto: esperaba 98.44, es {}",
            e.alto
        );
        assert!(
            (e.x - (-0.115_38)).abs() < 1e-2,
            "x: esperaba -0.12, es {}",
            e.x
        );
        assert!(
            (e.y - (-0.222_22)).abs() < 1e-2,
            "y: esperaba -0.22, es {}",
            e.y
        );
    }

    #[test]
    fn escalar_un_trazo_girado_deja_quieto_el_ancla_en_el_mundo() {
        // El hueco exacto que dejo sin cazar: ninguna prueba tenia una
        // figura con puntos Y angulo distinto de cero a la vez.
        for angulo in [FRAC_PI_6, FRAC_PI_2] {
            let mut e = Elemento {
                figura: Figura::Lapiz {
                    puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 50.0)],
                    presiones: Vec::new(),
                    opciones: None,
                },
                grosor: 0.0,
                angulo,
                ..rect()
            };
            let centro = Punto2::nuevo(50.0, 25.0);
            let ancla = esquina_no(&e);
            let destino = arrastre_equivalente(Punto2::nuevo(180.0, 90.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, destino, false, false);

            cerca(
                esquina_no(&e),
                ancla,
                &format!("el ancla del trazo a {angulo}"),
            );

            let (x0, y0, x1, y1) = e.caja();
            let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let Figura::Lapiz { puntos, .. } = &e.figura else {
                panic!()
            };
            cerca(
                puntos[1].girar(c, e.angulo),
                destino,
                &format!("el extremo arrastrado queda bajo el cursor a {angulo}"),
            );
        }
    }

    #[test]
    fn escalar_una_cota_mueve_sus_puntos() {
        // Critico 1 del re-revisor: la Cota caia en el comodin `_` de este
        // match, que recalcula x/y/ancho/alto como figura de caja SIN tocar
        // los puntos; y el `if e.puntos().is_some()` de mas abajo volvia a
        // pisar eso con la caja de esos mismos puntos, sin mover. Neto:
        // escalar una cota no hacia absolutamente nada. Misma cuenta que
        // `escalar_un_trazo_mueve_sus_puntos`, con `Figura::Cota`.
        let mut e = Elemento {
            figura: Figura::Cota {
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(50.0, 25.0),
                    Punto2::nuevo(100.0, 50.0),
                ],
            },
            grosor: 4.0,
            ..rect()
        };

        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 100.0),
            false,
            false,
        );

        let Figura::Cota { puntos } = &e.figura else {
            panic!("sigue siendo una cota");
        };
        assert_eq!(puntos.len(), 3, "sigue teniendo sus tres puntos");
        cerca(puntos[0], Punto2::nuevo(49.0 / 26.0, 16.0 / 9.0), "primero");
        cerca(puntos[1], Punto2::nuevo(99.0, 49.0), "segundo");
        cerca(
            puntos[2],
            Punto2::nuevo(5099.0 / 26.0, 866.0 / 9.0),
            "tercero",
        );

        assert!(
            (e.ancho - 198.230_77).abs() < 1e-2,
            "ancho: esperaba 198.23, es {}",
            e.ancho
        );
        assert!(
            (e.alto - 98.444_44).abs() < 1e-2,
            "alto: esperaba 98.44, es {}",
            e.alto
        );
    }

    #[test]
    fn escalar_una_cota_girada_deja_quieto_el_ancla_en_el_mundo() {
        // Mismo hueco que `escalar_un_trazo_girado_deja_quieto_el_ancla_en_el_mundo`,
        // con una figura Y angulo distinto de cero a la vez, pero con Cota.
        for angulo in [FRAC_PI_6, FRAC_PI_2] {
            let mut e = Elemento {
                figura: Figura::Cota {
                    puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 50.0)],
                },
                grosor: 0.0,
                angulo,
                ..rect()
            };
            let centro = Punto2::nuevo(50.0, 25.0);
            let ancla = esquina_no(&e);
            let destino = arrastre_equivalente(Punto2::nuevo(180.0, 90.0), centro, angulo);

            escalar(&mut e, Tirador::SuresteEsquina, destino, false, false);

            cerca(
                esquina_no(&e),
                ancla,
                &format!("el ancla de la cota a {angulo}"),
            );

            let (x0, y0, x1, y1) = e.caja();
            let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let Figura::Cota { puntos } = &e.figura else {
                panic!()
            };
            cerca(
                puntos[1].girar(c, e.angulo),
                destino,
                &format!("el extremo arrastrado queda bajo el cursor a {angulo}"),
            );
        }
    }

    #[test]
    fn escalar_anisotropo_un_trazo_girado_no_lo_manda_a_otro_sitio() {
        // El contraejemplo del critico 2, tal cual: tratar los puntos
        // -que estan en marco LOCAL- como si ya fueran del mundo manda el
        // trazo a otro sitio en cuanto angulo != 0.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 50.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            grosor: 0.0,
            angulo: FRAC_PI_2,
            ..rect()
        };

        // sx = 2, sy = 1 al tirar de la esquina sureste hasta (25, 175).
        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(25.0, 175.0),
            false,
            false,
        );

        let (x0, y0, x1, y1) = e.caja();
        let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let Figura::Lapiz { puntos, .. } = &e.figura else {
            panic!()
        };
        cerca(
            puntos[0].girar(c, e.angulo),
            Punto2::nuevo(75.0, -25.0),
            "el ancla, geometria efectiva",
        );
        cerca(
            puntos[1].girar(c, e.angulo),
            Punto2::nuevo(25.0, 175.0),
            "el extremo arrastrado, geometria efectiva",
        );
    }

    #[test]
    fn con_shift_se_conserva_la_proporcion() {
        let mut e = rect();
        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 999.0),
            true,
            false,
        );
        assert!(
            (e.ancho / e.alto - 2.0).abs() < 1e-3,
            "100x50 es 2:1, y es {}x{}",
            e.ancho,
            e.alto
        );
    }

    #[test]
    fn con_alt_se_escala_desde_el_centro() {
        let mut e = rect();
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(150.0, 75.0),
            false,
            true,
        );

        let (x0, y0, x1, y1) = e.caja();
        cerca(
            Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            centro,
            "el centro no se mueve",
        );
    }

    #[test]
    fn cruzar_el_ancla_voltea_en_vez_de_aplastar() {
        // D30: aplastar a cero pierde informacion sin remedio; voltear es
        // reversible y es lo que espera quien cruzo el raton al otro lado.
        let mut e = rect();
        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(-100.0, 50.0),
            false,
            false,
        );

        assert!(e.ancho >= MINIMO, "no se aplasta: {}", e.ancho);
        let (x0, _, x1, _) = e.caja();
        assert!(x0 < 0.0 && x1 <= 0.0 + 1e-3, "quedo al otro lado del ancla");
    }

    #[test]
    fn girar_alrededor_de_su_centro_suma_el_angulo_y_no_lo_mueve() {
        let mut e = rect();
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        girar(&mut e, centro, FRAC_PI_2);

        assert!((e.angulo - FRAC_PI_2).abs() < 1e-6);
        let (x0, y0, x1, y1) = e.caja();
        cerca(
            Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0),
            centro,
            "girar sobre si mismo no lo mueve",
        );
    }

    #[test]
    fn girar_alrededor_de_un_centro_ajeno_lo_mueve_en_orbita() {
        // Es lo que pasa al girar una seleccion de varios: cada uno gira
        // sobre si mismo Y orbita el centro comun.
        let mut e = rect();
        let ajeno = Punto2::nuevo(0.0, 0.0);
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        girar(&mut e, ajeno, FRAC_PI_2);

        let (x0, y0, x1, y1) = e.caja();
        let nuevo = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        cerca(
            nuevo,
            centro.girar(ajeno, FRAC_PI_2),
            "orbito el centro comun",
        );
    }

    #[test]
    fn girar_una_cota_alrededor_de_un_centro_ajeno_traslada_sus_puntos() {
        // Critico 2 del re-revisor: la Cota caia en el `_ => {}` de este
        // match. Con centro propio la traslacion es cero y no se nota, pero
        // con centro ajeno (una seleccion de varios) los puntos se
        // quedarian quietos mientras x/y orbitan: desincronizado en cuanto
        // exista el pintado.
        let mut e = Elemento {
            figura: Figura::Cota {
                puntos: vec![Punto2::nuevo(10.0, 0.0), Punto2::nuevo(20.0, 0.0)],
            },
            grosor: 0.0,
            ..rect()
        };
        let ajeno = Punto2::nuevo(0.0, 0.0);
        let x_antes = e.x;
        let y_antes = e.y;

        girar(&mut e, ajeno, FRAC_PI_2);

        assert!((e.angulo - FRAC_PI_2).abs() < 1e-6);
        let (x0, y0, x1, y1) = e.caja();
        let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let Figura::Cota { puntos } = &e.figura else {
            panic!()
        };
        // Con angulo inicial 0, girar alrededor de un centro ajeno es lo
        // mismo que girar los puntos originales alrededor de ese centro: es
        // la comprobacion independiente de la formula.
        cerca(
            puntos[0].girar(c, e.angulo),
            Punto2::nuevo(10.0, 0.0).girar(ajeno, FRAC_PI_2),
            "el primer punto, geometria efectiva",
        );
        cerca(
            puntos[1].girar(c, e.angulo),
            Punto2::nuevo(20.0, 0.0).girar(ajeno, FRAC_PI_2),
            "el segundo punto, geometria efectiva",
        );
        assert!(
            (e.x - x_antes).abs() > 1e-3 || (e.y - y_antes).abs() > 1e-3,
            "x/y tienen que orbitar, no solo el angulo"
        );
    }

    #[test]
    fn girar_un_trazo_gira_su_geometria_efectiva() {
        // Los puntos de un trazo estan en marco LOCAL: quien dibuja les
        // aplica `e.angulo` alrededor del centro de su caja. Por eso lo que
        // tiene que comprobarse es esa geometria EFECTIVA (puntos girados
        // por `e.angulo`), no los puntos crudos: girarlos aqui otra vez,
        // ademas de subir `e.angulo`, contaria la rotacion dos veces y el
        // trazo no se moveria.
        let mut e = Elemento {
            figura: Figura::Lapiz {
                puntos: vec![Punto2::nuevo(10.0, 0.0), Punto2::nuevo(20.0, 0.0)],
                presiones: Vec::new(),
                opciones: None,
            },
            grosor: 0.0,
            ..rect()
        };
        girar(&mut e, Punto2::nuevo(0.0, 0.0), FRAC_PI_2);

        assert!((e.angulo - FRAC_PI_2).abs() < 1e-6);
        let (x0, y0, x1, y1) = e.caja();
        let c = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let Figura::Lapiz { puntos, .. } = &e.figura else {
            panic!()
        };
        // Con angulo inicial 0, girar el elemento alrededor de un centro
        // ajeno es lo mismo que girar sus puntos originales alrededor de
        // ese mismo centro: es la comprobacion independiente de la formula.
        cerca(
            puntos[0].girar(c, e.angulo),
            Punto2::nuevo(10.0, 0.0).girar(Punto2::nuevo(0.0, 0.0), FRAC_PI_2),
            "el primero, geometria efectiva",
        );
        cerca(
            puntos[1].girar(c, e.angulo),
            Punto2::nuevo(20.0, 0.0).girar(Punto2::nuevo(0.0, 0.0), FRAC_PI_2),
            "el segundo, geometria efectiva",
        );
    }

    #[test]
    fn los_saltos_de_giro_son_de_quince_grados() {
        // La media division son 7,5 grados = 0,1309 rad: por debajo se baja
        // al salto anterior y por encima se sube al siguiente.
        assert!(
            (a_saltos(0.10) - 0.0).abs() < 1e-6,
            "0,10 rad (5,7 grados) baja a 0"
        );
        assert!(
            (a_saltos(0.20) - SALTO_GIRO).abs() < 1e-6,
            "0,20 rad (11,5) ya sube a 15"
        );
        assert!(
            (a_saltos(0.30) - SALTO_GIRO).abs() < 1e-6,
            "0,30 rad (17,2) tambien"
        );
        assert!((a_saltos(-0.30) + SALTO_GIRO).abs() < 1e-6, "y en negativo");
    }

    #[test]
    fn el_angulo_hacia_arriba_es_cero() {
        // El tirador de giro esta encima del elemento; con el cursor ahi
        // mismo, el giro tiene que ser cero y no un cuarto de vuelta.
        let c = Punto2::nuevo(0.0, 0.0);
        assert!(angulo_hacia(c, Punto2::nuevo(0.0, -10.0)).abs() < 1e-6);
        let derecha = angulo_hacia(c, Punto2::nuevo(10.0, 0.0));
        assert!(
            (derecha - FRAC_PI_2).abs() < 1e-6,
            "a la derecha, +90: {derecha}"
        );
    }

    #[test]
    fn transformar_sube_la_version_para_invalidar_la_cache() {
        let mut e = rect();
        let antes = e.version;
        escalar(
            &mut e,
            Tirador::SuresteEsquina,
            Punto2::nuevo(200.0, 100.0),
            false,
            false,
        );
        assert_ne!(e.version, antes, "sin esto la cache pintaria lo viejo");
    }

    // --- Voltear ---

    fn en(x: f32, ancho: f32) -> Elemento {
        Elemento { x, ancho, ..rect() }
    }

    #[test]
    fn voltear_en_horizontal_pasa_la_izquierda_a_la_derecha() {
        // Sobre la caja comun: es lo que hace que la composicion cambie. Dos
        // cajas de 20 en x=0 y x=80; el espejo esta en x=50.
        let mut es = [en(0.0, 20.0), en(80.0, 20.0)];
        voltear(&mut es, EjeVolteo::Horizontal);
        assert_eq!(es[0].x, 80.0, "la de la izquierda cruza a la derecha");
        assert_eq!(es[1].x, 0.0);
    }

    #[test]
    fn voltear_uno_solo_lo_refleja_sobre_si_mismo_y_no_lo_mueve() {
        // Con un solo elemento la caja comun es la suya, asi que se queda
        // donde esta: lo que cambia es su contenido. Es lo que se espera al
        // voltear una sola figura.
        let mut es = [rect()];
        voltear(&mut es, EjeVolteo::Horizontal);
        assert_eq!((es[0].x, es[0].y), (0.0, 0.0));
    }

    #[test]
    fn un_trazo_se_refleja_por_sus_puntos_y_no_sale_disparado() {
        // El fallo que el movil documenta: la caja de un trazo SALE de sus
        // puntos. Espejando la caja por un lado y los puntos por otro, un
        // garabato con puntos negativos salia despedido.
        let mut trazo = rect();
        trazo.figura = Figura::Linea {
            puntos: vec![
                Punto2::nuevo(-30.0, 0.0),
                Punto2::nuevo(-10.0, 0.0),
                Punto2::nuevo(-10.0, 40.0),
            ],
        };
        let antes = trazo.caja();
        let mut es = [trazo];
        voltear(&mut es, EjeVolteo::Horizontal);
        let despues = es[0].caja();
        assert!(
            (antes.0 - despues.0).abs() < 1e-3 && (antes.2 - despues.2).abs() < 1e-3,
            "la caja se quedo en {despues:?} en vez de {antes:?}"
        );
        let Figura::Linea { puntos } = &es[0].figura else {
            unreachable!()
        };
        // El espejo de su propia caja: el primer punto y el ultimo cambian de
        // lado.
        assert!((puntos[0].x + 10.0).abs() < 1e-3, "{:?}", puntos[0]);
    }

    #[test]
    fn voltear_en_horizontal_intercambia_las_puntas_de_la_flecha() {
        // Si no, una flecha que apuntaba a la izquierda sigue apuntando a la
        // izquierda despues de haber cruzado el dibujo entero.
        let mut f = rect();
        f.figura = Figura::Flecha {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        };
        let mut es = [f];
        voltear(&mut es, EjeVolteo::Horizontal);
        let Figura::Flecha {
            punta_inicio,
            punta_fin,
            ..
        } = &es[0].figura
        else {
            unreachable!()
        };
        assert_eq!(
            *punta_inicio,
            TipoPunta::Flecha,
            "la punta paso al principio"
        );
        assert_eq!(*punta_fin, TipoPunta::Ninguna);
    }

    #[test]
    fn voltear_en_vertical_no_toca_las_puntas() {
        // Caso negativo: arriba por abajo no cambia cual es el principio de
        // la flecha.
        let mut f = rect();
        f.figura = Figura::Flecha {
            puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        };
        let mut es = [f];
        voltear(&mut es, EjeVolteo::Vertical);
        let Figura::Flecha {
            punta_inicio,
            punta_fin,
            ..
        } = &es[0].figura
        else {
            unreachable!()
        };
        assert_eq!(*punta_inicio, TipoPunta::Ninguna);
        assert_eq!(*punta_fin, TipoPunta::Flecha, "la punta sigue al final");
    }

    #[test]
    fn voltear_invierte_el_sentido_del_giro() {
        let mut e = rect();
        e.angulo = FRAC_PI_6;
        let mut es = [e];
        voltear(&mut es, EjeVolteo::Horizontal);
        assert!((es[0].angulo + FRAC_PI_6).abs() < 1e-6);
    }

    #[test]
    fn los_agujeros_de_un_relleno_se_reflejan_con_su_contorno() {
        // O el anillo se abre y el relleno acaba tapando justo lo que se
        // queria dejar ver.
        let mut r = rect();
        r.figura = Figura::Region {
            contorno: vec![
                Punto2::nuevo(0.0, 0.0),
                Punto2::nuevo(100.0, 0.0),
                Punto2::nuevo(100.0, 100.0),
                Punto2::nuevo(0.0, 100.0),
            ],
            huecos: vec![vec![
                Punto2::nuevo(10.0, 40.0),
                Punto2::nuevo(30.0, 40.0),
                Punto2::nuevo(30.0, 60.0),
                Punto2::nuevo(10.0, 60.0),
            ]],
        };
        let mut es = [r];
        voltear(&mut es, EjeVolteo::Horizontal);
        let Figura::Region { huecos, .. } = &es[0].figura else {
            unreachable!()
        };
        // El agujero estaba pegado a la izquierda; ahora a la derecha.
        assert!((huecos[0][0].x - 90.0).abs() < 1e-3, "{:?}", huecos[0]);
    }

    #[test]
    fn voltear_respeta_lo_bloqueado_y_lo_borrado() {
        // Caso negativo: bloquear es pedir que no se mueva, pase lo que pase.
        let mut fijo = en(0.0, 20.0);
        fijo.bloqueado = true;
        let mut es = [fijo, en(80.0, 20.0)];
        voltear(&mut es, EjeVolteo::Horizontal);
        assert_eq!(es[0].x, 0.0, "lo bloqueado se quedo donde estaba");
        assert_eq!(es[1].x, 0.0);
    }

    #[test]
    fn voltear_una_seleccion_vacia_no_hace_nada() {
        let mut nada: [Elemento; 0] = [];
        voltear(&mut nada, EjeVolteo::Horizontal);
    }

    #[test]
    fn voltear_dos_veces_devuelve_el_dibujo_a_su_sitio() {
        // La comprobacion de ida y vuelta: es la unica forma de estar seguro
        // de que la caja comun y el reflejo de puntos no discrepan.
        let mut trazo = rect();
        trazo.figura = Figura::Linea {
            puntos: vec![Punto2::nuevo(-30.0, 5.0), Punto2::nuevo(70.0, 45.0)],
        };
        let original = [trazo.clone(), en(80.0, 20.0)];
        let mut es = original.clone();
        voltear(&mut es, EjeVolteo::Horizontal);
        voltear(&mut es, EjeVolteo::Horizontal);
        for (a, b) in es.iter().zip(original.iter()) {
            assert!(
                (a.caja().0 - b.caja().0).abs() < 1e-3 && (a.caja().2 - b.caja().2).abs() < 1e-3,
                "{:?} no volvio a {:?}",
                a.caja(),
                b.caja()
            );
        }
    }
}
