//! El iman: **un solo sitio que decide a que se pega el cursor**.
//!
//! Es lo que separa un esquema de un monton de rayas casi alineadas. Con el
//! raton se falla la esquina exacta de un rectangulo por dos o tres pixeles,
//! y esos dos o tres pixeles se ven.
//!
//! ## Por que hay una sola puerta
//!
//! En el Android que se porta, seis sitios distintos llamaban al buscador de
//! anclajes y cada uno decidia por su cuenta tres cosas: si engancha, con que
//! radio y que anclajes cuentan. El resultado previsible fue que cada
//! herramienta nueva habia que acordarse de anadirla a esa lista, y a alguna
//! se le olvidaba. Aqui se dice **que se esta haciendo** ([`Faena`]) y el
//! motor responde: una herramienta nueva encaja en una faena y hereda su
//! comportamiento entero, sin tocar nada de este fichero.
//!
//! Sin escritorio y sin estado: se le dan los elementos y un punto, y dice a
//! donde habria que pegarse. Asi se comprueba sin dispositivo que engancha
//! donde debe y -mas importante- que **no** engancha donde no debe.

use serde::{Deserialize, Serialize};

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Que clase de punto notable es. Sirve para pintar la pista de otra forma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoAnclaje {
    /// Una de las cuatro esquinas de la caja de una figura.
    Esquina,
    /// El primer o el ultimo punto de una figura con puntos.
    Extremo,
    /// El medio de un lado, o el punto de en medio de una lista de puntos.
    Medio,
    /// El centro de la caja. En una elipse es el centro de la
    /// circunferencia, que es lo que se busca al trazar un radio.
    Centro,

    /// **El eje de coordenadas**: su origen y sus dos rectas.
    ///
    /// No sale de ninguna figura: es la referencia del plano. Sin el, poner
    /// algo «justo en el eje» o «a la altura del cero» hay que hacerlo a
    /// ojo, que es lo contrario de para lo que se pone un eje.
    Eje,

    /// **Donde se cruzan dos figuras cualesquiera.**
    ///
    /// Es el que mas se echa de menos y el unico que **no pertenece a
    /// ninguna figura**: nace de la relacion entre dos. Sin el, cerrar un
    /// contorno donde dos trazos se cruzan es imposible a pulso, porque el
    /// punto que buscas no existe como vertice de nada.
    ///
    /// Cualesquiera de verdad —una elipse contra un rectangulo, un arco
    /// contra un garabato, o una figura consigo misma donde su propio trazo
    /// vuelve a cruzarse—, porque quien saca el perimetro es
    /// [`crate::perimetros`] y sabe reducir cualquier figura a tramos.
    Interseccion,

    /// **Cualquier punto del borde de una figura: la escuadra.**
    ///
    /// Es el unico enganche que no lleva a un punto notable sino a **todo un
    /// canto**. Los demas sirven para empezar y acabar un trazo sobre una
    /// guia; este sirve para **recorrerla**, que es justo lo que no se podia:
    /// entre esquina y esquina no hay ningun punto al que pegarse, asi que el
    /// lado salia torcido y la curva de un circulo no habia forma de
    /// repasarla a pulso.
    ///
    /// Va el ultimo en prioridad **y ademas fuera de la puja por cercania**:
    /// ver la nota de [`sitio_fino`].
    Borde,

    /// **Donde la recta que se esta trazando roza una circunferencia**: el
    /// punto de tangencia desde donde empezo el trazo (`AjustesFinos::desde`).
    ///
    /// Es el de las laminas de geometria —«la tangente desde P»— y a pulso es
    /// imposible: el punto no esta marcado en ningun sitio y unos pocos
    /// pixeles de mas o de menos convierten la tangente en secante.
    Tangente,
}

/// Un punto al que merece la pena pegarse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anclaje {
    /// Ya en coordenadas del mundo, girado si el elemento lo estaba.
    pub punto: Punto2,
    pub tipo: TipoAnclaje,
    pub id: u64,
}

/// Que se esta haciendo cuando se pide sitio.
///
/// No es «que herramienta hay puesta», y es a proposito. Lo que decide a que
/// debe pegarse el cursor no es el boton que este pulsado sino **que esta
/// pasando**: afinar la punta de una flecha ya trazada y afinar la de una
/// recta son la misma faena aunque sean dos herramientas, y trazar a mano
/// alzada es otra aunque el lapiz y el marcador sean dos botones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Faena {
    /// Naciendo una figura: se arrastra y va creciendo.
    Trazando,
    /// A mano alzada. **No engancha a nada**: ver [`ajustes_para`].
    AMano,
    /// Recolocando un punto de algo ya dibujado.
    Afinando,
    /// Arrastrando algo entero de un sitio a otro.
    Moviendo,
}

/// Que se engancha. Es la parte **configurable**: cada clase de punto se
/// puede apagar por separado, y apagarlas todas deja el dibujo libre.
///
/// No es una preferencia de adorno. Un iman que tira cuando no quieres es
/// peor que no tenerlo, y cual estorba depende de lo que estes dibujando.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    pub activo: bool,
    /// Gobierna [`TipoAnclaje::Esquina`] **y** [`TipoAnclaje::Extremo`]:
    /// para quien dibuja son la misma idea -el vertice de algo- y dos
    /// casillas para eso serian dos casillas que nadie entiende por
    /// separado.
    pub esquinas: bool,
    pub medios: bool,
    pub centros: bool,
    /// Radio de captura **en pixeles de pantalla**, no de escena.
    ///
    /// En pantalla es donde ocurre el problema: el cursor apunta con la
    /// misma precision mires al zoom que mires. Si el radio fuese de
    /// escena, muy acercado engancharia a medio dibujo y muy alejado no
    /// engancharia a nada.
    pub radio_px: f32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            activo: true,
            esquinas: true,
            medios: true,
            centros: true,
            radio_px: 14.0,
        }
    }
}

impl Ajustes {
    /// Todo apagado: dibujar a pulso, sin que nada tire del cursor.
    pub const NINGUNO: Self = Self {
        activo: false,
        esquinas: false,
        medios: false,
        centros: false,
        radio_px: 14.0,
    };
}

/// Cuanto tienen que diferenciarse dos distancias para que una gane por
/// cercania. Por debajo de esto es empate y manda la prioridad.
const EMPATE: f32 = 0.001;

/// Menor es mas prioritario.
fn prioridad(t: TipoAnclaje) -> u8 {
    match t {
        // La interseccion gana a todo: es la mas dificil de acertar a pulso
        // —no existe como vertice de ninguna figura— y por tanto la que mas
        // se agradece.
        TipoAnclaje::Interseccion => 0,
        // Un vertice es lo mas intencionado que hay en un dibujo. El eje va
        // con ellos: es igual de intencionado.
        TipoAnclaje::Esquina | TipoAnclaje::Extremo | TipoAnclaje::Eje => 1,
        TipoAnclaje::Medio | TipoAnclaje::Tangente => 2,
        // El centro de una caja grande esta lejos de todo, y engancharse a
        // el por sorpresa desconcierta mas de lo que ayuda.
        TipoAnclaje::Centro => 3,
        // El canto pasa por encima de los vertices de su propia figura: a
        // igual distancia gana el vertice, o apuntar a una esquina dejaria
        // el trazo *cerca* de la esquina en vez de *en* la esquina.
        TipoAnclaje::Borde => 4,
    }
}

/// Los tres anclajes que el movil tiene y aqui llegan ahora, con su origen
/// de coordenadas.
///
/// # Por que van aparte de [`Ajustes`]
///
/// [`Ajustes`] es lo que se guarda en las preferencias del usuario, y lo
/// construyen **campo a campo** dos sitios fuera de este crate
/// (`pixpin-store` y la ventana del editor). Anadirle un campo rompe su
/// compilacion, y esos ficheros son de otro dueno en esta tanda. Cuando esos
/// dos literales pasen a llevar `..Default::default()`, estos tres se mudan
/// a `Ajustes` y esto desaparece.
///
/// De fabrica los tres estan **encendidos**, que es como se comporta el
/// movil: quien no los quiera los apaga.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AjustesFinos {
    /// Los cruces entre figuras, y los de una figura consigo misma.
    pub intersecciones: bool,
    /// El canto de las figuras: la escuadra.
    pub bordes: bool,
    /// El `(0,0)` del dibujo y sus dos rectas, si el documento tiene uno.
    ///
    /// Es un punto y no un booleano porque **el origen no siempre es el cero
    /// de la escena**: el movil lo guarda en la clave `origenCoordenadas`,
    /// que aqui viaja intacta dentro de `Lienzo.resto`. Quien la sepa leer
    /// la pasa por aqui; mientras nadie lo haga, no hay eje y no se ofrece
    /// ningun ancla que el usuario no vea dibujada.
    pub origen: Option<Punto2>,
    /// Los puntos de tangencia a las circunferencias desde [`Self::desde`].
    pub tangentes: bool,
    /// De donde sale la raya que se esta trazando, si se esta trazando una:
    /// lo pone el gesto. Sin el no hay tangentes que ofrecer.
    pub desde: Option<Punto2>,
}

impl Default for AjustesFinos {
    fn default() -> Self {
        Self {
            intersecciones: true,
            bordes: true,
            origen: None,
            tangentes: true,
            desde: None,
        }
    }
}

impl AjustesFinos {
    /// Los tres apagados.
    pub const NINGUNO: Self = Self {
        intersecciones: false,
        bordes: false,
        origen: None,
        tangentes: false,
        desde: None,
    };
}

/// Los ajustes que tocan para esta faena, partiendo de los del usuario.
///
/// **Lo que el usuario apaga se queda apagado**, pase lo que pase: la faena
/// puede quitar cosas pero nunca encender lo que se ha desactivado a mano.
/// Al reves seria un ajuste que no se obedece, que es peor que no tenerlo.
pub fn ajustes_para(faena: Faena, config: &Ajustes) -> Ajustes {
    match faena {
        Faena::Trazando | Faena::Afinando => *config,

        // Moviendo se busca un sitio, no se traza: se puede ser mas
        // generoso con el radio sin que estorbe, porque no hay un trazo en
        // curso al que dar un tiron.
        Faena::Moviendo => Ajustes {
            radio_px: config.radio_px * 1.5,
            ..*config
        },

        // A mano alzada, nada. Un trazo que salta a un vertice en mitad del
        // recorrido no se corrige, **se rompe**: el garabato pega un tiron y
        // sigue. Lo unico que el original permite aqui es el canto de una
        // guia -no es un punto al que ir, es una superficie sobre la que
        // resbalar-, y ni el canto ni las guias existen todavia.
        //
        // Esta rama devolviendo siempre nada NO es codigo muerto: es lo que
        // hace imposible que alguien cablee el lapiz a `Trazando` y se coma
        // el tiron. El dia que exista el ancla del canto, se enciende aqui y
        // el lapiz empieza a resbalar sin que nadie recuerde nada.
        Faena::AMano => Ajustes::NINGUNO,
    }
}

/// A que se pega el cursor, o `None` si nada tira de el.
///
/// Es **la unica puerta**. Quien quiera enganchar algo pregunta aqui y no
/// busca anclajes por su cuenta: es lo que hace que anadir una herramienta no
/// obligue a acordarse del iman.
///
/// `excluir` son los elementos que se estan dibujando o moviendo: sus propios
/// puntos no cuentan. Es una rebanada y no un solo id porque al mover una
/// seleccion de varios, excluir uno solo deja que el grupo se enganche a sus
/// propios miembros, que se arrastran con el.
///
/// No monta ninguna lista de anclajes: esto corre en cada movimiento del
/// raton, y el camino caliente tiene presupuesto de cero asignaciones.
pub fn sitio(
    elementos: &[Elemento],
    p: Punto2,
    zoom: f32,
    faena: Faena,
    config: &Ajustes,
    excluir: &[u64],
) -> Option<Anclaje> {
    // **Sin los tres anclajes nuevos, y eso no es un olvido.** Esta funcion
    // es el camino caliente: corre en cada aviso del raton y tiene
    // presupuesto de **cero asignaciones**, que `tests/asignaciones.rs`
    // comprueba. Los cruces y el canto piden el perimetro de las figuras de
    // alrededor, y sacar un perimetro es montar listas de puntos: veintitres
    // reservas por movimiento del raton medidas.
    //
    // Por eso viven detras de [`sitio_fino`], que es lo que hay que llamar
    // cuando el usuario los enciende. Quien los quiera paga lo que cuestan,
    // a sabiendas, y quien no los use no paga nada.
    sitio_fino(
        elementos,
        p,
        zoom,
        faena,
        config,
        &AjustesFinos::NINGUNO,
        excluir,
    )
}

/// Como [`sitio`], pero diciendo ademas que hacer con los tres anclajes de
/// [`AjustesFinos`].
///
/// # El orden en que se mira, que no es arbitrario
///
/// 1. **Las intersecciones primero**, porque son las de mayor prioridad y
///    conviene que entren en la puja cuanto antes.
/// 2. **Los puntos notables de cada figura**, con la criba por caja que hace
///    soportable un plano importado.
/// 3. **El canto, y solo si no gano nada de lo anterior.** No se decide por
///    cercania como todo lo demas, y tiene que ser asi: el canto pasa por
///    encima de los vertices de su propia figura, asi que junto a una esquina
///    siempre hay un punto del borde mas cerca que la esquina. Por distancia,
///    la esquina no ganaria jamas y no habria forma de clavar un trazo en
///    ella. Siendo el ultimo recurso, el canto hace lo que se espera de una
///    escuadra: manda donde no hay nada mejor, que es a lo largo del lado.
pub fn sitio_fino(
    elementos: &[Elemento],
    p: Punto2,
    zoom: f32,
    faena: Faena,
    config: &Ajustes,
    finos: &AjustesFinos,
    excluir: &[u64],
) -> Option<Anclaje> {
    sitio_fino_con(elementos, p, zoom, faena, config, finos, &|id| {
        excluir.contains(&id)
    })
}

/// Como [`sitio`], pero preguntando a `excluye` si un id no cuenta, en vez
/// de recorrer una lista. Es lo que usa el gesto al mover una seleccion:
/// con mil elegidos, `excluir.contains` por cada elemento eran dos millones
/// de comparaciones por aviso del raton, y la seleccion contesta en tiempo
/// constante.
pub fn sitio_con(
    elementos: &[Elemento],
    p: Punto2,
    zoom: f32,
    faena: Faena,
    config: &Ajustes,
    excluye: &dyn Fn(u64) -> bool,
) -> Option<Anclaje> {
    sitio_fino_con(
        elementos,
        p,
        zoom,
        faena,
        config,
        &AjustesFinos::NINGUNO,
        excluye,
    )
}

/// **Lo encontrado, o un punto tecleado o el eje si quedan mas cerca**
/// (`anclajesDeTablas` y `anclajesDelEje` de `Tablas.kt`).
///
/// Los puntos de las tablas de coordenadas valen como una esquina: alguien
/// los tecleo para que estuvieran ahi. Del eje se ofrecen su cero y la
/// proyeccion del cursor sobre sus dos rectas, salvo **cerca del cero, donde
/// manda el cero**: a un paso del origen la proyeccion siempre queda algo mas
/// cerca que el origen mismo (un cateto contra su hipotenusa) y apuntar al
/// cero daria «encima del eje, un poco a la derecha». A mano alzada no tira
/// nada, como lo demas.
pub fn con_coordenadas(
    encontrado: Option<Anclaje>,
    p: Punto2,
    zoom: f32,
    faena: Faena,
    config: &Ajustes,
    origen: Option<Punto2>,
    puntos: &[Punto2],
) -> Option<Anclaje> {
    let ajustes = ajustes_para(faena, config);
    if !ajustes.activo || !ajustes.esquinas || faena == Faena::AMano {
        return encontrado;
    }
    let radio = ajustes.radio_px / zoom.max(0.0001);
    let mut mejor = encontrado;
    let mut mejor_d = encontrado.map_or(f32::MAX, |a| a.punto.distancia(p));
    let mut considerar = |a: Anclaje| {
        let d = a.punto.distancia(p);
        if d > radio {
            return;
        }
        let gana = d < mejor_d - EMPATE
            || ((d - mejor_d).abs() <= EMPATE
                && mejor.is_some_and(|m: Anclaje| prioridad(a.tipo) < prioridad(m.tipo)));
        if gana {
            mejor = Some(a);
            mejor_d = d;
        }
    };
    for &punto in puntos {
        considerar(Anclaje {
            punto,
            tipo: TipoAnclaje::Esquina,
            id: 0,
        });
    }
    if let Some(o) = origen {
        let eje = |punto| Anclaje {
            punto,
            tipo: TipoAnclaje::Eje,
            id: 0,
        };
        considerar(eje(o));
        if o.distancia(p) > radio {
            considerar(eje(Punto2::nuevo(p.x, o.y)));
            considerar(eje(Punto2::nuevo(o.x, p.y)));
        }
    }
    mejor
}

/// Como [`sitio_fino`], preguntando a `excluye` (lo que usa el gesto).
pub fn sitio_fino_con(
    elementos: &[Elemento],
    p: Punto2,
    zoom: f32,
    faena: Faena,
    config: &Ajustes,
    finos: &AjustesFinos,
    excluye: &dyn Fn(u64) -> bool,
) -> Option<Anclaje> {
    let ajustes = ajustes_para(faena, config);
    if !ajustes.activo {
        return None;
    }
    // El radio se da en pixeles de pantalla y aqui se trabaja en escena.
    let radio = ajustes.radio_px / zoom.max(0.0001);
    // A mano alzada no engancha nada, y eso incluye a los tres de aqui: un
    // trazo que salta a un cruce en mitad del recorrido se rompe igual que
    // uno que salta a un vertice.
    //
    // El canto es la excepcion que el original permite —no es un punto al
    // que ir, es una superficie sobre la que resbalar— **pero alla solo lo
    // ofrecen las guias**, y una guia se reconoce por el campo `reference`
    // del elemento, que aqui todavia no se lee. Encenderlo para todas las
    // figuras convertiria cada rectangulo del dibujo en un carril del que no
    // hay forma de despegar el lapiz. Asi que sigue apagado y este `if` es
    // donde se enciende el dia que exista la guia.
    let finos = if faena == Faena::AMano {
        AjustesFinos::NINGUNO
    } else {
        *finos
    };

    let mut mejor: Option<Anclaje> = None;
    let mut mejor_d = f32::MAX;

    let considerar = |a: Anclaje, mejor: &mut Option<Anclaje>, mejor_d: &mut f32| {
        let d = a.punto.distancia(p);
        if d > radio {
            return;
        }
        let gana = d < *mejor_d - EMPATE
            || ((d - *mejor_d).abs() <= EMPATE
                && mejor.is_some_and(|m: Anclaje| prioridad(a.tipo) < prioridad(m.tipo)));
        if gana {
            *mejor = Some(a);
            *mejor_d = d;
        }
    };

    // 1. Los cruces. El descarte por cercania vive dentro de
    //    `intersecciones_cerca` porque es parte de su algoritmo: sin el,
    //    cruzar dos curvas muestreadas seria cuadratico en cada fotograma.
    if finos.intersecciones && ajustes.esquinas {
        // Se piden los cruces de todos contra todos menos los excluidos.
        //
        // Hay que copiar porque `intersecciones_cerca` recibe una rebanada y
        // solo sabe excluir a uno, y aqui se excluye a la seleccion entera.
        // La criba por caja **antes** de copiar es lo que lo hace asumible:
        // sin ella, un plano importado se duplicaria en memoria en cada
        // movimiento del raton; con ella se copian los dos o tres elementos
        // que de verdad pasan cerca del cursor.
        let visibles: Vec<Elemento> = elementos
            .iter()
            .filter(|e| !excluye(e.id) && !e.borrado)
            .filter(|e| {
                let (x0, y0, x1, y1) = e.caja();
                p.x >= x0 - radio && p.x <= x1 + radio && p.y >= y0 - radio && p.y <= y1 + radio
            })
            .cloned()
            .collect();
        for (punto, id) in crate::perimetros::intersecciones_cerca(&visibles, p, radio, None, true)
        {
            considerar(
                Anclaje {
                    punto,
                    tipo: TipoAnclaje::Interseccion,
                    id,
                },
                &mut mejor,
                &mut mejor_d,
            );
        }
    }

    // Las tangentes desde donde salio la raya, a las circunferencias que
    // pasan cerca del cursor (y, en un arco, solo si caen en su tramo).
    if let Some(desde) = finos.desde.filter(|_| finos.tangentes) {
        for e in elementos {
            if e.borrado || excluye(e.id) || e.extras.marca_angulo.is_some() {
                continue;
            }
            let Some((c, r)) = crate::geometria_exacta::centro_y_radio(e) else {
                continue;
            };
            if (p.distancia(c) - r).abs() > radio {
                continue;
            }
            let Some(ts) = crate::geometria_exacta::tangentes_desde(desde, c, r) else {
                continue;
            };
            let (inicio, barrido) =
                crate::geometria_exacta::tramo_de(e).unwrap_or((0.0, std::f32::consts::TAU));
            for t in ts {
                let a = crate::geometria_exacta::angulo_de(c, t) - e.angulo;
                if crate::geometria_exacta::dentro_del_barrido(a, inicio, barrido) {
                    considerar(
                        Anclaje {
                            punto: t,
                            tipo: TipoAnclaje::Tangente,
                            id: e.id,
                        },
                        &mut mejor,
                        &mut mejor_d,
                    );
                }
            }
        }
    }

    // El eje: su origen, y los dos pies sobre sus rectas. El id es cero
    // porque no pertenece a ningun elemento, que es justamente lo que lo
    // hace util.
    if let Some(o) = finos.origen.filter(|_| ajustes.esquinas) {
        for punto in [o, Punto2::nuevo(p.x, o.y), Punto2::nuevo(o.x, p.y)] {
            considerar(
                Anclaje {
                    punto,
                    tipo: TipoAnclaje::Eje,
                    id: 0,
                },
                &mut mejor,
                &mut mejor_d,
            );
        }
    }

    for e in elementos {
        if e.borrado || excluye(e.id) {
            continue;
        }
        let (x0, y0, x1, y1) = e.caja();
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);

        // El angulo se deshace sobre el punto, no sobre la figura -el mismo
        // truco que `impacto::toca`-. Asi la caja local vale de criba gire o
        // no el elemento, y no hace falta calcular ninguna caja girada: solo
        // el ancla que gane vuelve al mundo. Girar conserva distancias, asi
        // que comparar en local es identico a comparar en el mundo.
        let local = if e.angulo != 0.0 {
            p.girar(centro, -e.angulo)
        } else {
            p
        };

        // Todos los anclajes caen dentro de la caja: si el cursor esta a mas
        // de un radio de ella, no hay nada que mirar. La resta es lo que hace
        // soportable un plano importado -sin ella, cada movimiento montaria
        // los anclajes de miles de elementos para tirarlos-.
        if local.x < x0 - radio
            || local.x > x1 + radio
            || local.y < y0 - radio
            || local.y > y1 + radio
        {
            continue;
        }

        por_cada_anclaje(e, &ajustes, |punto_local, tipo| {
            let d = punto_local.distancia(local);
            if d > radio {
                return;
            }
            let gana = d < mejor_d - EMPATE
                || ((d - mejor_d).abs() <= EMPATE
                    && mejor.is_some_and(|m| prioridad(tipo) < prioridad(m.tipo)));
            if gana {
                let punto = if e.angulo != 0.0 {
                    punto_local.girar(centro, e.angulo)
                } else {
                    punto_local
                };
                mejor = Some(Anclaje {
                    punto,
                    tipo,
                    id: e.id,
                });
                mejor_d = d;
            }
        });
    }

    // 3. El canto, y **solo si no habia nada notable a mano**. Ver la nota de
    //    la cabecera de esta funcion sobre por que no puja por cercania.
    if mejor.is_none() && finos.bordes {
        for e in elementos {
            if e.borrado || e.bloqueado || excluye(e.id) {
                continue;
            }
            // El marco se queda fuera: delimita hasta donde llega el dibujo,
            // no es algo dibujado, y pegarse a su canto convertiria los
            // cuatro bordes del papel en carriles.
            if matches!(e.figura, Figura::Marco { .. }) {
                continue;
            }
            // La caja primero, que es una resta: sacarle el perimetro a una
            // figura cuesta, y esto corre por CADA elemento en CADA
            // movimiento del raton. Con un plano importado —miles de
            // figuras— saltarse esta criba era recorrer y reservar el plano
            // entero en cada evento.
            let (x0, y0, x1, y1) = e.caja();
            if p.x < x0 - radio || p.x > x1 + radio || p.y < y0 - radio || p.y > y1 + radio {
                continue;
            }
            if let Some(q) = crate::perimetros::punto_en_el_perimetro(e, p, radio) {
                considerar(
                    Anclaje {
                        punto: q,
                        tipo: TipoAnclaje::Borde,
                        id: e.id,
                    },
                    &mut mejor,
                    &mut mejor_d,
                );
            }
        }
    }

    mejor
}

/// Los puntos notables de `e`, **en su marco local**, uno a uno.
///
/// Entrega a un sumidero en vez de devolver un `Vec` porque corre por cada
/// elemento y cada movimiento del raton: montar una lista para tirarla es
/// justo lo que el presupuesto de cero asignaciones prohibe.
fn por_cada_anclaje(e: &Elemento, ajustes: &Ajustes, mut f: impl FnMut(Punto2, TipoAnclaje)) {
    match &e.figura {
        // Las figuras con puntos dan sus extremos y su medio; **su caja no
        // significa nada**: la caja de una diagonal tiene dos esquinas por
        // las que la linea no pasa.
        Figura::Lapiz { puntos, .. }
        | Figura::Resaltador { puntos }
        | Figura::Linea { puntos }
        | Figura::Flecha { puntos, .. }
        | Figura::Cota { puntos } => {
            if puntos.len() < 2 {
                return;
            }
            if ajustes.esquinas {
                f(puntos[0], TipoAnclaje::Extremo);
                f(puntos[puntos.len() - 1], TipoAnclaje::Extremo);
            }
            if ajustes.medios {
                // Con dos puntos -que es como nacen Linea y Flecha, y como
                // se quedan- `puntos[len/2]` seria `puntos[1]`, o sea el
                // extremo otra vez: el ancla mas util faltaria y ademas se
                // colaria un extremo disfrazado de `Medio` a quien haya
                // apagado «Esquinas y extremos». Con dos, el medio es el
                // medio geometrico; con mas, el punto de en medio de la
                // lista.
                let medio = if puntos.len() == 2 {
                    Punto2::nuevo(
                        (puntos[0].x + puntos[1].x) / 2.0,
                        (puntos[0].y + puntos[1].y) / 2.0,
                    )
                } else {
                    puntos[puntos.len() / 2]
                };
                // Una polilinea de tres puntos con el primero repetido
                // ofreceria su propio extremo como medio. No se emite.
                if medio != puntos[0] && medio != puntos[puntos.len() - 1] {
                    f(medio, TipoAnclaje::Medio);
                }
            }
        }

        // **El arco**: sus dos puntas, su punto de en medio y el centro de su
        // circunferencia. Las esquinas de su caja no estan en ningun sitio
        // del dibujo (y la caja es la del trozo, ni siquiera la del ovalo).
        Figura::Arco {
            inicio,
            barrido: Some(barrido),
        } => {
            let (rx, ry) = (e.ancho / 2.0, e.alto / 2.0);
            let (cx, cy) = (e.x + rx, e.y + ry);
            let en = |t: f32| Punto2::nuevo(cx + rx * t.cos(), cy + ry * t.sin());
            let marca = e.extras.marca_angulo.is_some();
            if ajustes.esquinas && !marca {
                f(en(*inicio), TipoAnclaje::Extremo);
                f(en(inicio + barrido), TipoAnclaje::Extremo);
            }
            if ajustes.medios && !marca {
                f(en(inicio + barrido / 2.0), TipoAnclaje::Medio);
            }
            if ajustes.centros {
                f(Punto2::nuevo(cx, cy), TipoAnclaje::Centro);
            }
        }

        _ => {
            let (x0, y0, x1, y1) = e.caja();
            let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            if ajustes.esquinas {
                f(Punto2::nuevo(x0, y0), TipoAnclaje::Esquina);
                f(Punto2::nuevo(x1, y0), TipoAnclaje::Esquina);
                f(Punto2::nuevo(x1, y1), TipoAnclaje::Esquina);
                f(Punto2::nuevo(x0, y1), TipoAnclaje::Esquina);
            }
            if ajustes.medios {
                f(Punto2::nuevo(cx, y0), TipoAnclaje::Medio);
                f(Punto2::nuevo(x1, cy), TipoAnclaje::Medio);
                f(Punto2::nuevo(cx, y1), TipoAnclaje::Medio);
                f(Punto2::nuevo(x0, cy), TipoAnclaje::Medio);
            }
            if ajustes.centros {
                f(Punto2::nuevo(cx, cy), TipoAnclaje::Centro);
            }
        }
    }
}

/// Lado de la marca del iman, **en pixeles de pantalla**.
pub const LADO_PISTA_PX: f32 = 9.0;

/// Grosor de la marca, tambien en pixeles de pantalla.
const GROSOR_PISTA_PX: f32 = 1.5;

/// Cuantos lados aproximan el circulo del centro.
const LADOS_DEL_CIRCULO: usize = 12;

/// La marca que se pinta donde el iman ha agarrado.
///
/// Sin nada visible el iman es magia: el trazo se va dos pixeles y no hay
/// forma de distinguir un enganche de un fallo de punteria. Y la marca
/// cambia con el tipo para poder distinguir «me pego al centro» de «me pego
/// a la esquina», que es la queja tipica cuando hace algo que no esperabas.
///
/// El tamano va en pixeles de pantalla -dividido por el zoom- para que la
/// marca no crezca al acercarse, igual que el radio de captura y por el
/// mismo motivo.
pub fn pista(a: &Anclaje, zoom: f32) -> Orden {
    let z = zoom.max(0.0001);
    let r = LADO_PISTA_PX / z / 2.0;
    let (cx, cy) = (a.punto.x, a.punto.y);

    let puntos = match a.tipo {
        // El vertice: un cuadrado. Es la marca de «aqui hay una punta».
        TipoAnclaje::Esquina | TipoAnclaje::Extremo => vec![
            Punto2::nuevo(cx - r, cy - r),
            Punto2::nuevo(cx + r, cy - r),
            Punto2::nuevo(cx + r, cy + r),
            Punto2::nuevo(cx - r, cy + r),
            Punto2::nuevo(cx - r, cy - r),
        ],
        // El medio: un triangulo, que es como se marca un punto medio en un
        // plano de toda la vida.
        TipoAnclaje::Medio => vec![
            Punto2::nuevo(cx, cy - r),
            Punto2::nuevo(cx + r, cy + r),
            Punto2::nuevo(cx - r, cy + r),
            Punto2::nuevo(cx, cy - r),
        ],
        // La interseccion: **un aspa**, que es como se marca un cruce en un
        // plano de toda la vida. Tiene que distinguirse del cuadrado del
        // vertice sin mirar dos veces, porque las dos marcas aparecen a un
        // par de pixeles una de otra cuando una raya toca una esquina.
        //
        // Se pinta de una tirada volviendo por el centro: una polilinea no
        // puede levantar el lapiz, y dos ordenes por marca obligarian a
        // cambiar el tipo que devuelve esta funcion.
        TipoAnclaje::Interseccion => vec![
            Punto2::nuevo(cx - r, cy - r),
            Punto2::nuevo(cx + r, cy + r),
            Punto2::nuevo(cx, cy),
            Punto2::nuevo(cx + r, cy - r),
            Punto2::nuevo(cx - r, cy + r),
            Punto2::nuevo(cx, cy),
            Punto2::nuevo(cx - r, cy - r),
        ],
        // El eje: una cruz recta, la del origen de coordenadas de cualquier
        // grafica. Mismo truco de volver por el centro.
        TipoAnclaje::Eje => vec![
            Punto2::nuevo(cx - r, cy),
            Punto2::nuevo(cx + r, cy),
            Punto2::nuevo(cx, cy),
            Punto2::nuevo(cx, cy - r),
            Punto2::nuevo(cx, cy + r),
            Punto2::nuevo(cx, cy),
            Punto2::nuevo(cx - r, cy),
        ],
        // El canto: un rombo, la marca mas discreta de las cinco. Y tiene
        // que serlo: el canto engancha a lo largo de todo un lado, asi que
        // su marca aparece constantemente mientras se bordea una figura, y
        // una marca llamativa ahi parpadearia todo el rato.
        TipoAnclaje::Borde => vec![
            Punto2::nuevo(cx, cy - r),
            Punto2::nuevo(cx + r, cy),
            Punto2::nuevo(cx, cy + r),
            Punto2::nuevo(cx - r, cy),
            Punto2::nuevo(cx, cy - r),
        ],
        // La tangente: el circulito con la raya que lo roza por arriba.
        TipoAnclaje::Tangente => {
            let mut p = Vec::with_capacity(LADOS_DEL_CIRCULO + 4);
            for i in 0..=LADOS_DEL_CIRCULO {
                let t = std::f32::consts::FRAC_PI_2
                    + i as f32 / LADOS_DEL_CIRCULO as f32 * std::f32::consts::TAU;
                p.push(Punto2::nuevo(
                    cx + r * 0.7 * t.cos(),
                    cy + r * 0.7 * t.sin(),
                ));
            }
            p.push(Punto2::nuevo(cx - r * 1.3, cy + r * 0.7));
            p.push(Punto2::nuevo(cx + r * 1.3, cy + r * 0.7));
            p
        }
        // El centro: un circulo, porque de lo que suele ser centro -una
        // elipse- es justo el centro de su circunferencia.
        TipoAnclaje::Centro => {
            let mut p = Vec::with_capacity(LADOS_DEL_CIRCULO + 1);
            for i in 0..LADOS_DEL_CIRCULO {
                let t = i as f32 / LADOS_DEL_CIRCULO as f32 * std::f32::consts::TAU;
                p.push(Punto2::nuevo(cx + r * t.cos(), cy + r * t.sin()));
            }
            // Repite el primer punto en vez de calcular en TAU: sin(TAU) != 0
            // en f32, asi que con ancla en origen el circulo no cierra de
            // verdad. Literalmente el mismo punto garantiza cierre exacto.
            if let Some(primero) = p.first() {
                p.push(*primero);
            }
            p
        }
    };

    Orden::Polilinea {
        puntos,
        color: color_pista(),
        grosor: GROSOR_PISTA_PX / z,
        estilo: EstiloTrazo::Solido,
    }
}

/// El azul de lo que el editor senala: el iman es una ayuda, no tinta del
/// dibujo, asi que no se pinta con el color del trazo.
///
/// Reutiliza COLOR_SELECCION del marco de seleccion: un color unico para la
/// interfaz hace que sea coherente. Cambiar COLOR_SELECCION en pintado.rs
/// afecta tanto al marco como a esta marca.
fn color_pista() -> ColorRgba {
    crate::pintado::COLOR_SELECCION
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo};
    use crate::pintado::Orden;

    /// Un rectangulo de 100x50 en (0,0), sin girar.
    fn rectangulo(id: u64) -> Elemento {
        Elemento {
            figura: Figura::Rectangulo,
            id,
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

    /// Una linea de (0,0) a (100,0) con **dos** puntos: la forma que de
    /// verdad produce la aplicacion (`gesto::nuevo_elemento` arranca Linea
    /// y Flecha con dos puntos, y asi se quedan).
    fn linea_de_dos(id: u64) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)],
            },
            ..rectangulo(id)
        }
    }

    /// Una linea de (0,0) a (100,0), con un punto intermedio en (50,0).
    fn linea(id: u64) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![
                    Punto2::nuevo(0.0, 0.0),
                    Punto2::nuevo(50.0, 0.0),
                    Punto2::nuevo(100.0, 0.0),
                ],
            },
            ..rectangulo(id)
        }
    }

    fn todo() -> Ajustes {
        Ajustes::default()
    }

    #[test]
    fn engancha_a_la_esquina_de_un_rectangulo() {
        let es = [rectangulo(1)];
        // A 3 de escena de la esquina (0,0), con radio 14 px a zoom 1.
        let a = sitio(
            &es,
            Punto2::nuevo(3.0, 3.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(a.tipo, TipoAnclaje::Esquina);
        assert_eq!(a.punto, Punto2::nuevo(0.0, 0.0));
        assert_eq!(a.id, 1);
    }

    #[test]
    fn engancha_al_medio_de_un_lado() {
        let es = [rectangulo(1)];
        // El medio del lado de arriba es (50,0). El centro es (50,25),
        // que esta a 25 y queda fuera del radio.
        let a = sitio(
            &es,
            Punto2::nuevo(50.0, 2.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(a.tipo, TipoAnclaje::Medio);
        assert_eq!(a.punto, Punto2::nuevo(50.0, 0.0));
    }

    #[test]
    fn engancha_al_centro() {
        let es = [rectangulo(1)];
        let a = sitio(
            &es,
            Punto2::nuevo(51.0, 26.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(a.tipo, TipoAnclaje::Centro);
        assert_eq!(a.punto, Punto2::nuevo(50.0, 25.0));
    }

    #[test]
    fn una_linea_da_extremos_y_medio_pero_no_esquinas_de_su_caja() {
        // La trampa 2 del diseno: `caja()` de una linea es la caja de sus
        // puntos, y sus esquinas son de un rectangulo por el que la linea
        // no pasa. Aqui la caja es (0,-1)-(100,1) por el grosor, asi que
        // su esquina superior izquierda seria (0,-1): no debe existir.
        let es = [linea(1)];
        let a = sitio(
            &es,
            Punto2::nuevo(0.5, -1.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar al extremo");
        assert_eq!(a.tipo, TipoAnclaje::Extremo);
        assert_eq!(a.punto, Punto2::nuevo(0.0, 0.0));

        // Y el medio es el punto de en medio de la LISTA, no el centro de
        // la caja.
        let m = sitio(
            &es,
            Punto2::nuevo(50.0, 1.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar al medio");
        assert_eq!(m.tipo, TipoAnclaje::Medio);
        assert_eq!(m.punto, Punto2::nuevo(50.0, 0.0));
    }

    #[test]
    fn una_linea_de_dos_puntos_da_su_medio_geometrico() {
        // La forma que produce la aplicacion. Con `puntos[len/2]` el "medio"
        // seria `puntos[1]`, o sea el extremo (100,0): el medio de verdad
        // -(50,0)- no existiria y el extremo se colaria etiquetado `Medio`.
        let es = [linea_de_dos(1)];
        let m = sitio(
            &es,
            Punto2::nuevo(50.0, 2.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("el medio de un segmento recto es el ancla mas util que hay");
        assert_eq!(m.tipo, TipoAnclaje::Medio);
        assert_eq!(m.punto, Punto2::nuevo(50.0, 0.0));
    }

    #[test]
    fn apagar_las_esquinas_apaga_los_extremos_de_una_linea_de_dos_puntos() {
        // La fuga: quien apaga «Esquinas y extremos» y deja «Puntos medios»
        // seguia teniendo los extremos de toda linea y flecha agarrandole el
        // cursor, etiquetados `Medio`. Lo que el usuario apaga se queda
        // apagado.
        let es = [linea_de_dos(1)];
        let solo_medios = Ajustes {
            esquinas: false,
            centros: false,
            ..Ajustes::default()
        };
        // Se pide sin canto: esta prueba es sobre el interruptor de las
        // esquinas, y el canto de la propia raya pasa justo por su extremo,
        // asi que con el encendido engancharia ahi por otro motivo y la
        // prueba no comprobaria lo que dice comprobar.
        assert!(
            sitio_fino(
                &es,
                Punto2::nuevo(98.0, 0.0),
                1.0,
                Faena::Trazando,
                &solo_medios,
                &AjustesFinos::NINGUNO,
                &[]
            )
            .is_none(),
            "el extremo sigue enganchando con las esquinas apagadas"
        );
    }

    #[test]
    fn un_rectangulo_girado_engancha_por_su_esquina_de_verdad() {
        // La trampa 1 del diseno. Girado 90 grados sobre su centro
        // (50,25), la esquina (0,0) acaba en (75,-25) -fuera de la caja
        // local, que sigue siendo (0,0)-(100,50)-. Si la criba usara la
        // caja sin girar, este enganche no ocurriria.
        let mut e = rectangulo(1);
        e.angulo = std::f32::consts::FRAC_PI_2;
        let es = [e];
        let a = sitio(
            &es,
            Punto2::nuevo(76.0, -24.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("una esquina girada tambien engancha");
        assert_eq!(a.tipo, TipoAnclaje::Esquina);
        assert!(
            a.punto.distancia(Punto2::nuevo(75.0, -25.0)) < 0.01,
            "el ancla vuelve girada al mundo, no se queda en local: {:?}",
            a.punto
        );
    }

    #[test]
    fn el_elemento_excluido_no_se_engancha_a_si_mismo() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(3.0, 3.0),
                1.0,
                Faena::Trazando,
                &todo(),
                &[1]
            )
            .is_none(),
            "una figura naciendo se pegaria a su propia esquina"
        );
    }

    #[test]
    fn se_excluye_una_seleccion_entera_no_solo_uno() {
        let es = [rectangulo(1), rectangulo(2)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(3.0, 3.0),
                1.0,
                Faena::Moviendo,
                &todo(),
                &[1, 2]
            )
            .is_none(),
            "un grupo que se mueve no puede pegarse a sus propios miembros"
        );
    }

    #[test]
    fn un_elemento_borrado_no_ofrece_anclajes() {
        let mut e = rectangulo(1);
        e.borrado = true;
        assert!(
            sitio(
                &[e],
                Punto2::nuevo(3.0, 3.0),
                1.0,
                Faena::Trazando,
                &todo(),
                &[]
            )
            .is_none()
        );
    }

    #[test]
    fn fuera_del_radio_no_engancha_nada() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(-20.0, -20.0),
                1.0,
                Faena::Trazando,
                &todo(),
                &[]
            )
            .is_none()
        );
    }

    #[test]
    fn el_radio_es_de_pantalla_asi_que_el_zoom_lo_encoge() {
        let es = [rectangulo(1)];
        // Sobre el borde de arriba, a diez de la esquina. Se pide sin canto:
        // el punto esta EN la pared, asi que la escuadra engancharia siempre
        // y taparia lo que aqui se mide, que es como encoge el radio de un
        // ancla de punto notable al acercarse.
        let p = Punto2::nuevo(10.0, 0.0);
        let pedir = |zoom: f32| {
            sitio_fino(
                &es,
                p,
                zoom,
                Faena::Trazando,
                &todo(),
                &AjustesFinos::NINGUNO,
                &[],
            )
        };
        // A zoom 1 el radio de escena son 14: 10 entra.
        assert!(pedir(1.0).is_some());
        // A zoom 4 son 3,5: 10 ya no entra, a la MISMA distancia de escena.
        assert!(pedir(4.0).is_none());
    }

    #[test]
    fn a_mano_alzada_no_engancha_nunca() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(0.0, 0.0),
                1.0,
                Faena::AMano,
                &todo(),
                &[]
            )
            .is_none(),
            "un trazo que salta a un vertice no se corrige, se rompe"
        );
    }

    #[test]
    fn lo_que_el_usuario_apaga_sigue_apagado_en_todas_las_faenas() {
        let es = [rectangulo(1)];
        let sin_esquinas = Ajustes {
            esquinas: false,
            ..Ajustes::default()
        };
        for faena in [Faena::Trazando, Faena::Afinando, Faena::Moviendo] {
            let a = sitio(&es, Punto2::nuevo(1.0, 1.0), 1.0, faena, &sin_esquinas, &[]);
            assert!(
                a.is_none_or(|a| a.tipo != TipoAnclaje::Esquina),
                "la faena {faena:?} encendio una esquina que el usuario apago"
            );
        }
    }

    #[test]
    fn apagar_el_iman_entero_lo_deja_libre() {
        let es = [rectangulo(1)];
        assert!(
            sitio(
                &es,
                Punto2::nuevo(0.0, 0.0),
                1.0,
                Faena::Trazando,
                &Ajustes::NINGUNO,
                &[]
            )
            .is_none()
        );
    }

    #[test]
    fn en_empate_de_distancia_gana_la_esquina_al_centro() {
        // Un cuadrado de 20x20: la esquina (0,0) y el centro (10,10)
        // estan los dos a la misma distancia del punto (5,5).
        let mut e = rectangulo(1);
        e.ancho = 20.0;
        e.alto = 20.0;
        let a = sitio(
            &[e],
            Punto2::nuevo(5.0, 5.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        )
        .expect("tenia que enganchar");
        assert_eq!(
            a.tipo,
            TipoAnclaje::Esquina,
            "el centro desconcierta; el vertice es lo intencionado"
        );
    }

    #[test]
    fn moviendo_engancha_mas_lejos_que_trazando() {
        let es = [rectangulo(1)];
        // 18 de escena: fuera del radio 14 de `Trazando`, dentro del 21
        // (14 x 1,5) de `Moviendo`.
        let p = Punto2::nuevo(-18.0, 0.0);
        assert!(sitio(&es, p, 1.0, Faena::Trazando, &todo(), &[]).is_none());
        assert!(sitio(&es, p, 1.0, Faena::Moviendo, &todo(), &[]).is_some());
    }

    fn puntos_de(o: &Orden) -> Vec<Punto2> {
        match o {
            Orden::Polilinea { puntos, .. } => puntos.clone(),
            otro => panic!("la pista tiene que ser una polilinea, no {otro:?}"),
        }
    }

    fn ancla(tipo: TipoAnclaje) -> Anclaje {
        Anclaje {
            punto: Punto2::nuevo(100.0, 200.0),
            tipo,
            id: 1,
        }
    }

    #[test]
    fn cada_tipo_de_ancla_tiene_su_forma() {
        // Cerradas: el ultimo punto repite el primero, asi que un cuadrado
        // son 5 puntos y un triangulo 4.
        let cuadrado = puntos_de(&pista(&ancla(TipoAnclaje::Esquina), 1.0));
        let triangulo = puntos_de(&pista(&ancla(TipoAnclaje::Medio), 1.0));
        let circulo = puntos_de(&pista(&ancla(TipoAnclaje::Centro), 1.0));
        assert_eq!(cuadrado.len(), 5);
        assert_eq!(triangulo.len(), 4);
        assert!(circulo.len() > 8, "el circulo se aproxima con varios lados");
        // Y el extremo se pinta igual que la esquina: para quien dibuja son
        // la misma idea, igual que comparten interruptor.
        assert_eq!(
            puntos_de(&pista(&ancla(TipoAnclaje::Extremo), 1.0)).len(),
            5
        );
    }

    #[test]
    fn la_pista_se_cierra() {
        for tipo in [
            TipoAnclaje::Esquina,
            TipoAnclaje::Extremo,
            TipoAnclaje::Medio,
            TipoAnclaje::Centro,
        ] {
            let p = puntos_de(&pista(&ancla(tipo), 1.0));
            assert_eq!(p[0], p[p.len() - 1], "la marca de {tipo:?} queda abierta");
        }
    }

    #[test]
    fn la_pista_rodea_el_punto_del_ancla() {
        let a = ancla(TipoAnclaje::Esquina);
        let p = puntos_de(&pista(&a, 1.0));
        // Caja minima/maxima y no promedio de puntos: la polilinea repite su
        // primer punto para cerrar, asi que el promedio sale desplazado
        // hacia esa esquina y la prueba pasaria por poco por un motivo que
        // no tiene nada que ver con estar centrada.
        let cx = (p.iter().map(|q| q.x).fold(f32::MAX, f32::min)
            + p.iter().map(|q| q.x).fold(f32::MIN, f32::max))
            / 2.0;
        let cy = (p.iter().map(|q| q.y).fold(f32::MAX, f32::min)
            + p.iter().map(|q| q.y).fold(f32::MIN, f32::max))
            / 2.0;
        assert!(
            (cx - a.punto.x).abs() < 0.01 && (cy - a.punto.y).abs() < 0.01,
            "la marca tiene que salir centrada en el ancla, no al lado"
        );
    }

    #[test]
    fn la_pista_no_crece_con_el_zoom() {
        // Va en pixeles de pantalla, igual que el radio y por lo mismo: a
        // doble zoom mide la mitad de escena, o sea lo mismo en pantalla.
        let ancho = |zoom: f32| {
            let p = puntos_de(&pista(&ancla(TipoAnclaje::Esquina), zoom));
            let x0 = p.iter().map(|q| q.x).fold(f32::MAX, f32::min);
            let x1 = p.iter().map(|q| q.x).fold(f32::MIN, f32::max);
            x1 - x0
        };
        let grosor_marca = |zoom: f32| match pista(&ancla(TipoAnclaje::Esquina), zoom) {
            Orden::Polilinea { grosor, .. } => grosor,
            _ => panic!("tiene que ser polilinea"),
        };
        assert!((ancho(1.0) - LADO_PISTA_PX).abs() < 0.01);
        assert!((ancho(2.0) - LADO_PISTA_PX / 2.0).abs() < 0.01);
        // El grosor tambien se divide por zoom: sin esto, al acercarse la
        // marca engordaria como si fuese un elemento dibujado.
        assert!((grosor_marca(1.0) - GROSOR_PISTA_PX).abs() < 0.01);
        assert!((grosor_marca(2.0) - GROSOR_PISTA_PX / 2.0).abs() < 0.01);
    }

    #[test]
    fn el_circulo_cierra_incluso_con_ancla_en_origen() {
        // Latente: con ancla en (100, 200) el circulo cierra por suerte
        // (el residuo de sin(TAU) es menor que el ULP de 200). Con ancla en
        // origen falla si calculas el ultimo punto en TAU.
        let a = Anclaje {
            punto: Punto2::nuevo(0.0, 0.0),
            tipo: TipoAnclaje::Centro,
            id: 1,
        };
        let p = puntos_de(&pista(&a, 1.0));
        assert_eq!(p[0], p[p.len() - 1], "el circulo debe cerrar exacto");
    }

    // --- Los tres anclajes nuevos ---

    /// Pide sitio **con los tres anclajes nuevos encendidos**.
    ///
    /// Es `sitio_fino` y no `sitio` a proposito: `sitio` es el camino
    /// caliente y los deja fuera por presupuesto de asignaciones. Que estas
    /// pruebas tengan que pedirlos expresamente es justo lo que documenta
    /// esa frontera.
    fn finos(es: &[Elemento], p: Punto2, faena: Faena) -> Option<Anclaje> {
        sitio_fino(es, p, 1.0, faena, &todo(), &AjustesFinos::default(), &[])
    }

    #[test]
    fn el_camino_caliente_no_trae_los_anclajes_que_cuestan_memoria() {
        // El contrato con `tests/asignaciones.rs`, dicho aqui tambien: si
        // alguien enciende los cruces dentro de `sitio`, esta prueba lo
        // caza antes de que lo cace el presupuesto.
        let es = [horizontal(1, 0.0), vertical(2, 0.0)];
        let a = sitio(
            &es,
            Punto2::nuevo(3.0, 3.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &[],
        );
        assert!(a.is_none_or(|a| a.tipo != TipoAnclaje::Interseccion));
    }

    /// Una raya de (x0,y) a (x1,y).
    fn horizontal(id: u64, y: f32) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(-500.0, y), Punto2::nuevo(500.0, y)],
            },
            x: -500.0,
            y,
            ancho: 1000.0,
            alto: 0.0,
            ..rectangulo(id)
        }
    }

    fn vertical(id: u64, x: f32) -> Elemento {
        Elemento {
            figura: Figura::Linea {
                puntos: vec![Punto2::nuevo(x, -500.0), Punto2::nuevo(x, 500.0)],
            },
            x,
            y: -500.0,
            ancho: 0.0,
            alto: 1000.0,
            ..rectangulo(id)
        }
    }

    #[test]
    fn el_cruce_de_dos_rayas_engancha_y_gana_a_todo_lo_demas() {
        // Es el ancla de mayor prioridad del movil, y por el mejor motivo:
        // el punto que buscas no existe como vertice de nada, asi que a
        // pulso no hay forma de acertarlo.
        let es = [horizontal(1, 0.0), vertical(2, 0.0)];
        let a = finos(&es, Punto2::nuevo(3.0, 3.0), Faena::Trazando)
            .expect("tenia que enganchar al cruce");
        assert_eq!(a.tipo, TipoAnclaje::Interseccion);
        assert!(a.punto.distancia(Punto2::nuevo(0.0, 0.0)) < 0.01);
    }

    #[test]
    fn el_cruce_gana_al_medio_de_la_raya_aunque_esten_a_la_misma_distancia() {
        // El medio de cada raya esta en (0,0) tambien —son simetricas—, asi
        // que este es el empate exacto en el que manda la prioridad.
        let es = [horizontal(1, 0.0), vertical(2, 0.0)];
        let a = finos(&es, Punto2::nuevo(0.0, 0.0), Faena::Trazando).unwrap();
        assert_eq!(a.tipo, TipoAnclaje::Interseccion);
    }

    #[test]
    fn apagar_las_intersecciones_devuelve_el_ancla_de_antes() {
        // Caso negativo: lo que el usuario apaga se queda apagado, tambien
        // aqui.
        let es = [horizontal(1, 0.0), vertical(2, 0.0)];
        let sin_cruces = AjustesFinos {
            intersecciones: false,
            ..Default::default()
        };
        let a = sitio_fino(
            &es,
            Punto2::nuevo(0.0, 0.0),
            1.0,
            Faena::Trazando,
            &todo(),
            &sin_cruces,
            &[],
        )
        .unwrap();
        assert_ne!(a.tipo, TipoAnclaje::Interseccion);
    }

    #[test]
    fn el_canto_engancha_a_media_pared_pero_nunca_le_gana_a_una_esquina() {
        // Las dos mitades de la razon de ser del canto, en una prueba: sin
        // el, entre esquina y esquina no hay a que pegarse; y si pujara por
        // cercania, junto a una esquina siempre hay un punto del borde mas
        // cerca que la esquina y no habria forma de clavar un trazo en ella.
        let es = [rectangulo(1)];
        // A (25,3): la esquina (0,0) y el medio del lado (50,0) quedan los
        // dos a mas de veinticinco, o sea fuera del radio. Sin canto, ahi no
        // hay nada a lo que pegarse, que es justo el hueco que viene a tapar.
        let en_medio = finos(&es, Punto2::nuevo(25.0, 3.0), Faena::Trazando)
            .expect("en medio del lado de arriba tiene que haber canto");
        assert_eq!(en_medio.tipo, TipoAnclaje::Borde);
        assert_eq!(en_medio.punto, Punto2::nuevo(25.0, 0.0));

        let en_la_esquina = finos(&es, Punto2::nuevo(2.0, 2.0), Faena::Trazando).unwrap();
        assert_eq!(
            en_la_esquina.tipo,
            TipoAnclaje::Esquina,
            "el canto le robo la esquina"
        );
    }

    #[test]
    fn apagar_el_canto_deja_el_lado_libre_otra_vez() {
        let es = [rectangulo(1)];
        assert!(
            sitio_fino(
                &es,
                Punto2::nuevo(25.0, 3.0),
                1.0,
                Faena::Trazando,
                &todo(),
                &AjustesFinos::NINGUNO,
                &[]
            )
            .is_none(),
            "sin canto, en medio del lado no hay nada notable"
        );
    }

    #[test]
    fn el_canto_no_convierte_el_papel_en_un_carril() {
        // Caso negativo: el marco delimita hasta donde llega el dibujo, no es
        // algo dibujado. Pegarse a sus cuatro bordes seria insufrible.
        let marco = Elemento {
            figura: Figura::Marco {
                nombre: "hoja".into(),
            },
            ancho: 400.0,
            alto: 400.0,
            ..rectangulo(1)
        };
        let a = finos(&[marco], Punto2::nuevo(200.0, 3.0), Faena::Trazando);
        assert!(a.is_none_or(|a| a.tipo != TipoAnclaje::Borde));
    }

    #[test]
    fn a_mano_alzada_no_engancha_ni_a_cruces_ni_al_canto() {
        // Caso negativo, y el que protege el lapiz de los tres anclajes
        // nuevos: un trazo que salta a un cruce se rompe igual que uno que
        // salta a un vertice. El canto SI se permite trazando a pulso en el
        // original, pero alla solo lo ofrecen las guias —un campo del
        // elemento que aqui no se lee— y encenderlo para todas las figuras
        // convertiria cada rectangulo en un carril.
        let es = [horizontal(1, 0.0), vertical(2, 0.0), rectangulo(3)];
        for p in [Punto2::nuevo(1.0, 1.0), Punto2::nuevo(40.0, 3.0)] {
            assert!(
                finos(&es, p, Faena::AMano).is_none(),
                "el lapiz pego un tiron en {p:?}"
            );
        }
    }

    #[test]
    fn sin_origen_de_coordenadas_no_hay_ancla_de_eje() {
        // Caso negativo, y el que evita un ancla fantasma: mientras nadie sepa
        // leer `origenCoordenadas`, no se puede ofrecer un eje que el usuario
        // no ve dibujado en ninguna parte.
        assert!(AjustesFinos::default().origen.is_none());
        assert!(finos(&[], Punto2::nuevo(0.0, 0.0), Faena::Trazando).is_none());
    }

    #[test]
    fn con_origen_se_engancha_al_cero_y_a_sus_dos_rectas() {
        let con_eje = AjustesFinos {
            origen: Some(Punto2::nuevo(0.0, 0.0)),
            ..Default::default()
        };
        let pedir = |p: Punto2| sitio_fino(&[], p, 1.0, Faena::Trazando, &todo(), &con_eje, &[]);
        // Justo en el cero, el cero. Cerca de el manda la recta y no el
        // punto, y tiene que ser asi: el pie sobre el eje siempre esta mas
        // cerca que el origen, porque es su proyeccion.
        let origen = pedir(Punto2::nuevo(0.0, 0.0)).expect("el cero engancha");
        assert_eq!(origen.tipo, TipoAnclaje::Eje);
        assert_eq!(origen.punto, Punto2::nuevo(0.0, 0.0));

        // A la altura del cero, lejos del origen: se cae sobre la recta.
        let sobre_la_recta = pedir(Punto2::nuevo(300.0, 3.0)).expect("la recta engancha");
        assert_eq!(sobre_la_recta.punto, Punto2::nuevo(300.0, 0.0));

        // Y lejos de las dos rectas, nada.
        assert!(pedir(Punto2::nuevo(300.0, 300.0)).is_none());
    }

    #[test]
    fn las_cinco_marcas_se_distinguen_y_todas_cierran() {
        // Las marcas aparecen a un par de pixeles unas de otras cuando una
        // raya toca una esquina: si dos se pintaran igual, la queja «engancha
        // donde no quiero» no se podria ni diagnosticar.
        let formas: Vec<usize> = [
            TipoAnclaje::Esquina,
            TipoAnclaje::Medio,
            TipoAnclaje::Centro,
            TipoAnclaje::Interseccion,
            TipoAnclaje::Eje,
            TipoAnclaje::Borde,
        ]
        .into_iter()
        .map(|t| {
            let p = puntos_de(&pista(&ancla(t), 1.0));
            assert_eq!(p[0], p[p.len() - 1], "la marca de {t:?} queda abierta");
            p.len()
        })
        .collect();
        // El aspa y la cruz tienen los dos siete puntos, pero no los mismos:
        // se comparan las formas de verdad.
        let aspa = puntos_de(&pista(&ancla(TipoAnclaje::Interseccion), 1.0));
        let cruz = puntos_de(&pista(&ancla(TipoAnclaje::Eje), 1.0));
        assert_ne!(aspa, cruz, "el cruce y el eje se pintan igual");
        assert!(formas.iter().all(|n| *n >= 4));
    }
}
