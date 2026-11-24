"""Check the repository's relative markdown links.

Every relative link in a markdown file must point at a file that exists.
External links (http, https, mailto) are not fetched — the check is about
the repository's own links, which is what breaks when a file is renamed.

Usage: python scripts/check_links.py [repo-root]
Exit code 0 when every link resolves, 1 otherwise.
"""

import re
import sys
from pathlib import Path

LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
SKIP_DIRS = {".git", "target", "book", "node_modules", ".superpowers"}


def markdown_files(root: Path) -> list[Path]:
    return [
        path
        for path in root.rglob("*.md")
        if not any(part in SKIP_DIRS for part in path.relative_to(root).parts)
    ]


def is_external(target: str) -> bool:
    return target.startswith(("http://", "https://", "mailto:", "#", "/"))


def main() -> int:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
    broken: list[str] = []

    for path in markdown_files(root):
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in LINK.finditer(text):
            target = match.group(1)
            if is_external(target):
                continue
            clean = target.split("#", 1)[0]
            if not clean:
                continue
            resolved = (path.parent / clean).resolve()
            if not resolved.exists():
                broken.append(f"{path.relative_to(root)}: {target}")

    if broken:
        print("Broken relative links:")
        for line in broken:
            print(f"  {line}")
        print(f"\n{len(broken)} broken link(s).")
        return 1

    print("All relative markdown links resolve.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
