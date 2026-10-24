//! Pintar una vez lo que no se mueve, y copiarlo en cada fotograma.
//!
//! Mientras se arrastra un elemento, los otros 7.999 no cambian. En una iGPU
//! con memoria compartida, pintarlos una vez a un mapa de bits y copiarlo es
//! la diferencia entre arrastrar y **ver** arrastrar.
//!
//! # Solo vive durante un gesto
//!
//! La tentacion es cachear siempre e ir invalidando cuando algo cambia. Eso
//! obliga a saber que ha cambiado en la escena en cada fotograma y acaba en
//! un sistema de invalidacion tan caro como lo que ahorra.
//!
//! Aqui la capa se prepara al pulsar —con todo menos lo seleccionado—, se
//! copia mientras dura el arrastre, y se tira al soltar. Fuera de un gesto
//! no hay nada que invalidar porque no hay capa. Y durante el gesto lo unico
//! que cambia es lo seleccionado, que es justo lo que se excluye.
//!
//! # Lo que cuesta
//!
//! En 1080p son unos 8 MB, y en la maquina suelo la memoria de video sale de
//! los mismos 4 GB. Es **una de las tres copias vivas** que el presupuesto
//! concede al nivel `Ligero`, y solo mientras dura un arrastre. Queda para
//! medir en la tarea 15, no para prometer.

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_POINT_2U, D2D_RECT_U, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, ID2D1Bitmap1,
    ID2D1RenderTarget,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::Interface;

use crate::lienzo::{Interpolacion, Pintor, RectF};
use crate::motor::{ErrorRender, MotorRender};

/// Con que se preparo la capa.
#[derive(Debug, Clone, PartialEq)]
pub struct Estampa {
    /// `x`, `y` y aumento de la camara.
    pub camara: (f32, f32, f32),
    pub tamano: (u32, u32),
    /// Lo que NO esta en la capa porque se esta moviendo.
    pub excluidos: Vec<u64>,
}

/// Si la capa preparada con `preparada` sirve para pintar `ahora`.
///
/// La comparacion de la camara es exacta a proposito. Podria pensarse en una
/// tolerancia —«si se movio menos de un pixel, vale»— pero encuadrar mueve
/// la camara de verdad y la capa esta en coordenadas de pantalla: cualquier
/// desplazamiento la desalinea, y un dibujo medio pixel corrido se ve.
pub fn sigue_valiendo(preparada: &Estampa, ahora: &Estampa) -> bool {
    if preparada.camara != ahora.camara || preparada.tamano != ahora.tamano {
        return false;
    }
    // El orden de la seleccion no significa nada, asi que se comparan como
    // conjuntos: invalidar por reordenarla tiraria la capa sin motivo.
    if preparada.excluidos.len() != ahora.excluidos.len() {
        return false;
    }
    ahora
        .excluidos
        .iter()
        .all(|id| preparada.excluidos.contains(id))
}

/// El mapa de bits de lo que no se mueve durante un gesto.
///
/// Solo vive durante un gesto (ver la documentacion del modulo): fuera de
/// uno, `bitmap` es `None` y no hay nada que ocupe memoria de video ni que
/// invalidar.
pub struct CapaEstatica {
    bitmap: Option<ID2D1Bitmap1>,
    /// Con que se preparo. Vive junto al bitmap: si uno es `None` el otro
    /// tambien, nunca a medias.
    estampa: Option<Estampa>,
}

impl CapaEstatica {
    pub fn nueva() -> Self {
        Self {
            bitmap: None,
            estampa: None,
        }
    }

    /// Prepara la capa: crea un mapa de bits del tamano de `e.tamano`, lo
    /// pone como destino y ejecuta `pintar` sobre el —la escena sin lo
    /// excluido, que decide el llamante— antes de devolver el destino a la
    /// pantalla. Reusa `MotorRender::dibujar` para el protocolo
    /// SetTarget/BeginDraw/EndDraw en vez de repetirlo aqui.
    pub fn preparar(
        &mut self,
        motor: &mut MotorRender,
        e: Estampa,
        pintar: impl FnOnce(&Pintor),
    ) -> Result<(), ErrorRender> {
        let (ancho, alto) = e.tamano;
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                // Destino de dibujo: como el resto de destinos del crate
                // (ver `motor::envolver`), va premultiplicado.
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: sin datos iniciales (None), D2D solo reserva la memoria;
        // el tamano y el formato de las propiedades son coherentes entre si
        // y el contexto esta vivo (lo mantiene `motor`).
        let bitmap = unsafe {
            motor.contexto().CreateBitmap(
                D2D_SIZE_U {
                    width: ancho,
                    height: alto,
                },
                None,
                0,
                &propiedades,
            )?
        };
        motor.dibujar(&bitmap, pintar)?;
        self.bitmap = Some(bitmap);
        self.estampa = Some(e);
        Ok(())
    }

    /// Si la capa vale para `ahora`, copia el mapa de bits sobre `destino` y
    /// devuelve `true`. Si no vale, no toca `destino` y devuelve `false`:
    /// quien llama pinta todo como siempre. Nunca repinta la capa aqui —eso
    /// solo lo hace `preparar`, al principio del gesto.
    pub fn volcar(&self, motor: &mut MotorRender, destino: &ID2D1Bitmap1, ahora: &Estampa) -> bool {
        self.volcar_zona(motor, destino, ahora, None)
    }

    /// Lo mismo, pero copiando SOLO un trozo.
    ///
    /// Copiar la pantalla entera sesenta veces por segundo es lo que hacia
    /// lenta la tinta: en el equipo del usuario eran 6-8 ms por fotograma
    /// —medidos— antes de dibujar nada, y en una grafica integrada eso es
    /// medio fotograma tirado en mover pixeles que no cambian.
    ///
    /// Quien llama tiene que pasar la union de la zona sucia de ESTE
    /// fotograma y la del anterior: la cadena de intercambio tiene dos
    /// mapas, asi que el que se pinta ahora lleva dentro lo de hace DOS
    /// fotogramas. Con solo la zona de este quedaria pegada la punta del
    /// trazo de hace dos, que es el fantasma clasico de esta tecnica.
    pub fn volcar_zona(
        &self,
        motor: &mut MotorRender,
        destino: &ID2D1Bitmap1,
        ahora: &Estampa,
        zona: Option<(i32, i32, i32, i32)>,
    ) -> bool {
        let (Some(bitmap), Some(preparada)) = (&self.bitmap, &self.estampa) else {
            return false;
        };
        if !sigue_valiendo(preparada, ahora) {
            return false;
        }
        let (ancho, alto) = ahora.tamano;
        let entera = RectF {
            x: 0.0,
            y: 0.0,
            ancho: ancho as f32,
            alto: alto as f32,
        };
        let caja = match zona {
            None => entera,
            Some((x0, y0, x1, y1)) => {
                let x0 = x0.clamp(0, ancho as i32) as f32;
                let y0 = y0.clamp(0, alto as i32) as f32;
                let x1 = x1.clamp(0, ancho as i32) as f32;
                let y1 = y1.clamp(0, alto as i32) as f32;
                // Una zona vacia tras recortar no es «no cambio nada», es
                // «no se sabe»: se copia entera, como antes.
                if x1 <= x0 || y1 <= y0 {
                    entera
                } else {
                    RectF {
                        x: x0,
                        y: y0,
                        ancho: x1 - x0,
                        alto: y1 - y0,
                    }
                }
            }
        };
        motor
            .dibujar(destino, |p| {
                // **Se borra antes lo que habia** (8-oct-2026). `bitmap` pinta
                // ENCIMA: con una capa opaca da igual, pero en el anotador de
                // pantalla viva el papel es transparente, y donde la capa no
                // tiene nada quedaba el fotograma anterior. Redimensionar una
                // imagen pegada dejaba una estela de todos sus tamanos hasta
                // soltar (lo vio el usuario).
                p.empujar_recorte(caja);
                p.limpiar_transparente();
                p.soltar_recorte();
                // Mismo rectangulo en origen y en destino: es una copia, no
                // un escalado, asi que no hay interpolacion que valorar.
                p.bitmap(bitmap, caja, Some(caja), true);
            })
            .is_ok()
    }

    /// Tira el mapa de bits de verdad (no lo marca como invalido): si no, los
    /// 8 MB se quedan vivos entre gestos y el presupuesto de copias vivas se
    /// incumple en reposo, que es donde mas duele.
    pub fn soltar(&mut self) {
        self.bitmap = None;
        self.estampa = None;
    }

    pub fn lista(&self) -> bool {
        self.bitmap.is_some()
    }

    /// Cuanto ocupa el mapa de bits ahora mismo. Cero si no hay gesto en
    /// curso.
    pub fn bytes(&self) -> usize {
        match &self.estampa {
            Some(e) if self.bitmap.is_some() => e.tamano.0 as usize * e.tamano.1 as usize * 4,
            _ => 0,
        }
    }
}

impl Default for CapaEstatica {
    fn default() -> Self {
        Self::nueva()
    }
}

/// **Tapa un trozo de lo ya pintado: el mosaico de verdad.**
///
/// Esta aqui, junto a la capa estatica, porque es la misma idea llevada mas
/// lejos: leer lo que ya se pinto en vez de volver a pintarlo. La diferencia
/// es que la capa lo copia tal cual y esto lo **remuestrea**.
///
/// El camino es el mismo del movil (`Renderer.kt:1319-1425`), con la GPU
/// haciendo las dos escalas: se recorta la zona de lo ya pintado, se encoge a
/// la resolucion del grano —lo que promedia cada cuadro— y se vuelve a
/// estirar. Sin filtrar al estirar salen los bloques de canto duro; con filtro
/// bilineal, los mismos bloques se convierten en mancha. Dos efectos por un
/// solo camino, y **ninguno inventa pixeles**: lo que tapa el mosaico es lo
/// que habia debajo, promediado hasta no leerse.
///
/// Reducir SIEMPRE va con filtro, tambien al pixelar: asi cada bloque sale del
/// promedio de lo que tapa y no del pixel que caiga en la rejilla. Sin
/// promediar, mover el mosaico un pixel puede cambiar el bloque entero, y
/// sobre texto pequeno llegan a leerse letras dentro de un bloque.
///
/// `zona` va en **pixeles de pantalla**, que es donde estan los pixeles; el
/// grano del movil va en pixeles de la escena y lo traduce
/// `pixpin_motor2d::mosaico::lado_en_pantalla`. Y aqui **no hay opacidad**: un
/// mosaico a medio tapar no tapa (ver `mosaico.rs`).
///
/// Quien llama tiene que haber pintado ya lo que va **debajo** del mosaico y
/// no haber pintado todavia lo de encima; y no puede pasar aqui la caja de
/// otro mosaico, o se pixelaria lo ya pixelado y se degradaria en cada pasada.
pub fn tapar(
    motor: &mut MotorRender,
    destino: &ID2D1Bitmap1,
    zona: (i32, i32, i32, i32),
    lado: u32,
    desenfoque: bool,
) -> Result<(), ErrorRender> {
    let (x0, y0, x1, y1) = zona;
    let (ancho, alto) = ((x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32);
    // Una zona vacia no es un error: es un mosaico que quedo fuera de la
    // pantalla, y lo que hay que hacer con el es nada.
    if ancho == 0 || alto == 0 {
        return Ok(());
    }
    let lado = lado.max(1).min(ancho).min(alto);
    // Hacia arriba: con la division entera, una zona de 30 con grano 16 daria
    // un solo cuadro y el mosaico taparia mas grueso de lo pedido.
    let (mini_ancho, mini_alto) = (ancho.div_ceil(lado), alto.div_ceil(lado));

    let propiedades = |opciones| D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: opciones,
        colorContext: std::mem::ManuallyDrop::new(None),
    };

    let contexto = motor.contexto().clone();
    // SAFETY: el contexto esta vivo (lo mantiene `motor`) y el tamano es el de
    // la zona, que ya se comprobo no vacia. Sin datos iniciales, D2D solo
    // reserva memoria.
    let copia = unsafe {
        contexto.CreateBitmap(
            D2D_SIZE_U {
                width: ancho,
                height: alto,
            },
            None,
            0,
            &propiedades(D2D1_BITMAP_OPTIONS_NONE),
        )?
    };

    // El recorte de lo ya pintado. `CopyFromRenderTarget` necesita que el
    // destino de dibujo sea el que se quiere leer, asi que se pone y se
    // quita aqui mismo: no se puede hacer dentro de `MotorRender::dibujar`,
    // que abre un fotograma y no expone el contexto.
    let como_destino: ID2D1RenderTarget = contexto.cast()?;
    // SAFETY: `destino` es un bitmap de destino del llamante y `copia` es de
    // este dispositivo, del tamano exacto del rectangulo que se le copia, que
    // a su vez cae dentro del destino porque el llamante ya lo recorto. El
    // destino se suelta antes de salir.
    unsafe {
        contexto.SetTarget(destino);
        let r = copia.CopyFromRenderTarget(
            Some(&D2D_POINT_2U { x: 0, y: 0 }),
            &como_destino,
            Some(&D2D_RECT_U {
                left: x0.max(0) as u32,
                top: y0.max(0) as u32,
                right: x1.max(0) as u32,
                bottom: y1.max(0) as u32,
            }),
        );
        contexto.SetTarget(None);
        r?;
    }

    // SAFETY: lo mismo que arriba, con el tamano de la miniatura, que nunca
    // es cero (`div_ceil` sobre un ancho no nulo).
    let mini = unsafe {
        contexto.CreateBitmap(
            D2D_SIZE_U {
                width: mini_ancho,
                height: mini_alto,
            },
            None,
            0,
            &propiedades(D2D1_BITMAP_OPTIONS_TARGET),
        )?
    };
    motor.dibujar(&mini, |p| {
        p.bitmap_con(
            &copia,
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: mini_ancho as f32,
                alto: mini_alto as f32,
            },
            None,
            // Al reducir, siempre con filtro: es lo que promedia el cuadro.
            Interpolacion::Lineal,
        );
    })?;

    motor.dibujar(destino, |p| {
        p.bitmap_con(
            &mini,
            RectF {
                x: x0 as f32,
                y: y0 as f32,
                ancho: ancho as f32,
                alto: alto as f32,
            },
            None,
            if desenfoque {
                Interpolacion::Lineal
            } else {
                Interpolacion::Vecino
            },
        );
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn estampa() -> Estampa {
        Estampa {
            camara: (0.0, 0.0, 1.0),
            tamano: (1920, 1080),
            excluidos: vec![7],
        }
    }

    #[test]
    fn la_misma_estampa_vale() {
        assert!(sigue_valiendo(&estampa(), &estampa()));
    }

    #[test]
    fn mover_la_camara_la_invalida() {
        // La capa esta pintada en coordenadas de pantalla: si el lienzo se
        // desplaza, lo pintado ya no cae donde toca.
        let mut ahora = estampa();
        ahora.camara.0 += 1.0;
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_el_aumento_la_invalida() {
        let mut ahora = estampa();
        ahora.camara.2 = 2.0;
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_el_tamano_de_la_ventana_la_invalida() {
        let mut ahora = estampa();
        ahora.tamano = (1280, 720);
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_lo_excluido_la_invalida() {
        // Si la seleccion cambia a mitad de gesto, la capa lleva pintado un
        // elemento que ahora se esta moviendo: se veria por duplicado, uno
        // quieto y otro siguiendo al raton.
        let mut ahora = estampa();
        ahora.excluidos = vec![8];
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn el_orden_de_lo_excluido_no_importa() {
        // La seleccion es un Vec cuyo orden no significa nada. Invalidar
        // por reordenarlo tiraria la capa sin motivo.
        let preparada = Estampa {
            excluidos: vec![1, 2, 3],
            ..estampa()
        };
        let ahora = Estampa {
            excluidos: vec![3, 1, 2],
            ..estampa()
        };
        assert!(sigue_valiendo(&preparada, &ahora));
    }

    #[test]
    fn una_capa_recien_creada_no_esta_lista() {
        let c = CapaEstatica::nueva();
        assert!(!c.lista());
        assert_eq!(c.bytes(), 0, "sin gesto en curso no ocupa nada");
    }

    // A partir de aqui, lo que toca GPU: necesita un dispositivo Direct3D y
    // windows-latest no lo ofrece de forma fiable (la misma razon que las
    // pruebas de bandeja y de atajos van marcadas #[ignore]).

    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;

    use crate::motor::Color;

    fn dispositivo() -> (ID3D11Device, ID3D11DeviceContext) {
        let mut d3d = None;
        let mut ctx = None;
        // SAFETY: salidas locales, constantes documentadas.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                Some(&mut ctx),
            )
            .expect("GPU real");
        }
        (d3d.unwrap(), ctx.unwrap())
    }

    fn textura(d3d: &ID3D11Device, ancho: u32, alto: u32) -> ID3D11Texture2D {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            ..Default::default()
        };
        let mut t = None;
        // SAFETY: desc inicializada; t es la salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.unwrap();
        t.unwrap()
    }

    fn pixel(
        d3d: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        t: &ID3D11Texture2D,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc rellena la estructura local.
        unsafe { t.GetDesc(&mut desc) };
        let e_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut e = None;
        // SAFETY: desc inicializada; e es la salida local.
        unsafe { d3d.CreateTexture2D(&e_desc, None, Some(&mut e)) }.unwrap();
        let e = e.unwrap();
        // SAFETY: mismo formato/tamano; staging legible tras CopyResource.
        unsafe { ctx.CopyResource(&e, t) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: staging con lectura; se desmapea justo despues.
        unsafe { ctx.Map(&e, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.unwrap();
        // SAFETY: (x, y) dentro de la textura, obligacion del llamante.
        let v = unsafe {
            let base =
                (mapa.pData as *const u8).add(y as usize * mapa.RowPitch as usize + x as usize * 4);
            [*base, *base.add(1), *base.add(2), *base.add(3)]
        };
        // SAFETY: empareja el Map de arriba.
        unsafe { ctx.Unmap(&e, 0) };
        v
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn preparar_pinta_la_capa_y_volcar_la_copia_sobre_el_destino() {
        let (d3d, ctx) = dispositivo();
        let mut motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();

        let mut capa = CapaEstatica::nueva();
        assert!(!capa.lista());

        let e = Estampa {
            camara: (0.0, 0.0, 1.0),
            tamano: (64, 64),
            excluidos: vec![],
        };
        capa.preparar(&mut motor, e.clone(), |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        })
        .expect("deberia prepararse la capa");
        assert!(capa.lista());
        assert_eq!(capa.bytes(), 64 * 64 * 4);

        // Limpia el destino real a azul antes de volcar: si volcar no
        // copiara nada, el destino seguiria azul.
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
            })
            .unwrap();

        let volco = capa.volcar(&mut motor, &destino, &e);
        assert!(volco, "la misma estampa deberia seguir valiendo");
        // BGRA: el rojo de la capa quedo copiado sobre el destino.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 32, 32), [0, 0, 255, 255]);

        capa.soltar();
        assert!(!capa.lista(), "soltar tiene que dejar caer el bitmap");
        assert_eq!(capa.bytes(), 0, "sin capa no hay memoria ocupada");
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn el_mosaico_coge_los_pixeles_de_debajo_y_los_promedia_en_cuadros() {
        // **La prueba del mosaico contra la GPU.** Se pinta un damero de un
        // pixel —el peor caso, porque cualquier resto de detalle se ve— y se
        // tapa la mitad izquierda con cuadros de ocho.
        //
        // Lo que se comprueba es lo unico que dice que el mosaico tapa: que
        // dentro de un cuadro no queda un pixel distinto de otro, y que el
        // color que sale es el PROMEDIO de lo que habia —gris medio— y no un
        // color inventado. Y de paso, que la mitad de la derecha sigue siendo
        // el damero, porque un mosaico que se come lo que no es suyo tapa de
        // mas.
        let (d3d, ctx) = dispositivo();
        let mut motor = MotorRender::nuevo(&d3d).unwrap();
        let tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&tex).unwrap();

        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color::BLANCO);
                for y in 0..64 {
                    for x in 0..64 {
                        if (x + y) % 2 == 0 {
                            p.rellenar(
                                RectF {
                                    x: x as f32,
                                    y: y as f32,
                                    ancho: 1.0,
                                    alto: 1.0,
                                },
                                Color::NEGRO,
                            );
                        }
                    }
                }
            })
            .unwrap();
        // Antes de tapar, el damero es damero: dos pixeles vecinos distintos.
        assert_ne!(
            pixel(&d3d, &ctx, &tex, 4, 4),
            pixel(&d3d, &ctx, &tex, 5, 4),
            "el damero no se pinto"
        );

        tapar(&mut motor, &destino, (0, 0, 32, 64), 8, false).expect("deberia tapar");

        // Dentro de un cuadro, todo igual; y ese igual es gris medio.
        let base = pixel(&d3d, &ctx, &tex, 0, 0);
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(pixel(&d3d, &ctx, &tex, x, y), base, "({x},{y})");
            }
        }
        assert!(
            (100..=155).contains(&base[0]),
            "no es el promedio del damero: {base:?}"
        );
        // Caso negativo: fuera de la zona el damero sigue intacto.
        assert_ne!(
            pixel(&d3d, &ctx, &tex, 40, 4),
            pixel(&d3d, &ctx, &tex, 41, 4),
            "el mosaico tapo mas alla de su caja"
        );
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn un_mosaico_fuera_de_la_pantalla_no_es_un_error() {
        // Un mosaico que quedo fuera del encuadre no tiene nada que tapar, y
        // eso no puede tumbar el fotograma.
        let (d3d, _ctx) = dispositivo();
        let mut motor = MotorRender::nuevo(&d3d).unwrap();
        let tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&tex).unwrap();
        assert!(tapar(&mut motor, &destino, (10, 10, 10, 40), 8, false).is_ok());
        assert!(tapar(&mut motor, &destino, (0, 0, 0, 0), 8, true).is_ok());
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn volcar_no_copia_nada_si_la_estampa_ya_no_vale() {
        let (d3d, ctx) = dispositivo();
        let mut motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();

        let mut capa = CapaEstatica::nueva();
        let preparada = Estampa {
            camara: (0.0, 0.0, 1.0),
            tamano: (64, 64),
            excluidos: vec![],
        };
        capa.preparar(&mut motor, preparada, |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        })
        .unwrap();

        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
            })
            .unwrap();

        // La camara se movio a mitad de gesto (p.ej. se encuadro): la
        // estampa ya no vale y volcar no debe tocar el destino.
        let ahora = Estampa {
            camara: (5.0, 0.0, 1.0),
            tamano: (64, 64),
            excluidos: vec![],
        };
        let volco = capa.volcar(&mut motor, &destino, &ahora);
        assert!(!volco, "la camara se movio: la capa no vale");
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 32, 32), [255, 0, 0, 255]);
    }
}
