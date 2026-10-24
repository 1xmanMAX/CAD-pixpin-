//! **La sesion de dibujo**: la orden en marcha y lo que pide a cada paso
//! (un punto, una distancia, objetos…), como la linea de ordenes de
//! AutoCAD. La ventana solo le pasa clics, recuadros y lo que se escribe, y
//! pinta lo que esta devuelve: la indicacion, la vista previa y lo elegido.

use std::f64::consts::PI;

use opencadcodec::{EntityType, Handle};

use crate::dibujo::Dibujo;
use crate::enganche::{self, Enganche, Modos};
use crate::entrada::{self, Entrada};
use crate::especialidad as esp;
use crate::formas;
use crate::geo::{self, Forma, P};
use crate::operaciones as op;
use crate::ordenes::{self, Id};

/// Los ajustes del dibujo (los que se cambian en la barra de estado o al
/// escribir un numero en una orden).
#[derive(Debug, Clone)]
pub struct Ajustes {
    pub orto: bool,
    /// El paso del rastreo polar, en radianes (`None`: apagado).
    pub polar: Option<f64>,
    pub enganche: bool,
    pub modos: Modos,
    /// El lado de la rejilla a la que se pega el cursor (`None`: libre).
    pub rejilla: Option<f64>,
    pub alto_texto: f64,
    pub espesor_muro: f64,
    pub ancho_puerta: f64,
    pub radio_empalme: f64,
    pub distancia_chaflan: f64,
    pub desfase: f64,
    pub decimales: usize,
    pub formato_lamina: String,
    pub escala_lamina: f64,
}

impl Ajustes {
    /// Los de un dibujo en las unidades `insunits` (4 mm, 5 cm, 6 m…).
    pub fn para(insunits: i16) -> Ajustes {
        // Cuantas unidades del dibujo es un milimetro.
        let mm = match insunits {
            5 => 0.1,
            6 => 0.001,
            1 => 1.0 / 25.4,
            _ => 1.0,
        };
        Ajustes {
            orto: false,
            polar: Some(PI / 4.0),
            enganche: true,
            modos: Modos::default(),
            rejilla: None,
            // 2,5 mm de papel a escala 1/100.
            alto_texto: 250.0 * mm,
            espesor_muro: 150.0 * mm,
            ancho_puerta: 900.0 * mm,
            radio_empalme: 0.0,
            distancia_chaflan: 0.0,
            desfase: 100.0 * mm,
            decimales: if insunits == 6 { 2 } else { 0 },
            formato_lamina: "A1".into(),
            escala_lamina: 50.0,
        }
    }
}

/// Lo que se pinta encima del dibujo mientras se usa una orden.
#[derive(Debug, Clone, Default)]
pub struct Previa {
    /// Lo que va a quedar (en el color de lo nuevo).
    pub formas: Vec<Forma>,
    /// Las lineas guia (de la base al cursor), discontinuas.
    pub guias: Vec<(P, P)>,
    /// Etiquetas junto al cursor (la distancia, el angulo…).
    pub etiquetas: Vec<(P, String)>,
}

/// En que paso esta la orden.
#[derive(Debug, Clone, Default)]
struct Paso {
    puntos: Vec<P>,
    numeros: Vec<f64>,
    /// Objetos tomados con un clic dentro de la orden (empalme, desfase…), con donde se tomaron.
    objetos: Vec<(Handle, P)>,
    textos: Vec<String>,
    /// Ya se eligieron los objetos (en las que los piden).
    elegidos: bool,
}

/// **La sesion.**
pub struct Sesion {
    pub dibujo: Dibujo,
    pub seleccion: Vec<Handle>,
    pub ajustes: Ajustes,
    activa: Option<Id>,
    paso: Paso,
    ultima: Option<Id>,
    /// El pinzamiento tomado (entidad e indice) que el proximo clic mueve.
    pinzamiento: Option<(Handle, usize, P)>,
    /// Lo que va saliendo en el historial de la linea de ordenes.
    pub registro: Vec<String>,
    /// La direccion del ultimo tramo: escribir solo una distancia sigue por ahi.
    direccion: Option<P>,
}

impl Sesion {
    pub fn nueva(dibujo: Dibujo) -> Sesion {
        let ajustes = Ajustes::para(dibujo.doc.header.insertion_units);
        Sesion {
            dibujo,
            seleccion: Vec::new(),
            ajustes,
            activa: None,
            paso: Paso::default(),
            ultima: None,
            pinzamiento: None,
            registro: Vec::new(),
            direccion: None,
        }
    }

    pub fn activa(&self) -> Option<Id> {
        self.activa
    }

    fn decir(&mut self, s: impl Into<String>) {
        self.registro.push(s.into());
        if self.registro.len() > 500 {
            self.registro.drain(..100);
        }
    }

    fn medida(&self, d: f64) -> String {
        esp::texto_de_medida(d, self.ajustes.decimales.max(2))
    }

    // --------------------------------------------------------- empezar y salir

    /// **Empieza una orden** (la que estaba se cancela).
    pub fn orden(&mut self, id: Id) {
        self.cancelar_silencioso();
        match id {
            Id::Deshacer => {
                match self.dibujo.deshacer() {
                    Some(n) => self.decir(format!("Deshecho: {n}")),
                    None => self.decir("No hay nada que deshacer"),
                }
                self.limpiar_seleccion_borrada();
                return;
            }
            Id::Rehacer => {
                match self.dibujo.rehacer() {
                    Some(n) => self.decir(format!("Rehecho: {n}")),
                    None => self.decir("No hay nada que rehacer"),
                }
                return;
            }
            _ => {}
        }
        let o = ordenes::de(id);
        self.decir(format!("Orden: {}", o.nombre));
        self.activa = Some(id);
        self.ultima = Some(id);
        self.paso = Paso {
            elegidos: o.con_objetos && !self.seleccion.is_empty(),
            ..Default::default()
        };
        if self.paso.elegidos {
            self.decir(format!("{} objetos elegidos", self.seleccion.len()));
            // Las que no piden nada mas que los objetos, de una vez.
            if matches!(id, Id::Borrar | Id::Descomponer) {
                self.terminar_con_objetos();
            }
        }
    }

    /// Esc: cancela la orden o, si no hay, suelta lo elegido.
    pub fn escape(&mut self) {
        if self.activa.is_some() || self.pinzamiento.is_some() {
            // Lo que va hecho de una orden de varios pasos (lineas, muro…) se queda.
            self.cerrar_pendiente();
            self.cancelar_silencioso();
            self.decir("*Cancelar*");
        } else {
            self.seleccion.clear();
        }
    }

    fn cancelar_silencioso(&mut self) {
        self.activa = None;
        self.paso = Paso::default();
        self.pinzamiento = None;
    }

    fn limpiar_seleccion_borrada(&mut self) {
        let d = &self.dibujo;
        self.seleccion.retain(|h| d.entidad(*h).is_some());
    }

    /// El punto anterior de la orden (para orto, polar, la perpendicular y `@`).
    pub fn base(&self) -> Option<P> {
        if let Some((_, _, p)) = self.pinzamiento {
            return Some(p);
        }
        let id = self.activa?;
        match id {
            Id::Mover | Id::Copiar | Id::Girar | Id::Escalar | Id::MatrizPolar
                if self.paso.puntos.is_empty() =>
            {
                None
            }
            Id::Copiar => self.paso.puntos.first().copied(),
            _ => self.paso.puntos.last().copied(),
        }
    }

    /// **Donde cae de verdad el cursor**: enganchado a lo dibujado, a la
    /// rejilla, o llevado a orto o al angulo polar desde la base. `radio`, en
    /// unidades del dibujo (unos pixeles).
    pub fn ajustar(&self, cursor: P, radio: f64) -> (P, Option<Enganche>) {
        if self.ajustes.enganche
            && let Some(e) = enganche::buscar(
                &self.dibujo.indice(),
                cursor,
                radio,
                self.ajustes.modos,
                self.base(),
            )
        {
            return (e.p, Some(e));
        }
        let mut p = cursor;
        if let Some(paso) = self.ajustes.rejilla {
            p = enganche::rejilla(p, paso);
        }
        if let Some(b) = self.base() {
            if self.ajustes.orto {
                p = enganche::orto(b, p);
            } else if let Some(paso) = self.ajustes.polar
                && let Some((q, _)) = enganche::polar(b, p, paso, 2.0_f64.to_radians())
            {
                p = q;
            }
        }
        (p, None)
    }

    /// Si la orden esta pidiendo objetos (el recuadro elige en vez de ser un punto).
    pub fn pide_objetos(&self) -> bool {
        match self.activa {
            None => true,
            Some(id) => {
                (ordenes::de(id).con_objetos && !self.paso.elegidos)
                    || matches!(id, Id::Recortar | Id::Alargar)
                    || (matches!(id, Id::Empalme | Id::Chaflan) && self.paso.objetos.len() < 2)
                    || (id == Id::Desfase
                        && !self.paso.numeros.is_empty()
                        && self.paso.objetos.is_empty())
            }
        }
    }

    // ------------------------------------------------------------ indicacion

    /// **Lo que pide ahora** la linea de ordenes.
    pub fn indicacion(&self) -> String {
        if self.pinzamiento.is_some() {
            return "Nuevo punto para el pinzamiento:".into();
        }
        let Some(id) = self.activa else {
            return "Orden:".into();
        };
        let p = &self.paso;
        let n = p.puntos.len();
        let a = &self.ajustes;
        if ordenes::de(id).con_objetos && !p.elegidos {
            return "Designe objetos (Enter para terminar):".into();
        }
        match id {
            Id::Linea => {
                if n == 0 {
                    "Primer punto:".into()
                } else {
                    "Siguiente punto [Cerrar/Deshacer] (Enter termina):".into()
                }
            }
            Id::Polilinea => {
                if n == 0 {
                    "Punto inicial:".into()
                } else {
                    "Siguiente punto [Cerrar/Deshacer] (Enter termina):".into()
                }
            }
            Id::Rectangulo => {
                if n == 0 {
                    "Primera esquina:".into()
                } else {
                    "Esquina opuesta (o @ancho,alto):".into()
                }
            }
            Id::Circulo => {
                if n == 0 {
                    "Centro:".into()
                } else {
                    "Radio (punto o numero):".into()
                }
            }
            Id::Arco => {
                ["Punto inicial del arco:", "Segundo punto:", "Punto final:"][n.min(2)].into()
            }
            Id::Texto => match (n, p.numeros.len()) {
                (0, _) => "Punto de insercion del texto:".into(),
                (_, 0) => format!("Altura <{}>:", self.medida(a.alto_texto)),
                _ => "Escriba el texto:".into(),
            },
            Id::Mover | Id::Copiar => {
                if n == 0 {
                    "Punto base:".into()
                } else {
                    "Segundo punto (Enter termina):".into()
                }
            }
            Id::Girar => {
                if n == 0 {
                    "Punto base:".into()
                } else {
                    "Angulo de giro (grados o punto):".into()
                }
            }
            Id::Escalar => {
                if n == 0 {
                    "Punto base:".into()
                } else {
                    "Factor de escala:".into()
                }
            }
            Id::Simetria => match n {
                0 => "Primer punto del eje de simetria:".into(),
                1 => "Segundo punto del eje:".into(),
                _ => "¿Borrar los originales? [Si/No] <No>:".into(),
            },
            Id::Desfase => {
                if p.numeros.is_empty() {
                    format!("Distancia de desfase <{}>:", self.medida(a.desfase))
                } else if p.objetos.is_empty() {
                    "Designe el objeto a desfasar (Enter termina):".into()
                } else {
                    "Punto del lado hacia donde desfasar:".into()
                }
            }
            Id::Recortar => "Designe lo que se va a recortar (Enter termina):".into(),
            Id::Alargar => "Designe lo que se va a alargar (Enter termina):".into(),
            Id::Empalme => {
                if p.objetos.is_empty() {
                    format!(
                        "Primera linea (o escriba el radio) <radio {}>:",
                        self.medida(a.radio_empalme)
                    )
                } else {
                    "Segunda linea:".into()
                }
            }
            Id::Chaflan => {
                if p.objetos.is_empty() {
                    format!(
                        "Primera linea (o escriba la distancia) <{}>:",
                        self.medida(a.distancia_chaflan)
                    )
                } else {
                    "Segunda linea:".into()
                }
            }
            Id::MatrizRect => [
                "Numero de filas <1>:",
                "Numero de columnas <1>:",
                "Distancia entre filas:",
                "Distancia entre columnas:",
            ][p.numeros.len().min(3)]
            .into(),
            Id::MatrizPolar => {
                if n == 0 {
                    "Centro de la matriz:".into()
                } else if p.numeros.is_empty() {
                    "Numero de elementos:".into()
                } else {
                    "Angulo a llenar <360>:".into()
                }
            }
            Id::Borrar | Id::Descomponer => "Enter para terminar:".into(),
            Id::Cota => [
                "Origen de la primera linea de extension:",
                "Origen de la segunda:",
                "Posicion de la linea de cota:",
            ][n.min(2)]
            .into(),
            Id::Nivel => {
                if n == 0 {
                    "Punto del nivel:".into()
                } else {
                    "Valor del nivel en metros <0.00>:".into()
                }
            }
            Id::Norte => "Centro de la flecha del norte:".into(),
            Id::Muro => {
                if n == 0 {
                    format!(
                        "Inicio del eje del muro (o escriba el espesor) <{}>:",
                        self.medida(a.espesor_muro)
                    )
                } else {
                    "Siguiente punto del eje [Cerrar/Deshacer] (Enter termina):".into()
                }
            }
            Id::Ejes => match p.textos.len() {
                0 => "Separaciones entre ejes verticales (ej. 5000,4500 o 3*6000):".into(),
                1 => "Separaciones entre ejes horizontales:".into(),
                _ => "Origen de la malla (eje 1-A):".into(),
            },
            Id::Puerta => [
                "Punto de la bisagra:",
                "Hacia donde va la hoja cerrada (a lo largo del muro):",
                "Lado hacia el que abre:",
            ][n.min(2)]
            .into(),
            Id::Ventana => {
                if n == 0 {
                    "Inicio de la ventana (en el eje del muro):".into()
                } else {
                    "Fin de la ventana:".into()
                }
            }
            Id::Lamina => match p.textos.len() {
                0 => format!("Formato [A4/A3/A2/A1/A0] <{}>:", a.formato_lamina),
                1 => format!("Escala 1/<{}>:", a.escala_lamina),
                _ => "Esquina inferior izquierda de la lamina:".into(),
            },
            Id::Distancia => {
                if n == 0 {
                    "Primer punto:".into()
                } else {
                    "Segundo punto:".into()
                }
            }
            Id::Area => {
                if n == 0 {
                    "Primer vertice (o O para un objeto):".into()
                } else {
                    "Siguiente vertice (Enter calcula):".into()
                }
            }
            Id::Deshacer | Id::Rehacer => String::new(),
        }
    }

    // ------------------------------------------------------------- entradas

    /// **Lo que se escribe** en la linea de ordenes y Enter. Sin orden, el
    /// nombre de una; vacio, repite la ultima.
    pub fn escribir(&mut self, texto: &str) {
        let Some(id) = self.activa else {
            if texto.trim().is_empty() {
                if let Some(u) = self.ultima {
                    self.orden(u);
                }
                return;
            }
            match ordenes::buscar(texto) {
                Some(o) => self.orden(o.id),
                None => self.decir(format!("Orden desconocida: «{}»", texto.trim())),
            }
            return;
        };
        // Lo que se pide tal cual (un texto, una lista de separaciones),
        // aunque parezca un numero o un punto.
        let tal_cual = match id {
            Id::Texto => self.paso.puntos.len() == 1 && !self.paso.numeros.is_empty(),
            Id::Ejes => self.paso.textos.len() < 2,
            _ => false,
        };
        if tal_cual && !texto.trim().is_empty() {
            self.opcion(id, texto);
            return;
        }
        let e = entrada::interpretar(texto, self.base());
        match e {
            Entrada::Punto(p) => self.punto(p),
            Entrada::Vacio => self.enter(),
            Entrada::Numero(v) => self.numero(id, v),
            Entrada::Texto(t) => self.opcion(id, &t),
        }
    }

    /// **Un clic en el dibujo** (ya ajustado con [`Sesion::ajustar`]). `tol` es
    /// la tolerancia para tomar un objeto, en unidades del dibujo.
    pub fn clic(&mut self, p: P, tol: f64) {
        if let Some((h, i, _)) = self.pinzamiento.take() {
            self.dibujo.editar("PINZAMIENTO", |t| {
                t.cambiar(h, |e| {
                    op::mover_pinzamiento(e, i, p);
                })
            });
            return;
        }
        if self.pide_objetos() {
            self.tomar_objeto(p, tol);
            return;
        }
        self.punto(p);
    }

    /// Un clic sin orden ni objeto debajo, sobre un pinzamiento: lo toma.
    pub fn tomar_pinzamiento(&mut self, p: P, tol: f64) -> bool {
        if self.activa.is_some() {
            return false;
        }
        for (h, i, q) in self.pinzamientos() {
            if geo::dist(p, q) <= tol {
                self.pinzamiento = Some((h, i, q));
                return true;
            }
        }
        false
    }

    /// Los pinzamientos que se ven (de lo elegido, sin orden en marcha).
    pub fn pinzamientos(&self) -> Vec<(Handle, usize, P)> {
        if self.activa.is_some() || self.seleccion.len() > 100 {
            return Vec::new();
        }
        self.seleccion
            .iter()
            .filter_map(|h| Some((*h, self.dibujo.entidad(*h)?)))
            .flat_map(|(h, e)| {
                op::pinzamientos(e)
                    .into_iter()
                    .enumerate()
                    .map(move |(i, p)| (h, i, p))
            })
            .collect()
    }

    fn tomar_objeto(&mut self, p: P, tol: f64) {
        let h = crate::seleccion::elegir(&self.dibujo.indice(), p, tol);
        let Some(id) = self.activa else {
            // Sin orden: elegir (o soltar lo que ya estaba).
            match h {
                Some(h) => match self.seleccion.iter().position(|x| *x == h) {
                    Some(i) => {
                        self.seleccion.remove(i);
                    }
                    None => self.seleccion.push(h),
                },
                None => self.seleccion.clear(),
            }
            return;
        };
        let Some(h) = h else { return };
        match id {
            Id::Recortar => self.recortar_o_alargar(h, p, true),
            Id::Alargar => self.recortar_o_alargar(h, p, false),
            Id::Empalme | Id::Chaflan => {
                if op::extremos_de_linea(
                    self.dibujo
                        .entidad(h)
                        .unwrap_or(&EntityType::Line(Default::default())),
                )
                .is_none()
                {
                    self.decir("Solo entre lineas");
                    return;
                }
                if self.paso.objetos.first().is_some_and(|(x, _)| *x == h) {
                    return;
                }
                self.paso.objetos.push((h, p));
                if self.paso.objetos.len() == 2 {
                    self.empalmar(id);
                }
            }
            Id::Desfase => self.paso.objetos.push((h, p)),
            _ => {
                if !self.seleccion.contains(&h) {
                    self.seleccion.push(h);
                }
            }
        }
    }

    /// **Un recuadro arrastrado** de `a` a `b` (ventana o captura).
    pub fn recuadro(&mut self, a: P, b: P) {
        let hs = crate::seleccion::por_recuadro(&self.dibujo.indice(), a, b);
        match self.activa {
            Some(Id::Recortar) | Some(Id::Alargar) => {
                // Con captura, todo lo que cruza el recuadro (por el punto del cruce).
                let recortar = self.activa == Some(Id::Recortar);
                for h in hs {
                    let medio = geo::medio(a, b);
                    self.recortar_o_alargar(h, medio, recortar);
                }
            }
            _ if self.pide_objetos() => {
                for h in hs {
                    if !self.seleccion.contains(&h) {
                        self.seleccion.push(h);
                    }
                }
            }
            _ => {}
        }
    }

    /// Elige todo lo que se puede elegir (Ctrl+A).
    pub fn elegir_todo(&mut self) {
        self.seleccion = self
            .dibujo
            .indice()
            .entradas
            .iter()
            .filter(|e| !e.bloqueada)
            .map(|e| e.handle)
            .collect();
    }

    /// Borra lo elegido (Supr).
    pub fn borrar_seleccion(&mut self) {
        if self.seleccion.is_empty() || self.activa.is_some() {
            return;
        }
        let hs = std::mem::take(&mut self.seleccion);
        let n = self
            .dibujo
            .editar("BORRAR", |t| hs.iter().filter(|h| t.quitar(**h)).count());
        self.decir(format!("{n} objetos borrados"));
    }

    /// Pasa lo elegido a la capa `capa`.
    pub fn poner_capa(&mut self, capa: &str) {
        self.dibujo.asegurar_capa(capa, esp::color_de_capa(capa));
        let hs = self.seleccion.clone();
        let handle_capa = self.dibujo.doc.layers.get(capa).map(|l| l.handle);
        self.dibujo.editar("CAPA", |t| {
            for h in hs {
                t.cambiar(h, |e| {
                    e.common_mut().layer = capa.to_string();
                    e.common_mut().layer_handle = handle_capa;
                });
            }
        });
    }

    fn enter(&mut self) {
        let Some(id) = self.activa else { return };
        if ordenes::de(id).con_objetos && !self.paso.elegidos {
            if self.seleccion.is_empty() {
                self.decir("No hay objetos elegidos");
                self.cancelar_silencioso();
                return;
            }
            self.paso.elegidos = true;
            self.decir(format!("{} objetos elegidos", self.seleccion.len()));
            if matches!(id, Id::Borrar | Id::Descomponer) {
                self.terminar_con_objetos();
            }
            return;
        }
        match id {
            Id::Texto if self.paso.puntos.len() == 1 && self.paso.numeros.is_empty() => {
                self.paso.numeros.push(self.ajustes.alto_texto);
            }
            Id::Desfase if self.paso.numeros.is_empty() => {
                self.paso.numeros.push(self.ajustes.desfase);
            }
            Id::Simetria if self.paso.puntos.len() == 2 => self.opcion(id, "N"),
            Id::MatrizRect => {
                // Los que faltan, con lo de siempre (1 fila, 1 columna, sin separacion).
                let n = self.paso.numeros.len();
                if n < 2 {
                    self.paso.numeros.push(1.0);
                } else if n < 4 {
                    self.paso.numeros.push(0.0);
                }
                self.seguir_matriz();
            }
            Id::MatrizPolar if self.paso.numeros.len() == 1 => self.numero(id, 360.0),
            Id::Nivel if self.paso.puntos.len() == 1 => self.numero(id, 0.0),
            Id::Lamina if self.paso.textos.len() < 2 => {
                let defecto = if self.paso.textos.is_empty() {
                    self.ajustes.formato_lamina.clone()
                } else {
                    self.ajustes.escala_lamina.to_string()
                };
                self.paso.textos.push(defecto);
            }
            Id::Area => self.calcular_area(),
            _ => {
                self.cerrar_pendiente();
                self.cancelar_silencioso();
            }
        }
    }

    fn numero(&mut self, id: Id, v: f64) {
        let n = self.paso.puntos.len();
        let base = self.base();
        match id {
            Id::Circulo if n == 1 => {
                let c = self.paso.puntos[0];
                if v > 0.0 {
                    self.crear("CIRCULO", vec![esp::circulo(c, v)], None);
                    self.cancelar_silencioso();
                }
            }
            Id::Texto if n == 1 && self.paso.numeros.is_empty() => {
                if v > 0.0 {
                    self.ajustes.alto_texto = v;
                    self.paso.numeros.push(v);
                }
            }
            Id::Girar if n == 1 => {
                self.paso.numeros.push(v.to_radians());
                self.terminar_con_objetos();
            }
            Id::Escalar if n == 1 => {
                if v > 0.0 {
                    self.paso.numeros.push(v);
                    self.terminar_con_objetos();
                }
            }
            Id::Desfase if self.paso.numeros.is_empty() => {
                if v > 0.0 {
                    self.ajustes.desfase = v;
                    self.paso.numeros.push(v);
                }
            }
            Id::Empalme if self.paso.objetos.is_empty() => {
                self.ajustes.radio_empalme = v.max(0.0);
                self.decir(format!("Radio de empalme = {}", self.medida(v.max(0.0))));
            }
            Id::Chaflan if self.paso.objetos.is_empty() => {
                self.ajustes.distancia_chaflan = v.max(0.0);
            }
            Id::MatrizRect => {
                let k = self.paso.numeros.len();
                if k < 2 && (v < 1.0 || v.fract() != 0.0) {
                    self.decir("Tiene que ser un numero entero, 1 o mas");
                    return;
                }
                self.paso.numeros.push(v);
                self.seguir_matriz();
            }
            Id::MatrizPolar if n == 1 => {
                if self.paso.numeros.is_empty() {
                    if v >= 2.0 && v.fract() == 0.0 {
                        self.paso.numeros.push(v);
                    }
                } else {
                    self.paso.numeros.push(v.to_radians());
                    self.terminar_con_objetos();
                }
            }
            Id::Muro if n == 0 => {
                if v > 0.0 {
                    self.ajustes.espesor_muro = v;
                    self.decir(format!("Espesor de muro = {}", self.medida(v)));
                }
            }
            Id::Nivel if n == 1 => {
                let p = self.paso.puntos[0];
                let alto = self.ajustes.alto_texto;
                self.crear("NIVEL", esp::nivel(p, v, alto), Some(esp::CAPA_NIVELES));
                self.cancelar_silencioso();
            }
            Id::Ejes | Id::Lamina => self.opcion(id, &v.to_string()),
            // Una distancia sola: en la direccion del cursor desde la base.
            _ => {
                if let Some(b) = base {
                    let dir = self.direccion.unwrap_or([1.0, 0.0]);
                    self.punto(geo::sumar(b, geo::por(dir, v)));
                }
            }
        }
    }

    fn opcion(&mut self, id: Id, t: &str) {
        let up = ordenes::sin_tildes(t).to_uppercase();
        match id {
            Id::Linea | Id::Polilinea | Id::Muro if up == "C" || up == "CERRAR" => {
                if self.paso.puntos.len() >= 3 {
                    self.cerrar(id, true);
                }
                self.cancelar_silencioso();
            }
            Id::Linea | Id::Polilinea | Id::Muro if up == "D" || up == "U" || up == "DESHACER" => {
                // En LINEA cada tramo ya esta dibujado: se deshace tambien.
                let habia_tramo = self.paso.puntos.len() >= 2;
                self.paso.puntos.pop();
                if id == Id::Linea && habia_tramo {
                    self.dibujo.deshacer();
                }
            }
            Id::Simetria if self.paso.puntos.len() == 2 => {
                let borrar = up.starts_with('S') || up.starts_with('Y');
                self.paso.numeros.push(if borrar { 1.0 } else { 0.0 });
                self.terminar_con_objetos();
            }
            Id::Texto if self.paso.puntos.len() == 1 && !self.paso.numeros.is_empty() => {
                let p = self.paso.puntos[0];
                let alto = self.paso.numeros[0];
                self.crear(
                    "TEXTO",
                    vec![esp::texto(t.trim(), p, alto, 0.0, false)],
                    None,
                );
                self.cancelar_silencioso();
            }
            Id::Ejes if self.paso.textos.len() < 2 => match entrada::lista(t) {
                Some(_) => self.paso.textos.push(t.to_string()),
                None => self.decir("Escriba las separaciones con numeros, ej. 5000,4500"),
            },
            Id::Lamina if self.paso.textos.is_empty() => match esp::formato(t) {
                Some(_) => {
                    self.ajustes.formato_lamina = up.clone();
                    self.paso.textos.push(up);
                }
                None => self.decir("Formatos: A4, A3, A2, A1, A0"),
            },
            Id::Lamina if self.paso.textos.len() == 1 => {
                let limpio = t.trim().trim_start_matches("1/").trim_start_matches("1:");
                match limpio.parse::<f64>() {
                    Ok(e) if e > 0.0 && e.is_finite() => {
                        self.ajustes.escala_lamina = e;
                        self.paso.textos.push(e.to_string());
                    }
                    _ => self.decir("Escriba la escala, ej. 50 para 1/50"),
                }
            }
            Id::Area if self.paso.puntos.is_empty() && (up == "O" || up == "OBJETO") => {
                // La de lo elegido (o lo siguiente que se elija).
                self.area_de_seleccion();
            }
            _ => self.decir(format!("Opcion no valida: «{}»", t.trim())),
        }
    }

    // ------------------------------------------------------------ los puntos

    fn punto(&mut self, p: P) {
        let Some(id) = self.activa else { return };
        if ordenes::de(id).con_objetos && !self.paso.elegidos {
            return;
        }
        if let Some(b) = self.base() {
            let d = geo::restar(p, b);
            if geo::norma(d) > geo::EPS {
                self.direccion = Some(geo::unitario(d));
            }
        }
        self.paso.puntos.push(p);
        let n = self.paso.puntos.len();
        let pts = self.paso.puntos.clone();
        match id {
            Id::Linea if n >= 2 => {
                self.crear("LINEA", vec![esp::linea(pts[n - 2], pts[n - 1])], None);
            }
            Id::Rectangulo if n == 2 => {
                let (a, b) = (pts[0], pts[1]);
                self.crear(
                    "RECTANGULO",
                    vec![esp::polilinea(&[a, [b[0], a[1]], b, [a[0], b[1]]], true)],
                    None,
                );
                self.cancelar_silencioso();
            }
            Id::Circulo if n == 2 => {
                let r = geo::dist(pts[0], pts[1]);
                if r > geo::EPS {
                    self.crear("CIRCULO", vec![esp::circulo(pts[0], r)], None);
                }
                self.cancelar_silencioso();
            }
            Id::Arco if n == 3 => {
                if let Some(Forma::Arco {
                    c,
                    r,
                    inicio,
                    barrido,
                }) = Forma::arco_por_tres(pts[0], pts[1], pts[2])
                {
                    self.crear(
                        "ARCO",
                        vec![esp::arco(c, r, inicio, inicio + barrido)],
                        None,
                    );
                } else {
                    self.decir("Los tres puntos estan alineados");
                }
                self.cancelar_silencioso();
            }
            Id::Texto if n > 1 => {
                self.paso.puntos.truncate(1);
            }
            Id::Mover | Id::Copiar if n >= 2 => self.terminar_con_objetos(),
            Id::Girar if n == 2 => {
                let a = geo::angulo(pts[0], pts[1]);
                self.paso.puntos.pop();
                self.paso.numeros.push(a);
                self.terminar_con_objetos();
            }
            Id::Escalar if n == 2 => {
                self.paso.puntos.pop();
                self.decir("Escriba el factor de escala (un numero)");
            }
            Id::Simetria if n > 2 => {
                self.paso.puntos.truncate(2);
            }
            Id::Desfase => {
                self.paso.puntos.clear();
                if let Some((h, _)) = self.paso.objetos.pop() {
                    let d = self
                        .paso
                        .numeros
                        .first()
                        .copied()
                        .unwrap_or(self.ajustes.desfase);
                    let nueva = self.dibujo.entidad(h).and_then(|e| op::desfase(e, d, p));
                    match nueva {
                        Some(e) => {
                            self.dibujo.editar("DESFASE", |t| t.anadir(e));
                        }
                        None => self.decir("Ese objeto no se puede desfasar"),
                    }
                }
            }
            Id::MatrizPolar if n > 1 => {
                self.paso.puntos.truncate(1);
            }
            Id::Cota if n == 3 => {
                let alto = self.ajustes.alto_texto;
                let dec = self.ajustes.decimales;
                self.crear(
                    "COTA",
                    esp::cota(pts[0], pts[1], pts[2], alto, dec),
                    Some(esp::CAPA_COTAS),
                );
                self.cancelar_silencioso();
            }
            Id::Nivel if n > 1 => {
                self.paso.puntos.truncate(1);
            }
            Id::Norte => {
                let r = self.ajustes.alto_texto * 3.0;
                self.crear("NORTE", esp::norte(p, r, 0.0), Some(esp::CAPA_TEXTOS));
                self.cancelar_silencioso();
            }
            Id::Ejes => {
                if self.paso.textos.len() < 2 {
                    self.paso.puntos.clear();
                    return;
                }
                let sx = entrada::lista(&self.paso.textos[0]).unwrap_or_default();
                let sy = entrada::lista(&self.paso.textos[1]).unwrap_or_default();
                let r = self.ajustes.alto_texto * 1.6;
                self.crear("EJES", esp::ejes(p, &sx, &sy, r), Some(esp::CAPA_EJES));
                self.cancelar_silencioso();
            }
            Id::Puerta if n == 3 => {
                let izquierda =
                    geo::cruz(geo::restar(pts[1], pts[0]), geo::restar(pts[2], pts[0])) >= 0.0;
                let ancho = self.ajustes.ancho_puerta;
                self.crear(
                    "PUERTA",
                    esp::puerta(pts[0], pts[1], ancho, izquierda),
                    Some(esp::CAPA_PUERTAS),
                );
                self.cancelar_silencioso();
            }
            Id::Ventana if n == 2 => {
                let e = self.ajustes.espesor_muro;
                self.crear(
                    "VENTANA",
                    esp::ventana(pts[0], pts[1], e),
                    Some(esp::CAPA_VENTANAS),
                );
                self.cancelar_silencioso();
            }
            Id::Lamina => {
                if self.paso.textos.len() < 2 {
                    self.paso.puntos.clear();
                    return;
                }
                let (w, h) = esp::formato(&self.paso.textos[0]).unwrap_or((841.0, 594.0));
                let escala: f64 = self.paso.textos[1].parse().unwrap_or(50.0);
                let k = esp::unidades_por_mm(escala, self.dibujo.doc.header.insertion_units);
                let datos = esp::Membrete {
                    proyecto: self
                        .dibujo
                        .ruta
                        .as_ref()
                        .and_then(|r| r.file_stem())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    ..Default::default()
                };
                self.crear(
                    "LAMINA",
                    esp::lamina(p, w, h, k, escala, &datos),
                    Some(esp::CAPA_LAMINA),
                );
                self.cancelar_silencioso();
            }
            Id::Distancia if n == 2 => {
                let (a, b) = (pts[0], pts[1]);
                let d = geo::restar(b, a);
                self.decir(format!(
                    "Distancia = {}   ΔX = {}   ΔY = {}   Angulo = {:.2}°",
                    self.medida(geo::norma(d)),
                    self.medida(d[0]),
                    self.medida(d[1]),
                    geo::normalizar(geo::angulo(a, b)).to_degrees()
                ));
                self.cancelar_silencioso();
            }
            _ => {}
        }
    }

    /// Cierra lo que una orden de varios puntos tenia pendiente al terminar.
    fn cerrar_pendiente(&mut self) {
        if let Some(id) = self.activa
            && matches!(id, Id::Polilinea | Id::Muro)
            && self.paso.puntos.len() >= 2
        {
            self.cerrar(id, false);
        }
    }

    fn cerrar(&mut self, id: Id, cerrada: bool) {
        let pts = std::mem::take(&mut self.paso.puntos);
        match id {
            Id::Linea if cerrada => {
                self.crear(
                    "LINEA",
                    vec![esp::linea(*pts.last().unwrap(), pts[0])],
                    None,
                );
            }
            Id::Polilinea => {
                self.crear("POLILINEA", vec![esp::polilinea(&pts, cerrada)], None);
            }
            Id::Muro => {
                let e = self.ajustes.espesor_muro;
                self.crear("MURO", esp::muro(&pts, e, cerrada), Some(esp::CAPA_MUROS));
            }
            _ => {}
        }
    }

    /// Anade entidades nuevas, en `capa` (o en la actual).
    fn crear(&mut self, nombre: &str, v: Vec<EntityType>, capa: Option<&str>) {
        let capa = match capa {
            Some(c) => {
                self.dibujo.asegurar_capa(c, esp::color_de_capa(c));
                c.to_string()
            }
            None => self.dibujo.capa_actual(),
        };
        self.dibujo.editar(nombre, |t| {
            for e in v {
                t.anadir_en(e, &capa);
            }
        });
    }

    // ----------------------------------------------------- con lo elegido

    fn seguir_matriz(&mut self) {
        if self.paso.numeros.len() >= 4 {
            self.terminar_con_objetos();
        } else if self.paso.numeros.len() == 2 && self.paso.numeros[0] <= 1.0 {
            // Una sola fila: no hace falta su separacion.
            self.paso.numeros.push(0.0);
        }
    }

    /// Las entidades elegidas, copiadas.
    fn elegidas(&self) -> Vec<(Handle, EntityType)> {
        self.seleccion
            .iter()
            .filter_map(|h| Some((*h, self.dibujo.entidad(*h)?.clone())))
            .collect()
    }

    /// Lo que hace una orden con objetos a lo elegido. `cursor`: la vista
    /// previa (sin cursor, lo de verdad).
    fn transformar(&self, id: Id, e: &mut EntityType, cursor: Option<P>) -> bool {
        let p = &self.paso;
        let base = p.puntos.first().copied();
        match id {
            Id::Mover | Id::Copiar => {
                let (Some(b), Some(q)) = (base, cursor.or(p.puntos.last().copied())) else {
                    return false;
                };
                op::trasladar(e, geo::restar(q, b));
            }
            Id::Girar => {
                let Some(b) = base else { return false };
                let a = match cursor {
                    Some(q) => geo::angulo(b, q),
                    None => p.numeros.first().copied().unwrap_or(0.0),
                };
                op::girar(e, b, a);
            }
            Id::Escalar => {
                let Some(b) = base else { return false };
                let k = match cursor {
                    // En la previa: tantas veces como la distancia al cursor en
                    // unidades del alto de texto (para ver algo que cambia).
                    Some(q) => {
                        (geo::dist(b, q) / self.ajustes.alto_texto.max(geo::EPS) / 4.0).max(0.01)
                    }
                    None => p.numeros.first().copied().unwrap_or(1.0),
                };
                op::escalar(e, b, k);
            }
            Id::Simetria => {
                let (Some(a), Some(b)) = (base, cursor.or(p.puntos.get(1).copied())) else {
                    return false;
                };
                if geo::dist(a, b) < geo::EPS {
                    return false;
                }
                op::simetria(e, a, b);
            }
            _ => return false,
        }
        true
    }

    fn terminar_con_objetos(&mut self) {
        let Some(id) = self.activa else { return };
        let elegidas = self.elegidas();
        let nombre = ordenes::de(id).nombre;
        match id {
            Id::Borrar => {
                let n = self.dibujo.editar(nombre, |t| {
                    elegidas.iter().filter(|(h, _)| t.quitar(*h)).count()
                });
                self.decir(format!("{n} objetos borrados"));
                self.seleccion.clear();
                self.cancelar_silencioso();
            }
            Id::Descomponer => {
                let nuevas = self.dibujo.editar(nombre, |t| {
                    elegidas
                        .iter()
                        .flat_map(|(h, _)| t.descomponer(*h))
                        .collect::<Vec<_>>()
                });
                self.decir(format!("{} piezas", nuevas.len()));
                self.seleccion = nuevas;
                self.cancelar_silencioso();
            }
            Id::Mover | Id::Girar | Id::Escalar => {
                let cambios: Vec<(Handle, EntityType)> = elegidas
                    .into_iter()
                    .filter_map(|(h, mut e)| self.transformar(id, &mut e, None).then_some((h, e)))
                    .collect();
                self.dibujo.editar(nombre, |t| {
                    for (h, e) in cambios {
                        t.reemplazar(h, e);
                    }
                });
                self.cancelar_silencioso();
            }
            Id::Copiar => {
                // Copiar sigue pidiendo destinos hasta Enter.
                let copias: Vec<EntityType> = elegidas
                    .into_iter()
                    .filter_map(|(_, mut e)| self.transformar(id, &mut e, None).then_some(e))
                    .collect();
                self.dibujo.editar(nombre, |t| {
                    for e in copias {
                        t.anadir(e);
                    }
                });
                self.paso.puntos.truncate(1);
            }
            Id::Simetria => {
                let borrar = self.paso.numeros.first().copied().unwrap_or(0.0) > 0.5;
                let copias: Vec<(Handle, EntityType)> = elegidas
                    .into_iter()
                    .filter_map(|(h, mut e)| self.transformar(id, &mut e, None).then_some((h, e)))
                    .collect();
                self.dibujo.editar(nombre, |t| {
                    for (h, e) in copias {
                        if borrar {
                            t.reemplazar(h, e);
                        } else {
                            t.anadir(e);
                        }
                    }
                });
                self.cancelar_silencioso();
            }
            Id::MatrizRect => {
                let n = &self.paso.numeros;
                let (filas, cols, dy, dx) = (n[0] as u32, n[1] as u32, n[2], n[3]);
                let copias: Vec<EntityType> = elegidas
                    .iter()
                    .flat_map(|(_, e)| op::matriz_rectangular(e, filas, cols, dx, dy))
                    .collect();
                self.dibujo.editar(nombre, |t| {
                    for e in copias {
                        t.anadir(e);
                    }
                });
                self.cancelar_silencioso();
            }
            Id::MatrizPolar => {
                let (Some(c), Some(&n), Some(&ang)) = (
                    self.paso.puntos.first().copied(),
                    self.paso.numeros.first(),
                    self.paso.numeros.get(1),
                ) else {
                    return;
                };
                let copias: Vec<EntityType> = elegidas
                    .iter()
                    .flat_map(|(_, e)| op::matriz_polar(e, c, n as u32, ang))
                    .collect();
                self.dibujo.editar(nombre, |t| {
                    for e in copias {
                        t.anadir(e);
                    }
                });
                self.cancelar_silencioso();
            }
            _ => {}
        }
        self.limpiar_seleccion_borrada();
    }

    fn recortar_o_alargar(&mut self, h: Handle, p: P, recortar: bool) {
        let ind = self.dibujo.indice();
        let Some(e) = self.dibujo.entidad(h).cloned() else {
            return;
        };
        if recortar {
            match op::recortar(&ind, h, &e, p, None) {
                Some(trozos) => {
                    self.dibujo.editar("RECORTAR", |t| {
                        t.quitar(h);
                        for k in trozos {
                            t.anadir(k);
                        }
                    });
                }
                None => self.decir("Nada lo cruza: no hay que recortar"),
            }
        } else {
            match op::alargar(&ind, h, &e, p, None) {
                Some(n) => {
                    self.dibujo.editar("ALARGAR", |t| t.reemplazar(h, n));
                }
                None => self.decir("No hay nada hasta donde alargar"),
            }
        }
    }

    fn empalmar(&mut self, id: Id) {
        let [(h1, p1), (h2, p2)] = [self.paso.objetos[0], self.paso.objetos[1]];
        let (Some(e1), Some(e2)) = (
            self.dibujo.entidad(h1).cloned(),
            self.dibujo.entidad(h2).cloned(),
        ) else {
            return;
        };
        let (Some(l1), Some(l2)) = (op::extremos_de_linea(&e1), op::extremos_de_linea(&e2)) else {
            return;
        };
        let r = if id == Id::Empalme {
            op::empalme(l1, p1, l2, p2, self.ajustes.radio_empalme)
        } else {
            let d = self.ajustes.distancia_chaflan;
            op::chaflan(l1, p1, l2, p2, d, d)
        };
        let capa = e1.common().layer.clone();
        match r {
            Some((a, b, extra)) => {
                self.dibujo.editar(ordenes::de(id).nombre, |t| {
                    t.reemplazar(h1, op::con_extremos(&e1, a.0, a.1));
                    t.reemplazar(h2, op::con_extremos(&e2, b.0, b.1));
                    if let Some(x) = extra {
                        t.anadir_en(x, &capa);
                    }
                });
            }
            None => self.decir("Las lineas son paralelas"),
        }
        self.cancelar_silencioso();
    }

    fn calcular_area(&mut self) {
        let pts = std::mem::take(&mut self.paso.puntos);
        if pts.len() >= 3 {
            let a = geo::area_poligono(&pts).abs();
            let per: f64 = (0..pts.len())
                .map(|i| geo::dist(pts[i], pts[(i + 1) % pts.len()]))
                .sum();
            self.decir(format!(
                "Area = {}   Perimetro = {}",
                self.medida(a),
                self.medida(per)
            ));
        }
        self.cancelar_silencioso();
    }

    fn area_de_seleccion(&mut self) {
        let mut total = 0.0;
        let mut n = 0;
        for (_, e) in self.elegidas() {
            if let (_, Some(a)) = formas::largo_y_area(&e) {
                total += a;
                n += 1;
            }
        }
        if n == 0 {
            self.decir("Elija antes una polilinea cerrada o un circulo");
        } else {
            self.decir(format!("Area de {n} objetos = {}", self.medida(total)));
        }
        self.cancelar_silencioso();
    }

    // ------------------------------------------------------- vista previa

    /// **La vista previa** con el cursor (ya ajustado) en `c`.
    pub fn previa(&self, c: P) -> Previa {
        let mut v = Previa::default();
        if let Some((h, i, p)) = self.pinzamiento {
            if let Some(mut e) = self.dibujo.entidad(h).cloned()
                && op::mover_pinzamiento(&mut e, i, c)
            {
                v.formas = formas::formas(&e);
            }
            v.guias.push((p, c));
            return v;
        }
        let Some(id) = self.activa else { return v };
        let p = &self.paso;
        let pts = &p.puntos;
        let n = pts.len();
        let entidades: Vec<EntityType> = match id {
            Id::Linea if n >= 1 => vec![esp::linea(pts[n - 1], c)],
            Id::Polilinea if n >= 1 => {
                let mut q = pts.clone();
                q.push(c);
                vec![esp::polilinea(&q, false)]
            }
            Id::Muro if n >= 1 => {
                let mut q = pts.clone();
                q.push(c);
                esp::muro(&q, self.ajustes.espesor_muro, false)
            }
            Id::Rectangulo if n == 1 => {
                let a = pts[0];
                vec![esp::polilinea(&[a, [c[0], a[1]], c, [a[0], c[1]]], true)]
            }
            Id::Circulo if n == 1 => vec![esp::circulo(pts[0], geo::dist(pts[0], c).max(geo::EPS))],
            Id::Arco if n == 1 => vec![esp::linea(pts[0], c)],
            Id::Arco if n == 2 => match Forma::arco_por_tres(pts[0], pts[1], c) {
                Some(Forma::Arco {
                    c: k,
                    r,
                    inicio,
                    barrido,
                }) => vec![esp::arco(k, r, inicio, inicio + barrido)],
                _ => vec![esp::linea(pts[0], c)],
            },
            Id::Cota if n == 1 => vec![esp::linea(pts[0], c)],
            Id::Cota if n == 2 => esp::cota(
                pts[0],
                pts[1],
                c,
                self.ajustes.alto_texto,
                self.ajustes.decimales,
            ),
            Id::Puerta if n == 1 => esp::puerta(pts[0], c, self.ajustes.ancho_puerta, true),
            Id::Puerta if n == 2 => {
                let izq = geo::cruz(geo::restar(pts[1], pts[0]), geo::restar(c, pts[0])) >= 0.0;
                esp::puerta(pts[0], pts[1], self.ajustes.ancho_puerta, izq)
            }
            Id::Ventana if n == 1 => esp::ventana(pts[0], c, self.ajustes.espesor_muro),
            Id::Norte => esp::norte(c, self.ajustes.alto_texto * 3.0, 0.0),
            Id::Ejes if p.textos.len() >= 2 => esp::ejes(
                c,
                &entrada::lista(&p.textos[0]).unwrap_or_default(),
                &entrada::lista(&p.textos[1]).unwrap_or_default(),
                self.ajustes.alto_texto * 1.6,
            ),
            Id::Lamina if p.textos.len() >= 2 => {
                let (w, h) = esp::formato(&p.textos[0]).unwrap_or((841.0, 594.0));
                let escala: f64 = p.textos[1].parse().unwrap_or(50.0);
                let k = esp::unidades_por_mm(escala, self.dibujo.doc.header.insertion_units);
                vec![esp::polilinea(
                    &[
                        c,
                        [c[0] + w * k, c[1]],
                        [c[0] + w * k, c[1] + h * k],
                        [c[0], c[1] + h * k],
                    ],
                    true,
                )]
            }
            Id::Area if n >= 1 => {
                let mut q = pts.clone();
                q.push(c);
                vec![esp::polilinea(&q, true)]
            }
            Id::Distancia if n == 1 => vec![esp::linea(pts[0], c)],
            Id::Mover | Id::Copiar | Id::Girar | Id::Escalar | Id::Simetria
                if p.elegidos && n >= 1 =>
            {
                let elegidas = self.elegidas();
                elegidas
                    .into_iter()
                    .take(2000)
                    .filter_map(|(_, mut e)| self.transformar(id, &mut e, Some(c)).then_some(e))
                    .collect()
            }
            _ => Vec::new(),
        };
        for e in &entidades {
            v.formas.extend(formas::formas(e));
        }
        if let Some(b) = self.base() {
            v.guias.push((b, c));
            let d = geo::dist(b, c);
            let ang = geo::normalizar(geo::angulo(b, c)).to_degrees();
            v.etiquetas
                .push((c, format!("{}  <{ang:.1}°", self.medida(d))));
        }
        v
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn sesion() -> Sesion {
        Sesion::nueva(Dibujo::nuevo())
    }

    fn cuantas(s: &Sesion) -> usize {
        s.dibujo.doc.model_space_entities().count()
    }

    #[test]
    fn linea_por_teclado_con_relativas_y_distancia() {
        let mut s = sesion();
        s.escribir("l");
        assert_eq!(s.activa(), Some(Id::Linea));
        s.escribir("0,0");
        s.escribir("@1000,0");
        // Una distancia sola sigue la ultima direccion.
        s.escribir("500");
        s.escribir("");
        assert_eq!(s.activa(), None);
        assert_eq!(cuantas(&s), 2);
        // Enter vacio repite la ultima orden.
        s.escribir("");
        assert_eq!(s.activa(), Some(Id::Linea));
        s.escape();
        assert!(s.registro.iter().any(|l| l.contains("LINEA")));
    }

    #[test]
    fn polilinea_cerrada_y_area() {
        let mut s = sesion();
        s.escribir("pl");
        for p in ["0,0", "4000,0", "4000,3000", "0,3000"] {
            s.escribir(p);
        }
        s.escribir("c");
        assert_eq!(cuantas(&s), 1);
        s.elegir_todo();
        s.escribir("area");
        s.escribir("o");
        assert!(
            s.registro.last().unwrap().contains("12000000"),
            "{:?}",
            s.registro.last()
        );
    }

    #[test]
    fn mover_lo_elegido_y_deshacer() {
        let mut s = sesion();
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("100");
        s.elegir_todo();
        s.escribir("m");
        s.clic([0.0, 0.0], 1.0);
        s.clic([50.0, 0.0], 1.0);
        let EntityType::Circle(c) = s.dibujo.doc.model_space_entities().next().unwrap() else {
            panic!()
        };
        assert_eq!(c.center.x, 50.0);
        s.escribir("u");
        let EntityType::Circle(c) = s.dibujo.doc.model_space_entities().next().unwrap() else {
            panic!()
        };
        assert_eq!(c.center.x, 0.0);
    }

    #[test]
    fn copiar_varias_veces_hasta_enter() {
        let mut s = sesion();
        s.escribir("c");
        s.escribir("0,0");
        s.escribir("10");
        s.elegir_todo();
        s.escribir("co");
        s.escribir("0,0");
        s.escribir("100,0");
        s.escribir("200,0");
        s.escribir("");
        assert_eq!(cuantas(&s), 3);
    }

    #[test]
    fn pedir_objetos_cuando_no_hay_seleccion() {
        let mut s = sesion();
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("100,0");
        s.escape();
        s.escribir("gi");
        assert!(s.pide_objetos());
        s.clic([50.0, 0.5], 2.0);
        s.escribir("");
        assert!(!s.pide_objetos());
        s.escribir("0,0");
        s.escribir("90");
        let EntityType::Line(l) = s.dibujo.doc.model_space_entities().next().unwrap() else {
            panic!()
        };
        assert!((l.end.y - 100.0).abs() < 1e-9 && l.end.x.abs() < 1e-9);
    }

    #[test]
    fn muro_con_espesor_escrito() {
        let mut s = sesion();
        s.escribir("muro");
        s.escribir("200");
        s.escribir("0,0");
        s.escribir("5000,0");
        s.escribir("5000,4000");
        s.escribir("");
        // Dos caras y dos tapas, en la capa de muros.
        let v: Vec<_> = s.dibujo.doc.model_space_entities().collect();
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|e| e.common().layer == esp::CAPA_MUROS));
        assert_eq!(s.ajustes.espesor_muro, 200.0);
    }

    #[test]
    fn ejes_y_lamina_por_teclado() {
        let mut s = sesion();
        s.escribir("ejes");
        s.escribir("5000,4000");
        s.escribir("6000");
        s.escribir("0,0");
        assert_eq!(cuantas(&s), 15);
        s.escribir("lamina");
        s.escribir("a3");
        s.escribir("100");
        s.escribir("-2000,-2000");
        assert!(
            s.dibujo
                .doc
                .model_space_entities()
                .any(|e| e.common().layer == esp::CAPA_LAMINA)
        );
    }

    #[test]
    fn recortar_con_clic() {
        let mut s = sesion();
        for (a, b) in [("0,0", "100,0"), ("50,-10", "50,10")] {
            s.escribir("l");
            s.escribir(a);
            s.escribir(b);
            s.escribir("");
        }
        s.escribir("tr");
        s.clic([80.0, 0.0], 2.0);
        s.escribir("");
        let largo: f64 = s
            .dibujo
            .doc
            .model_space_entities()
            .filter_map(|e| match e {
                EntityType::Line(l) if l.start.y == l.end.y => Some(l.length()),
                _ => None,
            })
            .sum();
        assert!((largo - 50.0).abs() < 1e-9);
    }

    #[test]
    fn empalme_con_radio() {
        let mut s = sesion();
        for (a, b) in [("0,0", "80,0"), ("100,20", "100,100")] {
            s.escribir("l");
            s.escribir(a);
            s.escribir(b);
            s.escribir("");
        }
        s.escribir("f");
        s.escribir("10");
        s.clic([10.0, 0.0], 2.0);
        s.clic([100.0, 90.0], 2.0);
        assert!(
            s.dibujo
                .doc
                .model_space_entities()
                .any(|e| matches!(e, EntityType::Arc(_)))
        );
    }

    #[test]
    fn pinzamiento_estira_una_linea() {
        let mut s = sesion();
        s.escribir("l");
        s.escribir("0,0");
        s.escribir("100,0");
        s.escape();
        s.clic([50.0, 0.0], 2.0);
        assert_eq!(s.seleccion.len(), 1);
        assert!(s.tomar_pinzamiento([100.0, 0.0], 2.0));
        s.clic([100.0, 50.0], 2.0);
        let EntityType::Line(l) = s.dibujo.doc.model_space_entities().next().unwrap() else {
            panic!()
        };
        assert_eq!(l.end.y, 50.0);
    }

    #[test]
    fn la_previa_del_rectangulo() {
        let mut s = sesion();
        s.escribir("rec");
        s.escribir("0,0");
        let v = s.previa([10.0, 5.0]);
        assert_eq!(v.formas.len(), 4);
        assert_eq!(v.guias.len(), 1);
    }

    #[test]
    fn orden_desconocida_no_rompe() {
        let mut s = sesion();
        s.escribir("zzz");
        assert_eq!(s.activa(), None);
        assert!(s.registro.last().unwrap().contains("desconocida"));
    }
}
