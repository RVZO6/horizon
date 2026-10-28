#!/usr/bin/env python3
"""Fetches the touch controls' button textures into the prepared resources.

    python3 tools/touch_textures.py [--resources .local/horizon-client/26.3]

They are the TouchController mod's `new` set (Bedrock Edition's current touch buttons), at a pinned
commit: https://github.com/TouchController/TouchController (LGPL-3.0-or-later). Like Mojang's
files they are never committed; they land in `assets/minecraft/textures/gui/sprites/touch/`, where
the HUD atlas packs them (game/touch.rs draws them, or plain shapes when they are missing).
"""

import argparse
import hashlib
from pathlib import Path
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "62f46bae1e19a491257f5020ae31ef53466cdbe4"
SOURCE = (
    "https://raw.githubusercontent.com/TouchController/TouchController/"
    f"{COMMIT}/touchcontroller/resources/texture/control/new/"
)
FILES = [
    ("dpad/up.png", "42d95a9c0e7a134a4a018290d3838c4aef118fb701a3e3be8e8e517526ae7280"),
    ("dpad/up_active.png", "c1475c544cf9d2584358924048f46bc783d13e3236925de6e9d3e8b6feaf172a"),
    ("dpad/down.png", "8bd75eb0cd3c2f4b1190186c9c3af33eb308b5a4403b9b5773410caeb544cda7"),
    ("dpad/down_active.png", "a6e8a44ce1f2252bfa7713b77ee2e3240f987fa3cc249fa0a3b4f16a0fd1f7d0"),
    ("dpad/left.png", "9e237804563765a8845d416fa0ad420bd8c3a3d634fb60d9fbf0933f2b4146f1"),
    ("dpad/left_active.png", "65b1ded1863d068288175bdb8afac11231fb53bdb32b6bd22fc3b25e5244ac76"),
    ("dpad/right.png", "44e25a90580a8dff49431c009a1940ecb3bed6ee82eafff16323e98ed7fdee28"),
    ("dpad/right_active.png", "1c905d235bdac9ca827a7c9fa38196188819fee22a35bdc8339cdceb5f65585c"),
    ("dpad/up_left.png", "0220407eec6beab7f741ad7734b67fff5db83a294c6545d2ec044c495464a1ae"),
    ("dpad/up_left_active.png", "abf950e50b731bf34332cc4f6b51c94b9901774666e1e1e3fe9c5896a9213329"),
    ("dpad/up_right.png", "9527a0d000d48af43bcc6aabd684dc3be9495077f34290a578557618cccfd641"),
    ("dpad/up_right_active.png", "7640430c240e3a0d124690ce8b4a5044f8e7245d091ae82cb4933c933823245d"),
    ("dpad/down_left.png", "865ae2d2a68b8cd2ee931d6762a88979e8a01260919f6fbdab61436c6bd1c8a9"),
    ("dpad/down_left_active.png", "d6cfce7aa04a252b71b045e4be19bcaa2678735cd0f8eff0b9942515b459291f"),
    ("dpad/down_right.png", "bc15e586f20b909539352f36df368f1d2d79f50f5a447901e4dd890268e4280f"),
    ("dpad/down_right_active.png", "807906d85b6082469f968758e6e96363a09c030dd22617b8d475dac99d5872fa"),
    ("sneak/sneak.png", "df329b7904dfbaf95d6f541db5d635d09504d544c7f8bad7ea2cbe7aa85f30fe"),
    ("sneak/sneak_active.png", "ca67756b7f217874f769f1a47c0ed3e46b6081dc99e56ff6139d17b1fb566f3f"),
    ("jump/jump.png", "c21a7ec7f6a9af28d6df4f8a1f8c8de6e832f429306b33a4be1f4a8199770302"),
    ("jump/jump_active.png", "3c9d4fa754550fea9523ded3c0a9d2703be18091dcc9e3485bdcd382d73c0ce6"),
    ("ascend/ascend.png", "ff3a26ac38b227e8d054e33ec996bb0cbefae957d7c3856b1baa9db78af20b0e"),
    ("ascend/ascend_active.png", "46147834e4f9c87c4fbc0d0d8c745d03799c03c4cde775b05d676772633bb298"),
    ("ascend/ascend_swimming.png", "8a28a73dbb1852551a1c139a59fdb874407ee266fec4c17429c9a0c0698fe42e"),
    ("ascend/ascend_swimming_active.png", "7ed2db01c3795af94c842476b7bfc344fed030f632c7123b44aaf7afe2831bdc"),
    ("descend/descend.png", "b3b68a05214cfef8c8e7b177a831dfb47f50f887788b0adbc3487b42687579a5"),
    ("descend/descend_active.png", "d3eb9640d0bf85d2f3bca63d3de193b18623bb398724f83407fea00dd35bbd9d"),
    ("descend/descend_swimming.png", "3ea8ad8527555e9606f97b04f8ab89356f5a93185f799963c7fc99a16f879d16"),
    ("descend/descend_swimming_active.png", "f8fe48864a680933e3976318cd4a24897a0460f1bc23922e6baa3d22c6cc3d60"),
    ("chat/chat.png", "b368da2d0e6d917a0b4858d78ebc1492aa7d336807f918f6b9c0d7b668ff9b7c"),
    ("chat/chat_active.png", "9c6951ff68cb7e1dd376fe9223498dbdf2a921291f218cd2989f174f51ced721"),
    ("pause/pause.png", "5a9fcc5878bd76378b741067d2b2967d9fcbb5b6db6cc8f9197653b6e0d846aa"),
    ("pause/pause_active.png", "124185e9f85e4fd1523486ea1073193884dbf12ea36d2de6b38e5e2bf0fd5a8f"),
    ("inventory/inventory.png", "aa6f290109a33e32198644c49d82566645943f7182469183301530da608bf2f0"),
    ("inventory/inventory_active.png", "fa3172886a818dd6f86005ecf2423a7d553248fdb7740d1f9b78ecf128d5acd0"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--resources", type=Path, default=ROOT / ".local/horizon-client/26.3")
    args = parser.parse_args()
    if not args.resources.is_dir():
        sys.exit(f"{args.resources} is not a prepared resource folder")
    out = args.resources / "assets/minecraft/textures/gui/sprites/touch"
    fetched = 0
    for name, sha256 in FILES:
        target = out / name
        if target.exists() and hashlib.sha256(target.read_bytes()).hexdigest() == sha256:
            continue
        with urllib.request.urlopen(SOURCE + name, timeout=30) as response:
            data = response.read()
        if hashlib.sha256(data).hexdigest() != sha256:
            sys.exit(f"{name}: unexpected contents from {SOURCE + name}")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        fetched += 1
    (out / "SOURCE.txt").write_text(
        f"TouchController's `new` texture set, commit {COMMIT}.\n"
        "https://github.com/TouchController/TouchController (LGPL-3.0-or-later)\n"
    )
    print(f"touch textures: {fetched} fetched, {len(FILES) - fetched} already present")


if __name__ == "__main__":
    main()
