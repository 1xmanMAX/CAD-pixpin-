//! Pintar la tinta como Excalidraw: contorno cerrado, curvas cuadraticas
//! por puntos medios y relleno *winding*.
//!
//! `pasos_de_tinta` es `getSvgPathFromStroke` de Excalidraw
//! (`packages/element/src/shape.ts` @afa3a65, MIT) sin el recorte a dos
//! decimales, que solo existe para acortar el SVG. Es pura: se prueba sin
//! GPU contra el oraculo.
//!
//! Por que *winding* y no la regla por defecto de Direct2D (alternada): el
//! contorno de perfect-freehand se cruza consigo mismo en bucles, retrocesos
//! y en los arcos de las esquinas. Con la alternada cada cruce abre un
//! agujero; Canvas2D rellena con *nonzero*, que es *winding*. `Pintor::velo`
//! SI necesita la alternada para su hueco, por eso esto es una geometria
//! aparte y no un cambio en `Pintor::geometria`.

use std::collections::HashMap;
use std::time::Duration;

use windows::Win32::Graphics::Direct2D::ID2D1GeometryRealization;

/// Cuanto tiene que estar quieto el zoom para rehacer la tinta nitida (D122,
/// el `shouldCacheIgnoreZoom` de Excalidraw). Mientras tanto se estira lo
/// realizado: algo borroso y casi gratis.
pub fn retardo_nitido(nivel: pixpin_nivel::Nivel) -> Duration {
    match nivel {
        pixpin_nivel::Nivel::Ligero => Duration::from_millis(500),
        _ => Duration::from_millis(300),
    }
}

pub(crate) struct Realizada {
    pub(crate) version: u32,
    pub(crate) realizacion: ID2D1GeometryRealization,
    /// El fotograma en que se pinto por ultima vez: lo que decide a quien se
    /// echa cuando la cache se pasa de su presupuesto.
    pub(crate) usado: u64,
    /// Lo que pesa, en segmentos de la geometria de la que salio (ver
    /// `PESO_MAXIMO`).
    pub(crate) peso: u32,
    /// La forma de la geometria sin su sitio (`huella_de_forma`) y donde
    /// estaba su primer vertice al realizarla. Con eso, lo que solo se
    /// MOVIO se pinta con la misma realizacion corrida, sin crear otra
    /// (ver `reusar_trasladada`).
    pub(crate) forma: u64,
    pub(crate) ancla: (f32, f32),
    /// Cuanto hay que correrla para pintarla donde esta ahora.
    pub(crate) corrida: (f32, f32),
    /// La escala a la que se teselo. Distinta de la de la cache solo tras
    /// [`CacheTinta::pasar_a_escala`]: vale, algo facetada o de mas, hasta
    /// que haya tiempo de rehacerla.
    pub(crate) escala: f32,
}

/// Cuantas realizaciones se guardan a la vez, como mucho.
///
/// Generoso a proposito: una escena de 2.000 formas de rough.js con relleno
/// de sombreado puede pedir decenas de miles de ordenes, y echar lo que el
/// siguiente fotograma va a volver a pedir seria peor que no cachear. Lo
/// que de verdad acota la memoria es `PESO_MAXIMO`; esto es solo el techo
/// del numero de entradas del mapa.
const MAX_REALIZACIONES: usize = 60_000;

/// **Cuanto puede pesar todo lo realizado junto**, en segmentos de geometria.
///
/// Medido (`envejecer_el_editor_varios_minutos`, 2026-09-22): un trazo a
/// mano de 150 puntos -unos 240 segmentos de contorno- realizado ocupa
/// ~80 KB de memoria del proceso mas ~30 KB de memoria de video; de media,
/// entre medio kilo y un kilo por segmento. Y **no se soltaba nunca**: el
/// unico tope era el de arriba, sesenta mil entradas, que a ese peso son
/// varios gigas. Dibujando por un plano grande (cada minuto en otro sitio
/// del papel) la cache crecia unos 5.000 segmentos por minuto sin echar
/// nada de lo que ya no se veia. En la HD 4000 del equipo minimo la memoria
/// de video es la del sistema: cada trazo dibujado le quitaba memoria a
/// todo lo demas mientras el editor siguiera abierto.
///
/// Doscientos mil segmentos son unos 800 trazos como ese, del orden de 100 a
/// 200 MB entre proceso y video: mas de lo que cabe en una pantalla de
/// trabajo, asi que lo que se ve no se echa, y lo que no se ve deja de
/// crecer sin fin.
const PESO_MAXIMO: u64 = 200_000;

/// La geometria ya teselada por Direct2D, por elemento y orden.
///
/// Una realizacion es la geometria convertida en triangulos a una escala:
/// pintarla no recalcula nada en el procesador. Se rehace solo si cambia la
/// version del elemento o si se fija otra escala.
///
/// Empezo guardando solo la tinta (D121) y ahora guarda TODAS las ordenes
/// de geometria: los poligonos, los rellenos y las polilineas de rough.js
/// tambien. Sin eso, un fotograma de paneo creaba una `ID2D1PathGeometry`
/// por cada pasada de cada borde y por cada raya de cada sombreado —de 20 a
/// 60 por forma rellena— y las tiraba al acabar.
pub struct CacheTinta {
    pub(crate) mapa: HashMap<(u64, u32), Realizada>,
    pub(crate) escala: f32,
    /// La suma de `Realizada::peso` de todo el mapa.
    peso: u64,
    /// El fotograma en que se busco que echar y no habia nada que se
    /// pudiera: todo lo guardado se esta pintando. Mientras siga siendo ese
    /// fotograma no se vuelve a buscar, o cada realizacion nueva de un
    /// fotograma lleno recorreria el mapa entero.
    sin_hueco_en: Option<u64>,
    /// Cuantas realizaciones se han hecho desde que nacio. No baja nunca:
    /// quien mide resta dos lecturas y sabe cuanto se teselo entre medias
    /// (`[rendimiento] medir_fotogramas`), que es el coste que no se ve en
    /// el reloj de un fotograma sino en el siguiente que tira de lo mismo.
    realizadas: u64,
    /// Ver `reusar_trasladada`: no baja nunca, como `realizadas`.
    trasladadas: u64,
    /// **Cuanto tiempo de teselar hay en este fotograma** (ver
    /// [`CacheTinta::presupuesto`]): `None`, sin tope (lo de siempre).
    presupuesto: Option<std::time::Duration>,
    /// Lo gastado en teselar desde que se puso el presupuesto.
    gastado: std::time::Duration,
    /// Lo que se pinto con lo de antes por falta de tiempo desde que se
    /// puso el presupuesto: si no es cero, hace falta otro fotograma.
    pendientes: u32,
    /// **La tela con la que se tine la silueta**: las brochas de mosaico de
    /// `Pintor::grano`, una por material y color.
    ///
    /// Vive dentro y no al lado por una razon practica: va exactamente a los
    /// mismos sitios que la silueta —quien pinta un trazo pinta su grano— y
    /// las dos son recursos del MISMO dispositivo Direct2D, asi que se
    /// pierden y se vacian a la vez. Tenerlas separadas obligaba a llevar un
    /// segundo parametro por las cinco funciones de pintado de la ventana
    /// para no perderlo jamas de vista.
    pub grano: crate::grano::CacheGrano,
}

/// Que claves echar para volver a caber: las que no se pintaron ni en este
/// fotograma ni en el anterior, **de la mas vieja a la mas nueva**, hasta
/// bajar a tres cuartos del presupuesto (en peso y en numero).
///
/// Bajar a tres cuartos y no justo al tope es lo que hace que echar sea de
/// vez en cuando y no en cada realizacion nueva. Lo que se esta pintando no
/// se echa nunca: si todo lo guardado se ve, se queda aunque se pase, porque
/// echarlo seria volver a teselarlo en el fotograma siguiente -y en el
/// otro, y en el otro-.
///
/// Pura, sin GPU, para poder probar la politica sin una realizacion de
/// verdad: `entradas` son `(clave, usado, peso)`.
pub(crate) fn a_echar(
    entradas: impl Iterator<Item = ((u64, u32), u64, u32)>,
    fotograma: u64,
    peso_total: u64,
    cuantas: usize,
    peso_maximo: u64,
    max_entradas: usize,
) -> Vec<(u64, u32)> {
    let mut viejas: Vec<((u64, u32), u64, u32)> = entradas
        .filter(|(_, usado, _)| usado.saturating_add(1) < fotograma)
        .collect();
    viejas.sort_unstable_by_key(|(_, usado, _)| *usado);
    let (meta_peso, meta_cuantas) = (peso_maximo / 4 * 3, max_entradas / 4 * 3);
    let (mut peso, mut n) = (peso_total, cuantas);
    let mut fuera = Vec::new();
    for (clave, _, p) in viejas {
        if peso <= meta_peso && n <= meta_cuantas {
            break;
        }
        peso = peso.saturating_sub(p as u64);
        n -= 1;
        fuera.push(clave);
    }
    fuera
}

impl CacheTinta {
    pub fn nueva() -> Self {
        Self {
            mapa: HashMap::new(),
            escala: 1.0,
            peso: 0,
            sin_hueco_en: None,
            realizadas: 0,
            trasladadas: 0,
            presupuesto: None,
            gastado: std::time::Duration::ZERO,
            pendientes: 0,
            grano: crate::grano::CacheGrano::nueva(),
        }
    }

    /// Dispositivo perdido o documento nuevo: las realizaciones son del
    /// dispositivo viejo y no valen. **Las brochas del grano tampoco**: son
    /// bitmaps del mismo dispositivo, y sobrevivir a su muerte es justo el
    /// fallo que se ve como un lienzo en blanco.
    pub fn vaciar(&mut self) {
        self.mapa.clear();
        self.peso = 0;
        self.sin_hueco_en = None;
        self.grano.vaciar();
    }

    pub fn escala(&self) -> f32 {
        self.escala
    }

    /// Se llama cuando el zoom lleva `retardo_nitido` quieto.
    pub fn fijar_escala(&mut self, escala: f32) {
        if escala != self.escala {
            self.escala = escala;
            self.mapa.clear();
            self.peso = 0;
            self.sin_hueco_en = None;
        }
    }

    /// **Otra escala sin tiron** (13-oct-2026, «en los CAD con anotaciones el
    /// zoom va lagueado»): como [`CacheTinta::fijar_escala`], pero sin tirar
    /// lo realizado. Lo de la escala de antes se sigue pintando (los
    /// triangulos estan en coordenadas del documento: la transformada lo
    /// pone en su sitio y a su tamano, solo algo facetado acercando) y se
    /// rehace a la nueva conforme haya tiempo ([`CacheTinta::presupuesto`]).
    ///
    /// Medido en el visor de planos (`plano_cad::anotar::rendimiento`): con
    /// 378 trazos a la vista, tirarlo todo al pasar de vez y media de zoom
    /// era un fotograma de 140 a 260 ms (Direct2D tesela cada trazo de nuevo,
    /// ~0,4 ms cada uno), varias veces por cada vuelta de rueda.
    pub fn pasar_a_escala(&mut self, escala: f32) {
        self.escala = escala;
    }

    /// **El tiempo de teselar de este fotograma**: a partir de aqui, una vez
    /// gastado `tope` en teselar (lo apunta [`CacheTinta::gastar`]), lo que
    /// tiene realizacion de otra escala se pinta con ella en vez de
    /// rehacerla, y se cuenta como pendiente. Lo que no tiene ninguna se
    /// realiza igual: lo nuevo no puede faltar. `None` quita el tope.
    pub fn presupuesto(&mut self, tope: Option<std::time::Duration>) {
        self.presupuesto = tope;
        self.gastado = std::time::Duration::ZERO;
        self.pendientes = 0;
    }

    /// Apunta tiempo gastado en teselar (o en calcular lo que se tesela).
    pub fn gastar(&mut self, t: std::time::Duration) {
        self.gastado += t;
    }

    /// Si ya no queda tiempo de teselar en este fotograma.
    pub fn sin_tiempo(&self) -> bool {
        self.presupuesto.is_some_and(|p| self.gastado >= p)
    }

    /// Apunta algo pintado con lo de antes por falta de tiempo.
    pub fn apuntar_pendiente(&mut self) {
        self.pendientes += 1;
    }

    /// Lo pintado con lo de antes desde [`CacheTinta::presupuesto`]: si no es
    /// cero, quien pinta tiene que pedir otro fotograma para rehacerlo.
    pub fn pendientes(&self) -> u32 {
        self.pendientes
    }

    pub fn cuantas(&self) -> usize {
        self.mapa.len()
    }

    /// Lo que pesa todo lo realizado, en segmentos: para las mediciones y
    /// para la puerta que comprueba que no crece sin fin.
    pub fn peso(&self) -> u64 {
        self.peso
    }

    /// Si ya hay una realizacion valida para `clave` (mismo elemento, mismo
    /// indice de orden, misma version). Quien pinta la usa para decidir SI
    /// hace falta reconstruir el contorno en `Vec<(f32, f32)>` -en un
    /// acierto no hace falta, y esa reconstruccion es la que se paga cada
    /// fotograma para nada si no se consulta antes-.
    pub fn vale(&self, clave: (u64, u32, u32)) -> bool {
        let (id, version, indice) = clave;
        self.mapa
            .get(&(id, indice))
            .is_some_and(|r| r.version == version)
    }

    /// Guarda una realizacion recien hecha, echando lo viejo si hace falta.
    /// `fotograma` es el de `MotorRender`, que ya cuenta uno por `dibujar`;
    /// `peso`, los segmentos de la geometria de la que sale.
    pub(crate) fn guardar(
        &mut self,
        clave: (u64, u32, u32),
        fotograma: u64,
        realizacion: ID2D1GeometryRealization,
        peso: u32,
        forma: u64,
        ancla: (f32, f32),
    ) {
        let (id, version, indice) = clave;
        if let Some(vieja) = self.mapa.insert(
            (id, indice),
            Realizada {
                version,
                realizacion,
                usado: fotograma,
                peso,
                forma,
                ancla,
                corrida: (0.0, 0.0),
                escala: self.escala,
            },
        ) {
            self.peso = self.peso.saturating_sub(vieja.peso as u64);
        }
        self.peso += peso as u64;
        self.realizadas += 1;
        self.desalojar(fotograma);
    }

    /// Cuantas realizaciones se han hecho desde que nacio (ver el campo).
    pub fn realizadas(&self) -> u64 {
        self.realizadas
    }

    /// Echa lo que no se ve si la cache se pasa de su presupuesto.
    ///
    /// Antes, al llegar a `MAX_REALIZACIONES` se tiraba de golpe todo lo que
    /// no fuera de los dos ultimos fotogramas, y si no bastaba, el mapa
    /// entero: un tiron de volver a teselar la pantalla completa. Ahora se
    /// echa lo mas viejo primero y solo lo justo.
    fn desalojar(&mut self, fotograma: u64) {
        if self.peso <= PESO_MAXIMO && self.mapa.len() <= MAX_REALIZACIONES {
            return;
        }
        if self.sin_hueco_en == Some(fotograma) {
            return;
        }
        let fuera = a_echar(
            self.mapa.iter().map(|(k, r)| (*k, r.usado, r.peso)),
            fotograma,
            self.peso,
            self.mapa.len(),
            PESO_MAXIMO,
            MAX_REALIZACIONES,
        );
        if fuera.is_empty() {
            self.sin_hueco_en = Some(fotograma);
            return;
        }
        for k in fuera {
            if let Some(r) = self.mapa.remove(&k) {
                self.peso = self.peso.saturating_sub(r.peso as u64);
            }
        }
    }

    /// Apunta que `clave` se pinto en `fotograma`, para que la limpieza no
    /// la eche. Devuelve la realizacion si la hay y es de esa version.
    pub(crate) fn tomar(
        &mut self,
        clave: (u64, u32, u32),
        fotograma: u64,
    ) -> Option<(ID2D1GeometryRealization, (f32, f32))> {
        let (id, version, indice) = clave;
        let sin_tiempo = self.sin_tiempo();
        let escala = self.escala;
        let r = self.mapa.get_mut(&(id, indice))?;
        if r.version != version {
            return None;
        }
        // De otra escala: se rehace si hay tiempo; si no, vale la de antes.
        if r.escala != escala {
            if !sin_tiempo {
                return None;
            }
            self.pendientes += 1;
        }
        r.usado = fotograma;
        Some((r.realizacion.clone(), r.corrida))
    }

    /// **Lo que solo se movio no se vuelve a realizar.** Si `clave` tiene ya
    /// una realizacion de la MISMA forma (`huella_de_forma`) en otro sitio,
    /// se le pone la version nueva y lo que hay que correrla, y se devuelve
    /// `true`: quien pinta la usa tal cual, trasladada.
    ///
    /// Es por la memoria, no por el teselado. Medido
    /// (`tests/memoria_realizaciones.rs`, 2026-09-24): cada fotograma que
    /// pinta cientos de realizaciones DESPUES de crear una nueva hace crecer
    /// el proceso y la memoria de video unos megas -proporcional a cuantas
    /// pinta- y ni vaciar la cache ni `Trim` lo devuelven. Arrastrar una
    /// figura y soltarla rehacia su realizacion (version nueva), y cada
    /// arrastre sumaba asi varios megas en la HD 4000, donde la memoria de
    /// video es la del sistema: al cabo de un rato, el resto del equipo iba
    /// sin memoria. Moviendo con la misma realizacion no se crea ninguna.
    pub(crate) fn reusar_trasladada(
        &mut self,
        clave: (u64, u32, u32),
        forma: u64,
        ancla: (f32, f32),
        fotograma: u64,
    ) -> bool {
        let (id, version, indice) = clave;
        let escala = self.escala;
        let Some(r) = self.mapa.get_mut(&(id, indice)) else {
            return false;
        };
        // La de otra escala no se reusa: si se llego aqui es para rehacerla.
        if r.forma != forma || r.escala != escala {
            return false;
        }
        r.version = version;
        r.usado = fotograma;
        r.corrida = (ancla.0 - r.ancla.0, ancla.1 - r.ancla.1);
        self.trasladadas += 1;
        true
    }

    /// Cuantas veces se ha pintado algo movido con su realizacion de antes
    /// en vez de crear otra (ver `reusar_trasladada`). Para las mediciones.
    pub fn trasladadas(&self) -> u64 {
        self.trasladadas
    }
}

/// **La forma de unos vertices sin su sitio**: un resumen de sus posiciones
/// RELATIVAS al primero, a 1/64 de unidad, mas `extra` (lo que ademas de los
/// puntos cambia la realizacion: si es relleno o trazo, su grosor, si va a
/// rayas). Devuelve el resumen y el primer vertice, que es el ancla.
///
/// Dos geometrias con el mismo resumen son la misma trasladada: la
/// realizacion de una sirve para la otra corriendola. Si el redondeo de una
/// traslacion hace que un vertice caiga al otro lado de un 1/64 el resumen
/// sale distinto y se realiza otra vez, que es lo de siempre: fallar asi
/// solo cuesta lo de antes.
pub fn huella_de_forma(vertices: &[(f32, f32)], extra: u64) -> (u64, (f32, f32)) {
    let Some(&ancla) = vertices.first() else {
        return (extra, (0.0, 0.0));
    };
    // FNV-1a sobre enteros: sin reservar y estable entre ejecuciones.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ extra;
    let mut mezclar = |v: i32| {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    mezclar(vertices.len() as i32);
    for &(x, y) in vertices {
        mezclar(((x - ancla.0) * 64.0).round() as i32);
        mezclar(((y - ancla.1) * 64.0).round() as i32);
    }
    (h, ancla)
}

impl Default for CacheTinta {
    fn default() -> Self {
        Self::nueva()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasoTrayecto {
    Mover((f32, f32)),
    Cuadratica {
        control: (f32, f32),
        fin: (f32, f32),
    },
    Linea((f32, f32)),
    Cerrar,
}

/// `M p0 Q p0 m01 p1 m12 ... pN mN0 L p0 Z`.
pub fn pasos_de_tinta(contorno: &[(f32, f32)]) -> Vec<PasoTrayecto> {
    let Some(&primero) = contorno.first() else {
        return Vec::new();
    };
    let medio = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let max = contorno.len() - 1;
    let mut pasos = Vec::with_capacity(contorno.len() + 3);
    pasos.push(PasoTrayecto::Mover(primero));
    for (i, &p) in contorno.iter().enumerate() {
        let siguiente = if i == max { primero } else { contorno[i + 1] };
        pasos.push(PasoTrayecto::Cuadratica {
            control: p,
            fin: medio(p, siguiente),
        });
    }
    pasos.push(PasoTrayecto::Linea(primero));
    pasos.push(PasoTrayecto::Cerrar);
    pasos
}

#[cfg(test)]
pub(crate) mod pruebas {
    use super::*;
    use crate::motor::Color;

    #[test]
    fn el_retardo_nitido_es_mas_largo_en_ligero() {
        use pixpin_nivel::Nivel;
        assert_eq!(retardo_nitido(Nivel::Completo).as_millis(), 300);
        assert_eq!(retardo_nitido(Nivel::Ligero).as_millis(), 500);
    }

    #[test]
    fn fijar_otra_escala_tira_lo_realizado_y_la_misma_no() {
        let mut c = CacheTinta::nueva();
        assert_eq!(c.escala(), 1.0);
        c.fijar_escala(1.0);
        assert_eq!(c.escala(), 1.0);
        c.fijar_escala(2.0);
        assert_eq!(c.escala(), 2.0);
        assert_eq!(c.cuantas(), 0);
    }

    /// Lo que hace `CacheTinta::guardar` con su mapa, sin GPU: meter y, si
    /// se pasa, echar lo que diga `a_echar`.
    struct Simulada {
        mapa: HashMap<(u64, u32), (u64, u32)>,
        peso: u64,
    }

    impl Simulada {
        fn guardar(&mut self, clave: (u64, u32), fotograma: u64, peso: u32, maximo: u64) {
            if let Some((_, viejo)) = self.mapa.insert(clave, (fotograma, peso)) {
                self.peso -= viejo as u64;
            }
            self.peso += peso as u64;
            if self.peso > maximo {
                for k in a_echar(
                    self.mapa.iter().map(|(k, (u, p))| (*k, *u, *p)),
                    fotograma,
                    self.peso,
                    self.mapa.len(),
                    maximo,
                    usize::MAX,
                ) {
                    let (_, p) = self.mapa.remove(&k).expect("estaba");
                    self.peso -= p as u64;
                }
            }
        }
        fn tomar(&mut self, clave: (u64, u32), fotograma: u64) {
            if let Some(e) = self.mapa.get_mut(&clave) {
                e.0 = fotograma;
            }
        }
    }

    #[test]
    fn dibujar_sin_parar_no_hace_crecer_lo_realizado_sin_fin() {
        // La puerta del «al rato va lento»: una sesion larga dibujando un
        // trazo nuevo por fotograma y pintando solo los ultimos treinta
        // (lo que se ve). Antes el peso crecia con cada trazo hasta sesenta
        // mil entradas; ahora se queda en el presupuesto.
        let maximo = 10_000;
        let mut c = Simulada {
            mapa: HashMap::new(),
            peso: 0,
        };
        for fotograma in 1..=5_000u64 {
            let desde = fotograma.saturating_sub(30);
            for id in desde..fotograma {
                c.tomar((id, 0), fotograma);
            }
            c.guardar((fotograma, 0), fotograma, 250, maximo);
            assert!(c.peso <= maximo, "fotograma {fotograma}: pesa {}", c.peso);
        }
        // Y lo que se ve sigue ahi: echar lo visible seria volver a teselar
        // la pantalla en cada fotograma.
        for id in 4_971..=5_000u64 {
            assert!(c.mapa.contains_key(&(id, 0)), "se echo {id}, que se ve");
        }
    }

    #[test]
    fn lo_que_se_pinta_en_este_fotograma_no_se_echa_aunque_se_pase() {
        let entradas = (0..10u64).map(|id| ((id, 0), 7, 100));
        let fuera = a_echar(entradas, 7, 1_000, 10, 500, usize::MAX);
        assert!(fuera.is_empty(), "todo es de este fotograma: {fuera:?}");
    }

    #[test]
    fn la_huella_de_una_forma_corrida_es_la_misma_y_la_de_otra_forma_no() {
        let forma: Vec<(f32, f32)> = (0..50)
            .map(|i| (i as f32 * 3.1, (i as f32 * 0.7).sin() * 20.0))
            .collect();
        let corrida: Vec<(f32, f32)> = forma.iter().map(|(x, y)| (x + 80.0, y - 12.5)).collect();
        let (a, ancla_a) = super::huella_de_forma(&forma, 1);
        let (b, ancla_b) = super::huella_de_forma(&corrida, 1);
        assert_eq!(a, b, "la misma forma en otro sitio");
        assert_eq!(
            (ancla_b.0 - ancla_a.0, ancla_b.1 - ancla_a.1),
            (80.0, -12.5)
        );
        // Casos negativos: un vertice movido medio pixel, otra clase de
        // orden (trazo en vez de relleno) y un punto de menos.
        let mut tocada = forma.clone();
        tocada[20].1 += 0.5;
        assert_ne!(super::huella_de_forma(&tocada, 1).0, a);
        assert_ne!(super::huella_de_forma(&forma, 2).0, a);
        assert_ne!(super::huella_de_forma(&forma[..49], 1).0, a);
    }

    #[test]
    fn se_echa_primero_lo_mas_viejo_y_solo_hasta_tres_cuartos() {
        // Diez de peso 100 (1.000) con tope 800: hay que bajar a 600, o sea
        // echar cuatro, y los cuatro mas viejos.
        let entradas = (0..10u64).map(|id| ((id, 0), id, 100));
        let mut fuera = a_echar(entradas, 20, 1_000, 10, 800, usize::MAX);
        fuera.sort();
        assert_eq!(fuera, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    }

    #[test]
    fn una_cache_vacia_no_vale_para_ninguna_clave() {
        let c = CacheTinta::nueva();
        assert!(!c.vale((1, 1, 0)));
    }

    /// Motor y un destino de `ancho x alto` sobre un D3D11 hardware propio.
    pub(crate) fn motor_y_destino_de_prueba(
        ancho: u32,
        alto: u32,
    ) -> (
        crate::MotorRender,
        windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    ) {
        use windows::Win32::Foundation::HMODULE;
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice,
        };
        use windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
        };
        let mut d3d = None;
        // SAFETY: salidas locales; sin adaptador concreto ni capas de depuracion.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )
            .expect("sin D3D11 hardware");
        }
        let d3d = d3d.expect("dispositivo");
        let motor = crate::MotorRender::nuevo(&d3d).expect("motor");
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            ..Default::default()
        };
        let mut textura = None;
        // SAFETY: descripcion local valida; salida local.
        unsafe {
            d3d.CreateTexture2D(&desc, None, Some(&mut textura))
                .expect("textura")
        };
        let destino = motor
            .destino_desde_textura(&textura.expect("textura"))
            .expect("destino");
        (motor, destino)
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio"]
    fn pintar_dos_veces_la_misma_clave_realiza_una_sola_vez() {
        let (motor, destino) = motor_y_destino_de_prueba(64, 64);
        let mut cache = CacheTinta::nueva();
        let contorno = [(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)];
        assert!(!cache.vale((7, 1, 0)), "todavia no se ha pintado nada");
        for _ in 0..2 {
            motor
                .dibujar(&destino, |p| {
                    p.tinta_cacheada(&mut cache, (7, 1, 0), &contorno, Color::NEGRO)
                })
                .unwrap();
        }
        assert_eq!(cache.cuantas(), 1);
        assert!(cache.vale((7, 1, 0)), "la version 1 quedo realizada");
        assert!(!cache.vale((7, 2, 0)), "la version 2 no se ha pintado aun");
        motor
            .dibujar(&destino, |p| {
                p.tinta_cacheada(&mut cache, (7, 2, 0), &contorno, Color::NEGRO)
            })
            .unwrap();
        assert_eq!(
            cache.cuantas(),
            1,
            "una version nueva sustituye, no acumula"
        );
    }

    #[test]
    fn otra_escala_sin_tiempo_pinta_lo_de_antes_y_con_tiempo_lo_rehace() {
        let (motor, destino) = motor_y_destino_de_prueba(64, 64);
        let mut cache = CacheTinta::nueva();
        let contorno = [(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)];
        let pintar = |cache: &mut CacheTinta, clave| {
            motor
                .dibujar(&destino, |p| {
                    if !p.pintar_realizada(cache, clave, Color::NEGRO) {
                        p.tinta_cacheada(cache, clave, &contorno, Color::NEGRO)
                    }
                })
                .unwrap();
        };
        pintar(&mut cache, (7, 1, 0));
        assert_eq!(cache.realizadas(), 1);
        // Otra escala, sin tiempo: vale la de antes y queda pendiente.
        cache.pasar_a_escala(3.0);
        assert_eq!(cache.cuantas(), 1, "pasar de escala no tira nada");
        cache.presupuesto(Some(std::time::Duration::ZERO));
        pintar(&mut cache, (7, 1, 0));
        assert_eq!(cache.realizadas(), 1, "sin tiempo no se tesela");
        assert_eq!(cache.pendientes(), 1);
        // Caso negativo: algo que CAMBIO se rehace aunque no haya tiempo
        // (si no, se veria lo de antes).
        pintar(&mut cache, (7, 2, 0));
        assert_eq!(cache.realizadas(), 2, "lo cambiado no espera");
        // Con tiempo, lo de otra escala se rehace y deja de estar pendiente.
        pintar(&mut cache, (8, 1, 0));
        cache.pasar_a_escala(9.0);
        cache.presupuesto(Some(std::time::Duration::from_secs(1)));
        pintar(&mut cache, (8, 1, 0));
        assert_eq!(cache.realizadas(), 4);
        assert_eq!(cache.pendientes(), 0);
        // Y sin presupuesto (lo de siempre) tambien se rehace.
        cache.pasar_a_escala(1.0);
        cache.presupuesto(None);
        pintar(&mut cache, (8, 1, 0));
        assert_eq!(cache.realizadas(), 5);
    }

    #[test]
    fn el_presupuesto_cuenta_lo_gastado() {
        let mut c = CacheTinta::nueva();
        assert!(!c.sin_tiempo(), "sin presupuesto nunca falta tiempo");
        c.gastar(std::time::Duration::from_secs(9));
        assert!(!c.sin_tiempo());
        c.presupuesto(Some(std::time::Duration::from_millis(4)));
        assert!(!c.sin_tiempo(), "poner presupuesto empieza de cero");
        c.gastar(std::time::Duration::from_millis(3));
        assert!(!c.sin_tiempo());
        c.gastar(std::time::Duration::from_millis(1));
        assert!(c.sin_tiempo());
        c.apuntar_pendiente();
        assert_eq!(c.pendientes(), 1);
        c.presupuesto(Some(std::time::Duration::from_millis(4)));
        assert_eq!(c.pendientes(), 0, "cada fotograma cuenta los suyos");
    }

    #[test]
    fn un_contorno_vacio_no_da_ningun_paso() {
        assert!(pasos_de_tinta(&[]).is_empty());
    }

    #[test]
    fn cada_vertice_es_el_control_de_una_cuadratica_que_acaba_en_el_punto_medio() {
        let c = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        assert_eq!(
            pasos_de_tinta(&c),
            vec![
                PasoTrayecto::Mover((0.0, 0.0)),
                PasoTrayecto::Cuadratica {
                    control: (0.0, 0.0),
                    fin: (5.0, 0.0)
                },
                PasoTrayecto::Cuadratica {
                    control: (10.0, 0.0),
                    fin: (10.0, 5.0)
                },
                PasoTrayecto::Cuadratica {
                    control: (10.0, 10.0),
                    fin: (5.0, 5.0)
                },
                PasoTrayecto::Linea((0.0, 0.0)),
                PasoTrayecto::Cerrar,
            ]
        );
    }

    /// Los numeros del SVG de Excalidraw, en orden, sin las letras.
    fn numeros_svg(svg: &str) -> Vec<f32> {
        svg.split_whitespace()
            .filter(|t| !t.chars().all(|c| c.is_ascii_alphabetic()))
            .flat_map(|t| {
                t.split(',')
                    .map(|n| n.parse::<f32>().expect("numero"))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn numeros_pasos(p: &[PasoTrayecto]) -> Vec<f32> {
        p.iter()
            .flat_map(|paso| match *paso {
                PasoTrayecto::Mover(a) | PasoTrayecto::Linea(a) => vec![a.0, a.1],
                PasoTrayecto::Cuadratica { control, fin } => {
                    vec![control.0, control.1, fin.0, fin.1]
                }
                PasoTrayecto::Cerrar => vec![],
            })
            .collect()
    }

    #[test]
    fn el_trayecto_coincide_con_get_svg_path_from_stroke_de_excalidraw() {
        // NOTA (ruling del controlador): se compara contra `svg_crudo`, no
        // `svg`. El regex de truncado de Excalidraw a dos decimales corrompe
        // numeros en notacion exponencial (1.2e-16 pasa a "1.22"), lo que
        // daria un rojo falso. `svg_crudo` puede traer esa notacion
        // ("1.2246467991473532e-16"); f32::parse la acepta sin problema.
        let ruta = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../pixpin-motor2d/tests/oraculo/tinta.json"
        );
        let texto = std::fs::read_to_string(ruta).expect("falta el oraculo (Tarea 1)");
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        for caso in v["casos"].as_array().unwrap() {
            let contorno: Vec<(f32, f32)> = caso["contorno"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32))
                .collect();
            let esperado = numeros_svg(caso["svg_crudo"].as_str().unwrap());
            let obtenido = numeros_pasos(&pasos_de_tinta(&contorno));
            assert_eq!(esperado.len(), obtenido.len(), "{}", caso["nombre"]);
            for (e, o) in esperado.iter().zip(&obtenido) {
                // Coordenadas de pixel: la conversion f64->f32 se queda muy
                // por debajo de esta tolerancia.
                assert!((e - o).abs() <= 0.001, "{}: {e} contra {o}", caso["nombre"]);
            }
        }
    }
}
