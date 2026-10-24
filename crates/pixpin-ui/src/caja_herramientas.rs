//! La caja de herramientas de anotación: dónde está cada botón.
//!
//! Es geometría pura, como la barra de resultado: decide posiciones y dice
//! qué botón hay bajo un punto. Quien la dibuja es el consumidor, con su
//! pintor; quien reacciona es la máquina de anotar.
//!
//! Hay dos formas: la columna pegada a un lado del contenido (`colocar`) y
//! la barra de arriba de Excalidraw (`barra_superior`), que es la que usan
//! hoy todos los anfitriones de dibujo y va AGRUPADA como la del movil: un
//! boton por grupo y sus hermanas en un desplegable (`GrupoBarra`).

use pixpin_geom::{Punto, Rect};

use crate::evento_anotador::Herramienta;

/// Medidas en pixeles logicos (al 100 %).
const LADO_BOTON_LOGICO: u32 = 40;
const HUECO_LOGICO: u32 = 2;
const MARGEN_LOGICO: u32 = 6;
/// Separacion entre la caja y el borde del contenido.
const SEPARACION_LOGICA: u32 = 12;

/// La barra de arriba del editor copia la de Excalidraw
/// (`docs/excalidraw/interfaz.md` §2.4): isla con 4 px de relleno, botones
/// de 36 px separados 4 px, a 16 px del borde, y separadores de 1 px con
/// 4 px de margen entre grupos.
const LADO_BARRA_LOGICO: u32 = 36;
const HUECO_BARRA_LOGICO: u32 = 4;
const RELLENO_BARRA_LOGICO: u32 = 4;
const DISTANCIA_BORDE_LOGICA: u32 = 16;
const SEPARADOR_LOGICO: u32 = 1;
const MARGEN_SEPARADOR_LOGICO: u32 = 4;
const ALTO_SEPARADOR_LOGICO: u32 = 24;

/// Lo que se puede pulsar. Las herramientas y, al final, las acciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonCaja {
    Elegir(Herramienta),
    Deshacer,
    Rehacer,
    /// Abre la paleta de colores (el consumidor decide como).
    Color,
    Salir,
    /// **Meter una imagen** desde un fichero (`Tool.IMAGE` del movil, y el
    /// boton de imagen de la barra de Excalidraw). Es una accion y no una
    /// herramienta: abre el selector de ficheros y pone la foto en el medio
    /// de la vista.
    Imagen,
    /// **Las figuras**: la biblioteca (lo guardado de la seleccion) y las de
    /// fabrica —la grafica de una funcion, la tabla en blanco, pegar una
    /// tabla— (`PanelDeFiguras` del movil, `Library` de Excalidraw).
    Figuras,
    /// **Imprimir** (F11): el dialogo de imprimir de Windows con su vista
    /// previa. Estaba solo en `Ctrl+P` y en el clic derecho, y quien no
    /// sabe atajos no lo encontraba: aqui se ve, con las acciones.
    Imprimir,
    /// **Compartir** (G3 desde el lienzo): la hoja de compartir de toda la
    /// aplicacion. Es el `IosShare` de la barra de arriba del movil
    /// (`DrawEditorActivity`: «Exportar, en la barra y con el icono de
    /// compartir»); aqui estaba solo en `Ctrl+Mayus+S` y en el clic derecho.
    Compartir,
    /// **El clic a traves** del anotador de pantalla viva (el `TouchApp`
    /// de la barra de `CapaPantalla.kt`): el raton pasa a lo de debajo y
    /// la tinta se sigue viendo. Tambien esta en la pastilla de abajo, que
    /// es desde donde se vuelve; aqui esta para que se encuentre en la
    /// barra de siempre (2026-09-29: el usuario no lo veia).
    /// No va en `BOTONES_EDITOR` (la barra del lienzo, la de las pruebas de
    /// medidas): la anade `dibujo::permitidas` solo al anotador vivo.
    Atravesar,
    /// **Un grupo de la barra** (`GRUPOS_DE_FABRICA` del movil): ensena la
    /// herramienta del grupo que este puesta o la ultima usada, y al pulsarlo
    /// la coge y despliega las demas. Solo en la barra agrupada.
    Grupo(GrupoBarra),
}

/// El orden en que se ven. La mano primero porque es a la que se vuelve, y
/// las acciones al final, separadas por su propio grupo.
///
/// Era la caja del `Anotador` viejo (la capa de pantalla y la paleta del
/// pin), que no sabia hacer `Cota`, `Escalar` ni `EscalaGrafica`. Ese
/// anotador se borro el 2026-09-26: hoy todos usan `BOTONES_EDITOR` filtrada
/// por anfitrion (`dibujo::permitidas`). Esta se queda como la caja en
/// columna corta con la que se prueba la colocacion.
pub const BOTONES: [BotonCaja; 14] = [
    BotonCaja::Elegir(Herramienta::Mano),
    BotonCaja::Elegir(Herramienta::Lapiz),
    BotonCaja::Elegir(Herramienta::Resaltador),
    BotonCaja::Elegir(Herramienta::Linea),
    BotonCaja::Elegir(Herramienta::Flecha),
    BotonCaja::Elegir(Herramienta::Rectangulo),
    BotonCaja::Elegir(Herramienta::Elipse),
    BotonCaja::Elegir(Herramienta::Texto),
    BotonCaja::Elegir(Herramienta::Foco),
    BotonCaja::Elegir(Herramienta::Lupa),
    BotonCaja::Elegir(Herramienta::Borrador),
    BotonCaja::Deshacer,
    BotonCaja::Rehacer,
    BotonCaja::Salir,
];

/// La caja del editor avanzado (`apps/pixpin/src/ventana_editor.rs`), la
/// unica superficie que implementa `Cota`, `Escalar` y `EscalaGrafica` de
/// verdad: pinta `ordenes_medibles`, atiende `Peticion::Calibrar` y sabe
/// dibujar su cajetin. Por eso las tres van aqui, con las demas de dibujar,
/// antes de Deshacer.
///
/// En el orden de la barra de Excalidraw (seleccion, rectangulo, elipse,
/// flecha, linea, dibujo, texto, borrador) y despues, en su propio grupo,
/// las que Excalidraw no tiene.
/// Las doce de la tanda cero van aqui con las demas, cada una en el grupo al
/// que pertenece y no todas juntas al final: la barra se lee por grupos, y un
/// cajon de «lo nuevo» deja de tener sentido en cuanto deja de ser nuevo. El
/// rombo entra entre el rectangulo y la elipse porque es una de las diez
/// figuras principales de Excalidraw, y las dos flechas nuevas, junto a la
/// flecha.
///
/// F14 (2026-09-24): la imagen va tras el texto y el laser tras la goma, en
/// su sitio de la barra de Excalidraw; la zona, con el marco (las dos sacan
/// un trozo del lienzo); y las figuras, con las acciones, donde Excalidraw
/// tiene su biblioteca.
///
/// Hoy es el CATALOGO de todo lo que puede salir en la barra, sin agrupar:
/// cada anfitrion se queda con lo suyo (`dibujo::permitidas`) y
/// `barra_superior` lo agrupa con [`BARRA_AGRUPADA`]. Con cuarenta botones
/// sueltos no cabia en una fila ni a 120 % en 1080 p.
pub const BOTONES_EDITOR: [BotonCaja; 39] = [
    BotonCaja::Elegir(Herramienta::Mano),
    BotonCaja::Elegir(Herramienta::Lazo),
    // La bolita, junto al lazo: las dos eligen sin marquesina (`Tool.BOLITA`).
    BotonCaja::Elegir(Herramienta::Bolita),
    BotonCaja::Elegir(Herramienta::Rectangulo),
    BotonCaja::Elegir(Herramienta::Rombo),
    BotonCaja::Elegir(Herramienta::Elipse),
    BotonCaja::Elegir(Herramienta::Flecha),
    BotonCaja::Elegir(Herramienta::FlechaCodos),
    BotonCaja::Elegir(Herramienta::FlechaLibre),
    BotonCaja::Elegir(Herramienta::Linea),
    BotonCaja::Elegir(Herramienta::Lapiz),
    // El grafito junto al lapiz: es un lapiz mas, hecho de otra cosa (v0.75
    // del movil, donde tambien va al lado del lapiz en la barra).
    BotonCaja::Elegir(Herramienta::Grafito),
    BotonCaja::Elegir(Herramienta::Texto),
    BotonCaja::Imagen,
    BotonCaja::Elegir(Herramienta::Borrador),
    BotonCaja::Elegir(Herramienta::Laser),
    BotonCaja::Elegir(Herramienta::Resaltador),
    BotonCaja::Elegir(Herramienta::Foco),
    BotonCaja::Elegir(Herramienta::Lupa),
    BotonCaja::Elegir(Herramienta::Mosaico),
    BotonCaja::Elegir(Herramienta::Arco),
    BotonCaja::Elegir(Herramienta::Serie),
    BotonCaja::Elegir(Herramienta::Punto),
    // La marca de angulo con su letra griega, junto al punto con letra
    // (17-oct): las dos nombran lo que hay en una figura de geometria.
    BotonCaja::Elegir(Herramienta::Angulo),
    BotonCaja::Elegir(Herramienta::Cota),
    BotonCaja::Elegir(Herramienta::Escalar),
    BotonCaja::Elegir(Herramienta::EscalaGrafica),
    BotonCaja::Elegir(Herramienta::Zona),
    // Las cinco que no dibujan nada: miran lo que ya hay y lo cambian.
    BotonCaja::Elegir(Herramienta::Relleno),
    BotonCaja::Elegir(Herramienta::Recortar),
    BotonCaja::Elegir(Herramienta::Extender),
    // Soldar vertices (`Tool.NUDO`), junto a recortar y extender como en el
    // movil: las tres arreglan la geometria de lo ya trazado.
    BotonCaja::Elegir(Herramienta::Nudo),
    BotonCaja::Elegir(Herramienta::CopiarEstilo),
    BotonCaja::Figuras,
    // Imprimir con las acciones, antes de deshacer: es lo que se busca al
    // acabar, cerca de Salir, y no se confunde con una herramienta.
    BotonCaja::Imprimir,
    // Compartir junto a imprimir: las dos sacan el dibujo del lienzo.
    BotonCaja::Compartir,
    BotonCaja::Deshacer,
    BotonCaja::Rehacer,
    BotonCaja::Salir,
];

/// **Los grupos de la barra**, copiados de `GRUPOS_DE_FABRICA` del movil
/// (`motor/Barra.kt`): lo que hace lo mismo, junto. La barra ensena UN boton
/// por grupo -la herramienta de ese grupo que tengas puesta, o la ultima que
/// usaste- y el resto sale en un desplegable al pulsarlo.
///
/// Con cuarenta botones sueltos la barra ya no cabia en una fila en un
/// portatil (1366 px al 125 %, 1080 p al 150 %). Esconderlas detras de un
/// «mas» seria dos clics para coger un rectangulo; agrupar por parecido deja
/// cada una a un clic de su hermana, que es lo que resolvio el movil.
///
/// Diferencias con el movil, y por que:
///
/// - La mano (elegir), el lapiz y la goma van SUELTOS y no en su grupo: son
///   las de uso constante, y en el movil la seleccion es la cara de su grupo
///   casi siempre. Aqui se asegura que lo estan.
/// - Hay herramientas que el movil no tiene (el arco, la flecha de codos,
///   copiar estilo, el laser) y cada una va con sus parecidas.
/// - La tabla y la grafica («Figuras») van sueltas: su grupo era el del
///   cronograma, que se quito por no estar bien hecho.
/// - Compartir e imprimir, que el movil tiene en la barra de arriba, van en
///   su propio grupo al lado de deshacer: las dos sacan el dibujo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GrupoBarra {
    /// Elegir de otras formas: el lazo, la bolita y la zona.
    Elegir,
    /// Lo que pinta a mano ademas del lapiz: el grafito, el resaltador y el
    /// bote («el bote no traza, pero lo que se hace con el es dar color»).
    Trazar,
    Formas,
    Flechas,
    /// Lo que arregla una raya ya trazada.
    Arreglar,
    /// Lo que nombra cosas del dibujo: el texto, el numerito, el punto y el
    /// angulo con su letra griega.
    Nombrar,
    /// Lo que tapa o senala: pixelar, la lupa, el foco y el laser.
    Tapar,
    Medir,
    /// Sacar el dibujo: compartir e imprimir.
    Sacar,
}

impl GrupoBarra {
    pub const TODOS: [GrupoBarra; 9] = [
        GrupoBarra::Elegir,
        GrupoBarra::Trazar,
        GrupoBarra::Formas,
        GrupoBarra::Flechas,
        GrupoBarra::Arreglar,
        GrupoBarra::Nombrar,
        GrupoBarra::Tapar,
        GrupoBarra::Medir,
        GrupoBarra::Sacar,
    ];

    /// Lo que hay dentro, en el orden del desplegable. La primera es la cara
    /// del grupo mientras no se haya usado otra.
    pub fn miembros(self) -> &'static [BotonCaja] {
        use BotonCaja::Elegir as E;
        use Herramienta as H;
        match self {
            GrupoBarra::Elegir => &[E(H::Lazo), E(H::Bolita), E(H::Zona)],
            GrupoBarra::Trazar => &[E(H::Grafito), E(H::Resaltador), E(H::Relleno)],
            GrupoBarra::Formas => &[E(H::Rectangulo), E(H::Elipse), E(H::Rombo), E(H::Arco)],
            GrupoBarra::Flechas => &[
                E(H::Flecha),
                E(H::FlechaLibre),
                E(H::Linea),
                E(H::FlechaCodos),
            ],
            GrupoBarra::Arreglar => &[
                E(H::Recortar),
                E(H::Extender),
                E(H::Nudo),
                E(H::CopiarEstilo),
            ],
            GrupoBarra::Nombrar => &[E(H::Texto), E(H::Serie), E(H::Punto), E(H::Angulo)],
            GrupoBarra::Tapar => &[E(H::Mosaico), E(H::Lupa), E(H::Foco), E(H::Laser)],
            GrupoBarra::Medir => &[E(H::Cota), E(H::Escalar), E(H::EscalaGrafica)],
            GrupoBarra::Sacar => &[BotonCaja::Compartir, BotonCaja::Imprimir],
        }
    }

    /// El nombre estable del grupo: su titulo es `barra-grupo-<nombre>` en
    /// los `.ftl`.
    pub fn nombre(self) -> &'static str {
        match self {
            GrupoBarra::Elegir => "elegir",
            GrupoBarra::Trazar => "trazar",
            GrupoBarra::Formas => "formas",
            GrupoBarra::Flechas => "flechas",
            GrupoBarra::Arreglar => "arreglar",
            GrupoBarra::Nombrar => "nombrar",
            GrupoBarra::Tapar => "tapar",
            GrupoBarra::Medir => "medir",
            GrupoBarra::Sacar => "sacar",
        }
    }

    /// Su sitio en [`GrupoBarra::TODOS`], para quien guarda algo por grupo.
    pub fn indice(self) -> usize {
        self as usize
    }
}

/// El grupo al que pertenece un boton, o `None` si va suelto.
pub fn grupo_de_boton(b: BotonCaja) -> Option<GrupoBarra> {
    GrupoBarra::TODOS
        .into_iter()
        .find(|g| g.miembros().contains(&b))
}

/// **La barra agrupada**, en el orden de la del movil: elegir, lo que pinta,
/// las formas, las flechas, lo que arregla, lo que nombra, lo que tapa, lo
/// que mide, la tabla y la grafica, el marco y la goma; y al final lo que saca el
/// dibujo, el clic a traves (solo en la pantalla viva) y las acciones.
/// Dieciocho botones en vez de cuarenta y uno.
pub const BARRA_AGRUPADA: [BotonCaja; 18] = [
    BotonCaja::Elegir(Herramienta::Mano),
    BotonCaja::Grupo(GrupoBarra::Elegir),
    BotonCaja::Elegir(Herramienta::Lapiz),
    BotonCaja::Grupo(GrupoBarra::Trazar),
    BotonCaja::Grupo(GrupoBarra::Formas),
    BotonCaja::Grupo(GrupoBarra::Flechas),
    BotonCaja::Grupo(GrupoBarra::Arreglar),
    BotonCaja::Grupo(GrupoBarra::Nombrar),
    BotonCaja::Grupo(GrupoBarra::Tapar),
    BotonCaja::Grupo(GrupoBarra::Medir),
    // La tabla y la grafica, sueltas: eran un grupo con el cronograma, y sin
    // el serian un grupo de uno.
    BotonCaja::Figuras,
    // La imagen, suelta: iba en un grupo con el marco, y el marco ya no va
    // en la barra (22-oct, el usuario: no duplicar herramientas): lo pone el
    // boton del riel de la derecha, o la C.
    BotonCaja::Imagen,
    // La goma al final de las herramientas, como en el movil.
    BotonCaja::Elegir(Herramienta::Borrador),
    // Suelto y a la vista: es lo primero que se busca en la pantalla.
    BotonCaja::Atravesar,
    BotonCaja::Grupo(GrupoBarra::Sacar),
    BotonCaja::Deshacer,
    BotonCaja::Rehacer,
    BotonCaja::Salir,
];

/// **La barra de un anfitrion**: la agrupada, quedandose solo con lo que
/// sale ahi (`permitidos`, ya filtrado por los ajustes y por lo que el
/// anfitrion sabe hacer). Un grupo sin nada permitido desaparece entero.
///
/// Y **nada se pierde por el camino** (`gruposDe` del movil): un boton
/// permitido que no este ni suelto ni en ningun grupo sale al final, solo.
/// Es la red para que una herramienta nueva no quede inalcanzable si alguien
/// olvida darle grupo.
pub fn agrupar(permitidos: &[BotonCaja]) -> Vec<BotonCaja> {
    let mut v: Vec<BotonCaja> = BARRA_AGRUPADA
        .into_iter()
        .filter(|b| match b {
            BotonCaja::Grupo(g) => g.miembros().iter().any(|m| permitidos.contains(m)),
            otro => permitidos.contains(otro),
        })
        .collect();
    for b in permitidos {
        let con_sitio = BARRA_AGRUPADA.contains(b) || grupo_de_boton(*b).is_some();
        if !con_sitio && !v.contains(b) {
            v.push(*b);
        }
    }
    v
}

/// **La cara de un grupo**: la que se ve en la barra. La puesta si esta
/// dentro; si no, la ultima que se uso de el; si no, la primera (`caraDelGrupo`
/// del movil, con su mapa de `ultimas`). `miembros` es lo que sale en este
/// anfitrion: una recordada que aqui no sale no vale.
pub fn cara_del_grupo(
    miembros: &[BotonCaja],
    activa: Herramienta,
    recordada: Option<BotonCaja>,
) -> Option<BotonCaja> {
    let puesta = BotonCaja::Elegir(activa);
    if miembros.contains(&puesta) {
        return Some(puesta);
    }
    if let Some(r) = recordada
        && miembros.contains(&r)
    {
        return Some(r);
    }
    miembros.first().copied()
}

/// Cuantos botones caben en una barra: la lista plana entera y de sobra.
const MAX_BARRA: usize = 48;

/// El desplegable de un grupo, el `DropdownMenu` de Excalidraw
/// (`docs/excalidraw/interfaz.md` §2.4): entradas de 32 px con el icono y el
/// nombre, 1 px entre ellas, en una isla bajo el boton.
const ANCHO_MENU_LOGICO: u32 = 212;
const FILA_MENU_LOGICA: u32 = 32;
const HUECO_MENU_LOGICO: u32 = 1;
const RELLENO_MENU_LOGICO: u32 = 6;
const BAJO_LA_BARRA_LOGICO: u32 = 6;

/// El desplegable abierto: su isla y la fila de cada hermana.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuGrupo {
    pub grupo: GrupoBarra,
    pub marco: Rect,
    pub filas: Vec<(BotonCaja, Rect)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CajaHerramientas {
    pub marco: Rect,
    escala_por_cien: u32,
    /// Lo que se ve en la caja, en orden: botones sueltos y grupos. Va en un
    /// arreglo y no en una rebanada porque la barra agrupada se calcula al
    /// colocarla, y asi la caja sigue siendo `Copy` y sin prestamos.
    barra: [BotonCaja; MAX_BARRA],
    cuantos: usize,
    /// Todo lo que sale en este anfitrion, sin agrupar: lo que hay dentro de
    /// cada grupo es su lista de miembros quedandose con estos.
    permitidos: &'static [BotonCaja],
    /// Barra de arriba al estilo Excalidraw en vez de columna al lado.
    horizontal: bool,
    /// Donde empieza la segunda fila de la barra: `cuantos` si va en una
    /// sola. Solo en la barra horizontal.
    corte: usize,
    /// Donde se puede pintar: el desplegable no se sale de aqui.
    area: Rect,
    /// El grupo desplegado, si hay uno. Lo decide quien atiende los clics
    /// (`dibujo::mano`); la caja solo lo coloca y lo tiene en cuenta al
    /// decir que hay bajo el raton.
    desplegado: Option<GrupoBarra>,
}

/// La seccion de un boton en la barra: entre secciones va un separador.
/// Elegir; lo que dibuja; lo que tapa, mide o saca un trozo; y las acciones.
///
/// Que lo que no dibuja (el bote, recortar, copiar estilo) vaya en su propio
/// grupo es lo que mas se agradece: mezcladas con las que si dibujan, un
/// clic con una de ellas puesta parece que no ha hecho nada cuando lo que ha
/// pasado es que no habia nada cerca sobre lo que actuar.
pub fn grupo(b: BotonCaja) -> u8 {
    fn seccion(g: GrupoBarra) -> u8 {
        match g {
            GrupoBarra::Elegir => 0,
            GrupoBarra::Trazar
            | GrupoBarra::Formas
            | GrupoBarra::Flechas
            | GrupoBarra::Arreglar
            | GrupoBarra::Nombrar => 1,
            GrupoBarra::Tapar | GrupoBarra::Medir => 2,
            GrupoBarra::Sacar => 3,
        }
    }
    match b {
        BotonCaja::Grupo(g) => seccion(g),
        BotonCaja::Elegir(Herramienta::Mano) => 0,
        BotonCaja::Elegir(Herramienta::Lapiz) => 1,
        BotonCaja::Elegir(Herramienta::Borrador) => 2,
        // La tabla y la grafica, con lo que tapa, mide y enmarca.
        BotonCaja::Figuras => 2,
        // La imagen, suelta junto a las figuras (iba en el grupo del marco).
        BotonCaja::Imagen => 2,
        BotonCaja::Deshacer | BotonCaja::Rehacer | BotonCaja::Color | BotonCaja::Salir => 3,
        otro => grupo_de_boton(otro).map_or(1, seccion),
    }
}

/// Lo que mide a lo ancho una fila de la barra con esos botones.
fn ancho_de_fila(botones: &[BotonCaja], escala_por_cien: u32) -> u32 {
    let e = |v: u32| v * escala_por_cien / 100;
    let n = botones.len() as u32;
    let cortes = botones
        .windows(2)
        .filter(|par| grupo(par[0]) != grupo(par[1]))
        .count() as u32;
    2 * e(RELLENO_BARRA_LOGICO)
        + n * e(LADO_BARRA_LOGICO)
        + n.saturating_sub(1) * e(HUECO_BARRA_LOGICO)
        + cortes * (e(SEPARADOR_LOGICO) + e(MARGEN_SEPARADOR_LOGICO) + e(HUECO_BARRA_LOGICO))
}

/// **Donde partir la barra en dos filas** si en una no cabe en `ancho`: por
/// el separador de grupo que deja las dos filas mas parejas. Con los grupos
/// ya solo pasa en ventanas muy estrechas. `botones.len()` si cabe en una.
fn corte_para(botones: &[BotonCaja], escala_por_cien: u32, ancho: u32) -> usize {
    let n = botones.len();
    if ancho_de_fila(botones, escala_por_cien) <= ancho {
        return n;
    }
    (1..n)
        .filter(|&i| grupo(botones[i - 1]) != grupo(botones[i]))
        .min_by_key(|&i| {
            ancho_de_fila(&botones[..i], escala_por_cien)
                .max(ancho_de_fila(&botones[i..], escala_por_cien))
        })
        .unwrap_or(n)
}

fn a_arreglo(v: &[BotonCaja]) -> ([BotonCaja; MAX_BARRA], usize) {
    let mut a = [BotonCaja::Salir; MAX_BARRA];
    let n = v.len().min(MAX_BARRA);
    a[..n].copy_from_slice(&v[..n]);
    (a, n)
}

impl CajaHerramientas {
    /// La barra de herramientas de Excalidraw, AGRUPADA: en horizontal,
    /// centrada arriba del `area` y a 16 px de su borde. `permitidos` es lo
    /// que sale en este anfitrion, sin agrupar (`dibujo::permitidas`). Si
    /// aun asi no cabe a lo ancho, se parte en dos filas por un separador; y
    /// si ni asi, se pega a la izquierda en vez de salirse por los dos lados.
    pub fn barra_superior(
        area: Rect,
        escala_por_cien: u32,
        permitidos: &'static [BotonCaja],
    ) -> CajaHerramientas {
        let e = |v: u32| v * escala_por_cien / 100;
        let (barra, cuantos) = a_arreglo(&agrupar(permitidos));
        let botones = &barra[..cuantos];
        let corte = corte_para(botones, escala_por_cien, area.ancho);
        let ancho = ancho_de_fila(&botones[..corte], escala_por_cien)
            .max(ancho_de_fila(&botones[corte..], escala_por_cien));
        let filas = if corte < cuantos { 2 } else { 1 };
        let alto = 2 * e(RELLENO_BARRA_LOGICO)
            + filas * e(LADO_BARRA_LOGICO)
            + (filas - 1) * e(RELLENO_BARRA_LOGICO);
        let x_ideal = area.x + (area.ancho as i32 - ancho as i32) / 2;
        let x_max = (area.derecha() - ancho as i32).max(area.izquierda());
        CajaHerramientas {
            marco: Rect {
                x: x_ideal.clamp(area.izquierda(), x_max),
                y: area.y + e(DISTANCIA_BORDE_LOGICA) as i32,
                ancho,
                alto,
            },
            escala_por_cien,
            barra,
            cuantos,
            permitidos,
            horizontal: true,
            corte,
            area,
            desplegado: None,
        }
    }

    /// Cuantas filas tiene la barra.
    pub fn filas(&self) -> u32 {
        if self.horizontal && self.corte < self.cuantos {
            2
        } else {
            1
        }
    }

    /// Cuanto se desplaza el boton `indice` por los separadores que tiene
    /// delante EN SU FILA, en pixeles fisicos.
    fn desplazamiento_separadores(&self, indice: usize) -> u32 {
        let e = |v: u32| v * self.escala_por_cien / 100;
        let botones = self.botones();
        let indice = indice.min(botones.len().saturating_sub(1));
        let desde = if indice >= self.corte { self.corte } else { 0 };
        let antes = botones[desde..=indice]
            .windows(2)
            .filter(|par| grupo(par[0]) != grupo(par[1]))
            .count() as u32;
        antes * (e(SEPARADOR_LOGICO) + e(MARGEN_SEPARADOR_LOGICO) + e(HUECO_BARRA_LOGICO))
    }

    /// Los separadores de 1 px entre grupos de la barra. Vacio en la caja
    /// vertical, que no los lleva. Donde se parte la barra no hay separador:
    /// ya separa el cambio de fila.
    pub fn separadores(&self) -> Vec<Rect> {
        if !self.horizontal {
            return Vec::new();
        }
        let e = |v: u32| v * self.escala_por_cien / 100;
        let alto = e(ALTO_SEPARADOR_LOGICO);
        let botones = self.botones();
        (1..botones.len())
            .filter(|&i| i != self.corte && grupo(botones[i - 1]) != grupo(botones[i]))
            .map(|i| {
                let previo = self.rect_de(i - 1);
                Rect {
                    x: previo.derecha() + e(HUECO_BARRA_LOGICO) as i32,
                    y: previo.y + (previo.alto as i32 - alto as i32) / 2,
                    ancho: e(SEPARADOR_LOGICO).max(1),
                    alto,
                }
            })
            .collect()
    }

    pub fn es_horizontal(&self) -> bool {
        self.horizontal
    }

    /// A la izquierda del contenido si cabe; si no, a la derecha; si tampoco,
    /// dentro y pegada al borde izquierdo. Siempre entera en el area de
    /// trabajo: una caja medio fuera de pantalla no se puede usar. La columna
    /// no se agrupa: es corta y cada boton es suyo.
    pub fn colocar(
        contenido: Rect,
        area_trabajo: Rect,
        escala_por_cien: u32,
        botones: &'static [BotonCaja],
    ) -> CajaHerramientas {
        let e = |v: u32| v * escala_por_cien / 100;
        let lado = e(LADO_BOTON_LOGICO);
        let hueco = e(HUECO_LOGICO);
        let margen = e(MARGEN_LOGICO);
        let (barra, cuantos) = a_arreglo(botones);
        let n = cuantos as u32;

        let ancho = lado + 2 * margen;
        let alto = n * lado + n.saturating_sub(1) * hueco + 2 * margen;
        let sep = e(SEPARACION_LOGICA) as i32;

        let izquierda = contenido.x - sep - ancho as i32;
        let derecha = contenido.x + contenido.ancho as i32 + sep;
        let x = if izquierda >= area_trabajo.izquierda() {
            izquierda
        } else if derecha + ancho as i32 <= area_trabajo.derecha() {
            derecha
        } else {
            contenido.x
        };

        // Centrada en vertical sobre el contenido, y luego sujeta al area:
        // con muchas herramientas la caja es alta y en un monitor pequeño se
        // saldria por arriba y por abajo a la vez.
        let y_ideal = contenido.y + (contenido.alto as i32 - alto as i32) / 2;
        let y_max = (area_trabajo.abajo() - alto as i32).max(area_trabajo.arriba());
        let y = y_ideal.clamp(area_trabajo.arriba(), y_max);

        CajaHerramientas {
            marco: Rect {
                x: x.clamp(
                    area_trabajo.izquierda(),
                    (area_trabajo.derecha() - ancho as i32).max(area_trabajo.izquierda()),
                ),
                y,
                ancho,
                alto,
            },
            escala_por_cien,
            barra,
            cuantos,
            permitidos: botones,
            horizontal: false,
            corte: cuantos,
            area: area_trabajo,
            desplegado: None,
        }
    }

    /// El rectangulo de un boton de la barra por su indice.
    pub fn rect_de(&self, indice: usize) -> Rect {
        let e = |v: u32| v * self.escala_por_cien / 100;
        if self.horizontal {
            let lado = e(LADO_BARRA_LOGICO);
            let relleno = e(RELLENO_BARRA_LOGICO);
            let paso = lado + e(HUECO_BARRA_LOGICO);
            let (fila, en_fila) = if indice >= self.corte {
                (1, indice - self.corte)
            } else {
                (0, indice)
            };
            return Rect {
                x: self.marco.x
                    + (relleno + en_fila as u32 * paso + self.desplazamiento_separadores(indice))
                        as i32,
                y: self.marco.y + (relleno + fila * (lado + relleno)) as i32,
                ancho: lado,
                alto: lado,
            };
        }
        let lado = e(LADO_BOTON_LOGICO);
        let hueco = e(HUECO_LOGICO);
        let margen = e(MARGEN_LOGICO);
        Rect {
            x: self.marco.x + margen as i32,
            y: self.marco.y + margen as i32 + indice as i32 * (lado + hueco) as i32,
            ancho: lado,
            alto: lado,
        }
    }

    /// Lo que hay dentro de un grupo EN ESTE anfitrion, en su orden.
    pub fn miembros(&self, g: GrupoBarra) -> Vec<BotonCaja> {
        g.miembros()
            .iter()
            .copied()
            .filter(|m| self.permitidos.contains(m))
            .collect()
    }

    /// El grupo desplegado.
    pub fn desplegado(&self) -> Option<GrupoBarra> {
        self.desplegado
    }

    /// La misma caja con ese grupo desplegado (o ninguno). Un grupo que no
    /// esta en esta barra, o con una sola herramienta, no se despliega: no
    /// hay hermanas que ensenar (`grupo.size <= 1` del movil).
    pub fn con_desplegado(mut self, g: Option<GrupoBarra>) -> CajaHerramientas {
        self.desplegado = g.filter(|g| {
            self.horizontal
                && self.botones().contains(&BotonCaja::Grupo(*g))
                && self.miembros(*g).len() > 1
        });
        self
    }

    /// **El desplegable abierto**: una isla bajo el boton del grupo con una
    /// fila por hermana. No se sale del area por la derecha: el grupo de
    /// sacar va al final de la barra.
    pub fn menu(&self) -> Option<MenuGrupo> {
        let g = self.desplegado?;
        let i = self
            .botones()
            .iter()
            .position(|b| *b == BotonCaja::Grupo(g))?;
        let miembros = self.miembros(g);
        let e = |v: u32| v * self.escala_por_cien / 100;
        let boton = self.rect_de(i);
        let relleno = e(RELLENO_MENU_LOGICO);
        let fila = e(FILA_MENU_LOGICA);
        let hueco = e(HUECO_MENU_LOGICO);
        let ancho = e(ANCHO_MENU_LOGICO);
        let n = miembros.len() as u32;
        let alto = 2 * relleno + n * fila + n.saturating_sub(1) * hueco;
        let x_max = (self.area.derecha() - ancho as i32).max(self.area.izquierda());
        let x = (boton.x - relleno as i32).clamp(self.area.izquierda(), x_max);
        let y = self.marco.abajo() + e(BAJO_LA_BARRA_LOGICO) as i32;
        let filas = miembros
            .into_iter()
            .enumerate()
            .map(|(k, b)| {
                (
                    b,
                    Rect {
                        x: x + relleno as i32,
                        y: y + (relleno + k as u32 * (fila + hueco)) as i32,
                        ancho: ancho - 2 * relleno,
                        alto: fila,
                    },
                )
            })
            .collect();
        Some(MenuGrupo {
            grupo: g,
            marco: Rect { x, y, ancho, alto },
            filas,
        })
    }

    /// Donde esta un boton, en la barra o en el desplegable abierto. Para
    /// colgar debajo lo que abre (el menu de las figuras).
    pub fn rect_de_boton(&self, b: BotonCaja) -> Option<Rect> {
        if let Some(i) = self.botones().iter().position(|x| *x == b) {
            return Some(self.rect_de(i));
        }
        self.menu()?
            .filas
            .into_iter()
            .find(|(x, _)| *x == b)
            .map(|(_, r)| r)
    }

    /// Que boton hay bajo el punto, si hay alguno: primero el desplegable,
    /// que va encima de todo.
    pub fn boton_en(&self, p: Punto) -> Option<BotonCaja> {
        if let Some(m) = self.menu()
            && m.marco.contiene(p)
        {
            return m.filas.iter().find(|(_, r)| r.contiene(p)).map(|(b, _)| *b);
        }
        if !self.marco.contiene(p) {
            return None;
        }
        (0..self.cuantos)
            .find(|i| self.rect_de(*i).contiene(p))
            .map(|i| self.barra[i])
    }

    /// Lo que se ve en la caja, en orden: quien la pinta lo necesita para
    /// saber que dibujar en cada indice. En la barra, los grupos van como
    /// [`BotonCaja::Grupo`].
    pub fn botones(&self) -> &[BotonCaja] {
        &self.barra[..self.cuantos]
    }

    /// Si el punto cae sobre la caja o su desplegable. Sirve para NO empezar
    /// un trazo al pulsar un boton: sin esto, elegir el lapiz dejaria un
    /// punto de tinta.
    pub fn contiene(&self, p: Punto) -> bool {
        self.marco.contiene(p) || self.menu().is_some_and(|m| m.marco.contiene(p))
    }

    /// A que le pertenece un punto del raton: a un boton concreto, al hueco
    /// de la caja (entre botones o en su margen), o al lienzo de debajo.
    ///
    /// Junta `boton_en` y `contiene` en la UNICA pregunta que hace falta
    /// antes de dejar pasar un clic al gesto: `ventana_editor::abrir` la
    /// resolvia a mano con un `if`/`continue` dentro del bucle de eventos, un
    /// sitio que no se puede probar sin ventana -y por eso un revisor pudo
    /// apagar la guarda entera con un `if false` sin que ninguna de las 842
    /// pruebas se enterara. Sacar la decision aqui, pura, es lo que la pone
    /// bajo vigilancia.
    pub fn destino(&self, p: Punto) -> DestinoClic {
        match self.boton_en(p) {
            Some(b) => DestinoClic::Boton(b),
            None if self.contiene(p) => DestinoClic::Caja,
            None => DestinoClic::Lienzo,
        }
    }
}

/// El resultado de `CajaHerramientas::destino`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinoClic {
    /// Encima de un boton: el clic lo elige o dispara, no llega al lienzo.
    Boton(BotonCaja),
    /// Dentro del marco pero fuera de todo boton (el hueco o el margen):
    /// sigue sin ser del lienzo, para no dejar un punto de tinta detras de
    /// la barra.
    Caja,
    /// Fuera de la caja: le toca al gesto de siempre.
    Lienzo,
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        }
    }

    fn contenido() -> Rect {
        Rect {
            x: 600,
            y: 300,
            ancho: 400,
            alto: 300,
        }
    }

    #[test]
    fn la_caja_va_a_la_izquierda_si_cabe() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        assert!(
            c.marco.derecha() <= contenido().x,
            "deberia quedar a la izquierda del contenido"
        );
        assert!(c.marco.x >= 0);
    }

    #[test]
    fn si_no_cabe_a_la_izquierda_se_va_a_la_derecha() {
        // Un pin pegado al borde izquierdo de la pantalla.
        let pegado = Rect {
            x: 5,
            y: 300,
            ancho: 400,
            alto: 300,
        };
        let c = CajaHerramientas::colocar(pegado, area(), 100, &BOTONES);
        assert!(
            c.marco.x >= pegado.derecha(),
            "deberia irse a la derecha, esta en {}",
            c.marco.x
        );
    }

    #[test]
    fn la_caja_nunca_se_sale_del_area_de_trabajo() {
        // Caso negativo del centrado: en un monitor bajo, una caja de 14
        // botones no cabe centrada y se saldria por arriba y por abajo.
        let bajo = Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 500,
        };
        for y in [-200, 0, 250, 480, 900] {
            let c = CajaHerramientas::colocar(
                Rect {
                    x: 600,
                    y,
                    ancho: 400,
                    alto: 300,
                },
                bajo,
                100,
                &BOTONES,
            );
            assert!(
                c.marco.arriba() >= bajo.arriba(),
                "se sale por arriba: {c:?}"
            );
            assert!(
                c.marco.izquierda() >= bajo.izquierda() && c.marco.derecha() <= bajo.derecha(),
                "se sale de lado: {c:?}"
            );
        }
    }

    #[test]
    fn cada_boton_cae_dentro_del_marco_y_no_se_solapa_con_el_siguiente() {
        let c = CajaHerramientas::colocar(contenido(), area(), 150, &BOTONES);
        for i in 0..BOTONES.len() {
            let r = c.rect_de(i);
            assert!(
                r.arriba() >= c.marco.arriba() && r.abajo() <= c.marco.abajo(),
                "el boton {i} se sale del marco"
            );
            if i + 1 < BOTONES.len() {
                assert!(
                    r.abajo() <= c.rect_de(i + 1).arriba(),
                    "los botones {i} y {} se solapan",
                    i + 1
                );
            }
        }
    }

    #[test]
    fn se_encuentra_el_boton_bajo_el_punto() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let r = c.rect_de(1); // el lapiz
        let centro = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(
            c.boton_en(centro),
            Some(BotonCaja::Elegir(Herramienta::Lapiz))
        );
    }

    #[test]
    fn fuera_de_la_caja_no_hay_boton() {
        // Es lo que distingue "elegir herramienta" de "empezar a dibujar":
        // sin esto, pulsar junto a la caja no dibujaria.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        assert_eq!(c.boton_en(Punto { x: 1500, y: 900 }), None);
        assert!(!c.contiene(Punto { x: 1500, y: 900 }));
    }

    #[test]
    fn en_el_hueco_entre_botones_no_hay_boton_pero_si_caja() {
        // El hueco pertenece a la caja: pulsar ahi no debe empezar un trazo
        // por detras de la barra.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let r0 = c.rect_de(0);
        let hueco = Punto {
            x: r0.x + 1,
            y: r0.abajo() + 1,
        };
        assert_eq!(c.boton_en(hueco), None);
        assert!(c.contiene(hueco), "el hueco sigue siendo de la caja");
    }

    #[test]
    fn estan_las_once_herramientas_del_anotador_y_las_tres_acciones() {
        // BOTONES era la caja del anotador viejo: no lleva Cota, Escalar ni
        // EscalaGrafica porque aquel no sabia hacerlas. Ofrecer un boton que
        // no hace nada es peor que no ofrecerlo.
        let herramientas = BOTONES
            .iter()
            .filter(|b| matches!(b, BotonCaja::Elegir(_)))
            .count();
        assert_eq!(herramientas, 11, "faltan o sobran herramientas en la caja");
        assert!(!BOTONES.contains(&BotonCaja::Elegir(Herramienta::Cota)));
        assert!(!BOTONES.contains(&BotonCaja::Elegir(Herramienta::Escalar)));
        assert!(!BOTONES.contains(&BotonCaja::Elegir(Herramienta::EscalaGrafica)));
        assert!(BOTONES.contains(&BotonCaja::Deshacer));
        assert!(BOTONES.contains(&BotonCaja::Rehacer));
        assert!(BOTONES.contains(&BotonCaja::Salir));
    }

    #[test]
    fn estan_las_tres_herramientas_de_medir_en_la_caja_del_editor() {
        // BOTONES_EDITOR es la caja de `ventana_editor.rs`, la unica
        // superficie que implementa medir de verdad.
        let herramientas = BOTONES_EDITOR
            .iter()
            .filter(|b| matches!(b, BotonCaja::Elegir(_)))
            .count();
        // 28 desde que entro el grafito, junto al lapiz; 31 con la zona, el
        // laser y el cronograma (F8, F14, F12); 32 con soldar vertices, 33 con
        // la bolita y 32 otra vez al quitar el cronograma; 33 con la marca de
        // angulo (laminas de geometria); 32 sin el marco, que pone el riel (22-oct).
        assert_eq!(herramientas, 32, "faltan o sobran herramientas en la caja");
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Imagen));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Figuras));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Elegir(Herramienta::Grafito)));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Elegir(Herramienta::Cota)));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Elegir(Herramienta::Escalar)));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Elegir(Herramienta::EscalaGrafica)));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Deshacer));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Rehacer));
        assert!(BOTONES_EDITOR.contains(&BotonCaja::Salir));
    }

    #[test]
    fn destino_de_un_punto_dentro_de_un_boton_es_ese_boton() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let r = c.rect_de(1); // el lapiz
        let centro = Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        };
        assert_eq!(
            c.destino(centro),
            DestinoClic::Boton(BotonCaja::Elegir(Herramienta::Lapiz))
        );
    }

    #[test]
    fn destino_del_hueco_entre_botones_es_la_caja_no_el_lienzo() {
        // El caso que protegia la guarda de `ventana_editor::abrir`: sin
        // esto, un clic en el hueco caeria al lienzo y dejaria un trazo
        // detras de la barra.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let r0 = c.rect_de(0);
        let hueco = Punto {
            x: r0.x + 1,
            y: r0.abajo() + 1,
        };
        assert_eq!(c.destino(hueco), DestinoClic::Caja);
    }

    #[test]
    fn destino_justo_fuera_del_marco_es_el_lienzo() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let fuera = Punto {
            x: c.marco.derecha() + 50,
            y: c.marco.y,
        };
        assert_eq!(c.destino(fuera), DestinoClic::Lienzo);
    }

    #[test]
    fn destino_en_los_bordes_del_marco() {
        // Media apertura (`Rect::contiene`): el borde superior/izquierdo
        // pertenece al marco, el primer pixel tras el inferior/derecho no.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        let esquina_dentro = Punto {
            x: c.marco.izquierda(),
            y: c.marco.arriba(),
        };
        assert_ne!(c.destino(esquina_dentro), DestinoClic::Lienzo);

        let justo_fuera = Punto {
            x: c.marco.derecha(),
            y: c.marco.abajo() - 1,
        };
        assert_eq!(c.destino(justo_fuera), DestinoClic::Lienzo);

        let tambien_fuera = Punto {
            x: c.marco.derecha() - 1,
            y: c.marco.abajo(),
        };
        assert_eq!(c.destino(tambien_fuera), DestinoClic::Lienzo);
    }

    #[test]
    fn la_columna_del_anotador_cabe_entera_en_el_area_de_trabajo() {
        // `colocar` promete en su documentacion que la caja siempre queda
        // entera en el area de trabajo. Se mide con `BOTONES`, que es la
        // lista que de verdad se pinta en columna: el editor —el unico que
        // usa `BOTONES_EDITOR`— la pone en `barra_superior`, y sus treinta
        // botones puestos uno encima de otro miden 1270 px, mas alto que un
        // monitor de 1080. Probar la columna con una lista que nadie pone en
        // columna seria probar un caso que no existe.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES);
        assert!(
            c.marco.arriba() >= area().arriba() && c.marco.abajo() <= area().abajo(),
            "se sale por arriba o por abajo: {c:?}"
        );
        assert!(
            c.marco.izquierda() >= area().izquierda() && c.marco.derecha() <= area().derecha(),
            "se sale de lado: {c:?}"
        );
        for i in 0..BOTONES.len() {
            let r = c.rect_de(i);
            assert!(
                r.arriba() >= c.marco.arriba() && r.abajo() <= c.marco.abajo(),
                "el boton {i} se sale del marco"
            );
        }
    }

    fn centro(r: Rect) -> Punto {
        Punto {
            x: r.x + r.ancho as i32 / 2,
            y: r.y + r.alto as i32 / 2,
        }
    }

    /// **Cada boton de la barra de siempre tiene sitio en la agrupada: o va
    /// suelto, o en exactamente un grupo.** Es lo que se escapa al anadir una
    /// herramienta: que quede fuera de todo y nadie llegue a ella.
    #[test]
    fn cada_boton_de_la_barra_de_siempre_va_suelto_o_en_un_solo_grupo() {
        for b in BOTONES_EDITOR {
            let sueltos = BARRA_AGRUPADA.iter().filter(|x| **x == b).count();
            let grupos = GrupoBarra::TODOS
                .iter()
                .filter(|g| g.miembros().contains(&b))
                .count();
            assert_eq!(
                sueltos + grupos,
                1,
                "{b:?}: {sueltos} suelto y {grupos} grupos"
            );
        }
        // Y al reves: nada en un grupo que no sea de la barra de siempre.
        for g in GrupoBarra::TODOS {
            assert!(g.miembros().len() >= 2, "{g:?} es un grupo de uno");
            for m in g.miembros() {
                assert!(BOTONES_EDITOR.contains(m), "{m:?} de {g:?}");
            }
            assert_eq!(GrupoBarra::TODOS[g.indice()], g);
        }
    }

    #[test]
    fn lo_de_uso_constante_va_suelto_y_no_dentro_de_un_grupo() {
        for b in [
            BotonCaja::Elegir(Herramienta::Mano),
            BotonCaja::Elegir(Herramienta::Lapiz),
            BotonCaja::Elegir(Herramienta::Borrador),
            BotonCaja::Deshacer,
            BotonCaja::Rehacer,
            BotonCaja::Salir,
        ] {
            assert!(BARRA_AGRUPADA.contains(&b), "{b:?}");
            assert_eq!(grupo_de_boton(b), None, "{b:?}");
        }
        // Caso negativo: el lazo no va suelto, va con las de elegir.
        assert_eq!(
            grupo_de_boton(BotonCaja::Elegir(Herramienta::Lazo)),
            Some(GrupoBarra::Elegir)
        );
    }

    /// **La barra agrupada cabe en una fila** en un portatil de 1366 px al
    /// 125 % y en 1080 p al 150 %, y Salir queda dentro. Con los cuarenta
    /// botones sueltos no cabia ni a 120 % en 1080 p.
    #[test]
    fn la_barra_agrupada_cabe_en_una_fila_en_un_portatil_y_a_150_en_1080() {
        for (ancho, escala) in [(1366u32, 125u32), (1920, 150), (1280, 100), (1366, 150)] {
            let zona = Rect {
                x: 0,
                y: 0,
                ancho,
                alto: 700,
            };
            let b = CajaHerramientas::barra_superior(zona, escala, &BOTONES_EDITOR);
            assert_eq!(b.filas(), 1, "{ancho} a {escala}: {b:?}");
            assert_eq!(b.botones().len(), 17);
            let salir = b.rect_de(b.botones().len() - 1);
            assert_eq!(b.botones()[b.botones().len() - 1], BotonCaja::Salir);
            assert!(salir.derecha() <= zona.derecha(), "{salir:?}");
        }
        let b = CajaHerramientas::barra_superior(area(), 100, &BOTONES_EDITOR);
        assert_eq!(b.marco.ancho, 711, "la barra mide otra cosa: {b:?}");
    }

    /// **En una ventana muy estrecha se parte en dos filas por un separador**
    /// y nada se pisa ni se sale.
    #[test]
    fn en_una_ventana_estrecha_la_barra_va_en_dos_filas_por_un_separador() {
        let estrecha = Rect {
            x: 0,
            y: 0,
            ancho: 560,
            alto: 800,
        };
        let b = CajaHerramientas::barra_superior(estrecha, 100, &BOTONES_EDITOR);
        assert_eq!(b.filas(), 2);
        assert!(b.marco.derecha() <= estrecha.derecha(), "{b:?}");
        let botones = b.botones().to_vec();
        for (i, &boton) in botones.iter().enumerate() {
            let r = b.rect_de(i);
            assert!(b.marco.contiene(Punto { x: r.x, y: r.y }), "{i}");
            assert_eq!(
                b.destino(Punto {
                    x: r.x + 2,
                    y: r.y + 2
                }),
                DestinoClic::Boton(boton)
            );
            for j in 0..i {
                assert!(r.interseccion(b.rect_de(j)).is_none(), "{i} pisa {j}");
            }
        }
        assert!(grupo(botones[b.corte - 1]) != grupo(botones[b.corte]));
        // Caso negativo: donde cabe, una fila y sus tres separadores.
        let una = CajaHerramientas::barra_superior(area(), 100, &BOTONES_EDITOR);
        assert_eq!(una.filas(), 1);
        assert_eq!(una.separadores().len(), 3);
    }

    #[test]
    fn un_grupo_con_todo_apagado_desaparece_de_la_barra_y_con_una_sale() {
        let sin_medir: Vec<BotonCaja> = BOTONES_EDITOR
            .iter()
            .copied()
            .filter(|b| grupo_de_boton(*b) != Some(GrupoBarra::Medir))
            .collect();
        let v = agrupar(&sin_medir);
        assert!(!v.contains(&BotonCaja::Grupo(GrupoBarra::Medir)));
        assert!(v.contains(&BotonCaja::Grupo(GrupoBarra::Formas)));
        // Caso negativo: con solo la cota encendida, el grupo sigue.
        let mut con_cota = sin_medir.clone();
        con_cota.push(BotonCaja::Elegir(Herramienta::Cota));
        assert!(agrupar(&con_cota).contains(&BotonCaja::Grupo(GrupoBarra::Medir)));
    }

    #[test]
    fn lo_que_no_tiene_sitio_en_ningun_grupo_sale_al_final_y_no_se_pierde() {
        // El color no tiene grupo ni sitio suelto en la barra agrupada.
        let con_color = [BotonCaja::Elegir(Herramienta::Mano), BotonCaja::Color];
        assert_eq!(
            agrupar(&con_color),
            vec![BotonCaja::Elegir(Herramienta::Mano), BotonCaja::Color]
        );
        // Caso negativo: nada se repite aunque venga dos veces.
        let doble = [BotonCaja::Salir, BotonCaja::Salir];
        assert_eq!(agrupar(&doble), vec![BotonCaja::Salir]);
    }

    /// `caraDelGrupo` del movil: la puesta, si no la recordada, si no la
    /// primera; y una recordada que aqui no sale no vale.
    #[test]
    fn la_cara_de_un_grupo_es_la_puesta_o_la_ultima_usada_o_la_primera() {
        let formas = GrupoBarra::Formas.miembros();
        let rombo = BotonCaja::Elegir(Herramienta::Rombo);
        let elipse = BotonCaja::Elegir(Herramienta::Elipse);
        assert_eq!(
            cara_del_grupo(formas, Herramienta::Rombo, Some(elipse)),
            Some(rombo)
        );
        assert_eq!(
            cara_del_grupo(formas, Herramienta::Lapiz, Some(elipse)),
            Some(elipse)
        );
        assert_eq!(
            cara_del_grupo(formas, Herramienta::Lapiz, None),
            Some(BotonCaja::Elegir(Herramienta::Rectangulo))
        );
        // Casos negativos: recordada de otro grupo, y grupo vacio.
        let lazo = BotonCaja::Elegir(Herramienta::Lazo);
        assert_eq!(
            cara_del_grupo(formas, Herramienta::Lapiz, Some(lazo)),
            Some(BotonCaja::Elegir(Herramienta::Rectangulo))
        );
        assert_eq!(cara_del_grupo(&[], Herramienta::Lapiz, None), None);
    }

    /// **Al desplegar un grupo, sus hermanas salen en una isla bajo su
    /// boton**, una fila cada una, y un clic en una fila es esa herramienta.
    #[test]
    fn un_grupo_desplegado_ensena_sus_hermanas_bajo_su_boton_y_se_pueden_pulsar() {
        let b = CajaHerramientas::barra_superior(area(), 125, &BOTONES_EDITOR)
            .con_desplegado(Some(GrupoBarra::Formas));
        assert_eq!(b.desplegado(), Some(GrupoBarra::Formas));
        let m = b.menu().expect("abierto");
        let boton = b
            .rect_de_boton(BotonCaja::Grupo(GrupoBarra::Formas))
            .unwrap();
        assert!(m.marco.y >= b.marco.abajo(), "debajo de la barra");
        assert!(
            (m.marco.x - boton.x).abs() <= 10,
            "bajo su boton: {m:?} {boton:?}"
        );
        // Rectangulo, ovalo, rombo y arco (la marca de angulo esta con el
        // punto, en «Nombrar», desde el 17-oct).
        assert_eq!(m.filas.len(), 4);
        for (h, r) in &m.filas {
            assert!(m.marco.contiene(Punto { x: r.x, y: r.y }));
            assert_eq!(b.destino(centro(*r)), DestinoClic::Boton(*h));
            assert_eq!(b.rect_de_boton(*h), Some(*r));
        }
        // El relleno de la isla es de la caja: no pinta detras.
        let borde = Punto {
            x: m.marco.x + 1,
            y: m.marco.y + 1,
        };
        assert_eq!(b.destino(borde), DestinoClic::Caja);
        // Caso negativo: cerrado, el mismo sitio es lienzo.
        let cerrada = b.con_desplegado(None);
        assert!(cerrada.menu().is_none());
        assert_eq!(cerrada.destino(centro(m.filas[0].1)), DestinoClic::Lienzo);
    }

    #[test]
    fn no_se_despliega_un_grupo_que_no_esta_o_que_tiene_una_sola_herramienta() {
        let solo_cota: &'static [BotonCaja] = &[
            BotonCaja::Elegir(Herramienta::Mano),
            BotonCaja::Elegir(Herramienta::Cota),
            BotonCaja::Salir,
        ];
        let b = CajaHerramientas::barra_superior(area(), 100, solo_cota);
        assert_eq!(b.con_desplegado(Some(GrupoBarra::Medir)).desplegado(), None);
        assert_eq!(
            b.con_desplegado(Some(GrupoBarra::Formas)).desplegado(),
            None
        );
        // La columna tampoco despliega.
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES_EDITOR);
        assert_eq!(
            c.con_desplegado(Some(GrupoBarra::Formas)).desplegado(),
            None
        );
    }

    #[test]
    fn el_desplegable_del_ultimo_grupo_no_se_sale_por_la_derecha() {
        let zona = Rect {
            x: 0,
            y: 0,
            ancho: 760,
            alto: 700,
        };
        let b = CajaHerramientas::barra_superior(zona, 100, &BOTONES_EDITOR)
            .con_desplegado(Some(GrupoBarra::Sacar));
        let m = b.menu().expect("abierto");
        assert!(m.marco.derecha() <= zona.derecha(), "{m:?}");
        assert_eq!(
            m.filas.iter().map(|(b, _)| *b).collect::<Vec<_>>(),
            vec![BotonCaja::Compartir, BotonCaja::Imprimir]
        );
    }

    #[test]
    fn la_barra_de_excalidraw_va_centrada_arriba_a_16_px_y_mide_44_de_alto() {
        let b = CajaHerramientas::barra_superior(area(), 100, &BOTONES_EDITOR);
        assert!(b.es_horizontal());
        assert_eq!(b.marco.y, 16);
        assert_eq!(
            b.marco.alto, 44,
            "36 de boton mas 4 de relleno arriba y abajo"
        );
        let izq = b.marco.x - area().x;
        let der = area().derecha() - b.marco.derecha();
        assert!((izq - der).abs() <= 1, "centrada: {izq} y {der}");
    }

    #[test]
    fn los_botones_de_la_barra_son_de_36_y_no_se_pisan_con_los_separadores() {
        let b = CajaHerramientas::barra_superior(area(), 100, &BOTONES_EDITOR);
        let seps = b.separadores();
        assert_eq!(
            seps.len(),
            3,
            "elegir | dibujar | tapar, medir y sacar un trozo | acciones"
        );
        for i in 0..b.botones().len() {
            let r = b.rect_de(i);
            assert_eq!((r.ancho, r.alto), (36, 36));
            assert!(
                r.derecha() <= b.marco.derecha() - 4,
                "boton {i} fuera: {r:?}"
            );
            if i > 0 {
                assert!(
                    r.x >= b.rect_de(i - 1).derecha() + 4,
                    "boton {i} pisa al anterior"
                );
            }
            for s in &seps {
                assert!(r.interseccion(*s).is_none(), "boton {i} pisa un separador");
            }
        }
    }

    #[test]
    fn un_clic_en_la_barra_elige_su_boton_y_debajo_de_ella_es_lienzo() {
        let b = CajaHerramientas::barra_superior(area(), 150, &BOTONES_EDITOR);
        let r = b.rect_de(2);
        assert_eq!(b.destino(centro(r)), DestinoClic::Boton(b.botones()[2]));
        // Caso negativo: el separador es de la barra, pero no es un boton.
        let s = b.separadores()[0];
        assert_eq!(b.destino(Punto { x: s.x, y: s.y }), DestinoClic::Caja);
        assert_eq!(
            b.destino(Punto {
                x: centro(r).x,
                y: b.marco.abajo() + 1
            }),
            DestinoClic::Lienzo
        );
    }

    #[test]
    fn en_una_pantalla_estrecha_la_barra_se_pega_a_la_izquierda_sin_salirse() {
        let estrecha = Rect {
            x: 0,
            y: 0,
            ancho: 300,
            alto: 800,
        };
        let b = CajaHerramientas::barra_superior(estrecha, 100, &BOTONES_EDITOR);
        assert_eq!(b.marco.x, 0);
    }

    #[test]
    fn la_caja_vertical_no_tiene_separadores() {
        let c = CajaHerramientas::colocar(contenido(), area(), 100, &BOTONES_EDITOR);
        assert!(!c.es_horizontal());
        assert!(c.separadores().is_empty());
    }
}
