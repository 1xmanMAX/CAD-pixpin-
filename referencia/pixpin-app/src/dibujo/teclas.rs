//! **Las teclas y los botones de las herramientas**, comunes a todos los
//! anfitriones (lienzo, lector, pin y pantalla).
//!
//! Vivian dentro de `ventana_editor.rs` y se movieron aqui tal cual el
//! 2026-09-24 (ver `docs/superpowers/specs/2026-09-24-herramientas-unicas-design.md`):
//! son puras, no dependen de la ventana del editor, y el lector o el
//! anotador de pantalla tienen que responder a las mismas letras que el
//! lienzo. Lo que cambia por anfitrion (que herramientas admite, cuales
//! apago el usuario) lo decide `permitidas`, no estas tablas.

use pixpin_geom::Punto;
use pixpin_motor2d::camara::Camara;
use pixpin_motor2d::escena::Escena;
use pixpin_motor2d::gesto::{EventoGesto, FormaCursor, Gesto, Herramienta, direccion_del_tirador};
use pixpin_motor2d::seleccion::{OrdenEditor, Tecla};
use pixpin_motor2d::vector::Punto2;
use pixpin_shell::overlay::{EventoOverlay, FormaCursorWin};
use pixpin_ui::BotonCaja;

/// De la forma que pide el motor a la que entiende Windows.
///
/// Lo unico con sustancia es escalar: Windows solo trae cuatro flechas de
/// redimension, asi que la direccion del tirador —ya girada con el
/// elemento— se reparte entre ellas en octavos de vuelta. Y como una flecha
/// no tiene punta, norte y sur son la misma: se toma el angulo modulo media
/// vuelta.
pub fn forma_de(cursor: FormaCursor) -> FormaCursorWin {
    use std::f32::consts::PI;
    match cursor {
        FormaCursor::Flecha => FormaCursorWin::Flecha,
        FormaCursor::Cruz => FormaCursorWin::Cruz,
        FormaCursor::Mover => FormaCursorWin::Mover,
        FormaCursor::Texto => FormaCursorWin::Texto,
        FormaCursor::Giro => FormaCursorWin::Giro,
        FormaCursor::Escalar { tirador, angulo } => {
            let d = direccion_del_tirador(tirador, angulo);
            // A media vuelta, y en octavos: cada flecha cubre 45 grados.
            let media = PI;
            let d = d.rem_euclid(media);
            let octavo = media / 4.0;
            match (d / octavo).round() as i32 % 4 {
                0 => FormaCursorWin::RedimNS,
                1 => FormaCursorWin::RedimNeSo,
                2 => FormaCursorWin::RedimEO,
                _ => FormaCursorWin::RedimNoSe,
            }
        }
    }
}

/// Del evento de la ventana al del motor. `None` es «esto no le toca al
/// motor»: pintar, el DPI, el despertar de otro hilo.
///
/// `shift` y `alt` del raton salen a `false`: `EventoOverlay::BotonPulsado` y
/// `RatonMovido` no los traen (no son parte del mensaje de Windows). Quien
/// llama los sobreescribe con `con_modificadores` justo antes de pasarselo a
/// la maquina, leyendolos con `pixpin_shell::entrada::modificadores` en el
/// momento de traducir.
///
/// `origen` es la esquina de la ventana en coordenadas del escritorio
/// virtual: los mensajes de Windows traen esas coordenadas, pero el lienzo
/// empieza en la esquina de la ventana. Con la barra de tareas arriba o a la
/// izquierda el area de trabajo no empieza en (0,0), y sin restar el origen
/// la tinta salia desplazada del cursor.
pub fn a_evento(ev: &EventoOverlay, camara: &Camara, origen: Punto) -> Option<EventoGesto> {
    let (ox, oy) = (origen.x as f32, origen.y as f32);
    // `a_mundo` convierte un punto. NO `en_mundo`, que existe y convierte una
    // LONGITUD: compila igual y da otra cosa.
    let al_mundo = |x: f32, y: f32| camara.a_mundo(Punto2::nuevo(x - ox, y - oy));
    let entero = |p: &Punto| al_mundo(p.x as f32, p.y as f32);
    match ev {
        EventoOverlay::BotonPulsado(p) => Some(EventoGesto::Pulsar {
            p: entero(p),
            shift: false,
            alt: false,
            presion: None,
        }),
        EventoOverlay::RatonMovido(p) => Some(EventoGesto::Mover {
            p: entero(p),
            shift: false,
            alt: false,
            presion: None,
        }),
        // Con lapiz, `Muestra` puede llegar ANTES de `BotonPulsado` y un
        // trazo puede no traer `RatonMovido` de cola: cada muestra se
        // traduce sola, con su subpixel y su presion, como `Mover`.
        EventoOverlay::Muestra(m) => Some(EventoGesto::Mover {
            p: al_mundo(m.x(), m.y()),
            shift: false,
            alt: false,
            presion: m.presion(),
        }),
        EventoOverlay::BotonSoltado(p) => Some(EventoGesto::Soltar { p: entero(p) }),
        EventoOverlay::Tecla { vk, ctrl, .. } => {
            const VK_ESCAPE: u32 = 0x1B;
            const VK_DELETE: u32 = 0x2E;
            match (*vk, *ctrl) {
                (VK_ESCAPE, _) => Some(EventoGesto::Escape),
                (VK_DELETE, _) => Some(EventoGesto::Suprimir),
                (v, true) if v == b'Z' as u32 => Some(EventoGesto::Deshacer),
                (v, true) if v == b'Y' as u32 => Some(EventoGesto::Rehacer),
                (v, true) if v == b'A' as u32 => Some(EventoGesto::SeleccionarTodo),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Sobreescribe `shift` y `alt` de un `Pulsar`/`Mover` con lo que hay
/// pulsado AHORA. `a_evento` se queda puro y comprobable; esto es lo unico
/// que necesita preguntarle al sistema, y solo para dos campos.
pub(crate) fn con_modificadores(g: EventoGesto) -> EventoGesto {
    match g {
        EventoGesto::Pulsar { p, presion, .. } => {
            let (shift, alt) = pixpin_shell::entrada::modificadores();
            EventoGesto::Pulsar {
                p,
                shift,
                alt,
                presion,
            }
        }
        EventoGesto::Mover { p, presion, .. } => {
            // **Ctrl mantenido = figura perfecta**, igual que Mayus (lo pidio
            // el usuario el 2026-09-22): cuadrado y circulo exactos, lineas y
            // flechas a saltos de 15 grados, escalar en proporcion y girar a
            // saltos. Solo al arrastrar y no al pulsar: Ctrl+clic no debe
            // cambiar la seleccion como Mayus+clic.
            let m = pixpin_shell::entrada::modificadores_pulsados();
            EventoGesto::Mover {
                p,
                shift: m.shift || m.ctrl,
                alt: m.alt,
                presion,
            }
        }
        otro => otro,
    }
}

/// La herramienta que elige cada letra, siguiendo `caja_dibujo::etiqueta`.
///
/// Solo las herramientas que ahi se pintan con una letra de verdad tienen
/// atajo: Linea, Flecha, Rectangulo y Elipse se pintan con un simbolo
/// ("/", ">", "square", "circle") porque no hay icono todavia, y un simbolo
/// no es una tecla memorizable. Pura, para poder probarla sin ventana.
pub(crate) fn tecla_a_herramienta(c: char) -> Option<Herramienta> {
    match c.to_ascii_uppercase() {
        'M' => Some(Herramienta::Mano),
        'L' => Some(Herramienta::Lapiz),
        'R' => Some(Herramienta::Resaltador),
        'T' => Some(Herramienta::Texto),
        'F' => Some(Herramienta::Foco),
        'Q' => Some(Herramienta::Lupa),
        'B' => Some(Herramienta::Borrador),
        'A' => Some(Herramienta::Cota),
        'E' => Some(Herramienta::Escalar),
        'G' => Some(Herramienta::EscalaGrafica),
        // «C» de marCo: la «F» de «frame» ya la usa el foco.
        // La «I» tambien, como en el plano y en todos los visores (19-oct-2026).
        'C' | 'I' => Some(Herramienta::Marco),
        // «P» de «pencil»: la «G» del grafito ya es la escala grafica.
        'P' => Some(Herramienta::Grafito),
        // «Z» de zona. El laser es la «K» en Excalidraw, pero aqui la «K»
        // es el cuentagotas del motor: «J», la de al lado.
        'Z' => Some(Herramienta::Zona),
        'J' => Some(Herramienta::Laser),
        // La marca de angulo con su letra griega (17-oct): la «W» se parece a
        // la ω, y la «A» de angulo ya es la cota. Pulsada otra vez con el
        // angulo en la mano, cambia al cuadradito de 90 (`dibujo::mano`).
        'W' => Some(Herramienta::Angulo),
        _ => None,
    }
}

/// La tecla que elige `h`, para pintarla en la esquina de su boton. Es la
/// inversa de `tecla_a_herramienta`: si alguna vez se separan, la barra
/// ensenaria una tecla que no hace nada, y la prueba lo vigila.
pub(crate) fn tecla_de(h: Herramienta) -> Option<char> {
    [
        'M', 'L', 'R', 'T', 'F', 'Q', 'B', 'A', 'E', 'G', 'P', 'Z', 'J', 'W',
    ]
    .into_iter()
    .find(|c| tecla_a_herramienta(*c) == Some(h))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CambioPluma {
    Grosor(f32),
    AlternarVariabilidad,
}

/// Las plumas de Excalidraw sin interfaz nueva (la barra es E5): 1, 2 y 3
/// son sus tres grosores; V alterna variable y constante. Ninguna choca con
/// `tecla_a_herramienta`, y la prueba lo vigila.
pub(crate) fn tecla_a_pluma(c: char) -> Option<CambioPluma> {
    use pixpin_motor2d::tinta::{GROSOR_FINO, GROSOR_GRUESO, GROSOR_MEDIO};
    match c.to_ascii_uppercase() {
        '1' => Some(CambioPluma::Grosor(GROSOR_FINO)),
        '2' => Some(CambioPluma::Grosor(GROSOR_MEDIO)),
        '3' => Some(CambioPluma::Grosor(GROSOR_GRUESO)),
        'V' => Some(CambioPluma::AlternarVariabilidad),
        _ => None,
    }
}

/// Que hacer al pulsar un boton de la caja del editor. Pura -no toca la
/// ventana ni pinta nada-, asi se prueba sin GPU ni sesion de escritorio.
/// Mismo contrato que `CapaViva::pulsar_boton` en `capa.rs`: devuelve
/// `false` si el boton pide salir.
pub(crate) fn pulsar_boton(boton: BotonCaja, gesto: &mut Gesto, escena: &mut Escena) -> bool {
    match boton {
        BotonCaja::Elegir(h) => {
            elegir_herramienta(gesto, h);
            true
        }
        BotonCaja::Deshacer => {
            escena.deshacer();
            true
        }
        BotonCaja::Rehacer => {
            escena.rehacer();
            true
        }
        // Sin paleta de colores en el editor todavia.
        BotonCaja::Color => true,
        BotonCaja::Salir => false,
        // Las dos las atiende el anfitrion, que es quien sabe abrir un
        // selector de ficheros o un menu (`mano::Atendido::pedido`).
        BotonCaja::Imagen | BotonCaja::Figuras | BotonCaja::Imprimir | BotonCaja::Compartir => true,
        // El clic a traves lo cambia el anotador de pantalla (`Pedido::Atravesar`).
        BotonCaja::Atravesar => true,
        // Desplegar un grupo lo decide la mano (`dibujo::grupos::pulsar`).
        BotonCaja::Grupo(_) => true,
    }
}

/// Cambia de herramienta y suelta lo elegido si la nueva dibuja.
///
/// Sin esto, con algo elegido y el lapiz en la mano quedaban unos tiradores
/// flotando sobre el dibujo: el clic encima ya no los mueve (eso es solo de
/// la mano), asi que serian unos agarres que no hacen lo que prometen.
pub(crate) fn elegir_herramienta(gesto: &mut Gesto, h: Herramienta) {
    if h != Herramienta::Mano {
        gesto.seleccion.limpiar();
    }
    // Un lazo a medio trazar que sobrevive al cambio de herramienta es un
    // rastro mintiendo: dejaria de crecer y seguiria pintado.
    gesto.lazo = None;
    // Lo mismo con el rectangulo de la zona y la estela del laser.
    gesto.zona = None;
    gesto.laser.vaciar();
    // Por el gesto y no a pelo: coger el grafito deja las figuras de grafito
    // hasta coger el lapiz (`Gesto::tomar_herramienta`).
    gesto.tomar_herramienta(h);
}

/// **Traduce la tecla de Windows a la del motor.**
///
/// Vive aqui y no en el motor porque `VK_OEM_4`/`VK_OEM_6` son numeros de
/// Windows, y el motor no sabe de Windows. Solo las teclas que la tabla usa:
/// lo demas devuelve `None` y sigue su camino.
pub(crate) fn tecla_del_motor(vk: u32) -> Option<Tecla> {
    // Los corchetes en el teclado de EE. UU.; en otras distribuciones el
    // mismo codigo cae en otra tecla fisica, que es lo que hace Windows con
    // todos los atajos y lo que el usuario espera.
    const VK_OEM_4: u32 = 0xDB;
    const VK_OEM_6: u32 = 0xDD;
    match vk {
        VK_OEM_4 => Some(Tecla::CorcheteAbre),
        VK_OEM_6 => Some(Tecla::CorcheteCierra),
        v if (b'A' as u32..=b'Z' as u32).contains(&v) => {
            Some(Tecla::Letra((v as u8 as char).to_ascii_lowercase()))
        }
        _ => None,
    }
}

/// **Ejecuta una orden de la tabla de atajos.** `true` si algo cambio.
///
/// Es la contrapartida de `atajo_de`: alli estan las teclas y aqui lo que
/// hacen, porque quien tiene la escena, la seleccion y el historial es la
/// ventana. Pura salvo por lo que toca de `gesto` y `escena`, asi que se
/// prueba sin GPU y sin sintetizar una pulsacion.
pub(crate) fn aplicar_orden(orden: OrdenEditor, gesto: &mut Gesto, escena: &mut Escena) -> bool {
    use pixpin_motor2d::organizar;
    use pixpin_motor2d::transformar::{EjeVolteo, voltear};

    // Las que piden seleccion no hacen nada sin ella, en vez de hacer algo
    // raro: `necesita_seleccion` es la misma tabla que lo declara.
    if orden.necesita_seleccion() && gesto.seleccion.esta_vacia() {
        return false;
    }

    match orden {
        OrdenEditor::Lazo => {
            elegir_herramienta(gesto, Herramienta::Lazo);
            true
        }
        OrdenEditor::CopiarEstilo => {
            elegir_herramienta(gesto, Herramienta::CopiarEstilo);
            true
        }
        OrdenEditor::TomarEstilo => {
            // Del primero elegido: con varios, el de mas abajo en el orden
            // de pintado es el que el usuario «ve» como el modelo.
            let Some(e) = gesto
                .seleccion
                .ids()
                .first()
                .and_then(|id| escena.buscar(*id))
            else {
                return false;
            };
            gesto.estilo_tomado = Some(pixpin_motor2d::estilo::copiar(e));
            true
        }
        OrdenEditor::SoltarEstilo => match &gesto.estilo_tomado {
            None => false,
            Some(copiado) => pixpin_motor2d::estilo::pegar_a(escena, &gesto.seleccion, copiado) > 0,
        },
        OrdenEditor::VoltearHorizontal | OrdenEditor::VoltearVertical => {
            let eje = if orden == OrdenEditor::VoltearHorizontal {
                EjeVolteo::Horizontal
            } else {
                EjeVolteo::Vertical
            };
            volteando(escena, gesto, eje, voltear)
        }
        OrdenEditor::Agrupar => {
            escena.abrir_paso();
            let hecho = organizar::agrupar(escena, &gesto.seleccion).is_some();
            escena.cerrar_paso();
            hecho
        }
        OrdenEditor::Desagrupar => {
            escena.abrir_paso();
            organizar::desagrupar(escena, &gesto.seleccion);
            escena.cerrar_paso();
            true
        }
        OrdenEditor::AlFrente => {
            escena.abrir_paso();
            organizar::al_frente(escena, &gesto.seleccion);
            escena.cerrar_paso();
            true
        }
        OrdenEditor::AlFondo => {
            escena.abrir_paso();
            organizar::al_fondo(escena, &gesto.seleccion);
            escena.cerrar_paso();
            true
        }
        OrdenEditor::Subir => {
            escena.abrir_paso();
            organizar::adelante(escena, &gesto.seleccion);
            escena.cerrar_paso();
            true
        }
        OrdenEditor::Bajar => {
            escena.abrir_paso();
            organizar::atras(escena, &gesto.seleccion);
            escena.cerrar_paso();
            true
        }
        OrdenEditor::AlternarIman => {
            gesto.enganche.activo = !gesto.enganche.activo;
            true
        }
    }
}

/// Voltea lo elegido en un solo paso de deshacer.
///
/// `transformar::voltear` trabaja sobre un trozo de elementos y aqui hay una
/// escena con historial, asi que los elegidos se sacan, se voltean y se
/// devuelven a su sitio. Se sacan por ORDEN de la escena y no por el de la
/// seleccion: el espejo va alrededor de la caja comun y esa no depende del
/// orden, pero devolverlos cruzados si cambiaria el orden de pintado.
pub(crate) fn volteando(
    escena: &mut Escena,
    gesto: &Gesto,
    eje: pixpin_motor2d::transformar::EjeVolteo,
    hacer: fn(&mut [pixpin_motor2d::elemento::Elemento], pixpin_motor2d::transformar::EjeVolteo),
) -> bool {
    let sitios: Vec<usize> = escena
        .elementos
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.borrado && gesto.seleccion.contiene(e.id))
        .map(|(i, _)| i)
        .collect();
    if sitios.is_empty() {
        return false;
    }
    escena.abrir_paso();
    // Apuntados ANTES de tocarlos: `apuntar_edicion` guarda el elemento tal
    // como esta **ahora**, asi que hacerlo despues del volteo guardaria el
    // volteado y deshacer no devolveria nada.
    let ids: Vec<u64> = sitios.iter().map(|i| escena.elementos[*i].id).collect();
    for &id in &ids {
        escena.apuntar_edicion(id);
    }
    let mut copia: Vec<_> = sitios
        .iter()
        .map(|i| escena.elementos[*i].clone())
        .collect();
    hacer(&mut copia, eje);
    for (i, mut e) in sitios.iter().zip(copia) {
        // Sube la version, que es lo que le dice al otro aparato que esto se
        // ha movido. Sin ello, un volteo no viajaria al movil.
        e.tocar();
        escena.elementos[*i] = e;
    }
    // Las flechas atadas, en este mismo paso: voltear una caja sin que su
    // flecha la siga, o voltear la flecha con un enganche del lado de antes,
    // la descolocaria al primer movimiento.
    pixpin_motor2d::transformar::atar_tras_voltear(escena, &ids);
    escena.cerrar_paso();
    true
}
