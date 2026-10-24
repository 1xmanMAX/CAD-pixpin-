//! **El 3D en la tarjeta**: el modelo BIM con profundidad, luz y sus aristas
//! vivas encima, y lo que va plano (letreros) despues. Tambien sabe que
//! elemento hay bajo un pixel y en que punto del mundo (para girar alrededor
//! de lo que se toca y acercarse hacia ahi).
//!
//! Los sombreadores se compilan la primera vez que se usan: el visor de
//! planos no paga nada.
//!
//! Este modulo habla con Direct3D: cada `unsafe` lleva su `// SAFETY:`.

use std::ffi::c_void;

use windows::Win32::Graphics::Direct3D::{
    D3D_PRIMITIVE_TOPOLOGY_LINELIST, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::s;

use crate::gpu::{Gpu, PintarEncima, PlanoGpu, Vista, compilar_de, elemento};
use crate::modelo3d::Modelo3d;

const SOMBREADORES_3D: &str = r#"
cbuffer Vista3d : register(b1) {
    row_major float4x4 mvp;
    float4 luz;      // hacia la luz (xyz)
    float4 ojo;      // la camara (xyz)
    float4 corte;    // x: 1 si hay caja de seccion, z: el radio de los puntos en pixeles
    float4 sec_min;  // la caja de seccion (xyz), relativa al origen
    float4 sec_max;
    uint elegido;    // elemento resaltado + 1 (0: ninguno)
    float claro;     // 1 fondo claro
    float2 pantalla; // ancho y alto en pixeles
    uint aislado;    // el unico elemento que se ve + 1 (0: todos)
    float3 relleno3;
};

struct S3 {
    float4 pos : SV_Position;
    float3 n : NORMAL;
    float4 c : COLOR;
    float3 w : MUNDO;
    nointerpolation uint e : ELEM;
    // Las marcas del croquis (bits 29 a 31 de la normal): 1 sin luz, 2 de
    // contraste (negro en fondo claro, blanco en oscuro), 4 tono de pluma. El BIM no las usa.
    nointerpolation uint f : MARCA;
};

float3 normal_de(uint n) {
    int3 v = int3(int(n << 24) >> 24, int(n << 16) >> 24, int(n << 8) >> 24);
    return float3(v) / 127.0;
}
float4 color_de(uint c) {
    return float4(c & 255, (c >> 8) & 255, (c >> 16) & 255, (c >> 24) & 255) / 255.0;
}

S3 vs_3d(float3 p : POS, uint n : NORMAL, uint c : COLOR, uint e : ELEM) {
    S3 o;
    o.pos = mul(float4(p, 1), mvp);
    o.n = normal_de(n);
    o.c = color_de(c);
    o.w = p;
    o.e = e;
    o.f = (n >> 29) & 7;
    return o;
}

float3 de_contraste(S3 i) {
    if ((i.f & 2) != 0) return claro > 0.5 ? float3(0, 0, 0) : float3(1, 1, 1);
    return i.c.rgb;
}

// **La caja de seccion** (la de Revit): lo de fuera no se ve.
void cortar(float3 w, uint e) {
    if (corte.x > 0.5 && (any(w < sec_min.xyz) || any(w > sec_max.xyz))) discard;
    // Aislado: solo ese elemento.
    if (aislado != 0 && e + 1 != aislado) discard;
}

// Donde el rayo del ojo a `w` entra en la caja (en fraccion del camino;
// <= 0 si el ojo ya esta dentro) y por que cara (su normal, hacia fuera).
float entrada(float3 w, out float3 cara) {
    float3 d = w - ojo.xyz;
    d = (abs(d) < 1e-9) ? 1e-9 : d;
    float3 t1 = (sec_min.xyz - ojo.xyz) / d;
    float3 t2 = (sec_max.xyz - ojo.xyz) / d;
    float3 tn = min(t1, t2);
    float t = max(tn.x, max(tn.y, tn.z));
    float3 eje = (tn.x >= t) ? float3(1, 0, 0) : ((tn.y >= t) ? float3(0, 1, 0) : float3(0, 0, 1));
    cara = -eje * sign(d);
    return t;
}

// **La tapa del corte**: con la caja puesta, lo que se ve de un solido por
// el hueco que abre una cara de la caja es su cara de dentro. Esa es la
// tapa: se pinta maciza, como la seccion. La cara se mira con su normal
// geometrica (vuelta hacia el ojo) contra la guardada: asi las normales
// suaves no enganan en los bordes.
bool es_tapa(S3 i) {
    if (corte.x < 0.5) return false;
    float3 g = cross(ddx(i.w), ddy(i.w));
    if (dot(g, ojo.xyz - i.w) < 0) g = -g;
    float3 cara;
    return dot(i.n, g) < 0 && entrada(i.w, cara) > 0;
}

// El punto de la tapa: donde el rayo entra en la caja.
float3 en_el_corte(float3 w) {
    float3 cara;
    float t = max(entrada(w, cara), 0);
    return ojo.xyz + (w - ojo.xyz) * t;
}

float4 sombrear(S3 i) {
    if ((i.f & 4) != 0) {
        // **El tono de pluma del croquis** (`Croquis3DPluma.kt`): el color
        // elegido en el medio, hacia el blanco donde da la luz y hacia el
        // negro en el flanco, un 42 % a cada lado. Una tinta clara sigue clara.
        float3 np = normalize(i.n);
        if (dot(np, normalize(ojo.xyz - i.w)) < 0) np = -np;
        float t = dot(np, luz.xyz) * 0.42;
        float3 cp = t > 0 ? lerp(i.c.rgb, float3(1, 1, 1), t) : i.c.rgb * (1 + t);
        if (i.e + 1 == elegido) cp = lerp(cp, float3(0.10, 0.55, 1.0), 0.55);
        return float4(cp, i.c.a);
    }
    if (i.f != 0) {
        float3 plano = de_contraste(i);
        if (i.e + 1 == elegido) plano = lerp(plano, float3(0.10, 0.55, 1.0), 0.55);
        return float4(plano, i.c.a);
    }
    float3 n = normalize(i.n);
    float3 v = normalize(ojo.xyz - i.w);
    if (dot(n, v) < 0) n = -n;   // las dos caras
    float dif = saturate(dot(n, luz.xyz));
    float cielo = 0.5 + 0.5 * n.z;
    float3 h = normalize(luz.xyz + v);
    float brillo = pow(saturate(dot(n, h)), 40.0) * 0.12;
    float3 col = i.c.rgb * (0.30 + 0.22 * cielo + 0.55 * dif) + brillo;
    if (i.e + 1 == elegido) col = lerp(col, float3(0.10, 0.55, 1.0), 0.55);
    return float4(col, i.c.a);
}

// Lo opaco sin las tapas (van despues, en `ps_tapa`).
float4 ps_3d(S3 i) : SV_Target {
    cortar(i.w, i.e);
    if (es_tapa(i)) discard;
    return sombrear(i);
}

// Los vidrios: las dos caras, como siempre.
float4 ps_vidrio(S3 i) : SV_Target {
    cortar(i.w, i.e);
    return sombrear(i);
}

// La tapa: plana, mirando arriba y algo mas oscura que el elemento, para
// que se lea como corte.
float4 ps_tapa(S3 i) : SV_Target {
    cortar(i.w, i.e);
    if (!es_tapa(i)) discard;
    float3 cara;
    entrada(i.w, cara);
    float dif = saturate(dot(cara, luz.xyz));
    float3 col = i.c.rgb * (0.40 + 0.12 * (0.5 + 0.5 * cara.z) + 0.55 * dif) * 0.8;
    if (i.e + 1 == elegido) col = lerp(col, float3(0.10, 0.55, 1.0), 0.55);
    return float4(col, 1);
}

// Un punto: 4 vertices en el mismo sitio; la esquina (byte alto de la
// normal) lo abre en pantalla a un tamano fijo.
S3 vs_punto(float3 p : POS, uint n : NORMAL, uint c : COLOR, uint e : ELEM) {
    S3 o = vs_3d(p, 0, c, e);
    uint k = (n >> 24) & 3;
    float2 d = float2((k & 1) ? 1.0 : -1.0, (k & 2) ? 1.0 : -1.0);
    o.pos.xy += d * corte.z * 2.0 / pantalla * o.pos.w;
    o.n = float3(d, 0);
    return o;
}

float4 ps_punto(S3 i) : SV_Target {
    cortar(i.w, i.e);
    float r = length(i.n.xy);
    if (r > 1.0) discard;
    float3 col = i.c.rgb;
    if (i.e + 1 == elegido) col = float3(0.10, 0.55, 1.0);
    if (r > 0.62) col *= 0.35;
    return float4(col, 1);
}

float4 ps_linea(S3 i) : SV_Target {
    cortar(i.w, i.e);
    if (i.e + 1 == elegido) return float4(0.10, 0.55, 1.0, 1);
    return float4(de_contraste(i), i.c.a);
}

float4 ps_arista(S3 i) : SV_Target {
    cortar(i.w, i.e);
    float3 base = i.c.rgb * 0.28;
    if (i.e + 1 == elegido) base = float3(0.0, 0.30, 0.75);
    return float4(base, claro > 0.5 ? 0.75 : 0.85);
}

struct Eleccion { uint id : SV_Target0; float4 mundo : SV_Target1; };
Eleccion ps_id(S3 i) {
    cortar(i.w, i.e);
    if (i.n.z == 0 && length(i.n.xy) > 1.05) discard;
    Eleccion o;
    o.id = i.e + 1;
    // En una tapa, el punto es el del corte: alli se gira y se acerca.
    // (Un punto no tiene cara: su normal geometrica es cero y no es tapa.)
    o.mundo = float4(es_tapa(i) ? en_el_corte(i.w) : i.w, 1);
    return o;
}
"#;

/// Lo que el sombreador 3D sabe de la vista.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Vista3d {
    pub mvp: [[f32; 4]; 4],
    pub luz: [f32; 4],
    pub ojo: [f32; 4],
    pub corte: [f32; 4],
    pub sec_min: [f32; 4],
    pub sec_max: [f32; 4],
    pub elegido: u32,
    pub claro: f32,
    pub pantalla: [f32; 2],
    pub aislado: u32,
    pub relleno: [f32; 3],
}

/// El modelo en la tarjeta.
pub struct ModeloGpu {
    vertices: Option<ID3D11Buffer>,
    opacos: Option<ID3D11Buffer>,
    transparentes: Option<ID3D11Buffer>,
    aristas: Option<ID3D11Buffer>,
    lineas: Option<ID3D11Buffer>,
    puntos: Option<ID3D11Buffer>,
    n_opacos: u32,
    n_transparentes: u32,
    n_aristas: u32,
    n_lineas: u32,
    n_puntos: u32,
}

pub(crate) struct Tres {
    vs: ID3D11VertexShader,
    vs_punto: ID3D11VertexShader,
    ps_punto: ID3D11PixelShader,
    ps_linea: ID3D11PixelShader,
    ps: ID3D11PixelShader,
    ps_vidrio: ID3D11PixelShader,
    ps_tapa: ID3D11PixelShader,
    ps_arista: ID3D11PixelShader,
    ps_id: ID3D11PixelShader,
    il: ID3D11InputLayout,
    cb: ID3D11Buffer,
    /// Escribe y prueba profundidad; solo prueba (vidrios y aristas).
    prof: ID3D11DepthStencilState,
    prof_leer: ID3D11DepthStencilState,
    /// Los solidos un poco hacia atras: asi las aristas quedan encima.
    raster_solido: ID3D11RasterizerState,
    raster_aristas: ID3D11RasterizerState,
    raster_id: ID3D11RasterizerState,
    profundidad: Option<ID3D11DepthStencilView>,
    eleccion: Option<Destinos>,
}

struct Destinos {
    id: (ID3D11Texture2D, ID3D11RenderTargetView),
    mundo: (ID3D11Texture2D, ID3D11RenderTargetView),
    prof: ID3D11DepthStencilView,
    copia_id: ID3D11Texture2D,
    copia_mundo: ID3D11Texture2D,
}

impl Tres {
    pub(crate) fn soltar_destinos(&mut self) {
        self.profundidad = None;
        self.eleccion = None;
    }
}

fn falta() -> windows::core::Error {
    windows::core::Error::from(windows::Win32::Foundation::E_FAIL)
}

fn bytes_de<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: tipos `Copy` de solo numeros de 4 bytes, sin relleno.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

impl Gpu {
    fn preparar_3d(&mut self) -> windows::core::Result<()> {
        if self.tres.is_some() {
            return Ok(());
        }
        let d = &self.dispositivo;
        let vs = compilar_de(SOMBREADORES_3D, s!("vs_3d"), s!("vs_5_0"))?;
        let ps = compilar_de(SOMBREADORES_3D, s!("ps_3d"), s!("ps_5_0"))?;
        let pa = compilar_de(SOMBREADORES_3D, s!("ps_arista"), s!("ps_5_0"))?;
        let pi = compilar_de(SOMBREADORES_3D, s!("ps_id"), s!("ps_5_0"))?;
        let vp = compilar_de(SOMBREADORES_3D, s!("vs_punto"), s!("vs_5_0"))?;
        let pp = compilar_de(SOMBREADORES_3D, s!("ps_punto"), s!("ps_5_0"))?;
        let pl = compilar_de(SOMBREADORES_3D, s!("ps_linea"), s!("ps_5_0"))?;
        let pv = compilar_de(SOMBREADORES_3D, s!("ps_vidrio"), s!("ps_5_0"))?;
        let pt = compilar_de(SOMBREADORES_3D, s!("ps_tapa"), s!("ps_5_0"))?;
        // SAFETY: llamadas sobre el dispositivo propio; descripciones locales.
        unsafe {
            let (mut v, mut p, mut p2, mut p3, mut il, mut cb) =
                (None, None, None, None, None, None);
            d.CreateVertexShader(&vs, None, Some(&mut v))?;
            d.CreatePixelShader(&ps, None, Some(&mut p))?;
            d.CreatePixelShader(&pa, None, Some(&mut p2))?;
            d.CreatePixelShader(&pi, None, Some(&mut p3))?;
            let (mut v2, mut p4, mut p5) = (None, None, None);
            d.CreateVertexShader(&vp, None, Some(&mut v2))?;
            d.CreatePixelShader(&pp, None, Some(&mut p4))?;
            d.CreatePixelShader(&pl, None, Some(&mut p5))?;
            let (mut p6, mut p7) = (None, None);
            d.CreatePixelShader(&pv, None, Some(&mut p6))?;
            d.CreatePixelShader(&pt, None, Some(&mut p7))?;
            d.CreateInputLayout(
                &[
                    elemento(s!("POS"), DXGI_FORMAT_R32G32B32_FLOAT, 0, false),
                    elemento(s!("NORMAL"), DXGI_FORMAT_R32_UINT, 12, false),
                    elemento(s!("COLOR"), DXGI_FORMAT_R32_UINT, 16, false),
                    elemento(s!("ELEM"), DXGI_FORMAT_R32_UINT, 20, false),
                ],
                &vs,
                Some(&mut il),
            )?;
            d.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: std::mem::size_of::<Vista3d>() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut cb),
            )?;
            let prof_desc = |escribe: bool| D3D11_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: if escribe {
                    D3D11_DEPTH_WRITE_MASK_ALL
                } else {
                    D3D11_DEPTH_WRITE_MASK_ZERO
                },
                DepthFunc: D3D11_COMPARISON_LESS_EQUAL,
                ..Default::default()
            };
            let (mut pr, mut pl) = (None, None);
            d.CreateDepthStencilState(&prof_desc(true), Some(&mut pr))?;
            d.CreateDepthStencilState(&prof_desc(false), Some(&mut pl))?;
            let raster = |sesgo: i32, pendiente: f32, msaa: bool| D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthBias: sesgo,
                SlopeScaledDepthBias: pendiente,
                DepthClipEnable: true.into(),
                MultisampleEnable: msaa.into(),
                AntialiasedLineEnable: (!msaa).into(),
                ..Default::default()
            };
            let msaa = self.muestras > 1;
            let (mut rs, mut ra, mut ri) = (None, None, None);
            d.CreateRasterizerState(&raster(800, 1.5, msaa), Some(&mut rs))?;
            d.CreateRasterizerState(&raster(0, 0.0, msaa), Some(&mut ra))?;
            d.CreateRasterizerState(&raster(0, 0.0, false), Some(&mut ri))?;
            self.tres = Some(Tres {
                vs: v.ok_or_else(falta)?,
                vs_punto: v2.ok_or_else(falta)?,
                ps_punto: p4.ok_or_else(falta)?,
                ps_linea: p5.ok_or_else(falta)?,
                ps: p.ok_or_else(falta)?,
                ps_vidrio: p6.ok_or_else(falta)?,
                ps_tapa: p7.ok_or_else(falta)?,
                ps_arista: p2.ok_or_else(falta)?,
                ps_id: p3.ok_or_else(falta)?,
                il: il.ok_or_else(falta)?,
                cb: cb.ok_or_else(falta)?,
                prof: pr.ok_or_else(falta)?,
                prof_leer: pl.ok_or_else(falta)?,
                raster_solido: rs.ok_or_else(falta)?,
                raster_aristas: ra.ok_or_else(falta)?,
                raster_id: ri.ok_or_else(falta)?,
                profundidad: None,
                eleccion: None,
            });
        }
        Ok(())
    }

    fn textura(
        &self,
        formato: DXGI_FORMAT,
        muestras: u32,
        bind: D3D11_BIND_FLAG,
        ancho: u32,
        alto: u32,
    ) -> windows::core::Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: formato,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: muestras,
                Quality: 0,
            },
            Usage: if bind.0 == 0 {
                D3D11_USAGE_STAGING
            } else {
                D3D11_USAGE_DEFAULT
            },
            BindFlags: bind.0 as u32,
            CPUAccessFlags: if bind.0 == 0 {
                D3D11_CPU_ACCESS_READ.0 as u32
            } else {
                0
            },
            ..Default::default()
        };
        let mut t = None;
        // SAFETY: descripcion local; dispositivo propio.
        unsafe {
            self.dispositivo
                .CreateTexture2D(&desc, None, Some(&mut t))?
        };
        t.ok_or_else(falta)
    }

    fn vista_profundidad(
        &self,
        t: &ID3D11Texture2D,
    ) -> windows::core::Result<ID3D11DepthStencilView> {
        let mut v = None;
        // SAFETY: textura propia con BIND_DEPTH_STENCIL.
        unsafe {
            self.dispositivo
                .CreateDepthStencilView(t, None, Some(&mut v))?
        };
        v.ok_or_else(falta)
    }

    fn vista_destino(&self, t: &ID3D11Texture2D) -> windows::core::Result<ID3D11RenderTargetView> {
        let mut v = None;
        // SAFETY: textura propia con BIND_RENDER_TARGET.
        unsafe {
            self.dispositivo
                .CreateRenderTargetView(t, None, Some(&mut v))?
        };
        v.ok_or_else(falta)
    }

    /// Sube el modelo 3D.
    pub fn subir_3d(&self, m: &Modelo3d) -> windows::core::Result<ModeloGpu> {
        let vb: Vec<u32> = m
            .vertices
            .iter()
            .flat_map(|v| {
                [
                    v.pos[0].to_bits(),
                    v.pos[1].to_bits(),
                    v.pos[2].to_bits(),
                    v.normal,
                    v.color,
                    v.elemento,
                ]
            })
            .collect();
        Ok(ModeloGpu {
            vertices: self.buffer(bytes_de(&vb), D3D11_BIND_VERTEX_BUFFER, 0)?,
            opacos: self.buffer(bytes_de(&m.opacos), D3D11_BIND_INDEX_BUFFER, 0)?,
            transparentes: self.buffer(bytes_de(&m.transparentes), D3D11_BIND_INDEX_BUFFER, 0)?,
            aristas: self.buffer(bytes_de(&m.aristas), D3D11_BIND_INDEX_BUFFER, 0)?,
            lineas: self.buffer(bytes_de(&m.lineas), D3D11_BIND_INDEX_BUFFER, 0)?,
            puntos: self.buffer(bytes_de(&m.puntos), D3D11_BIND_INDEX_BUFFER, 0)?,
            n_lineas: m.lineas.len() as u32,
            n_puntos: m.puntos.len() as u32,
            n_opacos: m.opacos.len() as u32,
            n_transparentes: m.transparentes.len() as u32,
            n_aristas: m.aristas.len() as u32,
        })
    }

    /// Lo que se ve, de nuevo: solo los elementos para los que `ver` dice
    /// que si (los indices se rehacen; los vertices se quedan).
    pub fn filtrar_3d(
        &self,
        g: &mut ModeloGpu,
        m: &Modelo3d,
        ver: &dyn Fn(u32) -> bool,
    ) -> windows::core::Result<()> {
        let f = |l: &[u32], paso: usize| -> Vec<u32> {
            l.chunks_exact(paso)
                .filter(|t| {
                    m.vertices
                        .get(t[0] as usize)
                        .is_some_and(|v| ver(v.elemento))
                })
                .flatten()
                .copied()
                .collect()
        };
        let (o, t, a, l, p) = (
            f(&m.opacos, 3),
            f(&m.transparentes, 3),
            f(&m.aristas, 2),
            f(&m.lineas, 2),
            f(&m.puntos, 6),
        );
        g.opacos = self.buffer(bytes_de(&o), D3D11_BIND_INDEX_BUFFER, 0)?;
        g.transparentes = self.buffer(bytes_de(&t), D3D11_BIND_INDEX_BUFFER, 0)?;
        g.aristas = self.buffer(bytes_de(&a), D3D11_BIND_INDEX_BUFFER, 0)?;
        g.lineas = self.buffer(bytes_de(&l), D3D11_BIND_INDEX_BUFFER, 0)?;
        g.puntos = self.buffer(bytes_de(&p), D3D11_BIND_INDEX_BUFFER, 0)?;
        (
            g.n_opacos,
            g.n_transparentes,
            g.n_aristas,
            g.n_lineas,
            g.n_puntos,
        ) = (
            o.len() as u32,
            t.len() as u32,
            a.len() as u32,
            l.len() as u32,
            p.len() as u32,
        );
        Ok(())
    }

    fn poner_modelo(&self, t: &Tres, m: &ModeloGpu, v: &Vista3d) {
        let ctx = &self.contexto;
        // SAFETY: recursos propios; `v` vive durante la llamada.
        unsafe {
            ctx.UpdateSubresource(&t.cb, 0, None, v as *const Vista3d as *const c_void, 0, 0);
            ctx.VSSetConstantBuffers(1, Some(&[Some(t.cb.clone())]));
            ctx.PSSetConstantBuffers(1, Some(&[Some(t.cb.clone())]));
            ctx.IASetInputLayout(&t.il);
            ctx.IASetVertexBuffers(0, 1, Some(&m.vertices.clone()), Some(&24), Some(&0));
            ctx.VSSetShader(&t.vs, None);
        }
    }

    /// Dibuja el modelo y, encima, lo plano (`encima`, sin profundidad).
    pub fn dibujar_3d(
        &mut self,
        fondo: [f32; 4],
        modelo: Option<(&ModeloGpu, Vista3d)>,
        aristas: bool,
        encima: &[(&PlanoGpu, Vista)],
    ) -> windows::core::Result<()> {
        self.dibujar_3d_con(fondo, modelo, aristas, encima, None)
    }

    /// [`Gpu::dibujar_3d`], dejando a `tinta` pintar al final sobre el bufer
    /// de la ventana (lo anotado sobre un marcador, con el motor del lienzo).
    pub fn dibujar_3d_con(
        &mut self,
        fondo: [f32; 4],
        modelo: Option<(&ModeloGpu, Vista3d)>,
        aristas: bool,
        encima: &[(&PlanoGpu, Vista)],
        tinta: PintarEncima<'_>,
    ) -> windows::core::Result<()> {
        self.preparar_3d()?;
        let Some((tex, rtv)) = self.destino.clone() else {
            return Ok(());
        };
        if self.tres.as_ref().is_some_and(|t| t.profundidad.is_none()) {
            let t = self.textura(
                DXGI_FORMAT_D32_FLOAT,
                self.muestras,
                D3D11_BIND_DEPTH_STENCIL,
                self.ancho,
                self.alto,
            )?;
            let v = self.vista_profundidad(&t)?;
            if let Some(tr) = &mut self.tres {
                tr.profundidad = Some(v);
            }
        }
        let Some(t) = &self.tres else { return Ok(()) };
        let Some(dsv) = t.profundidad.clone() else {
            return Ok(());
        };
        let ctx = &self.contexto;
        // SAFETY: todo es de este dispositivo y de este hilo.
        unsafe {
            ctx.ClearRenderTargetView(&rtv, &fondo);
            ctx.ClearDepthStencilView(&dsv, D3D11_CLEAR_DEPTH.0, 1.0, 0);
            if let Some((m, v)) = modelo {
                ctx.OMSetRenderTargets(Some(&[Some(rtv.clone())]), &dsv);
                ctx.RSSetViewports(Some(&[self.ventana_entera()]));
                self.poner_modelo(t, m, &v);
                ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                // 1. Lo opaco.
                ctx.OMSetBlendState(None, None, 0xffff_ffff);
                ctx.OMSetDepthStencilState(&t.prof, 0);
                ctx.RSSetState(&t.raster_solido);
                ctx.PSSetShader(&t.ps, None);
                if let Some(ib) = &m.opacos {
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.DrawIndexed(m.n_opacos, 0, 0);
                }
                // 2. Las aristas, encima.
                if aristas && let Some(ib) = &m.aristas {
                    ctx.OMSetBlendState(&self.mezcla, None, 0xffff_ffff);
                    ctx.OMSetDepthStencilState(&t.prof_leer, 0);
                    ctx.RSSetState(&t.raster_aristas);
                    ctx.PSSetShader(&t.ps_arista, None);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINELIST);
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.DrawIndexed(m.n_aristas, 0, 0);
                }
                // 2a. Las tapas de la caja de seccion: despues de las aristas,
                // para que las de dentro del solido (las del fondo) queden tapadas.
                if v.corte[0] > 0.5
                    && let Some(ib) = &m.opacos
                {
                    ctx.OMSetBlendState(None, None, 0xffff_ffff);
                    ctx.OMSetDepthStencilState(&t.prof, 0);
                    ctx.RSSetState(&t.raster_solido);
                    ctx.PSSetShader(&t.ps_tapa, None);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.DrawIndexed(m.n_opacos, 0, 0);
                }
                // 2b. Las rayas sueltas y los puntos.
                if let Some(ib) = &m.lineas {
                    ctx.OMSetBlendState(&self.mezcla, None, 0xffff_ffff);
                    ctx.OMSetDepthStencilState(&t.prof, 0);
                    ctx.RSSetState(&t.raster_aristas);
                    ctx.PSSetShader(&t.ps_linea, None);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINELIST);
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.DrawIndexed(m.n_lineas, 0, 0);
                }
                if let Some(ib) = &m.puntos {
                    ctx.OMSetBlendState(None, None, 0xffff_ffff);
                    ctx.OMSetDepthStencilState(&t.prof, 0);
                    ctx.RSSetState(&t.raster_aristas);
                    ctx.VSSetShader(&t.vs_punto, None);
                    ctx.PSSetShader(&t.ps_punto, None);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.DrawIndexed(m.n_puntos, 0, 0);
                    ctx.VSSetShader(&t.vs, None);
                }
                // 3. Los vidrios, al final y sin tapar.
                if let Some(ib) = &m.transparentes {
                    ctx.OMSetBlendState(&self.mezcla, None, 0xffff_ffff);
                    ctx.OMSetDepthStencilState(&t.prof_leer, 0);
                    ctx.RSSetState(&t.raster_solido);
                    ctx.PSSetShader(&t.ps_vidrio, None);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.DrawIndexed(m.n_transparentes, 0, 0);
                }
            }
        }
        self.pasadas_2d(&rtv, encima);
        self.presentar_con(&tex, tinta)
    }

    /// Que elemento (y que punto del mundo, relativo al origen del modelo)
    /// hay bajo el pixel (x, y). Solo los opacos y los vidrios.
    pub fn elegir(
        &mut self,
        m: &ModeloGpu,
        v: &Vista3d,
        x: i32,
        y: i32,
    ) -> windows::core::Result<Option<(u32, [f32; 3])>> {
        if x < 0 || y < 0 || x as u32 >= self.ancho || y as u32 >= self.alto {
            return Ok(None);
        }
        self.preparar_3d()?;
        if self.tres.as_ref().is_some_and(|t| t.eleccion.is_none()) {
            let (w, h) = (self.ancho, self.alto);
            let ti = self.textura(DXGI_FORMAT_R32_UINT, 1, D3D11_BIND_RENDER_TARGET, w, h)?;
            let tm = self.textura(
                DXGI_FORMAT_R32G32B32A32_FLOAT,
                1,
                D3D11_BIND_RENDER_TARGET,
                w,
                h,
            )?;
            let tp = self.textura(DXGI_FORMAT_D32_FLOAT, 1, D3D11_BIND_DEPTH_STENCIL, w, h)?;
            let d = Destinos {
                id: (ti.clone(), self.vista_destino(&ti)?),
                mundo: (tm.clone(), self.vista_destino(&tm)?),
                prof: self.vista_profundidad(&tp)?,
                copia_id: self.textura(DXGI_FORMAT_R32_UINT, 1, D3D11_BIND_FLAG(0), 1, 1)?,
                copia_mundo: self.textura(
                    DXGI_FORMAT_R32G32B32A32_FLOAT,
                    1,
                    D3D11_BIND_FLAG(0),
                    1,
                    1,
                )?,
            };
            if let Some(t) = &mut self.tres {
                t.eleccion = Some(d);
            }
        }
        let Some(t) = &self.tres else { return Ok(None) };
        let Some(d) = &t.eleccion else {
            return Ok(None);
        };
        let ctx = &self.contexto;
        // SAFETY: recursos de este dispositivo; las lecturas mapean texturas
        // de 1x1 propias y se desmapean antes de salir.
        unsafe {
            ctx.ClearRenderTargetView(&d.id.1, &[0.0; 4]);
            ctx.ClearRenderTargetView(&d.mundo.1, &[0.0; 4]);
            ctx.ClearDepthStencilView(&d.prof, D3D11_CLEAR_DEPTH.0, 1.0, 0);
            ctx.OMSetRenderTargets(
                Some(&[Some(d.id.1.clone()), Some(d.mundo.1.clone())]),
                &d.prof,
            );
            ctx.RSSetViewports(Some(&[self.ventana_entera()]));
            ctx.RSSetState(&t.raster_id);
            ctx.OMSetBlendState(None, None, 0xffff_ffff);
            ctx.OMSetDepthStencilState(&t.prof, 0);
            // Solo ese pixel: el recorte ahorra todo lo demas.
            let mut sin_recorte = D3D11_RASTERIZER_DESC::default();
            t.raster_id.GetDesc(&mut sin_recorte);
            sin_recorte.ScissorEnable = true.into();
            let mut rr = None;
            self.dispositivo
                .CreateRasterizerState(&sin_recorte, Some(&mut rr))?;
            if let Some(rr) = &rr {
                ctx.RSSetState(rr);
            }
            ctx.RSSetScissorRects(Some(&[windows::Win32::Foundation::RECT {
                left: x,
                top: y,
                right: x + 1,
                bottom: y + 1,
            }]));
            self.poner_modelo(t, m, v);
            ctx.PSSetShader(&t.ps_id, None);
            ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            for (ib, n) in [
                (&m.opacos, m.n_opacos),
                (&m.transparentes, m.n_transparentes),
            ] {
                if let Some(ib) = ib {
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.DrawIndexed(n, 0, 0);
                }
            }
            if let Some(ib) = &m.puntos {
                ctx.VSSetShader(&t.vs_punto, None);
                ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                ctx.DrawIndexed(m.n_puntos, 0, 0);
            }
            let caja = D3D11_BOX {
                left: x as u32,
                top: y as u32,
                front: 0,
                right: x as u32 + 1,
                bottom: y as u32 + 1,
                back: 1,
            };
            ctx.CopySubresourceRegion(&d.copia_id, 0, 0, 0, 0, &d.id.0, 0, Some(&caja));
            ctx.CopySubresourceRegion(&d.copia_mundo, 0, 0, 0, 0, &d.mundo.0, 0, Some(&caja));
            ctx.OMSetRenderTargets(None, None);
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            ctx.Map(&d.copia_id, 0, D3D11_MAP_READ, 0, Some(&mut map))?;
            let id = *(map.pData as *const u32);
            ctx.Unmap(&d.copia_id, 0);
            if id == 0 {
                return Ok(None);
            }
            ctx.Map(&d.copia_mundo, 0, D3D11_MAP_READ, 0, Some(&mut map))?;
            let w = *(map.pData as *const [f32; 4]);
            ctx.Unmap(&d.copia_mundo, 0);
            Ok(Some((id - 1, [w[0], w[1], w[2]])))
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Los sombreadores 3D compilan (sin ventana ni tarjeta): con las marcas
    /// del croquis (sin luz, de contraste) no se puede romper el del BIM.
    #[test]
    fn los_sombreadores_3d_compilan() {
        for (punto, perfil) in [
            (s!("vs_3d"), s!("vs_5_0")),
            (s!("vs_punto"), s!("vs_5_0")),
            (s!("ps_3d"), s!("ps_5_0")),
            (s!("ps_arista"), s!("ps_5_0")),
            (s!("ps_id"), s!("ps_5_0")),
            (s!("ps_punto"), s!("ps_5_0")),
            (s!("ps_linea"), s!("ps_5_0")),
            (s!("ps_vidrio"), s!("ps_5_0")),
            (s!("ps_tapa"), s!("ps_5_0")),
        ] {
            assert!(compilar_de(SOMBREADORES_3D, punto, perfil).is_ok());
        }
        // Caso negativo: un punto de entrada que no existe no compila.
        assert!(compilar_de(SOMBREADORES_3D, s!("ps_no_esta"), s!("ps_5_0")).is_err());
    }
}
