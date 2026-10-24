//! **El texto rico del lienzo, de punta a punta** (15-oct-2026): se escribe
//! con `$...$` y marcas, se compone al salir de la caja, se vuelve a editar
//! como fuente, Ctrl+Z dentro y fuera, atajos de color sobre lo elegido, y
//! la ida y vuelta por el fichero (con su texto plano de reserva para quien
//! no entiende el formato).

use pixpin_motor2d::elemento::{Elemento, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::pintado::{Orden, ordenes};
use pixpin_motor2d::texto::{self, AtajoTexto, EstiloDeTexto, TeclaTexto};
use pixpin_motor2d::texto_rico;
use pixpin_motor2d::vector::Punto2;

fn medidor(t: &str, tam: f32, _familia: &str, _e: EstiloDeTexto) -> Option<(f32, f32)> {
    let largo = t.split('\n').map(|r| r.chars().count()).max().unwrap_or(0) as f32;
    Some((largo * tam * 0.5, t.split('\n').count() as f32 * tam * 1.25))
}

fn clic(g: &mut Gesto, e: &mut Escena, x: f32, y: f32) {
    g.evento(
        EventoGesto::Pulsar {
            p: Punto2::nuevo(x, y),
            shift: false,
            alt: false,
            presion: None,
        },
        e,
        1.0,
    );
    g.evento(
        EventoGesto::Soltar {
            p: Punto2::nuevo(x, y),
        },
        e,
        1.0,
    );
}

fn escribir(g: &mut Gesto, e: &mut Escena, s: &str) {
    for c in s.chars() {
        if c == '\n' {
            g.tecla_de_texto(TeclaTexto::Entrar, e);
        } else {
            g.escribir(c, e);
        }
    }
}

fn texto_de(e: &Escena, id: u64) -> (String, Option<String>) {
    let el = e.buscar(id).unwrap();
    let Figura::Texto { texto, .. } = &el.figura else {
        panic!()
    };
    (texto.clone(), el.extras.rico.clone())
}

#[test]
fn se_compone_al_salir_y_se_vuelve_a_editar_como_fuente() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Texto);
        clic(&mut g, &mut escena, 100.0, 50.0);
        let id = g.texto_en_curso().unwrap().0;
        escribir(
            &mut g,
            &mut escena,
            "Nos piden $m\\sphericalangle COD = \\color{red}{X}$",
        );
        // Mientras se escribe se ve la fuente.
        assert_eq!(texto_de(&escena, id).1, None);
        assert!(texto_de(&escena, id).0.contains("$m\\sphericalangle"));
        g.cerrar_texto_con_teclado(&mut escena);
        let (plano, rico) = texto_de(&escena, id);
        assert_eq!(plano, "Nos piden m∢COD = X");
        assert_eq!(
            rico.as_deref(),
            Some("Nos piden $m\\sphericalangle COD = \\color{red}{X}$")
        );
        // Pintado: la X en rojo, ninguna barra a la vista.
        let el = escena.buscar(id).unwrap().clone();
        let o = ordenes(&el);
        let rojo = texto_rico::color_de_nombre("red").unwrap();
        assert!(o.iter().any(
            |o| matches!(o, Orden::Texto { texto, color, .. } if texto == "X" && color.r == rojo.r)
        ));
        assert!(
            !o.iter()
                .any(|o| matches!(o, Orden::Texto { texto, .. } if texto.contains('\\')))
        );
        // Volver a abrirlo da la fuente para editar (sin elegir: elegido,
        // el clic agarra la caja).
        g.seleccion.limpiar();
        g.tomar_herramienta(Herramienta::Texto);
        clic(&mut g, &mut escena, el.x + 5.0, el.y + 5.0);
        assert_eq!(g.texto_en_curso().unwrap().1.texto(), rico.unwrap());
        g.cerrar_texto(&mut escena);
    });
}

#[test]
fn un_texto_sin_marcas_sigue_siendo_de_los_de_siempre() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Texto);
        clic(&mut g, &mut escena, 100.0, 50.0);
        let id = g.texto_en_curso().unwrap().0;
        escribir(&mut g, &mut escena, "cuesta $5 y 2*3*4");
        g.cerrar_texto_con_teclado(&mut escena);
        assert_eq!(
            texto_de(&escena, id),
            ("cuesta $5 y 2*3*4".to_string(), None)
        );
    });
}

#[test]
fn atajos_de_color_negrita_y_deshacer_dentro_del_texto() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Texto);
        clic(&mut g, &mut escena, 100.0, 50.0);
        let id = g.texto_en_curso().unwrap().0;
        escribir(&mut g, &mut escena, "Nos piden X");
        // Mayus+Izquierda elige la X; Ctrl+Mayus+1 la pone roja.
        let a = texto::atajo_de_tecla(0x25, false, true, false).unwrap();
        assert!(g.atajo_de_texto(a, &mut escena));
        assert_eq!(g.texto_en_curso().unwrap().1.texto_elegido(), "X");
        assert!(
            !g.seleccion_de_texto(&escena).is_empty(),
            "lo elegido se ve"
        );
        let rojo = texto::atajo_de_tecla(b'1' as u32, true, true, false).unwrap();
        assert_eq!(rojo, AtajoTexto::Color("rojo"));
        assert!(g.atajo_de_texto(rojo, &mut escena));
        assert_eq!(
            g.texto_en_curso().unwrap().1.texto(),
            "Nos piden \\color{rojo}{X}"
        );
        // Encadenado sobre lo mismo: negrita.
        assert!(g.atajo_de_texto(
            texto::atajo_de_tecla(b'B' as u32, true, false, false).unwrap(),
            &mut escena
        ));
        assert_eq!(
            g.texto_en_curso().unwrap().1.texto(),
            "Nos piden \\color{rojo}{**X**}"
        );
        // Ctrl+Z dos veces: vuelve a «Nos piden X».
        let z = texto::atajo_de_tecla(b'Z' as u32, true, false, false).unwrap();
        assert!(g.atajo_de_texto(z, &mut escena));
        assert!(g.atajo_de_texto(z, &mut escena));
        assert_eq!(g.texto_en_curso().unwrap().1.texto(), "Nos piden X");
        // Y Ctrl+Y lo rehace.
        let y = texto::atajo_de_tecla(b'Y' as u32, true, false, false).unwrap();
        assert!(g.atajo_de_texto(y, &mut escena));
        assert_eq!(
            g.texto_en_curso().unwrap().1.texto(),
            "Nos piden \\color{rojo}{X}"
        );
        // Ctrl+M sin nada elegido abre una formula con el cursor dentro.
        g.tecla_de_texto(TeclaTexto::Fin, &mut escena);
        assert!(g.atajo_de_texto(AtajoTexto::Formula, &mut escena));
        escribir(&mut g, &mut escena, "x^2");
        assert_eq!(
            g.texto_en_curso().unwrap().1.texto(),
            "Nos piden \\color{rojo}{X}$x^2$"
        );
        g.cerrar_texto(&mut escena);
        assert!(texto_de(&escena, id).1.is_some());
        // Una tecla sin atajo no hace nada.
        assert_eq!(texto::atajo_de_tecla(b'Q' as u32, true, false, false), None);
    });
}

#[test]
fn ctrl_z_del_lienzo_deshace_lo_escrito_en_un_texto_viejo() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Texto);
        clic(&mut g, &mut escena, 100.0, 50.0);
        let id = g.texto_en_curso().unwrap().0;
        escribir(&mut g, &mut escena, "hola");
        g.cerrar_texto(&mut escena);
        let el = escena.buscar(id).unwrap().clone();
        clic(&mut g, &mut escena, el.x + 2.0, el.y + 2.0);
        escribir(&mut g, &mut escena, " $x^2$");
        g.cerrar_texto(&mut escena);
        assert_eq!(texto_de(&escena, id).1.as_deref(), Some("hola $x^2$"));
        escena.deshacer();
        assert_eq!(texto_de(&escena, id), ("hola".to_string(), None));
    });
}

#[test]
fn ida_y_vuelta_por_el_fichero_con_su_texto_plano_de_reserva() {
    texto::con_medidor(medidor, || {
        let fuente = "\\centro $45^\\circ = \\frac{\\color{blue}{2a+2b}}{2}$\n**ya**";
        let mut e = Elemento {
            figura: Figura::Texto {
                texto: texto_rico::plano(fuente),
                tam: 20.0,
                familia: "Nunito".into(),
            },
            x: 10.0,
            y: 10.0,
            ..Default::default()
        };
        e.extras.rico = Some(fuente.to_string());
        let json = excalidraw::elementos_a_json(std::slice::from_ref(&e));
        assert_eq!(json[0]["textoRico"], fuente);
        assert_eq!(
            json[0]["text"], "45° = (2a + 2b)/2\nya",
            "el plano de reserva"
        );
        let vuelta = excalidraw::elementos_de_json(&json);
        assert_eq!(vuelta[0].extras.rico.as_deref(), Some(fuente));
        // El movil (que no lo entiende) cambio el texto plano: gana el suyo.
        let mut editado = json.clone();
        editado[0]["text"] = "otra cosa".into();
        let vuelta = excalidraw::elementos_de_json(&editado);
        assert_eq!(vuelta[0].extras.rico, None);
        assert!(matches!(&vuelta[0].figura, Figura::Texto { texto, .. } if texto == "otra cosa"));
        // Un texto de siempre no estrena la clave.
        let liso = Elemento {
            figura: Figura::Texto {
                texto: "a".into(),
                tam: 20.0,
                familia: "Nunito".into(),
            },
            ..Default::default()
        };
        let json = excalidraw::elementos_a_json(&[liso]);
        assert!(json[0].get("textoRico").is_none());
        // Y por serde (el guardado propio) tambien viaja.
        let s = serde_json::to_string(&e).unwrap();
        let e2: Elemento = serde_json::from_str(&s).unwrap();
        assert_eq!(e2.extras.rico, e.extras.rico);
    });
}

#[test]
fn el_svg_saca_el_texto_rico_compuesto() {
    texto::con_medidor(medidor, || {
        let fuente = "Si $\\overline{AB}$ y \\color{red}{X}";
        let mut e = Elemento {
            figura: Figura::Texto {
                texto: texto_rico::plano(fuente),
                tam: 20.0,
                familia: "Nunito".into(),
            },
            x: 10.0,
            y: 10.0,
            ancho: 200.0,
            alto: 40.0,
            ..Default::default()
        };
        e.extras.rico = Some(fuente.to_string());
        let mut escena = Escena::nueva();
        escena.anadir(e);
        let hojas = pixpin_motor2d::exportar::hojas(
            &escena,
            pixpin_motor2d::exportar::Alcance::Todo,
            &[],
            None,
        );
        let svg = pixpin_motor2d::exportar_svg::svg(&hojas[0], Default::default(), &|_| None);
        assert!(svg.contains(">X<"), "{svg}");
        assert!(svg.contains("#e03131"), "la X roja");
        assert!(!svg.contains("overline"), "sin la fuente a la vista");
    });
}
