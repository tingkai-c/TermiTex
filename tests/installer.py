#!/usr/bin/env python3
"""Offline installer lifecycle tests. All writes stay in a temporary home."""
import fcntl
import hashlib
import io
import os
from pathlib import Path
import pty
import select
import shutil
import subprocess
import sys
import tarfile
import tempfile
import termios
import time
import unittest

INSTALLER = Path(__file__).resolve().parents[1] / "install.sh"


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="termitex-installer-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "home with 'quote $cash `literal`"
        self.home.mkdir()
        self.bin = self.home / ".local/bin"
        self.state = self.home / ".local/share/termitex/install"
        self.config = self.home / ".config/termitex"
        self.stub = self.root / "stubs"
        self.stub.mkdir()
        self.releases = self.root / "releases"
        self.releases.mkdir()
        self.env = dict(os.environ)
        for key in ("XDG_DATA_HOME", "XDG_CONFIG_HOME", "ZDOTDIR", "CI", "BASH_ENV", "ENV"):
            self.env.pop(key, None)
        self.env.update(HOME=str(self.home), SHELL="/bin/bash", PATH=f"{self.stub}:/usr/bin:/bin:/usr/sbin:/sbin",
                        TEST_RELEASES=str(self.releases), TEST_LATEST="0.2.2", TEST_OS="Linux", TEST_ARCH="x86_64",
                        TEST_DOWNLOAD_LOG=str(self.root / "downloads"))
        self.write_executable(self.stub / "uname", '#!/bin/sh\ncase "$1" in -s) echo "$TEST_OS";; -m) echo "$TEST_ARCH";; esac\n')
        self.write_executable(self.stub / "curl", f'''#!{sys.executable}
import os, pathlib, shutil, sys
args=sys.argv[1:]
url=next(a for a in args if a.startswith("https://"))
with open(os.environ["TEST_DOWNLOAD_LOG"], "a") as log: log.write(url+"\\n")
if url.endswith("/latest"):
    print("https://github.com/tingkai-c/TermiTex/releases/tag/v"+os.environ["TEST_LATEST"], end="")
else:
    version, name=url.rsplit("/",2)[-2:]
    source=pathlib.Path(os.environ["TEST_RELEASES"])/version/name
    if not source.exists(): sys.exit(22)
    shutil.copyfile(source,args[args.index("-o")+1])
''')
        self.release("0.2.2")
        self.release("0.2.3")

    def write_executable(self, path, body):
        path.write_text(body)
        path.chmod(0o755)

    def release(self, version, platform="linux", arch="x86_64", broken=False):
        directory = self.releases / f"v{version}"
        directory.mkdir(exist_ok=True)
        archive = directory / f"termitex-{platform}-{arch}.tar.gz"
        binary = ('#!/bin/sh\necho "incompatible loader" >&2\nexit 1\n' if broken else
                  f'#!/bin/sh\nif [ "$1" = --version ]; then echo "termitex {version}"; else printf "<%s>\\n" "$@"; fi\n')
        with tarfile.open(archive, "w:gz") as tar:
            for name in ("termitex", "termitex/licenses"):
                info = tarfile.TarInfo(name)
                info.type = tarfile.DIRTYPE
                info.mode = 0o755
                tar.addfile(info)
            for name, content, mode in [("termitex/termitex", binary, 0o755),
                                        ("termitex/LICENSE", "MIT\n", 0o644),
                                        ("termitex/licenses/NOTICE", "Third-party notice\n", 0o644)]:
                data = content.encode()
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mode = mode
                tar.addfile(info, io.BytesIO(data))
        with (directory / "SHA256SUMS").open("a") as checks:
            checks.write(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")

    def run_installer(self, *args, ok=True):
        result = subprocess.run(["/bin/sh", str(INSTALLER), *args], env=self.env, capture_output=True, text=True, timeout=15)
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result.stdout + result.stderr

    def test_fresh_current_update_reinstall_and_downgrade(self):
        self.assertIn("install 0.2.2", self.run_installer("--no-modify-shell"))
        self.assertTrue((self.state / "licenses/NOTICE").is_file())
        self.assertFalse((self.home / ".bashrc").exists())
        self.assertIn("already up to date", self.run_installer())
        self.assertIn("reinstall", self.run_installer("--force", "--integrate", "both"))
        self.assertFalse((self.home / ".bashrc").exists())
        self.env["TEST_LATEST"] = "0.2.3"
        self.assertIn("update 0.2.2 -> 0.2.3", self.run_installer("--integrate", "both"))
        self.assertFalse((self.home / ".bashrc").exists())
        self.env["TEST_LATEST"] = "0.2.2"
        self.assertIn("keeping it", self.run_installer())
        self.assertIn("0.2.3 -> 0.2.2", self.run_installer("--version", "v0.2.2"))
        self.assertIn("0.2.2", (self.state / "receipt").read_text())

    def test_update_preserves_shell_config_and_missing_binary_repairs(self):
        rc = self.home / ".bashrc"
        rc.write_text("# existing config\n")
        self.run_installer("--integrate", "both")
        before = rc.read_bytes(), (self.config / "shell.bash").read_bytes()
        backups = list(self.home.glob(".bashrc.termitex-backup.*"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), "# existing config\n")
        self.env["TEST_LATEST"] = "0.2.3"
        self.run_installer()
        self.assertEqual(before, (rc.read_bytes(), (self.config / "shell.bash").read_bytes()))
        (self.bin / "termitex").unlink()
        self.run_installer("--integrate", "claude")
        self.assertEqual(before, (rc.read_bytes(), (self.config / "shell.bash").read_bytes()))

    def test_checksum_failure_and_incompatible_binary_leave_install_intact(self):
        self.run_installer("--no-modify-shell")
        original = (self.bin / "termitex").read_bytes()
        receipt = (self.state / "receipt").read_bytes()
        (self.releases / "v0.2.3/termitex-linux-x86_64.tar.gz").write_bytes(b"corrupt")
        self.assertIn("Checksum mismatch", self.run_installer("--version", "0.2.3", ok=False))
        self.release("0.2.4", broken=True)
        self.assertIn("cannot run", self.run_installer("--version", "0.2.4", ok=False))
        self.assertEqual(original, (self.bin / "termitex").read_bytes())
        self.assertEqual(receipt, (self.state / "receipt").read_bytes())

    def test_damaged_binary_requires_force(self):
        self.run_installer("--no-modify-shell")
        self.write_executable(self.bin / "termitex", "#!/bin/sh\nexit 1\n")
        self.assertIn("--force", self.run_installer(ok=False))
        self.run_installer("--force")
        self.assertIn("already up to date", self.run_installer())

    def test_refuses_unmanaged_and_symlink_installations(self):
        self.bin.mkdir(parents=True)
        self.write_executable(self.bin / "termitex", "#!/bin/sh\necho 'termitex 0.1.0'\n")
        self.assertIn("not installed by this installer", self.run_installer("--force", ok=False))
        (self.bin / "termitex").unlink()
        (self.bin / "termitex").symlink_to(self.root / "missing")
        self.assertIn("symlink", self.run_installer("--force", ok=False))
        self.assertFalse((self.root / "missing").exists())

    def test_other_path_install_and_custom_destination(self):
        self.write_executable(self.stub / "termitex", "#!/bin/sh\necho 'termitex 0.1.0'\n")
        self.assertIn("existing installation", self.run_installer(ok=False))
        destination = self.home / "custom bin"
        self.run_installer("--bin-dir", str(destination), "--no-modify-shell")
        self.assertTrue((destination / "termitex").exists())
        self.assertIn("already up to date", self.run_installer())
        self.assertIn("managed in", self.run_installer("--bin-dir", str(self.bin), ok=False))

    def test_all_platform_assets_and_unsupported_platform(self):
        for system, platform, arch, artifact_arch in [("Darwin", "macos", "arm64", "aarch64"),
                                                       ("Darwin", "macos", "x86_64", "x86_64"),
                                                       ("Linux", "linux", "aarch64", "aarch64")]:
            with self.subTest(system=system, arch=arch):
                self.release("0.2.2", platform, artifact_arch)
                self.env.update(TEST_OS=system, TEST_ARCH=arch)
                self.run_installer("--force", "--no-modify-shell")
                self.assertIn(f"termitex-{platform}-{artifact_arch}.tar.gz", (self.root / "downloads").read_text())
        self.env["TEST_ARCH"] = "riscv64"
        self.assertIn("architectures", self.run_installer(ok=False))

    def test_setup_idempotent_quotes_and_existing_aliases(self):
        self.write_executable(self.stub / "codex", "#!/bin/sh\necho plain-codex\n")
        self.run_installer("--integrate", "both")
        self.run_installer("--setup", "--integrate", "both")
        self.assertEqual((self.home / ".bashrc").read_text().count("# TermiTex"), 1)
        # Exercise alias expansion in a real interactive Bash, with shell-special
        # characters in both the executable and sourced file paths.
        result = subprocess.run(["/bin/bash", "--noprofile", "--rcfile", str(self.home / ".bashrc"), "-ic",
                                 'codex "two words"; claude test; command codex'], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("<codex>\n<two words>", result.stdout)
        self.assertIn("<claude>\n<test>", result.stdout)
        self.assertIn("plain-codex", result.stdout)
        with (self.home / ".bashrc").open("w") as rc:
            rc.write("alias codex='echo existing-codex'\nclaude() { echo existing-claude; }\n")
        self.run_installer("--setup", "--integrate", "both")
        result = subprocess.run(["/bin/bash", "--noprofile", "--rcfile", str(self.home / ".bashrc"), "-ic", 'codex; claude'],
                                env=self.env, capture_output=True, text=True)
        self.assertIn("existing-codex\nexisting-claude", result.stdout)
        (self.config / "shell.bash").unlink()
        result = subprocess.run(["/bin/sh", "-c", '. "$HOME/.bashrc"'], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_bash_preserves_existing_login_profile(self):
        profile = self.home / ".profile"
        profile.write_text("# existing login config\n")
        self.run_installer("--integrate", "path")
        self.assertFalse((self.home / ".bash_profile").exists())
        self.assertTrue(profile.read_text().startswith("# existing login config\n"))
        self.assertEqual(profile.read_text().count("# TermiTex"), 1)
        self.run_installer("--setup", "--integrate", "path")
        self.assertEqual(profile.read_text().count("# TermiTex"), 1)

    def test_deleted_binary_does_not_activate_aliases(self):
        self.write_executable(self.stub / "codex", "#!/bin/sh\necho plain-codex\n")
        self.run_installer("--integrate", "codex")
        (self.bin / "termitex").unlink()
        result = subprocess.run(["/bin/bash", "--noprofile", "--rcfile", str(self.home / ".bashrc"), "-ic", "codex"],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("plain-codex", result.stdout)

    def test_zsh_and_fish_configuration(self):
        self.env["ZDOTDIR"] = str(self.home / "zsh config")
        self.env["XDG_CONFIG_HOME"] = str(self.home / "xdg config")
        self.run_installer("--shell", "zsh", "--integrate", "codex")
        rc = Path(self.env["ZDOTDIR"]) / ".zshrc"
        self.assertIn("shell.zsh", rc.read_text())
        if shutil.which("zsh"):
            result = subprocess.run([shutil.which("zsh"), "-ic", 'codex "two words"'], env=self.env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("<codex>\n<two words>", result.stdout)
        self.run_installer("--setup", "--shell", "fish", "--integrate", "both")
        rc = Path(self.env["XDG_CONFIG_HOME"]) / "fish/config.fish"
        self.assertIn("shell.fish", rc.read_text())
        if shutil.which("fish"):
            result = subprocess.run([shutil.which("fish"), "-ic", 'codex "two words"; claude test'], env=self.env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("<codex>\n<two words>", result.stdout)

    def test_noninteractive_unset_shell_and_option_validation(self):
        self.env.pop("SHELL")
        self.run_installer()
        self.assertFalse((self.home / ".bashrc").exists())
        self.assertIn("requires a value", self.run_installer("--version", ok=False))
        self.assertIn("stable version", self.run_installer("--version", "../../bad", ok=False))
        self.assertIn("Unknown option", self.run_installer("--wat", ok=False))

    def test_prompt_reads_tty_when_script_is_piped(self):
        master, slave = pty.openpty()
        def tty_session():
            os.setsid()
            fcntl.ioctl(1, termios.TIOCSCTTY, 0)
        process = subprocess.Popen(["/bin/sh"], stdin=subprocess.PIPE, stdout=slave, stderr=slave,
                                   env=self.env, preexec_fn=tty_session)
        os.close(slave)
        output = b""
        try:
            process.stdin.write(INSTALLER.read_bytes())
            process.stdin.close()
            deadline = time.monotonic() + 15
            answered = False
            while time.monotonic() < deadline:
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError:
                        break
                    if not chunk:
                        break
                    output += chunk
                    if b"Choose [1-5]:" in output and not answered:
                        os.write(master, b"3\n")
                        answered = True
                if process.poll() is not None:
                    break
            self.assertTrue(answered, output.decode())
            self.assertEqual(process.wait(timeout=2), 0, output.decode())
            self.assertIn("alias codex=", (self.config / "shell.bash").read_text())
            self.assertIn("alias claude=", (self.config / "shell.bash").read_text())
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)


if __name__ == "__main__":
    unittest.main(verbosity=2)
