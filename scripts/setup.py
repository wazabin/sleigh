#!/usr/bin/env python3

"""
setup.py — Populate the SLEIGH spec tree from vanilla Ghidra.

By default the repo gets its SLEIGH specs from the `open_sleigh` git submodule
(`just setup`). This script is the *alternative* source: it overlays the spec
tree with files straight from upstream Ghidra, for anyone who prefers the
vanilla specs over open_sleigh's community ones.

It either copies the language files from an existing local Ghidra installation,
or downloads a pinned Ghidra release from GitHub and extracts just what's
needed. Files are written into the same layout the build expects
(precompile/open_sleigh/src/<processor>/), overwriting whatever
the submodule provided for those processors.

Re-runnable: replaces each installed processor directory wholesale.

Requires: Python 3.8+. Uses only the standard library.
"""
import argparse
import shutil
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
from pathlib import Path
from typing import List, Optional


# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

DEFAULT_GHIDRA_VERSION = "12.0.4"

# Inside a local Ghidra installation, the same relative layout applies.
PROCESSORS_SUBPATH = Path("Ghidra/Processors")
LANGUAGES_LEAF = Path("data/languages")

REPO_ROOT = Path(__file__).resolve().parent.parent
SLA_ROOT = REPO_ROOT / "precompile" / "open_sleigh" / "src"


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def ghidra_tag(version: str):
    return f"Ghidra_{version}_build"


def ghidra_url(version: str):
    return (
        "https://github.com/NationalSecurityAgency/ghidra/archive/refs/tags/"
        f"{ghidra_tag(version)}.tar.gz"
    )


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args():

    parser = argparse.ArgumentParser(
        prog=Path(sys.argv[0]).name,
        description="Install vanilla Ghidra SLA language files into precompile/open_sleigh/src/<processor>.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
Examples:
  Interactive (downloads Ghidra, prompts for processors):
    %(prog)s

  Use a local Ghidra install, prompt for processors:
    %(prog)s --local-ghidra ~/tools/ghidra_12.0.4_PUBLIC

  Fully non-interactive, download default version:
    %(prog)s --processors x86,ARM

  Install every available processor from a local copy:
    %(prog)s --local-ghidra ~/tools/ghidra_12.0.4_PUBLIC --processors all

  Download a specific Ghidra version:
    %(prog)s --version 11.3.2 --processors x86

  Find available version tags:
    https://github.com/NationalSecurityAgency/ghidra/tags
""",
    )

    parser.add_argument(
        "-l",
        "--local-ghidra",
        metavar="PATH",
        type=Path,
        help=(
            "Copy from an existing Ghidra install at PATH. PATH should be "
            "the Ghidra root (the directory containing the 'Ghidra/' "
            "subdirectory)."
        ),
    )

    parser.add_argument(
        "-p",
        "--processors",
        metavar="NAMES",
        help=(
            "Processor language to install (e.g. x86, ARM, MIPS). "
            "Case-sensitive; must match a subdirectory under "
            "Ghidra/Processors/. If omitted, the script lists available "
            "processors and prompts."
        ),
    )

    parser.add_argument(
        "-v",
        "--version",
        default=DEFAULT_GHIDRA_VERSION,
        help=(
            f"Ghidra version to download (default: {DEFAULT_GHIDRA_VERSION}). "
            "Ignored when --local-ghidra is provided. Must match a tag on "
            "https://github.com/NationalSecurityAgency/ghidra/tags "
            "(e.g. 12.0.4, 11.3.2)."
        ),
    )

    return parser.parse_args()


def download_ghidra(dest_dir: Path, version: str):

    url = ghidra_url(version)

    tarball = dest_dir / f"{ghidra_tag(version)}.tar.gz"
    print(f"- Downloading {url}")

    def reporthook(block_num: int, block_size: int, total_size: int) -> None:
        downloaded = min(block_num * block_size, total_size)
        mb_done = downloaded / (1024 * 1024)
        if total_size <= 0:
            # Unknown size — show a spinner.
            spinner = "|/-\\"[block_num % 4]
            sys.stderr.write(f"\r  {spinner} downloading")
        else:
            frac = downloaded / total_size
            bar_len = 28
            filled = int(bar_len * frac)
            bar = "#" * filled + "-" * (bar_len - filled)
            mb_total = total_size / (1024 * 1024)
            sys.stderr.write(
                f"\r  {bar} {frac*100:5.1f}%  "
                f"{f'{mb_done:6.1f} / {mb_total:6.1f} MiB'}"
            )
        sys.stderr.flush()

    try:
        urllib.request.urlretrieve(url, tarball, reporthook)
    except urllib.error.HTTPError as e:
        sys.stderr.write("\n")
        if e.code == 404:
            sys.exit(
                f"ERROR: Ghidra version '{version}' not found at {url}\n"
                "Check the version string against "
                "https://github.com/NationalSecurityAgency/ghidra/tags"
            )
        raise
    finally:
        sys.stderr.write("\n")

    print("- Extracting archives…")

    with tarfile.open(tarball, "r:gz") as tf:
        if sys.version_info >= (3, 12):
            tf.extractall(dest_dir, filter="data")
        else:
            tf.extractall(dest_dir)
    return dest_dir / f"ghidra-{ghidra_tag(version)}"


def discover_processors(ghidra_root: Path) -> List[str]:
    processors_dir = ghidra_root / PROCESSORS_SUBPATH

    if not processors_dir.is_dir():
        sys.exit(f"ERROR: Not a Ghidra root (missing {processors_dir})")

    names = [
        c.name
        for c in sorted(processors_dir.iterdir())
        if c.is_dir() and (c / LANGUAGES_LEAF).is_dir()
    ]

    if not names:
        sys.exit(f"ERROR: No processors found under {processors_dir}")
    return names


def prompt_processors(available: List[str]) -> List[str]:
    print("\nAvailable processors:")
    for i, name in enumerate(available, start=1):
        print(f" {i:>2}) {name:10}", end="")
        if i % 6 == 0 or i == len(available):
            print()

    print("\nEnter numbers or names separated by commas, or 'all'.")
    while True:
        reply = input("Processors: ").strip()
        if not reply:
            continue
        if reply.lower() == "all":
            return available

        chosen = []
        ok = True

        for token in (t.strip() for t in reply.split(",")):
            if not token:
                continue
            if token.isdigit() and 1 <= int(token) <= len(available):
                chosen.append(available[int(token) - 1])
            elif token in available:
                chosen.append(token)
            else:
                print(f"   '{token}' is not a valid choice.")
                ok = False
                break

        if ok and chosen:
            seen = set()
            return [p for p in chosen if not (p in seen or seen.add(p))]


def install_processor(ghidra_root: Path, processor: str):
    src = ghidra_root / PROCESSORS_SUBPATH / processor / LANGUAGES_LEAF

    if not src.is_dir():
        sys.exit(f"ERROR: {src} not found")

    dst = SLA_ROOT / processor
    print(f"- Installing {processor} → {dst}")

    if dst.exists():
        shutil.rmtree(dst)
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(src, dst, symlinks=False)

    old = dst / "old"
    if old.exists():
        shutil.rmtree(old)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    args = parse_args()

    if args.local_ghidra is not None:
        work_dir: Optional[Path] = None
        ghidra_root = args.local_ghidra
    else:
        work_dir = Path(tempfile.mkdtemp(prefix="ghidra-sla"))
        ghidra_root = download_ghidra(work_dir, args.version)

    chosen = None
    try:
        available = discover_processors(ghidra_root)

        if args.processors:
            if args.processors.strip().lower() == "all":
                chosen = available
            else:
                chosen = [x.strip() for x in args.processors.split(",") if x.strip()]
                unknown = [x for x in chosen if x not in available]
                if unknown:
                    sys.exit(
                        f"ERROR: Unknown processors: {', '.join(unknown)}\n"
                        f"Available: {', '.join(available)}"
                    )
        else:
            chosen = prompt_processors(available)

        print()
        for proc in chosen:
            install_processor(ghidra_root, proc)
        print()

    finally:
        if work_dir is not None:
            shutil.rmtree(work_dir)

        if chosen is not None:
            print(f"✓ Installed {len(chosen)} processor(s):")
            for processor in chosen:
                print(f"  • {processor} → {SLA_ROOT / processor}")
        else:
            print("No processors installed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
