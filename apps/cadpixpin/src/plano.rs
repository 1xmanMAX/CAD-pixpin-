//! **El plano en la tarjeta grafica**: el dibujo se pasa a rayas y
//! triangulos con el convertidor de `pixpin-cad` (el mismo del visor de
//! PixPin) en otros hilos y se sube cuando llega. Va partido en cubos
//! ([`crate::cubos`]): al editar solo se rehacen los cubos tocados, y
//! mientras tanto se ve lo de antes. La camara es la de la mesa, en
//! coordenadas absolutas del dibujo.

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use cad_nucleo::Dibujo;
use cad_nucleo::geo::{self, Caja, P};
use cad_nucleo::opencadcodec::tables::{Layer, Table};
use cad_nucleo::opencadcodec::{CadDocument, EntityType};
use pixpin_cad::convertir::Convertidor;
use pixpin_cad::gpu::{Gpu, PlanoGpu, Vista};
use pixpin_cad::modelo::Modelo;
use pixpin_cad::texto::Textos;

use crate::cubos::{self, CAPACIDAD, CRECER, Cambio, Reparto};

/// Que punto del dibujo esta en el centro de la ventana y cuantas unidades
/// mide un pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camara {
    pub centro: P,
    pub px: f64,
}

impl Camara {
    pub fn plano(&self, x: f64, y: f64, w: f64, h: f64) -> P {
        [
            self.centro[0] + (x - w / 2.0) * self.px,
            self.centro[1] - (y - h / 2.0) * self.px,
        ]
    }

    pub fn pantalla(&self, p: P, w: f64, h: f64) -> (f32, f32) {
        (
            ((p[0] - self.centro[0]) / self.px + w / 2.0) as f32,
            (h / 2.0 - (p[1] - self.centro[1]) / self.px) as f32,
        )
    }

    /// Acercar `factor` (<1 acerca) dejando quieto el punto bajo (x, y).
    pub fn zoom(&self, factor: f64, x: f64, y: f64, w: f64, h: f64) -> Camara {
        let antes = self.plano(x, y, w, h);
        let mut c = Camara {
            centro: self.centro,
            px: (self.px * factor).clamp(1e-9, 1e9),
        };
        let despues = c.plano(x, y, w, h);
        c.centro = [
            c.centro[0] + antes[0] - despues[0],
            c.centro[1] + antes[1] - despues[1],
        ];
        c
    }

    /// La caja entera dentro de la zona `zona` (x, y, ancho, alto en pixeles)
    /// de una ventana `w` x `h`.
    pub fn encuadrar(caja: Caja, zona: [f64; 4], w: f64, h: f64) -> Camara {
        let (cw, ch) = ((caja[2] - caja[0]).max(1e-9), (caja[3] - caja[1]).max(1e-9));
        let mut px = (cw / (zona[2] * 0.9).max(1.0)).max(ch / (zona[3] * 0.9).max(1.0));
        if !(px.is_finite() && px > 0.0) {
            px = 1.0;
        }
        // El centro de la caja en el centro de la zona (no de la ventana).
        let medio = [(caja[0] + caja[2]) / 2.0, (caja[1] + caja[3]) / 2.0];
        let (zx, zy) = (zona[0] + zona[2] / 2.0, zona[1] + zona[3] / 2.0);
        Camara {
            centro: [
                medio[0] - (zx - w / 2.0) * px,
                medio[1] + (zy - h / 2.0) * px,
            ],
            px,
        }
    }
}

// ------------------------------------------------------------ los cubos

/// Un pedido al hilo que convierte.
enum Pedido {
    /// Todo de nuevo, con una copia del documento.
    Todo { version: u64, doc: Box<CadDocument> },
    /// Lo que cambio desde el pedido anterior, con las capas de ahora (al
    /// apagar o bloquear una capa solo llegan sus entidades: el hilo pone
    /// estas capas en su copia del documento para convertirlas bien).
    Cambios {
        version: u64,
        cambios: Vec<Cambio>,
        capas: Box<Table<Layer>>,
        /// Una copia del documento cuando cambio algo de dentro de un bloque
        /// (el `*Dn` de una cota): el hilo la necesita para dibujar sus
        /// INSERT y cotas, y rehace solo los cubos donde estan.
        doc: Option<Box<CadDocument>>,
    },
}

/// **Un cubo ya convertido**, partido en lo de abajo (rellenos y
/// sombreados) y lo de encima (rayas, arcos y letras): se dibujan todos los
/// fondos antes que cualquier raya, como cuando el plano era un solo modelo.
struct Hecho {
    fondo: Option<Modelo>,
    encima: Option<Modelo>,
    /// Todo lo que dibuja (para saltarse el cubo si no se ve) y lo que
    /// importa (para encuadrar), en coordenadas del dibujo.
    caja: Caja,
    util: Caja,
}

/// Un cubo convertido (`None`: no dibuja nada) o por que no se pudo.
type Convertido = Result<Option<Hecho>, String>;

enum Llegado {
    /// Todos los cubos (la particion puede ser otra).
    Todo {
        version: u64,
        cubos: Vec<Convertido>,
    },
    /// Solo estos; ahora hay `total`.
    Cubos {
        version: u64,
        cubos: Vec<(usize, Convertido)>,
        total: usize,
    },
    /// Lo que cambio en esta version no se puede hacer por partes.
    PideTodo(u64),
}

/// Un cubo en la tarjeta: cada parte con su origen.
struct CuboGpu {
    caja: Caja,
    util: Caja,
    fondo: Option<(PlanoGpu, [f64; 2])>,
    encima: Option<(PlanoGpu, [f64; 2])>,
}

/// **El plano en la tarjeta**, en cubos.
pub struct PlanoSubido {
    cubos: Vec<Option<CuboGpu>>,
    /// Lo que importa del plano, para encuadrar.
    pub caja: Option<Caja>,
}

impl PlanoSubido {
    /// **Lo que hay que dibujar** con la camara `c` en una ventana `w` x `h`:
    /// los cubos que se ven, primero sus fondos y luego sus rayas.
    pub fn planos(&self, c: &Camara, w: u32, h: u32) -> Vec<(&PlanoGpu, Vista)> {
        let vista = caja_de_camara(c, w, h);
        let visibles: Vec<&CuboGpu> = self
            .cubos
            .iter()
            .flatten()
            .filter(|k| geo::se_tocan(&k.caja, &vista))
            .collect();
        let mut v = Vec::with_capacity(visibles.len() * 2);
        for fondo in [true, false] {
            for k in &visibles {
                if let Some((p, origen)) = if fondo { &k.fondo } else { &k.encima } {
                    v.push((p, vista_de(*origen, c, w, h)));
                }
            }
        }
        v
    }

    fn rehacer_caja(&mut self) {
        let mut total = geo::CAJA_VACIA;
        for k in self.cubos.iter().flatten() {
            total = geo::unir_cajas(total, k.util);
        }
        self.caja = geo::caja_valida(&total).then_some(total);
    }
}

/// Lo que ve la camara, en coordenadas del dibujo (con un margen de un par
/// de pixeles).
pub fn caja_de_camara(c: &Camara, w: u32, h: u32) -> Caja {
    let (mx, my) = (
        (w.max(1) as f64 / 2.0 + 2.0) * c.px,
        (h.max(1) as f64 / 2.0 + 2.0) * c.px,
    );
    [
        c.centro[0] - mx,
        c.centro[1] - my,
        c.centro[0] + mx,
        c.centro[1] + my,
    ]
}

/// **El plano subido** y el hilo que lo convierte.
pub struct Plano {
    pedir: Sender<Pedido>,
    llega: Receiver<Llegado>,
    /// La version pedida por ultima vez.
    pub pedida: u64,
    /// La version que se ve.
    pub vista: u64,
    /// La version del ultimo pedido entero (lo que el hilo pida de antes ya
    /// esta pedido).
    entero: u64,
    /// El hilo pidio convertirlo todo.
    rehacer: bool,
    pub gpu: Option<PlanoSubido>,
    pub error: Option<String>,
}

impl Plano {
    pub fn nuevo() -> Plano {
        let (pedir, rx) = channel::<Pedido>();
        let (tx, llega) = channel::<Llegado>();
        let _ = std::thread::Builder::new()
            .name("plano-convertir".into())
            .spawn(move || convertir_en_cubos(rx, tx));
        Plano {
            pedir,
            llega,
            pedida: 0,
            vista: 0,
            entero: 0,
            rehacer: false,
            gpu: None,
            error: None,
        }
    }

    /// **Pide dibujar el dibujo como esta**, si aun no se pidio: solo lo que
    /// cambio desde el pedido anterior (si se sabe) o todo.
    pub fn pedir(&mut self, d: &Dibujo) {
        let v = d.version();
        if v == self.pedida && !self.rehacer {
            return;
        }
        let cambios = if self.rehacer || self.pedida == 0 {
            None
        } else {
            d.cambios_desde(self.pedida)
        };
        let pedido = match cambios {
            Some(hs) => {
                let cambios: Vec<Cambio> = hs
                    .into_iter()
                    .map(|h| (h, d.doc.get_entity_arc(h)))
                    .collect();
                let de_bloque = cambios.iter().any(|(_, e)| {
                    e.as_deref()
                        .is_some_and(|e| !cubos::es_de_modelo(&d.doc, e))
                });
                Pedido::Cambios {
                    version: v,
                    cambios,
                    capas: Box::new(d.doc.layers.clone()),
                    doc: de_bloque.then(|| Box::new(d.doc.clone())),
                }
            }
            None => {
                self.entero = v;
                self.rehacer = false;
                Pedido::Todo {
                    version: v,
                    doc: Box::new(d.doc.clone()),
                }
            }
        };
        self.pedida = v;
        let _ = self.pedir.send(pedido);
    }

    /// Si esta convirtiendo (para seguir mirando el canal).
    pub fn esperando(&self) -> bool {
        self.pedida != self.vista || self.rehacer
    }

    /// **Sube lo que haya llegado**; hasta entonces se ve lo de antes. Dice
    /// si cambio lo que se ve.
    pub fn recibir(&mut self, gpu: &Gpu) -> bool {
        let mut cambio = false;
        while let Ok(l) = self.llega.try_recv() {
            cambio = true;
            match l {
                Llegado::PideTodo(v) => {
                    if v > self.entero {
                        self.rehacer = true;
                    }
                }
                Llegado::Todo { version, cubos } => {
                    self.vista = version;
                    self.error = None;
                    let mut nuevo = PlanoSubido {
                        cubos: Vec::with_capacity(cubos.len()),
                        caja: None,
                    };
                    for c in cubos {
                        let k = self.subir(gpu, c);
                        nuevo.cubos.push(k);
                    }
                    nuevo.rehacer_caja();
                    self.gpu = Some(nuevo);
                }
                Llegado::Cubos {
                    version,
                    cubos,
                    total,
                } => {
                    self.vista = version;
                    let Some(mut g) = self.gpu.take() else {
                        self.rehacer = true;
                        continue;
                    };
                    g.cubos.resize_with(total, || None);
                    for (k, c) in cubos {
                        let subido = self.subir(gpu, c);
                        if let Some(x) = g.cubos.get_mut(k) {
                            *x = subido;
                        }
                    }
                    g.rehacer_caja();
                    self.gpu = Some(g);
                }
            }
        }
        cambio
    }

    fn subir(&mut self, gpu: &Gpu, c: Convertido) -> Option<CuboGpu> {
        let h = match c {
            Ok(h) => h?,
            Err(e) => {
                self.error = Some(e);
                return None;
            }
        };
        let mut parte = |m: Option<Modelo>| -> Option<(PlanoGpu, [f64; 2])> {
            let m = m?;
            match gpu.subir(&m) {
                Ok(p) => Some((p, m.origen)),
                Err(e) => {
                    self.error = Some(format!("el plano no cabe en la tarjeta: {e}"));
                    None
                }
            }
        };
        let fondo = parte(h.fondo);
        let encima = parte(h.encima);
        Some(CuboGpu {
            caja: h.caja,
            util: h.util,
            fondo,
            encima,
        })
    }
}

/// Lo que el hilo sabe del dibujo: el documento del ultimo dibujo entero
/// (sus bloques, capas y estilos) y las entidades de cada cubo, al dia.
struct Estado {
    base: Arc<CadDocument>,
    reparto: Reparto,
    /// Si se puede rehacer por partes (no, si se dibuja el espacio papel).
    por_partes: bool,
}

/// **El hilo que convierte**: atiende los pedidos en orden (juntando los
/// que se acumulan) y reparte los cubos entre los obreros.
fn convertir_en_cubos(rx: Receiver<Pedido>, tx: Sender<Llegado>) {
    let obreros = Obreros::nuevos();
    let mut estado: Option<Estado> = None;
    // Sube con cada dibujo entero: los obreros olvidan las letras de antes.
    let mut generacion = 0u64;
    while let Ok(p) = rx.recv() {
        let mut cola = vec![p];
        while let Ok(q) = rx.try_recv() {
            cola.push(q);
        }
        for p in juntar(cola) {
            let r = match p {
                Pedido::Todo { version, doc } => {
                    generacion += 1;
                    let base: Arc<CadDocument> = Arc::from(doc);
                    let (espacio, papel) = Convertidor::espacio(&base);
                    let por_partes = !papel || espacio.is_empty();
                    let entidades: Vec<Arc<EntityType>> = espacio
                        .iter()
                        .filter_map(|e| base.get_entity_arc(e.common().handle))
                        .collect();
                    let reparto = Reparto::nuevo(entidades, CAPACIDAD);
                    let cubos = obreros.todos(&base, &reparto, generacion);
                    estado = Some(Estado {
                        base,
                        reparto,
                        por_partes,
                    });
                    Llegado::Todo { version, cubos }
                }
                Pedido::Cambios {
                    version,
                    cambios,
                    capas,
                    doc,
                } => {
                    let viejo = estado.as_ref().map(|est| est.base.clone());
                    let con_doc = doc.is_some();
                    if let Some(est) = estado.as_mut() {
                        if let Some(d) = doc {
                            est.base = Arc::from(d);
                        } else if est.base.layers != *capas {
                            // Los obreros ya soltaron su copia: no se clona el
                            // documento, solo se cambian sus capas.
                            Arc::make_mut(&mut est.base).layers = *capas;
                        }
                    }
                    // Lo de dentro de un bloque no va a ningun cubo: cuenta
                    // por los bloques que cambian.
                    let mut bloques: HashSet<String> = HashSet::new();
                    let mut modelo: Vec<Cambio> = Vec::with_capacity(cambios.len());
                    let mut imposible = false;
                    if let (Some(est), Some(viejo)) = (estado.as_ref(), viejo.as_ref()) {
                        for (h, e) in cambios {
                            if est.reparto.conoce(h)
                                || e.as_deref()
                                    .is_some_and(|e| cubos::es_de_modelo(&est.base, e))
                            {
                                modelo.push((h, e));
                                continue;
                            }
                            let duena = match &e {
                                Some(e) => Some(e.common().owner_handle),
                                None => viejo.get_entity(h).map(|v| v.common().owner_handle),
                            };
                            // Una que nunca se vio (anadida y quitada entre
                            // dos pedidos) no cambia nada.
                            let Some(duena) = duena else { continue };
                            match cubos::bloque_de(&est.base, duena)
                                .or_else(|| cubos::bloque_de(viejo, duena))
                            {
                                Some(n) => {
                                    bloques.insert(n);
                                }
                                None => imposible = true,
                            }
                        }
                    } else {
                        imposible = true;
                    }
                    // Sin el documento de ahora no se pueden dibujar los bloques
                    // cambiados, salvo que ya no los use nadie (se quitaron a la vez).
                    if !bloques.is_empty() && !con_doc {
                        let quitadas: HashSet<_> = modelo
                            .iter()
                            .filter(|(_, e)| e.is_none())
                            .map(|(h, _)| *h)
                            .collect();
                        if estado
                            .as_ref()
                            .is_none_or(|est| cubos::alguien_usa(&est.reparto, &bloques, &quitadas))
                        {
                            imposible = true;
                        } else {
                            bloques.clear();
                        }
                    }
                    let cambios = modelo;
                    let se_puede = !imposible
                        && estado.as_ref().is_some_and(|est| {
                            est.por_partes
                                && cambios.iter().all(|(h, e)| {
                                    cubos::admite(&est.base, &est.reparto, *h, e.as_deref())
                                })
                        });
                    match estado.as_mut() {
                        Some(est) if se_puede => {
                            let mut tocados = est.reparto.aplicar(cambios);
                            if !bloques.is_empty() {
                                tocados.extend(cubos::dependientes(
                                    &est.base,
                                    &est.reparto,
                                    &bloques,
                                ));
                            }
                            if est.reparto.mayor() > CRECER * CAPACIDAD {
                                // Se dibujo mucho en un sitio: se parte otra vez.
                                let r = std::mem::replace(
                                    &mut est.reparto,
                                    Reparto::nuevo(Vec::new(), CAPACIDAD),
                                );
                                est.reparto = Reparto::nuevo(r.todas(), CAPACIDAD);
                                let cubos = obreros.todos(&est.base, &est.reparto, generacion);
                                Llegado::Todo { version, cubos }
                            } else {
                                let cubos = obreros.convertir(
                                    &est.base,
                                    &est.reparto.cubos,
                                    tocados,
                                    generacion,
                                );
                                Llegado::Cubos {
                                    version,
                                    cubos,
                                    total: est.reparto.cubos.len(),
                                }
                            }
                        }
                        _ => {
                            tracing::info!(
                                version,
                                "plano: el cambio no va por partes, todo de nuevo"
                            );
                            estado = None;
                            Llegado::PideTodo(version)
                        }
                    }
                }
            };
            if tx.send(r).is_err() {
                return;
            }
        }
    }
}

/// **Junta los pedidos acumulados**: desde el ultimo entero, y los cambios
/// seguidos en uno (de cada entidad, como quedo al final).
fn juntar(cola: Vec<Pedido>) -> Vec<Pedido> {
    let desde = cola
        .iter()
        .rposition(|p| matches!(p, Pedido::Todo { .. }))
        .unwrap_or(0);
    let mut out: Vec<Pedido> = Vec::new();
    for p in cola.into_iter().skip(desde) {
        match (out.last_mut(), p) {
            (
                Some(Pedido::Cambios {
                    version,
                    cambios,
                    capas,
                    doc,
                }),
                Pedido::Cambios {
                    version: v,
                    cambios: nuevos,
                    capas: c,
                    doc: d,
                },
            ) => {
                *version = v;
                *capas = c;
                if d.is_some() {
                    *doc = d;
                }
                cubos::juntar_cambios(cambios, nuevos);
            }
            (_, p) => out.push(p),
        }
    }
    out
}

/// Un cubo que convertir.
struct Tarea {
    doc: Arc<CadDocument>,
    k: usize,
    entidades: Vec<Arc<EntityType>>,
    generacion: u64,
    vuelta: Sender<(usize, Convertido)>,
}

/// **Los hilos que convierten los cubos**: uno menos que los del equipo
/// (la ventana tambien quiere el suyo), de uno a cuatro. Cada uno guarda
/// sus letras ya hechas para el siguiente cubo.
struct Obreros {
    tareas: Sender<Tarea>,
}

impl Obreros {
    fn nuevos() -> Obreros {
        let (tareas, rx) = channel::<Tarea>();
        let rx = Arc::new(Mutex::new(rx));
        let n = std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .saturating_sub(1)
            .clamp(1, 4);
        for i in 0..n {
            let rx = rx.clone();
            let _ = std::thread::Builder::new()
                .name(format!("plano-cubo-{i}"))
                // El convertidor recorre bloques dentro de bloques: pila holgada.
                .stack_size(64 * 1024 * 1024)
                .spawn(move || obrero(&rx));
        }
        Obreros { tareas }
    }

    fn todos(&self, doc: &Arc<CadDocument>, r: &Reparto, generacion: u64) -> Vec<Convertido> {
        let n = r.cubos.len();
        let mut out: Vec<Convertido> = (0..n).map(|_| Ok(None)).collect();
        for (k, c) in self.convertir(doc, &r.cubos, 0..n, generacion) {
            out[k] = c;
        }
        out
    }

    /// Convierte los cubos `cuales` (los grandes primero, para repartir
    /// mejor) y espera a que esten todos.
    fn convertir(
        &self,
        doc: &Arc<CadDocument>,
        cubos: &[Vec<Arc<EntityType>>],
        cuales: impl IntoIterator<Item = usize>,
        generacion: u64,
    ) -> Vec<(usize, Convertido)> {
        let mut cuales: Vec<usize> = cuales.into_iter().filter(|k| *k < cubos.len()).collect();
        cuales.sort_by_key(|k| std::cmp::Reverse(cubos[*k].len()));
        let (vuelta, llegan) = channel();
        for k in cuales {
            let _ = self.tareas.send(Tarea {
                doc: doc.clone(),
                k,
                entidades: cubos[k].clone(),
                generacion,
                vuelta: vuelta.clone(),
            });
        }
        // Sin esta copia, el canal se cierra cuando acaban todas (o si un
        // obrero se muere).
        drop(vuelta);
        let mut hechos: Vec<(usize, Convertido)> = llegan.iter().collect();
        hechos.sort_by_key(|h| h.0);
        hechos
    }
}

fn obrero(rx: &Mutex<Receiver<Tarea>>) {
    let mut letras: Option<(u64, Textos)> = None;
    loop {
        let t = {
            let Ok(cola) = rx.lock() else { return };
            match cola.recv() {
                Ok(t) => t,
                Err(_) => return,
            }
        };
        let previas = letras
            .take()
            .filter(|(g, _)| *g == t.generacion)
            .map(|(_, l)| l);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut c = Convertidor::nuevo(&t.doc);
            if let Some(l) = previas {
                c = c.con_textos(l);
            }
            let (m, _, l) = c.convertir_estas(t.entidades.iter().map(|e| e.as_ref()));
            (partir_modelo(m), l)
        }));
        let r = match r {
            Ok((h, l)) => {
                letras = Some((t.generacion, l));
                Ok(h)
            }
            Err(_) => Err("el plano no se pudo dibujar".to_string()),
        };
        let _ = t.vuelta.send((t.k, r));
    }
}

/// **Parte un modelo** en lo de abajo y lo de encima, cada uno con solo
/// sus vertices.
fn partir_modelo(m: Modelo) -> Option<Hecho> {
    if m.vacio() {
        return None;
    }
    let caja = [
        m.caja[0] as f64 + m.origen[0],
        m.caja[1] as f64 + m.origen[1],
        m.caja[2] as f64 + m.origen[0],
        m.caja[3] as f64 + m.origen[1],
    ];
    let util = crate::mesa::caja_de_modelo(&m);
    let fondo = !m.triangulos.is_empty() || !m.triangulos_trama.is_empty();
    let encima = !m.lineas.is_empty() || !m.letras.is_empty() || !m.arcos.is_empty();
    let (fondo, encima) = match (fondo, encima) {
        (true, false) => (Some(m), None),
        (false, _) => (None, Some(m)),
        (true, true) => {
            let Modelo {
                origen,
                caja,
                vertices,
                lineas,
                triangulos,
                tramos_lineas,
                tramos_triangulos,
                vertices_trama,
                triangulos_trama,
                tramos_trama,
                tramas,
                familias,
                malla_letras,
                glifos,
                letras,
                tramos_letras,
                arcos,
                tramos_arcos,
                unidades,
                sin_dibujar,
            } = m;
            let (vf, tf) = cubos::compactar(&vertices, &triangulos);
            let (ve, le) = cubos::compactar(&vertices, &lineas);
            (
                Some(Modelo {
                    origen,
                    caja,
                    vertices: vf,
                    triangulos: tf,
                    tramos_triangulos,
                    vertices_trama,
                    triangulos_trama,
                    tramos_trama,
                    tramas,
                    familias,
                    unidades,
                    ..Default::default()
                }),
                Some(Modelo {
                    origen,
                    caja,
                    vertices: ve,
                    lineas: le,
                    tramos_lineas,
                    malla_letras,
                    glifos,
                    letras,
                    tramos_letras,
                    arcos,
                    tramos_arcos,
                    unidades,
                    sin_dibujar,
                    ..Default::default()
                }),
            )
        }
    };
    Some(Hecho {
        fondo,
        encima,
        caja,
        util,
    })
}

/// La vista del sombreador para un modelo con origen `origen` y la camara `c`.
pub fn vista_de(origen: [f64; 2], c: &Camara, w: u32, h: u32) -> Vista {
    Vista {
        escala: [
            (2.0 / (w.max(1) as f64 * c.px)) as f32,
            (2.0 / (h.max(1) as f64 * c.px)) as f32,
        ],
        centro: [
            (c.centro[0] - origen[0]) as f32,
            (c.centro[1] - origen[1]) as f32,
        ],
        // El color 7 (blanco o negro segun el fondo): blanco sobre oscuro.
        color7: [1.0; 4],
        px: c.px as f32,
        modo: 1.0,
        relleno: [0.0; 2],
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use cad_nucleo::opencadcodec::Handle;

    #[test]
    fn de_los_pedidos_acumulados_valen_desde_el_ultimo_entero_y_los_cambios_juntos() {
        let cambio = |v: u64, h: u64| Pedido::Cambios {
            version: v,
            cambios: vec![(Handle::new(h), None)],
            capas: Box::default(),
            doc: None,
        };
        let todo = |v: u64| Pedido::Todo {
            version: v,
            doc: Box::new(CadDocument::new()),
        };
        let resumen = |ps: Vec<Pedido>| -> Vec<(u64, usize)> {
            ps.iter()
                .map(|p| match p {
                    Pedido::Todo { version, .. } => (*version, usize::MAX),
                    Pedido::Cambios {
                        version, cambios, ..
                    } => (*version, cambios.len()),
                })
                .collect()
        };
        let j = juntar(vec![
            cambio(2, 1),
            todo(3),
            cambio(4, 1),
            cambio(5, 2),
            cambio(6, 1),
        ]);
        assert_eq!(resumen(j), vec![(3, usize::MAX), (6, 2)]);
        let j = juntar(vec![cambio(2, 1), cambio(3, 1)]);
        assert_eq!(resumen(j), vec![(3, 1)]);
    }

    #[test]
    fn la_caja_de_la_camara_es_lo_que_se_ve() {
        let c = Camara {
            centro: [100.0, 50.0],
            px: 0.5,
        };
        let k = caja_de_camara(&c, 800, 600);
        // 400 x 300 pixeles a cada lado, mas el margen de dos.
        assert_eq!(
            k,
            [100.0 - 201.0, 50.0 - 151.0, 100.0 + 201.0, 50.0 + 151.0]
        );
    }

    #[test]
    fn la_camara_va_y_vuelve() {
        let c = Camara {
            centro: [100.0, 50.0],
            px: 0.5,
        };
        let p = c.plano(10.0, 20.0, 800.0, 600.0);
        let (x, y) = c.pantalla(p, 800.0, 600.0);
        assert!((x - 10.0).abs() < 1e-4 && (y - 20.0).abs() < 1e-4);
        // Acercar deja quieto el punto del cursor.
        let z = c.zoom(0.5, 10.0, 20.0, 800.0, 600.0);
        let q = z.plano(10.0, 20.0, 800.0, 600.0);
        assert!((q[0] - p[0]).abs() < 1e-9 && (q[1] - p[1]).abs() < 1e-9);
    }

    #[test]
    fn encuadrar_en_una_zona_que_no_es_toda_la_ventana() {
        let caja = [0.0, 0.0, 1000.0, 500.0];
        let zona = [0.0, 100.0, 700.0, 400.0];
        let c = Camara::encuadrar(caja, zona, 1000.0, 600.0);
        let (x, y) = c.pantalla([500.0, 250.0], 1000.0, 600.0);
        assert!(
            (x - 350.0).abs() < 1e-3 && (y - 300.0).abs() < 1e-3,
            "{x} {y}"
        );
    }
}
