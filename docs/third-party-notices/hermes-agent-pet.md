# Hermes Agent pet code notice

ReadMD V2.3.8 adapts the native transparent-window drag contract from
[`NousResearch/hermes-agent`](https://github.com/NousResearch/hermes-agent),
revision `fb27614addac115d55299bc6538ae112fd01f688`, specifically
`apps/desktop/electron/pet-overlay-ipc.ts` and its accompanying desktop pet
components.

The upstream repository is licensed under the MIT License:

> MIT License
>
> Copyright (c) 2025 Nous Research
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

ReadMD's optional `packages/pet-overlay` copies the overlay architecture and
the upstream fallback sprite
`apps/desktop/public/hermes-sprite.png` (SHA-256
`e328d387a2fca8c02452fa534da1a89bdb8be9292cc51ffa66905018e74097a3`).
It does not copy generated user pets, service credentials, or Hermes UI
dependencies. Electron is not bundled in the ReadMD main application and the
overlay is not started on application launch. The selected default character is
the separately recorded Arch-chan model, not the Hermes fallback sprite.
