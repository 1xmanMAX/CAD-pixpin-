//! **Hornear la capa congelada**: lo que no se mueve, pintado una vez para
//! copiarlo en cada fotograma mientras algo cambia encima (D120). Estaba
//! dentro del bucle del editor; aparte porque ahora lo piden dos gestos:
//! mover lo elegido y apuntar una lupa a otro sitio (`dibujo::lupa`), que
//! antes rehacia la escena entera en cada aviso del raton.

use super::*;

/// Hornea en `capa` todo lo que se ve con `efectiva` menos lo excluido del
/// gesto (`excluidos`), del tamano de la superficie de escena (ventana mas
/// colchon `margen_escena`).
#[allow(clippy::too_many_arguments)]
pub(super) fn hornear(
    motor: &mut MotorRender,
    capa: &mut CapaEstatica,
    escena: &Escena,
    gesto: &Gesto,
    rejilla: &mut Rejilla,
    cache: &mut Cache,
    cache_tinta: &mut pixpin_render::CacheTinta,
    fondo: &mut Option<FondoLienzo>,
    imagenes: &mut ImagenesLienzo,
    efectiva: &Camara,
    ancho_px: f32,
    alto_px: f32,
    margen_escena: f32,
    excluidos: &[u64],
) {
    rejilla.sincronizar(escena);
    // A3: la capa congelada es del tamano de la SUPERFICIE
    // de escena (ventana mas colchon) y se hornea con el
    // mismo desplazamiento; si no, `volcar_zona` copiaria
    // el trozo equivocado.
    let h = margen_escena / efectiva.zoom;
    let v = efectiva.ventana(ancho_px, alto_px);
    let vista = (v.0 - h, v.1 - h, v.2 + h, v.3 + h);
    let candidatos = rejilla.candidatos(vista);
    let estampa = Estampa {
        camara: (efectiva.x, efectiva.y, efectiva.zoom),
        tamano: tamano_escena(ancho_px, alto_px, margen_escena),
        excluidos: excluidos.to_vec(),
    };
    if let Some(f) = fondo.as_mut() {
        f.asegurar(motor);
    }
    // Lo que no este subido cuando se hornea la capa se
    // queda fuera de ella, y la capa sigue valiendo: seria
    // una imagen que no aparece hasta soltar el raton.
    imagenes.asegurar(motor);
    let imagenes: &ImagenesLienzo = imagenes;
    let _ = capa.preparar(motor, estampa, |p| {
        p.limpiar(a_color(escena.fondo)); // el papel del lienzo, no blanco
        let origen = efectiva.a_pantalla(Punto2::nuevo(0.0, 0.0));
        p.poner_vista(
            (0.0, 0.0),
            efectiva.zoom,
            (origen.x + margen_escena, origen.y + margen_escena),
        );
        // D140: la imagen se pinta primera y entra en la capa:
        // mientras se dibuja, no cuesta nada por fotograma.
        if let Some(f) = fondo.as_ref() {
            f.pintar(p, vista, efectiva.zoom);
        }
        for id in candidatos {
            if es_excluido(gesto, id) {
                continue;
            }
            let Some(e) = elemento_por_id(escena, rejilla, id) else {
                continue;
            };
            if e.borrado {
                continue;
            }
            // El mosaico no entra en la capa congelada por lo
            // mismo que no entra en el pintado normal: lo que
            // tapa es lo que hay DEBAJO, y la pasada de
            // tapado corre sobre el destino ya volcado, en
            // cada fotograma. Si aqui se pintara su banda, la
            // pasada leeria la banda.
            if tapar::es_mosaico(e) {
                continue;
            }
            // Las dos llamadas -aqui y en `pintar`- tienen
            // que usar la MISMA funcion: lo que no pase por
            // `por_cada_orden` no entra en la capa, y
            // `pintar` ya no lo repinta mientras la capa
            // valga (ve su comentario para el porque).
            let mut indice = 0u32;
            // De que esta hecha su tinta. `None` en la
            // lisa y en las encendidas, que no llevan tela.
            let grano = pixpin_motor2d::pintado::grano_de(e);
            por_cada_orden(cache, e, efectiva.zoom, escena.escala.as_ref(), |orden| {
                dibujar_orden(
                    p,
                    orden,
                    vista,
                    Some((&mut *cache_tinta, (e.id, e.version, indice))),
                    imagenes,
                    efectiva.zoom,
                    grano,
                );
                indice += 1;
            });
        }
    });
}

/// **El trozo de ventana de lo elegido mientras se estira o se gira** por
/// sus tiradores, si basta con rehacer ese trozo encima de la capa congelada
/// (con el trozo de antes, que quien llama une). Estirar pedia la escena
/// entera en cada aviso del raton aunque lo unico que cambiaba era lo
/// elegido. `None` si no basta: flechas atadas que cambian fuera de lo
/// elegido, una lupa que lo mira (lo de dentro cambiaria fuera del trozo) o
/// un mosaico encima (su pasada lee lo de debajo), o algo que
/// `zona_de_seleccion` no sabe acotar.
pub(super) fn zona_al_transformar(
    escena: &Escena,
    gesto: &Gesto,
    camara: &Camara,
    cache: &mut Cache,
) -> Option<(i32, i32, i32, i32)> {
    if !gesto.transformando() || !gesto.flechas_que_siguen().is_empty() {
        return None;
    }
    let caja = gesto.seleccion.caja(escena)?;
    let pisa_mosaico = escena.elementos.iter().any(|m| {
        !m.borrado && tapar::es_mosaico(m) && {
            let c = pixpin_motor2d::mosaico::caja_girada(m);
            c.0 <= caja.2 && c.2 >= caja.0 && c.1 <= caja.3 && c.3 >= caja.1
        }
    });
    // Una lupa que se estira o se gira cambia lo de dentro: tampoco.
    let hay_lupa = gesto.seleccion.ids().iter().any(|id| {
        escena
            .buscar(*id)
            .is_some_and(|e| matches!(e.figura, Figura::Lupa { .. }))
    });
    if pisa_mosaico || hay_lupa || lupas::alguna_mira(&escena.elementos, caja) {
        return None;
    }
    zona_de_seleccion(escena, gesto, camara, cache, (0.0, 0.0))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_motor2d::gesto::EventoGesto;

    fn caja_elegida_estirandose() -> (Escena, Gesto) {
        let mut escena = Escena::nueva();
        let id = escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            x: 100.0,
            y: 100.0,
            ancho: 200.0,
            alto: 100.0,
            grosor: 2.0,
            ..Default::default()
        });
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Mano);
        g.seleccion.poner(id);
        let q = g.tiradores(&escena, 1.0).expect("tiradores").tamano[4].1;
        g.evento(
            EventoGesto::Pulsar {
                p: q,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        let p = Punto2::nuevo(q.x + 30.0, q.y + 20.0);
        g.evento(
            EventoGesto::Mover {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        (escena, g)
    }

    #[test]
    fn estirar_una_figura_rehace_solo_su_trozo_de_ventana() {
        let (escena, g) = caja_elegida_estirandose();
        assert!(g.transformando());
        let z = zona_al_transformar(&escena, &g, &Camara::nueva(), &mut Cache::nueva())
            .expect("se acota");
        // Cubre la caja estirada (100,100)-(330,220) y poco mas: no la ventana.
        assert!(
            z.0 <= 100 && z.1 <= 100 && z.2 >= 330 && z.3 >= 220,
            "{z:?}"
        );
        assert!(
            z.2 - z.0 < 400 && z.3 - z.1 < 300,
            "demasiado grande: {z:?}"
        );
    }

    fn marco_estirandose(nombre: &str) -> (Escena, Gesto) {
        let mut escena = Escena::nueva();
        let id = escena.anadir(Elemento {
            figura: Figura::Marco {
                nombre: nombre.into(),
            },
            x: 100.0,
            y: 100.0,
            ancho: 300.0,
            alto: 200.0,
            grosor: 1.5,
            ..Default::default()
        });
        let mut g = Gesto::nuevo();
        g.tomar_herramienta(Herramienta::Mano);
        g.seleccion.poner(id);
        let q = g.tiradores(&escena, 1.0).expect("tiradores").tamano[4].1;
        g.evento(
            EventoGesto::Pulsar {
                p: q,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        let p = Punto2::nuevo(q.x + 30.0, q.y + 20.0);
        g.evento(
            EventoGesto::Mover {
                p,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        (escena, g)
    }

    #[test]
    fn estirar_un_marco_con_nombre_rehace_su_trozo_con_el_nombre_y_no_la_escena_entera() {
        let (escena, g) = marco_estirandose("Lamina");
        assert!(g.transformando());
        let z = zona_al_transformar(&escena, &g, &Camara::nueva(), &mut Cache::nueva())
            .expect("se acota");
        // El nombre va encima de la raya de arriba (y = 100 - 16): tiene que
        // entrar en el trozo, y el marco estirado (100,100)-(430,320) tambien.
        let alto_nombre = 16;
        assert!(z.1 <= 100 - alto_nombre, "el nombre entra: {z:?}");
        assert!(z.0 <= 100 && z.2 >= 430 && z.3 >= 320, "{z:?}");
        assert!(z.2 - z.0 < 500 && z.3 - z.1 < 400, "no la ventana: {z:?}");
    }

    #[test]
    fn un_nombre_de_marco_largo_ya_no_obliga_a_la_escena_entera() {
        // Antes el nombre, partido al ancho del marco, bajaba por debajo de su
        // raya y obligaba a la escena entera. Desde el 22-oct el marco y su
        // nombre los pinta el editor aparte (`marcas::pintar_marcos`): el
        // trozo es el del marco.
        let (escena, g) = marco_estirandose(&"Lamina muy larga ".repeat(40));
        let z = zona_al_transformar(&escena, &g, &Camara::nueva(), &mut Cache::nueva())
            .expect("se acota");
        assert!(z.0 <= 100 && z.2 >= 430 && z.3 >= 320, "{z:?}");
    }

    #[test]
    fn con_una_lupa_mirando_lo_que_se_estira_se_rehace_la_escena_entera() {
        let (mut escena, g) = caja_elegida_estirandose();
        // Una lupa apartada que mira la caja: lo de dentro cambia fuera del
        // trozo de lo elegido.
        escena.anadir(Elemento {
            figura: Figura::Lupa {
                cristal: pixpin_motor2d::lupa_elemento::Cristal {
                    foco: Some(Punto2::nuevo(150.0, 150.0)),
                    aumento: Some(2.0),
                    foco_ancho: Some(50.0),
                    foco_alto: Some(50.0),
                    ..Default::default()
                },
            },
            x: 600.0,
            y: 0.0,
            ancho: 100.0,
            alto: 100.0,
            ..Default::default()
        });
        assert_eq!(
            zona_al_transformar(&escena, &g, &Camara::nueva(), &mut Cache::nueva()),
            None
        );
    }

    #[test]
    fn una_lupa_que_mira_a_otro_sitio_no_estorba_aunque_su_cristal_este_debajo() {
        let (mut escena, g) = caja_elegida_estirandose();
        escena.anadir(Elemento {
            figura: Figura::Lupa {
                cristal: pixpin_motor2d::lupa_elemento::Cristal {
                    foco: Some(Punto2::nuevo(900.0, 900.0)),
                    aumento: Some(2.0),
                    foco_ancho: Some(50.0),
                    foco_alto: Some(50.0),
                    ..Default::default()
                },
            },
            x: 150.0,
            y: 150.0,
            ancho: 100.0,
            alto: 100.0,
            ..Default::default()
        });
        assert!(zona_al_transformar(&escena, &g, &Camara::nueva(), &mut Cache::nueva()).is_some());
    }

    #[test]
    fn quieto_o_moviendo_no_hay_trozo_de_transformar() {
        let mut escena = Escena::nueva();
        let id = escena.anadir(Elemento {
            figura: Figura::Rectangulo,
            ancho: 50.0,
            alto: 50.0,
            ..Default::default()
        });
        let mut g = Gesto::nuevo();
        g.seleccion.poner(id);
        assert_eq!(
            zona_al_transformar(&escena, &g, &Camara::nueva(), &mut Cache::nueva()),
            None
        );
    }
}
