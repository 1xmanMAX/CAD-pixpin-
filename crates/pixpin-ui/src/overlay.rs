//! La maquina de estados del overlay de seleccion.
//!
//! Aqui vive TODO lo que el usuario hace con el raton y el teclado. Las
//! ventanas reales (pixpin_shell::overlay) solo traducen mensajes Win32 a
//! `EventoEntrada` y dibujan lo que este estado diga, de modo que la
//! interaccion completa se prueba sin abrir una ventana.
//!
//! Los candidatos de snap llegan por evento (`EventoEntrada::Candidatos`)
//! porque el hilo de UIA contesta cuando quiere: la regla de la spec es que
//! el overlay NUNCA espera a nadie.

use pixpin_geom::{
    Candidato, DisposicionMonitores, Punto, Rect, Seleccion, Tirador, resolver_ajuste,
};

/// Distancia maxima, en pixeles fisicos, para que pulsar+soltar cuente como
/// clic y no como arrastre.
const UMBRAL_CLIC: i32 = 4;
/// Radio de los tiradores, igual que en `Seleccion::tirador_en`.
const RADIO_TIRADOR: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    Explorando,
    Trazando,
    Redimensionando,
    Moviendo,
    Lista,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeclaOverlay {
    Flecha {
        dx: i32,
        dy: i32,
    },
    Espacio,
    Escape,
    Enter,
    /// `Ctrl+A` o el boton del panel: la pantalla entera bajo el cursor
    /// queda seleccionada y lista para confirmar.
    SeleccionarTodo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventoEntrada {
    RatonMovido(Punto),
    BotonPulsado(Punto),
    BotonSoltado(Punto),
    Tecla(TeclaOverlay),
    Candidatos(Vec<Candidato>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Efecto {
    Nada,
    Redibujar,
    AlternarVivo,
    Confirmar(Rect),
    Cancelar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormaCursor {
    Cruz,
    Mover,
    RedimNS,
    RedimEO,
    /// Noreste-suroeste (la diagonal /).
    RedimNeSo,
    /// Noroeste-sureste (la diagonal \).
    RedimNoSe,
}

#[derive(Debug)]
pub struct EstadoOverlay {
    disposicion: DisposicionMonitores,
    seleccion: Seleccion,
    fase: Fase,
    vivo: bool,
    cursor: Punto,
    ancla_clic: Punto,
    ultimo_cursor: Punto,
    candidatos: Vec<Candidato>,
    /// La proporcion a la que se ata el trazado (v2-captura: 16:9, 4:3,
    /// 1:1). `None` es libre, lo de siempre.
    proporcion: Option<(u32, u32)>,
}

impl EstadoOverlay {
    pub fn nuevo(disposicion: DisposicionMonitores) -> Self {
        Self {
            disposicion,
            seleccion: Seleccion::nueva(),
            fase: Fase::Explorando,
            vivo: false,
            cursor: Punto { x: 0, y: 0 },
            ancla_clic: Punto { x: 0, y: 0 },
            ultimo_cursor: Punto { x: 0, y: 0 },
            candidatos: Vec::new(),
            proporcion: None,
        }
    }

    /// La proporcion elegida, o `None` si es libre.
    pub fn proporcion(&self) -> Option<(u32, u32)> {
        self.proporcion
    }

    /// Ata el trazado a una proporcion (`None` la suelta). Si ya hay una
    /// seleccion lista, se ajusta en el sitio: se conserva la esquina de
    /// arriba a la izquierda y el ancho, y se calcula el alto; si no cabe
    /// en el escritorio, manda el alto y se calcula el ancho.
    pub fn poner_proporcion(&mut self, p: Option<(u32, u32)>) -> Efecto {
        self.proporcion = p.filter(|(a, b)| *a > 0 && *b > 0);
        let Some((pa, pb)) = self.proporcion else {
            return Efecto::Redibujar;
        };
        if self.fase != Fase::Lista {
            return Efecto::Redibujar;
        }
        let r = self.seleccion.rect();
        let v = self.disposicion.escritorio_virtual();
        let mut ancho = r.ancho.max(1);
        let mut alto = (ancho as u64 * pb as u64 / pa as u64).max(1) as u32;
        let cabe_alto = (v.abajo() - r.y).max(1) as u32;
        if alto > cabe_alto {
            alto = cabe_alto;
            ancho = (alto as u64 * pa as u64 / pb as u64).max(1) as u32;
        }
        self.seleccion.establecer(Rect {
            x: r.x,
            y: r.y,
            ancho,
            alto,
        });
        self.seleccion.sujetar_a(&self.disposicion);
        Efecto::Redibujar
    }

    /// Deja `r` como seleccion lista, como si se acabara de trazar: es
    /// «Repetir la ultima zona» y las medidas tecleadas. Un rectangulo vacio
    /// o fuera del escritorio no cambia nada.
    pub fn poner_seleccion(&mut self, r: Rect) -> Efecto {
        if r.esta_vacio() {
            return Efecto::Nada;
        }
        let v = self.disposicion.escritorio_virtual();
        if r.interseccion(v).is_none() {
            return Efecto::Nada;
        }
        self.seleccion.terminar_arrastre();
        self.seleccion.establecer(r);
        self.seleccion.sujetar_a(&self.disposicion);
        if self.seleccion.rect().esta_vacio() {
            return Efecto::Nada;
        }
        self.fase = Fase::Lista;
        Efecto::Redibujar
    }

    /// El rectangulo desde `ancla` hacia `p` con la proporcion puesta: manda
    /// el lado que mas se arrastro. `None` si la proporcion es libre.
    fn atado(&self, ancla: Punto, p: Punto) -> Option<Rect> {
        let (pa, pb) = self.proporcion?;
        let dx = p.x - ancla.x;
        let dy = p.y - ancla.y;
        let (ax, ay) = (dx.unsigned_abs(), dy.unsigned_abs());
        let (ancho, alto) = if ax as u64 * pb as u64 >= ay as u64 * pa as u64 {
            (ax, (ax as u64 * pb as u64 / pa as u64) as u32)
        } else {
            ((ay as u64 * pa as u64 / pb as u64) as u32, ay)
        };
        let x = if dx < 0 {
            ancla.x - ancho as i32
        } else {
            ancla.x
        };
        let y = if dy < 0 {
            ancla.y - alto as i32
        } else {
            ancla.y
        };
        Some(Rect { x, y, ancho, alto })
    }

    pub fn fase(&self) -> Fase {
        self.fase
    }

    pub fn seleccion(&self) -> Rect {
        self.seleccion.rect()
    }

    pub fn cursor(&self) -> Punto {
        self.cursor
    }

    pub fn vivo(&self) -> bool {
        self.vivo
    }

    /// El rectangulo del snap bajo el cursor, solo mientras se explora.
    pub fn rect_resaltado(&self) -> Option<Rect> {
        if self.fase != Fase::Explorando {
            return None;
        }
        resolver_ajuste(&self.candidatos, self.cursor)
    }

    pub fn forma_cursor(&self) -> FormaCursor {
        if self.fase == Fase::Lista {
            if let Some(t) = self.seleccion.tirador_en(self.cursor, RADIO_TIRADOR) {
                return match t {
                    Tirador::NorteBorde | Tirador::SurBorde => FormaCursor::RedimNS,
                    Tirador::EsteBorde | Tirador::OesteBorde => FormaCursor::RedimEO,
                    Tirador::NoresteEsquina | Tirador::SuroesteEsquina => FormaCursor::RedimNeSo,
                    Tirador::NoroesteEsquina | Tirador::SuresteEsquina => FormaCursor::RedimNoSe,
                };
            }
            if self.seleccion.rect().contiene(self.cursor) {
                return FormaCursor::Mover;
            }
        }
        FormaCursor::Cruz
    }

    pub fn procesar(&mut self, evento: EventoEntrada) -> Efecto {
        match evento {
            EventoEntrada::Candidatos(c) => {
                // Nunca interrumpen un gesto: solo alimentan el resaltado.
                self.candidatos = c;
                if self.fase == Fase::Explorando {
                    Efecto::Redibujar
                } else {
                    Efecto::Nada
                }
            }
            EventoEntrada::RatonMovido(p) => self.raton_movido(p),
            EventoEntrada::BotonPulsado(p) => self.boton_pulsado(p),
            EventoEntrada::BotonSoltado(p) => self.boton_soltado(p),
            EventoEntrada::Tecla(t) => self.tecla(t),
        }
    }

    fn raton_movido(&mut self, p: Punto) -> Efecto {
        self.cursor = p;
        match self.fase {
            Fase::Trazando => {
                self.seleccion.arrastrar_a(p);
                if let Some(r) = self.atado(self.ancla_clic, p) {
                    self.seleccion.establecer(r);
                }
                Efecto::Redibujar
            }
            Fase::Redimensionando => {
                self.seleccion.arrastrar_a(p);
                Efecto::Redibujar
            }
            Fase::Moviendo => {
                self.seleccion
                    .desplazar(p.x - self.ultimo_cursor.x, p.y - self.ultimo_cursor.y);
                self.deslizar_dentro();
                self.ultimo_cursor = p;
                Efecto::Redibujar
            }
            _ => Efecto::Redibujar,
        }
    }

    /// Sujeta un DESPLAZAMIENTO: desliza la seleccion de vuelta al interior
    /// del escritorio SIN cambiarle el tamano.
    ///
    /// No confundir con `sujetar_a`, que RECORTA (correcto al terminar un
    /// trazado que sobresale, destructivo aqui): empujar con las flechas
    /// contra el borde iba encogiendo la seleccion hasta vaciarla — el caso
    /// negativo del test lo cazo.
    fn deslizar_dentro(&mut self) {
        let v = self.disposicion.escritorio_virtual();
        let r = self.seleccion.rect();
        if v.esta_vacio() || r.esta_vacio() {
            return;
        }
        let x = r.x.clamp(
            v.izquierda(),
            (v.derecha() - r.ancho as i32).max(v.izquierda()),
        );
        let y =
            r.y.clamp(v.arriba(), (v.abajo() - r.alto as i32).max(v.arriba()));
        self.seleccion.establecer(Rect {
            x,
            y,
            ancho: r.ancho,
            alto: r.alto,
        });
    }

    fn boton_pulsado(&mut self, p: Punto) -> Efecto {
        self.cursor = p;
        self.ancla_clic = p;
        self.ultimo_cursor = p;
        match self.fase {
            Fase::Lista => {
                if let Some(t) = self.seleccion.tirador_en(p, RADIO_TIRADOR) {
                    self.seleccion.iniciar_redimension(t);
                    self.fase = Fase::Redimensionando;
                } else if self.seleccion.rect().contiene(p) {
                    self.fase = Fase::Moviendo;
                } else {
                    self.seleccion.iniciar_arrastre(p);
                    self.fase = Fase::Trazando;
                }
            }
            _ => {
                self.seleccion.iniciar_arrastre(p);
                self.fase = Fase::Trazando;
            }
        }
        Efecto::Redibujar
    }

    fn boton_soltado(&mut self, p: Punto) -> Efecto {
        self.cursor = p;
        let fue_clic = (p.x - self.ancla_clic.x).abs() <= UMBRAL_CLIC
            && (p.y - self.ancla_clic.y).abs() <= UMBRAL_CLIC;
        match self.fase {
            Fase::Trazando if fue_clic => {
                self.seleccion.terminar_arrastre();
                // El clic adopta el candidato resaltado; sin el no hay nada.
                match resolver_ajuste(&self.candidatos, p) {
                    Some(r) => {
                        self.seleccion.establecer(r);
                        self.seleccion.sujetar_a(&self.disposicion);
                        self.fase = Fase::Lista;
                    }
                    None => self.fase = Fase::Explorando,
                }
            }
            Fase::Trazando | Fase::Redimensionando | Fase::Moviendo => {
                self.seleccion.terminar_arrastre();
                self.seleccion.sujetar_a(&self.disposicion);
                self.fase = Fase::Lista;
            }
            _ => {}
        }
        Efecto::Redibujar
    }

    fn tecla(&mut self, t: TeclaOverlay) -> Efecto {
        match t {
            TeclaOverlay::Escape => Efecto::Cancelar,
            TeclaOverlay::Espacio => {
                self.vivo = !self.vivo;
                Efecto::AlternarVivo
            }
            TeclaOverlay::Enter => match self.fase {
                Fase::Lista => Efecto::Confirmar(self.seleccion.rect()),
                Fase::Explorando => match self.rect_resaltado() {
                    Some(r) => Efecto::Confirmar(r),
                    None => Efecto::Nada,
                },
                _ => Efecto::Nada,
            },
            TeclaOverlay::Flecha { dx, dy } => {
                if self.fase == Fase::Lista {
                    self.seleccion.desplazar(dx, dy);
                    self.deslizar_dentro();
                    Efecto::Redibujar
                } else {
                    Efecto::Nada
                }
            }
            TeclaOverlay::SeleccionarTodo => {
                // La pantalla bajo el cursor, no el escritorio virtual: una
                // captura que abarque dos monitores de distinta altura
                // dejaria franjas negras. Quien quiera los dos, que trace.
                let area = self
                    .disposicion
                    .monitor_en(self.cursor)
                    .or_else(|| self.disposicion.principal())
                    .map(|m| m.area);
                match area {
                    Some(a) => {
                        self.seleccion.establecer(a);
                        self.fase = Fase::Lista;
                        Efecto::Redibujar
                    }
                    None => Efecto::Nada,
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::{Candidato, DisposicionMonitores, Monitor, Punto, Rect};

    fn escritorio_1080p() -> DisposicionMonitores {
        DisposicionMonitores::nueva(vec![Monitor {
            id: 1,
            area: Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080,
            },
            area_trabajo: Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1040,
            },
            escala_por_cien: 100,
            principal: true,
        }])
    }

    fn estado() -> EstadoOverlay {
        EstadoOverlay::nuevo(escritorio_1080p())
    }

    fn candidato(x: i32, y: i32, ancho: u32, alto: u32) -> Candidato {
        Candidato {
            rect: Rect { x, y, ancho, alto },
            profundidad: 1,
        }
    }

    #[test]
    fn arrastrar_y_soltar_deja_la_seleccion_lista() {
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 100, y: 100 }));
        assert_eq!(e.fase(), Fase::Trazando);
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 300, y: 250 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 300, y: 250 }));
        assert_eq!(e.fase(), Fase::Lista);
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 100,
                y: 100,
                ancho: 200,
                alto: 150
            }
        );
    }

    #[test]
    fn un_clic_sin_arrastre_adopta_el_candidato_resaltado() {
        let mut e = estado();
        e.procesar(EventoEntrada::Candidatos(vec![candidato(50, 50, 400, 300)]));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 200, y: 200 }));
        assert_eq!(
            e.rect_resaltado(),
            Some(Rect {
                x: 50,
                y: 50,
                ancho: 400,
                alto: 300
            })
        );
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 200, y: 200 }));
        // Se movio 2 px: sigue contando como clic, no como arrastre.
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 202, y: 201 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 202, y: 201 }));
        assert_eq!(e.fase(), Fase::Lista);
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 50,
                y: 50,
                ancho: 400,
                alto: 300
            }
        );
    }

    #[test]
    fn un_clic_sin_candidato_no_deja_seleccion() {
        // Caso negativo del anterior: sin candidatos, el clic suelto no puede
        // inventarse un rectangulo.
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 200, y: 200 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 201, y: 200 }));
        assert_eq!(e.fase(), Fase::Explorando);
    }

    #[test]
    fn en_lista_el_tirador_redimensiona_y_el_interior_mueve() {
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 100, y: 100 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 300, y: 200 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 300, y: 200 }));

        // Esquina sureste: redimensiona.
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 300, y: 200 }));
        assert_eq!(e.fase(), Fase::Redimensionando);
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 400, y: 300 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 400, y: 300 }));
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 100,
                y: 100,
                ancho: 300,
                alto: 200
            }
        );

        // Interior: mueve sin cambiar el tamano.
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 200, y: 200 }));
        assert_eq!(e.fase(), Fase::Moviendo);
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 250, y: 220 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 250, y: 220 }));
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 150,
                y: 120,
                ancho: 300,
                alto: 200
            }
        );
    }

    #[test]
    fn pulsar_fuera_de_la_seleccion_empieza_una_nueva() {
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 100, y: 100 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 200, y: 200 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 200, y: 200 }));
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 800, y: 800 }));
        assert_eq!(e.fase(), Fase::Trazando);
    }

    #[test]
    fn las_flechas_desplazan_y_la_sujecion_impide_salir() {
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 10, y: 10 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 110, y: 110 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 110, y: 110 }));

        e.procesar(EventoEntrada::Tecla(TeclaOverlay::Flecha { dx: -1, dy: 0 }));
        assert_eq!(e.seleccion().x, 9);
        // Caso negativo de la sujecion: veinte pasos de -10 no pueden dejar
        // la seleccion en x negativa; se recorta contra el escritorio.
        for _ in 0..20 {
            e.procesar(EventoEntrada::Tecla(TeclaOverlay::Flecha {
                dx: -10,
                dy: 0,
            }));
        }
        assert_eq!(e.seleccion().x, 0);
        assert!(
            !e.seleccion().esta_vacio(),
            "la sujecion no debe vaciar la seleccion"
        );
    }

    #[test]
    fn enter_confirma_escape_cancela_espacio_alterna() {
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 100, y: 100 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 300, y: 300 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 300, y: 300 }));

        assert!(!e.vivo());
        assert_eq!(
            e.procesar(EventoEntrada::Tecla(TeclaOverlay::Espacio)),
            Efecto::AlternarVivo
        );
        assert!(e.vivo());

        assert_eq!(
            e.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter)),
            Efecto::Confirmar(Rect {
                x: 100,
                y: 100,
                ancho: 200,
                alto: 200
            })
        );

        // Escape cancela desde cualquier fase, incluida Trazando a medias.
        let mut e2 = estado();
        e2.procesar(EventoEntrada::BotonPulsado(Punto { x: 5, y: 5 }));
        assert_eq!(
            e2.procesar(EventoEntrada::Tecla(TeclaOverlay::Escape)),
            Efecto::Cancelar
        );
    }

    #[test]
    fn enter_en_explorando_confirma_el_candidato_resaltado() {
        let mut e = estado();
        e.procesar(EventoEntrada::Candidatos(vec![candidato(50, 50, 400, 300)]));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 100, y: 100 }));
        assert_eq!(
            e.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter)),
            Efecto::Confirmar(Rect {
                x: 50,
                y: 50,
                ancho: 400,
                alto: 300
            })
        );
        // Caso negativo: sin candidato bajo el cursor, Enter no confirma nada.
        let mut e2 = estado();
        assert_eq!(
            e2.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter)),
            Efecto::Nada
        );
    }

    #[test]
    fn la_forma_del_cursor_refleja_lo_que_haria_el_clic() {
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 100, y: 100 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 300, y: 200 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 300, y: 200 }));

        e.procesar(EventoEntrada::RatonMovido(Punto { x: 200, y: 150 }));
        assert_eq!(e.forma_cursor(), FormaCursor::Mover);
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 300, y: 200 }));
        assert_eq!(e.forma_cursor(), FormaCursor::RedimNoSe); // esquina SE
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 300, y: 150 }));
        assert_eq!(e.forma_cursor(), FormaCursor::RedimEO); // borde este
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 900, y: 900 }));
        assert_eq!(e.forma_cursor(), FormaCursor::Cruz);
    }

    #[test]
    fn los_candidatos_nuevos_no_interrumpen_un_gesto_en_curso() {
        // El hilo UIA contesta cuando quiere. Si su respuesta llegara en mitad
        // de un arrastre y cambiara la seleccion, el rectangulo saltaria bajo
        // la mano del usuario.
        let mut e = estado();
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 100, y: 100 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 200, y: 200 }));
        e.procesar(EventoEntrada::Candidatos(vec![candidato(0, 0, 1900, 1000)]));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 200, y: 200 }));
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 100,
                y: 100,
                ancho: 100,
                alto: 100
            }
        );
    }

    #[test]
    fn seleccionar_todo_toma_la_pantalla_bajo_el_cursor_y_queda_lista() {
        let mut e = EstadoOverlay::nuevo(escritorio_1080p());
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 500, y: 400 }));
        assert_eq!(
            e.procesar(EventoEntrada::Tecla(TeclaOverlay::SeleccionarTodo)),
            Efecto::Redibujar
        );
        assert_eq!(e.fase(), Fase::Lista);
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 0,
                y: 0,
                ancho: 1920,
                alto: 1080
            }
        );
        // Y Enter confirma esa misma pantalla.
        assert!(matches!(
            e.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter)),
            Efecto::Confirmar(r) if r.ancho == 1920 && r.alto == 1080
        ));
    }

    #[test]
    fn con_proporcion_el_trazado_sale_atado_y_libre_no() {
        let mut e = estado();
        e.poner_proporcion(Some((16, 9)));
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 100, y: 100 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 260, y: 140 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 260, y: 140 }));
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 100,
                y: 100,
                ancho: 160,
                alto: 90
            }
        );
        // Hacia arriba a la izquierda tambien, con el ancla quieta.
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 500, y: 500 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 480, y: 300 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 480, y: 300 }));
        let r = e.seleccion();
        assert_eq!((r.abajo(), r.derecha()), (500, 500));
        assert_eq!(r.ancho, 355);
        assert_eq!(r.alto, 200);
        // Caso negativo: libre, el trazado no se toca.
        e.poner_proporcion(None);
        e.procesar(EventoEntrada::BotonPulsado(Punto { x: 10, y: 10 }));
        e.procesar(EventoEntrada::RatonMovido(Punto { x: 110, y: 20 }));
        e.procesar(EventoEntrada::BotonSoltado(Punto { x: 110, y: 20 }));
        assert_eq!((e.seleccion().ancho, e.seleccion().alto), (100, 10));
    }

    #[test]
    fn poner_la_proporcion_con_seleccion_lista_la_ajusta_sin_salirse() {
        let mut e = estado();
        e.poner_seleccion(Rect {
            x: 100,
            y: 900,
            ancho: 400,
            alto: 100,
        });
        e.poner_proporcion(Some((1, 1)));
        // 400 de alto no cabe desde y=900 en 1080: manda el alto que cabe.
        assert_eq!(
            e.seleccion(),
            Rect {
                x: 100,
                y: 900,
                ancho: 180,
                alto: 180
            }
        );
        // Caso negativo: una proporcion con un cero no se acepta.
        e.poner_proporcion(Some((0, 9)));
        assert_eq!(e.proporcion(), None);
    }

    #[test]
    fn poner_seleccion_la_deja_lista_y_rechaza_lo_vacio_o_fuera() {
        let mut e = estado();
        assert_eq!(
            e.poner_seleccion(Rect {
                x: 10,
                y: 10,
                ancho: 300,
                alto: 200
            }),
            Efecto::Redibujar
        );
        assert_eq!(e.fase(), Fase::Lista);
        assert!(matches!(
            e.procesar(EventoEntrada::Tecla(TeclaOverlay::Enter)),
            Efecto::Confirmar(r) if r.ancho == 300
        ));
        // Casos negativos: vacio y fuera del escritorio.
        let mut f = estado();
        assert_eq!(
            f.poner_seleccion(Rect {
                x: 10,
                y: 10,
                ancho: 0,
                alto: 5
            }),
            Efecto::Nada
        );
        assert_eq!(
            f.poner_seleccion(Rect {
                x: 5000,
                y: 5000,
                ancho: 10,
                alto: 10
            }),
            Efecto::Nada
        );
        assert_eq!(f.fase(), Fase::Explorando);
    }
}
