#!/usr/bin/env python3
"""Builds the release licence record (spec 0002) from finished release builds.

Called by scripts/build-release.sh, inside the devcontainer, after it has built every target with
`RUSTFLAGS=-Csave-temps ZIG_VERBOSE_LINK=1 cargo zigbuild`. For each target it is given the cargo
target directory and the build log. It then:

  1. finds zig's final `ld.lld` command in the log and replays it with `-Map=`, requiring the
     replay to be byte-identical to the shipped binary (that ties the map to the real link);
  2. checks the binary is static and every linker input that contributes bytes is covered;
  3. writes fpx-convert/<arch>/fpx-convert and third-party/fpx-convert-<arch>/ (entries.tsv,
     licenses/, source/), and one BUILD_INFO.txt for all targets.

Any check that fails stops the release: a record that is wrong is worse than none.

Usage: licence-record.py <out-dir> <triple>:<arch>:<cargo-target-dir>:<build-log> ...
"""

import hashlib
import json
import re
import shutil
import struct
import subprocess
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Upstream sources, tied to the pinned versions. A pin changes -> these must be re-derived (the
# 2026-10-15 investigation compared zig's bundled copies with these tarballs file by file).
RUST_SOURCES = {
    "1.98.1": (
        "https://static.rust-lang.org/dist/rustc-1.98.1-src.tar.xz",
        "be1816e7f6c40abb90245ad6e024bed2a7e88d7dda4561e4d5470207df616b9f",
    ),
}
ZIG_BUNDLED = {
    "0.13.0": {
        "clang": "18.1.6",
        "musl": (
            "1.2.5",
            "https://musl.libc.org/releases/musl-1.2.5.tar.gz",
            "a9a118bbe84d8764da0ea0d28b3ab3fae8477fc7e4085d90102b8596fc7c75e4",
        ),
        "libunwind": (
            "18.1.6",
            "https://github.com/llvm/llvm-project/releases/download/llvmorg-18.1.6/"
            "libunwind-18.1.6.src.tar.xz",
            "7ed4651612a5c07aadfda38fdbd4e7e786877826790b5d74ce30edd638eb00b3",
        ),
    },
}
MUSL_LICENCE = "MIT"
LIBUNWIND_LICENCE = "Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT)"
STD_LICENCE = "MIT OR Apache-2.0"

# Recognisable text per SPDX identifier: every phrase must appear (whitespace and case ignored).
LICENCE_PHRASES = {
    "MIT": ["permission is hereby granted, free of charge"],
    "0BSD": ["permission to use, copy, modify, and/or distribute this software for any purpose"
             " with or without fee is hereby granted"],
    "Apache-2.0": ["apache license", "version 2.0", "terms and conditions for use, reproduction"],
    "LLVM-exception": ["llvm exceptions to the apache 2.0 license"],
    "NCSA": ["university of illinois/ncsa open source license"],
    "Zlib": ["this software is provided 'as-is'", "altered source versions must be plainly marked"],
    "BSD-2-Clause": ["redistribution and use in source and binary forms"],
    "BSD-3-Clause": [
        "redistribution and use in source and binary forms",
        "may be used to endorse or promote products",
    ],
    "Unlicense": ["this is free and unencumbered software released into the public domain"],
    "IJG": ["independent jpeg group"],
    "AGPL-3.0-or-later": ["gnu affero general public license", "version 3"],
}
LICENCE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|COPYRIGHT|NOTICE|UNLICENSE)", re.IGNORECASE)


class RecordError(Exception):
    pass


def fail(msg):
    raise RecordError(msg)


def run(*args, **kw):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kw).stdout


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# ---------------------------------------------------------------------------------------------
# Licence expressions


def normalise_licence(expr):
    """crates.io's legacy `/` separator means OR."""
    return re.sub(r"\s*/\s*", " OR ", expr.strip())


def spdx_ids(expr):
    return [t for t in re.findall(r"[A-Za-z0-9.+-]+", expr) if t not in ("AND", "OR", "WITH")]


def licence_covered(expr, texts):
    """True if `texts` (normalised file contents) cover the SPDX expression `expr`."""
    tokens = re.findall(r"\(|\)|[A-Za-z0-9.+-]+", expr)
    pos = 0

    def has(ident):
        if ident not in LICENCE_PHRASES:
            fail(f"licence identifier {ident!r} (in {expr!r}) is not one the build recognises")
        return any(all(p in t for p in LICENCE_PHRASES[ident]) for t in texts)

    def peek():
        return tokens[pos] if pos < len(tokens) else None

    def take():
        nonlocal pos
        pos += 1
        return tokens[pos - 1]

    def atom():
        if peek() == "(":
            take()
            v = disj()
            if take() != ")":
                fail(f"unbalanced licence expression {expr!r}")
            return v
        v = has(take())
        if peek() == "WITH":
            take()
            v = has(take()) and v
        return v

    def conj():
        v = atom()
        while peek() == "AND":
            take()
            v = atom() and v
        return v

    def disj():
        v = conj()
        while peek() == "OR":
            take()
            v = conj() or v
        return v

    result = disj()
    if pos != len(tokens):
        fail(f"cannot parse licence expression {expr!r}")
    return result


def normalised_text(path):
    return " ".join(path.read_text(errors="replace").split()).lower()


# ---------------------------------------------------------------------------------------------
# ELF / ar helpers (pure Python: zig's ar member names are full paths, so `ar x` by name fails)


def ar_members(path):
    data = Path(path).read_bytes()
    if not data.startswith(b"!<arch>\n"):
        fail(f"{path} is not an ar archive")
    pos, longnames = 8, b""
    while pos + 60 <= len(data):
        header = data[pos : pos + 60]
        name = header[:16].decode().rstrip()
        size = int(header[48:58].decode().strip())
        body = data[pos + 60 : pos + 60 + size]
        pos += 60 + size + (size & 1)
        if name == "//":
            longnames = body
            continue
        if name in ("/", "/SYM64/"):
            continue
        if name.startswith("/") and name[1:].isdigit():
            off = int(name[1:])
            name = longnames[off : longnames.index(b"\n", off)].decode()
        yield name.rstrip("/"), body


def elf_symbol_string(obj, symbol):
    """The NUL-terminated string a defined symbol points at, in a relocatable ELF64 LE object."""
    if obj[:4] != b"\x7fELF" or obj[4] != 2 or obj[5] != 1:
        fail("expected a 64-bit little-endian ELF object")
    shoff, = struct.unpack_from("<Q", obj, 0x28)
    shentsize, shnum = struct.unpack_from("<HH", obj, 0x3A)
    sections = [struct.unpack_from("<IIQQQQIIQQ", obj, shoff + i * shentsize) for i in range(shnum)]
    for sh in sections:
        if sh[1] != 2:  # SHT_SYMTAB
            continue
        strtab = sections[sh[6]]
        for off in range(sh[4], sh[4] + sh[5], sh[9]):
            st_name, _, _, st_shndx, st_value, _ = struct.unpack_from("<IBBHQQ", obj, off)
            name_at = strtab[4] + st_name
            name = obj[name_at : obj.index(b"\0", name_at)].decode()
            if name == symbol and 0 < st_shndx < shnum:
                at = sections[st_shndx][4] + st_value
                return obj[at : obj.index(b"\0", at)].decode()
    fail(f"symbol {symbol} not found")


# ---------------------------------------------------------------------------------------------
# Toolchain


def pins():
    toolchain = tomllib.loads((REPO / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    dockerfile = (REPO / ".devcontainer/Dockerfile").read_text()
    zig = re.search(r"^ARG ZIG_VERSION=(\S+)", dockerfile, re.M).group(1)
    zigbuild = re.search(r"cargo-zigbuild@(\S+)", dockerfile).group(1)
    return toolchain, zig, zigbuild


def toolchain_info():
    rust_pin, zig_pin, zigbuild_pin = pins()
    rustc = run("rustc", "-vV")
    rustc_version = re.search(r"^release: (\S+)", rustc, re.M).group(1)
    zig_version = run("zig", "version").strip()
    zigbuild_version = run("cargo-zigbuild", "--version").split()[-1]
    clang = run("zig", "cc", "--version").splitlines()[0]
    checks = [
        ("rustc", rustc_version, rust_pin),
        ("zig", zig_version, zig_pin),
        ("cargo-zigbuild", zigbuild_version, zigbuild_pin),
    ]
    for name, have, want in checks:
        if have != want:
            fail(f"{name} is {have}, but the pin is {want}")
    if rust_pin not in RUST_SOURCES:
        fail(f"no Rust source URL recorded for rustc {rust_pin}: extend RUST_SOURCES")
    if zig_pin not in ZIG_BUNDLED:
        fail(f"zig {zig_pin}'s bundled musl/libunwind are not recorded: extend ZIG_BUNDLED")
    if f"clang version {ZIG_BUNDLED[zig_pin]['clang']}" not in clang:
        fail(f"zig cc reports {clang!r}, expected clang {ZIG_BUNDLED[zig_pin]['clang']}")
    sysroot = Path(run("rustc", "--print", "sysroot").strip())
    return {
        "rustc": rustc.splitlines()[0],
        "rustc_version": rustc_version,
        "cargo": run("cargo", "--version").strip(),
        "zig": zig_version,
        "zig_cc": clang,
        "zigbuild": zigbuild_version,
        "sysroot": sysroot,
        "library": sysroot / "lib/rustlib/src/rust/library",
        "zig_lib": Path(json.loads(run("zig", "env"))["lib_dir"]),
    }


# ---------------------------------------------------------------------------------------------
# Crates


def app_crates(triple):
    """Packages that ship for `triple`: cargo tree over normal edges, proc-macros excluded."""
    tree = run(
        "cargo", "tree", "--locked", "-e", "normal,no-proc-macro", "--target", triple,
        "--prefix", "none", "--format", "{p}", cwd=REPO,
    )
    shipped = set()
    for line in tree.splitlines():
        m = re.match(r"^(\S+) v(\S+)", line)
        if m:
            shipped.add((m.group(1), m.group(2)))
    meta = json.loads(run(
        "cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", triple,
        cwd=REPO,
    ))
    lock = {
        (p["name"], p["version"]): p.get("checksum")
        for p in tomllib.loads((REPO / "Cargo.lock").read_text())["package"]
    }
    crates, lib_names = {}, {}
    for p in meta["packages"]:
        key = (p["name"], p["version"])
        if key not in shipped:
            continue
        for t in p["targets"]:
            if "lib" in t["kind"] or "rlib" in t["kind"]:
                lib_names[t["name"].replace("-", "_")] = p["name"]
        crates[p["name"]] = {
            "name": p["name"],
            "version": p["version"],
            "licence": p["license"] and normalise_licence(p["license"]),
            "dir": Path(p["manifest_path"]).parent,
            "checksum": lock.get(key),
            "local": p["source"] is None,
        }
    if len(crates) != len(shipped):
        fail(f"cargo tree and cargo metadata disagree for {triple}")
    return crates, lib_names


def sysroot_crates(library):
    """name -> crate info for everything in the toolchain's library workspace."""
    lock = tomllib.loads((library / "Cargo.lock").read_text())["package"]
    in_tree = {}
    for manifest in library.glob("**/Cargo.toml"):
        if "vendor" in manifest.relative_to(library).parts:
            continue
        pkg = tomllib.loads(manifest.read_text()).get("package", {})
        lic = pkg.get("license")
        if "name" in pkg and isinstance(lic, str):
            in_tree.setdefault(pkg["name"], []).append((manifest.parent, normalise_licence(lic)))
    crates = {}
    for p in lock:
        name, version = p["name"], p["version"]
        info = {"name": name, "version": version, "checksum": p.get("checksum")}
        if p.get("source"):
            vendored = library / "vendor" / f"{name}-{version}"
            if not vendored.is_dir():
                continue
            pkg = tomllib.loads((vendored / "Cargo.toml").read_text())["package"]
            recorded = json.loads((vendored / ".cargo-checksum.json").read_text())["package"]
            if recorded != p["checksum"]:
                fail(f"vendored {name} {version} checksum {recorded} != lockfile {p['checksum']}")
            info.update(dir=vendored, licence=normalise_licence(pkg["license"]), vendored=True)
        else:
            found = in_tree.get(name, [])
            if not found:
                continue
            licences = {lic for _, lic in found}
            if len(licences) != 1:
                fail(f"in-tree crate {name} has conflicting licences {licences}")
            info.update(dir=found[0][0], licence=licences.pop(), vendored=False)
        crates[name.replace("-", "_")] = info
    return crates


def root_licence_files(directory):
    return sorted(
        f for f in directory.iterdir() if f.is_file() and LICENCE_FILE.match(f.name)
    )


def in_tree_licence_files(crate_dir, library):
    """Licence files at the crate root, else the nearest enclosing directory below library/."""
    d = crate_dir
    while d != library and library in d.parents:
        files = root_licence_files(d)
        if files:
            return files
        d = d.parent
    return []


def ijg_notice(crate_dir):
    """jpeg-encoder declares IJG but ships no IJG file; its fdct.rs header carries the terms."""
    lines = (crate_dir / "src/fdct.rs").read_text().splitlines(keepends=True)
    for i, line in enumerate(lines):
        if "assumed by the product vendor." in line:
            return "".join(lines[: i + 1])
    fail("jpeg-encoder src/fdct.rs no longer carries the IJG notice")


# ---------------------------------------------------------------------------------------------
# The link


def final_link(log, deps_dir):
    pattern = re.compile(r"^\s*ld\.lld (.*\s-o (\S+/deps/fpx_convert-[0-9a-f]+)\s.*)$")
    found = [m for m in map(pattern.match, log.read_text().splitlines()) if m]
    if len(found) != 1:
        fail(f"{log}: expected one final ld.lld line, found {len(found)}")
    args = found[0].group(1).split()
    if Path(found[0].group(2)).parent.resolve() != deps_dir.resolve():
        fail(f"{log}: final link writes outside {deps_dir}")
    return args


def replay_with_map(args, binary, work):
    out, map_file = work / "replay", work / "link.map"
    replay = list(args)
    replay[replay.index("-o") + 1] = str(out)
    run("zig", "ld.lld", *replay, f"-Map={map_file}")
    if out.read_bytes() != binary.read_bytes():
        fail(f"replayed link of {binary} is not byte-identical: the map does not describe it")
    return map_file


def map_contributions(map_file):
    """Input file -> bytes of its sections placed in the output (lld map format)."""
    sizes = {}
    row = re.compile(r"^\s*[0-9a-f]+\s+[0-9a-f]+\s+([0-9a-f]+)\s+\d+ {9}(\S.*)$")
    for line in map_file.read_text().splitlines():
        m = row.match(line)
        if not m:
            continue
        source = m.group(2)
        cut = source.rfind(":(")
        source = source[:cut] if cut != -1 else source
        am = re.match(r"^(.*?\.(?:a|rlib))\(.*\)$", source)
        source = am.group(1) if am else source
        sizes[source] = sizes.get(source, 0) + int(m.group(1), 16)
    return sizes


# ---------------------------------------------------------------------------------------------
# One architecture


def record_arch(out, triple, arch, target_dir, log, tc, version, commit, archive):
    deps = target_dir / triple / "release" / "deps"
    binary = target_dir / triple / "release" / "fpx-convert"
    work = target_dir / "licence-record" / arch
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)

    dynamic = run("readelf", "-d", str(binary))
    if "There is no dynamic section in this file." not in dynamic:
        fail(f"{binary} has a dynamic section: it is not fully static")

    args = final_link(log, deps)
    contributions = map_contributions(replay_with_map(args, binary, work))
    zig_pin = tc["zig"]
    musl_ver, musl_url, musl_sha = ZIG_BUNDLED[zig_pin]["musl"]
    unwind_ver, unwind_url, unwind_sha = ZIG_BUNDLED[zig_pin]["libunwind"]

    app, lib_names = app_crates(triple)
    sysroot = sysroot_crates(tc["library"])
    sys_lib_dir = tc["sysroot"] / "lib/rustlib" / triple / "lib"
    zig_cache = Path.home() / ".cache/zig"

    # Classify every input on the link line, and every input the map says contributes bytes.
    used_sys, labels, musl_libc = set(), {}, None
    inputs, takes_value = [], {"-o", "-L", "-m", "-z", "--entry"}
    for i, a in enumerate(args):
        if not a.startswith("-") and args[i - 1] not in takes_value:
            inputs.append(a)
    for path in inputs:
        p = Path(path)
        if p.suffix == ".rlib":
            crate = re.match(r"^lib(.+)-[0-9a-f]+\.rlib$", p.name).group(1)
            if p.parent.resolve() == deps.resolve():
                if crate not in lib_names:
                    fail(f"{p.name} is linked but not in the crate list for {triple}")
                labels[path] = f"crate {lib_names[crate]}"
            elif p.parent == sys_lib_dir:
                if crate not in sysroot:
                    fail(f"sysroot {p.name} is linked but not in the toolchain's Cargo.lock")
                used_sys.add(crate)
                labels[path] = f"rust sysroot {sysroot[crate]['name']}"
            else:
                fail(f"rlib from an unexpected place is linked: {path}")
        elif p.parent.resolve() == deps.resolve() and re.match(r"^fpx_convert-.*\.rcgu\.o$", p.name):
            labels[path] = "fpx-convert objects"
        elif p.name == "symbols.o" and p.parent.parent.resolve() == deps.resolve():
            labels[path] = "rustc symbols.o"
        elif zig_cache in p.parents and p.name in ("crt1.o", "crti.o", "crtn.o", "libc.a"):
            labels[path] = f"zig musl {p.name}"
            if p.name == "libc.a":
                musl_libc = p
        elif zig_cache in p.parents and p.name == "libunwind.a":
            labels[path] = "zig libunwind.a"
        elif zig_cache in p.parents and p.name == "libcompiler_rt.a":
            labels[path] = "zig libcompiler_rt.a"
        else:
            labels[path] = f"unrecognised {p.name}"
    for source, size in contributions.items():
        if source == "<internal>":
            continue
        label = labels.get(source)
        if label is None:
            fail(f"{triple}: map shows {size} bytes from {source}, which is not on the link line")
        if size and (label.startswith("unrecognised") or label == "zig libcompiler_rt.a"):
            fail(f"{triple}: {source} contributes {size} bytes and the record does not cover it")

    if musl_libc is None:
        fail(f"{triple}: zig's musl libc.a is not on the link line")
    versions = [
        elf_symbol_string(body, "__libc_version")
        for name, body in ar_members(musl_libc)
        if name.endswith("/version.o") or name == "version.o"
    ]
    if versions != [musl_ver]:
        fail(f"{triple}: linked libc.a reports musl {versions}, the record names {musl_ver}")

    # --- entries and licence texts
    third = out / "third-party" / f"fpx-convert-{arch}"
    lic_dir, src_dir = third / "licenses", third / "source"
    lic_dir.mkdir(parents=True)
    src_dir.mkdir()
    entries, texts = [], {}

    def add_text(component, name, content):
        dest = lic_dir / name
        if dest.exists() and dest.read_bytes() != content:
            fail(f"two different licence texts want the name {name}")
        dest.write_bytes(content)
        texts.setdefault(component, []).append(dest)

    def add_files(component, prefix, files):
        for f in files:
            add_text(component, f"{prefix}-{f.name}", f.read_bytes())

    def entry(component, ver, licence, sha, source):
        if not licence or not ver or not sha or not re.fullmatch(r"[0-9a-f]{64}", sha):
            fail(f"{component}: cannot be pinned to an exact version, licence and source")
        line = (component, f"{ver} (statically linked)", licence, arch, sha, source)
        # The app and std can each link their own copy of the same crate version: one line.
        if line not in entries:
            entries.append(line)

    own = app.pop("fpx-convert")
    if own["licence"] != "AGPL-3.0-or-later":
        fail(f"Cargo.toml licence is {own['licence']}, spec 0002 says AGPL-3.0-or-later")
    shutil.copy(archive, src_dir / archive.name)
    entry("fpx-convert", version, own["licence"], sha256(archive), archive.name)
    add_text("fpx-convert", "fpx-convert-LICENSE", (REPO / "LICENSE").read_bytes())

    for name in sorted(app):
        c = app[name]
        if c["local"]:
            fail(f"{name} is a local path dependency: it has no crates.io source")
        entry(name, c["version"], c["licence"], c["checksum"], f"crate:{name} {c['version']}")
        add_files(name, name, root_licence_files(c["dir"]))
        if name == "jpeg-encoder":
            add_text(name, "jpeg-encoder-src-fdct.rs-IJG-notice", ijg_notice(c["dir"]).encode())

    rust_url, rust_sha = RUST_SOURCES[tc["rustc_version"]]
    std_parts = sorted(
        sysroot[c]["name"] for c in used_sys
        if not sysroot[c]["vendored"] and sysroot[c]["licence"] == STD_LICENCE
    )
    doc = tc["sysroot"] / "share/doc/rust"
    entry("Rust standard library", tc["rustc_version"], STD_LICENCE, rust_sha, f"url:{rust_url}")
    add_files("Rust standard library", "rust",
              [doc / "COPYRIGHT-library.html", doc / "licenses/MIT.txt",
               doc / "licenses/Apache-2.0.txt"])
    for crate in sorted(used_sys, key=lambda c: sysroot[c]["name"]):
        c = sysroot[crate]
        if c["vendored"]:
            entry(c["name"], c["version"], c["licence"], c["checksum"],
                  f"crate:{c['name']} {c['version']}")
            add_files(c["name"], c["name"], root_licence_files(c["dir"]))
        elif c["licence"] != STD_LICENCE:
            entry(c["name"], c["version"], c["licence"], rust_sha, f"url:{rust_url}")
            add_files(c["name"], c["name"], in_tree_licence_files(c["dir"], tc["library"]))

    entry("musl", musl_ver, MUSL_LICENCE, musl_sha, f"url:{musl_url}")
    add_files("musl", "musl", [tc["zig_lib"] / "libc/musl/COPYRIGHT"])
    entry("libunwind", unwind_ver, LIBUNWIND_LICENCE, unwind_sha, f"url:{unwind_url}")
    add_files("libunwind", "libunwind", [tc["zig_lib"] / "libunwind/LICENSE.TXT"])

    for e in entries:
        comp, licence = e[0], e[2]
        files = [normalised_text(f) for f in texts.get(comp, [])]
        if not licence_covered(licence, files):
            fail(f"{arch}: {comp} is {licence}, and its licence files do not contain that text")
        if any("\t" in field or "\n" in field for field in e):
            fail(f"{comp}: a field contains a tab or newline")

    (third / "entries.tsv").write_text("".join("\t".join(e) + "\n" for e in entries))
    ship = out / "fpx-convert" / arch / "fpx-convert"
    ship.parent.mkdir(parents=True)
    shutil.copy2(binary, ship)

    summary = {}
    for path, label in labels.items():
        summary[label] = summary.get(label, 0) + contributions.get(path, 0)
    return {
        "arch": arch,
        "triple": triple,
        "sha256": sha256(ship),
        "std_parts": std_parts,
        "musl": musl_ver,
        "inputs": summary,
        "entries": len(entries),
    }


def main(argv):
    if len(argv) < 2:
        sys.exit(__doc__)
    out = Path(argv[0])
    tc = toolchain_info()
    meta = json.loads(run("cargo", "metadata", "--no-deps", "--format-version", "1", cwd=REPO))
    version = next(p["version"] for p in meta["packages"] if p["name"] == "fpx-convert")
    if run("git", "status", "--porcelain", cwd=REPO).strip():
        fail("the working tree is not clean: a release must be built from a commit")
    commit = run("git", "rev-parse", "HEAD", cwd=REPO).strip()
    name = f"fpx-convert-{version}-{commit[:12]}"

    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    archive = out.parent / f".{name}.tar.gz"
    run("git", "archive", "--format=tar.gz", f"--prefix={name}/",
        f"--output={archive}", commit, cwd=REPO)
    archive = archive.rename(archive.with_name(f"{name}.tar.gz"))

    results = []
    try:
        for spec in argv[1:]:
            triple, arch, target_dir, log = spec.split(":")
            results.append(record_arch(out, triple, arch, Path(target_dir), Path(log), tc,
                                       version, commit, archive))
    finally:
        archive.unlink()

    lines = [
        f"fpx-convert {version}",
        f"commit          {commit}",
        "",
        "Toolchain (each checked against its pin):",
        f"  rustc           {tc['rustc']}",
        f"  cargo           {tc['cargo']}",
        f"  zig             {tc['zig']} ({tc['zig_cc']})",
        f"  cargo-zigbuild  {tc['zigbuild']}",
        "",
        "Binaries:",
    ]
    lines += [f"  fpx-convert/{r['arch']}/fpx-convert  {r['triple']}  sha256 {r['sha256']}"
              for r in results]
    lines += [
        "",
        "Link evidence (spec 0002, \"Proving what the final link used\"): each binary has no",
        "dynamic section (readelf -d); zig's final ld.lld command, replayed with -Map, gave a",
        "byte-identical binary; the map's per-input byte counts are below. Inputs with 0 bytes",
        "were given to the linker but contributed nothing.",
    ]
    for r in results:
        lines += ["", f"{r['arch']} ({r['triple']}): musl __libc_version {r['musl']}, "
                      f"{r['entries']} entries"]
        lines.append(f"  Rust standard library line covers: {', '.join(r['std_parts'])}")
        width = max(len(k) for k in r["inputs"])
        lines += [f"  {label:<{width}}  {size:>9}" for label, size in sorted(r["inputs"].items())]
    (out / "BUILD_INFO.txt").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except (RecordError, subprocess.CalledProcessError) as e:
        detail = e.stderr if isinstance(e, subprocess.CalledProcessError) else ""
        sys.exit(f"licence-record: FAILED: {e}\n{detail}")
