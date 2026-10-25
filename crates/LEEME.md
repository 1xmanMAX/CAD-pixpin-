# crates/ — lo copiado de PixPin Max y lo nuevo de CAD-pixpin

## Copiado tal cual de PIXPIN_PRO_WINDOWS (commit `831a7b6`)

Se copiaron con `cp -r`, sin tocar una línea, para partir del mismo visor y
del mismo motor de anotado que PixPin Max. Conservan su nombre `pixpin-*`
para que sus `use` internos sigan valiendo.

| Crate | Qué es |
|---|---|
| `pixpin-cad` | El visor de planos DWG/DXF (Direct3D 11) y el de modelos 3D, marcadores, regla, letras SHX, el puente `Anotador` para dibujar encima. Lee con `opencadcodec` (OpenCADStudio, MPL-2.0). |
| `pixpin-bim` | IFC (ifc-lite), Revit `.rvt` (rvt-rs), LandXML de Civil 3D, niveles. |
| `pixpin-motor2d` | El motor del lienzo: figuras, flechas, cotas, textos, deshacer… con el que se anota. |
| `pixpin-tinta` | El trazo a mano alzada. |
| `pixpin-ui` | Barras, paneles, propiedades, caja de herramientas. |
| `pixpin-render` | Direct2D / DirectWrite / DirectComposition e imprimir. |
| `pixpin-geom`, `pixpin-nivel`, `pixpin-sistema`, `pixpin-shell`, `pixpin-codec`, `pixpin-docs`, `pixpin-hojas`, `pixpin-office`, `pixpin-pdf` | Lo que los de arriba necesitan para compilar. |

Si en PixPin Max se arregla algo en estos crates, se puede volver a copiar
encima: CAD-pixpin no los modifica, pone lo suyo en crates `cad-*` aparte
(salvo lo poco de «Cambios en lo copiado», que habria que volver a poner).

## Cambios en lo copiado

Lo minimo, y solo anadido (lo de antes funciona igual):

| Donde | Que | Para que |
|---|---|---|
| `pixpin-cad/src/convertir.rs` | `Convertidor::espacio(doc)`: lo que se dibuja (el espacio modelo o, si no tiene nada, el papel), sacado de `recorrer`. `Convertidor::convertir_estas(entidades)`: convierte solo esas entidades (pueden no estar en el documento) con los bloques, capas y estilos del documento, y devuelve sus letras ya hechas. `Convertidor::con_textos(textos)`: empieza con esas letras. | La app parte el plano en cubos (`apps/cadpixpin/src/cubos.rs` y `plano.rs`) y al editar solo convierte los tocados, en varios hilos, sin volver a leer las fuentes en cada cubo. |

## Nuevo en CAD-pixpin

Los crates `cad-*` (ver el README de la raíz).
