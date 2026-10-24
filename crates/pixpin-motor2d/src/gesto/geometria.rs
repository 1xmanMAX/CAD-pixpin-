//! **Las herramientas de lamina en el gesto**: el arco exacto (por centro y
//! dos puntos, o por tres puntos), el arco repasado sobre una guia (el
//! transportador del movil) y la marca de angulo de un clic.
//!
//! Van aparte de `gesto.rs` porque son tres maquinitas de clics que solo se
//! cuelgan de tres sitios (`pulsar`, `mover` y `soltar`): asi el gesto de
//! siempre no cambia, y lo de aqui se lee de una vez. Las figuras salen de
//! `geometria_exacta`, las mismas que usa el asistente.
//!
//! # El arco
//!
//! - **Sobre una guia** ([`ModoArco::Guia`], el de fabrica, como el movil):
//!   con el ovalo o el arco puesto, se pulsa sobre el borde de un ovalo de
//!   referencia (o de una guia de arco sin repasar) y se arrastra: queda el
//!   trozo repasado (`empezarArcoSobre` y `Gesture.Arcoing` del movil). En
//!   cualquier otro sitio, la herramienta hace lo de siempre.
//! - **Centro y dos puntos** ([`ModoArco::CentroYPuntos`]): clic en el centro,
//!   clic donde empieza (da el radio) y clic donde acaba; mientras, el arco
//!   sigue al cursor en el sentido en que se mueva.
//! - **Tres puntos** ([`ModoArco::TresPuntos`]): donde empieza, uno por donde
//!   pasa y donde acaba.
//!
//! Todos los clics van con el iman: el centro de una circunferencia, el cruce
//! con una recta, el extremo de un lado...
//!
//! # El angulo
//!
//! Un clic dentro del angulo, cerca de donde se juntan dos rayas: la marca
//! sale entre los dos lados que encierran el clic, con el radio de la
//! distancia del clic al vertice. Si es recto, el cuadradito.

use super::{Estado, FormaCursor, Gesto, Herramienta, Region, Respuesta};
use crate::elemento::{ColorRgba, Elemento, Figura};
use crate::escena::Escena;
use crate::geometria_exacta as ge;
use crate::vector::Punto2;

pub use crate::geometria_exacta::ModoArco;

/// Los clics que lleva la herramienta Arco en sus modos de clics.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClicsDeArco {
    puntos: [Punto2; 2],
    cuantos: usize,
    /// El arco que se ensena mientras se elige el ultimo punto.
    previa: Option<u64>,
    /// Centro y dos puntos: el barrido que lleva y el angulo del cursor de la
    /// ultima vez, para seguir el sentido del raton (`barrido_acumulado`).
    barrido: f32,
    angulo_previo: f32,
}

impl Default for ClicsDeArco {
    fn default() -> Self {
        Self {
            puntos: [Punto2::nuevo(0.0, 0.0); 2],
            cuantos: 0,
            previa: None,
            barrido: 0.0,
            angulo_previo: 0.0,
        }
    }
}

/// A cuantos pixeles de pantalla del borde de una guia se empieza a repasar
/// (`UMBRAL_GUIA` del movil: mas ancho que el de picar).
const UMBRAL_GUIA_PX: f32 = 12.0;

/// Por debajo de este barrido, un arco repasado no deja nada
/// (`MIN_ARCO_BARRIDO` del movil).
const MIN_BARRIDO: f32 = 0.05;

/// A cuantos pixeles de pantalla del vertice se busca el angulo.
const ALCANCE_ANGULO_PX: f32 = 60.0;

fn respuesta() -> Respuesta {
    Respuesta {
        region: Region::Todo,
        cursor: FormaCursor::Cruz,
        pide: None,
    }
}

impl Gesto {
    /// El elemento de fabrica de la herramienta con la geometria de `g`
    /// (color, grosor, trazo, relleno, cierre y degradado del panel).
    fn con_estilo(&self, g: Elemento) -> Elemento {
        let plantilla = self.nuevo_elemento(Punto2::nuevo(g.x, g.y));
        let es_marca = g.extras.marca_angulo.is_some();
        Elemento {
            figura: g.figura,
            x: g.x,
            y: g.y,
            ancho: g.ancho,
            alto: g.alto,
            extras: crate::elemento::Extras {
                marca_angulo: g
                    .extras
                    .marca_angulo
                    .or(plantilla.extras.marca_angulo.clone()),
                cierre: if es_marca {
                    None
                } else {
                    plantilla.extras.cierre
                },
                ..plantilla.extras.clone()
            },
            ..plantilla
        }
    }

    /// La guia (ovalo de referencia o arco sin repasar) bajo `p`, si la hay.
    fn guia_en(&self, p: Punto2, escena: &Escena, escala: f32) -> Option<u64> {
        let radio = UMBRAL_GUIA_PX * escala.max(0.0001);
        escena
            .elementos
            .iter()
            .rev()
            .filter(|e| !e.borrado && !e.bloqueado)
            .filter(|e| match e.figura {
                Figura::Elipse => e.extras.referencia,
                Figura::Arco { barrido, .. } => e.extras.referencia || barrido.is_none(),
                _ => false,
            })
            .find(|e| crate::perimetros::punto_en_el_perimetro(e, p, radio).is_some())
            .map(|e| e.id)
    }

    /// **El pulsar de las herramientas de lamina.** `None` si no es para
    /// ellas, y el gesto sigue como siempre. El paso de deshacer ya esta
    /// abierto (lo abre `pulsar`) y lo cierra `soltar`.
    pub(super) fn pulsar_geometria(
        &mut self,
        p_crudo: Punto2,
        escena: &mut Escena,
        escala: f32,
    ) -> Option<Respuesta> {
        match self.herramienta {
            Herramienta::Angulo => {
                let alcance = ALCANCE_ANGULO_PX * escala.max(0.0001);
                let vivos: Vec<Elemento> = escena
                    .elementos
                    .iter()
                    .filter(|e| !e.borrado && e.extras.marca_angulo.is_none())
                    .cloned()
                    .collect();
                if let Some((v, a, b)) = ge::angulo_en(&vivos, p_crudo, alcance) {
                    let r = v.distancia(p_crudo).clamp(8.0, 120.0);
                    // Con letra: la siguiente griega libre del dibujo entero.
                    let usadas = ge::letras_de_angulo_usadas(&escena.elementos, None);
                    let marca = self.marca_con_estilo(ge::marca_con_modo(
                        self.estilo.modo_angulo,
                        v,
                        a,
                        b,
                        r,
                        self.estilo.arcos_angulo,
                        &usadas,
                    ));
                    let id = escena.anadir(marca);
                    self.seleccion.limpiar();
                    self.ultima_marca = Some(id);
                }
                Some(respuesta())
            }
            Herramienta::Arco if self.estilo.modo_arco != ModoArco::Guia => {
                let p = self.enganchar(p_crudo, escena, escala);
                let mut c = self.clics_arco;
                match c.cuantos {
                    0 | 1 => {
                        c.puntos[c.cuantos] = p;
                        c.cuantos += 1;
                        if c.cuantos == 2 && self.estilo.modo_arco == ModoArco::CentroYPuntos {
                            c.angulo_previo = ge::angulo_de(c.puntos[0], p);
                            c.barrido = 0.0;
                        }
                        self.clics_arco = c;
                    }
                    _ => {
                        let final_ = self.arco_de_los_clics(&mut c, p);
                        // El arco de muestra se va (con su paso) y entra el
                        // de verdad en el paso de este clic: un solo Ctrl+Z.
                        if c.previa.is_some() {
                            escena.cancelar_paso();
                            escena.deshacer();
                            escena.abrir_paso();
                        }
                        if let Some(a) = final_ {
                            let id = escena.anadir(self.con_estilo(a));
                            self.seleccion.limpiar();
                            let _ = id;
                        }
                        self.clics_arco = ClicsDeArco::default();
                    }
                }
                Some(respuesta())
            }
            Herramienta::Arco | Herramienta::Elipse => {
                let guia = self.guia_en(p_crudo, escena, escala)?;
                let g = escena.buscar(guia)?.clone();
                let p = self.enganchar(p_crudo, escena, escala);
                let angulo =
                    crate::arco::angulo_en_el_ovalo((g.x, g.y, g.ancho, g.alto), g.angulo, p);
                let mut arco = self.con_estilo(Elemento {
                    figura: Figura::Arco {
                        inicio: angulo,
                        barrido: Some(0.0),
                    },
                    x: g.x,
                    y: g.y,
                    ancho: g.ancho,
                    alto: g.alto,
                    ..Default::default()
                });
                arco.angulo = g.angulo;
                // Lo repasado es dibujo, aunque la guia no lo sea.
                arco.extras.referencia = self.modo_guia();
                let id = escena.anadir(arco);
                self.seleccion.limpiar();
                self.estado = Estado::Dibujando { id };
                self.arqueando = Some(angulo);
                Some(respuesta())
            }
            _ => None,
        }
    }

    /// La marca de angulo con el color del panel y su relleno translucido.
    fn marca_con_estilo(&self, mut m: Elemento) -> Elemento {
        m.trazo = self.estilo.trazo;
        m.opacidad = self.estilo.opacidad;
        m.estilo = self.estilo.estilo;
        m.relleno = Some(self.estilo.relleno.unwrap_or(ColorRgba {
            a: 0.3,
            ..self.estilo.trazo
        }));
        m.extras.referencia = self.modo_guia();
        m
    }

    /// El arco que dejan los clics con el cursor en `p`.
    fn arco_de_los_clics(&self, c: &mut ClicsDeArco, p: Punto2) -> Option<Elemento> {
        if c.cuantos < 2 {
            return None;
        }
        let [a, b] = c.puntos;
        match self.estilo.modo_arco {
            ModoArco::CentroYPuntos => {
                let r = a.distancia(b);
                if r < 0.5 {
                    return None;
                }
                let ang = ge::angulo_de(a, p);
                c.barrido = crate::arco::barrido_acumulado(c.barrido, c.angulo_previo, ang);
                c.angulo_previo = ang;
                (c.barrido.abs() >= 1e-3).then(|| ge::arco(a, r, ge::angulo_de(a, b), c.barrido))
            }
            ModoArco::TresPuntos => ge::arco_por_tres_puntos(a, b, p),
            ModoArco::Guia => None,
        }
    }

    /// **El mover de las herramientas de lamina**: el repasado de la guia y
    /// el arco de muestra de los clics. `None` si no es para ellas.
    pub(super) fn mover_geometria(&mut self, p: Punto2, escena: &mut Escena) -> Option<Respuesta> {
        if let (Some(previo), Estado::Dibujando { id }) = (self.arqueando, self.estado) {
            let e = escena.buscar_mut(id)?;
            let angulo = crate::arco::angulo_en_el_ovalo((e.x, e.y, e.ancho, e.alto), e.angulo, p);
            if let Figura::Arco { barrido, .. } = &mut e.figura {
                *barrido = Some(crate::arco::barrido_acumulado(
                    barrido.unwrap_or(0.0),
                    previo,
                    angulo,
                ));
            }
            e.tocar();
            self.arqueando = Some(angulo);
            return Some(respuesta());
        }
        if self.herramienta != Herramienta::Arco
            || self.estilo.modo_arco == ModoArco::Guia
            || self.clics_arco.cuantos < 2
        {
            return None;
        }
        if !matches!(self.estado, Estado::Reposo) {
            return None;
        }
        let mut c = self.clics_arco;
        let nuevo = self.arco_de_los_clics(&mut c, p);
        match (c.previa, nuevo) {
            (Some(id), Some(a)) => {
                if let Some(e) = escena.buscar_mut(id) {
                    e.figura = a.figura;
                    e.x = a.x;
                    e.y = a.y;
                    e.ancho = a.ancho;
                    e.alto = a.alto;
                    e.tocar();
                }
            }
            (None, Some(a)) => {
                escena.abrir_paso();
                c.previa = Some(escena.anadir(self.con_estilo(a)));
                escena.cerrar_paso();
            }
            _ => {}
        }
        self.clics_arco = c;
        Some(Respuesta {
            region: Region::Todo,
            cursor: FormaCursor::Cruz,
            pide: None,
        })
    }

    /// **El soltar del repasado**: un barrido de nada no deja nada.
    pub(super) fn soltar_geometria(&mut self, escena: &mut Escena) {
        if self.arqueando.take().is_none() {
            return;
        }
        if let Estado::Dibujando { id } = self.estado
            && escena.buscar(id).is_some_and(|e| match e.figura {
                Figura::Arco { barrido, .. } => barrido.is_none_or(|b| b.abs() < MIN_BARRIDO),
                _ => false,
            })
        {
            escena.cancelar_paso();
        }
    }

    /// Escape a media secuencia de clics: se va el arco de muestra.
    pub(super) fn cancelar_geometria(&mut self, escena: &mut Escena) {
        if self.clics_arco.previa.is_some() {
            escena.cancelar_paso();
            escena.deshacer();
        }
        self.clics_arco = ClicsDeArco::default();
        self.arqueando = None;
    }

    /// Cuantos clics lleva la herramienta Arco (0, 1 o 2): para la ayuda de
    /// la barra («clic en el centro», «clic donde empieza»...).
    pub fn clics_de_arco(&self) -> usize {
        self.clics_arco.cuantos
    }

    /// Los puntos ya fijados de la herramienta Arco, para pintar sus marcas.
    pub fn puntos_de_arco(&self) -> &[Punto2] {
        &self.clics_arco.puntos[..self.clics_arco.cuantos]
    }

    /// **Las marcas de los clics ya puestos** del arco (el centro con su
    /// circulito, los demas con su aspa), para que quien pinta lo de encima
    /// ensene por donde va la secuencia. Vacio si no hay ninguna.
    pub fn ordenes_de_arco_en_curso(&self, zoom: f32) -> Vec<crate::pintado::Orden> {
        use crate::enganche::{Anclaje, TipoAnclaje, pista};
        if self.herramienta != Herramienta::Arco || self.estilo.modo_arco == ModoArco::Guia {
            return Vec::new();
        }
        self.puntos_de_arco()
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let tipo = if i == 0 && self.estilo.modo_arco == ModoArco::CentroYPuntos {
                    TipoAnclaje::Centro
                } else {
                    TipoAnclaje::Interseccion
                };
                pista(
                    &Anclaje {
                        punto: *q,
                        tipo,
                        id: 0,
                    },
                    zoom,
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// La letra griega de la marca de angulo (17-oct-2026)

impl Gesto {
    /// **La marca a la que le cambian la letra la rueda y las flechas**: la
    /// elegida (si lo elegido es una sola marca de angulo) o, con la
    /// herramienta Angulo en la mano, la ultima que se puso. `None` si no hay
    /// ninguna viva o es un recto (el cuadradito no lleva letra).
    pub fn marca_para_letra(&self, escena: &Escena) -> Option<u64> {
        let es_marca = |id: u64| {
            escena.buscar(id).is_some_and(|e| {
                !e.borrado && e.extras.marca_angulo.as_ref().is_some_and(|m| !m.recto)
            })
        };
        let ids = self.seleccion.ids();
        if ids.len() == 1 && es_marca(ids[0]) {
            return Some(ids[0]);
        }
        if !ids.is_empty() || self.herramienta != Herramienta::Angulo {
            return None;
        }
        self.ultima_marca.filter(|id| es_marca(*id))
    }

    /// **Cambia la letra griega** de [`Gesto::marca_para_letra`] a la de al
    /// lado (`paso` > 0 la siguiente, < 0 la anterior), saltando las que ya
    /// usan otras marcas del dibujo, en un paso de deshacer. `true` si cambio.
    pub fn cambiar_letra_de_angulo(&mut self, escena: &mut Escena, paso: i32) -> bool {
        let Some(id) = self.marca_para_letra(escena) else {
            return false;
        };
        let usadas = ge::letras_de_angulo_usadas(&escena.elementos, Some(id));
        let actual = escena
            .buscar(id)
            .and_then(|e| e.extras.marca_angulo.as_ref()?.valor.clone());
        let nueva = ge::letra_vecina(actual.as_deref(), paso, &usadas);
        if actual.as_deref() == Some(nueva.as_str()) {
            return false;
        }
        escena.abrir_paso();
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            if let Some(m) = e.extras.marca_angulo.as_mut() {
                m.valor = Some(nueva);
            }
            e.tocar();
        }
        escena.cerrar_paso();
        true
    }

    /// **La variante de 90 grados** de la herramienta Angulo: pulsar su
    /// atajo con ella ya en la mano pasa del cuadradito a la letra griega y
    /// al reves (como coger otra vez el mismo lapiz en el movil cambia su
    /// punta). Devuelve el modo que queda.
    pub fn alternar_angulo_recto(&mut self) -> ge::ModoAngulo {
        self.estilo.modo_angulo = match self.estilo.modo_angulo {
            ge::ModoAngulo::Recto => ge::ModoAngulo::Griega,
            _ => ge::ModoAngulo::Recto,
        };
        self.estilo.modo_angulo
    }
}
