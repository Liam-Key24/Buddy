# -*- mode: python ; coding: utf-8 -*-
"""Portable PyInstaller spec for the Buddy backend sidecar."""

from __future__ import annotations

import os

from PyInstaller.utils.hooks import collect_submodules

backend = SPECPATH
triple = os.environ.get("BUDDY_SIDECAR_TRIPLE", "aarch64-apple-darwin")
entry = os.path.join(backend, "sidecar_entry.py")

hiddenimports = collect_submodules("app") + collect_submodules("uvicorn")

a = Analysis(
    [entry],
    pathex=[backend],
    binaries=[],
    datas=[],
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name=f"buddy-backend-{triple}",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
