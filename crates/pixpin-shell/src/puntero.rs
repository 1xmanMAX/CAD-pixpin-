//! Entrada fina para dibujar: todos los puntos del raton y del lapiz (E1).
//!
//! Windows fusiona los `WM_MOUSEMOVE` pendientes en uno: con un raton de
//! 1000 Hz y un fotograma ocupado, la aplicacion ve 40-60 puntos por
//! segundo y un trazo rapido sale como un poligono de pocos lados. Los
//! puntos no se pierden: `GetMouseMovePointsEx` guarda los 64 ultimos. Y el
//! lapiz trae los suyos, con presion y subpixel, en su historial de puntero.
//!
//! Excalidraw no hace nada de esto (se queda con un punto por fotograma);
//! aqui se puede hacer mejor que el original.
//!
//! Por que no se llama a `EnableMouseInPointer` (D106): convertiria el raton
//! en punteros para todo el proceso, y el overlay, los pines y los gestos
//! se reescribirian sin pruebas.

use std::cell::Cell;

use windows::Win32::Foundation::{RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GMMP_USE_DISPLAY_POINTS, GetMouseMovePointsEx, MOUSEMOVEPOINT,
};
use windows::Win32::UI::Input::Pointer::{
    GetPointerDeviceRects, GetPointerPenInfo, GetPointerPenInfoHistory, GetPointerType,
    POINTER_FLAG_INCONTACT, POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageExtraInfo, PEN_FLAG_ERASER, PEN_FLAG_INVERTED, PEN_MASK_PRESSURE, POINTER_INPUT_TYPE,
    PT_PEN,
};

/// Una muestra del puntero en el escritorio virtual.
///
/// En punto fijo (1/16 px y presion 0-1024) y no en `f32` porque viaja
/// dentro de `EventoOverlay`, que es `Copy + Eq`: un `f32` no es `Eq`.
/// 1/16 de pixel es mas fino que cualquier digitalizador de consumo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Muestra {
    x16: i32,
    y16: i32,
    presion: Option<u16>,
}

impl Muestra {
    pub fn nueva(x: f32, y: f32, presion: Option<f32>) -> Muestra {
        Muestra {
            x16: (x * 16.0).round() as i32,
            y16: (y * 16.0).round() as i32,
            presion: presion.map(|p| (p.clamp(0.0, 1.0) * 1024.0).round() as u16),
        }
    }
    pub fn x(&self) -> f32 {
        self.x16 as f32 / 16.0
    }
    pub fn y(&self) -> f32 {
        self.y16 as f32 / 16.0
    }
    pub fn presion(&self) -> Option<f32> {
        self.presion.map(|p| p as f32 / 1024.0)
    }
}

/// Los puntos que Windows fusiono entre el ultimo entregado y el actual.
///
/// `recientes` viene como lo devuelve `GetMouseMovePointsEx`: el mas nuevo
/// primero, y en `[0]` el punto del propio mensaje, que ya llega por su
/// cuenta y no se repite. Se para al llegar al anterior o a algo mas viejo
/// que el: si el anterior ya salio del historial, no se devuelve historia
/// de antes de pulsar. (El contador de tiempo da la vuelta cada 49 dias:
/// ese mensaje se queda sin recuperar, nada mas.)
pub fn perdidos_entre(recientes: &[(i32, i32, u32)], anterior: (i32, i32, u32)) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for &(x, y, t) in recientes.iter().skip(1) {
        if (x, y, t) == anterior || t < anterior.2 {
            break;
        }
        v.push((x, y));
    }
    v.reverse();
    v
}

/// HIMETRIC del digitalizador a pixeles del escritorio, por proporcion
/// entre los dos rectangulos que da `GetPointerDeviceRects`.
pub fn himetric_a_pixel(
    him: (i32, i32),
    dispositivo: (i32, i32, i32, i32),
    pantalla: (i32, i32, i32, i32),
) -> (f32, f32) {
    let (dl, dt, dr, db) = dispositivo;
    let (pl, pt, pr, pb) = pantalla;
    let ancho_d = ((dr - dl) as f32).max(1.0);
    let alto_d = ((db - dt) as f32).max(1.0);
    (
        pl as f32 + (him.0 - dl) as f32 * (pr - pl) as f32 / ancho_d,
        pt as f32 + (him.1 - dt) as f32 * (pb - pt) as f32 / alto_d,
    )
}

/// Con varios monitores, `GMMP_USE_DISPLAY_POINTS` devuelve las
/// coordenadas negativas como valores de 16 bits sin signo.
fn con_signo(c: i32) -> i32 {
    if c > 32767 { c - 65536 } else { c }
}

thread_local! {
    static AVISADO: Cell<bool> = const { Cell::new(false) };
}

fn avisar_una_vez(que: &str) {
    if !AVISADO.with(|a| a.replace(true)) {
        tracing::warn!(
            que,
            "sin historial de puntero: se usa solo el punto del mensaje"
        );
    }
}

/// Lo que hace falta recordar entre dos `WM_MOUSEMOVE` de una ventana.
#[derive(Debug, Default)]
pub struct HistorialRaton {
    anterior: Option<(i32, i32, u32)>,
}

impl HistorialRaton {
    pub const fn nuevo() -> Self {
        Self { anterior: None }
    }

    /// Al soltar el boton: el siguiente trazo no recupera puntos de este.
    pub fn olvidar(&mut self) {
        self.anterior = None;
    }

    /// Al pulsar el boton: el punto y la hora del pulsado son el "anterior"
    /// del primer movimiento. Sin esto, el primer `WM_MOUSEMOVE` no recupera
    /// nada, y como el editor gasta un fotograma entero en el pulsado, una
    /// letra rapida empezaba cada trazo con una cuerda recta.
    ///
    /// El punto del pulsado no tiene por que ser una entrada exacta del
    /// historial: `perdidos_entre` para tambien por tiempo, asi que lo peor
    /// es parar en el primer punto mas viejo que el pulsado.
    pub fn sembrar(&mut self, x: i32, y: i32, tiempo: u32) {
        self.anterior = Some((x, y, tiempo));
    }

    /// Los puntos fusionados antes de `(x, y)` (escritorio virtual) y
    /// `tiempo` (`GetMessageTime`). Sin `sembrar` ni movimiento previo no
    /// recupera nada: lo anterior seria de antes de pulsar.
    pub fn recuperar(&mut self, x: i32, y: i32, tiempo: u32) -> Vec<Muestra> {
        let Some(anterior) = self.anterior.replace((x, y, tiempo)) else {
            return Vec::new();
        };
        let entrada = MOUSEMOVEPOINT {
            x: x & 0xFFFF,
            y: y & 0xFFFF,
            time: tiempo,
            dwExtraInfo: 0,
        };
        let mut buf = [MOUSEMOVEPOINT::default(); 64];
        // SAFETY: estructura de entrada local y bufer local de 64, el maximo
        // que documenta la API; el tamano pasado es el de la estructura.
        let n = unsafe {
            GetMouseMovePointsEx(
                std::mem::size_of::<MOUSEMOVEPOINT>() as u32,
                &entrada,
                &mut buf,
                GMMP_USE_DISPLAY_POINTS,
            )
        };
        if n <= 0 {
            avisar_una_vez("GetMouseMovePointsEx");
            return Vec::new();
        }
        let recientes: Vec<(i32, i32, u32)> = buf[..n as usize]
            .iter()
            .map(|m| (con_signo(m.x), con_signo(m.y), m.time))
            .collect();
        perdidos_entre(&recientes, anterior)
            .into_iter()
            .map(|(x, y)| Muestra::nueva(x as f32, y as f32, None))
            .collect()
    }
}

/// De donde vino un `WM_MOUSEMOVE`, segun la firma que Windows deja en
/// `GetMessageExtraInfo` cuando lo sintetiza a partir de otro dispositivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigenRaton {
    /// Un raton de verdad (o cualquier firma que no reconocemos).
    Raton,
    /// Sintetizado a partir de un lapiz. Sus muestras buenas llegan por
    /// `WM_POINTERUPDATE`.
    Lapiz,
    /// Sintetizado a partir del tacto. El tacto no trae historial de
    /// puntero propio en esta entrega: este movimiento es lo unico que hay,
    /// y no debe tirarse.
    Tacto,
}

/// Clasifica la firma de `GetMessageExtraInfo` de un `WM_MOUSEMOVE`.
///
/// La firma documentada de lapiz y tacto comparten los tres bytes altos
/// (`0xFF5157xx`); el bit `0x80` del byte bajo es el que distingue tacto
/// (puesto) de lapiz (a cero). Antes de esta funcion, `es_raton_de_lapiz`
/// trataba ambos como lapiz y tiraba el movimiento sintetizado del tacto:
/// como `muestras_de_lapiz` devuelve `None` para `PT_TOUCH`, un arrastre con
/// el dedo se quedaba sin `RatonMovido` y sin `Muestra`, es decir, sin
/// dibujar nada.
pub fn origen_del_raton(extra: usize) -> OrigenRaton {
    if extra & 0xFFFF_FF00 != 0xFF51_5700 {
        OrigenRaton::Raton
    } else if extra & 0x80 != 0 {
        OrigenRaton::Tacto
    } else {
        OrigenRaton::Lapiz
    }
}

/// El origen del `WM_MOUSEMOVE` que se esta atendiendo en este momento.
pub fn origen_actual() -> OrigenRaton {
    // SAFETY: lee un valor del mensaje actual del hilo; sin precondiciones.
    let extra = unsafe { GetMessageExtraInfo() }.0 as usize;
    origen_del_raton(extra)
}

/// Si el `WM_MOUSEMOVE` que se esta atendiendo lo sintetizo Windows a partir
/// de un lapiz (no del tacto: ver `origen_del_raton`).
///
/// Esto por si solo NO basta para tirar el movimiento: hace falta ademas el
/// estado del ultimo `WM_POINTERUPDATE` (`EstadoLapiz`) y `descartar_movimiento`.
pub fn es_raton_de_lapiz() -> bool {
    origen_actual() == OrigenRaton::Lapiz
}

/// Lo que el ultimo `WM_POINTERUPDATE` de una ventana dejo saber sobre el
/// lapiz, para decidir si el `WM_MOUSEMOVE` que Windows sintetiza a partir
/// de el se puede tirar sin perder nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstadoLapiz {
    /// Aun no llego ningun `WM_POINTERUPDATE` de lapiz para este trazo, o el
    /// ultimo no trajo nada fiable (no es un lapiz, o fallo la API). El
    /// `WM_MOUSEMOVE` sintetizado es lo UNICO que hay: no se tira.
    #[default]
    SinDatos,
    /// El ultimo `WM_POINTERUPDATE` trajo tinta de verdad.
    ConMuestras,
    /// El ultimo `WM_POINTERUPDATE` fue el borrador o el lapiz invertido
    /// (D107): no hay tinta que dibujar, pero el lapiz SI respondio. El
    /// `WM_MOUSEMOVE` sintetizado se tira igual: si no, sus puntos
    /// fusionados (via `recuperar`) reintroducirian el trazo como tinta,
    /// justo lo que D107 pide evitar.
    Goma,
    /// El contacto EMPEZO con el borrador (`WM_POINTERDOWN` de goma): sus
    /// mensajes de puntero se tragan hasta el `WM_POINTERUP`, para que
    /// Windows no sintetice ni pulsado, ni movimiento, ni soltado. Distinto
    /// de `Goma`, que puede aparecer a mitad de un trazo de tinta cuyo
    /// soltado SI tiene que llegar (ver `decidir_goma`).
    ContactoDeGoma,
}

/// Si el `WM_MOUSEMOVE` sintetizado por Windows debe tirarse porque el
/// camino del lapiz ya lo cubre (o, en el caso del borrador, porque no debe
/// dibujar nada).
///
/// Pura: nada de esto toca Win32, para poder probarla con una tabla.
/// Solo el lapiz sintetiza un movimiento que otro camino sustituye mejor: el
/// raton no tiene sustituto, y el tacto (misma firma que el lapiz salvo un
/// bit, ver `origen_del_raton`) tampoco trae historial propio en esta
/// entrega. Y dentro del lapiz, `SinDatos` dice que el camino bueno no dio
/// nada esta vez (API caida, o el primer movimiento de un trazo antes de
/// que llegue su primer `WM_POINTERUPDATE`): ahi el punto del mensaje es lo
/// unico que hay, y tirarlo lo perderia sin que nadie lo repusiera.
pub fn descartar_movimiento(boton: bool, origen: OrigenRaton, estado: EstadoLapiz) -> bool {
    boton && origen == OrigenRaton::Lapiz && estado != EstadoLapiz::SinDatos
}

/// Si un `WM_MOUSEMOVE` (o su pulsado) puede usar el historial del raton.
///
/// Solo el raton de verdad y el tacto: un movimiento sintetizado por el
/// lapiz no es un punto de `GetMouseMovePointsEx` fiable, y sus muestras
/// buenas llegan por `WM_POINTERUPDATE`. Recuperar sobre el daba puntos ya
/// entregados por el camino del lapiz (un salto hacia atras en el trazo).
pub fn usa_historial_raton(origen: OrigenRaton) -> bool {
    origen != OrigenRaton::Lapiz
}

/// Si el estado del lapiz obliga a olvidar el historial del raton: en
/// cuanto el lapiz responde (tinta o borrador), el ultimo punto del raton
/// recordado ya no es el ultimo entregado, y recuperar desde el colaria
/// puntos viejos en el trazo.
pub fn olvida_historial_raton(estado: EstadoLapiz) -> bool {
    estado != EstadoLapiz::SinDatos
}

/// Si los indicadores `penFlags` de un lapiz dicen que lo que toca es el
/// borrador o la punta al reves (D107).
pub fn es_goma(pen_flags: u32) -> bool {
    pen_flags & (PEN_FLAG_ERASER | PEN_FLAG_INVERTED) != 0
}

/// En que momento del contacto llega un mensaje de puntero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaseContacto {
    /// `WM_POINTERDOWN`.
    Baja,
    /// `WM_POINTERUPDATE`.
    Actualiza,
    /// `WM_POINTERUP`.
    Sube,
}

/// Si un mensaje de puntero se traga (sin `DefWindowProc`) porque es de un
/// contacto de borrador, y con que estado queda la ventana. `None` = seguir
/// como siempre.
///
/// Por que tragarlo: `DefWindowProc` sintetiza `WM_LBUTTONDOWN` a partir de
/// `WM_POINTERDOWN`, y ese pulsado creaba un lapiz en el editor y un punto en
/// la capa y el pin (D107 pide ignorar el borrador). Se decide al bajar y se
/// mantiene hasta subir: si el lapiz cambiara a borrador A MITAD de un trazo
/// de tinta, tragarse su `WM_POINTERUP` dejaria el boton sin soltar y la
/// captura tomada para siempre.
pub fn decidir_goma(fase: FaseContacto, goma: bool, estado: EstadoLapiz) -> Option<EstadoLapiz> {
    match fase {
        FaseContacto::Baja if goma => Some(EstadoLapiz::ContactoDeGoma),
        FaseContacto::Actualiza if estado == EstadoLapiz::ContactoDeGoma => {
            Some(EstadoLapiz::ContactoDeGoma)
        }
        FaseContacto::Sube if estado == EstadoLapiz::ContactoDeGoma => Some(EstadoLapiz::SinDatos),
        _ => None,
    }
}

/// Los `penFlags` del puntero de un mensaje `WM_POINTER*`, si es un lapiz.
pub fn indicadores_de_lapiz(wparam: WPARAM) -> Option<u32> {
    let id = (wparam.0 & 0xFFFF) as u32;
    let mut tipo = POINTER_INPUT_TYPE::default();
    // SAFETY: id del mensaje actual y salida local.
    if unsafe { GetPointerType(id, &mut tipo) }.is_err() || tipo != PT_PEN {
        return None;
    }
    let mut info = POINTER_PEN_INFO::default();
    // SAFETY: id del mensaje actual y estructura de salida local.
    unsafe { GetPointerPenInfo(id, &mut info) }.ok()?;
    Some(info.penFlags)
}

/// Lo que devuelve leer el historial de puntero de un `WM_POINTERUPDATE`.
#[derive(Debug, Clone, PartialEq)]
pub enum MuestrasLapiz {
    /// No es un lapiz, o fallo la API: nada fiable que usar.
    SinDatos,
    /// La punta en contacto es el borrador o el lapiz esta invertido (D107):
    /// no hay tinta, pero el lapiz SI respondio.
    Goma,
    /// Tinta de verdad, de la mas vieja a la mas nueva.
    Muestras(Vec<Muestra>),
}

/// Las muestras de un `WM_POINTERUPDATE` de lapiz en contacto.
///
/// Distingue "no hay nada fiable" (`SinDatos`) de "el lapiz respondio pero
/// es el borrador" (`Goma`): antes de esta distincion, las dos llegaban como
/// un vector vacio, y el llamante no podia saber si el punto del mensaje
/// debia quedarse (fallback) o tirarse igual (borrador, D107).
pub fn muestras_de_lapiz(wparam: WPARAM) -> MuestrasLapiz {
    let id = (wparam.0 & 0xFFFF) as u32;
    let mut tipo = POINTER_INPUT_TYPE::default();
    // SAFETY: id del mensaje actual y salida local.
    if unsafe { GetPointerType(id, &mut tipo) }.is_err() {
        return MuestrasLapiz::SinDatos;
    }
    if tipo != PT_PEN {
        return MuestrasLapiz::SinDatos;
    }
    let mut cuantos = 0u32;
    // SAFETY: primera llamada solo para saber cuantas entradas hay.
    if unsafe { GetPointerPenInfoHistory(id, &mut cuantos, None) }.is_err() {
        avisar_una_vez("GetPointerPenInfoHistory");
        return MuestrasLapiz::SinDatos;
    }
    let mut buf = vec![POINTER_PEN_INFO::default(); cuantos.max(1) as usize];
    // SAFETY: bufer local del tamano pedido; `cuantos` sale con lo escrito.
    if unsafe { GetPointerPenInfoHistory(id, &mut cuantos, Some(buf.as_mut_ptr())) }.is_err() {
        return MuestrasLapiz::SinDatos;
    }
    buf.truncate(cuantos as usize);
    let Some(primero) = buf.first() else {
        return MuestrasLapiz::SinDatos;
    };
    let dispositivo = primero.pointerInfo.sourceDevice;
    let (mut rd, mut rp) = (RECT::default(), RECT::default());
    // SAFETY: dispositivo del propio mensaje; salidas locales.
    let con_rects = unsafe { GetPointerDeviceRects(dispositivo, &mut rd, &mut rp) }.is_ok();
    let en_contacto: Vec<&POINTER_PEN_INFO> = buf
        .iter()
        .rev()
        .filter(|i| i.pointerInfo.pointerFlags.contains(POINTER_FLAG_INCONTACT))
        .collect();
    if en_contacto.is_empty() {
        return MuestrasLapiz::SinDatos;
    }
    // D107: el borrador del lapiz (o la punta usada al reves) se detecta y
    // se ignora hasta E4, que es donde se implementa borrar con el. Si TODO
    // lo que hay en contacto es borrador, es Goma (el lapiz SI respondio,
    // pero no hay tinta); si hay alguna muestra de tinta de verdad, esa se
    // usa y el resto (si lo hubiera) se descarta en silencio.
    let tinta: Vec<Muestra> = en_contacto
        .iter()
        .filter(|i| !es_goma(i.penFlags))
        .map(|i| {
            let (x, y) = if con_rects {
                himetric_a_pixel(
                    (
                        i.pointerInfo.ptHimetricLocation.x,
                        i.pointerInfo.ptHimetricLocation.y,
                    ),
                    (rd.left, rd.top, rd.right, rd.bottom),
                    (rp.left, rp.top, rp.right, rp.bottom),
                )
            } else {
                (
                    i.pointerInfo.ptPixelLocation.x as f32,
                    i.pointerInfo.ptPixelLocation.y as f32,
                )
            };
            // penMask es un u32 de bits crudo, no un tipo con .contains().
            let presion = (i.penMask & PEN_MASK_PRESSURE != 0).then(|| i.pressure as f32 / 1024.0);
            Muestra::nueva(x, y, presion)
        })
        .collect();
    if tinta.is_empty() {
        MuestrasLapiz::Goma
    } else {
        MuestrasLapiz::Muestras(tinta)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn se_recuperan_los_puntos_fusionados_del_mas_viejo_al_mas_nuevo() {
        // Como lo da Windows: el mas nuevo primero, y el [0] es el del mensaje.
        let recientes = [
            (40, 0, 104),
            (30, 0, 103),
            (20, 0, 102),
            (10, 0, 101),
            (0, 0, 100),
        ];
        let anterior = (10, 0, 101);
        assert_eq!(perdidos_entre(&recientes, anterior), vec![(20, 0), (30, 0)]);
    }

    #[test]
    fn sin_nada_fusionado_no_se_recupera_nada() {
        let recientes = [(40, 0, 104), (30, 0, 103)];
        assert!(perdidos_entre(&recientes, (30, 0, 103)).is_empty());
    }

    #[test]
    fn no_se_cuela_nada_mas_viejo_que_el_ultimo_entregado() {
        // El anterior ya salio del historial (mas de 64 movimientos): se para
        // en el primero con tiempo menor, no se devuelve historia vieja.
        let recientes = [(40, 0, 204), (30, 0, 203), (20, 0, 150), (10, 0, 90)];
        assert_eq!(perdidos_entre(&recientes, (0, 0, 180)), vec![(30, 0)]);
    }

    #[test]
    fn un_anterior_que_no_esta_en_el_historial_para_por_tiempo_y_da_los_mas_nuevos() {
        // Es el caso de `sembrar`: el punto del pulsado no tiene por que ser
        // una entrada exacta del historial. No se encuentra por igualdad, asi
        // que se para en el primero mas viejo que el y devuelve lo posterior.
        let recientes = [
            (40, 5, 110),
            (30, 5, 108),
            (20, 5, 106),
            (12, 3, 104),
            (0, 0, 99),
        ];
        let pulsado = (7, 1, 103);
        assert_eq!(
            perdidos_entre(&recientes, pulsado),
            vec![(12, 3), (20, 5), (30, 5)]
        );
    }

    #[test]
    fn el_borrador_o_la_punta_al_reves_son_goma_y_la_punta_normal_no() {
        assert!(!es_goma(0), "sin indicadores es la punta normal");
        assert!(es_goma(PEN_FLAG_ERASER));
        assert!(es_goma(PEN_FLAG_INVERTED));
        assert!(es_goma(PEN_FLAG_ERASER | PEN_FLAG_INVERTED));
        // El boton del cilindro (PEN_FLAG_BARREL = 1) no es borrar.
        assert!(!es_goma(1));
    }

    #[test]
    fn el_contacto_de_goma_se_traga_de_bajar_a_subir_y_luego_se_olvida() {
        use EstadoLapiz::{ConMuestras, ContactoDeGoma, Goma, SinDatos};
        use FaseContacto::{Actualiza, Baja, Sube};
        let casos = [
            // (fase, es_goma, estado, decision, motivo)
            (
                Baja,
                true,
                SinDatos,
                Some(ContactoDeGoma),
                "bajar con el borrador se traga: no nace un pulsado",
            ),
            (
                Baja,
                false,
                SinDatos,
                None,
                "bajar con la punta sigue por DefWindowProc",
            ),
            (
                Actualiza,
                true,
                ContactoDeGoma,
                Some(ContactoDeGoma),
                "mover el borrador tampoco sintetiza movimientos",
            ),
            (
                Sube,
                true,
                ContactoDeGoma,
                Some(SinDatos),
                "subir el borrador se traga y deja el estado limpio",
            ),
            (
                Actualiza,
                true,
                ConMuestras,
                None,
                "un trazo de tinta que pasa a borrador sigue su camino",
            ),
            (
                Sube,
                true,
                Goma,
                None,
                "su soltado tiene que llegar, o la captura se queda tomada",
            ),
        ];
        for (fase, goma, estado, decision, motivo) in casos {
            assert_eq!(decidir_goma(fase, goma, estado), decision, "{motivo}");
        }
    }

    #[test]
    fn el_movimiento_del_lapiz_no_usa_el_historial_del_raton() {
        assert!(usa_historial_raton(OrigenRaton::Raton));
        assert!(usa_historial_raton(OrigenRaton::Tacto));
        assert!(!usa_historial_raton(OrigenRaton::Lapiz));
    }

    #[test]
    fn en_cuanto_el_lapiz_responde_se_olvida_el_historial_del_raton() {
        assert!(!olvida_historial_raton(EstadoLapiz::SinDatos));
        assert!(olvida_historial_raton(EstadoLapiz::ConMuestras));
        assert!(olvida_historial_raton(EstadoLapiz::Goma));
        assert!(olvida_historial_raton(EstadoLapiz::ContactoDeGoma));
    }

    #[test]
    fn sembrar_da_un_anterior_al_primer_movimiento() {
        // Sin sembrar, el primer movimiento no tiene anterior y no recupera.
        // Con la semilla, el anterior es el pulsado (se comprueba el estado,
        // no la llamada a Win32, que necesita una sesion de escritorio).
        let mut h = HistorialRaton::nuevo();
        assert_eq!(h.anterior, None);
        h.sembrar(3, -4, 77);
        assert_eq!(h.anterior, Some((3, -4, 77)));
        h.olvidar();
        assert_eq!(h.anterior, None);
    }

    #[test]
    fn himetric_se_reparte_proporcional_sobre_la_pantalla() {
        let (x, y) = himetric_a_pixel((5000, 2500), (0, 0, 10000, 5000), (100, 50, 2020, 1130));
        assert!(
            (x - 1060.0).abs() < 0.01 && (y - 590.0).abs() < 0.01,
            "{x},{y}"
        );
    }

    #[test]
    fn un_dispositivo_de_tamano_cero_no_divide_por_cero() {
        let (x, y) = himetric_a_pixel((5, 5), (0, 0, 0, 0), (0, 0, 100, 100));
        assert!(x.is_finite() && y.is_finite());
    }

    #[test]
    fn la_firma_distingue_raton_lapiz_y_tacto() {
        // Un raton de verdad no lleva ninguna firma.
        assert_eq!(origen_del_raton(0), OrigenRaton::Raton);
        // El lapiz: los tres bytes altos de la firma, con el bit 0x80 a cero.
        assert_eq!(origen_del_raton(0xFF51_5700), OrigenRaton::Lapiz);
        // El tacto: la misma firma, pero con el bit 0x80 puesto. Antes de
        // origen_del_raton, esto se confundia con un lapiz y el arrastre
        // con el dedo se tiraba sin dejar ni RatonMovido ni Muestra.
        assert_eq!(origen_del_raton(0xFF51_5780), OrigenRaton::Tacto);
    }

    #[test]
    fn solo_se_tira_el_movimiento_del_lapiz_que_ya_respondio() {
        use EstadoLapiz::{ConMuestras, Goma, SinDatos};
        use OrigenRaton::{Lapiz, Raton, Tacto};
        let casos = [
            // (boton, origen, estado, se_tira, motivo)
            (true, Raton, ConMuestras, false, "un raton nunca se tira"),
            (
                true,
                Tacto,
                ConMuestras,
                false,
                "el tacto no tiene sustituto",
            ),
            (
                true,
                Lapiz,
                SinDatos,
                false,
                "sin datos del lapiz, el punto del mensaje es lo unico que hay",
            ),
            (
                true,
                Lapiz,
                ConMuestras,
                true,
                "con tinta de verdad, el WM_POINTERUPDATE ya lo cubre",
            ),
            (
                true,
                Lapiz,
                Goma,
                true,
                "el borrador se tira igual: si no, reaparece como tinta (D107)",
            ),
            (
                false,
                Lapiz,
                ConMuestras,
                false,
                "sin el boton pulsado no hay trazo que sustituir",
            ),
        ];
        for (boton, origen, estado, se_tira, motivo) in casos {
            assert_eq!(
                descartar_movimiento(boton, origen, estado),
                se_tira,
                "{motivo}"
            );
        }
    }

    #[test]
    fn la_muestra_guarda_subpixel_y_presion_redondeados() {
        let m = Muestra::nueva(10.53, -3.25, Some(0.5));
        // 10.53 * 16 = 168.48, que redondea a 168 (1/16 mas cercano): 10.5.
        assert!((m.x() - 10.5).abs() < 1e-4, "{}", m.x());
        assert!((m.y() + 3.25).abs() < 1e-4);
        assert_eq!(m.presion(), Some(512.0 / 1024.0));
        assert_eq!(Muestra::nueva(0.0, 0.0, None).presion(), None);
    }
}
