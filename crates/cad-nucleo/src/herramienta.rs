//! **Las herramientas enchufables**: una orden que vive en su propio modulo
//! de [`crate::herramientas`] y que la [`crate::Sesion`] guia como las de
//! siempre (pide los objetos, le pasa los puntos, numeros y textos, y pinta
//! su vista previa). Asi una orden nueva no toca la sesion.

use opencadcodec::{EntityType, Handle};

use crate::dibujo::Dibujo;
use crate::especialidad as esp;
use crate::geo::{Forma, P};
use crate::ordenes::Orden;
use crate::sesion::Ajustes;

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

impl Previa {
    /// Anade las formas de unas entidades.
    pub fn con(mut self, v: &[EntityType]) -> Previa {
        for e in v {
            self.formas.extend(crate::formas::formas(e));
        }
        self
    }
}

/// Que hace la sesion despues de pasarle algo a la herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flujo {
    /// Sigue pidiendo.
    Sigue,
    /// Termino: la orden se cierra.
    Termina,
    /// No lo esperaba: la sesion hace lo de siempre (un numero solo es una
    /// distancia en la direccion del cursor; un texto, una opcion no valida).
    NoEntiende,
}

/// Lo que una herramienta puede leer (para pintar su previa o su indicacion).
pub struct Lectura<'a> {
    pub dibujo: &'a Dibujo,
    pub seleccion: &'a [Handle],
    pub ajustes: &'a Ajustes,
}

impl Lectura<'_> {
    /// Las entidades elegidas, copiadas.
    pub fn elegidas(&self) -> Vec<(Handle, EntityType)> {
        elegidas(self.dibujo, self.seleccion)
    }

    pub fn medida(&self, d: f64) -> String {
        esp::texto_de_medida(d, self.ajustes.decimales.max(2))
    }
}

/// **Lo que la sesion recuerda para las ordenes enchufables** de una a otra
/// (y lo que estas le piden a la ventana).
#[derive(Debug, Clone, Default)]
pub struct Memoria {
    /// Lo que se pide a la ventana (ver todo, una ventana de zoom…): la app lo
    /// toma con `Option::take` en cada vuelta.
    pub vista: Option<crate::herramientas::vista::PedidoVista>,
    /// Lo elegido para la ultima orden con objetos (SELECCIONPREVIA).
    pub seleccion_previa: Vec<Handle>,
    /// Las capas que apago CAPAAISLAR (las enciende CAPADESAISLAR).
    pub capas_aisladas: Vec<String>,
}

/// Lo que una herramienta puede cambiar.
pub struct Contexto<'a> {
    pub dibujo: &'a mut Dibujo,
    pub seleccion: &'a mut Vec<Handle>,
    pub ajustes: &'a mut Ajustes,
    pub memoria: &'a mut Memoria,
    pub(crate) registro: &'a mut Vec<String>,
}

impl Contexto<'_> {
    /// Dice algo en el historial de la linea de ordenes.
    pub fn decir(&mut self, s: impl Into<String>) {
        self.registro.push(s.into());
        if self.registro.len() > 500 {
            self.registro.drain(..100);
        }
    }

    pub fn leer(&self) -> Lectura<'_> {
        Lectura {
            dibujo: self.dibujo,
            seleccion: self.seleccion,
            ajustes: self.ajustes,
        }
    }

    pub fn elegidas(&self) -> Vec<(Handle, EntityType)> {
        elegidas(self.dibujo, self.seleccion)
    }

    pub fn medida(&self, d: f64) -> String {
        esp::texto_de_medida(d, self.ajustes.decimales.max(2))
    }

    /// **Anade entidades nuevas** de una vez (un paso de deshacer), en `capa`
    /// (que se crea con el color de la plantilla si no esta) o en la actual.
    pub fn crear(&mut self, nombre: &str, v: Vec<EntityType>, capa: Option<&str>) -> Vec<Handle> {
        let capa = match capa {
            Some(c) => {
                self.dibujo.asegurar_capa(c, esp::color_de_capa(c));
                c.to_string()
            }
            None => self.dibujo.capa_actual(),
        };
        self.dibujo.editar(nombre, |t| {
            v.into_iter()
                .filter_map(|e| t.anadir_en(e, &capa))
                .collect()
        })
    }
}

fn elegidas(d: &Dibujo, sel: &[Handle]) -> Vec<(Handle, EntityType)> {
    sel.iter()
        .filter_map(|h| Some((*h, d.entidad(*h)?.clone())))
        .collect()
}

/// **Una orden enchufable.** Todo tiene un comportamiento por defecto: una
/// orden sencilla solo escribe `indicacion` y `punto`.
pub trait Herramienta {
    /// Lo que pide ahora (sale en la linea de ordenes).
    fn indicacion(&self, cx: &Lectura) -> String;

    /// El punto anterior (para orto, polar, la perpendicular, `@` y la guia).
    fn base(&self) -> Option<P> {
        None
    }

    /// Si un clic toma un objeto (va a [`Herramienta::objeto`]) en vez de ser un punto.
    fn pide_objeto(&self) -> bool {
        false
    }

    /// Si lo escrito va tal cual a [`Herramienta::texto`] (un texto con espacios).
    fn pide_texto(&self) -> bool {
        false
    }

    /// Nada mas empezar (para las que hacen lo suyo de una vez, como ZE o
    /// CAPAON): `Termina` la cierra sin pedir nada.
    fn al_empezar(&mut self, _cx: &mut Contexto) -> Flujo {
        Flujo::Sigue
    }

    /// Si la orden [`Orden::con_objetos`] ya tiene los suyos (elegidos antes
    /// o al Enter): para las que no piden nada mas.
    fn al_tener_objetos(&mut self, _cx: &mut Contexto) -> Flujo {
        Flujo::Sigue
    }

    fn punto(&mut self, _cx: &mut Contexto, _p: P) -> Flujo {
        Flujo::NoEntiende
    }

    /// Un objeto tomado con un clic en `p` (si [`Herramienta::pide_objeto`]).
    fn objeto(&mut self, _cx: &mut Contexto, _h: Handle, _p: P) -> Flujo {
        Flujo::NoEntiende
    }

    /// Un recuadro de `a` a `b` mientras [`Herramienta::pide_objeto`], con lo
    /// que eligio (ventana o captura segun el sentido).
    fn recuadro(&mut self, _cx: &mut Contexto, _hs: Vec<Handle>, _a: P, _b: P) -> Flujo {
        Flujo::NoEntiende
    }

    fn numero(&mut self, _cx: &mut Contexto, _v: f64) -> Flujo {
        Flujo::NoEntiende
    }

    /// Una opcion o un texto escrito.
    fn texto(&mut self, _cx: &mut Contexto, _t: &str) -> Flujo {
        Flujo::NoEntiende
    }

    /// Enter sin nada escrito (por defecto, terminar).
    fn enter(&mut self, _cx: &mut Contexto) -> Flujo {
        Flujo::Termina
    }

    /// Esc o otra orden: lo que va hecho se puede dejar (como una polilinea a medias).
    fn cancelar(&mut self, _cx: &mut Contexto) {}

    fn previa(&self, _cx: &Lectura, _cursor: P) -> Previa {
        Previa::default()
    }
}

/// **Una orden del catalogo**: sus datos y como se crea.
pub struct Registro {
    /// Con `id: Id::Otra(nombre)`.
    pub orden: Orden,
    /// El nombre bonito, con tildes (el de la cinta).
    pub etiqueta: &'static str,
    pub crear: fn() -> Box<dyn Herramienta>,
}
