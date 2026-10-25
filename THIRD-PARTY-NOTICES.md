# Avisos de terceros

PixPin Max incluye código traducido a Rust de los siguientes proyectos, bajo sus licencias.

## Excalidraw

Módulos: `crates/pixpin-motor2d/src/tinta/{mod.rs,laser.rs}`, `crates/pixpin-render/src/tinta.rs`, `crates/pixpin-render/src/iconos_excalidraw.rs` (iconos de `packages/excalidraw/components/icons.tsx`; algunos de Tabler Icons, MIT).

MIT License

Copyright (c) 2020 Excalidraw

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## perfect-freehand

Módulo: `crates/pixpin-motor2d/src/tinta/freehand.rs`.

MIT License

Copyright (c) 2021 Stephen Ruiz Ltd

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Material Icons (Google)

Módulos: `crates/pixpin-render/src/icono/material.rs` y los iconos de `apps/pixpin/src/sincronizar.rs`: trazados `d` de `src/<grupo>/<nombre>/materialicons/24px.svg` del repositorio `google/material-design-icons`, los mismos `Icons.Filled.*` que usa PixPin Android.

Apache License 2.0 — Copyright Google LLC. Texto completo en <https://www.apache.org/licenses/LICENSE-2.0>. Los iconos se usan sin modificar.

## Dependencias del 3D de CAD-pixpin (`crates/cad-nucleo/src/tresd`)

Los escritores de OBJ, STL, glTF e IFC4 son propios (sin código copiado). Se usan como dependencias, sin modificar:

- `opencadcodec` con su función `import` (MPL-2.0, ya usado): lee STL, OBJ, glTF/GLB, FBX y COLLADA para IMPORTAR3D. Trae `quick-xml` (MIT), para el COLLADA.
- Solo en las pruebas: `ifc-lite-processing` (MPL-2.0, el mismo que usa `pixpin-bim`) vuelve a leer el IFC escrito, y `serde_json` (MIT o Apache-2.0) lee el JSON del glTF.

## flate2 y roxmltree (dependencias de `cad-nucleo`)

`crates/cad-nucleo/src/formatos/` usa `flate2` (comprimir y descomprimir los flujos de los PDF) y `roxmltree` (leer los SVG que se importan). Los dos ya estaban en el arbol de dependencias (opencadcodec, pixpin-pdf y pixpin-bim) y se distribuyen bajo MIT o Apache-2.0, a eleccion: <https://github.com/rust-lang/flate2-rs>, <https://github.com/RazrFalcon/roxmltree>.

## Fuentes de Hershey (romans.shx)

`crates/cad-nucleo/fuentes/romans.shx` (los textos exportados como rayas) es la de `crates/pixpin-cad/fuentes-shx/`, hecha por PixPin con los datos de Hershey (ver `crates/cad-nucleo/fuentes/LEEME-HERSHEY.txt`):

- The Hershey Fonts were originally created by Dr. A. V. Hershey while working at the U. S. National Bureau of Standards.
- The format of the Font data in this distribution was originally created by James Hurt, Cognition, Inc., 900 Technology Park Drive, Billerica, MA 01821 (mit-eddie!ci-dandelion!hurt).
