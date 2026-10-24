//! **La tarjeta grafica** (Direct3D 11): el plano se sube una vez y cada
//! fotograma solo cambia la vista (un `cbuffer` de 48 bytes). Se dibujan
//! los tramos que caen en la ventana y que de tamano se verian; lo demas ni
//! se toca.
//!
//! Cuatro pasadas, todas con antialias (MSAA 4x si la tarjeta puede):
//! rellenos, sombreados con patron (sus rayas se calculan pixel a pixel),
//! rayas y letras (cada letra distinta esta una vez; las demas son
//! instancias).
//!
//! Este modulo habla con Direct3D: cada `unsafe` lleva su `// SAFETY:`.

use std::ffi::c_void;

use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
    D3D_PRIMITIVE_TOPOLOGY_LINELIST, D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
    D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, D3D_SRV_DIMENSION_BUFFEREX, ID3DBlob,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{PCSTR, s};

use crate::modelo::{Modelo, Tramo};

const SOMBREADORES: &str = r#"
cbuffer Vista : register(b0) {
    float2 escala;     // del plano a NDC
    float2 centro;     // el punto del plano en el centro de la ventana
    float4 color7;     // el color 7 de AutoCAD sobre este fondo
    float  px;         // unidades del plano por pixel
    float  modo;       // 0 tal cual, 1 fondo oscuro, 2 fondo claro
    float2 relleno_;
};

struct Sal { float4 pos : SV_Position; float4 color : COLOR; };

float4 color_de(uint c) {
    float4 r = float4(c & 255, (c >> 8) & 255, (c >> 16) & 255, (c >> 24) & 255) / 255.0;
    if (r.a == 0) r = color7;
    return r;
}

// Los colores del plano se pensaron para un fondo; en el otro, los que no
// se leen se aclaran (u oscurecen) sin perder su tono, y los grises oscuros
// se vuelven claros, como el color 7 de AutoCAD.
float luz(float3 c) { return dot(c, float3(0.2126, 0.7152, 0.0722)); }
float4 tinta(float4 c) {
    if (modo < 0.5) return c;
    float l = luz(c.rgb);
    float sat = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
    if (modo < 1.5) {
        if (sat < 0.15 && l < 0.5) c.rgb = 1.0 - c.rgb * 0.8;
        else if (l < 0.55) c.rgb = lerp(c.rgb, 1.0, (0.55 - l) / (1.0 - l));
    } else if (l > 0.62) {
        c.rgb *= 0.5 / l;
    }
    return c;
}
// Los rellenos grandes, en oscuro, se apagan: un blanco o un amarillo de
// tabla deslumbraba y las letras de encima (ya aclaradas) no se leian.
float4 apagado(float4 c) {
    if (modo < 0.5 || modo > 1.5) return c;
    float sat = max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
    if (sat < 0.15) c.rgb = 0.17 + (1.0 - c.rgb) * 0.22;
    else c.rgb *= 0.42;
    return c;
}

float4 a_pantalla(float2 p) { return float4((p - centro) * escala, 0.5, 1); }

Sal vs_simple(float2 p : POS, uint c : COLOR) {
    Sal o; o.pos = a_pantalla(p); o.color = color_de(c); return o;
}

float4 ps_color(Sal i) : SV_Target { return tinta(i.color); }
float4 ps_relleno(Sal i) : SV_Target { return apagado(i.color); }

// ---- sombreados con patron
struct Trama { float2 centro; float4 inv; float escala; uint desde; uint cuantas; };
struct Familia { float2 base; float2 dir; float paso; float corr; float largo; uint n; float trazos[8]; };
StructuredBuffer<Trama> tramas : register(t0);
StructuredBuffer<Familia> familias : register(t1);

struct SalT { float4 pos : SV_Position; float4 color : COLOR; float2 mundo : MUNDO; nointerpolation uint trama : TRAMA; };

SalT vs_trama(float2 p : POS, uint c : COLOR, uint t : TRAMA) {
    SalT o; o.pos = a_pantalla(p); o.color = color_de(c); o.mundo = p; o.trama = t; return o;
}

float4 ps_trama(SalT i) : SV_Target {
    Trama tr = tramas[i.trama];
    float2 d = i.mundo - tr.centro;
    float2 q = float2(tr.inv.x * d.x + tr.inv.y * d.y, tr.inv.z * d.x + tr.inv.w * d.y);
    float pxl = px / max(tr.escala, 1e-30);
    float cob = 0;
    [loop] for (uint k = 0; k < tr.cuantas && k < 32; k++) {
        Familia f = familias[tr.desde + k];
        float2 v = q - f.base;
        float2 n = float2(-f.dir.y, f.dir.x);
        float s = dot(v, n);
        float paso = abs(f.paso);
        // Rayas a menos de 3 pixeles: un tono, no un muaré.
        if (paso < pxl * 3.0) { cob = max(cob, 0.3); continue; }
        float kk = round(s / f.paso);
        float dist = abs(s - kk * f.paso) / pxl;
        float a = saturate(1.25 - dist);
        if (a <= 0) continue;
        if (f.n > 0 && f.largo > pxl) {
            float t = dot(v, f.dir) - kk * f.corr;
            float ph = t - floor(t / f.largo) * f.largo;
            float acc = 0; float dentro = 0;
            [loop] for (uint j = 0; j < f.n && j < 8; j++) {
                float L = f.trazos[j];
                float largo = max(abs(L), pxl);
                if (ph >= acc && ph < acc + largo) { dentro = L >= 0 ? 1 : 0; break; }
                acc += abs(L);
            }
            a *= dentro;
        }
        cob = max(cob, a);
    }
    if (cob <= 0.004) discard;
    float4 t = tinta(i.color);
    return float4(t.rgb, t.a * cob);
}

// ---- letras
StructuredBuffer<float2> malla : register(t2);
StructuredBuffer<uint2> glifos : register(t3);

Sal vs_letra(uint vid : SV_VertexID, float2 pos : POS, float4 m : MAT, uint c : COLOR, uint g : GLIFO) {
    uint2 gl = glifos[g];
    uint n = gl.y & 0x7fffffff;
    Sal o;
    o.color = color_de(c);
    if (vid >= n) { o.pos = float4(0, 0, -2, 1); return o; }
    float2 e = malla[gl.x + vid];
    float2 w = pos + float2(m.x * e.x + m.y * e.y, m.z * e.x + m.w * e.y);
    o.pos = a_pantalla(w);
    return o;
}

// ---- circulos y arcos, exactos a cualquier zoom: un cuadrado por arco y
// en cada pixel la distancia a la circunferencia (una raya de un pixel).
struct SalA {
    float4 pos : SV_Position;
    float4 color : COLOR;
    float2 mundo : MUNDO;
    nointerpolation float4 arco : ARCO;
    nointerpolation float barrido : BARRIDO;
};

SalA vs_arco(uint vid : SV_VertexID, float2 c : CENTRO, float r : RADIO, float a0 : INICIO, float b : BARRIDO, uint col : COLOR) {
    float2 esq[6] = { float2(-1, -1), float2(1, -1), float2(-1, 1), float2(1, -1), float2(1, 1), float2(-1, 1) };
    float2 w = c + esq[vid] * (r + 2.0 * px);
    SalA o;
    o.pos = a_pantalla(w);
    o.color = color_de(col);
    o.mundo = w;
    o.arco = float4(c, r, a0);
    o.barrido = b;
    return o;
}

float4 ps_arco(SalA i) : SV_Target {
    float2 d = i.mundo - i.arco.xy;
    float dist = abs(length(d) - i.arco.z) / px;
    float a = saturate(1.1 - dist);
    if (a <= 0.003) discard;
    if (i.barrido < 6.2831) {
        float ang = atan2(d.y, d.x) - i.arco.w;
        ang = ang - floor(ang / 6.2831853) * 6.2831853;
        if (ang > i.barrido) discard;
    }
    float4 t = tinta(i.color);
    return float4(t.rgb, t.a * a);
}
"#;

/// Lo que el sombreador sabe de la vista.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vista {
    pub escala: [f32; 2],
    pub centro: [f32; 2],
    pub color7: [f32; 4],
    pub px: f32,
    /// 0 los colores tal cual; 1 fondo oscuro; 2 fondo claro.
    pub modo: f32,
    pub relleno: [f32; 2],
}

/// El plano en la tarjeta.
pub struct PlanoGpu {
    vertices: Option<ID3D11Buffer>,
    lineas: Option<ID3D11Buffer>,
    triangulos: Option<ID3D11Buffer>,
    vertices_trama: Option<ID3D11Buffer>,
    triangulos_trama: Option<ID3D11Buffer>,
    tramas: Option<ID3D11ShaderResourceView>,
    familias: Option<ID3D11ShaderResourceView>,
    malla: Option<ID3D11ShaderResourceView>,
    glifos: Option<ID3D11ShaderResourceView>,
    letras: Option<ID3D11Buffer>,
    arcos: Option<ID3D11Buffer>,
    pub tramos_arcos: Vec<Tramo>,
    pub tramos_lineas: Vec<Tramo>,
    pub tramos_triangulos: Vec<Tramo>,
    pub tramos_trama: Vec<Tramo>,
    pub tramos_letras: Vec<Tramo>,
}

/// Lo que pinta encima del bufer de la ventana antes de ensenarlo (la tinta
/// del lienzo, con Direct2D en el mismo dispositivo), si algo.
pub(crate) type PintarEncima<'a> = Option<&'a mut dyn FnMut(&ID3D11Device, &ID3D11Texture2D)>;

pub struct Gpu {
    pub dispositivo: ID3D11Device,
    pub(crate) contexto: ID3D11DeviceContext,
    cadena: IDXGISwapChain1,
    pub(crate) muestras: u32,
    pub(crate) destino: Option<(ID3D11Texture2D, ID3D11RenderTargetView)>,
    pub(crate) ancho: u32,
    pub(crate) alto: u32,
    /// Lo del 3D (sombreadores y profundidad), solo si se usa.
    pub(crate) tres: Option<crate::gpu3d::Tres>,
    cb: ID3D11Buffer,
    vs_simple: ID3D11VertexShader,
    vs_trama: ID3D11VertexShader,
    vs_letra: ID3D11VertexShader,
    vs_arco: ID3D11VertexShader,
    ps_arco: ID3D11PixelShader,
    ps_relleno: ID3D11PixelShader,
    il_arco: ID3D11InputLayout,
    ps_color: ID3D11PixelShader,
    ps_trama: ID3D11PixelShader,
    il_simple: ID3D11InputLayout,
    il_trama: ID3D11InputLayout,
    il_letra: ID3D11InputLayout,
    pub(crate) mezcla: ID3D11BlendState,
    raster: ID3D11RasterizerState,
    /// Cuantos refrescos espera `Present`: 1, y 0 solo al medir (ver
    /// [`Gpu::para_medir`]: sin esperar al refresco se mide el trabajo).
    intervalo: u32,
}

fn compilar(punto: PCSTR, perfil: PCSTR) -> windows::core::Result<Vec<u8>> {
    compilar_de(SOMBREADORES, punto, perfil)
}

pub(crate) fn compilar_de(
    fuente: &str,
    punto: PCSTR,
    perfil: PCSTR,
) -> windows::core::Result<Vec<u8>> {
    let mut codigo: Option<ID3DBlob> = None;
    let mut errores: Option<ID3DBlob> = None;
    // SAFETY: el texto vive durante la llamada; las salidas son locales.
    let r = unsafe {
        D3DCompile(
            fuente.as_ptr() as *const c_void,
            fuente.len(),
            s!("cad.hlsl"),
            None,
            None,
            punto,
            perfil,
            0,
            0,
            &mut codigo,
            Some(&mut errores),
        )
    };
    if let Err(e) = r {
        if let Some(b) = errores {
            // SAFETY: el blob es valido y su puntero y tamano van juntos.
            let t = unsafe {
                std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize())
            };
            tracing::error!(errores = %String::from_utf8_lossy(t), "sombreador del CAD");
        }
        return Err(e);
    }
    let b = codigo.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
    // SAFETY: el blob es valido y su puntero y tamano van juntos.
    Ok(
        unsafe { std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()) }
            .to_vec(),
    )
}

pub(crate) fn elemento(
    nombre: PCSTR,
    formato: DXGI_FORMAT,
    desplazamiento: u32,
    por_instancia: bool,
) -> D3D11_INPUT_ELEMENT_DESC {
    D3D11_INPUT_ELEMENT_DESC {
        SemanticName: nombre,
        SemanticIndex: 0,
        Format: formato,
        InputSlot: 0,
        AlignedByteOffset: desplazamiento,
        InputSlotClass: if por_instancia {
            D3D11_INPUT_PER_INSTANCE_DATA
        } else {
            D3D11_INPUT_PER_VERTEX_DATA
        },
        InstanceDataStepRate: if por_instancia { 1 } else { 0 },
    }
}

impl Gpu {
    pub fn nueva(hwnd: HWND, ancho: u32, alto: u32) -> windows::core::Result<Gpu> {
        let mut dispositivo = None;
        let mut contexto = None;
        let mut ultimo = Err(windows::core::Error::from(
            windows::Win32::Foundation::E_FAIL,
        ));
        for tipo in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
            ultimo = crear_dispositivo(tipo, &mut dispositivo, &mut contexto);
            if ultimo.is_ok() {
                break;
            }
        }
        ultimo?;
        let (Some(dispositivo), Some(contexto)) = (dispositivo, contexto) else {
            return Err(windows::core::Error::from(
                windows::Win32::Foundation::E_FAIL,
            ));
        };
        // SAFETY: llamadas sobre el dispositivo recien creado; descripciones
        // locales que viven durante cada llamada.
        unsafe {
            let fabrica: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: ancho.max(1),
                Height: alto.max(1),
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                ..Default::default()
            };
            let cadena = fabrica.CreateSwapChainForHwnd(&dispositivo, hwnd, &desc, None, None)?;
            let _ = fabrica.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER);
            let muestras = {
                let q = dispositivo
                    .CheckMultisampleQualityLevels(DXGI_FORMAT_B8G8R8A8_UNORM, 4)
                    .unwrap_or(0);
                if q > 0 { 4 } else { 1 }
            };

            let vs_s = compilar(s!("vs_simple"), s!("vs_5_0"))?;
            let vs_t = compilar(s!("vs_trama"), s!("vs_5_0"))?;
            let vs_l = compilar(s!("vs_letra"), s!("vs_5_0"))?;
            let ps_c = compilar(s!("ps_color"), s!("ps_5_0"))?;
            let ps_t = compilar(s!("ps_trama"), s!("ps_5_0"))?;
            let vs_a = compilar(s!("vs_arco"), s!("vs_5_0"))?;
            let ps_a = compilar(s!("ps_arco"), s!("ps_5_0"))?;
            let mut vs_arco = None;
            let mut ps_arco = None;
            let ps_r = compilar(s!("ps_relleno"), s!("ps_5_0"))?;
            let mut ps_relleno = None;
            dispositivo.CreatePixelShader(&ps_r, None, Some(&mut ps_relleno))?;
            dispositivo.CreateVertexShader(&vs_a, None, Some(&mut vs_arco))?;
            dispositivo.CreatePixelShader(&ps_a, None, Some(&mut ps_arco))?;
            let mut il_arco = None;
            dispositivo.CreateInputLayout(
                &[
                    elemento(s!("CENTRO"), DXGI_FORMAT_R32G32_FLOAT, 0, true),
                    elemento(s!("RADIO"), DXGI_FORMAT_R32_FLOAT, 8, true),
                    elemento(s!("INICIO"), DXGI_FORMAT_R32_FLOAT, 12, true),
                    elemento(s!("BARRIDO"), DXGI_FORMAT_R32_FLOAT, 16, true),
                    elemento(s!("COLOR"), DXGI_FORMAT_R32_UINT, 20, true),
                ],
                &vs_a,
                Some(&mut il_arco),
            )?;
            let mut vs_simple = None;
            let mut vs_trama = None;
            let mut vs_letra = None;
            let mut ps_color = None;
            let mut ps_trama = None;
            dispositivo.CreateVertexShader(&vs_s, None, Some(&mut vs_simple))?;
            dispositivo.CreateVertexShader(&vs_t, None, Some(&mut vs_trama))?;
            dispositivo.CreateVertexShader(&vs_l, None, Some(&mut vs_letra))?;
            dispositivo.CreatePixelShader(&ps_c, None, Some(&mut ps_color))?;
            dispositivo.CreatePixelShader(&ps_t, None, Some(&mut ps_trama))?;

            let mut il_simple = None;
            dispositivo.CreateInputLayout(
                &[
                    elemento(s!("POS"), DXGI_FORMAT_R32G32_FLOAT, 0, false),
                    elemento(s!("COLOR"), DXGI_FORMAT_R32_UINT, 8, false),
                ],
                &vs_s,
                Some(&mut il_simple),
            )?;
            let mut il_trama = None;
            dispositivo.CreateInputLayout(
                &[
                    elemento(s!("POS"), DXGI_FORMAT_R32G32_FLOAT, 0, false),
                    elemento(s!("COLOR"), DXGI_FORMAT_R32_UINT, 8, false),
                    elemento(s!("TRAMA"), DXGI_FORMAT_R32_UINT, 12, false),
                ],
                &vs_t,
                Some(&mut il_trama),
            )?;
            let mut il_letra = None;
            dispositivo.CreateInputLayout(
                &[
                    elemento(s!("POS"), DXGI_FORMAT_R32G32_FLOAT, 0, true),
                    elemento(s!("MAT"), DXGI_FORMAT_R32G32B32A32_FLOAT, 8, true),
                    elemento(s!("COLOR"), DXGI_FORMAT_R32_UINT, 24, true),
                    elemento(s!("GLIFO"), DXGI_FORMAT_R32_UINT, 28, true),
                ],
                &vs_l,
                Some(&mut il_letra),
            )?;

            let mut cb = None;
            dispositivo.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: std::mem::size_of::<Vista>() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut cb),
            )?;

            let mut rt = D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            rt.BlendEnable = true.into();
            let mut bd = D3D11_BLEND_DESC::default();
            bd.RenderTarget[0] = rt;
            let mut mezcla = None;
            dispositivo.CreateBlendState(&bd, Some(&mut mezcla))?;
            let mut raster = None;
            dispositivo.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    MultisampleEnable: (muestras > 1).into(),
                    AntialiasedLineEnable: (muestras == 1).into(),
                    ..Default::default()
                },
                Some(&mut raster),
            )?;

            let falta = || windows::core::Error::from(windows::Win32::Foundation::E_FAIL);
            let mut g = Gpu {
                dispositivo,
                contexto,
                cadena,
                muestras,
                destino: None,
                tres: None,
                ancho: ancho.max(1),
                alto: alto.max(1),
                cb: cb.ok_or_else(falta)?,
                vs_simple: vs_simple.ok_or_else(falta)?,
                vs_trama: vs_trama.ok_or_else(falta)?,
                vs_letra: vs_letra.ok_or_else(falta)?,
                vs_arco: vs_arco.ok_or_else(falta)?,
                ps_arco: ps_arco.ok_or_else(falta)?,
                ps_relleno: ps_relleno.ok_or_else(falta)?,
                il_arco: il_arco.ok_or_else(falta)?,
                ps_color: ps_color.ok_or_else(falta)?,
                ps_trama: ps_trama.ok_or_else(falta)?,
                il_simple: il_simple.ok_or_else(falta)?,
                il_trama: il_trama.ok_or_else(falta)?,
                il_letra: il_letra.ok_or_else(falta)?,
                mezcla: mezcla.ok_or_else(falta)?,
                raster: raster.ok_or_else(falta)?,
                intervalo: 1,
            };
            g.preparar_destino()?;
            Ok(g)
        }
    }

    fn preparar_destino(&mut self) -> windows::core::Result<()> {
        // SAFETY: recursos de este dispositivo; descripciones locales.
        unsafe {
            let tex = D3D11_TEXTURE2D_DESC {
                Width: self.ancho,
                Height: self.alto,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: self.muestras,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                ..Default::default()
            };
            let mut t = None;
            self.dispositivo.CreateTexture2D(&tex, None, Some(&mut t))?;
            let t =
                t.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
            let mut rtv = None;
            self.dispositivo
                .CreateRenderTargetView(&t, None, Some(&mut rtv))?;
            let rtv =
                rtv.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
            self.destino = Some((t, rtv));
        }
        Ok(())
    }

    pub fn redimensionar(&mut self, ancho: u32, alto: u32) -> windows::core::Result<()> {
        let (ancho, alto) = (ancho.max(1), alto.max(1));
        if (ancho, alto) == (self.ancho, self.alto) {
            return Ok(());
        }
        self.destino = None;
        if let Some(t) = &mut self.tres {
            t.soltar_destinos();
        }
        // SAFETY: no queda ninguna vista de los bufferes de la cadena (se
        // piden en cada `presentar`), asi que se pueden rehacer.
        unsafe {
            self.contexto.OMSetRenderTargets(None, None);
            self.contexto.Flush();
            self.cadena.ResizeBuffers(
                0,
                ancho,
                alto,
                DXGI_FORMAT_UNKNOWN,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
        }
        self.ancho = ancho;
        self.alto = alto;
        self.preparar_destino()
    }

    pub fn tamano(&self) -> (u32, u32) {
        (self.ancho, self.alto)
    }

    /// **Para medir fotogramas sin abrir nada** (pruebas de rendimiento de
    /// quien pinta encima, como lo anotado): la misma tarjeta sobre una
    /// ventana propia que nunca se muestra (no sale en el escritorio ni
    /// recibe nada del usuario).
    pub fn para_medir(ancho: u32, alto: u32) -> windows::core::Result<Gpu> {
        use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, WINDOW_EX_STYLE, WS_POPUP};
        // SAFETY: una ventana de la clase de sistema STATIC, sin mostrar;
        // cadenas estaticas.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                windows::core::w!("STATIC"),
                windows::core::w!("medir"),
                WS_POPUP,
                0,
                0,
                ancho as i32,
                alto as i32,
                None,
                None,
                None,
                None,
            )?
        };
        let mut g = Gpu::nueva(hwnd, ancho, alto)?;
        g.intervalo = 0;
        Ok(g)
    }

    /// Espera a que la tarjeta acabe todo lo encolado: para medir lo que
    /// cuesta de verdad un fotograma, no lo que tarda en encolarse.
    pub fn esperar(&self) {
        // SAFETY: consulta de evento del dispositivo propio, en su hilo;
        // `hecho` es local y del tamano que se pide.
        unsafe {
            let mut q: Option<ID3D11Query> = None;
            if self
                .dispositivo
                .CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: D3D11_QUERY_EVENT,
                        MiscFlags: 0,
                    },
                    Some(&mut q),
                )
                .is_err()
            {
                return;
            }
            let Some(q) = q else { return };
            self.contexto.End(&q);
            let mut hecho: i32 = 0;
            while self
                .contexto
                .GetData(&q, Some(&mut hecho as *mut i32 as *mut c_void), 4, 0)
                .is_err()
                || hecho == 0
            {
                std::hint::spin_loop();
            }
        }
    }

    pub(crate) fn buffer(
        &self,
        bytes: &[u8],
        bind: D3D11_BIND_FLAG,
        estructurado: u32,
    ) -> windows::core::Result<Option<ID3D11Buffer>> {
        if bytes.is_empty() {
            return Ok(None);
        }
        let desc = D3D11_BUFFER_DESC {
            ByteWidth: bytes.len() as u32,
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: bind.0 as u32,
            MiscFlags: if estructurado > 0 {
                D3D11_RESOURCE_MISC_BUFFER_STRUCTURED.0 as u32
            } else {
                0
            },
            StructureByteStride: estructurado,
            ..Default::default()
        };
        let datos = D3D11_SUBRESOURCE_DATA {
            pSysMem: bytes.as_ptr() as *const c_void,
            ..Default::default()
        };
        let mut b = None;
        // SAFETY: `bytes` vive durante la llamada; Direct3D copia los datos.
        unsafe {
            self.dispositivo
                .CreateBuffer(&desc, Some(&datos), Some(&mut b))?
        };
        Ok(b)
    }

    fn estructurado(
        &self,
        bytes: &[u8],
        paso: u32,
    ) -> windows::core::Result<Option<ID3D11ShaderResourceView>> {
        let Some(b) = self.buffer(bytes, D3D11_BIND_SHADER_RESOURCE, paso)? else {
            return Ok(None);
        };
        let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_UNKNOWN,
            ViewDimension: D3D_SRV_DIMENSION_BUFFEREX,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                BufferEx: D3D11_BUFFEREX_SRV {
                    FirstElement: 0,
                    NumElements: (bytes.len() as u32) / paso,
                    Flags: 0,
                },
            },
        };
        let mut v = None;
        // SAFETY: buffer de este dispositivo; descripcion local.
        unsafe {
            self.dispositivo
                .CreateShaderResourceView(&b, Some(&desc), Some(&mut v))?
        };
        Ok(v)
    }

    /// Sube el plano. Lo que no cabe en la tarjeta da error y no se dibuja.
    pub fn subir(&self, m: &Modelo) -> windows::core::Result<PlanoGpu> {
        fn bytes_de<T: Copy>(v: &[T]) -> &[u8] {
            // SAFETY: tipos `Copy` de solo numeros, sin relleno entre campos
            // (todos de 4 bytes): verlos como bytes es valido.
            unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
        }
        Ok(PlanoGpu {
            vertices: self.buffer(bytes_de(&m.vertices), D3D11_BIND_VERTEX_BUFFER, 0)?,
            lineas: self.buffer(bytes_de(&m.lineas), D3D11_BIND_INDEX_BUFFER, 0)?,
            triangulos: self.buffer(bytes_de(&m.triangulos), D3D11_BIND_INDEX_BUFFER, 0)?,
            vertices_trama: self.buffer(
                bytes_de(&m.vertices_trama),
                D3D11_BIND_VERTEX_BUFFER,
                0,
            )?,
            triangulos_trama: self.buffer(
                bytes_de(&m.triangulos_trama),
                D3D11_BIND_INDEX_BUFFER,
                0,
            )?,
            tramas: self.estructurado(bytes_de(&m.tramas), 36)?,
            familias: self.estructurado(bytes_de(&m.familias), 64)?,
            malla: self.estructurado(bytes_de(&m.malla_letras), 8)?,
            glifos: self.estructurado(bytes_de(&m.glifos), 8)?,
            letras: self.buffer(bytes_de(&m.letras), D3D11_BIND_VERTEX_BUFFER, 0)?,
            arcos: self.buffer(bytes_de(&m.arcos), D3D11_BIND_VERTEX_BUFFER, 0)?,
            tramos_arcos: m.tramos_arcos.clone(),
            tramos_lineas: m.tramos_lineas.clone(),
            tramos_triangulos: m.tramos_triangulos.clone(),
            tramos_trama: m.tramos_trama.clone(),
            tramos_letras: m.tramos_letras.clone(),
        })
    }

    /// Dibuja `planos` (el plano y, encima, la barra) con su vista cada uno.
    pub fn dibujar(
        &mut self,
        fondo: [f32; 4],
        planos: &[(&PlanoGpu, Vista)],
    ) -> windows::core::Result<()> {
        let Some((tex, rtv)) = self.destino.clone() else {
            return Ok(());
        };
        // SAFETY: destino de este dispositivo.
        unsafe {
            self.contexto.ClearRenderTargetView(&rtv, &fondo);
        }
        self.pasadas_2d(&rtv, planos);
        self.presentar(&tex)
    }

    /// [`Gpu::dibujar`] y, encima, lo que pinte `encima` (ver [`Gpu::presentar_con`]).
    pub fn dibujar_con(
        &mut self,
        fondo: [f32; 4],
        planos: &[(&PlanoGpu, Vista)],
        encima: &mut dyn FnMut(&ID3D11Device, &ID3D11Texture2D),
    ) -> windows::core::Result<()> {
        let Some((tex, rtv)) = self.destino.clone() else {
            return Ok(());
        };
        // SAFETY: destino de este dispositivo.
        unsafe {
            self.contexto.ClearRenderTargetView(&rtv, &fondo);
        }
        self.pasadas_2d(&rtv, planos);
        self.presentar_con(&tex, Some(encima))
    }

    pub(crate) fn ventana_entera(&self) -> D3D11_VIEWPORT {
        D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: self.ancho as f32,
            Height: self.alto as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        }
    }

    /// Lo plano encima de lo que ya hay en el destino (sin profundidad).
    pub(crate) fn pasadas_2d(&self, rtv: &ID3D11RenderTargetView, planos: &[(&PlanoGpu, Vista)]) {
        let ctx = &self.contexto;
        // SAFETY: todo es de este dispositivo y de este hilo; los punteros a
        // datos locales viven durante cada llamada.
        unsafe {
            ctx.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            ctx.OMSetDepthStencilState(None, 0);
            ctx.RSSetViewports(Some(&[self.ventana_entera()]));
            ctx.RSSetState(&self.raster);
            ctx.OMSetBlendState(&self.mezcla, None, 0xffff_ffff);
            ctx.VSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            ctx.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            for (p, vista) in planos {
                ctx.UpdateSubresource(
                    &self.cb,
                    0,
                    None,
                    vista as *const Vista as *const c_void,
                    0,
                    0,
                );
                let caja = caja_de_vista(vista, self.ancho, self.alto);
                // 1. Rellenos.
                if let (Some(vb), Some(ib)) = (&p.vertices, &p.triangulos) {
                    ctx.IASetInputLayout(&self.il_simple);
                    ctx.IASetVertexBuffers(0, 1, Some(&Some(vb.clone())), Some(&12), Some(&0));
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    ctx.VSSetShader(&self.vs_simple, None);
                    ctx.PSSetShader(&self.ps_relleno, None);
                    for (d, n) in visibles(&p.tramos_triangulos, &caja, vista.px * 1.0) {
                        ctx.DrawIndexed(n, d, 0);
                    }
                }
                // 2. Sombreados con patron.
                if let (Some(vb), Some(ib)) = (&p.vertices_trama, &p.triangulos_trama) {
                    ctx.IASetInputLayout(&self.il_trama);
                    ctx.IASetVertexBuffers(0, 1, Some(&Some(vb.clone())), Some(&16), Some(&0));
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    ctx.VSSetShader(&self.vs_trama, None);
                    ctx.PSSetShader(&self.ps_trama, None);
                    ctx.PSSetShaderResources(0, Some(&[p.tramas.clone(), p.familias.clone()]));
                    for (d, n) in visibles(&p.tramos_trama, &caja, vista.px * 2.0) {
                        ctx.DrawIndexed(n, d, 0);
                    }
                }
                // 3. Rayas.
                if let (Some(vb), Some(ib)) = (&p.vertices, &p.lineas) {
                    ctx.IASetInputLayout(&self.il_simple);
                    ctx.IASetVertexBuffers(0, 1, Some(&Some(vb.clone())), Some(&12), Some(&0));
                    ctx.IASetIndexBuffer(ib, DXGI_FORMAT_R32_UINT, 0);
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_LINESTRIP);
                    ctx.VSSetShader(&self.vs_simple, None);
                    ctx.PSSetShader(&self.ps_color, None);
                    for (d, n) in visibles(&p.tramos_lineas, &caja, vista.px * 0.75) {
                        ctx.DrawIndexed(n, d, 0);
                    }
                }
                // 3b. Circulos y arcos.
                if let Some(ab) = &p.arcos {
                    ctx.IASetInputLayout(&self.il_arco);
                    ctx.IASetVertexBuffers(0, 1, Some(&Some(ab.clone())), Some(&32), Some(&0));
                    ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                    ctx.VSSetShader(&self.vs_arco, None);
                    ctx.PSSetShader(&self.ps_arco, None);
                    for (d, n) in visibles(&p.tramos_arcos, &caja, vista.px * 0.75) {
                        ctx.DrawInstanced(6, n, 0, d);
                    }
                }
                // 4. Letras: un texto de menos de 3 pixeles no se lee.
                if let (Some(lb), Some(_)) = (&p.letras, &p.malla) {
                    ctx.IASetInputLayout(&self.il_letra);
                    ctx.IASetVertexBuffers(0, 1, Some(&Some(lb.clone())), Some(&32), Some(&0));
                    ctx.VSSetShader(&self.vs_letra, None);
                    ctx.PSSetShader(&self.ps_color, None);
                    ctx.VSSetShaderResources(2, Some(&[p.malla.clone(), p.glifos.clone()]));
                    for t in p
                        .tramos_letras
                        .iter()
                        .filter(|t| t.tamano >= vista.px * 3.0 && corta(&t.caja, &caja))
                    {
                        // Las de una fuente SHX son rayas; las TrueType, triangulos.
                        let rayas = t.clase & crate::modelo::GLIFO_DE_RAYAS != 0;
                        ctx.IASetPrimitiveTopology(if rayas {
                            D3D_PRIMITIVE_TOPOLOGY_LINELIST
                        } else {
                            D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST
                        });
                        ctx.DrawInstanced(
                            t.clase & !crate::modelo::GLIFO_DE_RAYAS,
                            t.cuantos,
                            0,
                            t.desde,
                        );
                    }
                }
            }
        }
    }

    /// Del destino a la ventana.
    pub(crate) fn presentar(&self, tex: &ID3D11Texture2D) -> windows::core::Result<()> {
        self.presentar_con(tex, None)
    }

    /// Como [`Gpu::presentar`], dejando a `encima` pintar sobre el bufer de la
    /// ventana ya hecho, antes de ensenarlo (la tinta del motor del lienzo,
    /// con Direct2D en este mismo dispositivo).
    pub(crate) fn presentar_con(
        &self,
        tex: &ID3D11Texture2D,
        encima: PintarEncima<'_>,
    ) -> windows::core::Result<()> {
        let ctx = &self.contexto;
        // SAFETY: recursos de este dispositivo y de este hilo.
        unsafe {
            if self.muestras > 1 {
                let atras: ID3D11Texture2D = self.cadena.GetBuffer(0)?;
                ctx.ResolveSubresource(&atras, 0, tex, 0, DXGI_FORMAT_B8G8R8A8_UNORM);
            } else {
                let atras: ID3D11Texture2D = self.cadena.GetBuffer(0)?;
                ctx.CopyResource(&atras, tex);
            }
            if let Some(f) = encima {
                let atras: ID3D11Texture2D = self.cadena.GetBuffer(0)?;
                f(&self.dispositivo, &atras);
            }
            self.cadena.Present(self.intervalo, DXGI_PRESENT(0)).ok()?;
        }
        Ok(())
    }
}

fn crear_dispositivo(
    tipo: D3D_DRIVER_TYPE,
    d: &mut Option<ID3D11Device>,
    c: &mut Option<ID3D11DeviceContext>,
) -> windows::core::Result<()> {
    // SAFETY: salidas locales; sin adaptador concreto (el de Windows).
    unsafe {
        D3D11CreateDevice(
            None,
            tipo,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(d),
            None,
            Some(c),
        )
    }
}

/// Lo que ve la ventana, en coordenadas del plano.
fn caja_de_vista(v: &Vista, ancho: u32, alto: u32) -> [f32; 4] {
    let (mx, my) = (ancho as f32 / 2.0 * v.px, alto as f32 / 2.0 * v.px);
    [
        v.centro[0] - mx,
        v.centro[1] - my,
        v.centro[0] + mx,
        v.centro[1] + my,
    ]
}

fn corta(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

/// Los tramos que se ven (por sitio y por tamano), juntando los seguidos.
pub fn visibles(tramos: &[Tramo], caja: &[f32; 4], minimo: f32) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for t in tramos {
        // Van de mayor a menor: en cuanto uno es pequeno, los demas tambien.
        if t.tamano < minimo {
            break;
        }
        if !corta(&t.caja, caja) {
            continue;
        }
        match out.last_mut() {
            Some((d, n)) if *d + *n == t.desde => *n += t.cuantos,
            _ => out.push((t.desde, t.cuantos)),
        }
    }
    out
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn tramo(desde: u32, cuantos: u32, caja: [f32; 4], tamano: f32) -> Tramo {
        Tramo {
            desde,
            cuantos,
            caja,
            tamano,
            clase: 0,
        }
    }

    #[test]
    fn solo_se_dibuja_lo_que_cae_en_la_ventana_y_se_veria() {
        let t = [
            tramo(0, 10, [0.0, 0.0, 100.0, 100.0], 64.0),
            tramo(10, 5, [0.0, 0.0, 10.0, 10.0], 8.0),
            tramo(15, 5, [500.0, 500.0, 510.0, 510.0], 8.0),
            tramo(20, 5, [0.0, 0.0, 1.0, 1.0], 0.5),
        ];
        // Ventana sobre el origen, un pixel = 1: los dos primeros seguidos.
        assert_eq!(visibles(&t, &[-5.0, -5.0, 50.0, 50.0], 1.0), vec![(0, 15)]);
        // De lejos (un pixel = 16): solo el grande.
        assert_eq!(
            visibles(&t, &[-500.0, -500.0, 600.0, 600.0], 16.0),
            vec![(0, 10)]
        );
        // Caso negativo: fuera de todo, nada.
        assert!(visibles(&t, &[2000.0, 2000.0, 3000.0, 3000.0], 1.0).is_empty());
    }
}
