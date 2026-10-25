//! **Los planos abiertos a la vez**: una mesa por pestana (cada una con su
//! dibujo, su deshacer, su camara, sus cubos en la tarjeta y su
//! autoguardado), la que se ve y la tira de pestanas de arriba.
//!
//! [`Documentos`] es la lista con la activa (sin ventanas: se prueba sola);
//! se puede usar como la mesa activa (`Deref`), asi el codigo de siempre
//! (`m.sesion…`) sigue valiendo con varias. Nunca esta vacia: cerrar la
//! ultima es volver al inicio (lo decide la ventana).

use std::ops::{Deref, DerefMut};
use std::path::Path;

#[cfg(windows)]
pub use con_ventana::*;

/// **La lista de abiertos** y cual se ve.
#[derive(Debug, Clone)]
pub struct Documentos<T> {
    lista: Vec<T>,
    activa: usize,
}

impl<T> Documentos<T> {
    /// Con uno solo (el activo).
    pub fn con(t: T) -> Documentos<T> {
        Documentos {
            lista: vec![t],
            activa: 0,
        }
    }

    /// Anade uno al final y lo deja activo.
    pub fn anadir(&mut self, t: T) {
        self.lista.push(t);
        self.activa = self.lista.len() - 1;
    }

    /// El numero del activo.
    pub fn indice_activo(&self) -> usize {
        self.activa
    }

    pub fn cuantos(&self) -> usize {
        self.lista.len()
    }

    /// Activa el `i` (si esta). Dice si cambio.
    pub fn activar(&mut self, i: usize) -> bool {
        if i >= self.lista.len() || i == self.activa {
            return false;
        }
        self.activa = i;
        true
    }

    /// Ctrl+Tab: el siguiente (despues del ultimo, el primero).
    pub fn siguiente(&mut self) -> bool {
        let n = self.lista.len();
        self.activar((self.activa + 1) % n)
    }

    /// Ctrl+Mayus+Tab: el anterior.
    pub fn anterior(&mut self) -> bool {
        let n = self.lista.len();
        self.activar((self.activa + n - 1) % n)
    }

    /// **Quita el `i`** y lo da; queda activo el de su derecha (o el ultimo),
    /// y si se quita otro, el activo sigue siendo el mismo. El ultimo que
    /// queda no se quita (`None`): la lista nunca esta vacia.
    pub fn quitar(&mut self, i: usize) -> Option<T> {
        if i >= self.lista.len() || self.lista.len() == 1 {
            return None;
        }
        let t = self.lista.remove(i);
        if i < self.activa || self.activa >= self.lista.len() {
            self.activa -= 1;
        }
        Some(t)
    }

    /// El primero que cumple `f`.
    pub fn buscar(&self, f: impl Fn(&T) -> bool) -> Option<usize> {
        self.lista.iter().position(f)
    }

    pub fn todos(&self) -> impl Iterator<Item = &T> {
        self.lista.iter()
    }

    pub fn todos_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.lista.iter_mut()
    }
}

impl<T> Deref for Documentos<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.lista[self.activa]
    }
}

impl<T> DerefMut for Documentos<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.lista[self.activa]
    }
}

/// **Si dos rutas son el mismo fichero** (con sus enlaces resueltos si
/// existen; en Windows sin mirar mayusculas).
pub fn es_la_misma(a: &Path, b: &Path) -> bool {
    let c = |p: &Path| {
        let p = std::fs::canonicalize(p)
            .or_else(|_| std::path::absolute(p))
            .unwrap_or_else(|_| p.to_path_buf());
        let s = p.to_string_lossy().into_owned();
        if cfg!(windows) { s.to_lowercase() } else { s }
    };
    c(a) == c(b)
}

/// El ancho de cada pestana: la de su nombre entre un minimo y un maximo,
/// y si no caben todas, todas iguales en lo que hay.
pub fn anchos(naturales: &[f32], disponible: f32, minimo: f32, maximo: f32) -> Vec<f32> {
    let v: Vec<f32> = naturales.iter().map(|a| a.clamp(minimo, maximo)).collect();
    let total: f32 = v.iter().sum();
    if total <= disponible || v.is_empty() {
        return v;
    }
    let cada = (disponible / v.len() as f32).max(0.0);
    vec![cada; v.len()]
}

/// Lo que necesita la ventana (la mesa y Direct2D).
#[cfg(windows)]
mod con_ventana {
    use std::ops::{Deref, DerefMut};

    use cad_nucleo::xref::Vigia;
    use pixpin_render::{Pintor, RectF};

    use super::{Documentos, anchos};
    use crate::mesa::Mesa;
    use crate::tema;
    use crate::ui::{self, Accion, Zona, rect};

    /// **Un plano abierto**: su mesa y lo que vigila de sus referencias.
    pub struct Documento {
        pub mesa: Box<Mesa>,
        vigia: Vigia,
    }

    impl Documento {
        pub fn nuevo(mesa: Mesa) -> Documento {
            Documento {
                mesa: Box::new(mesa),
                vigia: Vigia::default(),
            }
        }

        /// El nombre de la pestana.
        pub fn nombre(&self) -> String {
            self.mesa
                .ruta()
                .and_then(|r| r.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Sin nombre".into())
        }

        /// **Mira si cambio en el disco alguna referencia externa** y lo dice en
        /// su historial. Leer las fechas cuesta: la ventana lo llama cada pocos
        /// segundos, no en cada fotograma. Dice si dijo algo.
        pub fn revisar_referencias(&mut self) -> bool {
            let d = &self.mesa.sesion.dibujo;
            let cambiadas = self.vigia.revisar(&d.doc, d.ruta.as_deref());
            for n in &cambiadas {
                self.mesa.sesion.avisar(format!(
                    "La referencia «{n}» cambio en el disco: escriba RECARGARREF para verla al dia"
                ));
            }
            !cambiadas.is_empty()
        }
    }

    impl Deref for Documento {
        type Target = Mesa;

        fn deref(&self) -> &Mesa {
            &self.mesa
        }
    }

    impl DerefMut for Documento {
        fn deref_mut(&mut self) -> &mut Mesa {
            &mut self.mesa
        }
    }

    /// Lo que pinta una pestana: el nombre y si tiene cambios sin guardar.
    pub fn etiquetas(d: &Documentos<Documento>) -> Vec<(String, bool)> {
        d.todos()
            .map(|x| (x.nombre(), x.sesion.dibujo.sucio()))
            .collect()
    }

    /// **La tira de pestanas** en `r`: una por plano (el nombre, • si tiene
    /// cambios y × para cerrarlo) y + para uno nuevo. `activa`: la que se ve
    /// (`None` en el inicio). Deja sus zonas.
    pub fn pintar(
        p: &Pintor,
        r: RectF,
        e: f32,
        pestanas: &[(String, bool)],
        activa: Option<usize>,
        encima: Option<(f32, f32)>,
        zonas: &mut Vec<Zona>,
    ) {
        p.rellenar(r, tema::FONDO);
        p.rellenar(rect(r.x, r.y + r.alto - 1.0, r.ancho, 1.0), tema::BORDE);
        let tam = tema::LETRA * e;
        let cerrar = 18.0 * e;
        let relleno = 12.0 * e;
        let mas = r.alto;
        let naturales: Vec<f32> = pestanas
            .iter()
            .map(|(n, s)| {
                let t = if *s { format!("{n} •") } else { n.clone() };
                p.medir_texto(&t, tam).0 + relleno * 2.0 + cerrar
            })
            .collect();
        let anchos = anchos(&naturales, r.ancho - mas - 8.0 * e, 90.0 * e, 240.0 * e);
        let sobre = |z: &RectF| encima.is_some_and(|(x, y)| ui::dentro(z, x, y));
        let mut x = r.x + 4.0 * e;
        let alto = r.alto - 4.0 * e;
        for (i, ((nombre, sucio), ancho)) in pestanas.iter().zip(anchos).enumerate() {
            let t = rect(x, r.y + 4.0 * e, ancho - 2.0 * e, alto);
            let es = activa == Some(i);
            if es {
                p.rellenar_redondeado(t, 6.0 * e, tema::PANEL);
                p.rellenar(
                    rect(t.x + 6.0 * e, t.y, t.ancho - 12.0 * e, 2.0 * e),
                    tema::ACENTO,
                );
            } else if sobre(&t) {
                p.rellenar_redondeado(t, 6.0 * e, tema::PANEL_ALTO);
            }
            let texto = if *sucio {
                format!("{nombre} •")
            } else {
                nombre.clone()
            };
            let zona_texto = rect(
                t.x + relleno,
                t.y,
                (t.ancho - relleno - cerrar - 4.0 * e).max(0.0),
                t.alto,
            );
            p.con_recorte(zona_texto, |p| {
                p.texto(
                    &texto,
                    zona_texto.x,
                    t.y + (t.alto - tam) / 2.0 - 2.0 * e,
                    tam,
                    if es { tema::TEXTO } else { tema::TEXTO_SUAVE },
                );
            });
            let ayuda = if *sucio {
                format!("{nombre} (con cambios sin guardar)  Ctrl+Tab cambia")
            } else {
                format!("{nombre}  Ctrl+Tab cambia")
            };
            zonas.push(Zona {
                r: t,
                accion: Accion::Documento(i),
                ayuda,
            });
            // La × de cerrar.
            let c = rect(
                t.x + t.ancho - cerrar - 4.0 * e,
                t.y + (t.alto - cerrar) / 2.0,
                cerrar,
                cerrar,
            );
            if sobre(&c) {
                p.rellenar_redondeado(c, 4.0 * e, tema::BORDE);
            }
            let (cx, cy, l) = (c.x + c.ancho / 2.0, c.y + c.alto / 2.0, 4.0 * e);
            let color = if es || sobre(&t) {
                tema::TEXTO_SUAVE
            } else {
                tema::TEXTO_APAGADO
            };
            p.linea((cx - l, cy - l), (cx + l, cy + l), 1.3 * e, color);
            p.linea((cx - l, cy + l), (cx + l, cy - l), 1.3 * e, color);
            zonas.push(Zona {
                r: c,
                accion: Accion::CerrarDocumento(i),
                ayuda: "Cerrar este plano  Ctrl+W".into(),
            });
            x += ancho;
        }
        // El + de un plano nuevo.
        let n = rect(x + 2.0 * e, r.y + 4.0 * e, mas - 8.0 * e, alto);
        if sobre(&n) {
            p.rellenar_redondeado(n, 6.0 * e, tema::PANEL_ALTO);
        }
        let (cx, cy, l) = (n.x + n.ancho / 2.0, n.y + n.alto / 2.0, 6.0 * e);
        p.linea((cx - l, cy), (cx + l, cy), 1.5 * e, tema::TEXTO_SUAVE);
        p.linea((cx, cy - l), (cx, cy + l), 1.5 * e, tema::TEXTO_SUAVE);
        zonas.push(Zona {
            r: n,
            accion: Accion::PlanoNuevo,
            ayuda: "Plano nuevo  Ctrl+N".into(),
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn anadir_activar_y_dar_la_vuelta() {
        let mut d = Documentos::con("a");
        d.anadir("b");
        d.anadir("c");
        assert_eq!((*d, d.indice_activo(), d.cuantos()), ("c", 2, 3));
        assert!(d.siguiente());
        assert_eq!(*d, "a");
        assert!(d.anterior());
        assert_eq!(*d, "c");
        assert!(d.activar(1));
        assert_eq!(*d, "b");
        // Casos negativos: el mismo o uno que no esta.
        assert!(!d.activar(1));
        assert!(!d.activar(7));
        assert_eq!(d.buscar(|x| *x == "c"), Some(2));
        assert_eq!(d.buscar(|x| *x == "z"), None);
    }

    #[test]
    fn quitar_deja_activo_el_de_al_lado_y_nunca_vacia() {
        let mut d = Documentos::con(0);
        for i in 1..4 {
            d.anadir(i);
        }
        // Quitar el activo (el ultimo): queda el de su izquierda.
        assert_eq!(d.quitar(3), Some(3));
        assert_eq!(*d, 2);
        // Quitar el activo del medio: queda el de su derecha.
        d.activar(1);
        assert_eq!(d.quitar(1), Some(1));
        assert_eq!(*d, 2);
        // Quitar otro de la izquierda: el activo es el mismo.
        assert_eq!(d.quitar(0), Some(0));
        assert_eq!((*d, d.indice_activo()), (2, 0));
        // El ultimo no se quita.
        assert_eq!(d.quitar(0), None);
        assert_eq!(d.quitar(5), None);
        assert_eq!(d.cuantos(), 1);
        // Con uno solo, Ctrl+Tab no cambia nada.
        assert!(!d.siguiente() && !d.anterior());
    }

    #[test]
    fn los_anchos_se_reparten_si_no_caben() {
        assert_eq!(anchos(&[50.0, 300.0], 1000.0, 90.0, 240.0), [90.0, 240.0]);
        assert_eq!(anchos(&[200.0; 4], 400.0, 90.0, 240.0), [100.0; 4]);
        assert!(anchos(&[], 10.0, 1.0, 2.0).is_empty());
        assert_eq!(anchos(&[100.0], -5.0, 90.0, 240.0), [0.0]);
    }

    #[test]
    fn la_misma_ruta_escrita_distinto() {
        let dir = std::env::temp_dir().join(format!("cadpixpin-docs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let f = dir.join("plano.dwg");
        std::fs::write(&f, b"x").unwrap();
        assert!(es_la_misma(
            &f,
            &dir.join("sub").join("..").join("plano.dwg")
        ));
        assert!(!es_la_misma(&f, &dir.join("otro.dwg")));
    }
}
