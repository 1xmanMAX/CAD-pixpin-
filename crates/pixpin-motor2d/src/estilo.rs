//! El estilo del panel lateral de Excalidraw: con que nace lo que se dibuja
//! y como se cambia lo que ya esta dibujado.
//!
//! En Excalidraw el panel hace las dos cosas a la vez: si hay algo elegido,
//! cambia eso; y SIEMPRE deja el valor como el de lo proximo que se dibuje
//! (`currentItem*` en su `appState`). Aqui igual: `EstiloDibujo` es ese
//! «actual» y `aplicar_a` lo lleva a la seleccion en un solo paso de
//! deshacer.

use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
use crate::escena::Escena;
use crate::relleno::EstiloRelleno;
use crate::seleccion::Seleccion;
use crate::tinta::MaterialTinta;

/// Los tres grosores del panel de Excalidraw (`strokeWidth` 1, 2 y 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NivelGrosor {
    Fino,
    Medio,
    Grueso,
}

impl NivelGrosor {
    /// El grosor de una figura (rectangulo, linea...), en unidades del mundo.
    pub fn de_forma(self) -> f32 {
        match self {
            NivelGrosor::Fino => 1.0,
            NivelGrosor::Medio => 2.0,
            NivelGrosor::Grueso => 4.0,
        }
    }

    /// El grosor del resaltador: el del lapiz del movil (la mitad del de las
    /// formas, `freedrawWidthFor`) por `ENGORDE_DEL_MARCADOR` (5). Medio =
    /// 2 / 2 * 5 = 5, que su tinta pinta a unos 30 de ancho, como el movil.
    pub fn de_resaltador(self) -> f32 {
        self.de_forma() / 2.0 * crate::tinta::ENGORDE_DEL_MARCADOR
    }

    /// El `strokeWidth` del lapiz, que la tinta multiplica por su factor.
    pub fn de_tinta(self) -> f32 {
        match self {
            NivelGrosor::Fino => crate::tinta::GROSOR_FINO,
            NivelGrosor::Medio => crate::tinta::GROSOR_MEDIO,
            NivelGrosor::Grueso => crate::tinta::GROSOR_GRUESO,
        }
    }

    /// El nivel mas cercano a un grosor ya dibujado, para marcar en el panel
    /// el boton que corresponde a lo seleccionado.
    pub fn de_elemento(figura: &Figura, grosor: f32) -> NivelGrosor {
        let (fino, medio, grueso) = match figura {
            Figura::Lapiz { .. } => (
                crate::tinta::GROSOR_FINO,
                crate::tinta::GROSOR_MEDIO,
                crate::tinta::GROSOR_GRUESO,
            ),
            Figura::Resaltador { .. } => (
                NivelGrosor::Fino.de_resaltador(),
                NivelGrosor::Medio.de_resaltador(),
                NivelGrosor::Grueso.de_resaltador(),
            ),
            _ => (1.0, 2.0, 4.0),
        };
        let d = |v: f32| (grosor - v).abs();
        if d(fino) <= d(medio) && d(fino) <= d(grueso) {
            NivelGrosor::Fino
        } else if d(medio) <= d(grueso) {
            NivelGrosor::Medio
        } else {
            NivelGrosor::Grueso
        }
    }
}

/// El «actual» del panel: con esto nace cada elemento nuevo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstiloDibujo {
    pub trazo: ColorRgba,
    /// `None` = transparente, el fondo por defecto de Excalidraw.
    pub relleno: Option<ColorRgba>,
    /// Como se pinta ese fondo: solido, rayado o cruzado.
    pub estilo_relleno: EstiloRelleno,
    pub grosor: NivelGrosor,
    pub estilo: EstiloTrazo,
    /// 0 arquitecto, 1 artista, 2 caricaturista.
    pub rugosidad: f32,
    /// De 0 a 1.
    pub opacidad: f32,
    /// **De que esta hecha la tinta**: lisa, con grano o encendida. Va con el
    /// color y no con la herramienta, porque es la otra mitad de «de que
    /// color va esto» (misma razon que en el movil).
    pub material: MaterialTinta,
    /// **La dureza del grafito con que se dibuja** (`ItemStyle.dureza` del
    /// movil): se recuerda como el color y solo la usa lo que es de grafito.
    pub dureza: crate::tinta::grafito::Dureza,
    /// **Lo que Excalidraw recuerda aparte para lo siguiente**
    /// (`currentItemRoundness`, `currentItemArrowType`,
    /// `currentItemStart/EndArrowhead`, `currentItemFontFamily`,
    /// `currentItemFontSize`). Sin esto, elegir «Bordes: redondo» sin nada
    /// elegido no hacia nada: la figura nueva nacia en pico porque
    /// `gesto::nuevo_elemento` solo miraba el estilo comun.
    ///
    /// Esquinas redondeadas para el proximo rectangulo o rombo.
    pub redondo: bool,
    /// La proxima flecha, de codos.
    pub codos: bool,
    pub punta_inicio: crate::formas::TipoPunta,
    pub punta_fin: crate::formas::TipoPunta,
    /// El numero de familia del proximo texto (5, 6 u 8). `None` = la de
    /// siempre (`texto::FAMILIA_POR_DEFECTO`): hasta que alguien elija una,
    /// el texto nuevo nace igual que antes de existir este campo.
    pub familia: Option<u8>,
    /// El `fontSize` del proximo texto.
    pub tamano_letra: f32,
    /// La proxima flecha, curva (`currentItemArrowType: "round"`). Aparte de
    /// `redondo` a proposito: en Excalidraw el tipo de flecha y los bordes
    /// son dos «actuales» distintos, y redondear rectangulos no tiene por
    /// que curvar la proxima flecha.
    pub curva: bool,
    /// Los renglones del proximo texto (`currentItemTextAlign`).
    pub alineacion: crate::texto::AlineacionTexto,
    /// Y su altura dentro de una figura (`verticalAlign` del estilo del
    /// movil, que tambien lo guarda para lo siguiente).
    pub alineacion_vertical: crate::texto::AlineacionVertical,
    /// **El proximo mosaico desenfoca en vez de pixelar** (`mosaicBlur` del
    /// `ItemStyle` del movil).
    pub desenfoque: bool,
    /// **Cuanto agranda la proxima lupa** (`ItemStyle.aumento`, el doble).
    pub aumento: f32,
    /// **Con que senala la proxima lupa de donde sale** (`ItemStyle.guia`, la
    /// flecha).
    pub guia: crate::lupa_elemento::GuiaDeLupa,
    /// **Como se cierran los arcos nuevos** (abierto, sector, segmento).
    pub cierre_arco: Option<crate::geometria_exacta::CierreArco>,
    /// **Si el relleno nuevo va degradado**, y como (lineal o radial).
    pub degradado: Option<crate::geometria_exacta::TipoDegradado>,
    /// Cuantos arquitos llevan las marcas de angulo nuevas.
    pub arcos_angulo: u8,
    /// Como traza la herramienta Arco.
    pub modo_arco: crate::geometria_exacta::ModoArco,
    /// **Que pone la herramienta Angulo**: con letra griega (de fabrica), la
    /// marca sola o el cuadradito del recto.
    pub modo_angulo: crate::geometria_exacta::ModoAngulo,
}

impl Default for EstiloDibujo {
    /// Los valores por defecto de Excalidraw: trazo `#1e1e1e`, sin fondo,
    /// relleno rayado, grosor medio, trazo continuo, «artista» y opaco.
    fn default() -> Self {
        let gris = 0x1e as f32 / 255.0;
        Self {
            trazo: ColorRgba::opaco(gris, gris, gris),
            relleno: None,
            estilo_relleno: EstiloRelleno::default(),
            grosor: NivelGrosor::Medio,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            material: MaterialTinta::Lisa,
            dureza: crate::tinta::grafito::Dureza::Hb,
            // En pico, como hasta ahora. Excalidraw nace redondo fuera de
            // sus pruebas, pero cambiarlo aqui cambiaria el aspecto de todo
            // lo que el usuario dibuja sin que lo haya pedido.
            redondo: false,
            codos: false,
            punta_inicio: crate::formas::TipoPunta::Ninguna,
            punta_fin: crate::formas::TipoPunta::Flecha,
            familia: None,
            tamano_letra: crate::texto::TAM_POR_DEFECTO,
            // Recta, como hasta ahora: lo mismo que con los bordes.
            curva: false,
            alineacion: crate::texto::AlineacionTexto::Izquierda,
            alineacion_vertical: crate::texto::AlineacionVertical::Arriba,
            desenfoque: false,
            aumento: crate::lupa_elemento::AUMENTO_POR_DEFECTO,
            guia: crate::lupa_elemento::GuiaDeLupa::Flecha,
            cierre_arco: None,
            degradado: None,
            arcos_angulo: 1,
            modo_arco: crate::geometria_exacta::ModoArco::Guia,
            modo_angulo: crate::geometria_exacta::ModoAngulo::Griega,
        }
    }
}

/// Un control del panel pulsado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CambioEstilo {
    Trazo(ColorRgba),
    Relleno(Option<ColorRgba>),
    /// Como se pinta el fondo: solido, rayado o cruzado.
    EstiloRelleno(EstiloRelleno),
    Grosor(NivelGrosor),
    Estilo(EstiloTrazo),
    Rugosidad(f32),
    Opacidad(f32),
    /// De que esta hecha la tinta. Ver `tinta::material`.
    Material(MaterialTinta),
    /// **La dureza de la mina** (4H ... 8B). Solo llega a lo que es de
    /// grafito (`conEstilo` del movil: `if material == CUADRITOS`).
    Dureza(crate::tinta::grafito::Dureza),
}

impl EstiloDibujo {
    pub fn aplicar(&mut self, cambio: CambioEstilo) {
        match cambio {
            CambioEstilo::Trazo(c) => self.trazo = c,
            CambioEstilo::Relleno(c) => self.relleno = c,
            CambioEstilo::EstiloRelleno(r) => self.estilo_relleno = r,
            CambioEstilo::Grosor(g) => self.grosor = g,
            CambioEstilo::Estilo(e) => self.estilo = e,
            CambioEstilo::Rugosidad(r) => self.rugosidad = r.clamp(0.0, 2.0),
            CambioEstilo::Opacidad(o) => self.opacidad = o.clamp(0.0, 1.0),
            CambioEstilo::Material(m) => self.material = m,
            CambioEstilo::Dureza(d) => self.dureza = d,
        }
    }

    /// Deja un [`CambioForma`] como el valor de lo proximo que se dibuje,
    /// como hace Excalidraw con cada `currentItem*`. La presion no esta: el
    /// lapiz la lleva en `Gesto::variabilidad`, que es anterior a esto.
    pub fn aplicar_forma(&mut self, cambio: CambioForma) {
        match cambio {
            CambioForma::Bordes { redondo } => self.redondo = redondo,
            CambioForma::Codos(v) => self.codos = v,
            CambioForma::PuntaInicio(t) => self.punta_inicio = t,
            CambioForma::PuntaFin(t) => self.punta_fin = t,
            CambioForma::Familia(n) => {
                self.familia = Some(crate::texto::familia_resuelta(Some(n)));
            }
            CambioForma::TamanoLetra(t) => self.tamano_letra = t.max(1.0),
            CambioForma::Presion(_) => {}
            // Los tres tipos se excluyen: una flecha no es curva y de codos
            // a la vez, asi que elegir uno apaga los otros dos.
            CambioForma::TipoFlecha(t) => {
                self.codos = t == TipoFlecha::Codos;
                self.curva = t == TipoFlecha::Curva;
            }
            CambioForma::Alineacion(a) => self.alineacion = a,
            CambioForma::AlineacionVertical(v) => self.alineacion_vertical = v,
            CambioForma::Desenfoque(v) => self.desenfoque = v,
            CambioForma::AumentoLupa(z) => {
                self.aumento = z.clamp(
                    crate::lupa_elemento::AUMENTO_MINIMO,
                    crate::lupa_elemento::AUMENTO_MAXIMO,
                )
            }
            CambioForma::GuiaLupa(g) => self.guia = g,
            CambioForma::CierreArco(c) => self.cierre_arco = c,
            CambioForma::Degradado(t) => self.degradado = t,
            CambioForma::ArcosAngulo(n) => self.arcos_angulo = n.clamp(1, 3),
            CambioForma::ModoArco(m) => self.modo_arco = m,
            CambioForma::ModoAngulo(m) => self.modo_angulo = m,
            // Solo con un foco elegido: el foco nace de una figura con los de
            // fabrica del movil.
            CambioForma::Oscurecer(_) | CambioForma::ZonaFoco(_) => {}
        }
    }
}

/// **Los tres tipos de flecha de Excalidraw** (`currentItemArrowType`):
/// afilada, curva y de codos.
///
/// En el fichero son dos campos sueltos —`elbowed` y `roundness`— y eso deja
/// estados que no significan nada, como «de codos y curva a la vez». Aqui se
/// vuelven tres opciones, que es como se piensan y como las ofrece el panel
/// del movil (`FormaDeFlecha`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoFlecha {
    Afilada,
    Curva,
    Codos,
}

impl TipoFlecha {
    /// El tipo de una flecha. `None` si no es una flecha.
    pub fn de(e: &crate::elemento::Elemento) -> Option<TipoFlecha> {
        match &e.figura {
            Figura::Flecha { codos: true, .. } => Some(TipoFlecha::Codos),
            Figura::Flecha { .. } if e.redondo => Some(TipoFlecha::Curva),
            Figura::Flecha { .. } => Some(TipoFlecha::Afilada),
            _ => None,
        }
    }
}

/// El grosor que tendria `figura` con el nivel `g`.
fn grosor_para(figura: &Figura, g: NivelGrosor) -> f32 {
    match figura {
        Figura::Lapiz { .. } => g.de_tinta(),
        Figura::Resaltador { .. } => g.de_resaltador(),
        _ => g.de_forma(),
    }
}

/// Lleva el cambio a todo lo seleccionado, como UN paso de deshacer. Solo
/// toca lo que cambia de verdad: pulsar el color que ya tiene no ensucia el
/// historial.
pub fn aplicar_a(escena: &mut Escena, sel: &Seleccion, cambio: CambioEstilo) {
    let cambia = |e: &crate::elemento::Elemento| match cambio {
        CambioEstilo::Trazo(c) => e.trazo != c,
        CambioEstilo::Relleno(c) => e.relleno != c,
        CambioEstilo::EstiloRelleno(r) => e.estilo_relleno != r,
        CambioEstilo::Grosor(g) => e.grosor != grosor_para(&e.figura, g),
        CambioEstilo::Estilo(s) => e.estilo != s,
        CambioEstilo::Rugosidad(r) => e.rugosidad != r.clamp(0.0, 2.0),
        CambioEstilo::Opacidad(o) => e.opacidad != o.clamp(0.0, 1.0),
        CambioEstilo::Material(m) => e.material != m,
        CambioEstilo::Dureza(d) => e.material.es_grafito() && e.extras.dureza != d.para_guardar(),
    };
    let afectados: Vec<u64> = sel
        .ids()
        .iter()
        .copied()
        .filter(|id| escena.buscar(*id).is_some_and(|e| !e.borrado && cambia(e)))
        .collect();
    if afectados.is_empty() {
        return;
    }
    escena.abrir_paso();
    for id in afectados {
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            match cambio {
                CambioEstilo::Trazo(c) => e.trazo = c,
                CambioEstilo::Relleno(c) => e.relleno = c,
                CambioEstilo::EstiloRelleno(r) => e.estilo_relleno = r,
                CambioEstilo::Grosor(g) => e.grosor = grosor_para(&e.figura, g),
                CambioEstilo::Estilo(s) => e.estilo = s,
                CambioEstilo::Rugosidad(r) => e.rugosidad = r.clamp(0.0, 2.0),
                CambioEstilo::Opacidad(o) => e.opacidad = o.clamp(0.0, 1.0),
                CambioEstilo::Material(m) => e.material = m,
                CambioEstilo::Dureza(d) => e.extras.dureza = d.para_guardar(),
            }
            // Sin subir la version la cache seguiria pintando el aspecto
            // viejo.
            e.tocar();
        }
    }
    escena.cerrar_paso();
}

// -------------------------------------------------------------------------
// Lo que no es estilo comun: bordes, flechas, letra y presion
// -------------------------------------------------------------------------

/// Los mandos del panel que solo tienen sentido para UN tipo de figura:
/// las esquinas del recuadro, las puntas y el codo de la flecha, la letra
/// del texto y la pluma del lapiz.
///
/// Van aparte de [`CambioEstilo`] porque cada uno solo lo admiten algunas
/// figuras ([`CambioForma::admite`]). Tambien son un «actual», como en
/// Excalidraw: [`EstiloDibujo::aplicar_forma`] los deja para lo proximo que
/// se dibuje y `gesto::nuevo_elemento` los lee.
///
/// Los valores son los del formato de Excalidraw, para que lo cambiado aqui
/// viaje al movil tal cual: `roundness`, `elbowed`, `startArrowhead`,
/// `endArrowhead`, `fontFamily` (5, 6 u 8), `fontSize` y
/// `strokeOptions.variability`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CambioForma {
    /// Esquinas redondeadas (`roundness`) o en pico.
    Bordes {
        redondo: bool,
    },
    /// Flecha de codos (`elbowed`) o recta.
    Codos(bool),
    PuntaInicio(crate::formas::TipoPunta),
    PuntaFin(crate::formas::TipoPunta),
    /// El numero de familia del fichero: 5 a mano, 6 normal, 8 codigo.
    Familia(u8),
    /// El `fontSize`: 16, 20, 28 o 36 en el panel de Excalidraw.
    TamanoLetra(f32),
    /// Pluma constante o variable del lapiz.
    Presion(crate::tinta::Variabilidad),
    /// Afilada, curva (`roundness`) o de codos (`elbowed`): los tres botones
    /// de «Tipo de flecha». `Codos` sigue existiendo para quien solo quiere
    /// el interruptor; el panel usa este, que no deja estados a medias.
    TipoFlecha(TipoFlecha),
    /// Los renglones a la izquierda, al centro o a la derecha (`textAlign`).
    Alineacion(crate::texto::AlineacionTexto),
    /// El rotulo arriba, en medio o abajo de su figura (`verticalAlign`).
    AlineacionVertical(crate::texto::AlineacionVertical),
    /// **Pixelar o desenfocar** un mosaico (`Propiedad.MOSAICO`, `mosaicBlur`).
    Desenfoque(bool),
    /// **Cuanto agranda una lupa** (`Propiedad.LUPA`): crece el cristal desde
    /// su centro y lo mirado no se mueve (`conAumento`).
    AumentoLupa(f32),
    /// **Con que senala una lupa** de donde sale lo que ensena.
    GuiaLupa(crate::lupa_elemento::GuiaDeLupa),
    /// **Cuanto oscurece un foco** lo de su anillo, en por ciento (10-90,
    /// `Propiedad.OSCURECER` del movil).
    Oscurecer(u8),
    /// **Que parte de su marco ocupa el hueco de un foco** (`Propiedad.ZONA`,
    /// `conZona`): el marco no se mueve, crece o mengua lo iluminado.
    ZonaFoco(f32),
    /// **Como se cierra un arco**: abierto (`None`), sector o segmento
    /// (`cierreArco`, laminas de geometria).
    CierreArco(Option<crate::geometria_exacta::CierreArco>),
    /// **Relleno liso (`None`) o degradado**, lineal o radial (`degradado`).
    Degradado(Option<crate::geometria_exacta::TipoDegradado>),
    /// Cuantos arquitos lleva una marca de angulo (1 a 3).
    ArcosAngulo(u8),
    /// Como traza la herramienta Arco. Solo es un «actual»: no cambia nada
    /// de lo ya dibujado.
    ModoArco(crate::geometria_exacta::ModoArco),
    /// **Que pone la herramienta Angulo** (letra griega, sola, recto). En una
    /// marca elegida, «recto» la pasa al cuadradito y «sola» le quita la
    /// letra; la letra nueva la pone la herramienta, que ve el dibujo entero.
    ModoAngulo(crate::geometria_exacta::ModoAngulo),
}

impl CambioForma {
    /// Si esta figura admite el cambio. Es el filtro que deja cambiar una
    /// seleccion mezclada sin estropear lo que no lo admite: poner punta a un
    /// rectangulo no significa nada, y pasarlo a «de codos» tampoco.
    ///
    /// Los bordes, solo en rectangulo y rombo: son las dos figuras que
    /// `pintado.rs` sabe redondear, con cualquier rugosidad. La linea y la
    /// imagen tambien llevan `roundness` en Excalidraw, pero aqui cambiarlo
    /// no se veria, y un boton que no cambia nada de lo que se ve es un boton
    /// que miente.
    pub fn admite(&self, figura: &Figura) -> bool {
        match self {
            CambioForma::Bordes { .. } => matches!(figura, Figura::Rectangulo | Figura::Rombo),
            CambioForma::Codos(_)
            | CambioForma::PuntaInicio(_)
            | CambioForma::PuntaFin(_)
            | CambioForma::TipoFlecha(_) => matches!(figura, Figura::Flecha { .. }),
            // La vertical tambien la admite un texto suelto, como en
            // Excalidraw: se guarda y cuenta en cuanto se meta en una caja.
            // **La letra, en todo lo que tiene letra** (`fontFamily` del movil,
            // que la guarda tambien en la cota, el numero de serie, el punto y
            // la escala): el pedido del usuario fue «elegir el tipo de letra
            // en todo lo que tenga letra».
            CambioForma::Familia(_) => lleva_letra(figura),
            // El tamano: el texto y el numero de serie (su circulo es la letra
            // por 0,9 de radio, `SERIAL_RADIUS`).
            CambioForma::TamanoLetra(_) => {
                matches!(figura, Figura::Texto { .. } | Figura::Serie { .. })
            }
            CambioForma::Alineacion(_) | CambioForma::AlineacionVertical(_) => {
                matches!(figura, Figura::Texto { .. })
            }
            CambioForma::Presion(_) => matches!(figura, Figura::Lapiz { .. }),
            CambioForma::Desenfoque(_) => matches!(figura, Figura::Mosaico { .. }),
            CambioForma::AumentoLupa(_) | CambioForma::GuiaLupa(_) => {
                matches!(figura, Figura::Lupa { .. })
            }
            CambioForma::Oscurecer(_) | CambioForma::ZonaFoco(_) => {
                matches!(figura, Figura::Foco { .. })
            }
            CambioForma::CierreArco(_)
            | CambioForma::ArcosAngulo(_)
            | CambioForma::ModoAngulo(_) => {
                matches!(
                    figura,
                    Figura::Arco {
                        barrido: Some(_),
                        ..
                    }
                )
            }
            CambioForma::Degradado(_) => matches!(
                figura,
                Figura::Rectangulo
                    | Figura::Rombo
                    | Figura::Elipse
                    | Figura::Arco { .. }
                    | Figura::Region { .. }
            ),
            CambioForma::ModoArco(_) => false,
        }
    }
}

/// **Si la figura lleva letra** que se pueda elegir: el texto (en su
/// `familia`) y las que rotulan sin ser un texto (en `extras.familia`, que
/// viaja como su `fontFamily`). Las graficas, tablas y ecuaciones son textos
/// agrupados, asi que ya entran por el texto.
pub fn lleva_letra(figura: &Figura) -> bool {
    matches!(
        figura,
        Figura::Texto { .. }
            | Figura::Serie { .. }
            | Figura::Cota { .. }
            | Figura::Punto { .. }
            | Figura::EscalaGrafica
            | Figura::Instrumento(_)
    )
}

/// La pluma que de verdad pinta este lapiz: la presion firme del movil manda
/// sobre lo que diga `strokeOptions` (ver `tinta::opciones_con_presion_firme`).
pub fn variabilidad_de(e: &crate::elemento::Elemento) -> Option<crate::tinta::Variabilidad> {
    use crate::tinta::Variabilidad;
    match &e.figura {
        Figura::Lapiz { opciones, .. } => Some(if e.extras.presion_firme {
            Variabilidad::Constante
        } else {
            opciones.map(|o| o.variabilidad).unwrap_or_default()
        }),
        _ => None,
    }
}

/// Si aplicar `cambio` a `e` cambiaria algo. Pulsar el valor que ya tiene no
/// abre un paso de deshacer vacio, igual que en `aplicar_a`.
fn cambiaria_forma(e: &crate::elemento::Elemento, cambio: CambioForma) -> bool {
    if !cambio.admite(&e.figura) {
        return false;
    }
    match (cambio, &e.figura) {
        (CambioForma::Bordes { redondo }, _) => e.redondo != redondo,
        (CambioForma::Codos(v), Figura::Flecha { codos, .. }) => *codos != v,
        (CambioForma::PuntaInicio(t), Figura::Flecha { punta_inicio, .. }) => *punta_inicio != t,
        (CambioForma::PuntaFin(t), Figura::Flecha { punta_fin, .. }) => *punta_fin != t,
        (CambioForma::Familia(n), Figura::Texto { familia, .. }) => {
            crate::texto::numero_de_familia(familia) != crate::texto::familia_resuelta(Some(n))
        }
        // Las demas: sin letra elegida siempre cambia (pasa de la del
        // sistema a una del catalogo).
        (CambioForma::Familia(n), _) => {
            e.extras.familia.as_deref() != Some(crate::texto::nombre_de_familia(Some(n)))
        }
        (CambioForma::TamanoLetra(t), Figura::Serie { .. }) => {
            (e.ancho - crate::serie::diametro(t)).abs() > 0.01
        }
        (CambioForma::TamanoLetra(t), Figura::Texto { tam, .. }) => (*tam - t).abs() > 0.01,
        (CambioForma::Presion(v), _) => variabilidad_de(e) != Some(v),
        (CambioForma::TipoFlecha(t), _) => TipoFlecha::de(e) != Some(t),
        // Con lo que se ve y no con lo que dice el fichero: pulsar
        // «izquierda» en un texto que no trae `textAlign` —y que ya se pinta
        // a la izquierda— no es un cambio, y no deja un paso de deshacer.
        (CambioForma::Alineacion(a), _) => e.extras.alineacion.unwrap_or_default() != a,
        (CambioForma::AlineacionVertical(v), _) => {
            e.extras.alineacion_vertical.unwrap_or_default() != v
        }
        (CambioForma::Desenfoque(v), Figura::Mosaico { desenfoque }) => *desenfoque != v,
        (CambioForma::AumentoLupa(z), Figura::Lupa { cristal }) => {
            (crate::lupa_elemento::aumento_de(cristal, crate::lupa_elemento::caja_de(e)) - z).abs()
                > 0.01
        }
        (CambioForma::GuiaLupa(g), Figura::Lupa { cristal }) => cristal.guia != g,
        (CambioForma::Oscurecer(n), Figura::Foco { cristal }) => {
            crate::lupa_elemento::oscurecimiento_de(cristal) != n
        }
        (CambioForma::ZonaFoco(z), Figura::Foco { cristal }) => {
            (crate::lupa_elemento::zona_de(cristal, crate::lupa_elemento::caja_de(e)) - z).abs()
                > 0.005
        }
        // La marca de angulo no se cierra: ya es un sector.
        (CambioForma::CierreArco(c), _) => e.extras.marca_angulo.is_none() && e.extras.cierre != c,
        (CambioForma::ArcosAngulo(n), _) => e
            .extras
            .marca_angulo
            .as_ref()
            .is_some_and(|m| !m.recto && m.arcos != n.clamp(1, 3)),
        (CambioForma::ModoAngulo(m), _) => e.extras.marca_angulo.as_ref().is_some_and(|x| {
            use crate::geometria_exacta::ModoAngulo as M;
            match m {
                M::Recto => !x.recto,
                M::Simple => x.recto || x.valor.is_some(),
                M::Griega => x.recto,
            }
        }),
        (CambioForma::Degradado(t), _) => e.extras.degradado.map(|d| d.tipo) != t,
        _ => false,
    }
}

/// Escribe el cambio en un elemento que lo admite.
/// La caja de un texto suelto, otra vez a la medida de lo escrito
/// (`texto::medida`). Solo con medidor de verdad y solo en los sueltos: el de
/// dentro de una figura lo coloca su figura.
fn remedir_texto_suelto(e: &mut crate::elemento::Elemento) {
    if !crate::texto::hay_medidor() || e.extras.contenedor.is_some() {
        return;
    }
    if let Some((ancho, alto)) = crate::texto_rico::medida_del_elemento(e) {
        e.ancho = ancho;
        e.alto = alto;
        return;
    }
    let estilo = crate::texto::EstiloDeTexto {
        negrita: e.extras.negrita,
        cursiva: e.extras.cursiva,
        tachado: false,
    };
    if let Figura::Texto {
        texto,
        tam,
        familia,
    } = &e.figura
    {
        let (ancho, alto) = crate::texto::medida(texto, *tam, familia, estilo);
        e.ancho = ancho;
        e.alto = alto;
    }
}

fn escribir_forma(e: &mut crate::elemento::Elemento, cambio: CambioForma) {
    use crate::tinta::{FACTOR_VARIABLE, OpcionesTinta, Variabilidad};
    match cambio {
        CambioForma::Bordes { redondo } => e.redondo = redondo,
        CambioForma::Codos(v) => {
            // Solo el interruptor: el camino en escalera se deriva de los dos
            // extremos al pintar (`codo::trazado_de_flecha`) y los puntos se
            // quedan como estaban, asi que volver a recta devuelve la flecha
            // de antes y no una aproximacion.
            if let Figura::Flecha { codos, .. } = &mut e.figura {
                *codos = v;
            }
        }
        CambioForma::PuntaInicio(t) => {
            if let Figura::Flecha { punta_inicio, .. } = &mut e.figura {
                *punta_inicio = t;
            }
        }
        CambioForma::PuntaFin(t) => {
            if let Figura::Flecha { punta_fin, .. } = &mut e.figura {
                *punta_fin = t;
            }
        }
        CambioForma::Familia(n) => {
            // El nombre y no el numero, que es lo que guarda la figura; al
            // escribir el fichero `numero_de_familia` lo vuelve a traducir.
            if let Figura::Texto { familia, .. } = &mut e.figura {
                *familia = crate::texto::nombre_de_familia(Some(n)).to_string();
            } else {
                // La cota, el numero, el punto y la escala: su
                // letra va en los extras y sale en su `fontFamily`. Su rotulo
                // se mide al pintarlo, asi que no hay caja que remedir.
                e.extras.familia = Some(crate::texto::nombre_de_familia(Some(n)).to_string());
            }
            // Otra letra, otra caja: la de Excalifont le queda corta o larga
            // a Nunito. Con medidor se remide (`redrawTextBoundingBox` de
            // Excalidraw); sin el, se queda como estaba, que medir a ojo no
            // distingue una letra de otra.
            remedir_texto_suelto(e);
        }
        CambioForma::TamanoLetra(nuevo) => {
            // El numero de serie: su circulo es la letra por 0,9 de radio. Se
            // agranda desde su centro, que es lo que senala.
            if let Figura::Serie { .. } = e.figura {
                let (cx, cy) = (e.x + e.ancho / 2.0, e.y + e.alto / 2.0);
                let d = crate::serie::diametro(nuevo);
                (e.x, e.y, e.ancho, e.alto) = (cx - d / 2.0, cy - d / 2.0, d, d);
                return;
            }
            if let Figura::Texto { texto, tam, .. } = &mut e.figura {
                let viejo = *tam;
                *tam = nuevo;
                if crate::texto::hay_medidor() && e.extras.contenedor.is_none() {
                    remedir_texto_suelto(e);
                    return;
                }
                // La caja crece con la letra, como al remedir en Excalidraw.
                // Sin medidor, a escala y no a ojo: un texto del movil trae
                // su caja medida con su fuente, y `medida_estimada` le daria
                // otra proporcion. Se queda quieta la esquina de arriba a la
                // izquierda, que es donde empieza a leerse.
                if viejo > 0.0 && e.ancho > 0.0 && e.alto > 0.0 {
                    let k = nuevo / viejo;
                    e.ancho *= k;
                    e.alto *= k;
                } else {
                    let (ancho, alto) = crate::texto::medida_estimada(texto, nuevo);
                    e.ancho = ancho;
                    e.alto = alto;
                }
            }
        }
        CambioForma::Presion(v) => {
            if let Figura::Lapiz { opciones, .. } = &mut e.figura {
                // Un trazo de antes de E1 (sin `strokeOptions`) guarda su
                // grosor en pixeles; al estrenar opciones se pasa a
                // `strokeWidth` o el trazo se pintaria cuatro veces mas gordo.
                let o = match opciones {
                    Some(o) => *o,
                    None => {
                        e.grosor /= FACTOR_VARIABLE;
                        OpcionesTinta::default()
                    }
                };
                *opciones = Some(OpcionesTinta {
                    variabilidad: v,
                    ..o
                });
            }
            // La presion firme del movil fuerza la pluma constante: sin
            // apagarla, pedir la variable no cambiaria nada. Es el mismo
            // campo con otro nombre, asi que apagarlo es decir lo mismo.
            if v == Variabilidad::Variable {
                e.extras.presion_firme = false;
            }
        }
        CambioForma::TipoFlecha(t) => {
            // Los dos campos del fichero a la vez: de codos apaga la curva y
            // curva apaga los codos, como `FormaDeFlecha.aplicadaA` del movil.
            // Los puntos no se tocan, asi que volver a afilada devuelve la
            // flecha de antes.
            if let Figura::Flecha { codos, .. } = &mut e.figura {
                *codos = t == TipoFlecha::Codos;
                e.redondo = t == TipoFlecha::Curva;
            }
        }
        CambioForma::Alineacion(a) => e.extras.alineacion = Some(a),
        CambioForma::AlineacionVertical(v) => e.extras.alineacion_vertical = Some(v),
        CambioForma::Desenfoque(v) => {
            if let Figura::Mosaico { desenfoque } = &mut e.figura {
                *desenfoque = v;
            }
        }
        CambioForma::AumentoLupa(z) => {
            let caja = crate::lupa_elemento::caja_de(e);
            if let Figura::Lupa { cristal } = &mut e.figura {
                let (nuevo, c) = crate::lupa_elemento::con_aumento(cristal, caja, z);
                *cristal = nuevo;
                (e.x, e.y, e.ancho, e.alto) = c;
            }
        }
        CambioForma::GuiaLupa(g) => {
            if let Figura::Lupa { cristal } = &mut e.figura {
                cristal.guia = g;
            }
        }
        CambioForma::Oscurecer(n) => {
            if let Figura::Foco { cristal } = &mut e.figura {
                cristal.oscurecer = Some(n.clamp(
                    crate::lupa_elemento::OSCURECER_MINIMO,
                    crate::lupa_elemento::OSCURECER_MAXIMO,
                ));
            }
        }
        CambioForma::ZonaFoco(z) => {
            let caja = crate::lupa_elemento::caja_de(e);
            if let Figura::Foco { cristal } = &mut e.figura {
                *cristal = crate::lupa_elemento::con_zona(cristal, caja, z);
            }
        }
        CambioForma::CierreArco(c) => e.extras.cierre = c,
        CambioForma::ArcosAngulo(n) => {
            if let Some(m) = e.extras.marca_angulo.as_mut() {
                m.arcos = n.clamp(1, 3);
            }
        }
        // Del color de su relleno (o de su tinta) a uno mucho mas claro.
        CambioForma::Degradado(t) => {
            e.extras.degradado = t.map(|t| {
                crate::geometria_exacta::Degradado::desde_relleno(t, e.relleno.unwrap_or(e.trazo))
            });
        }
        CambioForma::ModoArco(_) => {}
        CambioForma::ModoAngulo(m) => {
            use crate::geometria_exacta::ModoAngulo as M;
            if let Some(x) = e.extras.marca_angulo.as_mut() {
                x.recto = m == M::Recto;
                if m != M::Griega {
                    x.valor = None;
                }
            }
        }
    }
}

/// Lleva un [`CambioForma`] a lo seleccionado que lo admite, en UN paso de
/// deshacer. Lo que no lo admite se queda como estaba. Devuelve si cambio
/// algo.
pub fn aplicar_forma(escena: &mut Escena, sel: &Seleccion, cambio: CambioForma) -> bool {
    let afectados: Vec<u64> = sel
        .ids()
        .iter()
        .copied()
        .filter(|id| {
            escena
                .buscar(*id)
                .is_some_and(|e| !e.borrado && cambiaria_forma(e, cambio))
        })
        .collect();
    if afectados.is_empty() {
        return false;
    }
    escena.abrir_paso();
    for id in afectados {
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            escribir_forma(e, cambio);
            e.tocar();
        }
        // **El rotulo de una caja se mueve a su nuevo sitio.** Su alineacion
        // no es solo como van los renglones: dice en que parte de la figura
        // va (`computeBoundTextPosition`). Sin recolocarlo, «abajo» se
        // guardaba y el rotulo seguia arriba. Dentro del mismo paso, asi que
        // deshacer lo devuelve de una vez.
        if matches!(
            cambio,
            CambioForma::Alineacion(_) | CambioForma::AlineacionVertical(_)
        ) {
            let sitio = escena.buscar(id).and_then(|t| {
                crate::texto_en_figuras::sitio_en_su_contenedor(t, &escena.elementos)
            });
            if let (Some(p), Some(e)) = (sitio, escena.buscar_mut(id)) {
                e.x = p.x;
                e.y = p.y;
                e.tocar();
            }
        }
    }
    escena.cerrar_paso();
    true
}

// -------------------------------------------------------------------------
// Copiar estilo
// -------------------------------------------------------------------------

/// Que se le puede tocar a un tipo de figura (`Propiedad` del movil,
/// `DrawProperties.kt:18-88`).
///
/// Son **las del PC**: del enumerado del movil faltan aqui las que hablan de
/// campos que este motor todavia no tiene (`VOLUMEN`, `LUPA`, `OSCURECER`,
/// `ZONA`, `PRESION`, `ESTILO_DE_TEXTO`, `FORMA_FLECHA`). Cuando su grupo las
/// traiga, se anaden aqui y a la tabla de abajo a la vez, que es el punto de
/// que la tabla exista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propiedad {
    Trazo,
    Fondo,
    /// Como se pinta el fondo: solido, rayado, cruzado.
    Relleno,
    /// Continuo, a trazos o de puntos.
    Linea,
    Grosor,
    Rugosidad,
    /// Esquinas redondeadas.
    Esquinas,
    /// De que esta hecha la tinta.
    Material,
    Opacidad,
}

/// **La tabla**: que mandos aplican a que tipo (`propiedadesDeTipo`,
/// `DrawProperties.kt:169-303`).
///
/// Es la misma tabla para dos cosas, y de eso depende todo: decide **que
/// controles salen** en el panel y decide **que se escribe encima** al pegar
/// un estilo. Con dos tablas, un panel acaba ofreciendo algo que no se
/// guarda, o guardando algo que no se ofrece.
pub fn propiedades_de(figura: &Figura) -> &'static [Propiedad] {
    use Propiedad::*;
    match figura {
        // Las dos que se rellenan y se redondean.
        Figura::Rectangulo | Figura::Rombo => &[
            Trazo, Fondo, Relleno, Linea, Grosor, Material, Rugosidad, Esquinas, Opacidad,
        ],
        // La elipse no tiene esquinas que redondear.
        Figura::Elipse => &[
            Trazo, Fondo, Relleno, Linea, Grosor, Material, Rugosidad, Opacidad,
        ],
        // La linea si admite fondo: cerrada sobre si misma se rellena.
        Figura::Linea { .. } => &[
            Trazo, Fondo, Relleno, Linea, Grosor, Material, Rugosidad, Esquinas, Opacidad,
        ],
        // La flecha no encierra nada, asi que ni fondo ni esquinas.
        Figura::Flecha { .. } => &[Trazo, Linea, Grosor, Material, Rugosidad, Opacidad],
        // El lapiz no lleva linea discontinua ni esquinas, que serian botones
        // muertos. **Fondo y relleno si**, y hacen falta justamente para
        // poder apagarlos: un garabato que se cierra sobre si mismo se rellena
        // solo. Se quitaron una vez por creer que sobraban y lo que se
        // consiguio fue dejar la trama puesta y sin interruptor.
        Figura::Lapiz { .. } | Figura::Resaltador { .. } => {
            &[Trazo, Fondo, Relleno, Grosor, Material, Rugosidad, Opacidad]
        }
        // El arco es una raya curva: ni fondo ni relleno.
        Figura::Arco { .. } => &[Trazo, Linea, Grosor, Material, Rugosidad, Opacidad],
        // El texto y el circulito numerado: color y nada mas de lo que aqui
        // hay. El tamano de letra no viaja en `EstiloDibujo`, asi que no
        // entra en la copia.
        Figura::Texto { .. } | Figura::Serie { .. } | Figura::Punto { .. } => &[Trazo, Opacidad],
        // La cota se pinta lisa a proposito, para que se vea exactamente
        // donde acaba la medida: ni fondo, ni relleno, ni rugosidad.
        Figura::Cota { .. } => &[Trazo, Grosor, Material, Opacidad],
        // La reglita: ni relleno ni rugosidad, que una escala temblorosa no
        // se lee.
        Figura::EscalaGrafica => &[Trazo, Opacidad],
        // El instrumento de graficar: el color de sus rayas, su grosor, su
        // material y la letra de sus cifras; ni relleno ni temblor, que unos
        // ejes temblorosos no se leen (`DrawProperties.kt`).
        Figura::Instrumento(_) => &[Trazo, Grosor, Material, Opacidad],
        // El relleno de un hueco es **solo mancha**: su borde lo dibujan las
        // figuras que lo encierran, asi que ofrecerle color de trazo seria un
        // boton que no puede cambiar nada de lo que se ve.
        Figura::Region { .. } => &[Fondo, Relleno, Opacidad],
        // En el mosaico el grosor hace de tamano de grano.
        //
        // **Sin opacidad, y a proposito.** Es la unica figura del lienzo a la
        // que la opacidad no se le aplica (regla 1 de `mosaico.rs`): un
        // mosaico al 20 % no es discreto, es un dato legible. Esta tabla
        // decide tambien que se ESCRIBE al pegar un estilo, asi que con
        // `Opacidad` aqui, copiar el estilo de un rectangulo al 20 % y
        // pegarlo sobre un mosaico persistia `opacity: 20` en el
        // `.excalidraw`. Aqui no se ve a traves —`pintado.rs` fuerza `a: 1`—
        // pero el valor viajaba, y cualquier visor que lo respete lo revela.
        Figura::Mosaico { .. } => &[Grosor, Material],
        // La imagen y el emoji: solo lo que los tapa o los redondea.
        Figura::Imagen { .. } => &[Esquinas, Opacidad],
        Figura::Emoji { .. } => &[Opacidad],
        // El foco no dibuja tinta: apaga lo de alrededor. Lo que gradua es
        // cuanto oscurece, y eso aqui todavia no es un mando.
        Figura::Foco { .. } => &[],
        // La lupa: la tinta y el grueso de su montura (`Propiedad.LUPA` va
        // aparte, en el panel), como `propiedadesDeTipo(LUPA)` del movil.
        Figura::Lupa { .. } => &[Trazo, Grosor, Opacidad],
        // La hoja no tiene estilo: es un limite, no un dibujo. Solo se
        // estira.
        Figura::Marco { .. } => &[],
    }
}

/// El estilo tomado de una figura, listo para pegarlo en otras.
///
/// Guarda **todos** los valores aunque el tipo de origen no admita todos:
/// quien decide que se escribe es el cruce de las dos tablas al pegar, y
/// recortar ya aqui obligaria a mirar el tipo de origen dos veces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstiloCopiado {
    /// Que mandos tenia el tipo del que se copio.
    origen: &'static [Propiedad],
    trazo: ColorRgba,
    relleno: Option<ColorRgba>,
    estilo_relleno: EstiloRelleno,
    grosor: NivelGrosor,
    estilo: EstiloTrazo,
    rugosidad: f32,
    opacidad: f32,
    material: MaterialTinta,
    /// Va con el material: copiar un grafito 8B lleva el 8B.
    dureza: Option<crate::tinta::grafito::Dureza>,
    redondo: bool,
}

impl EstiloCopiado {
    /// Las propiedades que de verdad se llevo.
    pub fn propiedades(&self) -> &'static [Propiedad] {
        self.origen
    }
}

/// **Copia el estilo de un elemento** (el cuentagotas de estilo).
///
/// # Por que existe si el movil no lo tiene
///
/// Porque es invencion de escritorio y ahi se gana. Con un raton y un teclado,
/// «ponle a esta caja lo mismo que a aquella» son dos clics; a dedo son seis
/// mandos del panel, uno por uno, mirando cual estaba puesto. Se busco en
/// todo el motor de Android —incluida su paleta de colores, 431 lineas— y no
/// existe.
///
/// Lo que si es del movil es **la tabla**: [`propiedades_de`] dice
/// exactamente que campos tiene sentido llevarse de cada tipo, y es la misma
/// que decide que mandos ensena el panel.
pub fn copiar(e: &crate::elemento::Elemento) -> EstiloCopiado {
    EstiloCopiado {
        origen: propiedades_de(&e.figura),
        trazo: e.trazo,
        relleno: e.relleno,
        estilo_relleno: e.estilo_relleno,
        grosor: NivelGrosor::de_elemento(&e.figura, e.grosor),
        estilo: e.estilo,
        rugosidad: e.rugosidad,
        opacidad: e.opacidad,
        material: e.material,
        dureza: e.extras.dureza,
        redondo: e.redondo,
    }
}

/// **Pega el estilo copiado sobre la seleccion**, en un solo paso de
/// deshacer.
///
/// Solo se escribe lo que admiten **los dos** tipos, el de origen y el de
/// destino. Es la regla que hace que copiar de un rectangulo a un texto
/// lleve el color y no el relleno rayado —que en un rotulo no significa
/// nada—, y que copiar de un texto a un rectangulo no le borre el fondo que
/// ya tenia.
///
/// Devuelve cuantos cambiaron. Como el resto del panel, no ensucia el
/// historial si no hay nada que cambiar.
pub fn pegar_a(escena: &mut Escena, sel: &Seleccion, copiado: &EstiloCopiado) -> usize {
    let afectados: Vec<u64> = sel
        .ids()
        .iter()
        .copied()
        .filter(|id| {
            escena
                .buscar(*id)
                .is_some_and(|e| !e.borrado && !e.bloqueado && cambiaria(e, copiado))
        })
        .collect();
    if afectados.is_empty() {
        return 0;
    }
    escena.abrir_paso();
    let cuantos = afectados.len();
    for id in afectados {
        escena.apuntar_edicion(id);
        let Some(e) = escena.buscar_mut(id) else {
            continue;
        };
        for p in comunes(e, copiado) {
            aplicar_propiedad(e, p, copiado);
        }
        // Sin subir la version la cache seguiria pintando el aspecto viejo.
        e.tocar();
    }
    escena.cerrar_paso();
    cuantos
}

/// Las propiedades que admiten a la vez el origen y el destino.
fn comunes(e: &crate::elemento::Elemento, c: &EstiloCopiado) -> Vec<Propiedad> {
    propiedades_de(&e.figura)
        .iter()
        .copied()
        .filter(|p| c.origen.contains(p))
        .collect()
}

fn aplicar_propiedad(e: &mut crate::elemento::Elemento, p: Propiedad, c: &EstiloCopiado) {
    match p {
        Propiedad::Trazo => e.trazo = c.trazo,
        Propiedad::Fondo => e.relleno = c.relleno,
        Propiedad::Relleno => e.estilo_relleno = c.estilo_relleno,
        Propiedad::Linea => e.estilo = c.estilo,
        // El grosor viaja como NIVEL y no como numero: el lapiz y las
        // figuras miden en escalas distintas, asi que copiar el 2 de un
        // rectangulo a un lapiz le pondria un trazo que no es ninguno de sus
        // tres.
        Propiedad::Grosor => e.grosor = grosor_para(&e.figura, c.grosor),
        Propiedad::Rugosidad => e.rugosidad = c.rugosidad,
        Propiedad::Esquinas => e.redondo = c.redondo,
        Propiedad::Material => {
            e.material = c.material;
            // La dureza es del grafito: se lleva con el, y lo que deja de ser
            // grafito no se queda con una que ya no significa nada.
            e.extras.dureza = if c.material.es_grafito() {
                c.dureza
            } else {
                None
            };
        }
        Propiedad::Opacidad => e.opacidad = c.opacidad,
    }
}

fn cambiaria(e: &crate::elemento::Elemento, c: &EstiloCopiado) -> bool {
    comunes(e, c).into_iter().any(|p| match p {
        Propiedad::Trazo => e.trazo != c.trazo,
        Propiedad::Fondo => e.relleno != c.relleno,
        Propiedad::Relleno => e.estilo_relleno != c.estilo_relleno,
        Propiedad::Linea => e.estilo != c.estilo,
        Propiedad::Grosor => e.grosor != grosor_para(&e.figura, c.grosor),
        Propiedad::Rugosidad => e.rugosidad != c.rugosidad,
        Propiedad::Esquinas => e.redondo != c.redondo,
        Propiedad::Material => {
            e.material != c.material || (c.material.es_grafito() && e.extras.dureza != c.dureza)
        }
        Propiedad::Opacidad => e.opacidad != c.opacidad,
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::Elemento;
    use crate::formas::TipoPunta;

    fn rect(escena: &mut Escena) -> u64 {
        escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
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
        })
    }

    #[test]
    fn pegar_un_estilo_translucido_sobre_un_mosaico_no_le_baja_la_opacidad() {
        // **Privacidad.** Esta tabla decide tambien que se escribe al pegar
        // un estilo, asi que con `Opacidad` entre las del mosaico, copiar el
        // estilo de un rectangulo al 20 % y pegarlo sobre un mosaico dejaba
        // `opacity: 20` guardado en el `.excalidraw`. Aqui no se ve a traves
        // porque `pintado.rs` fuerza el alfa a uno, pero el valor viajaba al
        // movil y a cualquier visor, y el que lo respete ensena lo tapado.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        escena.buscar_mut(origen).unwrap().opacidad = 0.2;
        let copiado = copiar(escena.buscar(origen).unwrap());

        let mut tapa = Elemento {
            figura: Figura::Mosaico { desenfoque: false },
            ..escena.buscar(origen).unwrap().clone()
        };
        tapa.id = 0;
        tapa.opacidad = 1.0;
        let id = escena.anadir(tapa);
        let mut sel = Seleccion::nueva();
        sel.poner_todos([id]);
        pegar_a(&mut escena, &sel, &copiado);
        assert_eq!(
            escena.buscar(id).unwrap().opacidad,
            1.0,
            "un mosaico a medias no tapa: la opacidad no se le pega"
        );
        assert!(
            !propiedades_de(&Figura::Mosaico { desenfoque: false }).contains(&Propiedad::Opacidad)
        );

        // El caso negativo: sobre un rectangulo si se pega, o la funcion no
        // estaria haciendo nada y la prueba pasaria por la razon equivocada.
        let otro = rect(&mut escena);
        let mut sel2 = Seleccion::nueva();
        sel2.poner_todos([otro]);
        pegar_a(&mut escena, &sel2, &copiado);
        assert_eq!(escena.buscar(otro).unwrap().opacidad, 0.2);
    }

    #[test]
    fn cambiar_el_color_de_lo_elegido_es_un_solo_paso_que_se_deshace() {
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let b = rect(&mut escena);
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        let rojo = ColorRgba::opaco(0.88, 0.19, 0.19);
        aplicar_a(&mut escena, &sel, CambioEstilo::Trazo(rojo));
        assert_eq!(escena.buscar(a).unwrap().trazo, rojo);
        assert_eq!(escena.buscar(b).unwrap().trazo, rojo);
        assert!(escena.deshacer());
        let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
        assert_eq!(escena.buscar(a).unwrap().trazo, negro);
        assert_eq!(escena.buscar(b).unwrap().trazo, negro);
    }

    #[test]
    fn el_cambio_sube_la_version_para_que_la_cache_repinte() {
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let antes = escena.buscar(a).unwrap().version;
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        aplicar_a(&mut escena, &sel, CambioEstilo::Opacidad(0.5));
        assert!(escena.buscar(a).unwrap().version > antes);
    }

    #[test]
    fn pulsar_el_valor_que_ya_tiene_no_cambia_nada() {
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let antes = escena.buscar(a).unwrap().clone();
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        // El rectangulo ya tenia 2 (medio).
        aplicar_a(&mut escena, &sel, CambioEstilo::Grosor(NivelGrosor::Medio));
        assert_eq!(escena.buscar(a).unwrap(), &antes, "ni version subida");
    }

    #[test]
    fn el_grosor_del_lapiz_va_en_su_escala_y_el_de_la_forma_en_la_suya() {
        assert_eq!(NivelGrosor::Grueso.de_forma(), 4.0);
        assert_eq!(NivelGrosor::Grueso.de_tinta(), crate::tinta::GROSOR_GRUESO);
        let lapiz = Figura::Lapiz {
            puntos: vec![],
            presiones: vec![],
            opciones: None,
        };
        assert_eq!(
            NivelGrosor::de_elemento(&lapiz, crate::tinta::GROSOR_FINO),
            NivelGrosor::Fino
        );
        assert_eq!(
            NivelGrosor::de_elemento(&Figura::Rectangulo, 3.9),
            NivelGrosor::Grueso
        );
    }

    #[test]
    fn el_relleno_elegido_va_a_lo_seleccionado_y_queda_para_lo_proximo() {
        // Las dos mitades del panel de Excalidraw: cambia lo elegido y ademas
        // deja el valor como el de lo proximo que se dibuje.
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        let cambio = CambioEstilo::EstiloRelleno(EstiloRelleno::Cruzado);
        aplicar_a(&mut escena, &sel, cambio);
        assert_eq!(
            escena.buscar(a).unwrap().estilo_relleno,
            EstiloRelleno::Cruzado
        );

        let mut actual = EstiloDibujo::default();
        actual.aplicar(cambio);
        assert_eq!(actual.estilo_relleno, EstiloRelleno::Cruzado);

        // Y se deshace en un solo paso, como los demas cambios de estilo.
        assert!(escena.deshacer());
        assert_eq!(
            escena.buscar(a).unwrap().estilo_relleno,
            EstiloRelleno::default()
        );
    }

    #[test]
    fn cambiar_el_relleno_de_nada_no_abre_un_paso_de_deshacer_vacio() {
        // Caso negativo: sin seleccion no hay a quien aplicarselo, y un paso
        // vacio en el historial obligaria a pulsar deshacer dos veces para
        // deshacer una sola cosa.
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let vacia = Seleccion::nueva();
        aplicar_a(
            &mut escena,
            &vacia,
            CambioEstilo::EstiloRelleno(EstiloRelleno::Solido),
        );
        // El unico paso del historial sigue siendo el de haber anadido el
        // rectangulo: deshacer una vez tiene que quitarlo.
        assert!(escena.deshacer());
        assert!(escena.buscar(a).is_none_or(|e| e.borrado));
    }

    #[test]
    fn el_estilo_por_defecto_es_el_de_excalidraw_y_los_limites_se_respetan() {
        let mut e = EstiloDibujo::default();
        assert_eq!(e.relleno, None);
        // El rayado, no el solido: es lo que dibuja Excalidraw sin `fillStyle`.
        assert_eq!(e.estilo_relleno, EstiloRelleno::Rayado);
        assert_eq!(e.grosor, NivelGrosor::Medio);
        e.aplicar(CambioEstilo::Opacidad(3.0));
        assert_eq!(e.opacidad, 1.0);
        e.aplicar(CambioEstilo::Rugosidad(-1.0));
        assert_eq!(e.rugosidad, 0.0);
    }

    // --- Copiar estilo ---

    fn con(escena: &mut Escena, figura: Figura) -> u64 {
        let id = rect(escena);
        escena.buscar_mut(id).unwrap().figura = figura;
        id
    }

    fn sel_de(id: u64) -> Seleccion {
        let mut s = Seleccion::nueva();
        s.poner(id);
        s
    }

    #[test]
    fn copiar_un_rectangulo_en_otro_lleva_todo_su_estilo() {
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let destino = rect(&mut escena);
        let rojo = ColorRgba::opaco(0.88, 0.19, 0.19);
        {
            let e = escena.buscar_mut(origen).unwrap();
            e.trazo = rojo;
            e.relleno = Some(rojo);
            e.estilo = EstiloTrazo::Discontinuo;
            e.redondo = true;
            e.opacidad = 0.5;
        }
        let copiado = copiar(escena.buscar(origen).unwrap());
        assert_eq!(pegar_a(&mut escena, &sel_de(destino), &copiado), 1);

        let d = escena.buscar(destino).unwrap();
        assert_eq!(d.trazo, rojo);
        assert_eq!(d.relleno, Some(rojo));
        assert_eq!(d.estilo, EstiloTrazo::Discontinuo);
        assert!(d.redondo);
        assert_eq!(d.opacidad, 0.5);
    }

    #[test]
    fn de_un_rectangulo_a_un_texto_va_el_color_pero_no_el_relleno() {
        // La regla de la tabla: solo se escribe lo que admiten los dos. Un
        // rotulo con «relleno rayado» guardado no significa nada, y un fondo
        // puesto ahi seria un dato invisible que luego viaja al movil.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let verde = ColorRgba::opaco(0.1, 0.6, 0.2);
        {
            let e = escena.buscar_mut(origen).unwrap();
            e.trazo = verde;
            e.relleno = Some(verde);
            e.estilo_relleno = EstiloRelleno::Cruzado;
        }
        let copiado = copiar(escena.buscar(origen).unwrap());

        let destino = con(
            &mut escena,
            Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
        );
        pegar_a(&mut escena, &sel_de(destino), &copiado);
        let d = escena.buscar(destino).unwrap();
        assert_eq!(d.trazo, verde, "el color si");
        assert_eq!(d.relleno, None, "el fondo no");
        assert_eq!(d.estilo_relleno, EstiloRelleno::Rayado, "la trama tampoco");
    }

    #[test]
    fn de_un_texto_a_un_rectangulo_no_se_le_borra_el_fondo_que_tenia() {
        // El otro lado de la misma regla, y el que mas molesta cuando falta:
        // copiar de algo que no tiene fondo no puede significar «quitale el
        // fondo».
        let mut escena = Escena::nueva();
        let origen = con(
            &mut escena,
            Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
        );
        let copiado = copiar(escena.buscar(origen).unwrap());

        let destino = rect(&mut escena);
        let azul = ColorRgba::opaco(0.2, 0.3, 0.9);
        escena.buscar_mut(destino).unwrap().relleno = Some(azul);
        pegar_a(&mut escena, &sel_de(destino), &copiado);
        assert_eq!(escena.buscar(destino).unwrap().relleno, Some(azul));
    }

    #[test]
    fn el_grosor_viaja_como_nivel_y_no_como_numero() {
        // Copiar el 4 de un rectangulo a un lapiz le pondria un trazo que no
        // es ninguno de sus tres, porque miden en escalas distintas.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        escena.buscar_mut(origen).unwrap().grosor = NivelGrosor::Grueso.de_forma();
        let copiado = copiar(escena.buscar(origen).unwrap());

        let lapiz = con(
            &mut escena,
            Figura::Lapiz {
                puntos: vec![],
                presiones: vec![],
                opciones: None,
            },
        );
        pegar_a(&mut escena, &sel_de(lapiz), &copiado);
        assert_eq!(
            escena.buscar(lapiz).unwrap().grosor,
            crate::tinta::GROSOR_GRUESO
        );
    }

    #[test]
    fn pegar_es_un_solo_paso_de_deshacer_para_toda_la_seleccion() {
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let rojo = ColorRgba::opaco(0.88, 0.19, 0.19);
        escena.buscar_mut(origen).unwrap().trazo = rojo;
        let copiado = copiar(escena.buscar(origen).unwrap());
        let a = rect(&mut escena);
        let b = rect(&mut escena);
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        assert_eq!(pegar_a(&mut escena, &sel, &copiado), 2);
        assert!(escena.deshacer());
        let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
        assert_eq!(escena.buscar(a).unwrap().trazo, negro);
        assert_eq!(escena.buscar(b).unwrap().trazo, negro);
    }

    #[test]
    fn pegar_lo_mismo_que_ya_habia_no_ensucia_el_historial() {
        // Caso negativo: un paso vacio obliga a pulsar deshacer dos veces
        // para deshacer una sola cosa.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let copiado = copiar(escena.buscar(origen).unwrap());
        let destino = rect(&mut escena);
        assert_eq!(pegar_a(&mut escena, &sel_de(destino), &copiado), 0);
        let antes = escena.buscar(destino).unwrap().clone();
        assert_eq!(escena.buscar(destino).unwrap(), &antes, "ni version subida");
    }

    #[test]
    fn a_un_marco_no_se_le_pega_nada_y_lo_bloqueado_no_se_toca() {
        // Caso negativo doble: la hoja es un limite, no un dibujo, y
        // bloquear es pedir que no cambie.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        escena.buscar_mut(origen).unwrap().trazo = ColorRgba::opaco(1.0, 0.0, 0.0);
        let copiado = copiar(escena.buscar(origen).unwrap());

        let marco = con(
            &mut escena,
            Figura::Marco {
                nombre: "hoja".into(),
            },
        );
        let fijo = rect(&mut escena);
        escena.buscar_mut(fijo).unwrap().bloqueado = true;
        let mut sel = Seleccion::nueva();
        sel.poner_todos([marco, fijo]);
        assert_eq!(pegar_a(&mut escena, &sel, &copiado), 0);
        assert!(
            propiedades_de(&Figura::Marco {
                nombre: String::new()
            })
            .is_empty()
        );
    }

    #[test]
    fn la_tabla_de_tipos_dice_lo_mismo_que_la_del_movil_donde_importa() {
        use Propiedad::*;
        // Las tres diferencias que el movil documenta una por una, y que son
        // las que se copian mal si la tabla se escribe de memoria.
        assert!(
            !propiedades_de(&Figura::Elipse).contains(&Esquinas),
            "la elipse no tiene esquinas que redondear"
        );
        assert!(
            propiedades_de(&Figura::Linea { puntos: vec![] }).contains(&Fondo),
            "la linea cerrada sobre si misma se rellena"
        );
        assert!(
            !propiedades_de(&Figura::Flecha {
                puntos: vec![],
                punta_inicio: TipoPunta::Ninguna,
                punta_fin: TipoPunta::Flecha,
                codos: false,
            })
            .contains(&Fondo),
            "la flecha no encierra nada"
        );
        assert!(
            !propiedades_de(&Figura::Region {
                contorno: vec![],
                huecos: vec![]
            })
            .contains(&Trazo),
            "el relleno de un hueco es solo mancha: su borde lo dibujan otros"
        );
        assert!(
            !propiedades_de(&Figura::Cota { puntos: vec![] }).contains(&Rugosidad),
            "una cota temblorosa no dice donde acaba la medida"
        );
    }

    // --- CambioForma -----------------------------------------------------

    fn flecha() -> Figura {
        Figura::Flecha {
            puntos: vec![
                crate::vector::Punto2::nuevo(0.0, 0.0),
                crate::vector::Punto2::nuevo(50.0, 30.0),
            ],
            punta_inicio: TipoPunta::Ninguna,
            punta_fin: TipoPunta::Flecha,
            codos: false,
        }
    }

    fn texto(tam: f32) -> Figura {
        Figura::Texto {
            texto: "hola".into(),
            tam,
            familia: "Excalifont".into(),
        }
    }

    #[test]
    fn redondear_una_seleccion_mezclada_solo_toca_lo_que_tiene_esquinas() {
        let mut escena = Escena::nueva();
        let caja = rect(&mut escena);
        let punta = con(&mut escena, flecha());
        let mut sel = Seleccion::nueva();
        sel.poner_todos([caja, punta]);
        assert!(aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::Bordes { redondo: true }
        ));
        assert!(escena.buscar(caja).unwrap().redondo);
        // Caso negativo: una flecha no tiene esquinas; marcarla redonda
        // escribiria un `roundness` que nadie pidio.
        assert!(!escena.buscar(punta).unwrap().redondo);
        assert!(escena.deshacer());
        assert!(!escena.buscar(caja).unwrap().redondo, "un solo paso");
    }

    #[test]
    fn el_tipo_de_flecha_es_uno_de_tres_y_elegir_uno_apaga_los_otros() {
        let mut escena = Escena::nueva();
        let id = con(&mut escena, flecha());
        let sel = sel_de(id);
        assert_eq!(
            TipoFlecha::de(escena.buscar(id).unwrap()),
            Some(TipoFlecha::Afilada)
        );
        assert!(aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::TipoFlecha(TipoFlecha::Codos)
        ));
        assert!(aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::TipoFlecha(TipoFlecha::Curva)
        ));
        let e = escena.buscar(id).unwrap();
        assert!(e.redondo, "curva es `roundness`");
        assert!(
            matches!(e.figura, Figura::Flecha { codos: false, .. }),
            "curva y de codos a la vez no significa nada"
        );
        assert!(aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::TipoFlecha(TipoFlecha::Codos)
        ));
        assert!(!escena.buscar(id).unwrap().redondo);
        // Caso negativo: pulsar el que ya tiene no abre un paso vacio, y un
        // rectangulo no tiene tipo de flecha.
        assert!(!aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::TipoFlecha(TipoFlecha::Codos)
        ));
        let caja = rect(&mut escena);
        assert!(!aplicar_forma(
            &mut escena,
            &sel_de(caja),
            CambioForma::TipoFlecha(TipoFlecha::Curva)
        ));
        assert!(!escena.buscar(caja).unwrap().redondo);
    }

    #[test]
    fn alinear_el_rotulo_de_una_caja_lo_lleva_a_su_sitio_en_un_solo_paso() {
        use crate::texto::{AlineacionTexto, AlineacionVertical};
        let mut escena = Escena::nueva();
        let caja = rect(&mut escena);
        {
            let c = escena.buscar_mut(caja).unwrap();
            c.ancho = 200.0;
            c.alto = 100.0;
            c.extras.id_de_fichero = Some("caja".into());
        }
        let rotulo = con(&mut escena, texto(20.0));
        {
            let t = escena.buscar_mut(rotulo).unwrap();
            t.ancho = 40.0;
            t.alto = 20.0;
            t.x = 5.0;
            t.y = 5.0;
            t.extras.contenedor = Some("caja".into());
        }
        let sel = sel_de(rotulo);
        assert!(aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::AlineacionVertical(AlineacionVertical::Abajo)
        ));
        let t = escena.buscar(rotulo).unwrap();
        assert_eq!(
            t.extras.alineacion_vertical,
            Some(AlineacionVertical::Abajo)
        );
        // Pegado al fondo del hueco: 100 de alto menos 5 de aire.
        assert!((t.y + t.alto - 95.0).abs() < 0.01, "{}", t.y);
        assert!(aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::Alineacion(AlineacionTexto::Derecha)
        ));
        let t = escena.buscar(rotulo).unwrap();
        assert!((t.x + t.ancho - 195.0).abs() < 0.01, "{}", t.x);
        // Un paso por boton: deshacer el ultimo lo devuelve a la izquierda.
        assert!(escena.deshacer());
        let t = escena.buscar(rotulo).unwrap();
        assert_eq!(t.extras.alineacion, None);
        assert!((t.x - 5.0).abs() < 0.01);
        // Caso negativo: «izquierda» en un texto que no la trae ya es lo que
        // se ve; no hay nada que cambiar.
        assert!(!aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::Alineacion(AlineacionTexto::Izquierda)
        ));
    }

    #[test]
    fn las_puntas_y_el_codo_se_cambian_cada_uno_por_su_lado() {
        let mut escena = Escena::nueva();
        let id = con(&mut escena, flecha());
        let sel = sel_de(id);
        aplicar_forma(
            &mut escena,
            &sel,
            CambioForma::PuntaInicio(TipoPunta::Circulo),
        );
        aplicar_forma(&mut escena, &sel, CambioForma::PuntaFin(TipoPunta::Ninguna));
        aplicar_forma(&mut escena, &sel, CambioForma::Codos(true));
        let Figura::Flecha {
            punta_inicio,
            punta_fin,
            codos,
            puntos,
        } = &escena.buscar(id).unwrap().figura
        else {
            panic!("sigue siendo flecha");
        };
        assert_eq!(*punta_inicio, TipoPunta::Circulo);
        assert_eq!(*punta_fin, TipoPunta::Ninguna);
        assert!(*codos);
        // Los puntos no se tocan: la escalera se deriva al pintar.
        assert_eq!(puntos.len(), 2);
    }

    #[test]
    fn pedir_la_punta_que_ya_tiene_no_abre_un_paso_de_deshacer() {
        let mut escena = Escena::nueva();
        let id = con(&mut escena, flecha());
        assert!(!aplicar_forma(
            &mut escena,
            &sel_de(id),
            CambioForma::PuntaFin(TipoPunta::Flecha)
        ));
        // El ultimo paso sigue siendo el de haberla anadido: deshacer una
        // vez la quita.
        assert!(escena.deshacer());
        assert!(escena.buscar(id).is_none_or(|e| e.borrado));
    }

    #[test]
    fn el_tamano_de_letra_estira_la_caja_en_proporcion() {
        let mut escena = Escena::nueva();
        let id = con(&mut escena, texto(20.0));
        {
            let e = escena.buscar_mut(id).unwrap();
            e.ancho = 100.0;
            e.alto = 26.0;
        }
        let version = escena.buscar(id).unwrap().version;
        aplicar_forma(&mut escena, &sel_de(id), CambioForma::TamanoLetra(28.0));
        let e = escena.buscar(id).unwrap();
        let Figura::Texto { tam, .. } = &e.figura else {
            panic!("sigue siendo texto")
        };
        assert_eq!(*tam, 28.0);
        assert!((e.ancho - 140.0).abs() < 0.01 && (e.alto - 36.4).abs() < 0.01);
        assert!(e.version > version, "sin version la cache no repinta");
    }

    #[test]
    fn la_familia_se_guarda_por_su_nombre_y_viaja_por_su_numero() {
        let mut escena = Escena::nueva();
        let id = con(&mut escena, texto(20.0));
        aplicar_forma(&mut escena, &sel_de(id), CambioForma::Familia(8));
        let Figura::Texto { familia, .. } = &escena.buscar(id).unwrap().figura else {
            panic!("sigue siendo texto")
        };
        assert_eq!(crate::texto::numero_de_familia(familia), 8);
        // Caso negativo: a un rectangulo no se le pone letra.
        let caja = rect(&mut escena);
        assert!(!aplicar_forma(
            &mut escena,
            &sel_de(caja),
            CambioForma::Familia(6)
        ));
    }

    #[test]
    fn la_presion_de_un_trazo_viejo_estrena_opciones_sin_engordar() {
        use crate::tinta::{FACTOR_VARIABLE, Variabilidad};
        let mut escena = Escena::nueva();
        let id = con(
            &mut escena,
            Figura::Lapiz {
                puntos: Vec::new(),
                presiones: Vec::new(),
                opciones: None,
            },
        );
        escena.buscar_mut(id).unwrap().grosor = 8.0;
        assert_eq!(
            variabilidad_de(escena.buscar(id).unwrap()),
            Some(Variabilidad::Variable)
        );
        assert!(aplicar_forma(
            &mut escena,
            &sel_de(id),
            CambioForma::Presion(Variabilidad::Constante)
        ));
        let e = escena.buscar(id).unwrap();
        assert_eq!(variabilidad_de(e), Some(Variabilidad::Constante));
        assert!((e.grosor - 8.0 / FACTOR_VARIABLE).abs() < 1e-4);
    }

    #[test]
    fn pedir_pluma_variable_apaga_la_presion_firme_del_movil() {
        use crate::tinta::{OpcionesTinta, Variabilidad};
        let mut escena = Escena::nueva();
        let id = con(
            &mut escena,
            Figura::Lapiz {
                puntos: Vec::new(),
                presiones: Vec::new(),
                opciones: Some(OpcionesTinta::default()),
            },
        );
        escena.buscar_mut(id).unwrap().extras.presion_firme = true;
        // Con la presion firme puesta el trazo ya es constante aunque sus
        // opciones digan variable: eso es lo que el panel tiene que marcar.
        assert_eq!(
            variabilidad_de(escena.buscar(id).unwrap()),
            Some(Variabilidad::Constante)
        );
        assert!(aplicar_forma(
            &mut escena,
            &sel_de(id),
            CambioForma::Presion(Variabilidad::Variable)
        ));
        let e = escena.buscar(id).unwrap();
        assert!(!e.extras.presion_firme);
        assert_eq!(variabilidad_de(e), Some(Variabilidad::Variable));
    }
}
