//! **Pegar texto en el lienzo** (pedido del usuario, 13-oct-2026): «texto
//! copiado, pegarlo en el canvas». Lo hace `Gesto::pegar_texto` con el
//! mismo elemento de texto que la herramienta Texto; aqui se mira que nace
//! con la letra del panel, centrado, partido, elegido, en un paso de
//! deshacer, que dentro de un texto abierto entra en el y que viaja al
//! fichero `.excalidraw` como cualquier texto.
//!
//! Medidor de mentira: 10 unidades por letra, renglones de `tam`.

use pixpin_motor2d::elemento::{ColorRgba, Figura};
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::excalidraw;
use pixpin_motor2d::gesto::{EventoGesto, Gesto, Herramienta};
use pixpin_motor2d::texto::{self, EstiloDeTexto};
use pixpin_motor2d::texto_pegado::TOPE_CARACTERES;
use pixpin_motor2d::vector::Punto2;

fn medidor(t: &str, tam: f32, _familia: &str, _e: EstiloDeTexto) -> Option<(f32, f32)> {
    let largo = t.split('\n').map(|r| r.chars().count()).max().unwrap_or(0) as f32;
    Some((largo * 10.0, t.split('\n').count() as f32 * tam))
}

fn texto_de(escena: &Escena, id: u64) -> String {
    match &escena.buscar(id).unwrap().figura {
        Figura::Texto { texto, .. } => texto.clone(),
        otra => panic!("no es un texto: {otra:?}"),
    }
}

fn vivos(escena: &Escena) -> usize {
    escena.visibles().count()
}

#[test]
fn nace_un_texto_con_la_letra_del_panel_centrado_y_elegido() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        // Con otra herramienta en la mano: el texto nace igual.
        g.tomar_herramienta(Herramienta::Rectangulo);
        g.estilo.tamano_letra = 36.0;
        g.estilo.trazo = ColorRgba {
            r: 0.8,
            g: 0.1,
            b: 0.2,
            a: 1.0,
        };
        let id = g
            .pegar_texto(
                &mut escena,
                "hola mundo",
                Punto2::nuevo(500.0, 300.0),
                800.0,
            )
            .expect("pegado");
        let e = escena.buscar(id).unwrap();
        let Figura::Texto { texto, tam, .. } = &e.figura else {
            panic!("no es texto");
        };
        assert_eq!(texto, "hola mundo");
        assert_eq!(*tam, 36.0);
        assert_eq!(e.trazo, g.estilo.trazo, "el color del panel");
        // Medida: 10 letras x 10 = 100 de ancho; centrado en (500, 300).
        assert_eq!(e.ancho, 100.0);
        assert_eq!(e.x, 450.0);
        assert!((e.y + e.alto / 2.0 - 300.0).abs() < 0.01);
        assert_eq!(g.seleccion.ids(), &[id], "queda elegido lo pegado");
        assert!(!g.esta_escribiendo(), "pegar no abre el texto");
        assert_eq!(
            g.herramienta,
            Herramienta::Rectangulo,
            "ni cambia la herramienta"
        );
    });
}

#[test]
fn varias_lineas_tildes_y_emojis_y_ajuste_por_palabras() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let id = g
            .pegar_texto(
                &mut escena,
                "Canción ñandú 😀\r\n\r\nsegundo párrafo con varias palabras\r\n",
                Punto2::nuevo(0.0, 0.0),
                200.0,
            )
            .unwrap();
        let t = texto_de(&escena, id);
        // Los renglones de mas de 200 (20 letras) se parten por palabras;
        // el renglon vacio entre parrafos se queda.
        assert_eq!(
            t,
            "Canción ñandú 😀\n\nsegundo párrafo con\nvarias palabras"
        );
    });
}

#[test]
fn un_solo_paso_de_deshacer() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.pegar_texto(
            &mut escena,
            "uno\ndos\ntres",
            Punto2::nuevo(0.0, 0.0),
            800.0,
        )
        .unwrap();
        assert_eq!(vivos(&escena), 1);
        assert!(escena.deshacer());
        assert_eq!(vivos(&escena), 0, "un deshacer se lo lleva entero");
        assert!(escena.rehacer());
        assert_eq!(vivos(&escena), 1);
    });
}

#[test]
fn escribiendo_un_texto_lo_pegado_entra_en_el_y_no_nace_otro() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Texto);
        let p = Punto2::nuevo(100.0, 50.0);
        g.evento(
            EventoGesto::Pulsar {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        g.evento(EventoGesto::Soltar { p }, &mut escena, 1.0);
        for c in "ab".chars() {
            g.escribir(c, &mut escena);
        }
        g.tecla_de_texto(texto::TeclaTexto::Izquierda, &mut escena);
        let abierto = g.texto_en_curso().unwrap().0;
        let id = g
            .pegar_texto(&mut escena, "XY\r\nZ", Punto2::nuevo(900.0, 900.0), 800.0)
            .unwrap();
        assert_eq!(id, abierto);
        assert_eq!(texto_de(&escena, id), "aXY\nZb", "en el cursor");
        assert_eq!(vivos(&escena), 1, "no nace otro texto");
        assert!(g.esta_escribiendo(), "se sigue escribiendo");
        // La caja sigue a lo escrito: 3 letras del renglon mas largo.
        assert_eq!(escena.buscar(id).unwrap().ancho, 30.0);
        // Y se sigue tecleando detras de lo pegado.
        g.escribir('!', &mut escena);
        assert_eq!(texto_de(&escena, id), "aXY\nZ!b");
    });
}

#[test]
fn nada_que_pegar_no_hace_nada() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        assert_eq!(
            g.pegar_texto(&mut escena, "", Punto2::nuevo(0.0, 0.0), 800.0),
            None
        );
        assert_eq!(
            g.pegar_texto(&mut escena, " \r\n\t\u{0}", Punto2::nuevo(0.0, 0.0), 800.0),
            None
        );
        assert_eq!(vivos(&escena), 0);
        assert!(!escena.deshacer(), "ni deja un paso vacio");
    });
}

#[test]
fn un_texto_enorme_se_queda_en_el_tope() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let enorme = "palabra ".repeat(10_000);
        let id = g
            .pegar_texto(&mut escena, &enorme, Punto2::nuevo(0.0, 0.0), 800.0)
            .unwrap();
        let t = texto_de(&escena, id);
        // Los saltos del ajuste ocupan lo que ocupaban los espacios.
        assert_eq!(t.chars().count(), TOPE_CARACTERES);
        assert!(t.ends_with('…'));
        assert!(t.lines().all(|r| r.chars().count() <= 80), "partido a 800");
    });
}

#[test]
fn se_guarda_en_el_fichero_como_cualquier_texto() {
    texto::con_medidor(medidor, || {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let id = g
            .pegar_texto(
                &mut escena,
                "Línea 1 😀\nhttps://ejemplo.com/a?b=c",
                Punto2::nuevo(10.0, 10.0),
                800.0,
            )
            .unwrap();
        let antes = escena.buscar(id).unwrap().clone();
        let json = excalidraw::escribir(&excalidraw::con_escena(
            &excalidraw::Lienzo::vacio(),
            &escena,
        ));
        let vuelta = excalidraw::a_escena(&excalidraw::leer(&json).unwrap());
        let e = vuelta.visibles().next().expect("el texto viajo");
        assert_eq!(
            texto_de(&vuelta, e.id),
            "Línea 1 😀\nhttps://ejemplo.com/a?b=c"
        );
        assert_eq!(e.trazo, antes.trazo);
        assert!((e.x - antes.x).abs() < 0.01 && (e.y - antes.y).abs() < 0.01);
    });
}
