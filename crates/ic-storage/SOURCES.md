# Source provenance

The allocation runtime is restored from published `ic-storage` 0.64.0,
commit `fd8c49975f3f0b4a34b988b4985040eb910fcf44` in
https://github.com/dragginzgame/ic-storage-legacy.
Upstream collection adapters are restored from 0.63.0,
commit `d545eccd9dbb52fa32dbc7716b67a0e729619ef1`, with their imports adapted to
the merged crate. Cell-envelope validation follows the corresponding ic-memory
0.63.0 implementation. Cached registry archives retain their source identities.

Changes here reconnect upstream Memory/Storable, adopt current Platform raw calls
and Storage-owned directory encoding, give the new directory its own magic, and qualify the combined
runtime. No old custom tree implementation is carried forward.
The workspaces select a modified copy of the Apache-2.0 ic-stable-structures
0.7.2 crate at `vendor/ic-stable-structures`. Its B-tree deletion change comes
from the IC DB bulk-delete leaf-batching prototype. The vendor directory
retains the upstream license. The Platform facade supplies the collection types.

The imported 0.63/0.64 source was distributed under this grant, retained here:

MIT License

Copyright (c) 2026 Toko Studios Ltd and borovan

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
