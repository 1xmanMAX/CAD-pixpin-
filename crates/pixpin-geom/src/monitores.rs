//! La disposicion de monitores, como dato puro.
//!
//! Vive aqui y no en `pixpin-capture` porque la aritmetica del escritorio
//! virtual con DPI mixto es donde de verdad se cometen errores, y es logica
//! pura: se puede probar con disposiciones inventadas —tres monitores con
//! escalados distintos, coordenadas negativas, un monitor por encima del
//! principal— que costaria una fortuna reproducir en hardware.
//!
//! `pixpin-capture` se limita a rellenarla desde Win32.

use crate::punto::Punto;
use crate::rect::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monitor {
    /// Identificador estable dentro de una misma enumeracion.
    pub id: u32,
    /// Area completa en pixeles fisicos del escritorio virtual.
    pub area: Rect,
    /// Area util, sin la barra de tareas ni las barras acopladas.
    pub area_trabajo: Rect,
    /// Escalado en tanto por ciento: 100, 125, 150, 175, 200...
    ///
    /// Se guarda entero y no como `f32` para que `Monitor` pueda derivar `Eq`
    /// y compararse sin sorpresas de coma flotante.
    pub escala_por_cien: u32,
    pub principal: bool,
}

impl Monitor {
    /// Escalado como factor: 150 -> 1.5.
    pub fn escala(&self) -> f32 {
        self.escala_por_cien as f32 / 100.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DisposicionMonitores {
    monitores: Vec<Monitor>,
}

impl DisposicionMonitores {
    pub fn nueva(monitores: Vec<Monitor>) -> Self {
        Self { monitores }
    }

    pub fn monitores(&self) -> &[Monitor] {
        &self.monitores
    }

    pub fn principal(&self) -> Option<&Monitor> {
        self.monitores.iter().find(|m| m.principal)
    }

    /// Rectangulo envolvente de todos los monitores.
    ///
    /// Ojo: **el escritorio virtual no tiene por que ser rectangular**. Con
    /// monitores de distinto tamano quedan huecos en forma de L dentro de
    /// este envolvente donde no hay pantalla. Para saber si un punto esta en
    /// pantalla de verdad hay que usar [`Self::monitor_en`], no esto.
    pub fn escritorio_virtual(&self) -> Rect {
        let mut iter = self.monitores.iter();
        let Some(primero) = iter.next() else {
            return Rect {
                x: 0,
                y: 0,
                ancho: 0,
                alto: 0,
            };
        };
        iter.fold(primero.area, |acumulado, m| acumulado.union(m.area))
    }

    /// El monitor que contiene el punto, si alguno lo contiene.
    ///
    /// Devuelve `None` en los huecos del escritorio virtual.
    pub fn monitor_en(&self, p: Punto) -> Option<&Monitor> {
        self.monitores.iter().find(|m| m.area.contiene(p))
    }

    /// Recorta un rectangulo al envolvente del escritorio.
    ///
    /// `None` si no queda nada dentro.
    pub fn recortar_al_escritorio(&self, r: Rect) -> Option<Rect> {
        let virtual_ = self.escritorio_virtual();
        if virtual_.esta_vacio() {
            return None;
        }
        r.interseccion(virtual_)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Disposicion realista y desagradable a proposito: portatil 4K al 150%
    /// como principal, un monitor 1080p al 100% a su izquierda (coordenadas
    /// negativas), y otro encima (coordenada Y negativa).
    fn disposicion_mixta() -> DisposicionMonitores {
        DisposicionMonitores::nueva(vec![
            Monitor {
                id: 1,
                area: Rect {
                    x: 0,
                    y: 0,
                    ancho: 3840,
                    alto: 2160,
                },
                area_trabajo: Rect {
                    x: 0,
                    y: 0,
                    ancho: 3840,
                    alto: 2100,
                },
                escala_por_cien: 150,
                principal: true,
            },
            Monitor {
                id: 2,
                area: Rect {
                    x: -1920,
                    y: 0,
                    ancho: 1920,
                    alto: 1080,
                },
                area_trabajo: Rect {
                    x: -1920,
                    y: 0,
                    ancho: 1920,
                    alto: 1040,
                },
                escala_por_cien: 100,
                principal: false,
            },
            Monitor {
                id: 3,
                area: Rect {
                    x: 0,
                    y: -1080,
                    ancho: 1920,
                    alto: 1080,
                },
                area_trabajo: Rect {
                    x: 0,
                    y: -1080,
                    ancho: 1920,
                    alto: 1080,
                },
                escala_por_cien: 125,
                principal: false,
            },
        ])
    }

    #[test]
    fn el_escritorio_virtual_abarca_todos_los_monitores() {
        let d = disposicion_mixta();
        assert_eq!(
            d.escritorio_virtual(),
            Rect {
                x: -1920,
                y: -1080,
                ancho: 5760,
                alto: 3240
            }
        );
    }

    #[test]
    fn el_escritorio_virtual_de_una_disposicion_vacia_es_vacio() {
        let d = DisposicionMonitores::nueva(vec![]);
        assert!(d.escritorio_virtual().esta_vacio());
        assert!(d.principal().is_none());
        assert!(d.monitor_en(Punto { x: 0, y: 0 }).is_none());
    }

    #[test]
    fn monitor_en_encuentra_el_correcto_en_coordenadas_negativas() {
        let d = disposicion_mixta();
        assert_eq!(d.monitor_en(Punto { x: -100, y: 500 }).unwrap().id, 2);
        assert_eq!(d.monitor_en(Punto { x: 100, y: -500 }).unwrap().id, 3);
        assert_eq!(d.monitor_en(Punto { x: 100, y: 500 }).unwrap().id, 1);
    }

    #[test]
    fn monitor_en_no_se_solapa_en_la_frontera() {
        // Caso negativo del que depende todo: con media apertura, x=0 cae en
        // el monitor 1 y x=-1 en el 2, sin ambiguedad. Una implementacion con
        // <= en el borde derecho devolveria el 2 para x=0.
        let d = disposicion_mixta();
        assert_eq!(d.monitor_en(Punto { x: 0, y: 10 }).unwrap().id, 1);
        assert_eq!(d.monitor_en(Punto { x: -1, y: 10 }).unwrap().id, 2);
    }

    #[test]
    fn monitor_en_devuelve_none_fuera_de_todo() {
        // El escritorio virtual no es rectangular: hay huecos en forma de L.
        // Este punto esta dentro del rectangulo envolvente pero en ningun
        // monitor. Una implementacion que solo comprobase el envolvente
        // devolveria Some y la captura saldria negra.
        let d = disposicion_mixta();
        assert!(d.monitor_en(Punto { x: -1000, y: -1000 }).is_none());
    }

    #[test]
    fn la_escala_se_expresa_como_factor() {
        let d = disposicion_mixta();
        assert_eq!(d.monitores()[0].escala(), 1.5);
        assert_eq!(d.monitores()[1].escala(), 1.0);
        assert_eq!(d.monitores()[2].escala(), 1.25);
    }

    #[test]
    fn recortar_al_escritorio_deja_fuera_lo_que_sobresale() {
        let d = disposicion_mixta();
        let r = Rect {
            x: -3000,
            y: 0,
            ancho: 2000,
            alto: 100,
        };
        let recortado = d.recortar_al_escritorio(r).unwrap();
        // El rectangulo va de x=-3000 a x=-1000; el escritorio empieza en
        // x=-1920. Queda [-1920, -1000): 920 de ancho. (El plan original
        // esperaba 1080, que es el trozo DESCARTADO, no el que queda.)
        assert_eq!(
            recortado,
            Rect {
                x: -1920,
                y: 0,
                ancho: 920,
                alto: 100
            }
        );
    }

    #[test]
    fn recortar_al_escritorio_devuelve_none_si_queda_fuera_del_todo() {
        let d = disposicion_mixta();
        let r = Rect {
            x: 100_000,
            y: 100_000,
            ancho: 10,
            alto: 10,
        };
        assert!(d.recortar_al_escritorio(r).is_none());
    }
}
