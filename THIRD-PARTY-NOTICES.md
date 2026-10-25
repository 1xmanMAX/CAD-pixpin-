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
