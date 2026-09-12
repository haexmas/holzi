"""Regression coverage for repository-owned Markdown selection."""

from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


class DocumentationChecksTest(unittest.TestCase):
    def test_ignored_docs_are_excluded_without_skipping_owned_or_required_docs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "--quiet", str(root)], check=True)
            script = root / "scripts" / "ci" / "check-docs.py"
            script.parent.mkdir(parents=True)
            shutil.copyfile(Path(__file__).with_name("check-docs.py"), script)
            required = root / "specs" / "001-frontend-onboarding"
            required.mkdir(parents=True)
            for name in ("plan", "research", "spec", "tasks"):
                (required / f"{name}.md").write_text("# Required document\n")

            (root / ".gitignore").write_text("node_modules/\ngenerated/\n")
            (root / "tracked.md").write_text("[tracked](missing.md)\n")
            subprocess.run(["git", "add", "tracked.md"], cwd=root, check=True)
            (root / "untracked.md").write_text("[new](missing.md)\n")
            for name in ("node_modules", "generated"):
                (root / name).mkdir()
                (root / name / "ignored.md").write_text("[vendored](absent.md)\n")

            result = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            self.assertIn("broken local Markdown link in tracked.md", result.stderr)
            self.assertIn("broken local Markdown link in untracked.md", result.stderr)
            self.assertNotIn("ignored.md", result.stderr)

            # Even an ignored directory must be checked if its file was tracked.
            subprocess.run(["git", "add", "--force", "generated/ignored.md"], cwd=root, check=True)
            result = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertIn("broken local Markdown link in generated/ignored.md", result.stderr)
            subprocess.run(["git", "rm", "--cached", "generated/ignored.md"], cwd=root, check=True, capture_output=True)

            (root / "missing.md").write_text("# Link target\n")
            result = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

            (required / "tasks.md").unlink()
            result = subprocess.run([sys.executable, str(script)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn("missing required document: specs/001-frontend-onboarding/tasks.md", result.stderr)


if __name__ == "__main__":
    unittest.main()
