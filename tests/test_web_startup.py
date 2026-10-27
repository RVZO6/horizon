"""The browser page's bootstrap checks (web/startup.test.mjs)."""
from pathlib import Path
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


class WebStartupTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which("node"), "Node is needed for the page's JavaScript tests")
    def test_startup_checks(self):
        subprocess.run(["node", "--test", str(ROOT / "web/startup.test.mjs")], check=True)


if __name__ == "__main__":
    unittest.main()
