//! Donde caen los nueve puntos de agarre y cual esta bajo el cursor.
//!
//! # Viven en el mundo, miden en pantalla
//!
//! Un tirador tiene ocho pixeles siempre: al 500 % no se convierte en un
//! ladrillo y al 20 % no desaparece. En unidades del mundo eso es
//! `LADO * escala`, donde `escala` es —como en todo este motor— unidades de
//! mundo por pixel de pantalla, o sea `1.0 / zoom`.
//!
//! # Por que la zona de picado es mas grande que el dibujo
//!
//! Diez pixeles contra ocho. Nadie acierta un cuadradito de ocho pixeles al
//! primer intento, y fallar un tirador no es un fallo pequeno: el clic cae
//! en el elemento de debajo y lo que el usuario queria redimensionar acaba
//! movido.

use pixpin_geom::Tirador;

use crate::elemento::{ColorRgba, Elemento, EstiloTrazo};
use crate::pintado::Orden;
use crate::vector::Punto2;

/// Lado del cuadradito que se dibuja, en pixeles de pantalla.
pub const LADO: f32 = 8.0;

/// Radio de la zona que responde al cursor, en pixeles de pantalla.
pub const ZONA: f32 = 10.0;

/// Cuanto se separa el tirador de giro del borde de arriba, en pixeles.
pub const SEPARACION_GIRO: f32 = 24.0;

const RELLENO: ColorRgba = ColorRgba {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const BORDE: ColorRgba = ColorRgba {
    r: 0.36,
    g: 0.42,
    b: 0.95,
    a: 1.0,
};

/// De que se ha agarrado el cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agarre {
    Tamano(Tirador),
    Giro,
}

/// Los nueve puntos de agarre, ya en coordenadas del mundo.
#[derive(Debug, Clone, PartialEq)]
pub struct Tiradores {
    pub tamano: [(Tirador, Punto2); 8],
    pub giro: Punto2,
    pub centro: Punto2,
    pub angulo: f32,
}

impl Tiradores {
    /// Los tiradores de una caja paralela a los ejes, girada `angulo`
    /// alrededor de su centro.
    pub fn de_caja(caja: (f32, f32, f32, f32), angulo: f32, escala: f32) -> Self {
        let (x0, y0, x1, y1) = caja;
        let centro = Punto2::nuevo((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (mx, my) = (centro.x, centro.y);

        // Sin girar primero, y luego se gira todo de una vez: es una
        // operacion por punto en vez de una formula distinta por esquina.
        let sitios = [
            (Tirador::NoroesteEsquina, Punto2::nuevo(x0, y0)),
            (Tirador::NorteBorde, Punto2::nuevo(mx, y0)),
            (Tirador::NoresteEsquina, Punto2::nuevo(x1, y0)),
            (Tirador::EsteBorde, Punto2::nuevo(x1, my)),
            (Tirador::SuresteEsquina, Punto2::nuevo(x1, y1)),
            (Tirador::SurBorde, Punto2::nuevo(mx, y1)),
            (Tirador::SuroesteEsquina, Punto2::nuevo(x0, y1)),
            (Tirador::OesteBorde, Punto2::nuevo(x0, my)),
        ];
        let tamano = sitios.map(|(c, p)| (c, p.girar(centro, angulo)));

        // El de giro, separado por encima del borde norte. La separacion va
        // en pixeles de pantalla, como todo lo demas de aqui.
        let giro = Punto2::nuevo(mx, y0 - SEPARACION_GIRO * escala).girar(centro, angulo);

        Self {
            tamano,
            giro,
            centro,
            angulo,
        }
    }

    /// Los tiradores de un elemento.
    pub fn de_elemento(e: &Elemento, escala: f32) -> Self {
        Self::de_caja(e.caja(), e.angulo, escala)
    }

    /// De que hay agarre bajo el punto, si de alguno.
    ///
    /// El de giro se mira primero: con una caja muy pequena puede caer
    /// encima del borde norte, y girar es el gesto mas dificil de acertar
    /// de los dos.
    pub fn en(&self, p: Punto2, escala: f32) -> Option<Agarre> {
        let radio = ZONA * escala;
        if p.distancia(self.giro) <= radio {
            return Some(Agarre::Giro);
        }
        self.tamano
            .iter()
            .find(|(_, q)| p.distancia(*q) <= radio)
            .map(|(c, _)| Agarre::Tamano(*c))
    }

    /// Como se pintan: un cuadradito blanco con borde por cada uno.
    ///
    /// Girados con el elemento, para que en una figura a 45 grados los
    /// cuadraditos vayan a 45 grados y no de canto.
    pub fn ordenes(&self, escala: f32) -> Vec<Orden> {
        let mitad = LADO * escala / 2.0;
        let mut fuera = Vec::with_capacity(9);
        let sitios = self.tamano.iter().map(|(_, p)| p).chain([&self.giro]);
        for p in sitios {
            let esquinas = [
                Punto2::nuevo(p.x - mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y + mitad),
                Punto2::nuevo(p.x - mitad, p.y + mitad),
            ];
            let puntos: Vec<Punto2> = esquinas.iter().map(|q| q.girar(*p, self.angulo)).collect();
            fuera.push(Orden::Relleno {
                puntos: puntos.clone(),
                color: RELLENO,
            });
            let mut cerrado = puntos;
            cerrado.push(cerrado[0]);
            fuera.push(Orden::Polilinea {
                puntos: cerrado,
                color: BORDE,
                grosor: (1.0 * escala).max(0.5),
                estilo: EstiloTrazo::Solido,
            });
        }
        fuera
    }
}

// -------------------------------------------------------------------------
// Los tiradores de punta de raya
// -------------------------------------------------------------------------

/// Cuanto mas pequeno se pinta el tirador de «anadir punto» que uno de
/// verdad (`ADD_HANDLE_RATIO`).
///
/// Mas pequeno a proposito: es el unico que **crea** algo en vez de mover lo
/// que ya hay, y conviene que se distinga de un vistazo de los que no.
pub const RAZON_ANADIR: f32 = 0.6;

/// Lo que tiene que dar de si un tramo para ofrecer su punto de anadir, en
/// veces el lado del tirador.
///
/// En un tramo corto el tirador de anadir se amontona con los de sus dos
/// puntas y no se acierta a ninguno de los tres.
const TRAMO_MINIMO: f32 = 2.5;

/// De que punta de una raya se ha agarrado el cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgarrePunta {
    /// El primer punto del trazo (`POINT_START`).
    Principio,
    /// El ultimo (`POINT_END`).
    Final,
    /// Un vertice intermedio, por su indice (`POINT_MID`).
    Vertice(usize),
    /// El medio de un tramo: al arrastrarlo **nace** un vertice nuevo
    /// detras del indice dado (`POINT_ADD`).
    Anadir(usize),
}

/// Los puntos de agarre de una figura con puntos, ya en el mundo.
///
/// # Por que no valen los ocho de la caja
///
/// La caja de una diagonal tiene dos esquinas por las que la linea **no
/// pasa**, y estirar una raya por una esquina que no le pertenece es lo que
/// hace que afinar un esquema sea imposible: se quiere alargar la flecha
/// hasta la caja de al lado, no escalar su caja. Estos van sobre los
/// extremos de verdad.
#[derive(Debug, Clone, PartialEq)]
pub struct TiradoresDePunta {
    /// Cada vertice con su indice en la lista de puntos.
    pub vertices: Vec<(usize, Punto2)>,
    /// El medio de cada tramo que da de si, con el indice del punto que
    /// tiene delante.
    pub anadir: Vec<(usize, Punto2)>,
}

impl TiradoresDePunta {
    /// Los de `e`, o `None` si no es una figura con puntos o esta bloqueada.
    ///
    /// Los puntos de este motor son **absolutos y ya en el mundo**, asi que
    /// —a diferencia del movil, que los guarda relativos y sin girar— aqui no
    /// hay que girarlos: lo que se pinta es lo que hay en la lista.
    pub fn de_elemento(e: &Elemento, escala: f32) -> Option<Self> {
        if e.bloqueado || e.borrado {
            return None;
        }
        let puntos = e.puntos()?;
        if puntos.len() < 2 {
            return None;
        }
        let lado = LADO * escala;
        let vertices = puntos.iter().copied().enumerate().collect();
        let anadir = puntos
            .windows(2)
            .enumerate()
            .filter(|(_, par)| par[0].distancia(par[1]) > lado * TRAMO_MINIMO)
            .map(|(i, par)| {
                (
                    i,
                    Punto2::nuevo((par[0].x + par[1].x) / 2.0, (par[0].y + par[1].y) / 2.0),
                )
            })
            .collect();
        Some(Self { vertices, anadir })
    }

    /// De que punta hay agarre bajo el punto, si de alguna.
    ///
    /// **Los vertices se miran antes que los de anadir**: en un tramo corto
    /// los dos caen cerca, y equivocarse creando un vertice donde se queria
    /// mover uno deja la raya con un punto de mas que hay que deshacer.
    pub fn en(&self, p: Punto2, escala: f32) -> Option<AgarrePunta> {
        let radio = ZONA * escala;
        let ultimo = self.vertices.len().saturating_sub(1);
        if let Some((i, _)) = self
            .vertices
            .iter()
            .find(|(_, q)| p.distancia(*q) <= radio)
            .copied()
        {
            return Some(match i {
                0 => AgarrePunta::Principio,
                i if i == ultimo => AgarrePunta::Final,
                i => AgarrePunta::Vertice(i),
            });
        }
        self.anadir
            .iter()
            .find(|(_, q)| p.distancia(*q) <= radio * RAZON_ANADIR)
            .map(|(i, _)| AgarrePunta::Anadir(*i))
    }

    /// Como se pintan: cuadradito blanco con borde, igual que los de tamano,
    /// y los de anadir mas pequenos.
    pub fn ordenes(&self, escala: f32) -> Vec<Orden> {
        let mut fuera = Vec::with_capacity((self.vertices.len() + self.anadir.len()) * 2);
        let sitios = self
            .vertices
            .iter()
            .map(|(_, p)| (*p, 1.0))
            .chain(self.anadir.iter().map(|(_, p)| (*p, RAZON_ANADIR)));
        for (p, razon) in sitios {
            let mitad = LADO * escala * razon / 2.0;
            let puntos = vec![
                Punto2::nuevo(p.x - mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y - mitad),
                Punto2::nuevo(p.x + mitad, p.y + mitad),
                Punto2::nuevo(p.x - mitad, p.y + mitad),
            ];
            fuera.push(Orden::Relleno {
                puntos: puntos.clone(),
                color: RELLENO,
            });
            let mut cerrado = puntos;
            cerrado.push(cerrado[0]);
            fuera.push(Orden::Polilinea {
                puntos: cerrado,
                color: BORDE,
                grosor: (1.0 * escala).max(0.5),
                estilo: EstiloTrazo::Solido,
            });
        }
        fuera
    }
}

/// Lleva un vertice de `e` hasta `p`.
///
/// Devuelve si movio algo. **Al mover una punta se suelta su enganche**, como
/// en el movil (`Transform.kt:409-410` hace lo propio al girar): si la punta
/// se lleva a mano a otro sitio, dejarla atada significaria que la proxima
/// vez que se mueva la caja la punta vuelve sola a donde estaba, deshaciendo
/// lo que se acaba de hacer.
pub fn mover_punta(e: &mut Elemento, agarre: AgarrePunta, p: Punto2) -> bool {
    let ultimo = match e.puntos() {
        Some(puntos) if puntos.len() >= 2 => puntos.len() - 1,
        _ => return false,
    };
    let indice = match agarre {
        AgarrePunta::Principio => 0,
        AgarrePunta::Final => ultimo,
        AgarrePunta::Vertice(i) if i <= ultimo => i,
        // Anadir no mueve: se resuelve con `insertar_punta`, que crea el
        // vertice, y a partir de ahi ya es un vertice corriente.
        _ => return false,
    };
    let Some(puntos) = puntos_mut(e) else {
        return false;
    };
    puntos[indice] = p;
    match agarre {
        AgarrePunta::Principio => e.extras.enganche_inicio = None,
        AgarrePunta::Final => e.extras.enganche_fin = None,
        _ => {}
    }
    recolocar_origen(e);
    e.tocar();
    true
}

/// Mete un vertice nuevo en el tramo `indice`, en el punto `p`.
///
/// Devuelve el agarre del vertice recien nacido, para que el arrastre siga
/// sin soltar: quien pincha el punto de anadir espera empezar a doblar la
/// raya ahi mismo, no tener que volver a pinchar.
pub fn insertar_punta(e: &mut Elemento, indice: usize, p: Punto2) -> Option<AgarrePunta> {
    let puntos = puntos_mut(e)?;
    if indice + 1 > puntos.len() {
        return None;
    }
    puntos.insert(indice + 1, p);
    recolocar_origen(e);
    e.tocar();
    Some(AgarrePunta::Vertice(indice + 1))
}

/// Quita un vertice intermedio. Los dos extremos no se quitan: una raya de un
/// punto no es una raya.
pub fn quitar_punta(e: &mut Elemento, indice: usize) -> bool {
    let Some(puntos) = puntos_mut(e) else {
        return false;
    };
    if indice == 0 || indice + 1 >= puntos.len() || puntos.len() <= 2 {
        return false;
    }
    puntos.remove(indice);
    recolocar_origen(e);
    e.tocar();
    true
}

fn puntos_mut(e: &mut Elemento) -> Option<&mut Vec<Punto2>> {
    match &mut e.figura {
        crate::elemento::Figura::Lapiz { puntos, .. }
        | crate::elemento::Figura::Resaltador { puntos }
        | crate::elemento::Figura::Linea { puntos }
        | crate::elemento::Figura::Flecha { puntos, .. }
        | crate::elemento::Figura::Cota { puntos } => Some(puntos),
        _ => None,
    }
}

/// `x`/`y` vuelven a la esquina de la caja: es el origen desde el que se
/// escriben los puntos en el fichero, y si se quedara atras, una raya afinada
/// aqui llegaria al movil desplazada.
fn recolocar_origen(e: &mut Elemento) {
    let (x0, y0, _, _) = e.caja();
    e.x = x0;
    e.y = y0;
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    /// Una caja de 100x50 en el origen.
    const CAJA: (f32, f32, f32, f32) = (0.0, 0.0, 100.0, 50.0);

    fn busca(t: &Tiradores, cual: Tirador) -> Punto2 {
        t.tamano.iter().find(|(c, _)| *c == cual).unwrap().1
    }

    fn cerca(a: Punto2, b: Punto2, que: &str) {
        assert!(
            (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3,
            "{que}: esperaba ({}, {}), es ({}, {})",
            b.x,
            b.y,
            a.x,
            a.y
        );
    }

    #[test]
    fn los_ocho_caen_en_las_esquinas_y_en_los_medios() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        cerca(
            busca(&t, Tirador::NoroesteEsquina),
            Punto2::nuevo(0.0, 0.0),
            "NO",
        );
        cerca(
            busca(&t, Tirador::NorteBorde),
            Punto2::nuevo(50.0, 0.0),
            "N",
        );
        cerca(
            busca(&t, Tirador::NoresteEsquina),
            Punto2::nuevo(100.0, 0.0),
            "NE",
        );
        cerca(
            busca(&t, Tirador::EsteBorde),
            Punto2::nuevo(100.0, 25.0),
            "E",
        );
        cerca(
            busca(&t, Tirador::SuresteEsquina),
            Punto2::nuevo(100.0, 50.0),
            "SE",
        );
        cerca(busca(&t, Tirador::SurBorde), Punto2::nuevo(50.0, 50.0), "S");
        cerca(
            busca(&t, Tirador::SuroesteEsquina),
            Punto2::nuevo(0.0, 50.0),
            "SO",
        );
        cerca(
            busca(&t, Tirador::OesteBorde),
            Punto2::nuevo(0.0, 25.0),
            "O",
        );
    }

    #[test]
    fn con_la_caja_girada_los_tiradores_giran_con_ella() {
        let t = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let centro = Punto2::nuevo(50.0, 25.0);
        // La esquina noroeste, girada un cuarto de vuelta sobre el centro.
        cerca(
            busca(&t, Tirador::NoroesteEsquina),
            Punto2::nuevo(0.0, 0.0).girar(centro, FRAC_PI_2),
            "NO girada",
        );
    }

    #[test]
    fn el_de_giro_queda_separado_por_encima_y_gira_tambien() {
        // Encima del borde norte, a SEPARACION_GIRO pixeles de pantalla.
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        cerca(t.giro, Punto2::nuevo(50.0, -SEPARACION_GIRO), "sin girar");

        let g = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let centro = Punto2::nuevo(50.0, 25.0);
        cerca(
            g.giro,
            Punto2::nuevo(50.0, -SEPARACION_GIRO).girar(centro, FRAC_PI_2),
            "girado",
        );
    }

    #[test]
    fn la_separacion_del_giro_se_mide_en_pixeles_de_pantalla() {
        // Al 20 % de aumento (escala 5.0) el tirador se separa cinco veces
        // mas en el mundo, para verse igual de separado en la pantalla.
        let t = Tiradores::de_caja(CAJA, 0.0, 5.0);
        cerca(
            t.giro,
            Punto2::nuevo(50.0, -SEPARACION_GIRO * 5.0),
            "al 20 %",
        );
    }

    #[test]
    fn picar_un_tirador_lo_encuentra_por_su_zona_y_no_por_su_dibujo() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);

        // Justo encima: lo encuentra.
        assert_eq!(
            t.en(Punto2::nuevo(100.0, 50.0), 1.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
        // A nueve pixeles: dentro de la zona de diez, aunque el dibujo mida
        // ocho. Nadie acierta un cuadradito de ocho pixeles al primer
        // intento.
        assert_eq!(
            t.en(Punto2::nuevo(100.0 + 9.0, 50.0), 1.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
        // A veinte: fuera.
        assert_eq!(t.en(Punto2::nuevo(100.0 + 20.0, 50.0), 1.0), None);
    }

    #[test]
    fn la_zona_de_picado_tambien_se_mide_en_pixeles_de_pantalla() {
        // Al 20 % (escala 5.0), diez pixeles de pantalla son cincuenta del
        // mundo. Si no, al alejarse los tiradores serian inalcanzables.
        let t = Tiradores::de_caja(CAJA, 0.0, 5.0);
        assert_eq!(
            t.en(Punto2::nuevo(100.0 + 45.0, 50.0), 5.0),
            Some(Agarre::Tamano(Tirador::SuresteEsquina))
        );
    }

    #[test]
    fn el_de_giro_manda_sobre_el_de_tamano_si_se_solapan() {
        // Con una caja muy pequena, el de giro puede caer encima del borde
        // norte. Girar es el gesto mas dificil de acertar de los dos, asi
        // que gana.
        let minuscula = (0.0, 0.0, 2.0, 2.0);
        let t = Tiradores::de_caja(minuscula, 0.0, 1.0);
        assert_eq!(t.en(t.giro, 1.0), Some(Agarre::Giro));
    }

    #[test]
    fn picar_lejos_de_todo_no_encuentra_nada() {
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        assert_eq!(t.en(Punto2::nuevo(500.0, 500.0), 1.0), None);
    }

    #[test]
    fn cada_tirador_se_pinta_con_relleno_y_borde() {
        // Nueve tiradores por dos ordenes cada uno. El borde no es adorno:
        // un cuadrado blanco sin el es invisible sobre fondo claro.
        let t = Tiradores::de_caja(CAJA, 0.0, 1.0);
        assert_eq!(t.ordenes(1.0).len(), 18);
    }

    #[test]
    fn los_cuadraditos_giran_con_el_elemento() {
        // En una figura a 45 grados, los cuadraditos van a 45 grados y no
        // de canto. Cuesta una linea y es de lo que separa un editor que
        // se siente bien de uno que no.
        let t = Tiradores::de_caja(CAJA, FRAC_PI_2, 1.0);
        let Orden::Relleno { puntos, .. } = &t.ordenes(1.0)[0] else {
            panic!("la primera orden es el relleno del primer tirador");
        };
        let lado = puntos[0].distancia(puntos[1]);
        assert!((lado - LADO).abs() < 1e-3, "sigue siendo cuadrado: {lado}");
        // Girado un cuarto de vuelta, el lado que iba en x ahora va en y.
        assert!(
            (puntos[0].x - puntos[1].x).abs() < 1e-3,
            "el primer lado quedo vertical"
        );
    }

    // --- Tiradores de punta ---

    use crate::elemento::{ColorRgba, Figura};

    fn raya(puntos: Vec<Punto2>) -> Elemento {
        Elemento {
            id: 1,
            figura: Figura::Linea { puntos },
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        }
    }

    #[test]
    fn una_raya_da_tiradores_en_sus_puntas_de_verdad_y_no_en_su_caja() {
        // La razon de ser: la caja de una diagonal tiene dos esquinas por las
        // que la linea no pasa.
        let d = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 100.0)]);
        let t = TiradoresDePunta::de_elemento(&d, 1.0).unwrap();
        assert_eq!(t.vertices.len(), 2);
        assert_eq!(
            t.en(Punto2::nuevo(0.0, 0.0), 1.0),
            Some(AgarrePunta::Principio)
        );
        assert_eq!(
            t.en(Punto2::nuevo(100.0, 100.0), 1.0),
            Some(AgarrePunta::Final)
        );
        // La esquina noreste de su caja esta en (100,0) y ahi no hay nada.
        assert_eq!(t.en(Punto2::nuevo(100.0, 0.0), 1.0), None);
    }

    #[test]
    fn los_vertices_de_en_medio_se_agarran_por_su_indice() {
        let l = raya(vec![
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(50.0, 30.0),
            Punto2::nuevo(100.0, 0.0),
        ]);
        let t = TiradoresDePunta::de_elemento(&l, 1.0).unwrap();
        assert_eq!(
            t.en(Punto2::nuevo(50.0, 30.0), 1.0),
            Some(AgarrePunta::Vertice(1))
        );
    }

    #[test]
    fn un_tramo_largo_ofrece_su_punto_de_anadir_y_uno_corto_no() {
        // En un tramo corto el tirador de anadir se amontona con los de sus
        // dos puntas y no se acierta a ninguno de los tres.
        let largo = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(200.0, 0.0)]);
        let t = TiradoresDePunta::de_elemento(&largo, 1.0).unwrap();
        assert_eq!(t.anadir.len(), 1);
        assert_eq!(t.anadir[0].1, Punto2::nuevo(100.0, 0.0));

        let corto = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(10.0, 0.0)]);
        assert!(
            TiradoresDePunta::de_elemento(&corto, 1.0)
                .unwrap()
                .anadir
                .is_empty()
        );
    }

    #[test]
    fn en_un_empate_gana_el_vertice_al_punto_de_anadir() {
        // Equivocarse creando un vertice donde se queria mover uno deja la
        // raya con un punto de mas que hay que deshacer.
        let l = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(60.0, 0.0)]);
        let t = TiradoresDePunta::de_elemento(&l, 1.0).unwrap();
        // El medio esta en (30,0); el extremo en (60,0). A (55,0) los dos
        // quedan a tiro si el radio fuera el mismo.
        assert_eq!(
            t.en(Punto2::nuevo(55.0, 0.0), 1.0),
            Some(AgarrePunta::Final)
        );
    }

    #[test]
    fn mover_una_punta_la_lleva_y_le_suelta_su_enganche() {
        // Dejarla atada significaria que la proxima vez que se mueva la caja
        // la punta vuelve sola, deshaciendo lo que se acaba de hacer.
        let mut l = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)]);
        l.extras.enganche_fin = Some(crate::elemento::Enganche {
            elemento: "x".into(),
            foco: 0.0,
            hueco: 5.0,
            punto_fijo: None,
            modo: Default::default(),
        });
        assert!(mover_punta(
            &mut l,
            AgarrePunta::Final,
            Punto2::nuevo(200.0, 50.0)
        ));
        let Figura::Linea { puntos } = &l.figura else {
            unreachable!()
        };
        assert_eq!(puntos[1], Punto2::nuevo(200.0, 50.0));
        assert!(l.extras.enganche_fin.is_none());
        assert!(l.x <= 0.0 && l.y <= 0.0, "el origen siguio a la caja");
    }

    #[test]
    fn anadir_un_vertice_deja_el_arrastre_sobre_el_recien_nacido() {
        // Quien pincha el punto de anadir espera empezar a doblar la raya ahi
        // mismo, no tener que volver a pinchar.
        let mut l = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(200.0, 0.0)]);
        let agarre = insertar_punta(&mut l, 0, Punto2::nuevo(100.0, 40.0)).unwrap();
        assert_eq!(agarre, AgarrePunta::Vertice(1));
        let Figura::Linea { puntos } = &l.figura else {
            unreachable!()
        };
        assert_eq!(puntos.len(), 3);
        assert_eq!(puntos[1], Punto2::nuevo(100.0, 40.0));
    }

    #[test]
    fn no_se_puede_quitar_una_punta_ni_dejar_una_raya_de_un_punto() {
        // Caso negativo: una raya de un punto no es una raya.
        let mut l = raya(vec![
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(50.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
        ]);
        assert!(!quitar_punta(&mut l, 0), "el principio no se quita");
        assert!(!quitar_punta(&mut l, 2), "el final tampoco");
        assert!(quitar_punta(&mut l, 1));
        assert!(!quitar_punta(&mut l, 1), "ya solo quedan las dos puntas");
    }

    #[test]
    fn una_figura_sin_puntos_o_bloqueada_no_tiene_tiradores_de_punta() {
        // Caso negativo: un rectangulo se estira por sus ocho, y lo bloqueado
        // no se estira de ninguna forma.
        let rectangulo = Elemento {
            id: 1,
            figura: Figura::Rectangulo,
            ancho: 100.0,
            alto: 50.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            ..Default::default()
        };
        assert!(TiradoresDePunta::de_elemento(&rectangulo, 1.0).is_none());

        let mut l = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(100.0, 0.0)]);
        l.bloqueado = true;
        assert!(TiradoresDePunta::de_elemento(&l, 1.0).is_none());
    }

    #[test]
    fn los_tiradores_de_punta_tampoco_crecen_con_el_zoom() {
        let l = raya(vec![Punto2::nuevo(0.0, 0.0), Punto2::nuevo(200.0, 0.0)]);
        let lado = |zoom: f32| {
            let escala = 1.0 / zoom;
            let t = TiradoresDePunta::de_elemento(&l, escala).unwrap();
            let Orden::Relleno { puntos, .. } = &t.ordenes(escala)[0] else {
                panic!("la primera orden es el relleno");
            };
            puntos[0].distancia(puntos[1]) * zoom
        };
        assert!((lado(1.0) - LADO).abs() < 1e-3);
        assert!((lado(4.0) - LADO).abs() < 1e-3, "en pantalla mide lo mismo");
    }
}
