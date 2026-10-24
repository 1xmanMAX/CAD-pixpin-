//! **Que herramientas salen en cada sitio.**
//!
//! Dos cosas lo deciden, y aqui se juntan en una sola pregunta para que
//! ningun anfitrion pueda olvidarse de una de las dos:
//!
//! 1. Lo que el usuario apago en los ajustes generales (`[herramientas]`).
//!    Una apagada no sale en la barra ni responde a su letra ni a su atajo
//!    del motor en NINGUN anfitrion.
//! 2. Lo que ese anfitrion sabe hacer. La lupa necesita la pantalla viva, el
//!    cajetin de calibrar es del lienzo, el mosaico lee los pixeles de la
//!    escena ya pintada...: ofrecer un boton que no hace nada se vive como
//!    una averia.
//!
//! La copia de los ajustes es global porque la leen cuatro ventanas que se
//! abren desde sitios que no tienen nada que ver entre si (la bandeja, el
//! chat, un pin, el lector), en hilos distintos, y la ventana de ajustes la
//! cambia en caliente: lo que se abra despues ya sale sin la apagada.

use pixpin_motor2d::gesto::{Gesto, Herramienta};
use pixpin_store::herramientas::Herramientas;
use pixpin_ui::{BOTONES_EDITOR, BotonCaja};
use std::sync::RwLock;

/// Donde se dibuja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anfitrion {
    /// El editor avanzado (el lienzo).
    Lienzo,
    /// Encima de un Word, un libro, una pagina o una hoja de PDF.
    Lector,
    /// Encima de un pin, en su sitio.
    Pin,
    /// El anotador de pantalla con la pantalla viva debajo.
    PantallaViva,
    /// El anotador de pantalla sobre una foto del escritorio.
    PantallaCongelada,
}

/// El nombre estable de una herramienta en el TOML (`pixpin_store::
/// herramientas::NOMBRES`). `None` para las que no se pueden apagar.
pub fn nombre_de(h: Herramienta) -> Option<&'static str> {
    Some(match h {
        // El marco tampoco (22-oct): ya no va en la barra, lo pone el boton
        // del riel de la derecha.
        Herramienta::Mano | Herramienta::Emoji | Herramienta::Marco => return None,
        Herramienta::Lazo => "lazo",
        Herramienta::Rectangulo => "rectangulo",
        Herramienta::Rombo => "rombo",
        Herramienta::Elipse => "elipse",
        Herramienta::Flecha => "flecha",
        Herramienta::FlechaCodos => "flecha-codos",
        Herramienta::FlechaLibre => "flecha-libre",
        Herramienta::Linea => "linea",
        Herramienta::Lapiz => "lapiz",
        Herramienta::Grafito => "grafito",
        Herramienta::Texto => "texto",
        Herramienta::Borrador => "borrador",
        Herramienta::Resaltador => "resaltador",
        Herramienta::Foco => "foco",
        Herramienta::Lupa => "lupa",
        Herramienta::Mosaico => "mosaico",
        Herramienta::Arco => "arco",
        Herramienta::Angulo => "angulo",
        Herramienta::Serie => "serie",
        Herramienta::Punto => "punto",
        Herramienta::Cota => "cota",
        Herramienta::Escalar => "escalar",
        Herramienta::EscalaGrafica => "escala-grafica",
        Herramienta::Relleno => "relleno",
        Herramienta::Recortar => "recortar",
        Herramienta::Extender => "extender",
        Herramienta::CopiarEstilo => "copiar-estilo",
        Herramienta::Zona => "zona",
        Herramienta::Laser => "laser",
        Herramienta::Nudo => "nudo",
        Herramienta::Bolita => "bolita",
    })
}

/// El nombre estable de un boton de la barra que se puede apagar: el de su
/// herramienta, o el de las acciones que tambien son del lienzo (meter una
/// imagen, las figuras, imprimir y compartir). Deshacer, rehacer y salir no:
/// sin ellos no se puede ni arreglar un error ni irse.
pub fn nombre_de_boton(b: BotonCaja) -> Option<&'static str> {
    match b {
        BotonCaja::Elegir(h) => nombre_de(h),
        BotonCaja::Imagen => Some("imagen"),
        BotonCaja::Figuras => Some("figuras"),
        BotonCaja::Imprimir => Some("imprimir"),
        BotonCaja::Compartir => Some("compartir"),
        _ => None,
    }
}

/// **El nombre que se lee** de un boton: el de los ajustes
/// (`herramientas-<nombre>`), el mismo en el desplegable de un grupo, en su
/// globo y en la pestana de ajustes. La mano no se apaga pero tiene nombre.
pub fn nombre_visible(b: BotonCaja, textos: &pixpin_store::Catalogo) -> String {
    match b {
        BotonCaja::Elegir(Herramienta::Mano) => textos.t("herramientas-mano"),
        BotonCaja::Atravesar => textos.t("anotador-boton-atravesar"),
        BotonCaja::Grupo(g) => textos.t(&format!("barra-grupo-{}", g.nombre())),
        otro => nombre_de_boton(otro)
            .map(|n| textos.t(&format!("herramientas-{n}")))
            .unwrap_or_default(),
    }
}

/// **El rotulo que sale al pasar el raton por un boton de la barra**: el
/// nombre de la herramienta (`herramientas-<nombre>`, el mismo de la ventana
/// de ajustes) y, en imprimir, su atajo. Con treinta y tantos iconos, varios
/// sin equivalente en Excalidraw (soldar, el grafito, el punto con letra), el
/// icono solo no basta para encontrarlos. `None` en los que no lo necesitan.
pub fn rotulo_de_boton(b: BotonCaja, textos: &pixpin_store::Catalogo) -> Option<String> {
    match b {
        BotonCaja::Imprimir => Some(textos.t("imprimir-pista")),
        // Las de imagen dicen ademas como se usan: la lupa es una varita
        // (no se arrastra), y pixelar y la zona se arrastran.
        BotonCaja::Compartir => Some(textos.t("lienzo-pista-compartir")),
        BotonCaja::Atravesar => Some(textos.t("anotador-pista-atravesar")),
        BotonCaja::Elegir(Herramienta::Lupa) => Some(textos.t("lienzo-pista-lupa")),
        BotonCaja::Elegir(Herramienta::Mosaico) => Some(textos.t("lienzo-pista-mosaico")),
        BotonCaja::Elegir(Herramienta::Zona) => Some(textos.t("lienzo-pista-zona")),
        // El foco tambien es varita, y los pasos se ponen de un clic.
        BotonCaja::Elegir(Herramienta::Foco) => Some(textos.t("lienzo-pista-foco")),
        BotonCaja::Elegir(Herramienta::Serie) => Some(textos.t("lienzo-pista-serie")),
        // Las dos de las laminas de geometria dicen como se usan.
        BotonCaja::Elegir(Herramienta::Arco) => Some(textos.t("lienzo-pista-arco")),
        BotonCaja::Elegir(Herramienta::Angulo) => Some(textos.t("lienzo-pista-angulo")),
        // Tambien lo de siempre, con su atajo: el usuario pidio globo en
        // todo, y quien no sabe `Ctrl+Z` es justo quien lo necesita.
        BotonCaja::Elegir(Herramienta::Mano) => Some(textos.t("barra-pista-mano")),
        BotonCaja::Deshacer => Some(textos.t("barra-pista-deshacer")),
        BotonCaja::Rehacer => Some(textos.t("barra-pista-rehacer")),
        BotonCaja::Salir => Some(textos.t("barra-pista-salir")),
        // El nombre del grupo; `caja_dibujo::pintar_pista` le pega detras
        // el de la herramienta que ensena.
        BotonCaja::Grupo(g) => Some(textos.t(&format!("barra-grupo-{}", g.nombre()))),
        _ => nombre_de_boton(b).map(|n| textos.t(&format!("herramientas-{n}"))),
    }
}

/// **Las secciones de la pestana de herramientas de los ajustes**: las
/// mismas que la barra, en su orden. Primero las sueltas que se pueden apagar
/// (`None`: el lapiz y la goma), luego cada grupo con lo suyo, y al final,
/// en su propia seccion sin titulo, cualquier nombre de los ajustes que no
/// este en la barra agrupada (la red de `gruposDe` del movil: nada se queda
/// sin interruptor).
pub fn secciones_de_ajustes() -> Vec<(Option<pixpin_ui::GrupoBarra>, Vec<&'static str>)> {
    let mut v: Vec<(Option<pixpin_ui::GrupoBarra>, Vec<&'static str>)> = Vec::new();
    let sueltas: Vec<&'static str> = pixpin_ui::BARRA_AGRUPADA
        .iter()
        .filter_map(|b| nombre_de_boton(*b))
        .collect();
    v.push((None, sueltas));
    for b in pixpin_ui::BARRA_AGRUPADA {
        if let BotonCaja::Grupo(g) = b {
            let nombres = g
                .miembros()
                .iter()
                .filter_map(|m| nombre_de_boton(*m))
                .collect();
            v.push((Some(g), nombres));
        }
    }
    let vistos: Vec<&'static str> = v.iter().flat_map(|(_, n)| n.iter().copied()).collect();
    let resto: Vec<&'static str> = pixpin_store::herramientas::NOMBRES
        .iter()
        .copied()
        .filter(|n| !vistos.contains(n))
        .collect();
    if !resto.is_empty() {
        v.push((None, resto));
    }
    v
}

/// La herramienta de un nombre del TOML (para la ventana de ajustes).
#[cfg(test)]
pub fn de_nombre(nombre: &str) -> Option<Herramienta> {
    BOTONES_EDITOR.iter().find_map(|b| match b {
        BotonCaja::Elegir(h) if nombre_de(*h) == Some(nombre) => Some(*h),
        _ => None,
    })
}

/// El boton de un nombre del TOML: herramienta o accion.
pub fn boton_de_nombre(nombre: &str) -> Option<BotonCaja> {
    BOTONES_EDITOR
        .iter()
        .copied()
        .find(|b| nombre_de_boton(*b) == Some(nombre))
}

impl Anfitrion {
    /// Su nombre en `[herramientas.por_sitio]` (`herramientas::SITIOS`). Las
    /// dos pantallas son un solo sitio: para el usuario es «dibujar en la
    /// pantalla», con la pantalla viva o congelada debajo.
    pub fn sitio(self) -> &'static str {
        match self {
            Anfitrion::Lienzo => "lienzo",
            Anfitrion::Lector => "lector",
            Anfitrion::Pin => "pin",
            Anfitrion::PantallaViva | Anfitrion::PantallaCongelada => "pantalla",
        }
    }

    /// El anfitrion que representa a un sitio en la ventana de ajustes: lo
    /// que ese sitio sabe hacer decide que interruptores salen.
    pub fn de_sitio(sitio: &str) -> Option<Anfitrion> {
        Some(match sitio {
            "lienzo" => Anfitrion::Lienzo,
            "lector" => Anfitrion::Lector,
            "pin" => Anfitrion::Pin,
            "pantalla" => Anfitrion::PantallaCongelada,
            _ => return None,
        })
    }

    /// Si este anfitrion sabe hacer la herramienta (sin mirar los ajustes).
    pub fn admite(self, h: Herramienta) -> bool {
        use Herramienta as H;
        match self {
            // El lienzo, todas las de su barra: es donde nacieron.
            Anfitrion::Lienzo => h != H::Emoji,
            // Sobre un documento: la lupa necesita la pantalla, calibrar
            // abre el cajetin del lienzo y el mosaico tapa leyendo la
            // escena ya pintada, que en el lector es el texto (una pasada
            // que el lector no tiene). La zona saca su foto con el
            // almacen de imagenes del lienzo (el movil tampoco la deja en el
            // editor rapido, `LECTOR_TOOLS_FUERA`), y el laser se apaga con
            // el reloj del bucle del lienzo.
            Anfitrion::Lector => !matches!(
                h,
                H::Lupa | H::Escalar | H::Mosaico | H::Emoji | H::Zona | H::Laser | H::Nudo
            ),
            // El pin pinta sus ordenes el mismo, sin los rotulos de medida
            // ni el cajetin: medir sobre una captura se hace en el lienzo
            // («Abrir en el lienzo»). Sin zona ni laser, por lo del lector.
            Anfitrion::Pin => !matches!(
                h,
                H::Lupa
                    | H::Escalar
                    | H::Cota
                    | H::EscalaGrafica
                    | H::Emoji
                    | H::Zona
                    | H::Laser
                    // Los clavos de soldar los pinta y los guarda el lienzo.
                    | H::Nudo
            ),
            // En pantalla la camara esta quieta a 1:1: calibrar no tiene
            // sentido. La lupa si: alli es la lupa viva de la capa vieja
            // (`pantalla::LupaViva`), que amplia la pantalla de verdad. El
            // laser tambien: senalar en la pantalla es para lo que se abre.
            Anfitrion::PantallaCongelada => !matches!(h, H::Escalar | H::Emoji | H::Zona | H::Nudo),
            // Con la pantalla viva debajo, ademas, el mosaico no tiene que
            // tapar: la escena es transparente y la pasada leeria vacio.
            Anfitrion::PantallaViva => {
                !matches!(h, H::Escalar | H::Mosaico | H::Emoji | H::Zona | H::Nudo)
            }
        }
    }

    /// Si este anfitrion sabe hacer lo que hace el boton. Meter una imagen y
    /// las figuras (con su cajetin de la grafica) son del lienzo: necesitan
    /// su almacen de imagenes y su bucle de pintar.
    pub fn admite_boton(self, b: BotonCaja) -> bool {
        match b {
            BotonCaja::Elegir(h) => self.admite(h),
            // Imprimir tambien: solo el lienzo tiene un dibujo que imprimir.
            BotonCaja::Imagen | BotonCaja::Figuras | BotonCaja::Imprimir | BotonCaja::Compartir => {
                self == Anfitrion::Lienzo
            }
            // Solo con la pantalla VIVA debajo hay a quien dejar pasar el raton.
            BotonCaja::Atravesar => self == Anfitrion::PantallaViva,
            _ => true,
        }
    }
}

/// Los ajustes de ahora. `None` = nadie los fijo (pruebas, arranque): todas
/// activas.
static AJUSTES: RwLock<Option<Herramientas>> = RwLock::new(None);

/// La llaman `main` al arrancar y la ventana de ajustes al guardar.
pub fn fijar(h: Herramientas) {
    if let Ok(mut g) = AJUSTES.write() {
        *g = Some(h);
    }
}

fn ajustes() -> Herramientas {
    AJUSTES
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default()
}

/// Pura: si `h` sale en `anfitrion` con esos ajustes.
pub fn permitida_con(anfitrion: Anfitrion, h: Herramienta, ajustes: &Herramientas) -> bool {
    anfitrion.admite(h) && nombre_de(h).is_none_or(|n| ajustes.activa_en(anfitrion.sitio(), n))
}

/// Si `h` sale ahora en `anfitrion`.
pub fn permitida(anfitrion: Anfitrion, h: Herramienta) -> bool {
    permitida_con(anfitrion, h, &ajustes())
}

/// Pura: los botones de la barra de `anfitrion` con esos ajustes. Las
/// acciones (deshacer, rehacer, salir) siempre.
pub fn botones_con(anfitrion: Anfitrion, ajustes: &Herramientas) -> Vec<BotonCaja> {
    // El clic a traves no es del lienzo: solo lo admite la pantalla viva.
    BOTONES_EDITOR
        .iter()
        .copied()
        .chain([BotonCaja::Atravesar])
        .filter(|b| boton_permitido_con(anfitrion, *b, ajustes))
        .collect()
}

/// Pura: si el boton `b` sale en `anfitrion` con esos ajustes.
pub fn boton_permitido_con(anfitrion: Anfitrion, b: BotonCaja, ajustes: &Herramientas) -> bool {
    anfitrion.admite_boton(b)
        && nombre_de_boton(b).is_none_or(|n| ajustes.activa_en(anfitrion.sitio(), n))
}

/// Si el boton `b` sale ahora en `anfitrion`.
pub fn boton_permitido(anfitrion: Anfitrion, b: BotonCaja) -> bool {
    boton_permitido_con(anfitrion, b, &ajustes())
}

/// Las listas ya hechas, por anfitrion y ajustes. `CajaHerramientas` guarda
/// una rebanada `'static` (asi se ahorra copiarla con cada evento), y la
/// lista de hoy depende de los ajustes: se hace una vez por combinacion y se
/// deja viva. Son unas pocas por sesion (una por cada cambio en los ajustes
/// y anfitrion), de unos cientos de bytes.
static HECHAS: RwLock<Vec<(Anfitrion, Herramientas, &'static [BotonCaja])>> =
    RwLock::new(Vec::new());

/// Los botones de la barra de `anfitrion` con los ajustes de ahora.
pub fn botones(anfitrion: Anfitrion) -> &'static [BotonCaja] {
    let a = ajustes();
    if let Ok(h) = HECHAS.read()
        && let Some((_, _, v)) = h.iter().find(|(an, aj, _)| *an == anfitrion && *aj == a)
    {
        return v;
    }
    let nueva: &'static [BotonCaja] = Box::leak(botones_con(anfitrion, &a).into_boxed_slice());
    if let Ok(mut h) = HECHAS.write() {
        h.push((anfitrion, a, nueva));
    }
    nueva
}

/// La herramienta de una letra en `anfitrion`, o `None` si esa letra no
/// elige nada o elige una apagada. Es la unica tabla de letras que deben
/// usar los anfitriones: `teclas::tecla_a_herramienta` a secas no mira los
/// ajustes.
pub fn herramienta_de_letra(anfitrion: Anfitrion, c: char) -> Option<Herramienta> {
    super::teclas::tecla_a_herramienta(c).filter(|h| permitida(anfitrion, *h))
}

/// Si la herramienta que tiene el gesto no sale aqui (la apagaron, o este
/// anfitrion no la sabe hacer), se cambia a la primera que si: la mano, que
/// no se puede apagar. Si no, el anfitrion abriria con una herramienta que
/// no esta en su barra y que su letra ya no elige.
pub fn asegurar(anfitrion: Anfitrion, gesto: &mut Gesto) {
    if !permitida(anfitrion, gesto.herramienta) {
        super::teclas::elegir_herramienta(gesto, Herramienta::Mano);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_apagada_en_el_pin_falta_en_su_barra_y_sigue_en_el_lienzo_y_la_pantalla() {
        let mut h = Herramientas::default();
        h.poner_en("pin", "flecha", false);
        let b = BotonCaja::Elegir(Herramienta::Flecha);
        assert!(!botones_con(Anfitrion::Pin, &h).contains(&b));
        assert!(botones_con(Anfitrion::Lienzo, &h).contains(&b));
        assert!(botones_con(Anfitrion::PantallaViva, &h).contains(&b));
        assert!(!permitida_con(Anfitrion::Pin, Herramienta::Flecha, &h));
    }

    #[test]
    fn las_dos_pantallas_son_un_solo_sitio() {
        assert_eq!(
            Anfitrion::PantallaViva.sitio(),
            Anfitrion::PantallaCongelada.sitio()
        );
        for s in pixpin_store::herramientas::SITIOS {
            assert_eq!(Anfitrion::de_sitio(s).map(Anfitrion::sitio), Some(*s));
        }
    }

    fn con_apagadas(v: &[&str]) -> Herramientas {
        Herramientas {
            apagadas: v.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    const TODOS: [Anfitrion; 5] = [
        Anfitrion::Lienzo,
        Anfitrion::Lector,
        Anfitrion::Pin,
        Anfitrion::PantallaViva,
        Anfitrion::PantallaCongelada,
    ];

    #[test]
    fn cada_herramienta_de_la_barra_tiene_un_nombre_de_los_ajustes_y_vuelta() {
        for b in BOTONES_EDITOR {
            if let BotonCaja::Elegir(h) = b {
                if h == Herramienta::Mano {
                    continue;
                }
                let n = nombre_de(h).expect("toda herramienta de la barra se puede apagar");
                assert!(pixpin_store::herramientas::NOMBRES.contains(&n), "{n}");
                assert_eq!(de_nombre(n), Some(h));
            }
            if let Some(n) = nombre_de_boton(b) {
                assert!(pixpin_store::herramientas::NOMBRES.contains(&n), "{n}");
                assert_eq!(boton_de_nombre(n), Some(b));
            }
        }
        // Y al reves: cada nombre de los ajustes es un boton de la barra.
        for n in pixpin_store::herramientas::NOMBRES {
            assert!(boton_de_nombre(n).is_some(), "{n} no esta en la barra");
        }
        // Las dos acciones de dibujar tambien se apagan; las demas no.
        assert_eq!(nombre_de_boton(BotonCaja::Imagen), Some("imagen"));
        assert_eq!(nombre_de_boton(BotonCaja::Salir), None);
    }

    #[test]
    fn una_herramienta_apagada_no_sale_en_la_barra_de_ningun_anfitrion() {
        let a = con_apagadas(&["lazo", "grafito"]);
        for anf in TODOS {
            let v = botones_con(anf, &a);
            assert!(
                !v.contains(&BotonCaja::Elegir(Herramienta::Lazo)),
                "{anf:?}"
            );
            assert!(
                !v.contains(&BotonCaja::Elegir(Herramienta::Grafito)),
                "{anf:?}"
            );
            // Caso negativo: lo demas sigue, y las acciones tambien.
            assert!(
                v.contains(&BotonCaja::Elegir(Herramienta::Lapiz)),
                "{anf:?}"
            );
            assert!(v.contains(&BotonCaja::Elegir(Herramienta::Mano)), "{anf:?}");
            assert!(v.contains(&BotonCaja::Deshacer) && v.contains(&BotonCaja::Salir));
        }
    }

    #[test]
    fn sin_ajustes_el_lienzo_tiene_la_barra_entera_de_siempre() {
        let v = botones_con(Anfitrion::Lienzo, &Herramientas::default());
        assert_eq!(v, BOTONES_EDITOR.to_vec());
    }

    #[test]
    fn el_lector_no_ofrece_lo_que_no_sabe_hacer_sobre_un_documento() {
        let v = botones_con(Anfitrion::Lector, &Herramientas::default());
        for h in [
            Herramienta::Lupa,
            Herramienta::Escalar,
            Herramienta::Mosaico,
        ] {
            assert!(!v.contains(&BotonCaja::Elegir(h)), "{h:?}");
        }
        for h in [
            Herramienta::Lapiz,
            Herramienta::Resaltador,
            Herramienta::Grafito,
            Herramienta::Borrador,
            Herramienta::Rectangulo,
            Herramienta::Flecha,
            Herramienta::Texto,
            Herramienta::Lazo,
        ] {
            assert!(v.contains(&BotonCaja::Elegir(h)), "{h:?}");
        }
    }

    #[test]
    fn la_pantalla_viva_no_ofrece_mosaico_y_la_congelada_si() {
        let d = Herramientas::default();
        assert!(!permitida_con(
            Anfitrion::PantallaViva,
            Herramienta::Mosaico,
            &d
        ));
        assert!(permitida_con(
            Anfitrion::PantallaCongelada,
            Herramienta::Mosaico,
            &d
        ));
    }

    #[test]
    fn la_lupa_viva_es_del_anotador_de_pantalla_y_no_de_los_documentos() {
        let d = Herramientas::default();
        assert!(permitida_con(
            Anfitrion::PantallaViva,
            Herramienta::Lupa,
            &d
        ));
        assert!(permitida_con(
            Anfitrion::PantallaCongelada,
            Herramienta::Lupa,
            &d
        ));
        // Caso negativo: sobre un documento o un pin no hay pantalla que
        // ampliar; y apagada en los ajustes no sale tampoco en la pantalla.
        assert!(!permitida_con(Anfitrion::Lector, Herramienta::Lupa, &d));
        assert!(!permitida_con(Anfitrion::Pin, Herramienta::Lupa, &d));
        let sin = con_apagadas(&["lupa"]);
        assert!(!permitida_con(
            Anfitrion::PantallaViva,
            Herramienta::Lupa,
            &sin
        ));
    }

    #[test]
    fn la_zona_la_imagen_y_las_figuras_son_del_lienzo_y_el_laser_tambien_de_la_pantalla() {
        let d = Herramientas::default();
        let lienzo = botones_con(Anfitrion::Lienzo, &d);
        for b in [
            BotonCaja::Elegir(Herramienta::Zona),
            BotonCaja::Elegir(Herramienta::Laser),
            BotonCaja::Imagen,
            BotonCaja::Figuras,
        ] {
            assert!(lienzo.contains(&b), "{b:?}");
        }
        for anf in [Anfitrion::Lector, Anfitrion::Pin] {
            let v = botones_con(anf, &d);
            assert!(
                !v.contains(&BotonCaja::Imagen) && !v.contains(&BotonCaja::Figuras),
                "{anf:?}"
            );
            assert!(
                !v.contains(&BotonCaja::Elegir(Herramienta::Zona)),
                "{anf:?}"
            );
            assert!(
                !v.contains(&BotonCaja::Elegir(Herramienta::Laser)),
                "{anf:?}"
            );
        }
        assert!(permitida_con(
            Anfitrion::PantallaViva,
            Herramienta::Laser,
            &d
        ));
        assert!(!permitida_con(
            Anfitrion::PantallaViva,
            Herramienta::Zona,
            &d
        ));
        // Y se apagan desde los ajustes como las demas.
        let sin = con_apagadas(&["imagen", "laser"]);
        let v = botones_con(Anfitrion::Lienzo, &sin);
        assert!(!v.contains(&BotonCaja::Imagen));
        assert!(!v.contains(&BotonCaja::Elegir(Herramienta::Laser)));
        assert!(
            v.contains(&BotonCaja::Figuras),
            "caso negativo: lo demas sigue"
        );
    }

    #[test]
    fn la_mano_no_se_puede_apagar() {
        let a = con_apagadas(&["mano"]);
        for anf in TODOS {
            assert!(permitida_con(anf, Herramienta::Mano, &a));
        }
    }

    #[test]
    fn ninguna_letra_elige_una_herramienta_apagada_en_ningun_anfitrion() {
        // Por la copia global, como la usan los anfitriones. Se deja como
        // estaba al acabar: otras pruebas del mismo binario la leen.
        fijar(con_apagadas(&["lapiz", "texto"]));
        for anf in TODOS {
            assert_eq!(herramienta_de_letra(anf, 'l'), None, "{anf:?}");
            assert_eq!(herramienta_de_letra(anf, 'T'), None, "{anf:?}");
            // Caso negativo: el resaltador sigue respondiendo a su R.
            assert_eq!(
                herramienta_de_letra(anf, 'r'),
                Some(Herramienta::Resaltador)
            );
            assert!(!botones(anf).contains(&BotonCaja::Elegir(Herramienta::Lapiz)));
        }
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Lapiz);
        asegurar(Anfitrion::Lector, &mut g);
        assert_eq!(
            g.herramienta,
            Herramienta::Mano,
            "abre con la mano, no con la apagada"
        );
        fijar(Herramientas::default());
        assert_eq!(
            herramienta_de_letra(Anfitrion::Lienzo, 'l'),
            Some(Herramienta::Lapiz)
        );
    }

    #[test]
    fn cada_herramienta_de_la_barra_dice_su_nombre_al_pasar_el_raton() {
        use pixpin_store::{Catalogo, Idioma};
        let t = Catalogo::nuevo(Idioma::Espanol);
        assert_eq!(
            rotulo_de_boton(BotonCaja::Elegir(Herramienta::Nudo), &t).as_deref(),
            Some("Soldar vértices")
        );
        assert_eq!(
            rotulo_de_boton(BotonCaja::Elegir(Herramienta::Punto), &t).as_deref(),
            Some("Punto con letra")
        );
        // TODO lo de la barra lleva globo (lo pidio el usuario), y con un
        // nombre de verdad y no la clave sin traducir: las cuarenta sueltas,
        // los grupos y las acciones.
        for b in BOTONES_EDITOR.into_iter().chain(pixpin_ui::BARRA_AGRUPADA) {
            let r = rotulo_de_boton(b, &t).unwrap_or_else(|| panic!("{b:?} sin globo"));
            assert!(
                !r.starts_with("herramientas-") && !r.starts_with("barra-") && !r.is_empty(),
                "{b:?} sin traducir: {r}"
            );
            assert!(!nombre_visible(b, &t).starts_with("barra-"), "{b:?}");
        }
        assert!(
            rotulo_de_boton(BotonCaja::Deshacer, &t)
                .unwrap()
                .contains("Ctrl+Z")
        );
        // Caso negativo: el color, que no sale en ninguna barra, no lleva.
        assert_eq!(rotulo_de_boton(BotonCaja::Color, &t), None);
    }

    #[test]
    fn soldar_solo_esta_en_el_lienzo_que_es_quien_pinta_y_guarda_los_clavos() {
        assert!(Anfitrion::Lienzo.admite(Herramienta::Nudo));
        for a in [
            Anfitrion::Lector,
            Anfitrion::Pin,
            Anfitrion::PantallaViva,
            Anfitrion::PantallaCongelada,
        ] {
            assert!(!a.admite(Herramienta::Nudo), "{a:?}");
        }
    }

    /// **Cada nombre de los ajustes sale una vez en la pestana, en la
    /// seccion de su grupo de la barra**, y las nuevas (soldar, la bolita,
    /// imprimir, compartir) tambien.
    #[test]
    fn la_pestana_de_ajustes_ensena_cada_herramienta_una_vez_en_el_grupo_de_la_barra() {
        use pixpin_ui::GrupoBarra;
        let s = secciones_de_ajustes();
        let todos: Vec<&str> = s.iter().flat_map(|(_, n)| n.iter().copied()).collect();
        for n in pixpin_store::herramientas::NOMBRES {
            assert_eq!(todos.iter().filter(|x| *x == n).count(), 1, "{n}");
        }
        assert_eq!(todos.len(), pixpin_store::herramientas::NOMBRES.len());
        let de = |g: GrupoBarra| s.iter().find(|(x, _)| *x == Some(g)).unwrap().1.clone();
        // La tabla y la grafica van sueltas desde que se quito el cronograma,
        // que era su grupo.
        // La imagen, suelta desde que el marco salio de la barra (22-oct).
        assert_eq!(s[0], (None, vec!["lapiz", "figuras", "imagen", "borrador"]));
        assert!(de(GrupoBarra::Arreglar).contains(&"nudo"));
        assert!(de(GrupoBarra::Elegir).contains(&"bolita"));
        assert_eq!(de(GrupoBarra::Sacar), vec!["compartir", "imprimir"]);
        // Caso negativo: la mano no se apaga y no sale.
        assert!(!todos.contains(&"mano"));
        // Sin resto: todas tienen grupo o van sueltas.
        assert_eq!(s.len(), 1 + GrupoBarra::TODOS.len());
    }
}
