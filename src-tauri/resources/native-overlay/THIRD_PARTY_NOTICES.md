# Third-party notices

Only the source/header/backend files required by the x64 build are vendored under `third_party/`.

## Dear ImGui

- Project: Dear ImGui
- Upstream: https://github.com/ocornut/imgui
- Pinned release: `v1.92.4`
- Pinned commit: `9a5d5c45f54b1301ea471622eddede70384243af`
- License: MIT
- Source license text: `third_party/imgui/LICENSE.txt`
- Distributed license text: `licenses/imgui-LICENSE.txt`
- Included components: core (`imgui*.cpp/.h`, bundled `imstb_*.h`) and Win32/DX11 backend pairs only.

Copyright (c) 2014-2025 Omar Cornut.

## MinHook

- Project: MinHook - The Minimalistic API Hooking Library for x64/x86
- Upstream: https://github.com/TsudaKageyu/minhook
- Pinned release: `v1.3.4`
- Pinned commit: `c3fcafdc10146beb5919319d0683e44e3c30d537`
- License: 2-clause BSD
- Source license text: `third_party/minhook/LICENSE.txt`
- Distributed license text: `licenses/minhook-LICENSE.txt`
- Included components: public header and required `src`/HDE implementation only.

Copyright (C) 2009-2025 Tsuda Kageyu.

The complete authoritative license terms for the source-only components are retained in the license files above.

## Microsoft Edge WebView2 SDK

- Project: Microsoft Edge WebView2
- Package: `Microsoft.Web.WebView2` `1.0.4078.44`
- Upstream: https://developer.microsoft.com/microsoft-edge/webview2/
- License: BSD 3-Clause
- Vendored files: native API headers and the x64 static loader library only
- Source license and notices: `third_party/webview2/LICENSE.txt`, `third_party/webview2/NOTICE.txt`
- Distributed license and notices: `licenses/webview2-LICENSE.txt`, `licenses/webview2-NOTICE.txt`

The static loader is linked into `hq_overlay.dll`; the Evergreen WebView2 Runtime remains a system prerequisite.
