//! **Un `.pptx` diapositiva a diapositiva.** Cada diapositiva se arma
//! cuando se pide (y se guarda): su fondo, las formas del patron y del
//! diseno que se ven, y las suyas, con los marcadores heredando posicion,
//! formato y estilo de texto del diseno y del patron.

pub mod pase;
pub mod texto;

use std::collections::HashMap;
use std::sync::Arc;

use pixpin_docs::xml::Nodo;

use crate::dml::{self, Contexto, Xfrm};
use crate::medidor::Medidor;
use crate::pagina::{Color, Elemento, Pagina, Rect, Recursos, Relleno, Trazo};
use crate::paquete::{ErrorOffice, Paquete, Relaciones, attr_num, emu};
use crate::tema::{self, Tema};
use texto::{Cuerpo, Entorno, Niveles};

/// Un diseno o un patron leido.
struct Plantilla {
    raiz: Nodo,
    rels: Relaciones,
    /// El patron de un diseno.
    padre: Option<String>,
    mapa: HashMap<String, String>,
    tema: Arc<Tema>,
}

impl Plantilla {
    fn arbol(&self) -> Option<&Nodo> {
        self.raiz.hijo("cSld").and_then(|c| c.hijo("spTree"))
    }

    fn mostrar_patron(&self) -> bool {
        !matches!(self.raiz.atributo("showMasterSp"), "0" | "false")
    }
}

pub struct Presentacion {
    paquete: Paquete,
    pub ancho: f32,
    pub alto: f32,
    diapositivas: Vec<String>,
    plantillas: HashMap<String, Arc<Plantilla>>,
    temas: HashMap<String, Arc<Tema>>,
    defecto: Option<Nodo>,
    estilos_tabla: Option<Nodo>,
    recursos: Arc<Recursos>,
    hechas: Vec<Option<Arc<Pagina>>>,
}

/// Que hereda un marcador: sus antecesores en el diseno y el patron.
#[derive(Default)]
struct Herencia<'a> {
    /// Del mas lejano (patron) al mas cercano (diseno).
    cadena: Vec<&'a Nodo>,
    tipo: String,
}

impl Presentacion {
    pub fn de_paquete(mut paquete: Paquete) -> Result<Presentacion, ErrorOffice> {
        let principal = paquete
            .principal()
            .unwrap_or_else(|| "ppt/presentation.xml".into());
        let Some(raiz) = paquete.xml(&principal) else {
            return Err(match paquete.ultimo_limite {
                Some(e) => ErrorOffice::Limite(e.to_string()),
                None => ErrorOffice::SinParte("ppt/presentation.xml"),
            });
        };
        let rels = paquete.relaciones(&principal);
        let (mut ancho, mut alto) = (720.0, 540.0);
        if let Some(s) = raiz.hijo("sldSz") {
            ancho = emu(attr_num(s, "cx", 1.0).unwrap_or(9144000.0));
            alto = emu(attr_num(s, "cy", 1.0).unwrap_or(6858000.0));
        }
        let diapositivas: Vec<String> = raiz
            .hijo("sldIdLst")
            .map(|l| {
                l.elementos()
                    .filter_map(|s| {
                        rels.get(s.atributo_con_prefijo("r", "id"))
                            .filter(|r| !r.externa)
                            .map(|r| r.destino.clone())
                    })
                    .filter(|r| paquete.existe(r))
                    .collect()
            })
            .unwrap_or_default();
        let estilos_tabla = rels
            .values()
            .find(|r| r.tipo == "tableStyles")
            .map(|r| r.destino.clone())
            .and_then(|r| paquete.xml(&r));
        let n = diapositivas.len();
        Ok(Presentacion {
            paquete,
            ancho: ancho.max(1.0),
            alto: alto.max(1.0),
            diapositivas,
            plantillas: HashMap::new(),
            temas: HashMap::new(),
            defecto: raiz.hijo("defaultTextStyle").cloned(),
            estilos_tabla,
            recursos: Recursos::nuevo(),
            hechas: vec![None; n],
        })
    }

    pub fn cuantas(&self) -> usize {
        self.diapositivas.len()
    }

    pub fn recursos(&self) -> Arc<Recursos> {
        self.recursos.clone()
    }

    fn tema(&mut self, ruta: &str) -> Arc<Tema> {
        if let Some(t) = self.temas.get(ruta) {
            return t.clone();
        }
        let t = Arc::new(
            self.paquete
                .xml(ruta)
                .map(|n| Tema::leer(&n))
                .unwrap_or_else(|| Tema::leer(&Nodo::default())),
        );
        self.temas.insert(ruta.to_string(), t.clone());
        t
    }

    fn plantilla(&mut self, ruta: &str) -> Option<Arc<Plantilla>> {
        if let Some(p) = self.plantillas.get(ruta) {
            return Some(p.clone());
        }
        let raiz = self.paquete.xml(ruta)?;
        let rels = self.paquete.relaciones(ruta);
        let padre = rels
            .values()
            .find(|r| r.tipo == "slideMaster")
            .map(|r| r.destino.clone());
        let (mapa, tema) = if raiz.nombre == "sldMaster" {
            let mapa = raiz
                .hijo("clrMap")
                .map(|m| {
                    m.atributos
                        .iter()
                        .map(|(k, v)| (local(k), v.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let tema = match rels
                .values()
                .find(|r| r.tipo == "theme")
                .map(|r| r.destino.clone())
            {
                Some(t) => self.tema(&t),
                None => Arc::new(Tema::leer(&Nodo::default())),
            };
            (mapa, tema)
        } else {
            let patron = padre.as_deref().and_then(|p| self.plantilla(p));
            let mut mapa = patron.as_ref().map(|p| p.mapa.clone()).unwrap_or_default();
            if let Some(o) = raiz
                .hijo("clrMapOvr")
                .and_then(|o| o.hijo("overrideClrMapping"))
            {
                mapa = o
                    .atributos
                    .iter()
                    .map(|(k, v)| (local(k), v.clone()))
                    .collect();
            }
            (
                mapa,
                patron
                    .map(|p| p.tema.clone())
                    .unwrap_or_else(|| Arc::new(Tema::leer(&Nodo::default()))),
            )
        };
        let p = Arc::new(Plantilla {
            raiz,
            rels,
            padre,
            mapa,
            tema,
        });
        self.plantillas.insert(ruta.to_string(), p.clone());
        Some(p)
    }

    /// La diapositiva `n` (desde 0), armada la primera vez que se pide.
    pub fn diapositiva(&mut self, n: usize, medidor: &dyn Medidor) -> Option<Arc<Pagina>> {
        if let Some(Some(p)) = self.hechas.get(n) {
            return Some(p.clone());
        }
        let ruta = self.diapositivas.get(n)?.clone();
        let p = Arc::new(self.armar(&ruta, n as u32 + 1, medidor));
        self.hechas[n] = Some(p.clone());
        Some(p)
    }

    fn armar(&mut self, ruta: &str, numero: u32, medidor: &dyn Medidor) -> Pagina {
        let mut pagina = Pagina {
            ancho: self.ancho,
            alto: self.alto,
            numero,
            ..Default::default()
        };
        let Some(raiz) = self.paquete.xml(ruta) else {
            pagina.fondo = Some(Relleno::Solido(Color::BLANCO));
            return pagina;
        };
        let rels = self.paquete.relaciones(ruta);
        let diseno = rels
            .values()
            .find(|r| r.tipo == "slideLayout")
            .map(|r| r.destino.clone())
            .and_then(|r| self.plantilla(&r));
        let patron = diseno
            .as_ref()
            .and_then(|d| d.padre.clone())
            .and_then(|r| self.plantilla(&r));
        let tema = patron
            .as_ref()
            .map(|p| p.tema.clone())
            .unwrap_or_else(|| Arc::new(Tema::leer(&Nodo::default())));
        let mut mapa = diseno.as_ref().map(|d| d.mapa.clone()).unwrap_or_default();
        if let Some(o) = raiz
            .hijo("clrMapOvr")
            .and_then(|o| o.hijo("overrideClrMapping"))
        {
            mapa = o
                .atributos
                .iter()
                .map(|(k, v)| (local(k), v.clone()))
                .collect();
        }
        let ctx = Contexto {
            tema: &tema,
            mapa: &mapa,
        };

        // El fondo: el de la diapositiva, el del diseno o el del patron.
        let fondos = [
            Some(&raiz),
            diseno.as_ref().map(|d| &d.raiz),
            patron.as_ref().map(|p| &p.raiz),
        ];
        pagina.fondo = fondos
            .iter()
            .flatten()
            .find_map(|r| {
                r.hijo("cSld")
                    .and_then(|c| c.hijo("bg"))
                    .and_then(|bg| fondo(bg, ctx))
            })
            .or(Some(Relleno::Solido(Color::BLANCO)));
        // Una imagen de fondo.
        for (r, rel) in [
            (Some(&raiz), Some(&rels)),
            (
                diseno.as_ref().map(|d| &d.raiz),
                diseno.as_ref().map(|d| &d.rels),
            ),
        ] {
            let (Some(r), Some(rel)) = (r, rel) else {
                continue;
            };
            if let Some(bf) = r
                .hijo("cSld")
                .and_then(|c| c.hijo("bg"))
                .and_then(|b| b.hijo("bgPr"))
                .and_then(|b| b.hijo("blipFill"))
            {
                if let Some(id) = self.imagen(rel, bf) {
                    pagina.elementos.push(Elemento::Imagen {
                        id,
                        rect: Rect::nuevo(0.0, 0.0, self.ancho, self.alto),
                        recorte: dml::recorte(bf),
                        opacidad: 1.0,
                    });
                }
                break;
            }
        }

        let defecto = self.defecto.clone();
        let mut base_niveles = texto::niveles_vacios();
        if let Some(d) = &defecto {
            texto::mezclar_lista(&mut base_niveles, d, ctx);
        }
        let estilos_patron = patron
            .as_ref()
            .and_then(|p| p.raiz.hijo("txStyles"))
            .cloned();
        let niveles_de = |tipo: &str| -> Niveles {
            // El estilo de texto de la presentacion solo vale para lo que no
            // es marcador (los marcadores parten de los estilos del patron).
            let mut n = if tipo.is_empty() {
                base_niveles.clone()
            } else {
                texto::niveles_vacios()
            };
            if let Some(tx) = &estilos_patron {
                let cual = match tipo {
                    "title" | "ctrTitle" => "titleStyle",
                    "" | "dt" | "ftr" | "sldNum" | "hdr" => "otherStyle",
                    _ => "bodyStyle",
                };
                // Las formas sin marcador usan el estilo «otro».
                if let Some(l) = tx.hijo(cual) {
                    texto::mezclar_lista(&mut n, l, ctx);
                }
            }
            n
        };

        let mostrar = diseno.as_ref().is_none_or(|d| d.mostrar_patron())
            && !matches!(raiz.atributo("showMasterSp"), "0" | "false");
        let mut lienzo = Lienzo {
            numero,
            medidor,
            elementos: Vec::new(),
        };
        if mostrar && let Some(p) = &patron {
            let p = p.clone();
            if let Some(arbol) = p.arbol() {
                self.arbol(&mut lienzo, arbol, &p.rels, ctx, &[], &niveles_de, false);
            }
        }
        if let Some(d) = &diseno {
            let d = d.clone();
            if let Some(arbol) = d.arbol() {
                let previos: Vec<&Plantilla> = patron.iter().map(|p| p.as_ref()).collect();
                self.arbol(
                    &mut lienzo,
                    arbol,
                    &d.rels,
                    ctx,
                    &previos,
                    &niveles_de,
                    false,
                );
            }
        }
        if let Some(arbol) = raiz.hijo("cSld").and_then(|c| c.hijo("spTree")) {
            let mut previos: Vec<&Plantilla> = Vec::new();
            if let Some(p) = &patron {
                previos.push(p);
            }
            if let Some(d) = &diseno {
                previos.push(d);
            }
            self.arbol(&mut lienzo, arbol, &rels, ctx, &previos, &niveles_de, true);
        }
        pagina.elementos.append(&mut lienzo.elementos);
        pagina
    }

    fn imagen(&mut self, rels: &Relaciones, blip_fill: &Nodo) -> Option<Arc<str>> {
        let rid = blip_fill.hijo("blip")?.atributo_con_prefijo("r", "embed");
        let rel = rels.get(rid)?;
        if rel.externa {
            return None;
        }
        let id: Arc<str> = Arc::from(rel.destino.as_str());
        if !self.recursos.tiene(&id) {
            let b = self.paquete.bytes(&rel.destino)?;
            self.recursos.poner(id.clone(), Arc::from(b));
        }
        Some(id)
    }

    /// Pinta un arbol de formas. `previos` son las plantillas de donde
    /// heredan los marcadores (patron y diseno). Los marcadores de las
    /// plantillas no se pintan: solo se heredan.
    #[allow(clippy::too_many_arguments)]
    fn arbol(
        &mut self,
        l: &mut Lienzo,
        arbol: &Nodo,
        rels: &Relaciones,
        ctx: Contexto,
        previos: &[&Plantilla],
        niveles_de: &dyn Fn(&str) -> Niveles,
        es_diapositiva: bool,
    ) {
        for n in arbol.elementos() {
            let mut el = Vec::new();
            self.forma(
                &mut el,
                l,
                n,
                rels,
                ctx,
                previos,
                niveles_de,
                es_diapositiva,
                None,
            );
            l.elementos.append(&mut el);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn forma(
        &mut self,
        salida: &mut Vec<Elemento>,
        l: &Lienzo,
        n: &Nodo,
        rels: &Relaciones,
        ctx: Contexto,
        previos: &[&Plantilla],
        niveles_de: &dyn Fn(&str) -> Niveles,
        es_diapositiva: bool,
        caja_grupo: Option<Rect>,
    ) {
        let nv = n.elementos().find(|e| e.nombre.starts_with("nv"));
        if nv
            .and_then(|v| v.hijo("cNvPr"))
            .is_some_and(|c| matches!(c.atributo("hidden"), "1" | "true"))
        {
            return;
        }
        let ph = nv.and_then(|v| v.hijo("nvPr")).and_then(|p| p.hijo("ph"));
        if ph.is_some() && !es_diapositiva {
            // Un marcador del patron o del diseno no se ve en la
            // diapositiva... salvo que sea de los que no se rellenan (el
            // numero, la fecha y el pie los pone la diapositiva).
            return;
        }
        let herencia = ph.map(|p| buscar_herencia(p, previos)).unwrap_or_default();
        match n.nombre.as_str() {
            "sp" | "cxnSp" => self.sp(salida, l, n, ctx, &herencia, niveles_de, caja_grupo),
            "pic" => {
                let sppr = n.hijo("spPr");
                let x = xfrm_heredado(sppr, &herencia, caja_grupo);
                let Some(x) = x else { return };
                let Some(bf) = n.hijo("blipFill") else { return };
                let Some(id) = self.imagen(rels, bf) else {
                    return;
                };
                let caja = Rect::nuevo(x.x, x.y, x.ancho, x.alto);
                let mut els = vec![Elemento::Imagen {
                    id,
                    rect: Rect::nuevo(0.0, 0.0, x.ancho, x.alto),
                    recorte: dml::recorte(bf),
                    opacidad: 1.0,
                }];
                if pase::medio_de(n).is_some() {
                    // Un video o un audio: su poster con el boton de reproducir.
                    els.extend(pase::icono_reproducir(x.ancho, x.alto));
                }
                if let Some(t) = sppr
                    .and_then(|s| s.hijo("ln"))
                    .and_then(|ln| dml::linea(ln, ctx, None))
                    .flatten()
                {
                    els.push(Elemento::Rect {
                        rect: Rect::nuevo(0.0, 0.0, x.ancho, x.alto),
                        relleno: None,
                        borde: Some(t),
                    });
                }
                let efectos = dml::efectos_forma(sppr, n.hijo("style"), &[], ctx, x.giro);
                salida.push(
                    dml::colocar(els, caja, x.giro, x.voltear_h, x.voltear_v).con_efectos(efectos),
                );
            }
            "grpSp" => {
                let gx = n
                    .hijo("grpSpPr")
                    .and_then(|g| g.hijo("xfrm"))
                    .map(dml::xfrm)
                    .unwrap_or_default();
                let gx = ajustar_a_grupo(gx, caja_grupo);
                let sx = if gx.hijo_ancho > 0.0 {
                    gx.ancho / gx.hijo_ancho
                } else {
                    1.0
                };
                let sy = if gx.hijo_alto > 0.0 {
                    gx.alto / gx.hijo_alto
                } else {
                    1.0
                };
                // Los hijos se pintan en coordenadas del grupo ya
                // escaladas, con origen en la esquina del grupo.
                let mut hijos = Vec::new();
                for h in n.elementos() {
                    let marco = Rect {
                        x: gx.hijo_x,
                        y: gx.hijo_y,
                        ancho: sx,
                        alto: sy,
                    };
                    self.forma(
                        &mut hijos,
                        l,
                        h,
                        rels,
                        ctx,
                        &[],
                        niveles_de,
                        es_diapositiva,
                        Some(marco),
                    );
                }
                let efectos = dml::efectos_forma(n.hijo("grpSpPr"), None, &[], ctx, gx.giro);
                let grupo = dml::colocar(
                    hijos,
                    Rect::nuevo(gx.x, gx.y, gx.ancho, gx.alto),
                    gx.giro,
                    gx.voltear_h,
                    gx.voltear_v,
                );
                salida.push(grupo.con_efectos(efectos));
            }
            "graphicFrame" => {
                let x = n
                    .hijo("xfrm")
                    .map(dml::xfrm)
                    .or_else(|| herencia_xfrm(&herencia));
                let Some(x) = x.map(|x| ajustar_a_grupo(x, caja_grupo)) else {
                    return;
                };
                let Some(datos) = n.hijo("graphic").and_then(|g| g.hijo("graphicData")) else {
                    return;
                };
                if let Some(t) = datos.hijo("tbl") {
                    let els = self.tabla(t, x.ancho, ctx, l);
                    let efectos = dml::efectos_forma(t.hijo("tblPr"), None, &[], ctx, 0.0);
                    salida.push(
                        dml::colocar(
                            els,
                            Rect::nuevo(x.x, x.y, x.ancho, x.alto),
                            0.0,
                            false,
                            false,
                        )
                        .con_efectos(efectos),
                    );
                } else if let Some(els) = crate::grafico::de_datos(
                    &mut self.paquete,
                    rels,
                    datos,
                    ctx,
                    l.medidor,
                    x.ancho,
                    x.alto,
                ) {
                    // Un grafico: dibujado con los datos de su cache.
                    salida.push(dml::colocar(
                        els,
                        Rect::nuevo(x.x, x.y, x.ancho, x.alto),
                        0.0,
                        false,
                        false,
                    ));
                } else if datos.atributo("uri").ends_with("/diagram") {
                    // SmartArt: el dibujo que PowerPoint guarda ya hecho.
                    let dm = datos
                        .hijo("relIds")
                        .map(|r| r.atributo_con_prefijo("r", "dm").to_string())
                        .unwrap_or_default();
                    if let Some(els) = self.smartart(rels, &dm, ctx, l, niveles_de) {
                        salida.push(dml::colocar(
                            els,
                            Rect::nuevo(x.x, x.y, x.ancho, x.alto),
                            0.0,
                            false,
                            false,
                        ));
                    }
                } else if let Some(bf) = datos
                    .descendientes("pic")
                    .into_iter()
                    .find_map(|p| p.hijo("blipFill"))
                {
                    // Un objeto incrustado (OLE): su imagen de muestra.
                    if let Some(id) = self.imagen(rels, bf) {
                        salida.push(Elemento::Imagen {
                            id,
                            rect: Rect::nuevo(x.x, x.y, x.ancho, x.alto),
                            recorte: dml::recorte(bf),
                            opacidad: 1.0,
                        });
                    }
                }
            }
            "AlternateContent" => {
                let rama = n.hijo("Fallback").or_else(|| n.hijo("Choice"));
                if let Some(r) = rama {
                    for h in r.elementos() {
                        self.forma(
                            salida,
                            l,
                            h,
                            rels,
                            ctx,
                            previos,
                            niveles_de,
                            es_diapositiva,
                            caja_grupo,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sp(
        &mut self,
        salida: &mut Vec<Elemento>,
        l: &Lienzo,
        n: &Nodo,
        ctx: Contexto,
        herencia: &Herencia,
        niveles_de: &dyn Fn(&str) -> Niveles,
        caja_grupo: Option<Rect>,
    ) {
        let sppr = n.hijo("spPr");
        let Some(x) = xfrm_heredado(sppr, herencia, caja_grupo) else {
            return;
        };
        let estilo = n
            .hijo("style")
            .map(|s| dml::estilo_forma(s, ctx))
            .unwrap_or_default();
        // Relleno y linea: los de la forma, los heredados del marcador, o
        // los del estilo.
        let mut sppr_cadena: Vec<&Nodo> = herencia
            .cadena
            .iter()
            .filter_map(|h| h.hijo("spPr"))
            .collect();
        if let Some(s) = sppr {
            sppr_cadena.push(s);
        }
        // Primero lo de la propia forma (su `spPr` y despues su estilo), y
        // solo si no dice nada, lo heredado del marcador.
        let tiene_estilo = n.hijo("style").is_some();
        let heredados: Vec<&Nodo> = herencia
            .cadena
            .iter()
            .filter_map(|h| h.hijo("spPr"))
            .collect();
        let relleno = match sppr.and_then(|s| dml::relleno(s, ctx, None)) {
            Some(r) => r,
            None if tiene_estilo => estilo.relleno.clone(),
            None => heredados
                .iter()
                .rev()
                .find_map(|s| dml::relleno(s, ctx, None))
                .flatten(),
        };
        let ln_color = estilo.linea.map(|t| t.color);
        let ln_propia = sppr.and_then(|s| s.hijo("ln"));
        let borde = match ln_propia.and_then(|ln| dml::linea(ln, ctx, ln_color)) {
            Some(t) => t,
            None if tiene_estilo => {
                let g = ln_propia.and_then(|ln| attr_num(ln, "w", 1.0)).map(emu);
                estilo.linea.map(|t| Trazo {
                    grosor: g.unwrap_or(t.grosor),
                    ..t
                })
            }
            None => heredados
                .iter()
                .rev()
                .filter_map(|s| s.hijo("ln"))
                .find_map(|ln| dml::linea(ln, ctx, None))
                .flatten(),
        };
        let geom = sppr_cadena
            .iter()
            .rev()
            .find(|s| s.hijo("prstGeom").is_some() || s.hijo("custGeom").is_some())
            .copied();
        let (figuras, es_linea) = match geom {
            Some(g) => dml::figuras(g, x.ancho, x.alto),
            None => dml::figuras(&Nodo::default(), x.ancho, x.alto),
        };
        let mut els = Vec::new();
        if !figuras.is_empty() && (relleno.is_some() || borde.is_some()) {
            els.push(Elemento::Trayecto {
                figuras,
                relleno: if es_linea { None } else { relleno },
                borde,
            });
        }
        if let Some(tx) = n.hijo("txBody") {
            let mut niveles = niveles_de(&herencia.tipo);
            let mut cuerpo = Cuerpo::default();
            for h in &herencia.cadena {
                if let Some(t) = h.hijo("txBody") {
                    if let Some(lst) = t.hijo("lstStyle") {
                        texto::mezclar_lista(&mut niveles, lst, ctx);
                    }
                    if let Some(b) = t.hijo("bodyPr") {
                        cuerpo.mezclar(&Cuerpo::leer(b));
                    }
                }
            }
            // El color de letra del estilo de la forma manda sobre el de los
            // estilos del patron (no sobre el de la propia forma).
            if let Some(c) = estilo.color_letra {
                for nv in niveles.iter_mut() {
                    nv.def.color = Some(c);
                }
            }
            let color = estilo
                .color_letra
                .or_else(|| ctx.tema.esquema("tx1", ctx.mapa))
                .unwrap_or(Color::NEGRO);
            let e = Entorno {
                ctx,
                medidor: l.medidor,
                color,
                numero: l.numero,
            };
            // El texto vertical (`vert`): se maqueta en la caja girada.
            let vertical = tx.hijo("bodyPr").map(|b| b.atributo("vert")).unwrap_or("");
            if matches!(vertical, "vert" | "vert270" | "eaVert") {
                let t = texto::maquetar(tx, x.alto, x.ancho, &niveles, &cuerpo, &e);
                let giro = if vertical == "vert270" { 270.0 } else { 90.0 };
                let (cx, cy) = (x.ancho / 2.0, x.alto / 2.0);
                let interior = Rect::nuevo(cx - x.alto / 2.0, cy - x.ancho / 2.0, x.alto, x.ancho);
                els.push(dml::colocar(t, interior, giro, false, false));
            } else {
                els.extend(texto::maquetar(tx, x.ancho, x.alto, &niveles, &cuerpo, &e));
            }
        }
        if els.is_empty() {
            return;
        }
        let efectos = dml::efectos_forma(sppr, n.hijo("style"), &heredados, ctx, x.giro);
        salida.push(
            dml::colocar(
                els,
                Rect::nuevo(x.x, x.y, x.ancho, x.alto),
                x.giro,
                x.voltear_h,
                x.voltear_v,
            )
            .con_efectos(efectos),
        );
    }

    fn smartart(
        &mut self,
        rels: &Relaciones,
        dm: &str,
        ctx: Contexto,
        l: &Lienzo,
        niveles_de: &dyn Fn(&str) -> Niveles,
    ) -> Option<Vec<Elemento>> {
        // El dibujo se encuentra por el `dataModelExt` de los datos, o el
        // primero que haya.
        let datos = rels.get(dm).map(|r| r.destino.clone());
        let mut ruta = None;
        if let Some(d) = datos
            && let Some(x) = self.paquete.xml(&d)
        {
            let rid = x
                .descendientes("dataModelExt")
                .first()
                .map(|e| e.atributo("relId").to_string())
                .unwrap_or_default();
            ruta = rels.get(&rid).map(|r| r.destino.clone());
        }
        let ruta = ruta.or_else(|| {
            rels.values()
                .find(|r| r.tipo == "diagramDrawing")
                .map(|r| r.destino.clone())
        })?;
        let raiz = self.paquete.xml(&ruta)?;
        let rels_d = self.paquete.relaciones(&ruta);
        let arbol = raiz.hijo("spTree")?;
        let mut v = Vec::new();
        for n in arbol.elementos() {
            self.forma(&mut v, l, n, &rels_d, ctx, &[], niveles_de, true, None);
        }
        Some(v)
    }

    /// Una tabla de DrawingML en (0, 0).
    fn tabla(&mut self, t: &Nodo, ancho_marco: f32, ctx: Contexto, l: &Lienzo) -> Vec<Elemento> {
        let rejilla: Vec<f32> = t
            .hijo("tblGrid")
            .map(|g| {
                g.elementos()
                    .filter(|c| c.nombre == "gridCol")
                    .map(|c| emu(attr_num(c, "w", 1.0).unwrap_or(0.0)))
                    .collect()
            })
            .unwrap_or_default();
        let _ = ancho_marco;
        let mut xs = vec![0.0f32];
        for w in &rejilla {
            xs.push(xs.last().copied().unwrap_or(0.0) + w);
        }
        let tblpr = t.hijo("tblPr");
        let marca = |k: &str| tblpr.is_some_and(|p| matches!(p.atributo(k), "1" | "true"));
        let (primera_fila, ultima_fila, primera_col, ultima_col, bandas_f, bandas_c) = (
            marca("firstRow"),
            marca("lastRow"),
            marca("firstCol"),
            marca("lastCol"),
            marca("bandRow"),
            marca("bandCol"),
        );
        let id_estilo = tblpr
            .and_then(|p| p.hijo("tableStyleId"))
            .map(|s| s.texto().trim().to_string())
            .unwrap_or_default();
        let estilo = self.estilo_tabla(&id_estilo, ctx);
        let filas: Vec<&Nodo> = t.elementos().filter(|e| e.nombre == "tr").collect();
        let n_filas = filas.len();
        let n_cols = rejilla.len();
        // Primero los altos: el de la fila o lo que pida el texto.
        struct C<'n> {
            nodo: &'n Nodo,
            fila: usize,
            col: usize,
            tramo_c: usize,
            tramo_f: usize,
            partes: Vec<&'static str>,
        }
        let mut celdas: Vec<C> = Vec::new();
        let mut altos: Vec<f32> = Vec::new();
        for (fi, f) in filas.iter().enumerate() {
            altos.push(emu(attr_num(f, "h", 1.0).unwrap_or(0.0)));
            for (ci, c) in f.elementos().filter(|e| e.nombre == "tc").enumerate() {
                if matches!(c.atributo("hMerge"), "1" | "true")
                    || matches!(c.atributo("vMerge"), "1" | "true")
                {
                    continue;
                }
                let mut partes: Vec<&'static str> = vec!["wholeTbl"];
                if bandas_c && !(primera_col && ci == 0) {
                    let k = if primera_col { ci.wrapping_sub(1) } else { ci };
                    partes.push(if k % 2 == 0 { "band1V" } else { "band2V" });
                }
                if bandas_f && !(primera_fila && fi == 0) {
                    let k = if primera_fila { fi.wrapping_sub(1) } else { fi };
                    partes.push(if k % 2 == 0 { "band1H" } else { "band2H" });
                }
                if primera_col && ci == 0 {
                    partes.push("firstCol");
                }
                if ultima_col && ci + 1 == n_cols {
                    partes.push("lastCol");
                }
                if ultima_fila && fi + 1 == n_filas {
                    partes.push("lastRow");
                }
                if primera_fila && fi == 0 {
                    partes.push("firstRow");
                }
                celdas.push(C {
                    nodo: c,
                    fila: fi,
                    col: ci,
                    tramo_c: c.atributo("gridSpan").parse().unwrap_or(1usize).max(1),
                    tramo_f: c.atributo("rowSpan").parse().unwrap_or(1usize).max(1),
                    partes,
                });
            }
        }
        // Maquetar el texto de cada celda (para saber cuanto mide).
        let mut maquetadas = Vec::new();
        for c in &celdas {
            let col = c.col.min(n_cols);
            let fin = (c.col + c.tramo_c).min(n_cols);
            let ancho = (xs[fin] - xs[col]).max(1.0);
            let tcpr = c.nodo.hijo("tcPr");
            let m = |k: &str, d: f32| tcpr.and_then(|p| attr_num(p, k, 1.0)).map(emu).unwrap_or(d);
            let (mi, md, ma, mb) = (
                m("marL", 7.2),
                m("marR", 7.2),
                m("marT", 3.6),
                m("marB", 3.6),
            );
            let mut negrita = None;
            let mut color_texto = None;
            for p in &c.partes {
                if let Some(e) = estilo.get(*p) {
                    if e.negrita.is_some() {
                        negrita = e.negrita;
                    }
                    if e.color_texto.is_some() {
                        color_texto = e.color_texto;
                    }
                }
            }
            let mut niveles = texto::niveles_vacios();
            if let Some(d) = &self.defecto {
                texto::mezclar_lista(&mut niveles, d, ctx);
            }
            // El estilo de la tabla manda sobre el estilo de texto por defecto.
            for nv in &mut niveles {
                if negrita.is_some() {
                    nv.def.negrita = negrita;
                }
                if color_texto.is_some() {
                    nv.def.color = color_texto;
                }
            }
            let cuerpo = Cuerpo {
                izq: Some(mi),
                der: Some(md),
                arriba: Some(ma),
                abajo: Some(mb),
                ..Default::default()
            };
            let color = color_texto
                .or_else(|| ctx.tema.esquema("tx1", ctx.mapa))
                .unwrap_or(Color::NEGRO);
            let e = Entorno {
                ctx,
                medidor: l.medidor,
                color,
                numero: l.numero,
            };
            let (els, alto) = match c.nodo.hijo("txBody") {
                Some(tx) => {
                    let els = texto::maquetar(tx, ancho, 0.0, &niveles, &cuerpo, &e);
                    let alto = alto_de(&els) + mb;
                    (els, alto)
                }
                None => (Vec::new(), ma + mb),
            };
            maquetadas.push((els, alto, ancho));
        }
        for (c, (_, alto, _)) in celdas.iter().zip(&maquetadas) {
            if c.tramo_f == 1 && c.fila < altos.len() {
                altos[c.fila] = altos[c.fila].max(*alto);
            }
        }
        let mut ys = vec![0.0f32];
        for h in &altos {
            ys.push(ys.last().copied().unwrap_or(0.0) + h);
        }
        let mut fondos = Vec::new();
        let mut textos = Vec::new();
        let mut lineas = Vec::new();
        for (c, (els, alto_texto, ancho)) in celdas.iter().zip(maquetadas) {
            let y0 = ys[c.fila.min(n_filas)];
            let y1 = ys[(c.fila + c.tramo_f).min(n_filas)];
            let x0 = xs[c.col.min(n_cols)];
            let caja = Rect::nuevo(x0, y0, ancho, y1 - y0);
            let tcpr = c.nodo.hijo("tcPr");
            // Relleno: el de la celda o el del estilo.
            let propio = tcpr.and_then(|p| dml::relleno(p, ctx, None));
            let relleno = match propio {
                Some(r) => r,
                None => c
                    .partes
                    .iter()
                    .rev()
                    .find_map(|p| estilo.get(*p).and_then(|e| e.relleno.clone())),
            };
            if let Some(r) = relleno {
                fondos.push(Elemento::Rect {
                    rect: caja,
                    relleno: Some(r),
                    borde: None,
                });
            }
            // Anclaje vertical.
            let libre = (caja.alto - alto_texto).max(0.0);
            let dy = match tcpr.map(|p| p.atributo("anchor")).unwrap_or("t") {
                "ctr" => libre / 2.0,
                "b" => libre,
                _ => 0.0,
            };
            for mut e in els {
                e.trasladar(x0, y0 + dy);
                textos.push(e);
            }
            // Bordes: los de la celda, o los del estilo.
            let lados = [
                ("lnT", "top", (x0, y0), (x0 + ancho, y0)),
                ("lnB", "bottom", (x0, y1), (x0 + ancho, y1)),
                ("lnL", "left", (x0, y0), (x0, y1)),
                ("lnR", "right", (x0 + ancho, y0), (x0 + ancho, y1)),
            ];
            for (k, lado, a, b) in lados {
                let propio = tcpr
                    .and_then(|p| p.hijo(k))
                    .and_then(|ln| dml::linea(ln, ctx, None));
                let trazo = match propio {
                    Some(t) => t,
                    None => {
                        // Del estilo: el borde exterior o el interior segun
                        // donde caiga.
                        let exterior = match lado {
                            "top" => c.fila == 0,
                            "bottom" => c.fila + c.tramo_f >= n_filas,
                            "left" => c.col == 0,
                            _ => c.col + c.tramo_c >= n_cols,
                        };
                        let clave = if exterior {
                            lado
                        } else if lado == "top" || lado == "bottom" {
                            "insideH"
                        } else {
                            "insideV"
                        };
                        c.partes.iter().rev().find_map(|p| {
                            estilo.get(*p).and_then(|e| {
                                e.bordes
                                    .get(clave)
                                    .copied()
                                    .or_else(|| e.bordes.get(lado).copied())
                            })
                        })
                    }
                };
                if let Some(t) = trazo {
                    lineas.push(Elemento::Linea {
                        desde: a,
                        hasta: b,
                        trazo: t,
                    });
                }
            }
        }
        fondos.extend(textos);
        fondos.extend(lineas);
        fondos
    }

    /// Las partes de un estilo de tabla (`ppt/tableStyles.xml`), o el de
    /// por defecto de PowerPoint si no esta escrito.
    fn estilo_tabla(&self, id: &str, ctx: Contexto) -> HashMap<&'static str, ParteTabla> {
        let mut m = HashMap::new();
        let nodo = self
            .estilos_tabla
            .as_ref()
            .and_then(|e| e.elementos().find(|s| s.atributo("styleId") == id));
        let partes = [
            "wholeTbl", "band1H", "band2H", "band1V", "band2V", "firstRow", "lastRow", "firstCol",
            "lastCol",
        ];
        let Some(nodo) = nodo else {
            // Los estilos de PowerPoint que no van escritos en el fichero: los
            // mas comunes, aproximados con los colores del tema.
            let blanco = Color::BLANCO;
            let a1 = ctx
                .tema
                .esquema("accent1", ctx.mapa)
                .unwrap_or(Color::rgb(0x44, 0x72, 0xC4));
            let dk1 = ctx.tema.esquema("tx1", ctx.mapa).unwrap_or(Color::NEGRO);
            let tinte = |c: Color, t: f32| {
                let m = |x: u8| (x as f32 * t + 255.0 * (1.0 - t)).round() as u8;
                Color::rgb(m(c.r), m(c.g), m(c.b))
            };
            let todos = |t: Trazo| -> HashMap<&'static str, Trazo> {
                ["left", "right", "top", "bottom", "insideH", "insideV"]
                    .into_iter()
                    .map(|k| (k, t))
                    .collect()
            };
            let resaltada = |c: Color| ParteTabla {
                relleno: Some(Relleno::Solido(c)),
                negrita: Some(true),
                color_texto: Some(blanco),
                ..Default::default()
            };
            match id {
                // Sin estilo, sin cuadricula.
                "{2D5ABB26-0587-4C30-8999-92F81FD0307C}" => {}
                // Sin estilo, cuadricula de tabla.
                "{5940675A-B579-460E-94D1-54222C63F5DA}" => {
                    m.insert(
                        "wholeTbl",
                        ParteTabla {
                            bordes: todos(Trazo::solido(dk1, 1.0)),
                            ..Default::default()
                        },
                    );
                }
                // Estilo medio 2 (con el enfasis 1 o con el texto).
                "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"
                | ""
                | "{073A0DAA-6AF3-43AB-8588-CEC1D06C72B9}" => {
                    let c = if id == "{073A0DAA-6AF3-43AB-8588-CEC1D06C72B9}" {
                        dk1
                    } else {
                        a1
                    };
                    // El texto del cuerpo en `dk1` del tema (sin pasar por el mapa:
                    // en un tema oscuro `tx1` seria blanco).
                    let oscuro = ctx.tema.colores.get("dk1").copied().unwrap_or(Color::NEGRO);
                    m.insert(
                        "wholeTbl",
                        ParteTabla {
                            relleno: Some(Relleno::Solido(tinte(c, 0.2))),
                            bordes: todos(Trazo::solido(blanco, 1.0)),
                            color_texto: Some(oscuro),
                            ..Default::default()
                        },
                    );
                    m.insert(
                        "band1H",
                        ParteTabla {
                            relleno: Some(Relleno::Solido(tinte(c, 0.4))),
                            ..Default::default()
                        },
                    );
                    m.insert(
                        "band1V",
                        ParteTabla {
                            relleno: Some(Relleno::Solido(tinte(c, 0.4))),
                            ..Default::default()
                        },
                    );
                    for k in ["firstRow", "lastRow", "firstCol", "lastCol"] {
                        m.insert(k, resaltada(c));
                    }
                }
                // Cualquier otro: como el «estilo claro 2» del enfasis 1
                // (cabecera llena y marco del color).
                _ => {
                    let mut b = HashMap::new();
                    for k in ["left", "right", "top", "bottom"] {
                        b.insert(k, Trazo::solido(a1, 1.0));
                    }
                    m.insert(
                        "wholeTbl",
                        ParteTabla {
                            bordes: b,
                            ..Default::default()
                        },
                    );
                    m.insert("firstRow", resaltada(a1));
                }
            }
            return m;
        };
        for parte in partes {
            let Some(x) = nodo.hijo(parte) else { continue };
            let mut pt = ParteTabla::default();
            if let Some(tx) = x.hijo("tcTxStyle") {
                pt.negrita = match tx.atributo("b") {
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => None,
                };
                pt.color_texto = tema::color_hijo(tx, ctx.tema, ctx.mapa, None).or_else(|| {
                    tx.hijo("fontRef")
                        .and_then(|f| tema::color_hijo(f, ctx.tema, ctx.mapa, None))
                });
            }
            if let Some(tc) = x.hijo("tcStyle") {
                if let Some(f) = tc.hijo("fill") {
                    pt.relleno = dml::relleno(f, ctx, None).flatten();
                } else if let Some(f) = tc.hijo("fillRef") {
                    pt.relleno = tema::color_hijo(f, ctx.tema, ctx.mapa, None).map(Relleno::Solido);
                }
                if let Some(b) = tc.hijo("tcBdr") {
                    for (k, lado) in [
                        ("left", "left"),
                        ("right", "right"),
                        ("top", "top"),
                        ("bottom", "bottom"),
                        ("insideH", "insideH"),
                        ("insideV", "insideV"),
                    ] {
                        let Some(l) = b.hijo(k) else { continue };
                        let trazo = match l.hijo("ln") {
                            Some(ln) => dml::linea(ln, ctx, None).flatten(),
                            None => l
                                .hijo("lnRef")
                                .and_then(|r| tema::color_hijo(r, ctx.tema, ctx.mapa, None))
                                .map(|c| Trazo::solido(c, 1.0)),
                        };
                        if let Some(t) = trazo {
                            pt.bordes.insert(lado, t);
                        }
                    }
                }
            }
            m.insert(parte, pt);
        }
        m
    }
}

/// Una parte de un estilo de tabla.
#[derive(Debug, Clone, Default)]
struct ParteTabla {
    relleno: Option<Relleno>,
    negrita: Option<bool>,
    color_texto: Option<Color>,
    bordes: HashMap<&'static str, Trazo>,
}

struct Lienzo<'a> {
    numero: u32,
    medidor: &'a dyn Medidor,
    elementos: Vec<Elemento>,
}

fn local(k: &str) -> String {
    k.rsplit(':').next().unwrap_or(k).to_string()
}

/// Lo mas bajo que llega algo de una lista de elementos (para el alto del
/// texto de una celda).
fn alto_de(els: &[Elemento]) -> f32 {
    let mut m: f32 = 0.0;
    for e in els {
        let abajo = match e {
            Elemento::Texto(t) => t.y + t.letra.tam * 0.3,
            Elemento::Rect { rect, .. } | Elemento::Imagen { rect, .. } => rect.abajo(),
            Elemento::Linea { desde, hasta, .. } => desde.1.max(hasta.1),
            Elemento::Efecto { elementos, .. } => alto_de(elementos),
            _ => 0.0,
        };
        m = m.max(abajo);
    }
    m
}

/// El fondo de un `p:bg`.
fn fondo(bg: &Nodo, ctx: Contexto) -> Option<Relleno> {
    if let Some(pr) = bg.hijo("bgPr") {
        return dml::relleno(pr, ctx, None).flatten();
    }
    if let Some(r) = bg.hijo("bgRef") {
        let idx = r.atributo("idx").parse::<usize>().unwrap_or(0);
        let ph = tema::color_hijo(r, ctx.tema, ctx.mapa, None);
        if idx >= 1001
            && let Some(n) = ctx.tema.rellenos_fondo.get(idx - 1001)
            && let Some(rel) = dml::relleno_de(n, ctx, ph)
        {
            return rel;
        }
        if (1..1000).contains(&idx)
            && let Some(n) = ctx.tema.rellenos.get(idx - 1)
            && let Some(rel) = dml::relleno_de(n, ctx, ph)
        {
            return rel;
        }
        return ph.map(Relleno::Solido);
    }
    None
}

/// Busca de quien hereda un marcador en el diseno y el patron.
fn buscar_herencia<'a>(ph: &Nodo, previos: &[&'a Plantilla]) -> Herencia<'a> {
    let tipo = ph.atributo("type");
    let tipo = if tipo.is_empty() { "body" } else { tipo };
    let idx = ph.atributo("idx");
    let mut cadena = Vec::new();
    let mut tipo_final = tipo.to_string();
    // Del diseno (el ultimo) hacia el patron.
    let mut tipo_buscado = tipo.to_string();
    let mut idx_buscado = idx.to_string();
    for (k, p) in previos.iter().enumerate().rev() {
        let es_patron = k == 0 && previos.len() > 1 || p.raiz.nombre == "sldMaster";
        let Some(arbol) = p.arbol() else { continue };
        let candidatos: Vec<(&Nodo, String, String)> = marcadores(arbol);
        let mismo_tipo = |t: &str| {
            let norm = |x: &str| match x {
                "ctrTitle" => "title".to_string(),
                "subTitle" | "obj" | "" => "body".to_string(),
                o => o.to_string(),
            };
            norm(t) == norm(&tipo_buscado)
        };
        let encontrado = if !es_patron && !idx_buscado.is_empty() {
            candidatos
                .iter()
                .find(|(_, _, i)| *i == idx_buscado)
                .or_else(|| candidatos.iter().find(|(_, t, _)| mismo_tipo(t)))
        } else {
            candidatos.iter().find(|(_, t, _)| mismo_tipo(t))
        };
        if let Some((n, t, i)) = encontrado {
            cadena.push(*n);
            if !t.is_empty() {
                tipo_buscado = t.clone();
                if tipo_final == "body" || tipo_final == "obj" {
                    tipo_final = t.clone();
                }
            }
            idx_buscado = i.clone();
        }
    }
    cadena.reverse();
    Herencia {
        cadena,
        tipo: tipo_final,
    }
}

/// Los marcadores de un arbol: (forma, tipo, idx).
fn marcadores(arbol: &Nodo) -> Vec<(&Nodo, String, String)> {
    let mut v = Vec::new();
    for n in arbol.elementos() {
        let nv = n.elementos().find(|e| e.nombre.starts_with("nv"));
        if let Some(ph) = nv.and_then(|v| v.hijo("nvPr")).and_then(|p| p.hijo("ph")) {
            let t = ph.atributo("type");
            v.push((
                n,
                if t.is_empty() {
                    "body".to_string()
                } else {
                    t.to_string()
                },
                ph.atributo("idx").to_string(),
            ));
        }
    }
    v
}

fn herencia_xfrm(h: &Herencia) -> Option<Xfrm> {
    h.cadena
        .iter()
        .rev()
        .find_map(|n| n.hijo("spPr").and_then(|s| s.hijo("xfrm")).map(dml::xfrm))
}

/// La posicion de una forma: la suya o la de su marcador.
fn xfrm_heredado(sppr: Option<&Nodo>, h: &Herencia, grupo: Option<Rect>) -> Option<Xfrm> {
    let x = sppr
        .and_then(|s| s.hijo("xfrm"))
        .map(dml::xfrm)
        .or_else(|| herencia_xfrm(h))?;
    Some(ajustar_a_grupo(x, grupo))
}

/// Pasa una posicion en coordenadas de hijo de un grupo (`marco` lleva el
/// origen de los hijos y la escala) a coordenadas del grupo.
fn ajustar_a_grupo(mut x: Xfrm, marco: Option<Rect>) -> Xfrm {
    if let Some(m) = marco {
        x.x = (x.x - m.x) * m.ancho;
        x.y = (x.y - m.y) * m.alto;
        x.ancho *= m.ancho;
        x.alto *= m.alto;
    }
    x
}

#[cfg(test)]
mod pruebas;
