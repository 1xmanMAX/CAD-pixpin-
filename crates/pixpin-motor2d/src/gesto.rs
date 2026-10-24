//! Todo lo que pasa entre pulsar y soltar.
//!
//! En el Android son 3.482 lineas (`DrawController.kt`) porque atiende a
//! dos, tres y cuatro dedos, al lapiz y al canto de la mano. Aqui hay un
//! raton y un teclado, asi que donde el Android pregunta «hay segundo
//! dedo?», esto pregunta «esta Shift?».
//!
//! # Que toca y que devuelve (D23)
//!
//! **La escena si la toca.** Es un `Vec` de datos, se prueba sin pantalla, y
//! mantener aparte un registro de ordenes pendientes seria una segunda
//! verdad sobre el mismo dibujo.
//!
//! **Lo que devuelve es lo que la ventana tiene que hacer**: que region
//! redibujar y que cursor poner. De Direct2D no sabe nada. Por eso esto vive
//! en el motor y no en la interfaz: una prueba puede decir «pulsar aqui,
//! mover cuarenta pixeles con Shift, soltar» y comprobar el resultado exacto
//! sin abrir una ventana.
//!
//! # El orden de decision al pulsar
//!
//! Es la parte que hay que leer despacio:
//!
//! 1. **Un tirador manda sobre lo que haya debajo.** Sin esto, un tirador
//!    encima de un trazo es inalcanzable.
//! 2. **Lo ya seleccionado manda sobre lo de encima.** Sin esto, mover un
//!    grupo se convierte en seleccionar por accidente lo que estaba encima.
//! 3. Lo que haya bajo el cursor.
//! 4. Vacio: marquesina si la herramienta es la mano, dibujar si no.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::escena::Escena;
use crate::impacto::{dentro_de, elemento_en};
use crate::medida::Escala;
use crate::seleccion::Seleccion;
use crate::tiradores::{Agarre, Tiradores};
use crate::transformar::{self, a_saltos, angulo_hacia};
use crate::vector::Punto2;

/// Puntos que se reservan de una vez para el trazo en curso.
///
/// 512 porque el trazo mas largo del fichero del movil tiene 492. Reservar
/// de una vez es lo que permite que mover el raton dibujando no asigne
/// memoria — la regla del camino caliente.
pub const PUNTOS_RESERVADOS: usize = 512;

/// El punto final restringido a un angulo redondo, para Shift-arrastrar.
///
/// El giro medio -de 0 a 180 grados- se reparte en `pasos` tramos iguales:
/// con 12 salen horizontal, vertical y las dos diagonales cada 15 grados
/// (Linea y Flecha, en `pixpin-ui/anotador.rs`); con 2 solo salen horizontal
/// y vertical (`Calibrando` aqui abajo: calibrar sobre el borde de una
/// pared es el caso normal, y a pulso sale torcida -un grado de mas son dos
/// centimetros de error por metro-).
///
/// Publica y aqui, en el motor, y no repetida en `pixpin-ui`: es logica
/// pura sin nada de interfaz, y `pixpin-ui` ya depende de este crate.
pub fn restringir_angulo(inicio: Punto2, fin: Punto2, pasos: f32) -> Punto2 {
    let (dx, dy) = (fin.x - inicio.x, fin.y - inicio.y);
    let radio = (dx * dx + dy * dy).sqrt();
    if radio == 0.0 {
        return fin;
    }
    let paso = std::f32::consts::PI / pasos;
    let angulo = (dy.atan2(dx) / paso).round() * paso;
    Punto2 {
        x: inicio.x + radio * angulo.cos(),
        y: inicio.y + radio * angulo.sin(),
    }
}

/// Con que se dibuja. Vino de `pixpin-ui/anotador.rs` en el paso 1: quien
/// decide que hace un clic tiene que saber que herramienta hay puesta, y esa
/// decision es logica pura, no interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herramienta {
    /// Seleccionar y mover lo ya dibujado.
    Mano,
    Lapiz,
    /// **Grafito** (v0.75 del movil): el lapiz de verdad, hecho de sellos
    /// sobre la rejilla fija de cuadritos (`tinta::grafito`). Por dentro es
    /// el mismo trazo que el lapiz, con `material:"cuadritos"`: un lapiz
    /// mas, hecho de otra cosa, y por eso comparte con el todo lo de trazar a
    /// mano (el gesto de pararse, el iman, la forma rapida).
    Grafito,
    Resaltador,
    Linea,
    Flecha,
    Rectangulo,
    Elipse,
    Texto,
    /// Oscurece todo menos una zona (D51).
    Foco,
    /// Amplia alrededor del cursor. No deja rastro: es una vista (D52).
    Lupa,
    Borrador,
    /// Acota: deja una raya que dice cuanto mide.
    Cota,
    /// Calibra: se arrastra sobre algo de medida conocida y al soltar
    /// pregunta cuanto mide de verdad. La raya no se guarda.
    Escalar,
    /// La reglita a cuadros que sobrevive a la fotocopia.
    EscalaGrafica,
    /// Un marco: recuadro con nombre que se lleva consigo lo que encierra.
    Marco,
    /// Un emoji suelto (universo). El gesto no crea nada con ella: quien
    /// sabe que emoji se eligio es la sesion del universo, y es ella la que
    /// lo coloca en la escena.
    Emoji,

    // --- Las que abre la tanda cero ---
    //
    // Estan aqui, en un solo sitio y de una vez, para que los cuatro grupos
    // que vienen a portar herramientas no tengan que volver a tocar este
    // enumerado ni el `match` central: cada uno rellena su modulo y ya.
    // Todas producen ALGO honesto desde el primer dia —una figura que se ve
    // y que viaja al movil— aunque el detalle fino sea de su grupo.
    /// El rombo de Excalidraw. Una de sus diez figuras principales.
    Rombo,
    /// Un trozo de ovalo: se pone la guia y se repasa (grupo A).
    Arco,
    /// La flecha de codos, el conector de organigrama. `Elbow.kt` ya esta
    /// portado en `codo.rs` y esta nace con `codos` puesto: dobla en angulo
    /// recto desde el primer arrastre, y `elbowed` viaja con ella.
    FlechaCodos,
    /// La flecha a pulso: por dentro ES una flecha, con todos los puntos
    /// del trazo en vez de dos (`Scene.kt:57-64`).
    FlechaLibre,
    /// Selecciona con un contorno a mano (grupo B).
    Lazo,
    /// Tapa lo que hay debajo (grupo D).
    Mosaico,
    /// El circulito numerado de anotar una captura paso a paso (grupo D).
    Serie,
    /// El bote: rellena el hueco entre varias figuras (grupo C).
    Relleno,
    /// Quita el trozo de raya que sobra hasta donde la cruzan las demas
    /// (grupo C).
    Recortar,
    /// Alarga la punta que se queda corta hasta lo primero que topa
    /// (grupo C).
    Extender,
    /// Un punto con su letra, para un croquis de geometria (grupo C).
    Punto,
    /// **La marca de un angulo** (laminas de geometria): un clic dentro del
    /// angulo, cerca de donde se juntan dos rayas. Ver `gesto/geometria.rs`.
    Angulo,
    /// Copia el estilo de una figura y lo pega en otra (grupo B). Es
    /// invencion de escritorio: en el movil no existe.
    CopiarEstilo,
    /// **La zona** (F8, `Tool.ZONA` del movil): se arrastra un rectangulo y
    /// sale una foto de lo que hay dentro, recortada en redondo, para
    /// llevarla a otro sitio. La foto la saca quien pinta; el gesto no crea
    /// nada (ver `zona.rs`).
    Zona,
    /// **El puntero laser** (F14): una estela que se apaga sola. No deja
    /// nada en el dibujo (ver `puntero_laser.rs`).
    Laser,
    /// **Soldar vertices** (`Tool.NUDO` del movil): un clic en el cruce o la
    /// junta de dos figuras las clava por ahi y desde entonces no se separan
    /// (ver `nudos.rs`). Otro clic en el clavo lo quita. No crea nada: los
    /// clavos son de la escena.
    Nudo,
    /// **La bolita** (`Tool.BOLITA` del movil): se pasa por encima de lo que
    /// se quiera y va entrando en la seleccion; volver a pasar lo saca. La
    /// lleva quien tiene la escena, como el lazo (ver `bolita.rs`).
    Bolita,
}

impl Herramienta {
    /// Si necesita un arrastre de verdad para producir algo. El lapiz no:
    /// un clic deja un punto de tinta, que es lo que espera cualquiera que
    /// haya usado un rotulador.
    pub fn necesita_arrastre(self) -> bool {
        !matches!(
            self,
            Herramienta::Lapiz
                | Herramienta::Grafito
                | Herramienta::Texto
                // La flecha a pulso se traza como el lapiz, punto a punto.
                | Herramienta::FlechaLibre
                // Estas tres se plantan de un toque: el punto no tiene caja
                // que arrastrar y el numero de serie tampoco.
                | Herramienta::Serie
                | Herramienta::Punto
                // La marca de angulo se pone de un clic.
                | Herramienta::Angulo
        )
    }

    /// Si lo que dibuja se guarda en el documento.
    ///
    /// Las que dicen que no son de dos clases: las que solo MIRAN (la mano,
    /// la lupa-vista) y **las que trabajan sobre lo que ya hay** —el bote,
    /// recortar, extender, copiar estilo, el lazo—. Estas ultimas si tocan
    /// el documento, pero no naciendo un elemento bajo el arrastre, que es
    /// lo unico que esta bandera decide.
    pub fn deja_rastro(self) -> bool {
        !matches!(
            self,
            Herramienta::Mano
                | Herramienta::Lupa
                // El foco es una varita, como la lupa del lienzo: convierte la
                // figura que toca (`dibujo::lupa::convertir_en_foco`).
                | Herramienta::Foco
                | Herramienta::Borrador
                | Herramienta::Escalar
                | Herramienta::Lazo
                | Herramienta::Relleno
                | Herramienta::Recortar
                | Herramienta::Extender
                | Herramienta::CopiarEstilo
                // Las dos las lleva quien tiene la escena, como el lazo: la
                // zona saca una foto al soltar y el laser solo se pinta.
                | Herramienta::Zona
                | Herramienta::Laser
                // Clava lo que ya hay; el clavo va a la escena, no nace un
                // elemento (`dibujo::construir`).
                | Herramienta::Nudo
                // Selecciona, como el lazo.
                | Herramienta::Bolita
                // La marca la pone el gesto entera al pulsar
                // (`gesto/geometria.rs`): no nace arrastrando.
                | Herramienta::Angulo
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventoGesto {
    /// `presion` es la del lapiz (0-1) o `None` con raton.
    Pulsar {
        p: Punto2,
        shift: bool,
        alt: bool,
        presion: Option<f32>,
    },
    Mover {
        p: Punto2,
        shift: bool,
        alt: bool,
        presion: Option<f32>,
    },
    Soltar {
        p: Punto2,
    },
    Escape,
    Suprimir,
    Deshacer,
    Rehacer,
    SeleccionarTodo,
}

/// Que hay que volver a pintar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Region {
    Nada,
    /// Solo esta caja del mundo. Es lo que se usa en el camino caliente.
    Caja(f32, f32, f32, f32),
    Todo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormaCursor {
    Flecha,
    Cruz,
    Mover,
    Texto,
    Giro,
    /// Escalar, con el tirador (para saber a que lado apunta) y el angulo
    /// del elemento (para girar esa direccion con el).
    Escalar {
        tirador: Tirador,
        angulo: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Respuesta {
    pub region: Region,
    pub cursor: FormaCursor,
    pub pide: Option<Peticion>,
}

/// Algo que la maquina necesita y solo la ventana puede conseguir.
///
/// La maquina dice «hay que preguntar esto»; quien pregunta y como es asunto
/// del que pinta (D40). Mismo corte que `Region` y `FormaCursor`: aqui no se
/// sabe lo que es una ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Peticion {
    /// Cuanto mide de verdad el trazo que se acaba de arrastrar.
    Calibrar { largo_px: f32 },
    /// **Cuanto mide y hacia donde va la cota recien trazada** (el
    /// `DialogoDeCota` del movil). Quien lleva la ventana abre el cajetin y
    /// aplica lo tecleado con [`Gesto::dictar_cota`]; si se cancela, la cota
    /// se queda como se trazo, que ya dice lo que mide.
    DictarCota { id: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Estado {
    Reposo,
    Dibujando {
        id: u64,
    },
    Moviendo {
        anterior: Punto2,
    },
    Escalando {
        tirador: Tirador,
    },
    Girando {
        anterior: f32,
    },
    Marquesina {
        origen: Punto2,
        hasta: Punto2,
    },
    /// Arrastrando la raya de `Escalar`. No hay id: no deja rastro, y sus
    /// puntos van en `trazo` en vez de en un elemento de la escena.
    Calibrando,
    /// Arrastrando un punto de una flecha ya dibujada (el editor de puntos
    /// de Excalidraw). Si es un extremo, al soltar se ata a lo que tenga
    /// debajo o se suelta.
    ArrastrandoPunta {
        id: u64,
        agarre: crate::tiradores::AgarrePunta,
    },
}

pub struct Gesto {
    estado: Estado,
    pub seleccion: Seleccion,
    pub herramienta: Herramienta,
    /// El trazo en curso. Se reserva una vez y se reutiliza con `clear()`.
    trazo: Vec<Punto2>,
    /// A que se pega el cursor. Lo escribe la ventana desde los ajustes
    /// guardados; por defecto, encendido con todas las clases de punto.
    pub enganche: crate::enganche::Ajustes,
    /// El ancla a la que se ha pegado el ultimo punto, para que la ventana
    /// pinte la pista. Se apaga al soltar y al cancelar: una marca que
    /// sobrevive al gesto es una marca mintiendo.
    pub anclaje_activo: Option<crate::enganche::Anclaje>,
    /// **Los anclajes finos del iman** (cruces, cantos, tangentes). De
    /// fabrica apagados: cuestan reservas de memoria en cada movimiento del
    /// raton y quien los quiere —la ventana del editor— los enciende a
    /// sabiendas (ver `enganche::sitio`). El `desde` lo pone el gesto.
    pub finos: crate::enganche::AjustesFinos,
    /// El `strokeWidth` de Excalidraw para el lapiz: 0,5 / 1 / 2.
    pub grosor_tinta: f32,
    /// Pluma variable o constante para los trazos nuevos.
    pub variabilidad: crate::tinta::Variabilidad,
    /// Una elipse de forma rapida que se esta ajustando: centro, distancia
    /// del cursor al convertirla y radios de entonces. Escala desde su
    /// centro; crecer desde una esquina la haria saltar al convertirse.
    forma_elipse: Option<(Punto2, f32, f32, f32)>,
    /// Si lo que se esta dibujando salio del gesto de pararse (compas, «L»,
    /// forma rapida). Desde ese instante ya no es un trazo a mano sino una
    /// figura: el iman la trata como tal (`faena`), y al soltar un compas que
    /// no llego a abrirse se descarta, como cualquier figura de tamano cero
    /// del movil (`finishCreating`).
    forma_por_gesto: bool,
    /// El «actual» del panel lateral: con esto nacen las figuras nuevas.
    /// El grosor del lapiz sigue en `grosor_tinta` (las teclas 1/2/3).
    pub estilo: crate::estilo::EstiloDibujo,
    /// El texto que se esta escribiendo: que elemento es y como va su
    /// cursor. Mientras esto tiene algo, las teclas son del texto y no
    /// atajos de herramienta.
    pub escribiendo: Option<(u64, crate::texto::EdicionTexto)>,
    /// Como estaba el texto antes de abrirlo para escribir, si ya existia:
    /// al cerrarlo, si cambio, queda UN paso de deshacer con todo lo escrito
    /// (antes escribir en un texto viejo no se podia deshacer con Ctrl+Z).
    antes_de_escribir: Option<Elemento>,
    /// El lazo que se esta trazando ahora, si la herramienta es el lazo.
    ///
    /// Vive aqui y no en la ventana porque **quien lo dibuja y quien lo mueve
    /// tienen que ver lo mismo**: el bucle de eventos le anade puntos y el
    /// pintado le pide su `orden()`, y con dos copias el rastro que se ve y
    /// lo que acaba seleccionandose se separan en cuanto uno se olvide de
    /// actualizar al otro.
    ///
    /// El lazo no `deja_rastro()`, asi que la maquina de estados no crea
    /// ningun elemento por el: lo lleva entero quien tiene la escena.
    pub lazo: Option<crate::lazo::Lazo>,
    /// El rectangulo de la Zona que se esta arrastrando (esquina de salida y
    /// cursor). Aqui por lo mismo que el lazo: lo pinta quien pinta lo de
    /// encima, en cualquier anfitrion, y lo mueve quien tiene la escena.
    pub zona: Option<(Punto2, Punto2)>,
    /// La estela del puntero laser (ver `puntero_laser.rs`).
    pub laser: crate::puntero_laser::PunteroLaser,
    /// El estilo que el cuentagotas se llevo, a la espera de pegarlo.
    ///
    /// Tambien aqui por lo mismo: la caja de herramientas pinta el
    /// cuentagotas «cargado» o «vacio» segun esto, y el clic siguiente hace
    /// una cosa u otra segun esto.
    pub estilo_tomado: Option<crate::estilo::EstiloCopiado>,
    /// **Con el grafito en la mano** (`conElGrafitoEnLaMano` del movil):
    /// se enciende al coger el grafito y se apaga al coger el lapiz o el
    /// resaltador. Mientras dura, las figuras nuevas (rectangulo, rombo,
    /// ovalo, linea, flecha) nacen de grafito, como en el movil desde la
    /// v0.80.2. Ver [`Gesto::tomar_herramienta`].
    pub grafito_en_la_mano: bool,
    /// Las flechas atadas a lo que se esta moviendo, apuntadas al pulsar
    /// para re-trazarlas en cada aviso sin buscarlas (ver
    /// `enlace::Seguidoras`).
    seguidoras: crate::enlace::Seguidoras,
    /// Lo que encerraban los marcos elegidos al cogerlos para mover,
    /// ordenado y sin lo elegido (`preparar_seguidoras`).
    del_marco: Vec<u64>,
    /// Lo elegido tal como estaba al pulsar un tirador, si son varios: el
    /// bloque se estira siempre desde aqui (`estirar_bloque`).
    originales_del_bloque: Vec<Elemento>,
    /// La figura a la que se ataria cada extremo —inicio y fin— de la
    /// flecha que se dibuja o cuya punta se arrastra. La ventana la resalta
    /// (`resaltado_de_union`) para que atar no sea una sorpresa al soltar.
    pub candidatas: [Option<u64>; 2],
    /// Si el gesto en curso llego a moverse. Un clic sobre una flecha
    /// elegida sin arrastrar no puede re-atar nada: dejaria un paso de
    /// deshacer sin nada que se viera.
    arrastrado: bool,
    /// Alt pulsado: al pulsar (`[0]`) y en el ultimo aviso del raton (`[1]`).
    /// Con Alt, la punta de flecha que se suelta encima de una figura se ata
    /// **dentro** (`bindMode: "inside"` de Excalidraw). Se apunta porque el
    /// soltar no trae modificadores: lo que cuenta es lo que habia pulsado
    /// justo antes. Dos, porque la punta de salida se deja al pulsar y la de
    /// llegada al soltar.
    alt: [bool; 2],
    /// **Los clavos de soldar vertices** en este gesto (`nudos`): si la
    /// escena tenia alguno al pulsar, y el clavo cogido si se esta llevando
    /// uno. Con clavos, arrastrar lo elegido puede girarlo en vez de
    /// trasladarlo, y entonces no vale moverlo como una capa quieta.
    con_clavos: bool,
    clavo_en_mano: Option<usize>,
    /// **De que serie salen las letras de los proximos puntos**: A, a o 1
    /// (`seriePuntos` del movil). Es del gesto y no del estilo porque es una
    /// decision del dibujo entero, no de cada punto: quien elige una serie va
    /// a poner diez puntos seguidos de esa serie.
    pub serie_de_punto: crate::puntos_etiquetados::SerieDePunto,
    /// **Si al trazar una cota se pide su medida** (`pedirLaMedida` del
    /// movil, encendido de fabrica). Son dos formas de acotar: dictandola, la
    /// raya acaba midiendo lo que uno dice (levantar un plano); sin dictar,
    /// mide lo que hay (medir sobre una foto). Es un interruptor del panel.
    pub pedir_la_medida: bool,
    /// El barrido de la bolita en curso, para pasarla y pintarla.
    pub bolita: crate::bolita::Bolita,
    /// **El modo guia** (`modoReferencia` del movil): lo que se traza ahora
    /// es **linea de referencia**, no dibujo, con cualquier herramienta. Es
    /// un interruptor y no una lista de herramientas paralela: las de guia
    /// tienen que ser exactamente las mismas que las normales.
    ///
    /// Y decide en que mundo se trabaja (`impacto::Mundo`): dentro solo se
    /// eligen y se borran guias; fuera, solo dibujo. Se cambia con
    /// [`Gesto::poner_modo_guia`], que suelta lo elegido.
    modo_guia: bool,
    /// Los clics que lleva la herramienta Arco en sus modos de clics.
    clics_arco: geometria::ClicsDeArco,
    /// Repasando una guia: el angulo del cursor la ultima vez.
    arqueando: Option<f32>,
    /// **La ultima marca de angulo que puso la herramienta Angulo**: la rueda
    /// y las flechas le cambian la letra griega sin tener que elegirla
    /// (`Gesto::cambiar_letra_de_angulo`).
    pub ultima_marca: Option<u64>,
}

mod geometria;
pub use geometria::ModoArco;

impl Default for Gesto {
    fn default() -> Self {
        Self {
            estado: Estado::Reposo,
            seleccion: Seleccion::nueva(),
            herramienta: Herramienta::Lapiz,
            clics_arco: Default::default(),
            arqueando: None,
            ultima_marca: None,
            trazo: Vec::with_capacity(PUNTOS_RESERVADOS),
            enganche: crate::enganche::Ajustes::default(),
            anclaje_activo: None,
            finos: crate::enganche::AjustesFinos::NINGUNO,
            grosor_tinta: crate::tinta::GROSOR_MEDIO,
            variabilidad: crate::tinta::Variabilidad::Variable,
            estilo: crate::estilo::EstiloDibujo::default(),
            escribiendo: None,
            antes_de_escribir: None,
            forma_elipse: None,
            forma_por_gesto: false,
            lazo: None,
            zona: None,
            laser: crate::puntero_laser::PunteroLaser::default(),
            estilo_tomado: None,
            grafito_en_la_mano: false,
            seguidoras: crate::enlace::Seguidoras::default(),
            del_marco: Vec::new(),
            originales_del_bloque: Vec::new(),
            candidatas: [None; 2],
            arrastrado: false,
            alt: [false; 2],
            con_clavos: false,
            clavo_en_mano: None,
            serie_de_punto: crate::puntos_etiquetados::SerieDePunto::Mayusculas,
            pedir_la_medida: true,
            bolita: crate::bolita::Bolita::default(),
            modo_guia: false,
        }
    }
}

/// Las figuras que nacen de grafito con el en la mano: las de
/// `FIGURAS_DE_GRAFITO` del movil (`DrawController.kt`).
pub const FIGURAS_DE_GRAFITO: [Herramienta; 6] = [
    Herramienta::Rectangulo,
    Herramienta::Rombo,
    Herramienta::Elipse,
    Herramienta::Linea,
    Herramienta::Flecha,
    Herramienta::FlechaCodos,
];

impl Gesto {
    /// **Coge una herramienta**, y con ella lo que se tiene en la mano.
    ///
    /// El grafito es una herramienta y no un material del panel, pero deja
    /// huella en lo que viene detras: tras el grafito, el rectangulo o la
    /// flecha salen de grafito (`conElGrafitoEnLaMano` del movil). Se deja al
    /// coger el lapiz o el resaltador; las demas no lo tocan, para que ir a
    /// la mano a mover algo y volver no lo apague.
    pub fn tomar_herramienta(&mut self, h: Herramienta) {
        match h {
            Herramienta::Grafito => self.grafito_en_la_mano = true,
            Herramienta::Lapiz | Herramienta::Resaltador => self.grafito_en_la_mano = false,
            _ => {}
        }
        // Una secuencia de clics del arco no sobrevive a cambiar de mano.
        if h != self.herramienta {
            self.clics_arco = Default::default();
        }
        self.herramienta = h;
    }

    /// El material con el que nace lo que se va a dibujar: el grafito si es
    /// el grafito o si se tiene en la mano y es una de sus figuras; si no, el
    /// «actual» del panel, como siempre.
    fn material_de_lo_nuevo(&self) -> crate::tinta::MaterialTinta {
        let de_grafito = self.herramienta == Herramienta::Grafito
            || (self.grafito_en_la_mano && FIGURAS_DE_GRAFITO.contains(&self.herramienta));
        if de_grafito {
            crate::tinta::MaterialTinta::Cuadritos
        } else {
            self.estilo.material
        }
    }
}

impl Gesto {
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Si el modo guia esta puesto.
    pub fn modo_guia(&self) -> bool {
        self.modo_guia
    }

    /// **Pone o quita el modo guia.** Lo elegido deja de ser tocable al
    /// cambiar de mundo, asi que se suelta: los tiradores se quedarian
    /// encima de algo que ya no responde, que es peor que no verlos (el
    /// `set` de `modoReferencia` del movil). Devuelve si cambio.
    pub fn poner_modo_guia(&mut self, si: bool) -> bool {
        if self.modo_guia == si {
            return false;
        }
        self.modo_guia = si;
        self.seleccion.limpiar();
        true
    }

    /// **Esconde o ensena las lineas de referencia** (`alternarReferencias`
    /// del movil). No borra nada. Lo escondido no puede quedar elegido: los
    /// tiradores flotarian sobre algo que ya no se ve.
    pub fn alternar_referencias(&mut self, escena: &mut Escena) {
        escena.referencias_visibles = !escena.referencias_visibles;
        if !escena.referencias_visibles {
            let quedan: Vec<u64> = self
                .seleccion
                .ids()
                .iter()
                .copied()
                .filter(|id| escena.buscar(*id).is_none_or(|e| !e.extras.referencia))
                .collect();
            self.seleccion.poner_todos(quedan);
        }
    }

    /// El mundo en el que pica este gesto en `escena` (ver `impacto::Mundo`).
    pub fn mundo(&self, escena: &Escena) -> crate::impacto::Mundo {
        crate::impacto::Mundo {
            guias: self.modo_guia,
            guias_visibles: escena.referencias_visibles,
        }
    }

    /// **Cuanto tiene que quedarse quieto el cursor** para el gesto de
    /// pararse: el medio segundo del lapiz, o el segundo entero del grafito
    /// (`ESPERA_DEL_GRAFITO` del movil, v0.92: escribiendo, la mano se para
    /// a cada letra).
    pub fn espera_de_la_forma_ms(&self) -> u64 {
        if self.herramienta == Herramienta::Grafito {
            crate::forma_rapida::ESPERA_DEL_GRAFITO_MS
        } else {
            crate::forma_rapida::ESPERA_PARA_LA_FORMA_MS
        }
    }

    pub fn en_reposo(&self) -> bool {
        matches!(self.estado, Estado::Reposo)
    }

    /// Si se esta arrastrando la seleccion (solo trasladarla: escalar y
    /// girar cambian la forma y no valen). La ventana pinta entonces lo
    /// elegido una sola vez en su propia capa y la mueve con la
    /// composicion, en vez de repintar la escena en cada aviso del raton.
    pub fn moviendo(&self) -> bool {
        // Con clavos lo elegido puede girar en vez de trasladarse: una capa
        // que solo se traslada ensenaria otra cosa que lo que pasa.
        matches!(self.estado, Estado::Moviendo { .. }) && !self.con_clavos
    }

    /// Si se esta estirando o girando lo elegido por sus tiradores. Cambia
    /// de forma en cada aviso (no vale una capa quieta), pero **solo cambia
    /// lo elegido** y las flechas que lo siguen: la ventana rehace entonces
    /// solo ese trozo encima de la capa congelada.
    pub fn transformando(&self) -> bool {
        matches!(
            self.estado,
            Estado::Escalando { .. } | Estado::Girando { .. }
        )
    }

    /// El trazo a mano que se esta dibujando ahora. La ventana lo excluye de
    /// la capa congelada (D120): es lo unico que cambia en cada fotograma.
    pub fn trazo_en_curso(&self) -> Option<u64> {
        match self.estado {
            Estado::Dibujando { id }
                if matches!(
                    self.herramienta,
                    Herramienta::Lapiz | Herramienta::Resaltador | Herramienta::Grafito
                ) =>
            {
                Some(id)
            }
            _ => None,
        }
    }

    /// El gesto de pararse (`DrawController.latido` del movil, v0.80.2): si
    /// se esta dibujando a mano y el cursor se ha quedado quieto, lo trazado
    /// sale limpio y los movimientos siguientes lo ajustan. Segun lo que se
    /// lleve (ver `forma_rapida::al_pararse`):
    /// - clavado sin ir a ningun sitio: **compas**, un circulo con centro
    ///   donde se pulso y radio hasta el cursor;
    /// - una «L»: **rectangulo** de la esquina de salida a la del cursor;
    /// - una linea, un rectangulo cerrado o una elipse: esa figura.
    ///
    /// Lo llama la ventana cuando el cursor se queda quieto con el boton
    /// pulsado. `escala` son unidades de escena por pixel de pantalla, como en
    /// `evento`: los umbrales del gesto son de la mano. `None` si no hay nada
    /// que convertir. Sigue siendo el mismo elemento, anadido en este paso:
    /// un Ctrl+Z lo quita entero.
    pub fn convertir_en_forma(
        &mut self,
        escena: &mut Escena,
        cursor: Punto2,
        escala: f32,
    ) -> Option<Respuesta> {
        use crate::forma_rapida::{AlPararse, FormaReconocida, al_pararse};
        let Estado::Dibujando { id } = self.estado else {
            return None;
        };
        // Los tres que trazan a mano, como en el movil (alli los tres son un
        // FREEDRAW). El grafito desde la v0.80.2: sin el no saltaba la recta,
        // el compas ni el rectangulo. La figura que sale conserva el
        // material, asi que sale de grafito.
        if !matches!(
            self.herramienta,
            Herramienta::Lapiz | Herramienta::Grafito | Herramienta::Resaltador
        ) {
            return None;
        }
        let e = escena.buscar(id)?;
        let (Figura::Lapiz { puntos, .. } | Figura::Resaltador { puntos }) = &e.figura else {
            return None;
        };
        let es_resaltador = matches!(e.figura, Figura::Resaltador { .. });
        let zoom = 1.0 / escala.max(0.0001);
        let forma = al_pararse(puntos, zoom)?;
        // **Escribiendo con grafito, una letra no es una raya** (v0.92 del
        // movil): la recta, la «L» y la forma reconocida piden un trazo que
        // en pantalla mida al menos `TRAZO_DE_FIGURA_DEL_GRAFITO_PX`. El
        // compas no: clavar la punta sin moverla no pasa escribiendo.
        if self.herramienta == Herramienta::Grafito
            && !matches!(forma, AlPararse::Compas { .. })
            && !crate::forma_rapida::trazo_de_figura_del_grafito(puntos, zoom)
        {
            return None;
        }
        // **La figura del resaltador mide lo que media su tinta.** Con el
        // grosor de las lineas (1/2/4) una raya de resaltador grueso (15 de
        // tinta) se quedaba en 4 al pararse: «delgadisima». El movil tiene el
        // mismo fallo (`comoUnaRaya` le pone el `strokeWidth` del estilo);
        // aqui se arregla. El lapiz si toma el de las lineas, como alli.
        let grosor = if es_resaltador {
            crate::tinta::ancho_del_resaltador(e.grosor)
        } else {
            crate::estilo::NivelGrosor::de_elemento(&e.figura, e.grosor).de_forma()
        };
        let relleno = self.estilo.relleno;
        let e = escena.buscar_mut(id)?;
        e.grosor = grosor;
        e.angulo = 0.0;
        // Y de una sola pasada: a mano alzada son dos, y translucidas su
        // cruce sale mas oscuro que el resto, cosa que la tinta no hace.
        if es_resaltador {
            e.rugosidad = 0.0;
        }
        // El resaltador pinta su tinta al 40 % (`pintado.rs`); la figura que
        // sale de el sigue siendo de resaltador, translucida, y no tapa lo que
        // se estaba subrayando.
        if es_resaltador {
            e.opacidad *= crate::tinta::OPACIDAD_DEL_RESALTADOR;
        }
        self.forma_elipse = None;
        self.forma_por_gesto = true;
        match forma {
            AlPararse::Compas { centro } => {
                // Un ovalo de verdad y no un poligono: el mismo que deja la
                // herramienta de elipse. Reutiliza el ajuste de la elipse
                // rapida con distancia y radios de 1: asi el radio es, en cada
                // aviso, la distancia del centro al cursor (el compas).
                let r = cursor.distancia(centro);
                e.figura = Figura::Elipse;
                e.relleno = relleno;
                (e.x, e.y, e.ancho, e.alto) = (centro.x - r, centro.y - r, 2.0 * r, 2.0 * r);
                self.forma_elipse = Some((centro, 1.0, 1.0, 1.0));
            }
            AlPararse::Ele { esquina } => {
                // Una esquina donde empezo el trazo y la otra en el cursor,
                // que sigue tirando de ella (las figuras de caja crecen desde
                // `trazo[0]` en `mover`).
                e.figura = Figura::Rectangulo;
                e.relleno = relleno;
                (e.x, e.y) = (esquina.x.min(cursor.x), esquina.y.min(cursor.y));
                (e.ancho, e.alto) = ((cursor.x - esquina.x).abs(), (cursor.y - esquina.y).abs());
                self.trazo.clear();
                self.trazo.push(esquina);
            }
            AlPararse::Forma(FormaReconocida::Linea { a, .. }) => {
                e.figura = Figura::Linea {
                    puntos: vec![a, cursor],
                };
                e.x = a.x;
                e.y = a.y;
                self.trazo.clear();
                self.trazo.push(a);
            }
            AlPararse::Forma(FormaReconocida::Rectangulo { caja }) => {
                e.figura = Figura::Rectangulo;
                e.relleno = relleno;
                (e.x, e.y, e.ancho, e.alto) = (caja.0, caja.1, caja.2 - caja.0, caja.3 - caja.1);
                // Crece desde la esquina opuesta al cursor: la que queda fija
                // mientras se tira de la otra.
                let esquinas = [
                    Punto2::nuevo(caja.0, caja.1),
                    Punto2::nuevo(caja.2, caja.1),
                    Punto2::nuevo(caja.0, caja.3),
                    Punto2::nuevo(caja.2, caja.3),
                ];
                let ancla = esquinas
                    .into_iter()
                    .max_by(|a, b| a.distancia(cursor).total_cmp(&b.distancia(cursor)))?;
                self.trazo.clear();
                self.trazo.push(ancla);
            }
            AlPararse::Forma(FormaReconocida::Elipse { caja }) => {
                e.figura = Figura::Elipse;
                e.relleno = relleno;
                (e.x, e.y, e.ancho, e.alto) = (caja.0, caja.1, caja.2 - caja.0, caja.3 - caja.1);
                let centro = Punto2::nuevo((caja.0 + caja.2) / 2.0, (caja.1 + caja.3) / 2.0);
                self.forma_elipse = Some((
                    centro,
                    cursor.distancia(centro).max(1.0),
                    e.ancho / 2.0,
                    e.alto / 2.0,
                ));
            }
        }
        e.tocar();
        Some(Respuesta {
            region: Region::Todo,
            cursor: FormaCursor::Cruz,
            pide: None,
        })
    }

    /// Cualquier elemento que se esta dibujando (no solo el trazo a mano) y
    /// el punto donde se pulso: la ventana pinta su punta predicha.
    pub fn elemento_en_curso(&self) -> Option<(u64, Option<Punto2>)> {
        match self.estado {
            // La elipse rapida no crece desde un origen: sin origen, la punta
            // predicha no la deforma.
            Estado::Dibujando { id } if self.forma_elipse.is_some() => Some((id, None)),
            Estado::Dibujando { id } => Some((id, self.trazo.first().copied())),
            _ => None,
        }
    }

    /// La marquesina en curso, para que la ventana la pinte.
    pub fn marquesina(&self) -> Option<(f32, f32, f32, f32)> {
        match self.estado {
            Estado::Marquesina { origen, hasta } => Some((origen.x, origen.y, hasta.x, hasta.y)),
            _ => None,
        }
    }

    /// Para la prueba de que el buffer se reutiliza.
    pub fn capacidad_del_trazo(&self) -> usize {
        self.trazo.capacity()
    }

    pub fn evento(&mut self, ev: EventoGesto, escena: &mut Escena, escala: f32) -> Respuesta {
        // Todo lo que elige, mueve o borra pica en el mundo del modo guia.
        let mundo = self.mundo(escena);
        crate::impacto::con_mundo(mundo, || self.evento_en_su_mundo(ev, escena, escala))
    }

    fn evento_en_su_mundo(
        &mut self,
        ev: EventoGesto,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        match ev {
            // Aqui el punto va CRUDO, a proposito: `pulsar` criba con el
            // punto de verdad y solo engancha una vez decidido que la rama
            // es de trazo o de calibrar. Si se enganchara antes, todo
            // anclaje de esquina, extremo o medio -que caen justo sobre la
            // geometria del vecino- haria que el paso 3 de `pulsar`
            // (`elemento_en`) diera positivo, y el clic se convertiria en
            // «seleccionar el vecino» en vez de dibujar o calibrar.
            EventoGesto::Pulsar {
                p,
                shift,
                alt,
                presion,
            } => {
                // Solo si el pulsar se atiende: un segundo pulsar sin su
                // soltar se ignora, y no debe cambiar el Alt de la salida.
                if self.en_reposo() {
                    self.alt = [alt; 2];
                }
                let r = self.pulsar(p, shift, alt, presion, escena, escala);
                // **Un clavo solo cuenta si sujeta algo de lo elegido.** Lo
                // que no atraviesa ninguno se traslada, gira y se estira
                // igual con clavos que sin ellos (`Libertad::Libre`), asi que
                // puede seguir moviendose como capa. Antes bastaba un clavo
                // en cualquier rincon del dibujo para que cada aviso del
                // raton rehiciera la escena entera.
                if self.con_clavos && self.clavo_en_mano.is_none() {
                    self.con_clavos = self.seleccion.ids().iter().any(|&id| {
                        crate::nudos::alfileres_de(&escena.alfileres, id)
                            .next()
                            .is_some()
                    });
                }
                r
            }
            EventoGesto::Mover {
                p,
                shift,
                alt,
                presion,
            } => {
                let p = self.enganchar(p, escena, escala);
                self.arrastrado |= !self.en_reposo();
                self.alt[1] = alt;
                self.mover(p, shift, alt, presion, escena, escala)
            }
            EventoGesto::Soltar { p } => {
                self.anclaje_activo = None;
                self.soltar(p, escena, escala)
            }
            EventoGesto::Escape => {
                escena.cancelar_paso();
                self.cancelar_geometria(escena);
                self.estado = Estado::Reposo;
                self.seleccion.limpiar();
                self.anclaje_activo = None;
                // Cancelar deshace lo movido, y con ello las flechas que lo
                // seguian: ni lista de seguidoras ni resaltado sobreviven.
                self.seguidoras.limpiar();
                self.candidatas = [None; 2];
                self.forma_por_gesto = false;
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::Suprimir => {
                escena.abrir_paso();
                for &id in self.seleccion.ids() {
                    // `borrar_apuntando` y no `borrar`: el primero apunta el
                    // cambio en el paso, el segundo no. Con `borrar`, la
                    // prueba `suprimir_borra_lo_seleccionado_de_una_vez`
                    // falla en su segunda mitad.
                    escena.borrar_apuntando(id);
                }
                escena.cerrar_paso();
                self.seleccion.limpiar();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::Deshacer => {
                escena.deshacer();
                self.seleccion.limpiar();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::Rehacer => {
                escena.rehacer();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::SeleccionarTodo => {
                // Todo lo de este mundo: fuera del modo guia, sin las guias.
                let mundo = crate::impacto::mundo();
                let vivos: Vec<u64> = escena
                    .visibles()
                    .filter(|e| mundo.admite(e))
                    .map(|e| e.id)
                    .collect();
                self.seleccion.poner_todos(vivos);
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
        }
    }

    /// Que se esta haciendo, para que el iman sepa a que debe pegarse.
    ///
    /// Sale de la maquina de estados y no de la herramienta puesta, que es
    /// la idea entera de `Faena`: lo que decide el enganche es que esta
    /// pasando, no que boton hay pulsado.
    fn faena(&self) -> Option<crate::enganche::Faena> {
        use crate::enganche::Faena;
        match self.estado {
            // El punto que hace nacer una figura.
            Estado::Reposo => match self.herramienta {
                Herramienta::Lapiz | Herramienta::Resaltador | Herramienta::Grafito => {
                    Some(Faena::AMano)
                }
                // `Mano` selecciona y mueve; `Lupa` es una vista y no deja
                // rastro; el `Borrador` quita, no coloca. Ninguna de las
                // tres pone un punto que merezca engancharse.
                Herramienta::Mano | Herramienta::Lupa | Herramienta::Borrador => None,
                _ => Some(Faena::Trazando),
            },
            // Lo que salio del gesto de pararse ya no es un trazo a mano: su
            // punta se engancha como la de una figura (`Iman.Faena.TRAZANDO`
            // del movil al enderezar o abrir el compas).
            Estado::Dibujando { .. } if self.forma_por_gesto => Some(Faena::Trazando),
            Estado::Dibujando { .. } => match self.herramienta {
                Herramienta::Lapiz | Herramienta::Resaltador | Herramienta::Grafito => {
                    Some(Faena::AMano)
                }
                _ => Some(Faena::Trazando),
            },
            // Calibrar es trazar una raya de dos puntos, y es donde mas
            // falta hace: el error de picar a pulso entra directo en la
            // escala y lo hereda todo lo que se mida despues.
            Estado::Calibrando => Some(Faena::Trazando),
            // El clavo que se lleva no se imanta: el iman se pegaria a las
            // propias figuras que atraviesa en cuanto se moviera un pelo.
            Estado::Moviendo { .. } if self.clavo_en_mano.is_some() => None,
            Estado::Moviendo { .. } => Some(Faena::Moviendo),
            Estado::Escalando { .. } | Estado::Girando { .. } => Some(Faena::Afinando),
            // Seleccionar no es dibujar: nada tira del cursor.
            Estado::Marquesina { .. } => None,
            // La punta de una flecha no se engancha al iman: la ayuda aqui
            // es el resaltado de la figura a la que se va a atar, y el iman
            // ademas se pegaria al punto viejo de la propia flecha.
            Estado::ArrastrandoPunta { .. } => None,
        }
    }

    /// El punto ya enganchado, y deja apuntado a que -para pintar la pista-.
    fn enganchar(&mut self, p: Punto2, escena: &Escena, escala: f32) -> Punto2 {
        let Some(faena) = self.faena() else {
            self.anclaje_activo = None;
            return p;
        };
        // `escala` son unidades de escena por pixel de pantalla -la ventana
        // pasa `1.0 / camara.zoom`-, y `sitio` quiere el zoom. Sin esta
        // vuelta el radio del iman sale invertido: acercarse lo haria
        // agarrar mas lejos.
        let zoom = 1.0 / escala.max(0.0001);
        // Lo que se esta dibujando o moviendo no cuenta: se engancharia a si
        // mismo en cuanto naciera.
        // Se pregunta con una funcion y no con una lista: al mover mil
        // elegidos, `contains` sobre la lista por cada elemento de la escena
        // se llevaba casi todo el tiempo de cada aviso del raton.
        let seleccion = &self.seleccion;
        let dibujando = match self.estado {
            Estado::Dibujando { id } => Some(id),
            _ => None,
        };
        let moviendo = matches!(self.estado, Estado::Moviendo { .. });
        let excluye = |id: u64| dibujando == Some(id) || (moviendo && seleccion.contiene(id));
        // La raya que se esta trazando sale de su primer punto: desde ahi se
        // buscan las tangentes.
        let desde = dibujando
            .and_then(|id| escena.buscar(id))
            .and_then(|e| match &e.figura {
                Figura::Linea { puntos }
                | Figura::Flecha { puntos, .. }
                | Figura::Cota { puntos } => puntos.first().copied(),
                _ => None,
            });
        let finos = crate::enganche::AjustesFinos {
            desde,
            ..self.finos
        };
        let encontrado = crate::enganche::sitio_fino_con(
            &escena.elementos,
            p,
            zoom,
            faena,
            &self.enganche,
            &finos,
            &excluye,
        );
        // Los puntos de las tablas de coordenadas y su eje tambien imantan.
        let encontrado = if escena.origen_coordenadas.is_some() {
            let tecleados = crate::instrumento::tabla::anclajes(
                &escena.tablas,
                escena.origen_coordenadas,
                escena.escala.as_ref(),
            );
            crate::enganche::con_coordenadas(
                encontrado,
                p,
                zoom,
                faena,
                &self.enganche,
                escena.origen_coordenadas,
                &tecleados,
            )
        } else {
            encontrado
        };
        self.anclaje_activo = encontrado;
        encontrado.map_or(p, |a| a.punto)
    }

    /// Pone la escala del lienzo a partir de una medida real.
    ///
    /// Devuelve si pudo: una calibracion imposible no toca la escena (D36).
    /// Va en un paso de deshacer porque calibrar mal y no poder volver atras
    /// seria tener que rehacer el lienzo entero.
    pub fn calibrar(
        &mut self,
        escena: &mut Escena,
        largo_px: f32,
        valor: f32,
        unidad: &str,
    ) -> bool {
        let Some(nueva) = Escala::calibrando(largo_px, valor, unidad, 2) else {
            return false;
        };
        escena.abrir_paso();
        escena.apuntar_escala();
        escena.escala = Some(nueva);
        escena.cerrar_paso();
        true
    }

    /// Los tiradores de la seleccion, si hay algo elegido.
    ///
    /// Publico a proposito: es la unica fuente de verdad sobre donde caen
    /// los tiradores. `cursor_en` y `pulsar` ya lo usaban para decidir que
    /// agarra el clic; quien los PINTA tiene que llamar a este mismo
    /// metodo y no recalcular el angulo por su cuenta, que es justo lo que
    /// paso una vez: los tiradores se pintaban rectos mientras el clic
    /// respondia girado.
    pub fn tiradores(&self, escena: &Escena, escala: f32) -> Option<Tiradores> {
        // Mientras se escribe no hay marco (`marco_visible`), y un tirador
        // que no se ve no puede agarrar: el clic cerca del texto que se
        // escribe lo estiraria sin que se supiera por que.
        if !self.marco_visible() {
            return None;
        }
        let caja = self.seleccion.caja(escena)?;
        // Con un solo elemento, el marco lleva su angulo. Con varios, la
        // caja es paralela a los ejes y cada uno conserva el suyo.
        let angulo = match self.seleccion.ids() {
            [uno] => escena.buscar(*uno).map_or(0.0, |e| e.angulo),
            _ => 0.0,
        };
        Some(Tiradores::de_caja(caja, angulo, escala))
    }

    /// El elemento nuevo que empieza esta herramienta en este punto.
    ///
    /// Los puntos se reservan de una vez, igual que el buffer del gesto: es
    /// lo que hace que mover el raton dibujando **no asigne memoria**. Si
    /// este `Vec` empezara vacio, crecer de 4 a 8 a 16... asignaria una
    /// docena de veces por trazo, en el unico camino del programa con un
    /// plazo sagrado. La tarea 15 lo comprueba contando asignaciones.
    /// El tamano de letra con que nace lo nuevo: el del pincel, o el de
    /// fabrica (20, el `fontSize` de `ItemStyle`).
    fn tam_letra_de_lo_nuevo(&self) -> f32 {
        if self.estilo.tamano_letra > 0.0 {
            self.estilo.tamano_letra
        } else {
            crate::texto::TAM_POR_DEFECTO
        }
    }

    fn nuevo_elemento(&self, p: Punto2) -> Elemento {
        let reservados = || {
            let mut v = Vec::with_capacity(PUNTOS_RESERVADOS);
            v.push(p);
            v
        };
        let figura = match self.herramienta {
            Herramienta::Lapiz | Herramienta::Grafito => Figura::Lapiz {
                puntos: reservados(),
                // Reservadas igual que los puntos: la primera presion real no
                // puede pedir memoria en el camino caliente.
                presiones: Vec::with_capacity(PUNTOS_RESERVADOS),
                opciones: Some(crate::tinta::OpcionesTinta {
                    variabilidad: self.variabilidad,
                    streamline: crate::tinta::STREAMLINE_RATON_NUEVO,
                }),
            },
            Herramienta::Resaltador => Figura::Resaltador {
                puntos: reservados(),
            },
            Herramienta::Linea => Figura::Linea { puntos: vec![p, p] },
            // Puntas y tipo, los que el panel dejo como «actual»
            // (`currentItemArrowType` y compania en Excalidraw).
            Herramienta::Flecha => Figura::Flecha {
                puntos: vec![p, p],
                punta_inicio: self.estilo.punta_inicio,
                punta_fin: self.estilo.punta_fin,
                codos: self.estilo.codos,
            },
            Herramienta::Elipse => Figura::Elipse,
            Herramienta::Rombo => Figura::Rombo,
            // Pixelado o desenfocado, lo que el panel dejo puesto (`mosaicBlur`).
            Herramienta::Mosaico => Figura::Mosaico {
                desenfoque: self.estilo.desenfoque,
            },
            // Por dentro es una flecha, como en el movil: asi se edita, se
            // exporta y viaja por el mismo camino que la recta.
            Herramienta::FlechaLibre => Figura::Flecha {
                puntos: reservados(),
                punta_inicio: self.estilo.punta_inicio,
                punta_fin: self.estilo.punta_fin,
                codos: false,
            },
            // **Nace YA doblando.** `codo.rs` es el porte entero de
            // `Elbow.kt` y `pintado.rs` lo usa cuando `codos` esta puesto,
            // pero la herramienta seguia poniendolo a `false`: el boton del
            // conector de organigrama hacia una flecha recta, que es lo mismo
            // que el boton de al lado. `elbowed` va y vuelve por el fichero,
            // asi que llega al movil diciendo lo que es.
            Herramienta::FlechaCodos => Figura::Flecha {
                puntos: vec![p, p],
                punta_inicio: self.estilo.punta_inicio,
                punta_fin: self.estilo.punta_fin,
                codos: true,
            },
            // Nace como la GUIA —el ovalo sin repasar—, que es el primer
            // estado de verdad del arco y no un arco a medio hacer.
            Herramienta::Arco => Figura::Arco {
                inicio: 0.0,
                barrido: None,
            },
            // El numero lo pone quien sabe cuantos hay en la escena (el
            // movil suma uno al mayor); el motor no cuenta elementos.
            Herramienta::Serie => Figura::Serie { numero: 1 },
            // La letra la pone la serie de quien la coloque, por lo mismo.
            Herramienta::Punto => Figura::Punto {
                letra: String::new(),
                angulo: -std::f32::consts::FRAC_PI_4,
                radio: 14.0,
            },
            // No nace arrastrando: es una varita (`dibujo::lupa`), como la lupa.
            Herramienta::Foco => Figura::Foco {
                cristal: Default::default(),
            },
            Herramienta::Cota => Figura::Cota {
                puntos: reservados(),
            },
            Herramienta::EscalaGrafica => Figura::EscalaGrafica,
            Herramienta::Marco => Figura::Marco {
                // Sin nombre: ponerle uno automatico obligaria a contar los
                // marcos aqui, y el motor no sabe de nombres bonitos.
                nombre: String::new(),
            },
            // Rectangulo y todo lo demas que deje rastro.
            _ => Figura::Rectangulo,
        };
        let mut e = Elemento {
            id: 0, // lo pone `Escena::anadir`
            figura,
            x: p.x,
            y: p.y,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: self.estilo.trazo,
            // El fondo solo tiene sentido en lo que encierra un area.
            estilo_relleno: self.estilo.estilo_relleno,
            relleno: match self.herramienta {
                Herramienta::Rectangulo
                | Herramienta::Elipse
                | Herramienta::Rombo
                | Herramienta::Arco
                | Herramienta::Serie => self.estilo.relleno,
                _ => None,
            },
            grosor: if matches!(self.herramienta, Herramienta::Lapiz | Herramienta::Grafito) {
                self.grosor_tinta
            } else if self.herramienta == Herramienta::Resaltador {
                self.estilo.grosor.de_resaltador()
            } else {
                self.estilo.grosor.de_forma()
            },
            estilo: self.estilo.estilo,
            rugosidad: self.estilo.rugosidad,
            opacidad: self.estilo.opacidad,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            // Las esquinas del «actual», solo en lo que `pintado.rs` sabe
            // redondear: en otra figura seria un `roundness` que no se ve.
            // La flecha recta nace curva si ese es el tipo «actual»: en una
            // flecha, `roundness` es la curva (`curva.rs`).
            redondo: (self.estilo.redondo
                && matches!(
                    self.herramienta,
                    Herramienta::Rectangulo | Herramienta::Rombo
                ))
                || (self.estilo.curva
                    && !self.estilo.codos
                    && self.herramienta == Herramienta::Flecha),
            // **El trazo nuevo nace con el material elegido**, igual que
            // nace con el color y con el grosor: el panel deja el «actual» y
            // lo siguiente que se dibuje sale de ahi.
            material: self.material_de_lo_nuevo(),
            // El texto nace con la alineacion «actual», escrita: es lo que
            // hace `newElement` del movil con `textAlign` y `verticalAlign`.
            extras: if self.herramienta == Herramienta::Texto {
                crate::elemento::Extras {
                    alineacion: Some(self.estilo.alineacion),
                    alineacion_vertical: Some(self.estilo.alineacion_vertical),
                    ..Default::default()
                }
            } else if matches!(
                self.herramienta,
                Herramienta::Serie
                    | Herramienta::Cota
                    | Herramienta::Punto
                    | Herramienta::EscalaGrafica
            ) {
                // **La letra del pincel, si se eligio una** (`fontFamily`, como
                // la cota del movil): sin elegir, su letra de siempre.
                crate::elemento::Extras {
                    familia: self
                        .estilo
                        .familia
                        .map(|n| crate::texto::nombre_de_familia(Some(n)).to_string()),
                    ..Default::default()
                }
            } else {
                Default::default()
            },
        };
        // El cierre del arco y el degradado del panel (laminas de geometria).
        if matches!(self.herramienta, Herramienta::Arco) {
            e.extras.cierre = self.estilo.cierre_arco;
        }
        if let (Some(t), true) = (
            self.estilo.degradado,
            matches!(
                self.herramienta,
                Herramienta::Rectangulo
                    | Herramienta::Elipse
                    | Herramienta::Rombo
                    | Herramienta::Arco
                    | Herramienta::Relleno
            ),
        ) {
            let base = e.relleno.unwrap_or(self.estilo.trazo);
            e.extras.degradado = Some(crate::geometria_exacta::Degradado::desde_relleno(t, base));
        }
        // Lo que se traza con el modo guia puesto es guia, sea con lo que sea.
        e.extras.referencia = self.modo_guia;
        // **El grafito nace con la dureza elegida** (`dureza =
        // scene.style.dureza` del movil). HB no se apunta.
        if e.material.es_grafito() {
            e.extras.dureza = self.estilo.dureza.para_guardar();
        }
        e
    }

    /// Que cursor toca en este punto, estando en reposo.
    fn cursor_en(&self, p: Punto2, escena: &Escena, escala: f32) -> FormaCursor {
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                // Van los dos: cual, para saber a que lado apunta; y el
                // angulo, para girar esa direccion con el elemento.
                Some(Agarre::Tamano(t)) => {
                    return FormaCursor::Escalar {
                        tirador: t,
                        angulo: ts.angulo,
                    };
                }
                Some(Agarre::Giro) => return FormaCursor::Giro,
                None => {}
            }
        }
        match self.herramienta {
            Herramienta::Mano => {
                if elemento_en(&escena.elementos, p).is_some() {
                    FormaCursor::Mover
                } else {
                    FormaCursor::Flecha
                }
            }
            Herramienta::Texto => FormaCursor::Texto,
            _ => FormaCursor::Cruz,
        }
    }

    fn pulsar(
        &mut self,
        p: Punto2,
        shift: bool,
        _alt: bool,
        presion: Option<f32>,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        // Un segundo pulsar sin su soltar: la ventana puede recibirlo si el
        // usuario solto fuera. Se ignora en vez de perder el gesto entero.
        if !self.en_reposo() {
            return Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
                pide: None,
            };
        }
        escena.abrir_paso();
        // El punto llega crudo y todas las cribas de abajo lo usan crudo.
        // La pista de un gesto anterior no puede sobrevivir a este clic.
        self.anclaje_activo = None;
        // El arco exacto, el repasado de una guia y la marca de angulo
        // (`gesto/geometria.rs`): si son ellos, ya esta.
        if let Some(r) = self.pulsar_geometria(p, escena, escala) {
            return r;
        }
        self.arrastrado = false;
        self.seguidoras.limpiar();
        self.con_clavos = !escena.alfileres.is_empty();
        self.clavo_en_mano = None;

        // Elegir, agarrar tiradores y mover es SOLO de la mano (el selector).
        // Con el lapiz en la mano, pulsar encima de un trazo tiene que
        // dibujar, no arrastrarlo: el usuario lo reporto dibujando sobre lo
        // que acababa de trazar y llevandoselo por delante.
        let selecciona = self.herramienta == Herramienta::Mano;

        // 0. ¿Un clavo de soldar? **Antes que nada** (`beginSelectionGesture`
        //    del movil): un clavo y un tirador en el mismo sitio son dos
        //    reglas peleandose por el mismo clic, y donde hay clavo manda el
        //    clavo. Cogerlo es arrancarlo para clavarlo en otro sitio con todo
        //    lo que atraviesa (`nudos::mover_clavo`).
        if selecciona
            && self.con_clavos
            && let Some(i) =
                crate::nudos::clavo_en(escena, p, crate::nudos::RADIO_DEL_CLAVO * escala)
        {
            self.clavo_en_mano = Some(i);
            self.estado = Estado::Moviendo { anterior: p };
            return Respuesta {
                region: Region::Nada,
                cursor: FormaCursor::Mover,
                pide: None,
            };
        }

        // Con otra herramienta en la mano, un clic cierra el texto que se
        // estuviera escribiendo (el de la de texto lo cierra su rama, abajo).
        if self.herramienta != Herramienta::Texto && self.escribiendo.is_some() {
            self.cerrar_texto(escena);
        }

        // 1. Un tirador manda sobre lo que haya debajo. Este SI vale con
        // cualquier herramienta: si se ve el tirador, tiene que agarrar.
        //
        // Los puntos de una flecha elegida van antes que los de su caja: en
        // una flecha en diagonal dos esquinas de la caja caen justo sobre sus
        // extremos, y lo que se quiere al pinchar ahi es llevar la punta a
        // otra figura, no estirar la flecha entera.
        if let Some((id, agarre)) = self.agarre_de_punta(p, escena, escala) {
            escena.apuntar_edicion(id);
            self.candidatas = [None; 2];
            self.estado = Estado::ArrastrandoPunta { id, agarre };
            return Respuesta {
                region: Region::Nada,
                cursor: FormaCursor::Mover,
                pide: None,
            };
        }
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                Some(Agarre::Tamano(t)) => {
                    self.estado = Estado::Escalando { tirador: t };
                    // Con varios elegidos se estiran como bloque desde como
                    // estaban al pulsar (`estirar_bloque`).
                    self.originales_del_bloque = if self.seleccion.cuantos() > 1 {
                        escena
                            .elementos
                            .iter()
                            .filter(|e| self.seleccion.contiene(e.id))
                            .cloned()
                            .collect()
                    } else {
                        Vec::new()
                    };
                    self.preparar_seguidoras(escena);
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Escalar {
                            tirador: t,
                            angulo: ts.angulo,
                        },
                        pide: None,
                    };
                }
                Some(Agarre::Giro) => {
                    self.estado = Estado::Girando {
                        anterior: angulo_hacia(ts.centro, p),
                    };
                    self.preparar_seguidoras(escena);
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Giro,
                        pide: None,
                    };
                }
                None => {}
            }
        }

        // 2. Lo ya seleccionado manda sobre lo de encima.
        let sobre_lo_elegido = self
            .seleccion
            .ids()
            .iter()
            .filter_map(|id| escena.buscar(*id))
            .any(|e| crate::impacto::toca(e, p));
        if selecciona && sobre_lo_elegido && !shift {
            // La instantanea de cada elemento se toma aqui, al pulsar, y no
            // en el primer `mover`: eso deja el camino caliente del
            // arrastre —los avisos del raton que siguen— en cero
            // asignaciones. `apuntar_edicion` ya evita duplicarla si el
            // gesto la vuelve a pedir.
            for &id in self.seleccion.ids() {
                escena.apuntar_edicion(id);
            }
            self.estado = Estado::Moviendo { anterior: p };
            self.preparar_seguidoras(escena);
            return Respuesta {
                region: Region::Nada,
                cursor: FormaCursor::Mover,
                pide: None,
            };
        }

        // 3. Lo que haya bajo el cursor.
        if selecciona && let Some(id) = elemento_en(&escena.elementos, p) {
            // **Pinchar uno de un grupo coge el grupo entero**, como en
            // Excalidraw. Sin esto «Agrupar» del panel no hacia nada que se
            // viera: se guardaba el grupo y el clic seguia eligiendo una
            // sola pieza.
            let grupo = crate::organizar::hermanos_de(escena, id);
            // **El marco de un texto, del tamano del texto.** Un texto
            // guardado con la caja medida a ojo, o traido del movil con su
            // letra, se remide al elegirlo: el marco que aparece al hacerle
            // clic es el de lo escrito y no el de la cuenta de antes.
            for &h in &grupo {
                Self::ajustar_caja_de_texto(escena, h);
            }
            if shift {
                for h in grupo {
                    self.seleccion.alternar(h);
                }
            } else {
                self.seleccion.poner_todos(grupo);
            }
            self.estado = Estado::Moviendo { anterior: p };
            self.preparar_seguidoras(escena);
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Mover,
                pide: None,
            };
        }

        // 4. Vacio.
        if self.herramienta == Herramienta::Mano {
            if !shift {
                self.seleccion.limpiar();
            }
            self.estado = Estado::Marquesina {
                origen: p,
                hasta: p,
            };
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Flecha,
                pide: None,
            };
        }
        // Escalar no deja rastro: su raya no es un elemento de la escena,
        // solo puntos en `trazo`. Al soltar se convierte en una peticion,
        // no en un `Cambio::Anadido`.
        if self.herramienta == Herramienta::Escalar {
            // Aqui si: ya no hay ninguna criba que envenenar, y este es el
            // enganche que justifica la fase -picar el extremo exacto de
            // una pared de medida conocida-.
            let p = self.enganchar(p, escena, escala);
            self.trazo.clear();
            self.trazo.push(p);
            self.estado = Estado::Calibrando;
            return Respuesta {
                region: Region::Nada,
                cursor: FormaCursor::Cruz,
                pide: None,
            };
        }
        // Con la herramienta de texto, pulsar abre uno para escribir: el que
        // haya bajo el cursor si es un texto, y si no uno nuevo ahi mismo.
        if self.herramienta == Herramienta::Texto {
            self.cerrar_texto(escena);
            let elementos: Vec<Elemento> = escena.visibles().cloned().collect();
            let debajo = elemento_en(&elementos, p).and_then(|id| {
                escena.buscar(id).and_then(|e| match &e.figura {
                    // Un texto rico se vuelve a editar como fuente: con sus
                    // `$...$` y sus marcas, que se componen al salir.
                    Figura::Texto { texto, .. } => {
                        Some((id, e.extras.rico.clone().unwrap_or_else(|| texto.clone())))
                    }
                    _ => None,
                })
            });
            self.antes_de_escribir = debajo
                .as_ref()
                .and_then(|(id, _)| escena.buscar(*id).cloned());
            let (id, contenido) = match debajo {
                Some(par) => par,
                None => {
                    // Nace vacio. Si al terminar sigue vacio, `cerrar_texto`
                    // lo quita: un elemento invisible solo estorba al elegir.
                    let mut e = self.nuevo_elemento(p);
                    e.figura = Figura::Texto {
                        texto: String::new(),
                        // La letra que el panel dejo como «actual».
                        tam: self.estilo.tamano_letra,
                        familia: self
                            .estilo
                            .familia
                            .map_or(crate::texto::FAMILIA_POR_DEFECTO, |n| {
                                crate::texto::nombre_de_familia(Some(n))
                            })
                            .to_string(),
                    };
                    e.x = p.x;
                    e.y = p.y;
                    escena.abrir_paso();
                    let id = escena.anadir(e);
                    escena.cerrar_paso();
                    (id, String::new())
                }
            };
            // **Elegido, pero sin marco ni tiradores mientras se escribe**
            // (`marco_visible`): como en Excalidraw, se escribe donde se hizo
            // clic y solo se ve el cursor. Queda elegido para que el panel
            // cambie la letra, el tamano o el color de lo que se esta
            // escribiendo y no los del siguiente.
            self.seleccion.poner(id);
            // La caja, ya a la medida (un texto vacio mide un espacio): la del
            // texto de antes podia venir medida a ojo o con otra letra.
            Self::volcar_texto(escena, id, &contenido);
            self.escribiendo = Some((id, crate::texto::EdicionTexto::nueva(contenido)));
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Texto,
                pide: None,
            };
        }
        // Texto y emoji no nacen aqui: el texto se escribe, y el emoji lo
        // coloca la sesion del universo con el caracter ya elegido.
        if self.herramienta.deja_rastro()
            && !matches!(self.herramienta, Herramienta::Texto | Herramienta::Emoji)
        {
            // Igual que en calibrar: la figura nace ya pegada al vertice
            // ajeno, pero la decision de que NACE se tomo con el punto
            // crudo.
            let p = self.enganchar(p, escena, escala);
            self.seleccion.limpiar();
            self.trazo.clear();
            self.trazo.push(p);
            let mut e = self.nuevo_elemento(p);
            // Con lapiz, la muestra de presion puede llegar en el mismo
            // mensaje que el clic. Sin raton no hay presion: el trazo
            // arranca sin ella y `anadir_a_lapiz` la rellena hacia atras en
            // cuanto llegue la primera de verdad.
            if let (
                Some(pr),
                Figura::Lapiz {
                    presiones,
                    opciones,
                    ..
                },
            ) = (presion, &mut e.figura)
            {
                presiones.push(pr);
                if let Some(o) = opciones {
                    o.streamline = crate::tinta::STREAMLINE_LAPIZ;
                }
            }
            // **Un toque = un numero, ya hecho** (`Tool.SERIAL` del movil:
            // «no se arrastra, igual que el texto»). Nacia con el 1 y sin
            // tamano, y habia que agrandarlo como un circulo: ahora sale el
            // siguiente de la escena (`serie::siguiente`), centrado en el
            // clic y del tamano de la letra del pincel (radio `fontSize` x
            // 0,9), y el siguiente clic pone el que sigue.
            if self.herramienta == Herramienta::Serie {
                let s = crate::serie::nuevo(&escena.elementos, p, self.tam_letra_de_lo_nuevo(), &e);
                escena.anadir(s);
                self.trazo.clear();
                self.estado = Estado::Reposo;
                return Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Cruz,
                    pide: None,
                };
            }
            let id = escena.anadir(e);
            self.forma_elipse = None;
            self.forma_por_gesto = false;
            self.estado = Estado::Dibujando { id };
            // Una flecha que nace encima de una figura sale de ella: se
            // resalta desde el primer momento, como en Excalidraw.
            self.candidatas = [None; 2];
            if self.dibujando_flecha() {
                self.candidatas[0] =
                    crate::enlace::figura_bajo(&escena.elementos, p, 1.0 / escala.max(0.0001), id);
            }
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Cruz,
                pide: None,
            };
        }
        Respuesta {
            region: Region::Nada,
            cursor: FormaCursor::Cruz,
            pide: None,
        }
    }

    fn mover(
        &mut self,
        p: Punto2,
        shift: bool,
        alt: bool,
        presion: Option<f32>,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        if let Some(r) = self.mover_geometria(p, escena) {
            return r;
        }
        match self.estado {
            Estado::Reposo => Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
                pide: None,
            },

            Estado::Dibujando { id } => {
                // El camino caliente. Ni una asignacion: el buffer ya esta
                // reservado, y se ensucia el tramo y no la pantalla.
                let anterior = *self.trazo.last().unwrap_or(&p);
                if self.trazo.len() < PUNTOS_RESERVADOS {
                    self.trazo.push(p);
                }
                let grosor = escena.buscar(id).map_or(3.0, |e| e.grosor);
                // Lapiz y Resaltador pintan mucho mas ancho que su `grosor`
                // (el contorno de `freehand` llega a `grosor * FACTOR_VARIABLE`
                // para el Lapiz y a `grosor * 3.0` para el Resaltador), y el
                // streamline atrasa los puntos ya suavizados: la caja
                // anterior->p con margen `grosor/2` dejaba fuera casi todo el
                // contorno que de verdad cambia, y el trazo dejaba estela.
                //
                // La caja se calcula sobre los puntos de la FIGURA
                // (`puntos`, ya sin duplicados exactos), no sobre
                // `self.trazo`: ese buffer deja de crecer a partir de
                // `PUNTOS_RESERVADOS` (512) para no asignar, asi que un
                // trazo mas largo lo dejaria congelado en un tramo viejo y
                // la tinta nueva quedaria fuera de la zona presentada.
                let mut caja_ancha: Option<(f32, f32, f32, f32)> = None;
                // Las figuras (no el trazo) cambian ENTERAS al mover el cursor: un
                // rectangulo crece desde donde se pulso y sus lados lejanos
                // tambien se mueven. La zona sucia es su caja antes y despues;
                // con solo el tramo anterior->p esos lados se presentaban tarde
                // y la figura parecia arrastrarse (lo noto el usuario).
                let caja_antes = escena.buscar(id).map(|e| e.caja());
                let elipse_rapida = self.forma_elipse;
                if let Some(e) = escena.buscar_mut(id) {
                    match &mut e.figura {
                        Figura::Lapiz {
                            puntos,
                            presiones,
                            opciones,
                        } => {
                            anadir_a_lapiz(puntos, presiones, opciones, p, presion);
                            let margen = grosor * crate::tinta::FACTOR_VARIABLE + 2.0;
                            caja_ancha = Some(caja_ancha_de(puntos, p, margen));
                        }
                        Figura::Resaltador { puntos } => {
                            if puntos.last() != Some(&p) {
                                puntos.push(p);
                            }
                            let margen = grosor * 3.0 + 2.0;
                            caja_ancha = Some(caja_ancha_de(puntos, p, margen));
                        }
                        Figura::Linea { puntos } | Figura::Flecha { puntos, .. } => {
                            // Linea y flecha son dos puntos: el segundo sigue
                            // al cursor en vez de acumularse.
                            if let Some(ultimo) = puntos.last_mut() {
                                *ultimo = p;
                            }
                        }
                        Figura::Cota { puntos } => {
                            // Como la Linea: el segundo punto sigue al
                            // cursor. Pero `nuevo_elemento` la arranca con
                            // un solo punto (`reservados()`), asi que el
                            // primer aviso anade el segundo en vez de
                            // sustituirlo.
                            if puntos.len() < 2 {
                                puntos.push(p);
                            } else if let Some(ultimo) = puntos.last_mut() {
                                *ultimo = p;
                            }
                        }
                        Figura::Elipse if elipse_rapida.is_some() => {
                            // Forma rapida: escala desde el centro, en
                            // proporcion a como se aleja el cursor.
                            if let Some((c, d0, rx, ry)) = elipse_rapida {
                                let k = p.distancia(c) / d0.max(1.0);
                                e.x = c.x - rx * k;
                                e.y = c.y - ry * k;
                                e.ancho = 2.0 * rx * k;
                                e.alto = 2.0 * ry * k;
                            }
                        }
                        _ => {
                            // Las figuras de caja crecen desde donde se
                            // pulso. La escala grafica cae aqui tambien.
                            let o = *self.trazo.first().unwrap_or(&p);
                            e.x = o.x.min(p.x);
                            e.y = o.y.min(p.y);
                            e.ancho = (p.x - o.x).abs();
                            e.alto = (p.y - o.y).abs();
                        }
                    }
                    e.tocar();
                }
                if caja_ancha.is_none() {
                    if let (Some(a), Some(e)) = (caja_antes, escena.buscar(id)) {
                        let d = e.caja();
                        // Margen para el grosor, el temblor de la rugosidad y la
                        // punta de una flecha, que salen de la caja geometrica.
                        let m = e.grosor * 2.0 + 4.0 * e.rugosidad + 24.0;
                        caja_ancha = Some((
                            a.0.min(d.0) - m,
                            a.1.min(d.1) - m,
                            a.2.max(d.2) + m,
                            a.3.max(d.3) + m,
                        ));
                    }
                }
                let region = match caja_ancha {
                    // El contorno de un punto nuevo se apoya en varios
                    // puntos anteriores (streamline y suavizado), y el
                    // extremo del trazo se recalcula entero: la caja tiene
                    // que cubrir la cola de puntos de la figura, no solo el
                    // tramo anterior->p.
                    Some((x0, y0, x1, y1)) => Region::Caja(x0, y0, x1, y1),
                    None => {
                        let m = grosor / 2.0 + 1.0;
                        Region::Caja(
                            anterior.x.min(p.x) - m,
                            anterior.y.min(p.y) - m,
                            anterior.x.max(p.x) + m,
                            anterior.y.max(p.y) + m,
                        )
                    }
                };
                // La figura a la que se ataria la punta si se soltase aqui.
                // Cuando cambia, el resaltado se va de una figura y aparece en
                // otra, lejos de la caja de la flecha: se repinta todo ese
                // aviso. Mientras no cambia, basta la caja de siempre.
                let region = if self.dibujando_flecha() {
                    let antes = self.candidatas[1];
                    self.candidatas[1] = crate::enlace::figura_bajo(
                        &escena.elementos,
                        p,
                        1.0 / escala.max(0.0001),
                        id,
                    );
                    if antes != self.candidatas[1] {
                        Region::Todo
                    } else {
                        region
                    }
                } else {
                    region
                };
                Respuesta {
                    region,
                    cursor: FormaCursor::Cruz,
                    pide: None,
                }
            }

            Estado::Moviendo { anterior } => {
                let (dx, dy) = (p.x - anterior.x, p.y - anterior.y);
                // Un marco arrastra lo que encierra (fase 5): es para lo que
                // sirve. Se pregunta primero si hay alguno porque lo normal
                // es que no: `con_contenidos` reserva memoria, y esto corre en
                // cada movimiento del raton (prueba `asignaciones`).
                // Una pasada por la escena preguntando a la seleccion (que
                // contesta en tiempo constante), y no una busqueda en la
                // escena por cada elegido: con mil elegidos de dos mil eran
                // 2,2 ms por aviso del raton (`tests/puertas.rs`).
                let hay_marco = escena
                    .elementos
                    .iter()
                    .any(|e| self.seleccion.contiene(e.id) && crate::marco::es_marco(e));
                if let Some(i) = self.clavo_en_mano {
                    // El clavo cogido: se lleva lo que atraviesa (`nudos`).
                    crate::nudos::mover_clavo(escena, i, p);
                } else if self.con_clavos && !hay_marco {
                    // Con clavos cada figura hace lo que su ley le deja:
                    // trasladarse, girar sobre su clavo o quedarse quieta.
                    let ids = self.seleccion.ids().to_vec();
                    crate::nudos::arrastrar(escena, &ids, anterior, p);
                } else if hay_marco {
                    // Lo elegido y lo que encerraba cada marco al cogerlo
                    // (`preparar_seguidoras`), ya apuntado para deshacer.
                    for &id in self.seleccion.ids() {
                        escena.apuntar_edicion(id);
                    }
                    for e in escena.elementos.iter_mut() {
                        if self.seleccion.contiene(e.id)
                            || self.del_marco.binary_search(&e.id).is_ok()
                        {
                            e.mover(dx, dy);
                        }
                    }
                } else {
                    // Sin esto el paso queda vacio y no hay nada que
                    // deshacer. Es el error mas facil de cometer aqui. Ya
                    // apuntado, `apuntar_edicion` solo mira un conjunto.
                    for &id in self.seleccion.ids() {
                        escena.apuntar_edicion(id);
                    }
                    for e in escena.elementos.iter_mut() {
                        if self.seleccion.contiene(e.id) {
                            e.mover(dx, dy);
                        }
                    }
                }
                // Las flechas atadas a lo movido se re-trazan en este mismo
                // aviso, no al soltar. Vacia —lo normal— no cuesta nada.
                self.seguidoras.seguir(&mut escena.elementos);
                self.estado = Estado::Moviendo { anterior: p };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Mover,
                    pide: None,
                }
            }

            Estado::Escalando { tirador } => {
                for &id in self.seleccion.ids() {
                    escena.apuntar_edicion(id);
                }
                if self.originales_del_bloque.len() > 1 {
                    // Un bloque (grafica, tabla, figura de la biblioteca): se
                    // estira entero y no cada pieza hacia el cursor.
                    let nuevos = crate::estirar_bloque::estirar(
                        &self.originales_del_bloque,
                        tirador,
                        p,
                        shift.then_some(true),
                    );
                    for n in nuevos {
                        if let Some(e) = escena.buscar_mut(n.id) {
                            (e.figura, e.x, e.y, e.ancho, e.alto) =
                                (n.figura, n.x, n.y, n.ancho, n.alto);
                            e.extras.tam_letra = n.extras.tam_letra;
                            e.tocar();
                        }
                    }
                } else {
                    for e in escena.elementos.iter_mut() {
                        if self.seleccion.contiene(e.id) {
                            transformar::escalar(e, tirador, p, shift, alt);
                        }
                    }
                }
                // Lo clavado vuelve a su clavo: estirar no lo despega.
                if self.con_clavos {
                    let ids = self.seleccion.ids().to_vec();
                    crate::nudos::sujetar(escena, &ids);
                }
                // Estirar una caja cambia su borde: la flecha atada lo sigue.
                self.seguidoras.seguir(&mut escena.elementos);
                let angulo = self.tiradores(escena, escala).map_or(0.0, |t| t.angulo);
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Escalar { tirador, angulo },
                    pide: None,
                }
            }

            Estado::Girando { anterior } => {
                let Some(centro) = self.seleccion.centro(escena) else {
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Giro,
                        pide: None,
                    };
                };
                let ahora = angulo_hacia(centro, p);
                let ahora = if shift { a_saltos(ahora) } else { ahora };
                let delta = ahora - anterior;
                if self.con_clavos {
                    // Con un clavo se gira sobre el clavo; con dos, nada.
                    let ids = self.seleccion.ids().to_vec();
                    crate::nudos::girar(escena, &ids, centro, delta);
                } else {
                    for &id in self.seleccion.ids() {
                        escena.apuntar_edicion(id);
                    }
                    let solo = self.seleccion.cuantos() == 1;
                    for e in escena.elementos.iter_mut() {
                        if self.seleccion.contiene(e.id) {
                            // El espacio de tres ejes, solo, no se inclina:
                            // se le da la vuelta (`girarElEspacio`).
                            if solo && transformar::girar_el_espacio(e, p, shift) {
                                continue;
                            }
                            transformar::girar(e, centro, delta);
                        }
                    }
                }
                self.seguidoras.seguir(&mut escena.elementos);
                self.estado = Estado::Girando { anterior: ahora };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Giro,
                    pide: None,
                }
            }

            Estado::Marquesina { origen, .. } => {
                self.estado = Estado::Marquesina { origen, hasta: p };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }

            Estado::ArrastrandoPunta { id, agarre } => {
                // `mover_punta` suelta el enganche de la punta que se lleva:
                // mientras se arrastra, la punta va con el cursor y no con su
                // figura vieja. Al soltar se decide si se ata de nuevo.
                if let Some(e) = escena.buscar_mut(id) {
                    crate::tiradores::mover_punta(e, agarre, p);
                }
                let indice = match agarre {
                    crate::tiradores::AgarrePunta::Principio => Some(0),
                    crate::tiradores::AgarrePunta::Final => Some(1),
                    _ => None,
                };
                if let Some(i) = indice {
                    self.candidatas[i] = crate::enlace::figura_bajo(
                        &escena.elementos,
                        p,
                        1.0 / escala.max(0.0001),
                        id,
                    );
                }
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Mover,
                    pide: None,
                }
            }

            Estado::Calibrando => {
                // Calibrar sobre el borde de una pared es el caso normal, y
                // a pulso sale torcida: un grado de mas son dos centimetros
                // de error por metro (diseno S5). Con Shift se sujeta a
                // horizontal o vertical, con el mismo mecanismo que ya usan
                // Linea y Flecha en `pixpin-ui/anotador.rs` -aqui con 2
                // pasos por media vuelta en vez de 12, porque calibrar solo
                // ofrece las dos, no las diagonales.
                let origen = *self.trazo.first().unwrap_or(&p);
                let p = if shift {
                    restringir_angulo(origen, p, 2.0)
                } else {
                    p
                };
                // Mismo buffer que `Dibujando`, ya reservado: mover el raton
                // calibrando tampoco asigna memoria.
                if self.trazo.len() < PUNTOS_RESERVADOS {
                    self.trazo.push(p);
                } else if let Some(ultimo) = self.trazo.last_mut() {
                    *ultimo = p;
                }
                Respuesta {
                    region: Region::Nada,
                    cursor: FormaCursor::Cruz,
                    pide: None,
                }
            }
        }
    }

    fn soltar(&mut self, p: Punto2, escena: &mut Escena, escala: f32) -> Respuesta {
        if let Estado::Marquesina { origen, .. } = self.estado {
            let caja = (origen.x, origen.y, p.x, p.y);
            let cogidos = dentro_de(&escena.elementos, caja);
            self.seleccion.poner_todos(cogidos);
        }
        // Escalar no deja rastro: al soltar, la distancia entre el primer y
        // el ultimo punto se convierte en una peticion. Menos de dos
        // pixeles DE PANTALLA es un clic, no una medida, y no se pide nada.
        // `largo_px` viaja en unidades de MUNDO -son las de `p`-, asi que el
        // umbral tiene que pasar por `escala` (unidades de mundo por pixel
        // de pantalla, la misma convencion que `HOLGURA_SELECCION*escala`
        // en `pintado::marco_de_seleccion`) o dejaria de ser dos pixeles en
        // cuanto la camara tuviera zoom.
        let pide = if let Estado::Calibrando = self.estado {
            let origen = *self.trazo.first().unwrap_or(&p);
            // El extremo es el ultimo punto del `trazo`, no el `p` crudo del
            // evento: es el que `mover` dejo ya sujeto a Shift. Si se usara
            // `p` a pulso, la sujecion de arriba pintaria la raya recta
            // mientras se arrastra y luego mediria la torcida real al
            // soltar -justo el fallo que Shift existe para evitar.
            let fin = self.trazo.last().copied().unwrap_or(p);
            let largo_px = origen.distancia(fin);
            (largo_px >= 2.0 * escala.max(f32::EPSILON)).then_some(Peticion::Calibrar { largo_px })
        } else {
            None
        };
        // **Las flechas, al soltar**: atar o soltar sus puntas. Las que
        // seguian a lo movido ya van donde tienen que ir —se re-trazaron en
        // cada aviso—; aqui se decide a que queda atada cada punta que se ha
        // dejado en un sitio. Todo DENTRO del paso de deshacer que sigue
        // abierto: si abriera el suyo, un solo Ctrl+Z dejaria la caja en su
        // sitio viejo y la flecha en el nuevo.
        // **Un compas que no llego a abrirse no deja nada.** Se clavo la
        // punta, se espero, y se solto sin arrastrar: un circulo de radio
        // cero no es un dibujo (el movil descarta en `finishCreating` las
        // figuras de menos de 2). Se cancela el paso entero, que es lo que
        // lo hizo nacer, y asi no gasta un Ctrl+Z.
        if self.forma_por_gesto
            && let Estado::Dibujando { id } = self.estado
            && escena.buscar(id).is_some_and(|e| {
                matches!(e.figura, Figura::Elipse)
                    && e.ancho.max(e.alto) < 2.0 * escala.max(f32::EPSILON)
            })
        {
            escena.cancelar_paso();
        }
        self.forma_por_gesto = false;
        // Un arco repasado de nada no deja nada (`gesto/geometria.rs`).
        self.soltar_geometria(escena);
        self.soltar_flechas(escena, escala);
        // Un paso sin cambios no entra en el historial, asi que hacer clic
        // sin arrastrar no consume un Ctrl+Z. De eso se encarga cerrar_paso.
        escena.cerrar_paso();
        // **La cota pide su medida nada mas trazarla** (`pendingCotaId` del
        // movil, con `pedirLaMedida` encendido de fabrica): es el momento en
        // que uno sabe cuanto mide. Una raya de menos de dos pixeles de
        // pantalla es un clic, no una cota que dictar.
        let pide = match (pide, self.estado) {
            (None, Estado::Dibujando { id })
                if self.herramienta == Herramienta::Cota
                    && self.pedir_la_medida
                    && escena.buscar(id).is_some_and(|e| {
                        !e.borrado
                            && crate::medida::longitud_de(e) >= 2.0 * escala.max(f32::EPSILON)
                    }) =>
            {
                Some(Peticion::DictarCota { id })
            }
            (pide, _) => pide,
        };
        self.forma_elipse = None;
        self.estado = Estado::Reposo;
        Respuesta {
            region: Region::Todo,
            cursor: FormaCursor::Flecha,
            pide,
        }
    }

    /// **Aplica lo dictado a la cota `id`**: el largo en las unidades de la
    /// escala (o en pixeles sin ella) y el angulo como en un plano. Un paso de
    /// deshacer propio, como `aplicarCota` del movil. `false` si no cambio
    /// nada (la cota ya no esta, o los numeros no valen).
    pub fn dictar_cota(escena: &mut Escena, id: u64, largo: f32, grados: f32) -> bool {
        let largo_px = crate::medida::largo_en_pixeles(largo, escena.escala.as_ref());
        let Some(antes) = escena
            .buscar(id)
            .filter(|e| !e.borrado && !e.bloqueado)
            .cloned()
        else {
            return false;
        };
        let mut nueva = antes.clone();
        crate::medida::con_largo_y_angulo(&mut nueva, largo_px, grados);
        if nueva == antes {
            return false;
        }
        escena.abrir_paso();
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            *e = nueva;
        }
        escena.cerrar_paso();
        true
    }
}

/// Hacia donde apunta un tirador, en un elemento girado `angulo`.
///
/// Cero es hacia arriba y crece en el sentido de las agujas, igual que
/// `transformar::angulo_hacia`. La ventana usa esto para elegir entre las
/// cuatro flechas que trae Windows.
pub fn direccion_del_tirador(t: Tirador, angulo: f32) -> f32 {
    use std::f32::consts::PI;
    let base = match t {
        Tirador::NorteBorde => 0.0,
        Tirador::NoresteEsquina => PI / 4.0,
        Tirador::EsteBorde => PI / 2.0,
        Tirador::SuresteEsquina => 3.0 * PI / 4.0,
        Tirador::SurBorde => PI,
        Tirador::SuroesteEsquina => 5.0 * PI / 4.0,
        Tirador::OesteBorde => 3.0 * PI / 2.0,
        Tirador::NoroesteEsquina => 7.0 * PI / 4.0,
    };
    base + angulo
}

/// La caja que envuelve los ultimos `n` puntos de una cola de puntos (o
/// todos si hay menos). Sin asignar: solo recorre la cola de un slice ya
/// reservado -es el camino caliente de `mover` mientras se dibuja a mano-.
fn caja_de_cola(puntos: &[Punto2], n: usize) -> (f32, f32, f32, f32) {
    let inicio = puntos.len().saturating_sub(n);
    let cola = &puntos[inicio..];
    let mut caja = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    for p in cola {
        caja.0 = caja.0.min(p.x);
        caja.1 = caja.1.min(p.y);
        caja.2 = caja.2.max(p.x);
        caja.3 = caja.3.max(p.y);
    }
    if cola.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        caja
    }
}

/// La caja sucia de Lapiz/Resaltador mientras se dibuja: la cola de los
/// ultimos 6 puntos DE LA FIGURA (no de `self.trazo`, que deja de crecer a
/// los 512 puntos para no asignar y por eso no vale como referencia de un
/// trazo largo), con el punto nuevo `p` metido en el minimo/maximo por si
/// se dedujo como duplicado exacto y no llego a entrar en `puntos`, y
/// expandida por `margen` (el radio maximo que puede alcanzar el contorno
/// de `freehand` para ese grosor).
fn caja_ancha_de(puntos: &[Punto2], p: Punto2, margen: f32) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = caja_de_cola(puntos, 6);
    x0 = x0.min(p.x);
    y0 = y0.min(p.y);
    x1 = x1.max(p.x);
    y1 = y1.max(p.y);
    (x0 - margen, y0 - margen, x1 + margen, y1 + margen)
}

/// Anade un punto a un trazo de lapiz respetando sus presiones.
///
/// - Solo se descartan duplicados exactos (D109), como Excalidraw.
/// - La primera presion real rellena hacia atras los puntos que llegaron sin
///   ella (el clic lo entrega un mensaje de raton, la presion llega despues)
///   y cambia el suavizado al del lapiz.
/// - Un punto sin presion en un trazo que ya la tiene repite la ultima, para
///   que las dos listas no se desalineen.
fn anadir_a_lapiz(
    puntos: &mut Vec<Punto2>,
    presiones: &mut Vec<f32>,
    opciones: &mut Option<crate::tinta::OpcionesTinta>,
    p: Punto2,
    presion: Option<f32>,
) {
    if puntos.last() == Some(&p) {
        return;
    }
    puntos.push(p);
    match presion {
        Some(pr) => {
            if presiones.is_empty() {
                presiones.resize(puntos.len() - 1, pr);
                if let Some(o) = opciones {
                    o.streamline = crate::tinta::STREAMLINE_LAPIZ;
                }
            }
            presiones.push(pr);
        }
        None => {
            if let Some(&ultima) = presiones.last() {
                presiones.push(ultima);
            }
        }
    }
}

impl Gesto {
    /// **Pega un texto del portapapeles** (`texto_pegado`).
    ///
    /// - Escribiendo un texto: entra en ESE texto, donde esta el cursor, como
    ///   si se hubiera tecleado (no nace otro).
    /// - Si no: nace un texto nuevo con la letra, el tamano, el color y la
    ///   alineacion «actuales» del panel (los mismos con que nace el de la
    ///   herramienta Texto), centrado en `centro`, partido en renglones que
    ///   no pasen de `ancho_maximo`, elegido y en UN paso de deshacer.
    ///
    /// Devuelve el id del texto, o `None` si no habia nada que pegar.
    pub fn pegar_texto(
        &mut self,
        escena: &mut Escena,
        crudo: &str,
        centro: Punto2,
        ancho_maximo: f32,
    ) -> Option<u64> {
        if let Some((id, edicion)) = self.escribiendo.as_mut() {
            // Dentro de un texto no se recorta: el espacio de delante de lo
            // pegado lo puso el usuario.
            let texto = crate::texto_pegado::preparar_para_insertar(crudo)?;
            edicion.insertar_texto(&texto);
            let id = *id;
            let ahora = edicion.texto().to_string();
            Self::volcar_texto(escena, id, &ahora);
            return Some(id);
        }
        let texto = crate::texto_pegado::preparar(crudo)?;
        // Nace como lo haria con la herramienta Texto en la mano, tenga la
        // que tenga: el mismo `nuevo_elemento`, nada copiado.
        let antes = std::mem::replace(&mut self.herramienta, Herramienta::Texto);
        let mut e = self.nuevo_elemento(centro);
        self.herramienta = antes;
        let tam = self.estilo.tamano_letra;
        let familia = self
            .estilo
            .familia
            .map_or(crate::texto::FAMILIA_POR_DEFECTO, |n| {
                crate::texto::nombre_de_familia(Some(n))
            })
            .to_string();
        let estilo = crate::texto::EstiloDeTexto {
            negrita: e.extras.negrita,
            cursiva: e.extras.cursiva,
            tachado: false,
        };
        let mide = |r: &str| crate::texto::ancho_real(r, tam, &familia, estilo);
        let texto = crate::texto_pegado::ajustar(&texto, ancho_maximo, &mide);
        let (ancho, alto) = crate::texto::medida(&texto, tam, &familia, estilo);
        e.figura = Figura::Texto {
            texto,
            tam,
            familia,
        };
        e.ancho = ancho;
        e.alto = alto;
        // Centrado donde se pega (`x - width / 2` de Excalidraw).
        e.x = centro.x - ancho / 2.0;
        e.y = centro.y - alto / 2.0;
        escena.abrir_paso();
        let id = escena.anadir(e);
        escena.cerrar_paso();
        self.seleccion.poner(id);
        Some(id)
    }

    /// Mete una letra en el texto que se esta escribiendo. Devuelve si algo
    /// cambio, para que la ventana sepa si repintar.
    pub fn escribir(&mut self, c: char, escena: &mut Escena) -> bool {
        let Some((id, edicion)) = self.escribiendo.as_mut() else {
            return false;
        };
        edicion.insertar(c);
        let texto = edicion.texto().to_string();
        Self::volcar_texto(escena, *id, &texto);
        true
    }

    /// Una tecla de edicion (flechas, borrar, entrar).
    pub fn tecla_de_texto(&mut self, t: crate::texto::TeclaTexto, escena: &mut Escena) -> bool {
        let Some((id, edicion)) = self.escribiendo.as_mut() else {
            return false;
        };
        if !edicion.tecla(t) {
            return false;
        }
        let texto = edicion.texto().to_string();
        Self::volcar_texto(escena, *id, &texto);
        true
    }

    /// Termina de escribir **porque se pulso en otro sitio**: el texto queda
    /// sin elegir, como en Excalidraw (su `onSubmit` solo deja elegido lo
    /// que se cerro con el teclado). Lo que se ve al salir es el texto, sin
    /// marco; el marco sale cuando se le hace clic.
    ///
    /// Un texto que quedo vacio se borra: dejarlo seria un elemento
    /// invisible que se puede elegir sin querer y que nadie sabria quitar.
    pub fn cerrar_texto(&mut self, escena: &mut Escena) -> bool {
        if self.terminar_texto(escena).is_none() {
            return false;
        }
        self.seleccion.limpiar();
        true
    }

    /// Termina de escribir **con el teclado** (Escape): el texto se queda
    /// elegido, con su marco ya ajustado a lo escrito, que es lo que hace
    /// Excalidraw cuando se sale con el teclado (`viaKeyboard`). Si quedo
    /// vacio se borra y no queda nada elegido.
    pub fn cerrar_texto_con_teclado(&mut self, escena: &mut Escena) -> bool {
        let Some(id) = self.terminar_texto(escena) else {
            return false;
        };
        let sigue = escena.buscar(id).is_some_and(|e| !e.borrado);
        if sigue {
            self.seleccion.poner(id);
        } else {
            self.seleccion.limpiar();
        }
        true
    }

    /// Lo comun a las dos maneras de cerrar: soltar la edicion y borrar el
    /// texto si quedo vacio. Devuelve el id del texto que se escribia.
    fn terminar_texto(&mut self, escena: &mut Escena) -> Option<u64> {
        let (id, edicion) = self.escribiendo.take()?;
        let antes = self.antes_de_escribir.take().filter(|a| a.id == id);
        if edicion.texto().trim().is_empty() {
            // Vacio: se borra, y deshacer lo devuelve como estaba.
            if let Some(a) = antes {
                Self::volver_a(escena, id, a);
            }
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            if let Some(e) = escena.buscar_mut(id) {
                e.borrado = true;
                e.tocar();
            }
            escena.cerrar_paso();
            return Some(id);
        }
        Self::componer_texto(escena, id, edicion.texto());
        // UN paso de deshacer con todo lo escrito en un texto que ya existia.
        if let Some(a) = antes
            && escena
                .buscar(id)
                .is_some_and(|e| e.figura != a.figura || e.extras.rico != a.extras.rico)
        {
            let ahora = escena.buscar(id).cloned();
            if let (Some(ahora), Some(e)) = (ahora, escena.buscar_mut(id)) {
                *e = a;
                escena.abrir_paso();
                escena.apuntar_edicion(id);
                if let Some(e) = escena.buscar_mut(id) {
                    *e = ahora;
                    e.tocar();
                }
                escena.cerrar_paso();
            }
        }
        Some(id)
    }

    /// Devuelve un texto a como estaba, sin paso de deshacer (es el estado
    /// del que parte el paso que viene).
    fn volver_a(escena: &mut Escena, id: u64, antes: Elemento) {
        if let Some(e) = escena.buscar_mut(id) {
            *e = antes;
        }
    }

    /// **Al salir de la caja se compone**: si lo escrito lleva formulas o
    /// marcas (`texto_rico::tiene_marcas`), la fuente queda en
    /// `Extras::rico`, el texto de siempre pasa a ser su version plana y la
    /// caja se ajusta a lo compuesto. Si no, es un texto de los de siempre.
    fn componer_texto(escena: &mut Escena, id: u64, fuente: &str) {
        if !crate::texto_rico::tiene_marcas(fuente) {
            return;
        }
        let Some(e) = escena.buscar_mut(id) else {
            return;
        };
        let estilo = crate::texto::EstiloDeTexto {
            negrita: e.extras.negrita,
            cursiva: e.extras.cursiva,
            tachado: e.extras.tachado,
        };
        let Figura::Texto {
            texto,
            tam,
            familia,
        } = &mut e.figura
        else {
            return;
        };
        *texto = crate::texto_rico::plano(fuente);
        let (ancho, alto) = crate::texto_rico::medida(fuente, *tam, familia, estilo);
        e.extras.rico = Some(fuente.to_string());
        if e.extras.contenedor.is_none() {
            e.ancho = ancho;
        }
        e.alto = alto;
        e.tocar();
    }

    /// **Un atajo de texto mientras se escribe** (`texto::atajo_de_tecla`):
    /// Ctrl+Z/Y dentro del texto, elegir con Mayus, negrita, color del tramo
    /// elegido, formula. Devuelve si cambio algo.
    pub fn atajo_de_texto(&mut self, a: crate::texto::AtajoTexto, escena: &mut Escena) -> bool {
        let Some((id, edicion)) = self.escribiendo.as_mut() else {
            return false;
        };
        if !edicion.atajo(a) {
            return false;
        }
        let texto = edicion.texto().to_string();
        Self::volcar_texto(escena, *id, &texto);
        true
    }

    /// **Lo elegido del texto que se escribe**, pintado como un fondo
    /// translucido renglon a renglon. Vacio si no hay nada elegido.
    pub fn seleccion_de_texto(&self, escena: &Escena) -> Vec<crate::pintado::Orden> {
        let Some((id, edicion)) = self.escribiendo.as_ref() else {
            return Vec::new();
        };
        let Some((a, b)) = edicion.seleccion() else {
            return Vec::new();
        };
        let Some(e) = escena.buscar(*id) else {
            return Vec::new();
        };
        let Figura::Texto { tam, familia, .. } = &e.figura else {
            return Vec::new();
        };
        let estilo = crate::texto::EstiloDeTexto {
            negrita: e.extras.negrita,
            cursiva: e.extras.cursiva,
            tachado: false,
        };
        let paso =
            tam * crate::texto::interlineado_de(familia).unwrap_or(crate::texto::INTERLINEADO);
        let renglones: Vec<&str> = edicion.texto().split('\n').collect();
        let apartados = e
            .extras
            .alineacion
            .filter(|a| *a != crate::texto::AlineacionTexto::Izquierda)
            .map(|al| {
                crate::texto::apartados_de_renglones(
                    edicion.texto(),
                    *tam,
                    familia,
                    estilo,
                    e.ancho,
                    al,
                )
            })
            .unwrap_or_default();
        let (fa, fb) = (edicion.fila_de(a), edicion.fila_de(b));
        let mut v = Vec::new();
        let mut inicio = 0usize;
        for (fila, r) in renglones.iter().enumerate() {
            let largo = r.chars().count();
            if (fa..=fb).contains(&fila) {
                let desde = if fila == fa { a - inicio } else { 0 };
                let hasta = if fila == fb { b - inicio } else { largo };
                let medir = |n: usize| {
                    let s: String = r.chars().take(n).collect();
                    crate::texto::ancho_real(&s, *tam, familia, estilo)
                };
                let dx = e.x + apartados.get(fila).copied().unwrap_or(0.0);
                let (x0, x1) = (
                    dx + medir(desde),
                    dx + medir(hasta).max(medir(desde) + tam * 0.25),
                );
                let y0 = e.y + fila as f32 * paso;
                v.push(crate::pintado::Orden::Relleno {
                    puntos: vec![
                        crate::vector::Punto2::nuevo(x0, y0),
                        crate::vector::Punto2::nuevo(x1, y0),
                        crate::vector::Punto2::nuevo(x1, y0 + paso),
                        crate::vector::Punto2::nuevo(x0, y0 + paso),
                    ],
                    color: crate::ColorRgba {
                        r: 0.26,
                        g: 0.52,
                        b: 0.96,
                        a: 0.3,
                    },
                });
            }
            inicio += largo + 1;
        }
        v
    }

    /// Si se esta escribiendo ahora mismo. La ventana lo mira para saber si
    /// una tecla es texto o un atajo de herramienta.
    pub fn esta_escribiendo(&self) -> bool {
        self.escribiendo.is_some()
    }

    /// Si hay que pintar el marco y los tiradores de lo elegido. **No
    /// mientras se escribe**: el texto que se escribe esta elegido (para que
    /// el panel le cambie la letra), pero lo que se ve es solo el cursor,
    /// como en Excalidraw.
    pub fn marco_visible(&self) -> bool {
        self.escribiendo.is_none()
    }

    /// Lo escrito y donde va el cursor, para pintarlo.
    pub fn texto_en_curso(&self) -> Option<(u64, &crate::texto::EdicionTexto)> {
        self.escribiendo.as_ref().map(|(id, e)| (*id, e))
    }

    /// **La barra del cursor del texto que se escribe**, como una raya del
    /// mundo: de arriba abajo de su renglon, detras de lo que va delante de
    /// el en ese renglon, medido con la misma letra con que se pinta (el
    /// medidor del anfitrion). `None` si no se esta escribiendo.
    ///
    /// Fija y no parpadeante a proposito: parpadear pide repintar el lienzo
    /// dos veces por segundo mientras se escribe, y en el suelo (HD 4000)
    /// cada repintado del lienzo entero cuesta lo que cuesta.
    pub fn cursor_de_texto(&self, escena: &Escena, zoom: f32) -> Option<crate::pintado::Orden> {
        let (id, edicion) = self.escribiendo.as_ref()?;
        let e = escena.buscar(*id)?;
        let Figura::Texto { tam, familia, .. } = &e.figura else {
            return None;
        };
        let estilo = crate::texto::EstiloDeTexto {
            negrita: e.extras.negrita,
            cursiva: e.extras.cursiva,
            tachado: false,
        };
        let paso =
            tam * crate::texto::interlineado_de(familia).unwrap_or(crate::texto::INTERLINEADO);
        let fila = edicion.fila();
        // Centrado o a la derecha, el renglon empieza donde lo aparta
        // `pintado` (la misma cuenta).
        let apartado = e
            .extras
            .alineacion
            .filter(|a| *a != crate::texto::AlineacionTexto::Izquierda)
            .and_then(|a| {
                crate::texto::apartados_de_renglones(
                    edicion.texto(),
                    *tam,
                    familia,
                    estilo,
                    e.ancho,
                    a,
                )
                .get(fila)
                .copied()
            })
            .unwrap_or(0.0);
        let x = e.x + apartado + crate::texto::ancho_real(edicion.prefijo(), *tam, familia, estilo);
        let y0 = e.y + fila as f32 * paso;
        // Un pixel y medio de pantalla, o un dieciseisavo de la letra si es
        // mas: a mucho zoom una raya de pixel se pierde junto a letras
        // gordas.
        let grosor = (1.5 / zoom.max(1e-3)).max(tam / 16.0);
        Some(crate::pintado::Orden::Polilinea {
            puntos: vec![
                crate::vector::Punto2::nuevo(x, y0 + paso * 0.1),
                crate::vector::Punto2::nuevo(x, y0 + paso * 0.9),
            ],
            color: crate::ColorRgba {
                a: e.trazo.a * e.opacidad.clamp(0.0, 1.0),
                ..e.trazo
            },
            grosor,
            estilo: crate::EstiloTrazo::Solido,
        })
    }

    /// Escribe el texto en su elemento y le ajusta la caja a lo escrito
    /// (`texto::medida`, con la letra del texto: la caja de Excalidraw con
    /// `autoResize`). Si nada cambia, no toca la version: abrir un texto y
    /// salir sin escribir no deja el dibujo por guardar.
    ///
    /// Todo dentro de UN paso de deshacer por pulsacion no vale: serian
    /// treinta pasos para una palabra. Se apunta la edicion una vez, al
    /// abrirse el texto, y aqui solo se cambia el contenido.
    fn volcar_texto(escena: &mut Escena, id: u64, texto: &str) {
        let Some(e) = escena.buscar_mut(id) else {
            return;
        };
        let estilo = crate::texto::EstiloDeTexto {
            negrita: e.extras.negrita,
            cursiva: e.extras.cursiva,
            tachado: false,
        };
        let Figura::Texto {
            texto: dentro,
            tam,
            familia,
        } = &mut e.figura
        else {
            return;
        };
        // Mientras se escribe se ve la fuente tal cual (con sus `$...$`): se
        // compone al salir (`terminar_texto`).
        let cambia_texto = dentro != texto || e.extras.rico.is_some();
        e.extras.rico = None;
        if cambia_texto {
            dentro.clear();
            dentro.push_str(texto);
        }
        let (ancho, alto) = crate::texto::medida(texto, *tam, familia, estilo);
        let cambia_caja = (e.ancho - ancho).abs() > 0.01 || (e.alto - alto).abs() > 0.01;
        if cambia_caja {
            e.ancho = ancho;
            e.alto = alto;
        }
        if cambia_texto || cambia_caja {
            e.tocar();
        }
    }

    /// **Ajusta la caja de un texto suelto a lo que mide de verdad**, sin
    /// tocar lo escrito: para los textos que llegan con la caja medida a ojo
    /// (los de antes de medir con DirectWrite) o con la letra de otro
    /// aparato. Solo con medidor de verdad —a ojo no se «corrige» nada— y
    /// solo en los sueltos: el de dentro de una figura lo coloca su figura.
    pub fn ajustar_caja_de_texto(escena: &mut Escena, id: u64) {
        if !crate::texto::hay_medidor() {
            return;
        }
        let suelto = escena.buscar(id).is_some_and(|e| {
            e.extras.contenedor.is_none() && matches!(e.figura, Figura::Texto { .. })
        });
        if !suelto {
            return;
        }
        // Uno rico se mide compuesto: volcarlo como texto plano le quitaria
        // la fuente.
        if let Some(e) = escena.buscar_mut(id)
            && let Some((ancho, alto)) = crate::texto_rico::medida_del_elemento(e)
        {
            if (e.ancho - ancho).abs() > 0.01 || (e.alto - alto).abs() > 0.01 {
                e.ancho = ancho;
                e.alto = alto;
                e.tocar();
            }
            return;
        }
        let texto = match escena.buscar(id).map(|e| &e.figura) {
            Some(Figura::Texto { texto, .. }) => texto.clone(),
            _ => return,
        };
        Self::volcar_texto(escena, id, &texto);
    }
}

/// **Las flechas atadas** (`binding.ts` de Excalidraw): unir al dibujar,
/// resaltar la figura candidata, seguir en vivo y re-atar al soltar.
///
/// Van en su propio bloque porque son un solo asunto repartido por los tres
/// momentos del gesto; la maquina de estados de arriba solo los llama.
impl Gesto {
    /// Si lo que se esta dibujando es una flecha de las que se atan: la
    /// recta y la de codos. La de pulso no, que es un trazo con punta y no
    /// un conector.
    fn dibujando_flecha(&self) -> bool {
        matches!(self.estado, Estado::Dibujando { .. })
            && matches!(
                self.herramienta,
                Herramienta::Flecha | Herramienta::FlechaCodos
            )
    }

    /// Apunta, una vez al pulsar, las flechas que tienen que seguir a lo que
    /// se va a mover (ver `enlace::Seguidoras`), y guarda su instantanea de
    /// deshacer ya: el arrastre que sigue no pide memoria por ellas.
    fn preparar_seguidoras(&mut self, escena: &mut Escena) {
        // Un marco se lleva lo que encierra: lo de dentro tambien se mueve,
        // y una flecha que va dentro con su caja no tiene que estirarse.
        let hay_marco = escena
            .elementos
            .iter()
            .any(|e| self.seleccion.contiene(e.id) && crate::marco::es_marco(e));
        //
        // **Lo de dentro se apunta aqui, una vez**, y no en cada aviso: el
        // arrastre lo mueve y la ventana lo pinta con lo elegido (fuera de la
        // capa congelada). Antes se recalculaba en cada movimiento del raton
        // y ademas iba recogiendo lo que el marco pisaba por el camino; y la
        // ventana, que no lo sabia, lo dejaba quieto en la capa congelada
        // hasta soltar (28-sep-2026: «al poner los frames pasa lo mismo»).
        self.del_marco.clear();
        if hay_marco {
            let seleccion = &self.seleccion;
            self.del_marco.extend(
                crate::marco::con_contenidos(&escena.elementos, seleccion.ids())
                    .into_iter()
                    .filter(|id| !seleccion.contiene(*id)),
            );
            self.del_marco.sort_unstable();
        }
        for &id in &self.del_marco {
            escena.apuntar_edicion(id);
        }
        let seleccion = &self.seleccion;
        let del_marco = &self.del_marco;
        self.seguidoras.preparar(&escena.elementos, |id| {
            seleccion.contiene(id) || del_marco.binary_search(&id).is_ok()
        });
        for &id in self.seguidoras.ids() {
            escena.apuntar_edicion(id);
        }
    }

    /// Si `id` es de lo que un marco elegido se lleva en este arrastre (lo
    /// que encerraba al cogerlo). Solo mientras se mueve: estirar o girar un
    /// marco no mueve lo de dentro.
    pub fn lleva_el_marco(&self, id: u64) -> bool {
        matches!(self.estado, Estado::Moviendo { .. }) && self.del_marco.binary_search(&id).is_ok()
    }

    /// Todo lo que los marcos elegidos se llevan en este arrastre, ordenado.
    /// Vacio si no se esta moviendo un marco.
    pub fn lo_que_lleva_el_marco(&self) -> &[u64] {
        if matches!(self.estado, Estado::Moviendo { .. }) {
            &self.del_marco
        } else {
            &[]
        }
    }

    /// El punto de flecha que hay bajo `p`, si la seleccion es una flecha
    /// sola. El de «anadir punto» no: ese crea, y aqui solo se arrastra.
    fn agarre_de_punta(
        &self,
        p: Punto2,
        escena: &Escena,
        escala: f32,
    ) -> Option<(u64, crate::tiradores::AgarrePunta)> {
        use crate::tiradores::AgarrePunta;
        let [id] = self.seleccion.ids() else {
            return None;
        };
        let e = escena.buscar(*id)?;
        if !matches!(e.figura, Figura::Flecha { .. }) {
            return None;
        }
        match crate::tiradores::TiradoresDePunta::de_elemento(e, escala)?.en(p, escala)? {
            AgarrePunta::Anadir(_) => None,
            agarre => Some((*id, agarre)),
        }
    }

    /// Al soltar: a que queda atada cada punta que se acaba de dejar.
    fn soltar_flechas(&mut self, escena: &mut Escena, escala: f32) {
        use crate::elemento::ModoEnganche;
        use crate::enlace::{Extremo, revisar_extremo_en};
        use crate::tiradores::AgarrePunta;
        let zoom = 1.0 / escala.max(0.0001);
        // **La punta se queda donde se solto** (lo pidio el usuario el
        // 2026-09-23: «si inicie al centro de una figura ahi se sujeta, y si lo
        // solte en la esquina ahi se queda»). Es el `bindMode: "inside"` de
        // Excalidraw hecho norma: el punto fijo sigue a la figura al moverla,
        // estirarla o girarla. Alt hace lo de Excalidraw por omision: la
        // punta al borde, en orbita.
        let modo = |alt: bool| {
            if alt {
                ModoEnganche::Orbita
            } else {
                ModoEnganche::Dentro
            }
        };
        let (al_pulsar, al_soltar) = (modo(self.alt[0]), modo(self.alt[1]));
        match self.estado {
            // Recien dibujada: las dos puntas, la de salida —que se dejo al
            // pulsar, con el Alt de entonces— y la de llegada.
            Estado::Dibujando { id } if self.dibujando_flecha() => {
                revisar_extremo_en(escena, id, Extremo::Inicio, zoom, al_pulsar);
                revisar_extremo_en(escena, id, Extremo::Fin, zoom, al_soltar);
            }
            Estado::ArrastrandoPunta { id, agarre } if self.arrastrado => match agarre {
                // `mover_punta` ya solto el enganche al empezar a arrastrar,
                // asi que la figura de antes todavia la cree suya: se pone
                // al dia su `boundElements` aunque la punta no se ate a nada.
                AgarrePunta::Principio => {
                    revisar_extremo_en(escena, id, Extremo::Inicio, zoom, al_soltar);
                    crate::enlace::sincronizar_atados(escena, id);
                }
                AgarrePunta::Final => {
                    revisar_extremo_en(escena, id, Extremo::Fin, zoom, al_soltar);
                    crate::enlace::sincronizar_atados(escena, id);
                }
                // Un codo de en medio: las puntas atadas no cambian de
                // figura, pero se vuelven a posar mirando al tramo nuevo.
                _ => {
                    if let Some(mut copia) = escena.buscar(id).cloned() {
                        if crate::enlace::recolocar(&mut copia, &escena.elementos) {
                            if let Some(f) = escena.buscar_mut(id) {
                                *f = copia;
                            }
                        }
                    }
                }
            },
            // Una flecha movida SIN su figura: si su punta sigue encima,
            // sigue atada; si se la ha llevado lejos, se suelta. Sin esto, la
            // proxima vez que se moviera la caja la punta volveria sola a
            // ella, deshaciendo lo que el usuario acaba de hacer.
            // La misma revision que tras alinear o repartir: vive en
            // `enlace` para que las dos no discrepen. Cada punta conserva su
            // modo: mover la flecha no es pedir otro.
            Estado::Moviendo { .. } | Estado::Escalando { .. } | Estado::Girando { .. }
                if self.arrastrado =>
            {
                let seleccion = &self.seleccion;
                crate::enlace::revisar_flechas_movidas(escena, |id| seleccion.contiene(id), zoom);
            }
            _ => {}
        }
        self.seguidoras.limpiar();
        self.candidatas = [None; 2];
    }

    /// Si esta flecha se esta re-trazando en vivo porque se mueve algo a lo
    /// que esta atada. La ventana la saca de la capa congelada: si no, se
    /// veria quieta en su sitio viejo debajo de la que se mueve.
    pub fn sigue_la_flecha(&self, id: u64) -> bool {
        self.seguidoras.contiene(id)
    }

    /// Las flechas que se re-trazan en vivo en este arrastre, ordenadas.
    pub fn flechas_que_siguen(&self) -> &[u64] {
        self.seguidoras.ids()
    }

    /// El resaltado de las figuras a las que se atarian las puntas de la
    /// flecha en curso, listo para pintar encima de todo.
    pub fn resaltado_de_union(&self, escena: &Escena, zoom: f32) -> Vec<crate::pintado::Orden> {
        let [a, b] = self.candidatas;
        [a, b.filter(|b| Some(*b) != a)]
            .into_iter()
            .flatten()
            .filter_map(|id| escena.buscar(id))
            .flat_map(|e| crate::enlace::resaltado(e, zoom))
            .collect()
    }

    /// Los puntos de agarre de la flecha elegida, si lo elegido es una
    /// flecha sola y no se esta arrastrando entera. Son los mismos que mira
    /// `pulsar`, asi que lo que se pinta es lo que se puede coger.
    pub fn tiradores_de_punta(
        &self,
        escena: &Escena,
        escala: f32,
    ) -> Option<crate::tiradores::TiradoresDePunta> {
        if self.moviendo() {
            return None;
        }
        let [id] = self.seleccion.ids() else {
            return None;
        };
        let e = escena.buscar(*id)?;
        if !matches!(e.figura, Figura::Flecha { .. }) {
            return None;
        }
        crate::tiradores::TiradoresDePunta::de_elemento(e, escala)
    }
}
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use pixpin_geom::Tirador;

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
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
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
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

    fn pulsar(p: Punto2) -> EventoGesto {
        EventoGesto::Pulsar {
            p,
            shift: false,
            alt: false,
            presion: None,
        }
    }
    fn mover(p: Punto2) -> EventoGesto {
        EventoGesto::Mover {
            p,
            shift: false,
            alt: false,
            presion: None,
        }
    }

    #[test]
    fn con_el_lapiz_pulsar_encima_de_un_trazo_dibuja_y_no_lo_mueve() {
        // El fallo que reporto el usuario: dibujaba encima de lo que acababa
        // de trazar y se lo llevaba por delante en vez de seguir dibujando.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let antes = escena.buscar(id).unwrap().clone();

        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;
        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(80.0, 80.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(80.0, 80.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(
            escena.buscar(id).unwrap().x,
            antes.x,
            "el rectangulo se movio"
        );
        assert_eq!(escena.elementos.len(), 2, "y el trazo nuevo no se hizo");
        assert_eq!(g.seleccion.cuantos(), 0, "ni se eligio nada");
    }

    /// Igual que `pulsar`/`mover`, pero llevando la presion del lapiz.
    fn pulsar_con_gesto(g: &mut Gesto, e: &mut Escena, x: f32, y: f32, presion: Option<f32>) {
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(x, y),
                shift: false,
                alt: false,
                presion,
            },
            e,
            1.0,
        );
    }
    fn mover_con_gesto(g: &mut Gesto, e: &mut Escena, x: f32, y: f32, presion: Option<f32>) {
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(x, y),
                shift: false,
                alt: false,
                presion,
            },
            e,
            1.0,
        );
    }
    fn lapiz_de(
        e: &Escena,
    ) -> (
        Vec<Punto2>,
        Vec<f32>,
        Option<crate::tinta::OpcionesTinta>,
        f32,
    ) {
        let el = e.visibles().last().expect("hay un trazo");
        let Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } = &el.figura
        else {
            panic!("no es lapiz")
        };
        (puntos.clone(), presiones.clone(), *opciones, el.grosor)
    }

    #[test]
    fn con_raton_el_trazo_nace_con_la_pluma_elegida_y_sin_presiones() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        g.variabilidad = crate::tinta::Variabilidad::Constante;
        g.grosor_tinta = crate::tinta::GROSOR_GRUESO;
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, None);
        let (_, presiones, opciones, grosor) = lapiz_de(&e);
        assert!(presiones.is_empty());
        assert_eq!(grosor, crate::tinta::GROSOR_GRUESO);
        let o = opciones.expect("un trazo nuevo nunca es legado");
        assert_eq!(o.variabilidad, crate::tinta::Variabilidad::Constante);
        assert_eq!(o.streamline, crate::tinta::STREAMLINE_RATON_NUEVO);
    }

    #[test]
    fn la_primera_presion_real_rellena_hacia_atras_y_afina_el_suavizado() {
        // El boton lo pulsa un mensaje de raton sin presion; la presion
        // llega con la primera muestra del lapiz.
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, Some(0.7));
        mover_con_gesto(&mut g, &mut e, 9.0, 0.0, Some(0.9));
        let (puntos, presiones, opciones, _) = lapiz_de(&e);
        assert_eq!(puntos.len(), presiones.len());
        assert_eq!(presiones, vec![0.7, 0.7, 0.9]);
        assert_eq!(opciones.unwrap().streamline, crate::tinta::STREAMLINE_LAPIZ);
    }

    /// El hallazgo critico de la revision de la Tarea 9: `Region::Caja`
    /// solo cubria el tramo anterior->p con margen `grosor/2`, pero el
    /// contorno de `freehand` es mucho mas ancho que eso (`grosor *
    /// FACTOR_VARIABLE`) y el streamline mueve puntos YA pintados varias
    /// muestras atras -sin contar que el remate del trazo se recalcula
    /// entero en cada punto nuevo-. Con la caja vieja, dibujar a mano
    /// dejaba estela en pantalla.
    ///
    /// La prueba compara el contorno con diecinueve puntos contra el de
    /// veinte y comprueba que CUALQUIER punto que no tenga pareja casi
    /// exacta en el otro contorno -es decir, cualquier punto que de verdad
    /// cambio de sitio entre un fotograma y el siguiente- cae dentro de la
    /// `Region::Caja` que devuelve el ultimo `mover`.
    ///
    /// No se uso la receta literal de la revision ("los ultimos ~40 puntos
    /// del contorno viejo son su remate"): en `tinta::freehand::contorno`,
    /// el orden real del `Vec` que se devuelve es `izquierda ++ tapa_fin ++
    /// derecha.rev() ++ tapa_inicio` (ver `freehand.rs`), asi que los
    /// ULTIMOS puntos del array son `tapa_inicio` -la tapa del PRINCIPIO
    /// del trazo, junto al primer clic-, no el remate del extremo que
    /// crece. Esos puntos no cambian de un fotograma a otro (dependen solo
    /// de los primeros puntos en bruto, que no se tocan al anadir uno
    /// nuevo al final), así que exigir que caigan en la caja sucia habria
    /// forzado una caja mas grande de lo necesario sin detectar ningun
    /// pixel que de verdad cambie. La comparacion punto a punto contra el
    /// OTRO contorno mide lo que realmente hay que repintar, sin asumir en
    /// que posicion del array cae el remate.
    #[test]
    fn la_region_sucia_de_dibujar_a_mano_cubre_todo_lo_que_cambia_en_el_contorno() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        g.grosor_tinta = crate::tinta::GROSOR_GRUESO;

        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        // 18 movimientos mas: 19 puntos en total antes del ultimo, un
        // zigzag para que el contorno de verdad cambie de forma con cada
        // punto nuevo (una recta apenas mueve nada al alargarse).
        for i in 1..19 {
            let x = i as f32 * 3.0;
            let y = if i % 2 == 0 { 0.0 } else { 6.0 };
            mover_con_gesto(&mut g, &mut e, x, y, None);
        }
        let (puntos_antes, presiones_antes, opciones, grosor) = lapiz_de(&e);
        assert_eq!(puntos_antes.len(), 19);

        // El punto 20, con su Respuesta de verdad (la que se comprueba).
        let r = g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(19.0 * 3.0, 6.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut e,
            1.0,
        );
        let Region::Caja(cx0, cy0, cx1, cy1) = r.region else {
            panic!("dibujar a mano tiene que ensuciar una caja")
        };

        let (puntos_despues, presiones_despues, _, _) = lapiz_de(&e);
        assert_eq!(puntos_despues.len(), 20);

        let contorno_antes =
            crate::tinta::contorno_de_lapiz(&puntos_antes, &presiones_antes, grosor, opciones);
        let contorno_despues =
            crate::tinta::contorno_de_lapiz(&puntos_despues, &presiones_despues, grosor, opciones);

        let dentro = |x: f32, y: f32| x >= cx0 && x <= cx1 && y >= cy0 && y <= cy1;
        // Casi exacta: la misma entrada por el mismo camino de codigo da el
        // mismo `f64` bit a bit, pero un margen pequeno cubre por si algun
        // punto se reordena sin desplazarse de verdad.
        let tiene_pareja = |p: Punto2, otro: &[Punto2]| {
            otro.iter()
                .any(|q| (p.x - q.x).abs() < 0.01 && (p.y - q.y).abs() < 0.01)
        };

        // Todo punto que aparece en un contorno y no en el otro es, por
        // definicion, un pixel que cambio entre los dos fotogramas.
        let mut algo_cambio = false;
        for p in &contorno_despues {
            if !tiene_pareja(*p, &contorno_antes) {
                algo_cambio = true;
                assert!(
                    dentro(p.x, p.y),
                    "el contorno nuevo en ({}, {}) no estaba antes y cae fuera de la region sucia",
                    p.x,
                    p.y
                );
            }
        }
        for p in &contorno_antes {
            if !tiene_pareja(*p, &contorno_despues) {
                algo_cambio = true;
                assert!(
                    dentro(p.x, p.y),
                    "el contorno viejo en ({}, {}) desaparece y cae fuera de la region sucia",
                    p.x,
                    p.y
                );
            }
        }
        assert!(
            algo_cambio,
            "el trazo de prueba tiene que hacer cambiar el contorno, o esto no mide nada"
        );
    }

    /// La regresion de la ronda 2: `self.trazo` deja de crecer a partir de
    /// `PUNTOS_RESERVADOS` (512 puntos) para no asignar, pero la caja ancha
    /// se calculaba sobre `self.trazo` -asi que un trazo mas largo que eso
    /// se quedaba congelado en el tramo 507-512 y la Region::Caja nunca
    /// volvia a contener el punto nuevo `p`. Con la capa congelada valida,
    /// esa tinta quedaba invisible hasta el siguiente present completo (y
    /// con la Tarea 7 recuperando cada punto fusionado, 512 movimientos son
    /// entre medio segundo y varios segundos de trazo real).
    ///
    /// Esta prueba dibuja 600 puntos DISTINTOS (bien separados: nada de
    /// duplicados que el propio `puntos` de la figura deduplique) y
    /// comprueba que la `Region::Caja` del ultimo movimiento contiene el
    /// punto nuevo y los cinco anteriores de la FIGURA (no de `self.trazo`,
    /// que para entonces ya esta congelado).
    #[test]
    fn un_trazo_de_mas_de_quinientos_puntos_sigue_ensuciando_la_punta_que_crece() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        g.grosor_tinta = crate::tinta::GROSOR_GRUESO;

        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        // 599 movimientos mas: 600 puntos en total, todos distintos entre
        // si (avanzan siempre en x) para que ninguno se deduplique.
        for i in 1..600 {
            mover_con_gesto(&mut g, &mut e, i as f32, 0.0, None);
        }
        let (puntos, ..) = lapiz_de(&e);
        assert_eq!(
            puntos.len(),
            600,
            "ninguno de los 600 puntos era un duplicado"
        );

        // El punto 601, con su Respuesta de verdad.
        let ultimo = Punto2::nuevo(600.0, 0.0);
        let r = g.evento(
            EventoGesto::Mover {
                p: ultimo,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut e,
            1.0,
        );
        let Region::Caja(cx0, cy0, cx1, cy1) = r.region else {
            panic!("dibujar a mano tiene que ensuciar una caja")
        };
        let dentro = |x: f32, y: f32| x >= cx0 && x <= cx1 && y >= cy0 && y <= cy1;

        assert!(
            dentro(ultimo.x, ultimo.y),
            "el punto nuevo ({}, {}) tiene que estar dentro de la region sucia",
            ultimo.x,
            ultimo.y
        );

        let (puntos_despues, ..) = lapiz_de(&e);
        assert_eq!(puntos_despues.len(), 601);
        for p in puntos_despues.iter().rev().take(6) {
            assert!(
                dentro(p.x, p.y),
                "el punto de la figura ({}, {}) -de la cola reciente- tiene que \
                 estar dentro de la region sucia",
                p.x,
                p.y
            );
        }
    }

    #[test]
    fn un_punto_repetido_exacto_no_se_guarda() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, None);
        assert_eq!(lapiz_de(&e).0.len(), 2);
    }

    #[test]
    fn trazo_en_curso_solo_existe_mientras_se_dibuja_a_mano() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        assert_eq!(g.trazo_en_curso(), None);
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        assert!(g.trazo_en_curso().is_some());
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(1.0, 0.0),
            },
            &mut e,
            1.0,
        );
        assert_eq!(g.trazo_en_curso(), None);
    }

    /// Un arrastre entero: pulsar, mover y soltar.
    fn arrastrar(g: &mut Gesto, e: &mut Escena, de: Punto2, a: Punto2) {
        g.evento(pulsar(de), e, 1.0);
        g.evento(mover(a), e, 1.0);
        g.evento(EventoGesto::Soltar { p: a }, e, 1.0);
    }

    #[test]
    fn un_arrastre_entero_deja_exactamente_un_paso_de_deshacer() {
        // El invariante que hace util el historial de la tarea 1.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(50.0, 50.0),
            Punto2::nuevo(150.0, 50.0),
        );
        assert_eq!(escena.buscar(id).unwrap().x, 100.0, "se movio");

        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "de una vez");
    }

    #[test]
    fn escape_a_mitad_de_un_arrastre_lo_cancela() {
        // Sale gratis: el paso ya guarda el estado anterior, asi que
        // cancelar es aplicarlo y tirar el paso.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(500.0, 500.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Escape, &mut escena, 1.0);

        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "volvio a su sitio");
        assert!(g.en_reposo());
    }

    #[test]
    fn un_tirador_manda_sobre_el_elemento_que_haya_debajo() {
        // Sin esta prioridad, un tirador encima de otro elemento es
        // inalcanzable: el clic cae en el de debajo.
        let mut escena = Escena::nueva();
        let grande = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(grande);

        // La esquina sureste del grande, que cae DENTRO del propio grande.
        let ts = Tiradores::de_elemento(escena.buscar(grande).unwrap(), 1.0);
        let se = ts
            .tamano
            .iter()
            .find(|(c, _)| *c == Tirador::SuresteEsquina)
            .unwrap()
            .1;

        g.evento(pulsar(se), &mut escena, 1.0);
        g.evento(
            mover(Punto2::nuevo(se.x + 100.0, se.y + 100.0)),
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(se.x + 100.0, se.y + 100.0),
            },
            &mut escena,
            1.0,
        );

        let e = escena.buscar(grande).unwrap();
        assert!(e.ancho > 250.0, "escalo en vez de moverse: {}", e.ancho);
        assert_eq!(e.x, 0.0, "y no se movio");
    }

    #[test]
    fn lo_ya_seleccionado_manda_sobre_lo_que_haya_encima() {
        // Sin esta regla, mover un grupo se convierte en seleccionar por
        // accidente lo que estaba encima.
        let mut escena = Escena::nueva();
        let abajo = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let encima = escena.anadir(rect(50.0, 50.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(abajo);

        // Pulsar donde estan los dos: gana el ya elegido.
        //
        // El destino del arrastre se aleja hasta 175 -y no los 85
        // originales- porque ahora `Estado::Moviendo` es `Faena::Moviendo` a
        // ojos del iman: (85,75) cae a 15 del lado derecho de "encima" y a
        // 10 de su centro, dentro del radio de 21 (14 x 1,5) que usa
        // `Faena::Moviendo`. El punto se pegaba al propio "encima" -que no
        // esta en `excluir` porque no es el seleccionado- y el arrastre se
        // quedaba en delta cero. A 175 no hay ningun ancla de "encima" a
        // menos de 21.
        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(75.0, 75.0),
            Punto2::nuevo(175.0, 75.0),
        );

        assert_eq!(g.seleccion.ids(), &[abajo], "sigue el de antes");
        assert_eq!(
            escena.buscar(abajo).unwrap().x,
            100.0,
            "se movio el de antes"
        );
        assert_eq!(
            escena.buscar(encima).unwrap().x,
            50.0,
            "el de encima, quieto"
        );
    }

    #[test]
    fn arrastrar_en_vacio_con_la_mano_hace_marquesina() {
        let mut escena = Escena::nueva();
        let dentro = escena.anadir(rect(20.0, 20.0, 30.0, 30.0));
        let fuera = escena.anadir(rect(500.0, 500.0, 30.0, 30.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert!(g.marquesina().is_some(), "hay marquesina en curso");
        g.evento(mover(Punto2::nuevo(200.0, 200.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(200.0, 200.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(g.seleccion.ids(), &[dentro]);
        assert!(!g.seleccion.contiene(fuera));
        assert!(g.marquesina().is_none(), "y se acabo al soltar");
    }

    #[test]
    fn shift_mas_clic_anade_a_la_seleccion() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 50.0, 50.0));
        let b = escena.anadir(rect(100.0, 0.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(25.0, 25.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(25.0, 25.0),
            },
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(125.0, 25.0),
                shift: true,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(125.0, 25.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(g.seleccion.ids(), &[a, b]);
    }

    #[test]
    fn dibujar_un_trazo_lo_anade_y_lo_deja_deshacible() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..20 {
            g.evento(mover(Punto2::nuevo(i as f32 * 5.0, 0.0)), &mut escena, 1.0);
        }
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(95.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0, "un trazo, un Ctrl+Z");
    }

    #[test]
    fn dibujando_se_ensucia_el_tramo_y_no_la_pantalla() {
        // En 1080p es la diferencia entre dos millones de pixeles por
        // fotograma y unos cientos.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        let r = g.evento(mover(Punto2::nuevo(10.0, 0.0)), &mut escena, 1.0);

        let Region::Caja(x0, y0, x1, y1) = r.region else {
            panic!("tiene que ser una caja, es {:?}", r.region);
        };
        assert!(x1 - x0 < 40.0 && y1 - y0 < 40.0, "el tramo, no la pantalla");
    }

    #[test]
    fn dibujar_no_asigna_memoria_por_cada_aviso_del_raton() {
        // El buffer del trazo en curso se reserva de una vez y se reutiliza
        // entre trazos con clear(). La prueba de verdad —contar
        // asignaciones— es la tarea 15; aqui se comprueba lo que se puede
        // observar desde dentro.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..400 {
            g.evento(mover(Punto2::nuevo(i as f32, 0.0)), &mut escena, 1.0);
        }
        assert!(
            g.capacidad_del_trazo() >= PUNTOS_RESERVADOS,
            "se reservo de una vez"
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(400.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        let tras_el_primero = g.capacidad_del_trazo();
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert_eq!(
            g.capacidad_del_trazo(),
            tras_el_primero,
            "el segundo trazo reutiliza el buffer del primero"
        );
    }

    #[test]
    fn suprimir_borra_lo_seleccionado_de_una_vez() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        let mut g = Gesto::nuevo();
        g.seleccion.poner_todos([a, b]);

        g.evento(EventoGesto::Suprimir, &mut escena, 1.0);
        assert_eq!(escena.cuantos_visibles(), 0);

        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 2, "los dos vuelven de una vez");
    }

    #[test]
    fn seleccionar_todo_no_coge_los_borrados() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        escena.borrar(b);
        let mut g = Gesto::nuevo();

        g.evento(EventoGesto::SeleccionarTodo, &mut escena, 1.0);
        assert_eq!(g.seleccion.ids(), &[a]);
    }

    #[test]
    fn pulsar_dos_veces_sin_soltar_no_pierde_el_gesto() {
        // La ventana puede recibir un WM_LBUTTONDOWN sin su WM_LBUTTONUP si
        // el usuario suelta fuera. Perder el gesto entero por eso seria
        // peor que ignorar el segundo pulsar.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(150.0, 50.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(150.0, 50.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.buscar(id).unwrap().x, 100.0);
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "sigue siendo un paso");
    }

    #[test]
    fn un_clic_sobre_lo_ya_seleccionado_sin_arrastrar_no_deja_paso_fantasma() {
        // La instantanea de apuntar_edicion() se toma al pulsar (para que
        // el arrastre que sigue no asigne memoria), pero un clic sin
        // arrastre no cambia nada. Si ese paso entrara igual en el
        // historial, Ctrl+Z no deshaceria nada visible y habria que
        // pulsarlo dos veces para llegar al cambio de verdad.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(50.0, 50.0),
            },
            &mut escena,
            1.0,
        );

        assert!(escena.deshacer(), "queda el paso de haberlo anadido");
        assert!(
            escena.buscar(id).unwrap().borrado,
            "y es el unico paso: el clic no dejo nada de por medio"
        );
        assert!(!escena.deshacer(), "no hay paso fantasma del clic");
    }

    #[test]
    fn el_cursor_de_escalar_va_girado_con_el_elemento() {
        // En una figura a 45 grados, el tirador de la esquina ensena la
        // flecha que de verdad apunta hacia donde va a crecer.
        use std::f32::consts::FRAC_PI_4;
        let mut escena = Escena::nueva();
        let mut e = rect(0.0, 0.0, 100.0, 100.0);
        e.angulo = FRAC_PI_4;
        let id = escena.anadir(e);
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        let ts = Tiradores::de_elemento(escena.buscar(id).unwrap(), 1.0);
        let se = ts
            .tamano
            .iter()
            .find(|(c, _)| *c == Tirador::SuresteEsquina)
            .unwrap()
            .1;
        let r = g.evento(mover(se), &mut escena, 1.0);

        let FormaCursor::Escalar { tirador, angulo } = r.cursor else {
            panic!("sobre un tirador toca cursor de escalar, es {:?}", r.cursor);
        };
        assert_eq!(
            tirador,
            Tirador::SuresteEsquina,
            "cual, para saber la direccion"
        );
        assert!(
            (angulo - FRAC_PI_4).abs() < 1e-3,
            "y el angulo del elemento, para girarla: {angulo}"
        );
    }

    #[test]
    fn la_direccion_de_un_tirador_gira_con_el_elemento() {
        // Sin girar, la esquina sureste apunta a 135 grados (abajo y a la
        // derecha). Girado un cuarto de vuelta, apunta a 225.
        use std::f32::consts::{FRAC_PI_2, PI};
        let recta = direccion_del_tirador(Tirador::SuresteEsquina, 0.0);
        assert!((recta - 3.0 * PI / 4.0).abs() < 1e-3, "sin girar: {recta}");

        let girada = direccion_del_tirador(Tirador::SuresteEsquina, FRAC_PI_2);
        assert!((girada - 5.0 * PI / 4.0).abs() < 1e-3, "girada: {girada}");
    }

    #[test]
    fn escalar_pide_la_medida_al_soltar() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, 0.0)), &mut escena, 1.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        let Some(Peticion::Calibrar { largo_px }) = r.pide else {
            panic!("al soltar tiene que pedir la medida, dio {:?}", r.pide);
        };
        assert!((largo_px - 100.0).abs() < 1e-3);
    }

    #[test]
    fn shift_sujeta_la_raya_de_calibrar_a_horizontal_o_vertical() {
        // Diseno S5: «con Shift se sujeta a horizontal o vertical. Calibrar
        // sobre el borde de una pared es el caso normal, y a pulso sale
        // torcida: un grado de mas son dos centimetros de error por
        // metro». `Estado::Calibrando` recibia `shift` y no lo miraba.
        //
        // `restringir_angulo` conserva el radio y solo redondea el angulo
        // -el mismo mecanismo que `Linea`-, asi que `largo_px` (la distancia
        // al origen) no varia al sujetar: lo que cambia es HACIA DONDE cae
        // el punto. Por eso la prueba mira el ultimo punto del `trazo`
        // -campo privado, pero este modulo de pruebas es descendiente de
        // `gesto` y lo alcanza- y no `Peticion::Calibrar`.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        // Casi horizontal, pero no del todo: sin sujetar, el ultimo punto
        // del trazo seria justo este, con y = 10.0.
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(100.0, 10.0),
                shift: true,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );

        let ultimo = *g.trazo.last().expect("el trazo tiene al menos un punto");
        assert!(
            ultimo.y.abs() < 1e-2,
            "sujeto a horizontal, y tiene que caer en 0: salio {ultimo:?}"
        );
    }

    #[test]
    fn sin_shift_la_raya_de_calibrar_no_se_sujeta() {
        // Caso negativo del anterior: sin Shift el punto se queda tal cual
        // llega, torcido incluido -es justo lo que Shift existe para poder
        // evitar cuando se quiere.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, 10.0)), &mut escena, 1.0);

        let ultimo = *g.trazo.last().expect("el trazo tiene al menos un punto");
        assert!(
            (ultimo.y - 10.0).abs() < 1e-3,
            "sin Shift no se sujeta: y tenia que seguir en 10, salio {ultimo:?}"
        );
    }

    #[test]
    fn el_umbral_de_calibrar_son_dos_pixeles_de_pantalla_y_no_de_mundo() {
        // Hallazgo 9: `largo_px` viaja en unidades de MUNDO, y el umbral se
        // comparaba contra 2.0 a pulso -dos unidades de mundo, no dos
        // pixeles de pantalla. Con un zoom que aleja la camara, `escala`
        // (unidades de mundo por pixel) crece, y dos pixeles de pantalla
        // son mas de dos unidades de mundo.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        // 3 unidades de mundo: mas de dos unidades de mundo a pulso, pero
        // con un zoom al 10% (escala = 10.0 unidades de mundo por pixel)
        // son solo 0,3 pixeles de pantalla, menos que el umbral de dos.
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 10.0);
        g.evento(mover(Punto2::nuevo(3.0, 0.0)), &mut escena, 10.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(3.0, 0.0),
            },
            &mut escena,
            10.0,
        );
        assert_eq!(
            r.pide, None,
            "3 unidades de mundo con escala 10 son 0,3 px de pantalla: no llega al umbral"
        );

        // La misma distancia de mundo, con la camara a tamano natural
        // (escala = 1.0): ahora si son tres pixeles de pantalla, por
        // encima del umbral.
        let mut g2 = Gesto::nuevo();
        g2.herramienta = Herramienta::Escalar;
        g2.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g2.evento(mover(Punto2::nuevo(3.0, 0.0)), &mut escena, 1.0);
        let r2 = g2.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(3.0, 0.0),
            },
            &mut escena,
            1.0,
        );
        assert!(
            r2.pide.is_some(),
            "3 unidades de mundo con escala 1 son 3 px de pantalla: llega al umbral"
        );
    }

    #[test]
    fn la_raya_de_calibrar_no_se_queda_en_el_dibujo() {
        // Era un metro, no un dibujo. Si se quedara, cada calibrado dejaria
        // basura en el lienzo.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, 0.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.cuantos_visibles(), 0, "la raya se fue");
    }

    #[test]
    fn calibrar_pone_la_escala_en_la_escena() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        assert!(g.calibrar(&mut escena, 100.0, 3.0, "m"));
        let e = escena.escala.as_ref().expect("hay escala");
        assert!((e.unidades_por_pixel - 0.03).abs() < 1e-6);
        assert_eq!(e.unidad, "m");
    }

    #[test]
    fn calibrar_con_una_medida_imposible_no_toca_la_escena() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        assert!(
            !g.calibrar(&mut escena, 100.0, 0.0, "m"),
            "devuelve que no pudo"
        );
        assert!(escena.escala.is_none(), "y no deja nada a medias");
    }

    #[test]
    fn recalibrar_sustituye_la_escala_anterior() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.calibrar(&mut escena, 100.0, 3.0, "m");
        g.calibrar(&mut escena, 100.0, 6.0, "m");
        assert!((escena.escala.unwrap().unidades_por_pixel - 0.06).abs() < 1e-6);
    }

    #[test]
    fn calibrar_se_puede_deshacer() {
        // Calibrar mal y no poder volver atras seria tener que rehacer el
        // lienzo entero.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.calibrar(&mut escena, 100.0, 3.0, "m");
        assert!(escena.escala.is_some());
        assert!(escena.deshacer());
        assert!(escena.escala.is_none(), "vuelve a estar sin calibrar");
    }

    #[test]
    fn la_cota_deja_una_cota_y_un_paso_de_deshacer() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Cota;

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
        );

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(matches!(
            escena.visibles().next().unwrap().figura,
            Figura::Cota { .. }
        ));
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0);
    }

    #[test]
    fn la_cota_deja_los_puntos_donde_se_arrastro() {
        // Una cota con los puntos mal mide mal: "una cota no puede mentir"
        // es la propiedad que justifica el diseno de toda esta fase. No
        // basta con comprobar que hay un elemento y que es una Cota (eso ya
        // lo hace `la_cota_deja_una_cota_y_un_paso_de_deshacer`); hace falta
        // comprobar que el primer punto es donde se pulso y el segundo el
        // que sigue al cursor, tal como hace `mover` para Linea y Flecha.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Cota;

        // Un arrastre real trae varios `Mover` antes de soltar, no uno
        // solo: si el segundo punto se acumulase en vez de sustituirse (el
        // fallo que esta prueba busca), harian falta varios avisos de
        // movimiento para notarlo.
        g.evento(pulsar(Punto2::nuevo(10.0, 20.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(60.0, 20.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(80.0, 20.0)), &mut escena, 1.0);
        // `soltar` no toca la geometria: el ultimo punto es el del ultimo
        // `Mover`, asi que el arrastre real termina con uno en el mismo
        // sitio donde se suelta.
        g.evento(mover(Punto2::nuevo(110.0, 20.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(110.0, 20.0),
            },
            &mut escena,
            1.0,
        );

        let Figura::Cota { puntos } = &escena.visibles().next().unwrap().figura else {
            panic!("tiene que ser una Cota");
        };
        assert_eq!(
            puntos.len(),
            2,
            "solo dos puntos, no uno por cada aviso de movimiento"
        );
        assert_eq!(
            puntos[0],
            Punto2::nuevo(10.0, 20.0),
            "el primer punto es donde se pulso"
        );
        assert_eq!(
            puntos[1],
            Punto2::nuevo(110.0, 20.0),
            "el segundo sigue al cursor, no se acumula"
        );
    }

    #[test]
    fn la_cota_funciona_sin_haber_calibrado() {
        // D35: sin escala mide en pixeles. La herramienta no se bloquea.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Cota;
        assert!(escena.escala.is_none());

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
        );
        assert_eq!(escena.cuantos_visibles(), 1, "deja la cota igual");
    }

    #[test]
    fn la_escala_grafica_deja_una_barra() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::EscalaGrafica;

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(400.0, 24.0),
        );

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(matches!(
            escena.visibles().next().unwrap().figura,
            Figura::EscalaGrafica
        ));
    }

    #[test]
    fn las_herramientas_de_medir_dejan_rastro_menos_escalar() {
        assert!(Herramienta::Cota.deja_rastro());
        assert!(Herramienta::EscalaGrafica.deja_rastro());
        assert!(
            !Herramienta::Escalar.deja_rastro(),
            "la raya de calibrar no se guarda"
        );
    }

    #[test]
    fn los_gestos_que_no_piden_nada_no_piden_nada() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;
        let r = g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert!(r.pide.is_none());
    }

    /// Un rectangulo de 100x50 en (0,0) con el id que se pida.
    ///
    /// `Elemento` no tiene constructora: se monta con el literal entero,
    /// igual que hacen las pruebas vecinas de este fichero.
    fn rect_para_iman(id: u64) -> Elemento {
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

    #[test]
    fn trazar_una_figura_engancha_a_la_esquina_de_otra() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        // Se pulsa a 10 de escena de la esquina (100,50): fuera del margen
        // de picado de `impacto::toca` (grosor/2 + 6 = 7, asi que el clic
        // no selecciona el vecino en vez de dibujar) y dentro del radio del
        // iman (14).
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);

        let a = g.anclaje_activo.expect("tenia que enganchar");
        assert_eq!(a.punto, Punto2::nuevo(100.0, 50.0));
        assert_eq!(a.id, 7);
    }

    #[test]
    fn el_lapiz_no_engancha_aunque_pase_por_encima_de_un_vertice() {
        // La guarda de `Faena::AMano`. Un trazo que salta a un vertice en
        // mitad del recorrido no se corrige, se rompe.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        // A 10 de la esquina (100,50) y no a 4,24: dentro del margen de
        // picado de `impacto::toca` (grosor/2 + 6 = 7) el clic selecciona el
        // rectangulo y ni llega a la rama del lapiz, asi que la prueba
        // pasaria sin ejercitar `Faena::AMano` -que es todo lo que aqui
        // importa-.
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);
        assert!(
            escena.elementos.len() == 2 && g.seleccion.ids().is_empty(),
            "el clic tiene que hacer nacer un trazo, no seleccionar al vecino"
        );
        // Aislada aqui: cubre la rama `Estado::Reposo` de `faena()`, que el
        // `Mover` de abajo ya no toca -ese pasa por `Estado::Dibujando`-.
        assert!(
            g.anclaje_activo.is_none(),
            "en reposo el lapiz tampoco engancha"
        );

        // Y ahora dentro del trazo, con el cursor a 1,41 del vertice ajeno:
        // esta es la rama `Estado::Dibujando`. El elemento que nace se
        // excluye a si mismo, pero el rectangulo 7 no esta excluido, asi que
        // lo unico que puede devolver `None` aqui es `Faena::AMano`.
        g.evento(mover(Punto2::nuevo(101.0, 51.0)), &mut escena, 1.0);
        assert!(
            g.anclaje_activo.is_none(),
            "el lapiz no puede pegar tirones a mitad de trazo"
        );
    }

    #[test]
    fn la_figura_que_nace_no_se_engancha_a_si_misma() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        // Al arrastrar, la esquina del propio rectangulo que esta naciendo
        // queda justo bajo el cursor.
        g.evento(mover(Punto2::nuevo(40.0, 40.0)), &mut escena, 1.0);

        let nacido = escena.elementos.last().expect("nacio algo").id;
        assert!(
            g.anclaje_activo.is_none_or(|a| a.id != nacido),
            "se ha pegado a su propia esquina"
        );
    }

    #[test]
    fn calibrar_engancha_a_los_extremos() {
        // La fila que justifica la fase: picar los dos extremos de una pared
        // de medida conocida deja de ser punteria.
        //
        // El arrastre entero -pulsar, mover, soltar- y no un solo `pulsar`:
        // el segundo punto, el que de verdad fija la medida, viaja por
        // `mover` en `Estado::Calibrando`, que es la unica rama de `faena()`
        // que el diseno llama «la fila que justifica la fase». Y se asere
        // sobre `Respuesta.pide`, que es lo observable: `anclaje_activo` se
        // escribe antes de que la maquina de estados haga nada, asi que no
        // dice si el punto enganchado sirvio para algo.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        // El rectangulo va de (0,0) a (100,50). Se pica a 11 a la izquierda
        // de la esquina (0,0) y se suelta a 12 por encima de la (100,0): las
        // dos fuera del margen de picado de `impacto::toca` (grosor/2 + 6 =
        // 7, asi que el clic no selecciona el vecino) y dentro del radio del
        // iman (14). Y a proposito descuadradas: a pulso el largo saldria
        // 111,6, asi que 100 solo puede venir de los dos enganches.
        g.evento(pulsar(Punto2::nuevo(-11.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, -12.0)), &mut escena, 1.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, -12.0),
            },
            &mut escena,
            1.0,
        );

        // Con los dos extremos enganchados el largo es exactamente 100: la
        // distancia de (0,0) a (100,0).
        match r.pide {
            Some(Peticion::Calibrar { largo_px }) => assert!(
                (largo_px - 100.0).abs() < 0.01,
                "el iman no llevo los dos extremos al vertice: {largo_px}"
            ),
            otro => panic!("calibrar tenia que pedir la medida, y pidio {otro:?}"),
        }
        assert!(
            g.seleccion.ids().is_empty(),
            "calibrar no puede acabar seleccionando el vecino del que se engancha"
        );
    }

    #[test]
    fn con_el_iman_encendido_calibrar_sigue_calibrando_y_no_selecciona() {
        // El circulo vicioso que esto cierra: todo anclaje de esquina,
        // extremo o medio cae JUSTO sobre la geometria del elemento, asi que
        // si el punto se engancha antes de las cribas de `pulsar`, el paso 3
        // (`elemento_en`) da positivo con certeza y el clic se convierte en
        // «seleccionar y mover el vecino». Encender el iman -que es el
        // estado de fabrica- rompia justo la fila que justifica la fase.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        // (0,-12): a 12 de la esquina (0,0), fuera del margen de picado (7)
        // y dentro del radio del iman (14).
        g.evento(pulsar(Punto2::nuevo(0.0, -12.0)), &mut escena, 1.0);
        assert!(
            g.seleccion.ids().is_empty(),
            "el iman robo el clic y selecciono el rectangulo de al lado"
        );
        g.evento(mover(Punto2::nuevo(100.0, 60.0)), &mut escena, 1.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, 60.0),
            },
            &mut escena,
            1.0,
        );
        assert!(
            matches!(r.pide, Some(Peticion::Calibrar { .. })),
            "con el iman encendido calibrar deja de calibrar: {:?}",
            r.pide
        );
    }

    #[test]
    fn con_el_iman_encendido_una_figura_nace_en_la_esquina_de_otra() {
        // La otra mitad del mismo fallo: empezar un rectangulo pegado a la
        // esquina del vecino seleccionaba al vecino en vez de hacer nacer
        // nada.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        // A 10 de la esquina (100,50) por fuera: fuera del margen de picado
        // (7), dentro del radio del iman (14).
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);

        assert_eq!(
            escena.elementos.len(),
            2,
            "no nacio ningun elemento: el clic se lo comio la seleccion"
        );
        assert!(
            g.seleccion.ids().is_empty(),
            "dibujar no puede acabar seleccionando el vecino"
        );
        let nacido = escena.elementos.last().expect("nacio algo");
        assert_eq!(
            (nacido.x, nacido.y),
            (100.0, 50.0),
            "y nace pegado a la esquina del vecino, que es para lo que esta el iman"
        );
    }

    fn dibujar_a_mano(g: &mut Gesto, escena: &mut Escena, puntos: &[Punto2]) {
        g.herramienta = Herramienta::Lapiz;
        g.enganche.activo = false;
        g.evento(pulsar(puntos[0]), escena, 1.0);
        for p in &puntos[1..] {
            g.evento(mover(*p), escena, 1.0);
        }
    }

    #[test]
    fn un_circulo_a_mano_quieto_se_vuelve_elipse_y_crece_desde_su_centro() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let circulo: Vec<Punto2> = (0..=48)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / 48.0;
                Punto2::nuevo(100.0 + 50.0 * t.cos(), 100.0 + 50.0 * t.sin())
            })
            .collect();
        dibujar_a_mano(&mut g, &mut escena, &circulo);
        let cursor = *circulo.last().unwrap();
        assert!(g.convertir_en_forma(&mut escena, cursor, 1.0).is_some());
        let (id, _) = g.elemento_en_curso().unwrap();
        assert!(matches!(escena.buscar(id).unwrap().figura, Figura::Elipse));
        // Alejarse al doble del centro dobla los radios sin mover el centro.
        g.evento(mover(Punto2::nuevo(200.0, 100.0)), &mut escena, 1.0);
        let e = escena.buscar(id).unwrap();
        assert!((e.ancho - 200.0).abs() < 4.0, "ancho {}", e.ancho);
        assert!(
            ((e.x + e.ancho / 2.0) - 100.0).abs() < 2.0,
            "el centro no se mueve"
        );
    }

    #[test]
    fn una_l_quieta_se_vuelve_rectangulo_que_se_ajusta_al_tirar_y_se_deshace_de_una() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let mut l: Vec<Punto2> = (0..=20)
            .map(|i| Punto2::nuevo(0.0, 100.0 - 5.0 * i as f32))
            .collect();
        l.extend((1..=30).map(|i| Punto2::nuevo(5.0 * i as f32, 0.0)));
        dibujar_a_mano(&mut g, &mut escena, &l);
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(150.0, 0.0), 1.0)
                .is_some()
        );
        let (id, _) = g.elemento_en_curso().unwrap();
        // Tirar hacia abajo a la derecha: la esquina opuesta (abajo a la
        // izquierda, donde empezo) se queda fija.
        g.evento(mover(Punto2::nuevo(220.0, -40.0)), &mut escena, 1.0);
        let e = escena.buscar(id).unwrap().clone();
        assert!(matches!(e.figura, Figura::Rectangulo));
        assert_eq!((e.x, e.y + e.alto), (0.0, 100.0), "el ancla es el inicio");
        assert_eq!(e.ancho, 220.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(220.0, -40.0),
            },
            &mut escena,
            1.0,
        );
        assert!(escena.deshacer());
        assert!(
            escena.buscar(id).is_none_or(|e| e.borrado),
            "un Ctrl+Z lo quita entero"
        );
    }

    #[test]
    fn un_garabato_quieto_no_se_convierte_y_sin_dibujar_tampoco() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(0.0, 0.0), 1.0)
                .is_none()
        );
        let zig: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 30.0 }))
            .collect();
        dibujar_a_mano(&mut g, &mut escena, &zig);
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(195.0, 30.0), 1.0)
                .is_none()
        );
    }

    #[test]
    fn arrastrar_un_rectangulo_repinta_la_figura_entera_y_no_solo_el_cursor() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;
        g.enganche.activo = false;
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(300.0, 200.0)), &mut escena, 1.0);
        let r = g.evento(mover(Punto2::nuevo(310.0, 210.0)), &mut escena, 1.0);
        match r.region {
            Region::Caja(x0, y0, x1, y1) => {
                // El lado izquierdo y el de arriba nacen en el origen y
                // cambian con cada movimiento: tienen que entrar en la zona.
                assert!(x0 <= 0.0 && y0 <= 0.0, "deja fuera el origen: {x0},{y0}");
                assert!(
                    x1 >= 310.0 && y1 >= 210.0,
                    "deja fuera el cursor: {x1},{y1}"
                );
            }
            otra => panic!("esperaba una caja, llego {otra:?}"),
        }
    }

    #[test]
    fn soltar_apaga_la_pista() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);
        assert!(g.anclaje_activo.is_some());
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(150.0, 90.0),
            },
            &mut escena,
            1.0,
        );
        assert!(
            g.anclaje_activo.is_none(),
            "la marca se queda pintada despues de soltar"
        );
    }

    #[test]
    fn con_el_iman_apagado_el_punto_llega_intacto() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;
        g.enganche = crate::enganche::Ajustes::NINGUNO;

        // A distancia intermedia de la esquina (100,50): a 10, fuera del
        // margen de picado normal de `impacto::toca` (grosor/2 + 6 = 7, asi
        // que el clic no selecciona el rectangulo 7 en vez de dibujar uno
        // nuevo) pero dentro del radio del iman (14 a esta escala). Un punto
        // mas lejos -(300,300), por ejemplo- no distinguiria "iman apagado"
        // de "no habia nada que enganchar": ahi tampoco engancharia con el
        // iman encendido, y la prueba pasaria igual con `self.enganche` roto.
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);

        assert!(g.anclaje_activo.is_none());
        let nuevo = escena.elementos.last().expect("nacio un rectangulo");
        assert_eq!(nuevo.x, 110.0, "el iman apagado no puede mover el punto");
    }

    #[test]
    fn la_marquesina_no_engancha() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        // `Mano` es la herramienta de seleccionar y mover: no hay variante
        // `Seleccion` en el enum.
        g.herramienta = Herramienta::Mano;

        // Se pulsa en vacio, lejos del rectangulo, y se arrastra hacia su
        // esquina: seleccionar no es trazar, no debe pegarse.
        g.evento(pulsar(Punto2::nuevo(300.0, 300.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(102.0, 52.0)), &mut escena, 1.0);

        assert!(g.anclaje_activo.is_none());
    }

    #[test]
    fn enganchar_convierte_la_escala_a_zoom_no_la_pasa_directa() {
        // `evento` recibe `escala` -unidades de escena por pixel de
        // pantalla, `1.0 / camara.zoom`-, y el iman quiere el zoom. Las
        // pruebas de arriba usan todas escala 1.0, y el inverso de 1 es 1:
        // si `enganchar` pasara `escala` directa a `sitio()` como si fuera
        // el zoom, esta prueba es la unica que lo notaria.
        //
        // A escala 0.25 el zoom es 4 y el radio de escena queda en
        // 14/4=3.5: un punto a 10 de la esquina no debe enganchar. Si se
        // pasara la escala sin invertir, "zoom" seria 0.25 y el radio
        // saldria en 14/0.25=56, y si engancharia.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;
        // (110,50) y no (10,0): a 10 de la esquina (100,50) por fuera del
        // rectangulo, o sea fuera del margen de picado (7). Sobre la propia
        // geometria el clic seleccionaria el vecino y ni llegaria al iman.
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 0.25);
        assert!(
            g.anclaje_activo.is_none(),
            "a escala 0.25 (zoom 4, radio de escena 3.5) 10 no puede enganchar"
        );

        // El mismo punto, a escala 1.0 (zoom 1, radio de escena 14), si
        // debe enganchar: confirma que la conversion no rompe el caso
        // normal, solo el invertido.
        let mut escena2 = Escena::nueva();
        escena2.elementos.push(rect_para_iman(7));
        let mut g2 = Gesto::nuevo();
        g2.herramienta = Herramienta::Rectangulo;
        g2.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena2, 1.0);
        assert!(
            g2.anclaje_activo.is_some(),
            "a escala 1.0 (zoom 1, radio de escena 14) 10 tiene que enganchar"
        );
    }

    /// **La flecha sigue a la caja, y un solo Ctrl+Z deshace las dos.**
    ///
    /// Es la razon de que `enlace::seguir` se llame aqui y no desde la
    /// ventana: dentro del paso de deshacer que el arrastre dejo abierto. Si
    /// abriera el suyo, un Ctrl+Z dejaria la caja en su sitio viejo y la
    /// flecha en el nuevo, que se ve peor que no seguirla.
    #[test]
    fn al_mover_la_caja_la_flecha_la_sigue_y_se_deshacen_juntas() {
        use crate::elemento::Enganche;

        let mut escena = Escena::nueva();
        let caja = escena.anadir(rect(200.0, 0.0, 80.0, 40.0));
        let flecha = escena.anadir(Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 20.0), Punto2::nuevo(190.0, 20.0)],
                punta_inicio: crate::formas::TipoPunta::Ninguna,
                punta_fin: crate::formas::TipoPunta::Flecha,
                codos: false,
            },
            extras: crate::elemento::Extras {
                enganche_fin: Some(Enganche {
                    elemento: crate::enlace::id_de_texto(caja),
                    foco: 0.0,
                    hueco: 1.0,
                    punto_fijo: None,
                    modo: Default::default(),
                }),
                ..Default::default()
            },
            ..rect(0.0, 0.0, 190.0, 40.0)
        });
        let punta_antes = match &escena.buscar(flecha).unwrap().figura {
            Figura::Flecha { puntos, .. } => puntos[1],
            _ => unreachable!(),
        };

        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner_todos(vec![caja]);
        g.evento(pulsar(Punto2::nuevo(240.0, 20.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(240.0, 320.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(240.0, 320.0),
            },
            &mut escena,
            1.0,
        );

        let punta_despues = match &escena.buscar(flecha).unwrap().figura {
            Figura::Flecha { puntos, .. } => puntos[1],
            _ => unreachable!(),
        };
        assert_ne!(
            punta_antes, punta_despues,
            "la flecha no siguio a la caja que se movio"
        );

        // Y un SOLO deshacer devuelve las dos cosas a la vez.
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(caja).unwrap().y, 0.0, "la caja no volvio");
        let punta_vuelta = match &escena.buscar(flecha).unwrap().figura {
            Figura::Flecha { puntos, .. } => puntos[1],
            _ => unreachable!(),
        };
        assert_eq!(
            punta_vuelta, punta_antes,
            "un Ctrl+Z dejo la caja en su sitio viejo y la flecha en el nuevo"
        );
    }

    #[test]
    fn el_boton_de_la_flecha_de_codos_hace_una_flecha_que_dobla() {
        // **El comentario prometia menos de lo que habia, y el boton hacia
        // menos de lo que prometia el commit.** `codo.rs` es el porte entero
        // de `Elbow.kt` y `pintado.rs` lo usa cuando `codos` esta puesto,
        // pero la herramienta lo ponia a `false`: el conector de organigrama
        // salia recto, o sea igual que el boton de al lado.
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::FlechaCodos;
        let e = g.nuevo_elemento(Punto2::nuevo(0.0, 0.0));
        let Figura::Flecha { codos, .. } = &e.figura else {
            panic!("la de codos tiene que ser una flecha por dentro");
        };
        assert!(*codos, "el conector de organigrama nace recto");

        // Caso negativo: la flecha normal y la de pulso siguen sin codos, o
        // toda flecha del editor doblaria.
        for h in [Herramienta::Flecha, Herramienta::FlechaLibre] {
            g.herramienta = h;
            let Figura::Flecha { codos, .. } = &g.nuevo_elemento(Punto2::nuevo(0.0, 0.0)).figura
            else {
                panic!("{h:?} tiene que ser una flecha");
            };
            assert!(!*codos, "{h:?} no puede nacer de codos");
        }
    }

    #[test]
    fn dibujar_una_figura_nueva_no_recoloca_ninguna_flecha() {
        use crate::elemento::Enganche;

        // Caso negativo: tras dibujar no hay nada que seguir -a lo que acaba
        // de nacer no le cuelga ninguna flecha- y repasar la escena entera
        // por cada trazo seria pagar el organigrama al apoyar el lapiz.
        let mut escena = Escena::nueva();
        let caja = escena.anadir(rect(200.0, 0.0, 80.0, 40.0));
        let flecha = escena.anadir(Elemento {
            figura: Figura::Flecha {
                puntos: vec![Punto2::nuevo(0.0, 20.0), Punto2::nuevo(190.0, 20.0)],
                punta_inicio: crate::formas::TipoPunta::Ninguna,
                punta_fin: crate::formas::TipoPunta::Flecha,
                codos: false,
            },
            extras: crate::elemento::Extras {
                enganche_fin: Some(Enganche {
                    elemento: crate::enlace::id_de_texto(caja),
                    foco: 0.0,
                    hueco: 1.0,
                    punto_fijo: None,
                    modo: Default::default(),
                }),
                ..Default::default()
            },
            ..rect(0.0, 0.0, 190.0, 40.0)
        });
        let version_antes = escena.buscar(flecha).unwrap().version;

        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;
        g.evento(pulsar(Punto2::nuevo(500.0, 500.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(560.0, 560.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(560.0, 560.0),
            },
            &mut escena,
            1.0,
        );
        assert_eq!(
            escena.buscar(flecha).unwrap().version,
            version_antes,
            "la flecha se toco sin que nadie moviera su caja"
        );
    }
}
