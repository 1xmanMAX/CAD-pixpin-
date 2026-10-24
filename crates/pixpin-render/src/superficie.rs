//! Una superficie de composicion por ventana de overlay.
//!
//! `WS_EX_NOREDIRECTIONBITMAP` quita a la ventana su superficie GDI: lo
//! unico que se ve es lo que este swapchain presenta a traves de
//! DirectComposition. Es el camino sin parpadeo y sin copias del escritorio
//! moderno, el mismo que usa QuickView — la tecnica, no el codigo.
//!
//! El present es A DEMANDA: se presenta cuando algo cambio, nunca en un
//! bucle de fotogramas. De ahi sale el 0% de CPU en reposo del overlay.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::DirectComposition::{
    DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR, DCompositionCreateDevice, IDCompositionDevice,
    IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2,
    IDXGISwapChain1,
};
use windows::core::Interface;

use crate::motor::{ErrorRender, MotorRender};

/// Lo que hace falta saber del reloj de DWM para pintar justo a tiempo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RitmoComposicion {
    /// Milisegundos que faltan para la siguiente composicion.
    pub hasta_el_plazo_ms: f32,
    /// Cada cuanto compone DWM (16,67 ms a 60 Hz).
    pub periodo_ms: f32,
}

/// Una capa de composicion encima de la escena: su visual y su superficie.
///
/// Existe por A3: si la escena se desplaza moviendo su visual, lo que no es
/// escena no puede ir en la misma superficie o la barra de herramientas se
/// iria con el lienzo. Son superficies de composicion y no segundas
/// swapchains a proposito: una swapchain tiene dos buffers (el doble de
/// memoria de video) y, sobre todo, en una swapchain el mapa que se pinta
/// lleva dentro lo de hace DOS presentes, que es lo que obliga a la union de
/// zonas sucias de D148. Una `IDCompositionSurface` conserva lo que tenia:
/// se actualiza el trozo que cambia y ya.
///
/// Hay dos: la de la INTERFAZ (la barra, el panel y la ruta del universo) y
/// la de la TINTA viva (B2), que va entre la escena y la interfaz.
struct CapaDComp {
    visual: IDCompositionVisual,
    superficie: windows::Win32::Graphics::DirectComposition::IDCompositionSurface,
    tamano: (u32, u32),
    /// Si el visual tiene la superficie puesta. La capa de tinta se queda
    /// SIN contenido mientras no se dibuja: un visual transparente de
    /// pantalla entera le cuesta a DWM una mezcla por composicion aunque no
    /// tenga ni un pixel pintado.
    encendida: std::cell::Cell<bool>,
}

pub struct Superficie {
    dcomp: IDCompositionDevice,
    /// En `Option` solo para poder soltarlo ANTES de pedir otro para la
    /// misma ventana (`rehacer`): DirectComposition no deja dos objetivos
    /// sobre una ventana.
    objetivo: Option<IDCompositionTarget>,
    /// La ventana y el dispositivo sobre los que nacio, para `rehacer` y
    /// para el fallo inyectado de las pruebas (`perdida`).
    hwnd: HWND,
    d3d_crudo: usize,
    /// El visual de la ESCENA (lo que lleva la swapchain). Es el que se
    /// mueve al desplazar y el que se estira al acercar.
    visual: IDCompositionVisual,
    /// La raiz, para colgar de ella la capa de interfaz.
    _raiz: IDCompositionVisual,
    swapchain: IDXGISwapChain1,
    /// Tamano real de los buffers. Puede ser MAYOR que la ventana: la
    /// composicion recorta al area de la ventana, y asi un zoom no tiene
    /// que reasignar memoria de video en cada fotograma (histeresis).
    asignado: std::cell::Cell<(u32, u32)>,
    /// Hay una transformada de estirado puesta en el visual.
    estirada: std::cell::Cell<bool>,
    /// La escala y el desplazamiento del estirado (`estirar`), guardados
    /// aparte del paneo porque se combinan en una sola matriz.
    estiramiento: std::cell::Cell<(f32, f32, f32, f32)>,
    /// A3: cuantos pixeles de mas tiene la superficie de escena POR CADA
    /// LADO. Es el colchon que permite desplazar moviendo el visual sin
    /// ensenar borde vacio; a 0 la superficie se comporta como siempre.
    margen: u32,
    /// A3: cuanto se ha corrido el visual de la escena desde el ultimo
    /// repintado, en pixeles de ventana.
    desplazamiento: std::cell::Cell<(f32, f32)>,
    /// Banderas con las que nacio la swapchain: `ResizeBuffers` tiene que
    /// repetirlas o pierde la espera de fotograma.
    banderas: DXGI_SWAP_CHAIN_FLAG,
    /// La senal de «ya se puede presentar otro fotograma», solo en la
    /// superficie de baja latencia. Se cierra en `Drop`.
    senal: Option<windows::Win32::Foundation::HANDLE>,
    interfaz: std::cell::RefCell<Option<CapaDComp>>,
    /// B2: la capa del trazo en curso, entre la escena y la interfaz.
    tinta: std::cell::RefCell<Option<CapaDComp>>,
    /// A3 fase 2, DEBAJO de la escena: el degradado del cielo del universo,
    /// que no se mueve nunca (esta en pixeles de pantalla).
    cielo: std::cell::RefCell<Option<CapaDComp>>,
    /// Y las estrellas, que se mueven a su paralaje, no al del lienzo.
    fondo: std::cell::RefCell<Option<CapaDComp>>,
    margen_fondo: std::cell::Cell<u32>,
    desplazamiento_fondo: std::cell::Cell<(f32, f32)>,
}

/// Si `hwnd` es la ventana del primer plano. Vive aqui porque quien la
/// necesita es el bucle de pintado del editor (para devolver la memoria de
/// video al dejar de estar delante, `MotorRender::devolver_memoria`), y la
/// aplicacion no escribe `unsafe`.
pub fn en_primer_plano(hwnd: HWND) -> bool {
    // SAFETY: consulta de solo lectura del estado global de ventanas.
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() == hwnd }
}

impl Drop for Superficie {
    fn drop(&mut self) {
        if let Some(s) = self.senal.take() {
            // SAFETY: el handle lo dio GetFrameLatencyWaitableObject y es
            // nuestro; se cierra una sola vez.
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(s);
            }
        }
    }
}

impl Superficie {
    /// La ventana del llamante debe sobrevivir a la Superficie (obligacion
    /// del llamante: en el overlay, la ventana posee a su Superficie).
    pub fn nueva(
        motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
    ) -> Result<Self, ErrorRender> {
        Self::crear(motor, d3d, hwnd, ancho, alto, false, 0)
    }

    /// Superficie para dibujar a mano: latencia de fotograma 1 y senal de
    /// espera (`senal_fotograma`).
    ///
    /// Por defecto DXGI deja encolar hasta tres fotogramas. Un editor que
    /// pinta en 3 ms los llena, y cada trazo sale con esos fotogramas de
    /// retraso: la tinta va detras del raton. Con la cola en 1 y esperando a
    /// la senal ANTES de leer el raton, los puntos se leen lo mas tarde
    /// posible y se ven en el siguiente refresco (tecnica de «frame latency
    /// waitable object» de Microsoft para aplicaciones de tinta y juegos).
    pub fn nueva_baja_latencia(
        motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
    ) -> Result<Self, ErrorRender> {
        Self::crear(motor, d3d, hwnd, ancho, alto, true, 0)
    }

    /// Como `nueva_baja_latencia`, pero con **colchon** alrededor de la
    /// escena y una capa aparte para la interfaz (A3).
    ///
    /// La superficie de escena se hace `margen` pixeles mas grande por cada
    /// lado y su visual nace corrido `-margen`: lo que se ve por la ventana
    /// sigue siendo el centro. A cambio, desplazar el lienzo hasta `margen`
    /// pixeles es mover el visual (`desplazar_escena`) en vez de repintar la
    /// escena entera, que es lo que costaba 15 ms con 2.000 elementos y
    /// 83 ms con 10.000 aunque no se creara ni un objeto de Direct2D.
    ///
    /// La barra y el panel van en `pintar_interfaz`, que no se mueve.
    pub fn nueva_con_capas(
        motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
        margen: u32,
    ) -> Result<Self, ErrorRender> {
        Self::crear(motor, d3d, hwnd, ancho, alto, true, margen)
    }

    /// El handle crudo que se puede esperar (junto con los mensajes) hasta
    /// que DXGI admita otro fotograma. `None` en la superficie normal.
    pub fn senal_fotograma(&self) -> Option<isize> {
        self.senal.map(|h| h.0 as isize)
    }

    /// Pixeles de colchon por lado. 0 en las superficies de siempre.
    pub fn margen(&self) -> f32 {
        self.margen as f32
    }

    /// Si esta superficie tiene capa de interfaz separada, es decir, si la
    /// escena se puede mover sin llevarse la barra.
    pub fn tiene_capas(&self) -> bool {
        self.margen > 0
    }

    fn crear(
        _motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
        baja_latencia: bool,
        margen: u32,
    ) -> Result<Self, ErrorRender> {
        let (ancho, alto) = (ancho.max(1), alto.max(1));
        // La superficie de la escena lleva el colchon; la ventana sigue
        // siendo `ancho` x `alto` y la composicion recorta a ella.
        let (ancho_escena, alto_escena) = (ancho + margen * 2, alto + margen * 2);
        use windows::Win32::Graphics::Dxgi::{
            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, IDXGISwapChain2,
        };
        let banderas = if baja_latencia {
            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT
        } else {
            DXGI_SWAP_CHAIN_FLAG(0)
        };
        let dxgi: IDXGIDevice = d3d.cast().map_err(|_| ErrorRender::SinDxgi)?;
        // SAFETY: el adaptador y la factoria se obtienen del dispositivo del
        // llamante, vivo durante toda la llamada.
        let fabrica: IDXGIFactory2 = unsafe { dxgi.GetAdapter()?.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: ancho_escena,
            Height: alto_escena,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            // Dos buffers y flip: el minimo que permite componer sin copia.
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            Scaling: DXGI_SCALING_STRETCH,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: banderas.0 as u32,
            ..Default::default()
        };
        // SAFETY: swapchain DE COMPOSICION (sin HWND propio): es el unico
        // tipo valido para NOREDIRECTIONBITMAP; parametros documentados.
        let swapchain = unsafe { fabrica.CreateSwapChainForComposition(&dxgi, &desc, None)? };
        let senal = if baja_latencia {
            // SAFETY: la swapchain se creo con la bandera de espera, que es
            // la condicion de las dos llamadas; el handle es nuestro.
            unsafe {
                let s2: IDXGISwapChain2 = swapchain.cast()?;
                s2.SetMaximumFrameLatency(1)?;
                Some(s2.GetFrameLatencyWaitableObject())
            }
        } else {
            None
        };

        // SAFETY: dispositivo de composicion sobre el mismo DXGI; el target
        // toma la ventana del llamante (ver la obligacion en el doc de
        // `nueva`).
        let dcomp: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi)? };
        // SAFETY: los objetos se acaban de crear y estan vivos; Commit
        // publica el arbol de composicion.
        // Con capas, la raiz es un visual vacio del que cuelgan la escena y
        // la interfaz. Sin capas, la raiz ES el visual de la escena, igual
        // que siempre: una superficie de pin no paga ni un objeto de mas.
        let (objetivo, visual, raiz, tinta, interfaz) = unsafe {
            let objetivo = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swapchain)?;
            if margen == 0 {
                objetivo.SetRoot(&visual)?;
                dcomp.Commit()?;
                (objetivo, visual.clone(), visual, None, None)
            } else {
                let raiz = dcomp.CreateVisual()?;
                raiz.AddVisual(&visual, false, None)?;
                // Una capa por cada cosa que se mueve a su ritmo, de abajo
                // arriba: la escena (se desplaza al panear), la tinta viva
                // (no se mueve: se dibuja donde esta la mano) y la interfaz
                // (no se mueve nunca).
                //
                // **El orden se consigue anadiendo por el FINAL de la lista
                // de hijos** (`AddVisual(v, false, None)`), y el final es lo
                // que se ve encima. Medido, no supuesto: con
                // `AddVisual(v, true, None)` -que es «al principio de la
                // lista»- la barra de herramientas quedaba DEBAJO del
                // lienzo, que se pinta opaco, y no se veia; la prueba
                // `una_ventana_compuesta_ensena_la_interfaz_encima_de_la_escena`
                // lo comprueba leyendo la pantalla. Pasar un visual de
                // referencia tampoco vale: con referencia, `true` dejaba la
                // capa de la tinta invisible.
                let capa = || -> Result<CapaDComp, ErrorRender> {
                    let sup = dcomp.CreateSurface(
                        ancho,
                        alto,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        DXGI_ALPHA_MODE_PREMULTIPLIED,
                    )?;
                    let v = dcomp.CreateVisual()?;
                    raiz.AddVisual(&v, false, None)?;
                    Ok(CapaDComp {
                        visual: v,
                        superficie: sup,
                        tamano: (ancho, alto),
                        encendida: std::cell::Cell::new(false),
                    })
                };
                // En este orden: la tinta encima de la escena, la interfaz
                // encima de la tinta.
                let tinta = capa()?;
                let interfaz = capa()?;
                // Las dos con su superficie puesta DESDE AQUI. Ponersela
                // despues, al empezar a trazar, no se veia: el visual se
                // quedaba sin ensenar nada por mucho que la superficie
                // tuviera pixeles. La tinta se apaga vaciandola, que ademas
                // es lo que hay que hacer de todos modos para que no quede
                // el trazo anterior dentro.
                tinta.visual.SetContent(&tinta.superficie)?;
                interfaz.visual.SetContent(&interfaz.superficie)?;
                interfaz.encendida.set(true);
                objetivo.SetRoot(&raiz)?;
                dcomp.Commit()?;
                (objetivo, visual, raiz, Some(tinta), Some(interfaz))
            }
        };

        let s = Self {
            dcomp,
            objetivo: Some(objetivo),
            hwnd,
            d3d_crudo: d3d.as_raw() as usize,
            visual,
            _raiz: raiz,
            swapchain,
            asignado: std::cell::Cell::new((ancho_escena, alto_escena)),
            estirada: std::cell::Cell::new(false),
            estiramiento: std::cell::Cell::new((1.0, 1.0, 0.0, 0.0)),
            margen,
            desplazamiento: std::cell::Cell::new((0.0, 0.0)),
            banderas,
            senal,
            interfaz: std::cell::RefCell::new(interfaz),
            tinta: std::cell::RefCell::new(tinta),
            cielo: std::cell::RefCell::new(None),
            fondo: std::cell::RefCell::new(None),
            margen_fondo: std::cell::Cell::new(0),
            desplazamiento_fondo: std::cell::Cell::new((0.0, 0.0)),
        };
        // El colchon se pone UNA vez y por la misma matriz que el paneo y
        // el estirado. Ponerlo ademas con `SetOffsetX2` lo contaria dos
        // veces en cuanto alguien llamara a `estirar`, y el lienzo saltaria
        // medio colchon en el primer zoom.
        if margen > 0 {
            s.aplicar_transformada();
        }
        Ok(s)
    }

    /// Estira lo YA dibujado sin volver a dibujarlo: el compositor escala la
    /// textura en la GPU. Es como se hace un zoom fluido — redibujar el
    /// contenido en cada fotograma de la animacion es lo que producia
    /// tirones en equipos con graficos integrados.
    ///
    /// `escala_x`/`escala_y` multiplican, `dx`/`dy` desplazan despues, todo
    /// en pixeles de la ventana. Mientras hay transformada, el filtro pasa a
    /// ser el barato: es un fotograma intermedio de una animacion, y el
    /// nitido llega al terminar, con el repintado de verdad.
    pub fn estirar(&self, escala_x: f32, escala_y: f32, dx: f32, dy: f32) {
        self.estiramiento.set((escala_x, escala_y, dx, dy));
        // SAFETY: el visual es propio y sigue vivo; un error solo significa
        // que el fotograma sale sin el filtro barato.
        unsafe {
            let _ = self
                .visual
                .SetBitmapInterpolationMode(DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR);
        }
        self.estirada.set(true);
        self.aplicar_transformada();
    }

    /// Si hay una transformada de estirado puesta, es decir, si lo que se ve
    /// es una textura escalada y no un dibujo nitido.
    pub fn esta_estirada(&self) -> bool {
        self.estirada.get()
    }

    /// Deshace `estirar`. Solo toca la composicion si habia algo que
    /// deshacer: un Commit por fotograma en balde tambien cuesta.
    pub fn dejar_de_estirar(&self) {
        if !self.estirada.get() {
            return;
        }
        self.estiramiento.set((1.0, 1.0, 0.0, 0.0));
        self.estirada.set(false);
        self.aplicar_transformada();
    }

    /// A3: corre el visual de la ESCENA `dx`/`dy` pixeles de ventana, sin
    /// repintar nada y sin tocar la interfaz.
    ///
    /// Es todo el truco: un paneo deja de ser «pintar lo visible sesenta
    /// veces por segundo» y pasa a ser una matriz y un `Commit`. Lo que
    /// asoma por el borde sale del colchon de `margen`; pasado el colchon,
    /// el llamante tiene que repintar (`desplazamiento_valido`).
    pub fn desplazar_escena(&self, dx: f32, dy: f32) {
        if self.margen == 0 || self.desplazamiento.get() == (dx, dy) {
            return;
        }
        self.desplazamiento.set((dx, dy));
        self.aplicar_transformada();
    }

    /// Cuanto lleva corrido el visual de la escena.
    pub fn desplazamiento(&self) -> (f32, f32) {
        self.desplazamiento.get()
    }

    /// Si un desplazamiento de `dx`/`dy` sigue cubierto por el colchon. A
    /// partir de aqui asomaria el borde vacio y hay que repintar.
    pub fn desplazamiento_valido(&self, dx: f32, dy: f32) -> bool {
        self.margen > 0 && dx.abs() <= self.margen as f32 && dy.abs() <= self.margen as f32
    }

    /// Vuelve a dejar la escena en su sitio. Lo llama el repintado nitido:
    /// lo que se acaba de pintar ya esta en la camara nueva.
    pub fn reponer_escena(&self) {
        if self.desplazamiento.get() == (0.0, 0.0) {
            return;
        }
        self.desplazamiento.set((0.0, 0.0));
        self.aplicar_transformada();
    }

    /// La unica matriz del visual de la escena: colchon, paneo y estirado
    /// juntos.
    ///
    /// Van juntos y no en dos propiedades distintas porque el orden importa:
    /// primero se lleva la superficie a coordenadas de ventana (quitarle el
    /// colchon y sumarle el paneo) y solo despues se estira. Con
    /// `SetOffsetX2` por un lado y `SetTransform2` por otro, un zoom a mitad
    /// de un paneo escalaria tambien el desplazamiento.
    fn aplicar_transformada(&self) {
        let (sx, sy, ex, ey) = self.estiramiento.get();
        let (dx, dy) = self.desplazamiento.get();
        let m = self.margen as f32;
        let m = windows_numerics::Matrix3x2 {
            M11: sx,
            M12: 0.0,
            M21: 0.0,
            M22: sy,
            M31: (dx - m) * sx + ex,
            M32: (dy - m) * sy + ey,
        };
        // SAFETY: la matriz vive durante la llamada; el visual y el
        // dispositivo son propios y siguen vivos. Un error solo significa
        // que el fotograma sale sin mover.
        unsafe {
            let _ = self.visual.SetTransform2(&m);
            let _ = self.dcomp.Commit();
        }
    }

    /// Pinta la capa de la INTERFAZ (barra, panel, ruta) y la publica.
    ///
    /// El cierre recibe el `Pintor` y el desplazamiento que tiene que sumar
    /// a todo lo que dibuje: DirectComposition puede devolver la superficie
    /// dentro de una textura mas grande, y no sumarlo pintaria la barra en
    /// el sitio equivocado justo en los equipos donde el atlas no empieza en
    /// cero.
    ///
    /// Hace su propio `Commit`, asi que lo que se pinte aqui aparece con el
    /// siguiente fotograma de la escena o solo, si la escena no cambia.
    pub fn pintar_interfaz(
        &self,
        motor: &MotorRender,
        dibuja: impl FnOnce(&crate::lienzo::Pintor<'_>, (f32, f32)),
    ) -> Result<(), ErrorRender> {
        // Sin rectangulo: la interfaz es casi toda transparente y un trozo
        // sin repintar dejaria basura del fotograma anterior.
        self.pintar_capa(&self.interfaz, motor, None, dibuja)
    }

    /// **B2: el trazo en curso, en su propia capa.**
    ///
    /// Pinta SOLO `zona` (pixeles de ventana) de la capa de la tinta: la
    /// limpia y ejecuta `dibuja` dentro. El resto de la capa se queda como
    /// estaba, que es lo que deja al trazo seguir ahi sin volver a pintarlo.
    ///
    /// Mientras esto dura, la escena NO se toca: ni se hornea al apoyar el
    /// lapiz —un `ID2D1Bitmap1` de pantalla entera, 33,7 MB a 3000 x 2000
    /// con colchon, y la escena entera pintada dentro: 194 ms medidos con
    /// 10.000 elementos—, ni se copia por fotograma (D148), ni se presenta.
    /// Un fotograma de trazo pasa a ser un rectangulo de esta capa y un
    /// `Commit`.
    ///
    /// `dibuja` recibe el desplazamiento que tiene que sumar a todo lo que
    /// pinte: DirectComposition puede devolver la superficie dentro de una
    /// textura mas grande, y no sumarlo pintaria el trazo en el sitio
    /// equivocado justo en los equipos donde el atlas no empieza en cero.
    pub fn pintar_tinta(
        &self,
        motor: &MotorRender,
        zona: Option<(i32, i32, i32, i32)>,
        dibuja: impl FnOnce(&crate::lienzo::Pintor<'_>, (f32, f32)),
    ) -> Result<(), ErrorRender> {
        self.pintar_capa(&self.tinta, motor, zona, dibuja)
    }

    /// Enciende la capa de la tinta: la deja vacia y lista para el trazo.
    ///
    /// Es lo unico que cuesta apoyar el lapiz con B2: un `Clear` de pantalla
    /// entera que la grafica resuelve con su borrado rapido (1,9 ms medidos
    /// a 3000 x 2000, contra los 20-194 ms de hornear la escena).
    pub fn encender_tinta(&self, motor: &MotorRender) -> Result<(), ErrorRender> {
        {
            let prestamo = self.tinta.borrow();
            match prestamo.as_ref() {
                None => return Ok(()),
                Some(capa) if capa.encendida.get() => return Ok(()),
                Some(_) => {}
            }
        }
        self.pintar_capa(&self.tinta, motor, None, |_, _| {})?;
        if let Some(capa) = self.tinta.borrow().as_ref() {
            capa.encendida.set(true);
        }
        Ok(())
    }

    /// Apaga la capa de la tinta: la vacia entera.
    ///
    /// Se llama al soltar, cuando el trazo ya ha pasado al motor 2D y lo va
    /// a pintar la escena; si no se vaciara, el trazo se veria dos veces.
    ///
    /// Vaciarla y no quitarle el contenido al visual: quitarlo ahorraria a
    /// DWM la mezcla de una capa transparente de pantalla entera, pero un
    /// visual al que se le pone la superficie DESPUES de estar en el arbol
    /// no ensena nada -medido con la prueba de aqui abajo, que es
    /// exactamente el fallo por el que el trazo en curso no se veia-.
    pub fn apagar_tinta(&self, motor: &MotorRender) {
        let apagar = {
            let prestamo = self.tinta.borrow();
            prestamo.as_ref().is_some_and(|c| c.encendida.get())
        };
        if !apagar {
            return;
        }
        // Si se uso para arrastrar una seleccion, vuelve a su sitio: la
        // proxima vez que se encienda tiene que pintar donde se le diga.
        self.desplazar_tinta(0.0, 0.0);
        let _ = self.pintar_capa(&self.tinta, motor, None, |_, _| {});
        if let Some(capa) = self.tinta.borrow().as_ref() {
            capa.encendida.set(false);
        }
    }

    /// **A3 fase 2: el cielo y las estrellas, DEBAJO de la escena.**
    ///
    /// Hasta ahora el universo no podia componerse: su cielo, sus estrellas
    /// con paralaje y sus astros se pintan todos en pixeles de pantalla y en
    /// la misma superficie, asi que correr la superficie entera los correria
    /// a todos al mismo paso, y el paralaje es justamente que no van al
    /// mismo paso. Con esto, cada cosa va en su visual:
    ///
    /// - el **cielo** (el degradado y las nebulosas) no se mueve: es fondo de
    ///   pantalla. Su superficie es chica -el degradado se hornea a 192 px de
    ///   ancho y se estira- y quien la estira es la composicion, asi que son
    ///   unos cientos de kilobytes y no los 24 MB de una capa de pantalla
    ///   entera;
    /// - las **estrellas** se mueven a SU paralaje (`desplazar_fondo`), con
    ///   su propio colchon;
    /// - la **escena** con los astros sigue moviendose al paso del lienzo.
    ///
    /// Solo lo monta el universo, y solo si la superficie tiene capas: un
    /// lienzo normal no paga ni un visual de mas.
    pub fn montar_fondo(
        &self,
        ancho_cielo: u32,
        alto_cielo: u32,
        ancho: u32,
        alto: u32,
        margen_fondo: u32,
    ) -> Result<(), ErrorRender> {
        if self.margen == 0 || self.fondo.borrow().is_some() {
            return Ok(());
        }
        let (ancho_fondo, alto_fondo) = (ancho + margen_fondo * 2, alto + margen_fondo * 2);
        // SAFETY: el dispositivo y la raiz son propios y siguen vivos. Estas
        // dos van DEBAJO de la escena, o sea al PRINCIPIO de la lista de
        // hijos (`AddVisual(v, true, None)`), que es el otro extremo del que
        // usa `crear` para las de encima.
        let (cielo, fondo) = unsafe {
            let nueva = |w: u32, h: u32| -> Result<CapaDComp, ErrorRender> {
                let sup = self.dcomp.CreateSurface(
                    w.max(1),
                    h.max(1),
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                )?;
                let v = self.dcomp.CreateVisual()?;
                v.SetContent(&sup)?;
                self._raiz.AddVisual(&v, true, None)?;
                Ok(CapaDComp {
                    visual: v,
                    superficie: sup,
                    tamano: (w.max(1), h.max(1)),
                    encendida: std::cell::Cell::new(true),
                })
            };
            // Cada una se mete delante de la anterior POR EL PRINCIPIO, asi
            // que primero las estrellas (justo debajo de la escena) y luego
            // el cielo, que queda debajo de las estrellas: el orden en que
            // se ven es cielo, estrellas, escena.
            let fondo = nueva(ancho_fondo, alto_fondo)?;
            let cielo = nueva(ancho_cielo, alto_cielo)?;
            // El cielo se hornea chico y lo estira la composicion, con el
            // filtro lineal: es un degradado, no hay detalle que perder.
            cielo.visual.SetTransform2(&windows_numerics::Matrix3x2 {
                M11: ancho as f32 / ancho_cielo.max(1) as f32,
                M12: 0.0,
                M21: 0.0,
                M22: alto as f32 / alto_cielo.max(1) as f32,
                M31: 0.0,
                M32: 0.0,
            })?;
            cielo
                .visual
                .SetBitmapInterpolationMode(DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR)?;
            (cielo, fondo)
        };
        *self.cielo.borrow_mut() = Some(cielo);
        *self.fondo.borrow_mut() = Some(fondo);
        self.margen_fondo.set(margen_fondo);
        // El colchon de las estrellas se pone por la misma matriz que su
        // paneo, como el de la escena (ver `aplicar_transformada`).
        self.aplicar_transformada_fondo();
        Ok(())
    }

    /// Si el cielo y las estrellas viven en sus propios visuales.
    pub fn tiene_fondo(&self) -> bool {
        self.fondo.borrow().is_some()
    }

    /// Pixeles de colchon por lado de la capa de las estrellas.
    pub fn margen_fondo(&self) -> f32 {
        self.margen_fondo.get() as f32
    }

    /// Lo que mide la superficie del cielo, que NO es la ventana: se hornea
    /// chica y la estira la composicion.
    pub fn tamano_cielo(&self) -> Option<(u32, u32)> {
        self.cielo.borrow().as_ref().map(|c| c.tamano)
    }

    /// Repinta el cielo. Solo hace falta al abrir y al cambiar de tamano la
    /// ventana: no se mueve ni cambia con la camara.
    pub fn pintar_cielo(
        &self,
        motor: &MotorRender,
        dibuja: impl FnOnce(&crate::lienzo::Pintor<'_>, (f32, f32)),
    ) -> Result<(), ErrorRender> {
        self.pintar_capa(&self.cielo, motor, None, dibuja)
    }

    /// Repinta las estrellas. Entera y no por trozos: son unas pocas miles
    /// de teselas ya realizadas, y partirlas costaria mas que rehacerlas.
    pub fn pintar_fondo(
        &self,
        motor: &MotorRender,
        dibuja: impl FnOnce(&crate::lienzo::Pintor<'_>, (f32, f32)),
    ) -> Result<(), ErrorRender> {
        self.pintar_capa(&self.fondo, motor, None, dibuja)
    }

    /// Corre la capa de las estrellas `dx`/`dy` pixeles de ventana. Quien
    /// llama pasa ya el paralaje aplicado: la superficie no sabe de cielos.
    pub fn desplazar_fondo(&self, dx: f32, dy: f32) {
        if self.fondo.borrow().is_none() || self.desplazamiento_fondo.get() == (dx, dy) {
            return;
        }
        self.desplazamiento_fondo.set((dx, dy));
        self.aplicar_transformada_fondo();
    }

    /// Si un desplazamiento de las estrellas sigue cubierto por su colchon.
    pub fn desplazamiento_fondo_valido(&self, dx: f32, dy: f32) -> bool {
        let m = self.margen_fondo.get() as f32;
        self.fondo.borrow().is_some() && dx.abs() <= m && dy.abs() <= m
    }

    pub fn reponer_fondo(&self) {
        self.desplazar_fondo(0.0, 0.0);
    }

    fn aplicar_transformada_fondo(&self) {
        let prestamo = self.fondo.borrow();
        let Some(capa) = prestamo.as_ref() else {
            return;
        };
        let (dx, dy) = self.desplazamiento_fondo.get();
        let m = self.margen_fondo.get() as f32;
        let t = windows_numerics::Matrix3x2 {
            M11: 1.0,
            M12: 0.0,
            M21: 0.0,
            M22: 1.0,
            M31: dx - m,
            M32: dy - m,
        };
        // SAFETY: la matriz vive durante la llamada; el visual y el
        // dispositivo son propios y siguen vivos. Un error solo significa
        // que el fotograma sale sin mover las estrellas.
        unsafe {
            let _ = capa.visual.SetTransform2(&t);
            let _ = self.dcomp.Commit();
        }
    }

    /// Corre la capa de la tinta `dx`/`dy` pixeles de ventana, sin repintarla.
    ///
    /// Es lo que hace fluido arrastrar una seleccion: lo elegido se pinta
    /// UNA vez en esta capa al empezar, y cada aviso del raton despues es
    /// esta matriz y un `Commit`. Excalidraw consigue lo mismo copiando el
    /// bitmap que guarda de cada elemento; aqui ni se copia: lo mueve la
    /// composicion. `apagar_tinta` la devuelve a su sitio.
    pub fn desplazar_tinta(&self, dx: f32, dy: f32) {
        let prestamo = self.tinta.borrow();
        let Some(capa) = prestamo.as_ref() else {
            return;
        };
        let t = windows_numerics::Matrix3x2 {
            M11: 1.0,
            M12: 0.0,
            M21: 0.0,
            M22: 1.0,
            M31: dx,
            M32: dy,
        };
        // SAFETY: la matriz vive durante la llamada; el visual y el
        // dispositivo son propios y siguen vivos. Un error solo significa
        // que este fotograma sale sin mover la capa.
        unsafe {
            let _ = capa.visual.SetTransform2(&t);
            let _ = self.dcomp.Commit();
        }
    }

    /// Si la capa de la tinta esta puesta ahora mismo.
    pub fn tinta_encendida(&self) -> bool {
        self.tinta
            .borrow()
            .as_ref()
            .is_some_and(|c| c.encendida.get())
    }

    /// El trozo comun de las dos capas: abrir la superficie, limpiar lo que
    /// se va a repintar, dibujar y publicarlo.
    fn pintar_capa(
        &self,
        cual: &std::cell::RefCell<Option<CapaDComp>>,
        motor: &MotorRender,
        zona: Option<(i32, i32, i32, i32)>,
        dibuja: impl FnOnce(&crate::lienzo::Pintor<'_>, (f32, f32)),
    ) -> Result<(), ErrorRender> {
        use windows::Win32::Foundation::{POINT, RECT};
        let prestamo = cual.borrow();
        let Some(capa) = prestamo.as_ref() else {
            return Ok(());
        };
        let (w, h) = capa.tamano;
        // Una zona vacia tras recortar no es «no cambio nada», es «no se
        // sabe»: se actualiza la capa entera, como sin zona.
        let rect = zona
            .map(|(l, t, r, b)| RECT {
                left: l.clamp(0, w as i32),
                top: t.clamp(0, h as i32),
                right: r.clamp(0, w as i32),
                bottom: b.clamp(0, h as i32),
            })
            .filter(|r| r.right > r.left && r.bottom > r.top);
        let entera = RECT {
            left: 0,
            top: 0,
            right: w as i32,
            bottom: h as i32,
        };
        let usada = rect.unwrap_or(entera);
        let mut offset = POINT::default();
        // SAFETY: la superficie es propia y esta viva; `rect` y `offset` son
        // locales y viven toda la llamada. `BeginDraw` dice donde hay que
        // dibujar DENTRO de la textura: lo que esta en la capa en
        // `usada.left/top` cae en `offset`.
        let textura: ID3D11Texture2D = unsafe {
            capa.superficie.BeginDraw(
                rect.as_ref().map(|r| r as *const RECT),
                &mut offset as *mut POINT,
            )?
        };
        let destino = motor.destino_backbuffer(&textura)?;
        // De coordenadas de la CAPA a coordenadas de la textura.
        let d = (
            offset.x as f32 - usada.left as f32,
            offset.y as f32 - usada.top as f32,
        );
        let r = motor.dibujar(&destino, |p| {
            // El recorte va ANTES de limpiar y con la matriz en identidad:
            // `Clear` borra el destino ENTERO dentro del recorte, y el
            // destino puede ser un atlas compartido con otras superficies de
            // composicion. Sin el recorte, limpiar la capa podria borrar lo
            // que no es nuestro.
            p.empujar_recorte(crate::lienzo::RectF {
                x: offset.x as f32,
                y: offset.y as f32,
                ancho: (usada.right - usada.left) as f32,
                alto: (usada.bottom - usada.top) as f32,
            });
            p.limpiar_transparente();
            // Puesto ya: lo que pinta la interfaz sale en su sitio sin tener
            // que saber nada del atlas. Quien pone su propia vista (la
            // tinta, que pinta en coordenadas del mundo) suma `d` al origen,
            // que es lo mismo por otro camino.
            p.desplazar(d.0, d.1);
            dibuja(p, d);
            p.desplazar(0.0, 0.0);
            p.soltar_recorte();
        });
        // SAFETY: cierra el `BeginDraw` de arriba, pase lo que pase en el
        // dibujo: dejarlo abierto bloquearia la siguiente vuelta.
        unsafe {
            let _ = capa.superficie.EndDraw();
            let _ = self.dcomp.Commit();
        }
        r?;
        Ok(())
    }

    /// El tamano de la capa de interfaz, si la hay.
    pub fn tamano_interfaz(&self) -> Option<(u32, u32)> {
        self.interfaz.borrow().as_ref().map(|c| c.tamano)
    }

    /// Garantiza buffers de al menos `ancho` x `alto`. Si hay que crecer,
    /// crece con margen (un cuarto mas) para que el siguiente paso de un
    /// zoom no vuelva a reasignar. Encoger lo hace `compactar`, al acabar
    /// el gesto: durante el gesto, encoger cada fotograma es tan caro como
    /// crecer.
    pub fn asegurar(&self, ancho: u32, alto: u32) -> Result<(), ErrorRender> {
        let (aw, ah) = self.asignado.get();
        if ancho <= aw && alto <= ah {
            return Ok(());
        }
        let nw = if ancho > aw {
            (ancho + ancho / 4).min(16_384)
        } else {
            aw
        };
        let nh = if alto > ah {
            (alto + alto / 4).min(16_384)
        } else {
            ah
        };
        self.redimensionar(nw, nh)
    }

    /// Devuelve los buffers al tamano justo si estan muy sobrados (mas del
    /// doble en alguna dimension). Para el final de un gesto. Devuelve si
    /// los toco: entonces el contenido se perdio y hay que repintar.
    pub fn compactar(&self, ancho: u32, alto: u32) -> Result<bool, ErrorRender> {
        let (aw, ah) = self.asignado.get();
        if aw > ancho.max(1) * 2 || ah > alto.max(1) * 2 {
            self.redimensionar(ancho, alto)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lo que DirectComposition sabe del ritmo de DWM: cuanto falta para la
    /// siguiente composicion y cada cuanto compone.
    ///
    /// Es la pieza que le faltaba a `pixpin_tinta::ritmo::Planificador` para
    /// decidir a que hora empezar el fotograma (B1 de la investigacion del
    /// 2026-09-19): hoy se pinta nada mas dispararse la senal de la
    /// swapchain y el fotograma se queda esperando quieto los 10-13 ms que
    /// faltan para que DWM lo componga, con los puntos del lapiz
    /// envejeciendo dentro.
    ///
    /// Devuelve `None` si DirectComposition no lo sabe (lo dice devolviendo
    /// una frecuencia de 0, por ejemplo con la ventana tapada): el llamante
    /// se queda con el comportamiento de siempre, que es pintar ya.
    pub fn ritmo(&self) -> Option<RitmoComposicion> {
        // SAFETY: el dispositivo es propio y sigue vivo; la estructura de
        // salida es local y del tipo que pide la API.
        let e = unsafe { self.dcomp.GetFrameStatistics().ok()? };
        if e.timeFrequency <= 0 {
            return None;
        }
        let a_ms = |t: i64| (t as f64) * 1000.0 / (e.timeFrequency as f64);
        let periodo_ms = if e.currentCompositionRate.Numerator > 0 {
            (e.currentCompositionRate.Denominator as f32) * 1000.0
                / (e.currentCompositionRate.Numerator as f32)
        } else {
            return None;
        };
        // El plazo puede venir en el pasado si la consulta llega tarde: eso
        // no es «falta tiempo negativo», es «ya no da tiempo», o sea 0.
        let hasta_el_plazo_ms =
            (a_ms(e.nextEstimatedFrameTime) - a_ms(e.currentTime)).max(0.0) as f32;
        Some(RitmoComposicion {
            hasta_el_plazo_ms: hasta_el_plazo_ms.min(periodo_ms * 2.0),
            periodo_ms,
        })
    }

    /// El backbuffer ACTUAL envuelto como destino D2D. Hay que volver a
    /// llamarlo en cada fotograma: con flip, el backbuffer rota en cada
    /// present y el bitmap anterior queda apuntando al buffer equivocado.
    pub fn empezar(&self, motor: &MotorRender) -> Result<ID2D1Bitmap1, ErrorRender> {
        self.fallo_inyectado()?;
        // SAFETY: el indice 0 es siempre el backbuffer escribible actual.
        let textura: ID3D11Texture2D = unsafe { self.swapchain.GetBuffer(0)? };
        motor.destino_backbuffer(&textura)
    }

    /// **Iguala el mapa de atras con el que se esta viendo**, en `zona`
    /// (pixeles de la superficie; `None` = entero).
    ///
    /// La cadena de intercambio tiene dos mapas, y el que se pinta ahora
    /// lleva dentro lo de hace DOS presentes (D148). Para pintar SOLO un
    /// trozo encima de lo que ya se ve —el trazo que se acaba de soltar—
    /// sin repintar la escena entera, el mapa de atras tiene que llevar
    /// primero lo mismo que el de delante: si no, al presentar se veria lo
    /// de hace dos fotogramas en todo lo demas.
    ///
    /// Es una copia de la GPU, del mapa 1 (el ultimo presentado: con dos
    /// mapas es el unico otro que hay, y DXGI deja leerlo) al 0 (el que se
    /// pinta). Cuesta lo que mide la zona y no lo que haya dibujado: a
    /// pantalla entera son unos megas de copia, contra recorrer y pintar
    /// cada elemento visible.
    pub fn igualar_trasero(&self, zona: Option<(i32, i32, i32, i32)>) -> Result<(), ErrorRender> {
        use windows::Win32::Graphics::Direct3D11::D3D11_BOX;
        let (ancho, alto) = self.asignado.get();
        let caja = zona.map(|(l, t, r, b)| D3D11_BOX {
            left: l.clamp(0, ancho as i32) as u32,
            top: t.clamp(0, alto as i32) as u32,
            front: 0,
            right: r.clamp(0, ancho as i32) as u32,
            bottom: b.clamp(0, alto as i32) as u32,
            back: 1,
        });
        // Una zona que se queda vacia al recortar es que no hay nada que
        // igualar: no es «igualarlo todo».
        if caja.is_some_and(|c| c.right <= c.left || c.bottom <= c.top) {
            return Ok(());
        }
        // SAFETY: los dos mapas son de esta cadena y viven mientras ella;
        // el 1 se lee (DXGI lo da de solo lectura en el modelo flip) y el 0
        // se escribe. Se llama fuera de todo `BeginDraw`, en el hilo que
        // pinta, que es el unico que usa el contexto inmediato.
        unsafe {
            let atras: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let delante: ID3D11Texture2D = self.swapchain.GetBuffer(1)?;
            let contexto = atras.GetDevice()?.GetImmediateContext()?;
            match caja {
                Some(c) => contexto.CopySubresourceRegion(
                    &atras,
                    0,
                    c.left,
                    c.top,
                    0,
                    &delante,
                    0,
                    Some(&c),
                ),
                None => contexto.CopyResource(&atras, &delante),
            }
        }
        Ok(())
    }

    /// Cambia el tamano de los buffers sin recrear la composicion. Mucho
    /// mas barato que una `Superficie` nueva (el pin lo hace en cada paso
    /// de un arrastre) y sin el riesgo de que la ventana se quede con la
    /// superficie vieja si la nueva falla.
    pub fn redimensionar(&self, ancho: u32, alto: u32) -> Result<(), ErrorRender> {
        // SAFETY: ningun bitmap del backbuffer sigue vivo fuera de un
        // fotograma (`empezar` lo envuelve de nuevo en cada uno), que es la
        // condicion de ResizeBuffers.
        unsafe {
            self.swapchain.ResizeBuffers(
                0,
                ancho.max(1),
                alto.max(1),
                DXGI_FORMAT_UNKNOWN,
                self.banderas,
            )?
        };
        self.asignado.set((ancho.max(1), alto.max(1)));
        Ok(())
    }

    /// **La ventana cambio de tamano** (el lienzo en ventana, estirado por
    /// sus bordes o pasado a pantalla completa): la escena, con su colchon,
    /// y las capas de encima (la tinta y la interfaz), a `ancho` x `alto`
    /// de ventana, sin recrear la composicion. Lo pintado se pierde: quien
    /// llama repinta entero, con la tinta apagada.
    ///
    /// Las capas de encima estrenan visual Y superficie, y se cuelgan otra
    /// vez por el final en el mismo orden que en `crear`: ponerle una
    /// superficie nueva a un visual que ya esta en el arbol no ensenaba nada
    /// (ver `apagar_tinta`), y colgar con un visual de referencia tampoco
    /// (ver `crear`). Las de debajo (el cielo del universo) no se tocan:
    /// ninguna ventana que cambie de tamano las tiene.
    pub fn cambiar_tamano(&self, ancho: u32, alto: u32) -> Result<(), ErrorRender> {
        let (ancho, alto) = (ancho.max(1), alto.max(1));
        self.redimensionar(ancho + self.margen * 2, alto + self.margen * 2)?;
        let mismas = self
            .interfaz
            .borrow()
            .as_ref()
            .is_none_or(|c| c.tamano == (ancho, alto));
        if mismas {
            return Ok(());
        }
        // SAFETY: el dispositivo y la raiz son propios y siguen vivos; las
        // superficies viejas se sueltan al sustituirlas, fuera de todo
        // `BeginDraw` (cada pintado abre y cierra el suyo).
        unsafe {
            for cual in [&self.tinta, &self.interfaz] {
                if let Some(vieja) = cual.borrow().as_ref() {
                    self._raiz.RemoveVisual(&vieja.visual)?;
                }
            }
            for (cual, encendida) in [(&self.tinta, false), (&self.interfaz, true)] {
                let mut prestamo = cual.borrow_mut();
                if prestamo.is_none() {
                    continue;
                }
                let sup = self.dcomp.CreateSurface(
                    ancho,
                    alto,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                )?;
                let v = self.dcomp.CreateVisual()?;
                v.SetContent(&sup)?;
                self._raiz.AddVisual(&v, false, None)?;
                *prestamo = Some(CapaDComp {
                    visual: v,
                    superficie: sup,
                    tamano: (ancho, alto),
                    encendida: std::cell::Cell::new(encendida),
                });
            }
            self.dcomp.Commit()?;
        }
        Ok(())
    }

    /// **Rehace la superficie sobre otro dispositivo** (el de antes se
    /// perdio), en la misma ventana y con el mismo tamano, colchon y
    /// latencia. Lo pintado se pierde: quien llama repinta despues.
    ///
    /// Las capas de fondo (`montar_fondo`) no se rehacen: las monta quien
    /// las usa, y ninguna ventana que se rehaga asi las tiene.
    pub fn rehacer(&mut self, motor: &MotorRender, d3d: &ID3D11Device) -> Result<(), ErrorRender> {
        let (ancho, alto) = self.asignado.get();
        let margen = self.margen;
        let baja_latencia = self.senal.is_some();
        // Primero se suelta el objetivo viejo: DirectComposition no da un
        // segundo objetivo para una ventana que ya tiene uno.
        if let Some(objetivo) = self.objetivo.take() {
            // SAFETY: objetos propios; con el dispositivo perdido estas
            // llamadas pueden fallar, y da igual: solo se esta soltando.
            unsafe {
                let _ = objetivo.SetRoot(None);
                let _ = self.dcomp.Commit();
            }
            drop(objetivo);
        }
        let nueva = Self::crear(
            motor,
            d3d,
            self.hwnd,
            ancho.saturating_sub(margen * 2),
            alto.saturating_sub(margen * 2),
            baja_latencia,
            margen,
        )?;
        *self = nueva;
        Ok(())
    }

    /// El fallo que una prueba inyecto en este dispositivo, si hay
    /// (`perdida::inyectar_perdida`).
    fn fallo_inyectado(&self) -> Result<(), ErrorRender> {
        match crate::perdida::inyectado(self.d3d_crudo) {
            Some(hr) => Err(windows::core::Error::from(hr).into()),
            None => Ok(()),
        }
    }

    pub fn presentar(&self) -> Result<(), ErrorRender> {
        self.fallo_inyectado()?;
        // SAFETY: present sin espera de vsync (0,0): el bucle es dirigido
        // por eventos y no debe bloquear el hilo de interfaz.
        unsafe { self.swapchain.Present(0, Default::default()).ok()? };
        Ok(())
    }

    /// Presenta sincronizado con el refresco y, si se sabe, diciendo que
    /// rectangulo cambio (D125). Solo lo usan las ventanas de dibujo: el pin
    /// sigue con `presentar`, que no bloquea su hilo.
    ///
    /// La zona sucia no ahorra pintar (el fotograma se pinta entero sobre la
    /// capa congelada), ahorra componer: DWM solo recompone ese trozo.
    pub fn presentar_sincronizado(
        &self,
        sucio: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), ErrorRender> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Dxgi::{DXGI_PRESENT, DXGI_PRESENT_PARAMETERS};
        let (ancho, alto) = self.asignado.get();
        let mut rect = sucio.map(|(l, t, r, b)| RECT {
            left: l.clamp(0, ancho as i32),
            top: t.clamp(0, alto as i32),
            right: r.clamp(0, ancho as i32),
            bottom: b.clamp(0, alto as i32),
        });
        // Un rectangulo vacio tras recortar no es "nada cambio", es "no se
        // sabe": se presenta entero.
        if rect.is_some_and(|r| r.right <= r.left || r.bottom <= r.top) {
            rect = None;
        }
        self.fallo_inyectado()?;
        let parametros = DXGI_PRESENT_PARAMETERS {
            DirtyRectsCount: u32::from(rect.is_some()),
            pDirtyRects: rect
                .as_mut()
                .map_or(std::ptr::null_mut(), |r| r as *mut RECT),
            pScrollRect: std::ptr::null_mut(),
            pScrollOffset: std::ptr::null_mut(),
        };
        // SAFETY: `parametros` y el rectangulo viven hasta el final de la
        // llamada; la swapchain es de modelo flip, que admite zonas sucias.
        unsafe {
            self.swapchain
                .Present1(1, DXGI_PRESENT(0), &parametros)
                .ok()?
        };
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::{Color, MotorRender};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
    };
    use windows::core::w;

    fn d3d() -> ID3D11Device {
        let mut d = None;
        // SAFETY: igual que en motor.rs: salidas locales, constantes documentadas.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            )
            .expect("GPU real");
        }
        d.unwrap()
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn se_crea_se_dibuja_y_se_presenta_sin_error() {
        // Una ventana minima descartable. La clase "STATIC" del sistema
        // evita registrar una propia solo para el test.
        // SAFETY: CreateWindowExW con la clase del sistema no exige mas que
        // un modulo valido; la ventana se destruye al final del test.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("prueba"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                256,
                256,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            )
            .expect("ventana de prueba")
        };

        let d3d = d3d();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let superficie = Superficie::nueva(&motor, &d3d, hwnd, 256, 256)
            .expect("deberia crearse la superficie de composicion");

        let destino = superficie.empezar(&motor).unwrap();
        let c = motor.contexto();
        // SAFETY: protocolo SetTarget/BeginDraw/EndDraw sobre objetos vivos.
        unsafe {
            c.SetTarget(&destino);
            c.BeginDraw();
            c.Clear(Some(&Color::ACENTO.a_d2d()));
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }
        superficie.presentar().expect("present deberia funcionar");

        // Dos fotogramas seguidos: el swapchain rota los buffers y el
        // segundo present fallaria si empezar() no re-envolviera el
        // backbuffer actual. Es el caso negativo del diseno de un solo uso.
        let destino2 = superficie.empezar(&motor).unwrap();
        // SAFETY: igual que arriba.
        unsafe {
            c.SetTarget(&destino2);
            c.BeginDraw();
            c.Clear(Some(&Color::NEGRO.a_d2d()));
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }
        superficie.presentar().expect("el segundo present tambien");

        drop(superficie);
        // SAFETY: la ventana la creo este test y nadie mas la usa.
        unsafe { DestroyWindow(hwnd).unwrap() };
    }

    /// Lo que se ve de esa ventana EN LA PANTALLA, en BGRA y fila a fila.
    ///
    /// Se lee de la pantalla y no con `PrintWindow`: probado, a una ventana
    /// sin bits de redireccion (`WS_EX_NOREDIRECTIONBITMAP`) `PrintWindow`
    /// le devuelve negro aun con `PW_RENDERFULLCONTENT`, porque no hay
    /// superficie GDI que imprimir y lo unico que existe es el arbol de
    /// composicion. Leer la pantalla mide justo lo que se quiere medir: lo
    /// que DWM acabo poniendo delante del usuario.
    fn capturar_ventana(hwnd: HWND, ancho: i32, alto: i32) -> Vec<u8> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, ReleaseDC, SRCCOPY, SelectObject,
        };
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        let mut caja = RECT::default();
        // SAFETY: la ventana es del llamante y `caja` es local.
        unsafe { GetWindowRect(hwnd, &mut caja).expect("donde esta la ventana") };
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ancho,
                // Negativo: de arriba abajo, como se lee despues.
                biHeight: -alto,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        // SAFETY: todos los objetos GDI se crean aqui, se sueltan aqui, y
        // `bits` apunta a la seccion DIB que vive hasta el `DeleteObject`.
        unsafe {
            let pantalla = GetDC(None);
            let hdc = CreateCompatibleDC(Some(pantalla));
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .expect("seccion DIB");
            let viejo = SelectObject(hdc, bmp.into());
            let _ = BitBlt(
                hdc,
                0,
                0,
                ancho,
                alto,
                Some(pantalla),
                caja.left,
                caja.top,
                SRCCOPY,
            );
            let px =
                std::slice::from_raw_parts(bits as *const u8, (ancho * alto * 4) as usize).to_vec();
            SelectObject(hdc, viejo);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(hdc);
            ReleaseDC(None, pantalla);
            px
        }
    }

    /// Deja que DWM componga y vuelve a capturar hasta que `listo` diga que
    /// ya esta todo, o hasta que se acabe la paciencia.
    ///
    /// Esperar a «que algo deje de estar negro» no basta: los `Commit` de
    /// cada capa llegan a DWM en orden y la pantalla puede ensenar un
    /// fotograma con la escena y sin la ultima capa. Sin esta espera, la
    /// prueba fallaba por carrera y no por z.
    fn capturar_cuando_componga(hwnd: HWND, lado: i32, listo: impl Fn(&[u8]) -> bool) -> Vec<u8> {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
        };
        let mut px = Vec::new();
        for _ in 0..60 {
            // SAFETY: bombear la cola de esta ventana; `m` es local.
            unsafe {
                let mut m = MSG::default();
                while PeekMessageW(&mut m, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = DispatchMessageW(&m);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
            px = capturar_ventana(hwnd, lado, lado);
            if listo(&px) {
                break;
            }
        }
        px
    }

    /// **La prueba que faltaba: que la interfaz se VEA.**
    ///
    /// Las capas se creaban, se pintaban y no daban error, y aun asi la
    /// barra de herramientas no aparecia: el visual de la interfaz quedaba
    /// DEBAJO del de la escena, que se pinta opaca. Colgarlos con
    /// `AddVisual(..., true, None)` deja el orden a lo que signifique «el
    /// principio de la lista», y eso no esta documentado.
    ///
    /// Aqui se pinta la escena entera de rojo, un rectangulo azul en la capa
    /// de la interfaz y otro verde en la de la tinta, se captura lo que DWM
    /// compone de verdad y se mira que color gana en cada sitio.
    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn una_ventana_compuesta_ensena_la_interfaz_encima_de_la_escena() {
        // **Consciente de DPI, como la aplicacion.** Sin esto, en una
        // pantalla al 150 % Windows estira la ventana de 256 a 384 pixeles
        // fisicos, pero las capas miden 256 FISICOS: solo cubren los dos
        // tercios de arriba, y lo que se pinte mas abajo «no se ve». Por eso
        // se dio la capa de la tinta por invisible y se apago B2 (medido el
        // 2026-09-22: la misma banda verde, pintada en la capa de la
        // INTERFAZ a y=200, tampoco se veia; con esto se ven las dos).
        // SAFETY: cambia una propiedad del proceso de pruebas; no toca nada
        // que la prueba no controle.
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
        }
        use windows::Win32::UI::WindowsAndMessaging::{
            SW_SHOWNA, ShowWindow, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOPMOST,
        };
        const LADO: i32 = 256;
        let rojo = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let azul = Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        };
        let verde = Color {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };
        // SAFETY: clase del sistema y ventana propia, destruida al final.
        // Sin bits de redireccion, como el overlay de verdad: es la unica
        // forma de que lo que se capture sea el arbol de composicion. Y
        // siempre encima, porque lo que se lee es la PANTALLA: si otra
        // ventana la tapara se estaria midiendo esa otra.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST,
                w!("STATIC"),
                w!("prueba de capas"),
                WS_POPUP,
                0,
                0,
                LADO,
                LADO,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            )
            .expect("ventana de prueba")
        };
        // SAFETY: la ventana es nuestra; `SW_SHOWNA` no le roba el foco a
        // nadie, que en una prueba importa.
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNA);
        }

        let d3d = d3d();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let superficie =
            Superficie::nueva_con_capas(&motor, &d3d, hwnd, LADO as u32, LADO as u32, 64)
                .expect("la superficie con capas deberia crearse");
        let destino = superficie.empezar(&motor).unwrap();
        motor
            .dibujar(&destino, |p| p.limpiar(rojo))
            .expect("la escena se pinta");
        superficie
            .presentar_sincronizado(None)
            .expect("present de la escena");
        superficie
            .pintar_interfaz(&motor, |p, _| {
                p.rellenar(
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: LADO as f32,
                        alto: 32.0,
                    },
                    azul,
                );
            })
            .expect("la interfaz se pinta");
        // La capa de la tinta, entre las dos: una banda verde abajo.
        superficie
            .encender_tinta(&motor)
            .expect("la capa de tinta se enciende");
        superficie
            .pintar_tinta(&motor, None, |p, _| {
                p.rellenar(
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 200.0,
                        ancho: LADO as f32,
                        alto: 32.0,
                    },
                    verde,
                );
            })
            .expect("la tinta se pinta");

        // La seccion DIB viene en BGRA.
        let color_de = |px: &[u8], x: i32, y: i32| {
            let i = ((y * LADO + x) * 4) as usize;
            (px[i + 2], px[i + 1], px[i])
        };
        // Los tres sitios que se miran: el lienzo en medio, la barra arriba y
        // la tinta abajo.
        let px = capturar_cuando_componga(hwnd, LADO, |px| {
            let escena = color_de(px, 128, 128);
            let barra = color_de(px, 128, 16);
            let tinta = color_de(px, 128, 216);
            escena.0 > 150 && barra.2 > 150 && tinta.1 > 150
        });
        let color = |x: i32, y: i32| color_de(&px, x, y);
        let (r, g, b) = color(128, 128);
        assert!(
            r > 150 && g < 100 && b < 100,
            "en medio tiene que verse la escena, y se ve {:?}",
            (r, g, b)
        );
        let (r, g, b) = color(128, 16);
        assert!(
            b > 150 && r < 100,
            "la interfaz tiene que verse ENCIMA de la escena, y ahi se ve {:?}",
            (r, g, b)
        );
        let (r, g, b) = color(128, 216);
        assert!(
            g > 150 && r < 100,
            "la tinta tiene que verse encima de la escena, y ahi se ve {:?}",
            (r, g, b)
        );

        // Arrastrar una seleccion: la capa se corre SIN repintarla. La banda
        // verde tiene que subir 100 pixeles y dejar ver la escena debajo.
        superficie.desplazar_tinta(0.0, -100.0);
        let px = capturar_cuando_componga(hwnd, LADO, |px| {
            color_de(px, 128, 116).1 > 150 && color_de(px, 128, 216).0 > 150
        });
        let (r, g, _) = color_de(&px, 128, 116);
        assert!(g > 150 && r < 100, "la tinta no subio: {:?}", (r, g));
        let (r, g, _) = color_de(&px, 128, 216);
        assert!(
            r > 150 && g < 100,
            "donde estaba la tinta queda {:?}",
            (r, g)
        );

        // **Cambiar de tamano** (el lienzo en ventana): las capas nuevas
        // tienen que seguir en su orden, la interfaz encima de la tinta y
        // la tinta encima de la escena. Se encoge y se vuelve al tamano de
        // la ventana, para poder mirar los mismos pixeles. Como en el
        // editor, sin ningun mapa de la cadena vivo al cambiarla.
        drop(destino);
        superficie.apagar_tinta(&motor);
        superficie.cambiar_tamano(128, 96).expect("encoger");
        assert_eq!(superficie.tamano_interfaz(), Some((128, 96)));
        superficie
            .cambiar_tamano(LADO as u32, LADO as u32)
            .expect("crecer");
        assert_eq!(
            superficie.tamano_interfaz(),
            Some((LADO as u32, LADO as u32))
        );
        assert!(!superficie.tinta_encendida(), "la tinta vuelve apagada");
        let azul_claro = Color {
            r: 0.0,
            g: 0.5,
            b: 1.0,
            a: 1.0,
        };
        let destino = superficie.empezar(&motor).unwrap();
        motor
            .dibujar(&destino, |p| p.limpiar(rojo))
            .expect("la escena se pinta");
        superficie
            .presentar_sincronizado(None)
            .expect("present de la escena");
        superficie
            .pintar_interfaz(&motor, |p, _| {
                p.rellenar(
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: LADO as f32,
                        alto: 32.0,
                    },
                    azul_claro,
                );
            })
            .expect("la interfaz se pinta");
        superficie
            .encender_tinta(&motor)
            .expect("la capa de tinta se enciende");
        superficie
            .pintar_tinta(&motor, None, |p, _| {
                p.rellenar(
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 200.0,
                        ancho: LADO as f32,
                        alto: 32.0,
                    },
                    verde,
                );
            })
            .expect("la tinta se pinta");
        let px = capturar_cuando_componga(hwnd, LADO, |px| {
            color_de(px, 128, 16).1 > 100 && color_de(px, 128, 216).1 > 150
        });
        let (r, g, b) = color_de(&px, 128, 16);
        assert!(
            b > 150 && g > 100 && r < 100,
            "tras cambiar de tamano la interfaz sigue encima: {:?}",
            (r, g, b)
        );
        let (r, g, _) = color_de(&px, 128, 216);
        assert!(g > 150 && r < 100, "y la tinta tambien: {:?}", (r, g));
        let (r, g, _) = color_de(&px, 128, 128);
        assert!(r > 150 && g < 100, "y la escena debajo: {:?}", (r, g));
        drop(superficie);
        // SAFETY: la ventana la creo este test y nadie mas la usa.
        unsafe { DestroyWindow(hwnd).unwrap() };
    }

    /// A3 de punta a punta contra la GPU de verdad: la superficie con
    /// colchon y capa de interfaz se crea, se pinta la escena, se pinta la
    /// interfaz, se desplaza el visual y se estira, todo sin error.
    ///
    /// Es la unica forma de comprobar sin abrir la aplicacion que
    /// `CreateSurface`, `AddVisual`, `BeginDraw`/`EndDraw` y las matrices
    /// del visual funcionan en este equipo: las cuentas de
    /// `transformada_de_camara` se prueban sin GPU, pero que
    /// DirectComposition acepte el arbol de dos capas no se puede deducir.
    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn una_superficie_con_capas_se_crea_se_pinta_se_desplaza_y_se_estira() {
        // SAFETY: igual que en el test de arriba: clase del sistema, y la
        // ventana se destruye al final.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("prueba capas"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                256,
                256,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            )
            .expect("ventana de prueba")
        };
        let d3d = d3d();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let superficie = Superficie::nueva_con_capas(&motor, &d3d, hwnd, 256, 256, 64)
            .expect("la superficie con capas deberia crearse");
        assert!(superficie.tiene_capas());
        assert_eq!(superficie.margen(), 64.0);
        // La superficie de escena lleva el colchon por cada lado; la de la
        // interfaz, no: es del tamano de la ventana.
        assert_eq!(superficie.tamano_interfaz(), Some((256, 256)));

        let destino = superficie.empezar(&motor).unwrap();
        motor
            .dibujar(&destino, |p| p.limpiar(Color::ACENTO))
            .expect("la escena se pinta");
        let mut pintada = false;
        superficie
            .pintar_interfaz(&motor, |p, _base| {
                p.rellenar(
                    crate::lienzo::RectF {
                        x: 0.0,
                        y: 0.0,
                        ancho: 64.0,
                        alto: 16.0,
                    },
                    Color::NEGRO,
                );
                pintada = true;
            })
            .expect("la interfaz se pinta");
        assert!(pintada, "el cierre de la interfaz tiene que ejecutarse");
        superficie
            .presentar_sincronizado(None)
            .expect("present de la escena");

        // B2: la capa de la tinta nace apagada, se enciende al apoyar el
        // lapiz, se pinta por trozos y se apaga al soltar. Que
        // DirectComposition acepte un `BeginDraw` con rectangulo sobre una
        // tercera superficie no se puede deducir sin GPU.
        assert!(!superficie.tinta_encendida(), "apagada hasta que se traza");
        superficie
            .encender_tinta(&motor)
            .expect("la capa de tinta se enciende");
        assert!(superficie.tinta_encendida());
        let mut trazada = false;
        superficie
            .pintar_tinta(&motor, Some((10, 10, 90, 90)), |p, _base| {
                p.rellenar(
                    crate::lienzo::RectF {
                        x: 20.0,
                        y: 20.0,
                        ancho: 30.0,
                        alto: 30.0,
                    },
                    Color::NEGRO,
                );
                trazada = true;
            })
            .expect("el trozo de tinta se pinta");
        assert!(trazada, "el cierre de la tinta tiene que ejecutarse");
        // Caso negativo: una zona vacia tras recortar no deja la capa sin
        // actualizar, actualiza la capa entera.
        superficie
            .pintar_tinta(&motor, Some((50, 50, 10, 10)), |_, _| {})
            .expect("una zona vacia no revienta");
        superficie.apagar_tinta(&motor);
        assert!(!superficie.tinta_encendida());

        // Y lo que hace A3 en cada fotograma de paneo: mover el visual.
        superficie.desplazar_escena(30.0, -12.0);
        assert_eq!(superficie.desplazamiento(), (30.0, -12.0));
        assert!(superficie.desplazamiento_valido(30.0, -12.0));
        // Caso negativo: pasado el colchon ya no vale, y quien llama tiene
        // que repintar en vez de ensenar el borde vacio.
        assert!(!superficie.desplazamiento_valido(65.0, 0.0));
        superficie.estirar(1.2, 1.2, -10.0, -10.0);
        assert!(superficie.esta_estirada());
        superficie.dejar_de_estirar();
        superficie.reponer_escena();
        assert_eq!(superficie.desplazamiento(), (0.0, 0.0));

        drop(superficie);
        // SAFETY: la ventana la creo este test y nadie mas la usa.
        unsafe { DestroyWindow(hwnd).unwrap() };
    }

    /// El pixel (x, y) del mapa que se pinta ahora (el 0), en BGRA.
    fn pixel_de_atras(s: &Superficie, d3d: &ID3D11Device, x: u32, y: u32) -> [u8; 4] {
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
            D3D11_USAGE_STAGING,
        };
        // SAFETY: los objetos son de este test y estan vivos; la textura de
        // lectura se crea, se copia, se mapea y se desmapea aqui mismo.
        unsafe {
            let atras: ID3D11Texture2D = s.swapchain.GetBuffer(0).unwrap();
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            atras.GetDesc(&mut desc);
            let desc = D3D11_TEXTURE2D_DESC {
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
                ..desc
            };
            let mut copia = None;
            d3d.CreateTexture2D(&desc, None, Some(&mut copia)).unwrap();
            let copia = copia.unwrap();
            let ctx = d3d.GetImmediateContext().unwrap();
            ctx.CopyResource(&copia, &atras);
            let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
            ctx.Map(&copia, 0, D3D11_MAP_READ, 0, Some(&mut mapa))
                .unwrap();
            let p = (mapa.pData as *const u8).add((y * mapa.RowPitch + x * 4) as usize);
            let px = [*p, *p.add(1), *p.add(2), *p.add(3)];
            ctx.Unmap(&copia, 0);
            px
        }
    }

    const ROJO: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    const AZUL: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Pinta todo el mapa de atras de un color y lo presenta.
    fn presentar_de_color(s: &Superficie, motor: &MotorRender, color: Color) {
        let destino = s.empezar(motor).unwrap();
        motor.dibujar(&destino, |p| p.limpiar(color)).unwrap();
        s.presentar_sincronizado(None).unwrap();
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn igualar_el_trasero_copia_lo_que_se_esta_viendo_y_solo_en_su_zona() {
        // Lo que sostiene soltar el lapiz sin repintar la escena: antes de
        // pintar el trazo nuevo encima, el mapa de atras (que lleva lo de
        // hace DOS presentes) tiene que llevar lo que se ve ahora. La
        // ventana no se ensena nunca: sin `WS_VISIBLE` no sale en pantalla.
        // SAFETY: igual que en la prueba de arriba.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("prueba"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                64,
                64,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            )
            .expect("ventana de prueba")
        };
        let d3d = d3d();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let s = Superficie::nueva(&motor, &d3d, hwnd, 64, 64).unwrap();
        // Rojo y luego azul: se ve azul, y atras queda el rojo.
        presentar_de_color(&s, &motor, ROJO);
        presentar_de_color(&s, &motor, AZUL);
        let rojo = pixel_de_atras(&s, &d3d, 5, 5);
        assert!(
            rojo[2] > 200 && rojo[0] < 50,
            "atras deberia quedar el rojo: {rojo:?}"
        );

        // Caso negativo primero: igualar solo un trozo deja el resto como
        // estaba. Es lo que hace barata la copia al soltar trazo tras trazo.
        s.igualar_trasero(Some((0, 0, 10, 10))).unwrap();
        let dentro = pixel_de_atras(&s, &d3d, 5, 5);
        let fuera = pixel_de_atras(&s, &d3d, 40, 40);
        assert!(
            dentro[0] > 200 && dentro[2] < 50,
            "la zona ya es azul: {dentro:?}"
        );
        assert!(
            fuera[2] > 200 && fuera[0] < 50,
            "fuera de la zona sigue el rojo: {fuera:?}"
        );

        // Y entero, todo.
        s.igualar_trasero(None).unwrap();
        let fuera = pixel_de_atras(&s, &d3d, 40, 40);
        assert!(
            fuera[0] > 200 && fuera[2] < 50,
            "entero, todo azul: {fuera:?}"
        );

        drop(s);
        // SAFETY: la ventana la creo este test y nadie mas la usa.
        unsafe { DestroyWindow(hwnd).unwrap() };
    }
}
