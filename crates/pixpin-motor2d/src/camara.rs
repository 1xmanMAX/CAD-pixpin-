//! La camara: por donde se mira un lienzo que no tiene bordes.
//!
//! Un lienzo infinito no cabe en la pantalla por definicion, asi que hay dos
//! sistemas de coordenadas y conviene tenerlos separados en la cabeza:
//!
//! - **El mundo.** Donde viven los elementos. No tiene origen ni limites; el
//!   (0,0) es un sitio cualquiera por el que se empezo a dibujar.
//! - **La pantalla.** Pixeles, con el (0,0) en la esquina de arriba a la
//!   izquierda del hueco donde se pinta.
//!
//! La camara es lo unico que traduce entre los dos, y son tres numeros: que
//! punto del mundo cae en esa esquina, y cuanto se aumenta.
//!
//! # Por que la geometria se queda en el mundo
//!
//! La tentacion es convertir cada punto a pantalla antes de pintar. Seria un
//! error caro: encuadrar moveria ocho mil puntos por fotograma. Direct2D
//! tiene matriz de transformacion, asi que la geometria se calcula **una vez
//! en coordenadas del mundo** y encuadrar o acercar solo cambia esos tres
//! numeros. Es lo que hace que mover el lienzo no cueste nada.
//!
//! Por eso aqui no hay ninguna funcion que transforme elementos. Solo hay
//! conversion de puntos sueltos —hace falta para saber donde pincho el
//! raton— y el recorte, que decide **que** se pinta y no **como**.

use crate::elemento::Elemento;
use crate::escena::Escena;
use crate::vector::Punto2;

/// Aumento minimo. Por debajo de esto no se distingue nada y solo sirve para
/// perderse: el usuario cree que ha borrado el dibujo cuando lo que ha hecho
/// es alejarse tanto que mide un pixel.
pub const ZOOM_MINIMO: f32 = 0.05;

/// Aumento maximo. Treinta aumentos ya es mas fino que el subpixel de
/// cualquier trazo; mas alla solo se ven los dientes de la geometria.
pub const ZOOM_MAXIMO: f32 = 30.0;

/// Por donde se mira el lienzo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camara {
    /// Punto del mundo que cae en la esquina superior izquierda.
    pub x: f32,
    pub y: f32,
    /// Pixeles de pantalla por unidad de mundo.
    pub zoom: f32,
}

impl Default for Camara {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Camara {
    /// El origen del mundo arriba a la izquierda, sin aumento.
    pub const fn nueva() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }

    /// La camara guardada en un fichero del movil.
    ///
    /// El Android guarda `scrollX`/`scrollY` con el convenio de Excalidraw:
    /// `pantalla = (mundo + scroll) * zoom`. Es el mismo punto que el nuestro
    /// con el signo cambiado, porque nosotros guardamos **donde esta** la
    /// esquina y ellos **cuanto se ha desplazado**.
    pub fn de_excalidraw(scroll_x: f32, scroll_y: f32, zoom: f32) -> Self {
        Self {
            x: -scroll_x,
            y: -scroll_y,
            zoom: zoom.clamp(ZOOM_MINIMO, ZOOM_MAXIMO),
        }
    }

    /// Los tres numeros como los espera el movil: `(scrollX, scrollY, zoom)`.
    pub fn a_excalidraw(&self) -> (f32, f32, f32) {
        (-self.x, -self.y, self.zoom)
    }

    /// De coordenadas del mundo a pixeles de pantalla.
    pub fn a_pantalla(&self, p: Punto2) -> Punto2 {
        Punto2::nuevo((p.x - self.x) * self.zoom, (p.y - self.y) * self.zoom)
    }

    /// De pixeles de pantalla a coordenadas del mundo. Lo que hace falta para
    /// saber donde ha pinchado el raton.
    pub fn a_mundo(&self, p: Punto2) -> Punto2 {
        Punto2::nuevo(p.x / self.zoom + self.x, p.y / self.zoom + self.y)
    }

    /// El trozo de mundo que se ve en un hueco de este tamano, como
    /// `(x0, y0, x1, y1)`.
    pub fn ventana(&self, ancho_px: f32, alto_px: f32) -> (f32, f32, f32, f32) {
        (
            self.x,
            self.y,
            self.x + ancho_px / self.zoom,
            self.y + alto_px / self.zoom,
        )
    }

    /// Arrastra el lienzo. Se le dan pixeles —es lo que ha movido el raton— y
    /// se convierten a mundo: con el doble de aumento, el mismo gesto tiene
    /// que recorrer la mitad de dibujo.
    pub fn desplazar(&mut self, dx_px: f32, dy_px: f32) {
        self.x -= dx_px / self.zoom;
        self.y -= dy_px / self.zoom;
    }

    /// Acerca o aleja dejando quieto el punto de pantalla que se le diga.
    ///
    /// Es el detalle que separa un lienzo que se maneja de uno que marea: la
    /// rueda tiene que aumentar **hacia donde apunta el raton**, no hacia el
    /// centro. Se consigue exigiendo que el punto del mundo que habia bajo el
    /// cursor siga estando bajo el cursor despues.
    ///
    /// Devuelve `false` si el aumento ya estaba en el tope y no cambio nada:
    /// asi quien llama puede ahorrarse repintar.
    pub fn acercar_en(&mut self, foco_px: Punto2, factor: f32) -> bool {
        self.acercar_en_entre(foco_px, factor, ZOOM_MINIMO, ZOOM_MAXIMO)
    }

    /// Como `acercar_en`, con topes propios. El universo deja alejarse mas
    /// que un dibujo: en un dibujo, alejarse tanto es perderse; en el
    /// universo es ver todos los proyectos a la vez.
    pub fn acercar_en_entre(
        &mut self,
        foco_px: Punto2,
        factor: f32,
        minimo: f32,
        maximo: f32,
    ) -> bool {
        let nuevo = (self.zoom * factor).clamp(minimo, maximo);
        if nuevo == self.zoom {
            return false;
        }
        let antes = self.a_mundo(foco_px);
        self.zoom = nuevo;
        let despues = self.a_mundo(foco_px);
        self.x += antes.x - despues.x;
        self.y += antes.y - despues.y;
        true
    }

    /// La camara que encaja una caja del mundo en un hueco, con holgura.
    ///
    /// Se usa el menor de los dos aumentos para que quepa entero por los dos
    /// lados, y se centra lo que sobra. Una caja sin tamano o un hueco de
    /// cero no tienen encuadre posible: se devuelve la camara de partida.
    pub fn encajar(caja: (f32, f32, f32, f32), ancho_px: f32, alto_px: f32, holgura: f32) -> Self {
        Self::encajar_entre(caja, ancho_px, alto_px, holgura, ZOOM_MINIMO, ZOOM_MAXIMO)
    }

    /// Como `encajar`, con topes de zoom propios (los del universo son mas
    /// amplios que los de un dibujo).
    pub fn encajar_entre(
        caja: (f32, f32, f32, f32),
        ancho_px: f32,
        alto_px: f32,
        holgura: f32,
        minimo: f32,
        maximo: f32,
    ) -> Self {
        let (x0, y0, x1, y1) = caja;
        let (ancho, alto) = (x1 - x0, y1 - y0);
        let (util_x, util_y) = (ancho_px - 2.0 * holgura, alto_px - 2.0 * holgura);
        if ancho <= 0.0 || alto <= 0.0 || util_x <= 0.0 || util_y <= 0.0 {
            return Self::nueva();
        }
        let zoom = (util_x / ancho).min(util_y / alto).clamp(minimo, maximo);
        // Lo que sobra a cada lado, en mundo, para que quede centrado.
        let sobra_x = (ancho_px / zoom - ancho) / 2.0;
        let sobra_y = (alto_px / zoom - alto) / 2.0;
        Self {
            x: x0 - sobra_x,
            y: y0 - sobra_y,
            zoom,
        }
    }

    /// Cuanto mide en el mundo lo que en pantalla mide `px` pixeles.
    ///
    /// Es la conversion que necesita cualquier tolerancia: la holgura para
    /// acertar con el raton, o cuanto se puede simplificar un trazo sin que
    /// se note. Todas esas se piensan en pixeles —son de lo que ve el ojo— y
    /// se aplican en el mundo.
    pub fn en_mundo(&self, px: f32) -> f32 {
        px / self.zoom
    }
}

/// Si dos cajas `(x0, y0, x1, y1)` se tocan.
///
/// Se cuenta tocarse por el borde: un elemento justo en el limite se pinta.
/// Equivocarse hacia pintar de mas es invisible; hacia pintar de menos deja
/// medio trazo cortado en el borde de la pantalla.
fn se_cruzan(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 <= b.2 && a.2 >= b.0 && a.1 <= b.3 && a.3 >= b.1
}

/// Los elementos visibles que caen dentro de la ventana, de abajo arriba.
///
/// Es la optimizacion que sostiene el lienzo infinito: en un dibujo de diez
/// mil elementos, en la pantalla caben treinta. Sin esto, cada fotograma
/// convierte diez mil figuras en geometria para tirar el 99,7 %.
///
/// La caja de un elemento ya trae sumado su medio grosor, asi que no hace
/// falta holgura extra: un trazo cuyo centro esta fuera pero cuya tinta entra
/// tiene la caja dentro.
pub fn recortar(escena: &Escena, ventana: (f32, f32, f32, f32)) -> impl Iterator<Item = &Elemento> {
    escena
        .visibles()
        .filter(move |e| se_cruzan(e.caja(), ventana))
}

/// Cuantos elementos se ven. Para el contador de la barra y para las pruebas.
pub fn cuantos_se_ven(escena: &Escena, ventana: (f32, f32, f32, f32)) -> usize {
    recortar(escena, ventana).count()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, Elemento, EstiloTrazo, Figura};

    fn caja_en(x: f32, y: f32, lado: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho: lado,
            alto: lado,
            angulo: 0.0,
            trazo: ColorRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 1.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 0.0,
            opacidad: 1.0,
            semilla: 1,
            version: 1,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    fn cerca(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn ir_y_volver_entre_mundo_y_pantalla_no_pierde_el_punto() {
        let c = Camara {
            x: -670.0,
            y: -1302.6,
            zoom: 2.35,
        };
        let p = Punto2::nuevo(389.62, 1465.35);
        let v = c.a_mundo(c.a_pantalla(p));
        assert!(
            cerca(v.x, p.x) && cerca(v.y, p.y),
            "se perdio el punto: {v:?}"
        );
    }

    #[test]
    fn la_esquina_de_la_pantalla_es_el_punto_de_la_camara() {
        // Es la definicion de los tres numeros; si esto se rompe, todo lo
        // demas esta calculando sobre otra cosa.
        let c = Camara {
            x: 100.0,
            y: 200.0,
            zoom: 3.0,
        };
        let e = c.a_pantalla(Punto2::nuevo(100.0, 200.0));
        assert!(cerca(e.x, 0.0) && cerca(e.y, 0.0));
    }

    #[test]
    fn la_camara_del_movil_se_lee_con_su_convenio() {
        // Los numeros son los del fichero real del usuario. Su dibujo va de
        // x 165 a 1178 y tiene que caer en la pantalla de un movil; si el
        // signo estuviera al reves se iria a coordenadas muy negativas y el
        // lienzo apareceria vacio.
        let c = Camara::de_excalidraw(-670.0558, -1302.6286, 2.3510719);
        let izq = c.a_pantalla(Punto2::nuevo(165.0, 228.0));
        assert!(
            izq.x > -1300.0 && izq.x < 1300.0,
            "fuera de la pantalla: {izq:?}"
        );
        let (sx, sy, z) = c.a_excalidraw();
        assert!(cerca(sx, -670.0558) && cerca(sy, -1302.6286) && cerca(z, 2.3510719));
    }

    #[test]
    fn arrastrar_mueve_menos_mundo_cuanto_mas_cerca_se_esta() {
        // El mismo gesto de raton tiene que recorrer la mitad de dibujo con
        // el doble de aumento; si no, encuadrar de cerca se vuelve un tiro.
        let mut lejos = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        };
        let mut pegado = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 2.0,
        };
        lejos.desplazar(100.0, 0.0);
        pegado.desplazar(100.0, 0.0);
        assert!(cerca(lejos.x, -100.0));
        assert!(cerca(pegado.x, -50.0));
    }

    #[test]
    fn la_rueda_aumenta_hacia_donde_apunta_el_raton() {
        // Lo que separa un lienzo que se maneja de uno que marea: el punto
        // del mundo bajo el cursor tiene que seguir bajo el cursor.
        let mut c = Camara::nueva();
        let foco = Punto2::nuevo(300.0, 400.0);
        let antes = c.a_mundo(foco);
        assert!(c.acercar_en(foco, 4.0));
        let despues = c.a_mundo(foco);
        assert!(
            cerca(antes.x, despues.x) && cerca(antes.y, despues.y),
            "el dibujo se escapo del cursor: {antes:?} -> {despues:?}"
        );
        assert!(cerca(c.zoom, 4.0));
    }

    #[test]
    fn el_aumento_tiene_topes_y_los_avisa() {
        // Caso negativo: pasado el tope no se mueve nada, y quien llama tiene
        // que enterarse para no repintar una pantalla identica.
        let mut c = Camara::nueva();
        c.acercar_en(Punto2::nuevo(0.0, 0.0), 1000.0);
        assert!(cerca(c.zoom, ZOOM_MAXIMO));
        assert!(
            !c.acercar_en(Punto2::nuevo(0.0, 0.0), 2.0),
            "dijo que habia cambiado algo estando en el tope"
        );
        c.acercar_en(Punto2::nuevo(0.0, 0.0), 0.000_001);
        assert!(cerca(c.zoom, ZOOM_MINIMO));
        assert!(!c.acercar_en(Punto2::nuevo(0.0, 0.0), 0.5));
    }

    #[test]
    fn encajar_deja_el_dibujo_entero_dentro_y_centrado() {
        let caja = (0.0, 0.0, 100.0, 50.0);
        let c = Camara::encajar(caja, 400.0, 400.0, 20.0);
        // Manda el lado que peor cabe: 360/100, no 360/50.
        assert!(cerca(c.zoom, 3.6), "zoom {}", c.zoom);
        let a = c.a_pantalla(Punto2::nuevo(0.0, 0.0));
        let b = c.a_pantalla(Punto2::nuevo(100.0, 50.0));
        assert!(
            a.x >= 0.0 && a.y >= 0.0 && b.x <= 400.0 && b.y <= 400.0,
            "se sale del hueco: {a:?} {b:?}"
        );
        // Centrado: lo que sobra arriba es lo que sobra abajo.
        assert!(
            cerca(a.y, 400.0 - b.y),
            "descentrado: {} arriba, {} abajo",
            a.y,
            400.0 - b.y
        );
    }

    #[test]
    fn encajar_algo_sin_tamano_no_revienta() {
        // Caso negativo: un lienzo vacio da una caja plana, y un hueco de
        // cero pasa cada vez que se minimiza la ventana.
        assert_eq!(
            Camara::encajar((5.0, 5.0, 5.0, 5.0), 400.0, 400.0, 0.0),
            Camara::nueva()
        );
        assert_eq!(
            Camara::encajar((0.0, 0.0, 10.0, 10.0), 0.0, 0.0, 0.0),
            Camara::nueva()
        );
        // Y la holgura que se come el hueco entero tampoco.
        assert_eq!(
            Camara::encajar((0.0, 0.0, 10.0, 10.0), 30.0, 30.0, 20.0),
            Camara::nueva()
        );
    }

    #[test]
    fn solo_se_pinta_lo_que_se_ve() {
        // La razon de ser del recorte: en un dibujo grande la pantalla ensena
        // una parte minima, y el resto no debe ni convertirse a geometria.
        let mut escena = Escena::nueva();
        for i in 0..100 {
            escena.anadir(caja_en(i as f32 * 1000.0, 0.0, 10.0));
        }
        let c = Camara::nueva();
        assert_eq!(cuantos_se_ven(&escena, c.ventana(800.0, 600.0)), 1);
        assert_eq!(escena.cuantos_visibles(), 100, "no debia borrar nada");
    }

    #[test]
    fn un_elemento_a_medias_en_el_borde_se_pinta() {
        // Equivocarse pintando de mas es invisible; de menos deja medio trazo
        // cortado en el borde de la pantalla.
        let mut escena = Escena::nueva();
        escena.anadir(caja_en(-5.0, -5.0, 10.0));
        escena.anadir(caja_en(795.0, 0.0, 10.0));
        let c = Camara::nueva();
        assert_eq!(cuantos_se_ven(&escena, c.ventana(800.0, 600.0)), 2);
    }

    #[test]
    fn lo_borrado_no_se_pinta_aunque_este_delante() {
        // El recorte filtra por sitio; no puede saltarse el filtro de
        // borrados que ya hacia la escena.
        let mut escena = Escena::nueva();
        let id = escena.anadir(caja_en(0.0, 0.0, 10.0));
        escena.borrar(id);
        let c = Camara::nueva();
        assert_eq!(cuantos_se_ven(&escena, c.ventana(800.0, 600.0)), 0);
    }

    #[test]
    fn la_tolerancia_se_piensa_en_pixeles_y_se_aplica_en_el_mundo() {
        let c = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 4.0,
        };
        assert!(cerca(c.en_mundo(8.0), 2.0));
        let lejos = Camara {
            x: 0.0,
            y: 0.0,
            zoom: 0.25,
        };
        assert!(cerca(lejos.en_mundo(8.0), 32.0));
    }

    #[test]
    fn acercar_entre_respeta_limites_propios_por_debajo_del_minimo_general() {
        let mut c = Camara::nueva();
        c.zoom = 0.01;
        assert!(c.acercar_en_entre(Punto2::nuevo(0.0, 0.0), 0.5, 0.002, ZOOM_MAXIMO));
        assert!((c.zoom - 0.005).abs() < 1e-6);
        // Caso negativo: el acercar de siempre sigue sin bajar de 0,05.
        let mut d = Camara::nueva();
        d.zoom = 0.06;
        d.acercar_en(Punto2::nuevo(0.0, 0.0), 0.1);
        assert_eq!(d.zoom, ZOOM_MINIMO);
    }
}
