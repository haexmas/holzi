#!/usr/bin/env python3
"""Validate the repository's documentation-only CI contract."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[2]
DOC_ROOT = ROOT / "specs" / "001-frontend-onboarding"

REQUIRED_FILES = (
    DOC_ROOT / "plan.md",
    DOC_ROOT / "research.md",
    DOC_ROOT / "spec.md",
    DOC_ROOT / "tasks.md",
)

ANDROID_RESEARCH_REQUIREMENT = (
    "OS `CAMERA` runtime grant"
    " and the WebView's `PermissionRequest` handler allows only"
    " `RESOURCE_VIDEO_CAPTURE`"
)


def check_required_files(errors: list[str]) -> None:
    for path in REQUIRED_FILES:
        if not path.is_file():
            errors.append(f"missing required document: {path.relative_to(ROOT)}")


def check_qr_permission_contract(errors: list[str]) -> None:
    research = DOC_ROOT / "research.md"
    tasks = DOC_ROOT / "tasks.md"

    if research.is_file() and ANDROID_RESEARCH_REQUIREMENT not in research.read_text():
        errors.append(
            "research.md must document Android's runtime CAMERA grant and "
            "RESOURCE_VIDEO_CAPTURE PermissionRequest handling"
        )

    if tasks.is_file():
        task_text = tasks.read_text()
        required_task_terms = (
            "android.permission.CAMERA",
            "PermissionRequest",
            "RESOURCE_VIDEO_CAPTURE",
        )
        missing = [term for term in required_task_terms if term not in task_text]
        if missing:
            errors.append(
                "tasks.md is missing Android camera requirements: "
                + ", ".join(missing)
            )


def check_local_markdown_links(errors: list[str]) -> None:
    link_pattern = re.compile(r"(?<!!)\[[^]]+\]\(([^)]+)\)")
    for document in ROOT.rglob("*.md"):
        if ".git" in document.parts:
            continue
        for raw_target in link_pattern.findall(document.read_text()):
            target = raw_target.strip().split(maxsplit=1)[0].strip("<>")
            if not target or re.match(r"^[a-z][a-z0-9+.-]*:", target) or target.startswith("#"):
                continue
            # These links intentionally point at the sibling haex-vault repository,
            # which is not checked out in this repository's CI workspace.
            if target.startswith("../../../../haex-vault/"):
                continue
            target_path = (document.parent / target.split("#", 1)[0]).resolve()
            if not target_path.exists():
                errors.append(
                    f"broken local Markdown link in {document.relative_to(ROOT)}: {target}"
                )


def main() -> int:
    errors: list[str] = []
    check_required_files(errors)
    check_qr_permission_contract(errors)
    check_local_markdown_links(errors)

    if errors:
        print("Documentation checks failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("Documentation checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
