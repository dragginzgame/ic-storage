# Source provenance

The allocation runtime is restored from published `ic-storage` 0.64.0,
commit `fd8c49975f3f0b4a34b988b4985040eb910fcf44` in
https://github.com/dragginzgame/ic-storage-legacy.
Upstream collection adapters are restored from 0.63.0,
commit `d545eccd9dbb52fa32dbc7716b67a0e729619ef1`, with their imports adapted to
the merged crate. Cell-envelope validation follows the corresponding ic-memory
0.63.0 implementation. Cached registry archives retain their source identities.

The B-tree and checked record framing derive from the MIT-licensed
`ic-storage` 0.67.0, revision `816e520bf3bfba5763700f1330817127cf039a00`.
The original grant is reproduced below. Its
[provenance record](https://github.com/dragginzgame/ic-storage-legacy/blob/816e520bf3bfba5763700f1330817127cf039a00/docs/storage/upstreams.md)
documents an independent implementation of standard B-tree algorithms. This
cut adds bounded bulk mutation and a dedicated binary root and free-link codec.

The Cell, backing interface and primitive record codecs derive from
`ic-storage` 0.74.1, revision `f001cf4373786ad76dc6cc55c62eb353029987b8`,
under the Toko Studios Network Licence. See its
[implementation record](https://github.com/dragginzgame/ic-storage-legacy/blob/f001cf4373786ad76dc6cc55c62eb353029987b8/docs/storage/upstreams.md).
The current API uses initialized slice reads and retains Storage's existing
collection call signatures.

The imported 0.63/0.64/0.67 source was distributed under this grant, retained here:

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
