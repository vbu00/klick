"""Подпись выпусков kl!ck для обновления в один клик.

Служба ставит обновление сама, без окна «Разрешить изменения?», — только если установщик подписан
ключом владельца. Закрытый ключ лежит только на его компьютере (не в репозитории и не в секретах
GitHub): даже тот, кто может выпускать релизы в репозитории, не выпустит обновление, которое
поставится само.

    python -I tools/sign-release.py keygen            # один раз: ключ и открытый ключ для службы
    python -I tools/sign-release.py pub               # открытый ключ ещё раз
    python -I tools/sign-release.py sign v0.5.3       # подписать установщики выпуска и выложить .sig

Подписывается строка `klick-update-v1\\n<имя файла>\\n<sha256>\\n`: имя файла с версией не даёт
выдать старый подписанный установщик за новый. Перед подписью sha256 скачанного файла сверяется
с тем, что GitHub записал при загрузке (digest), — подписываем ровно то, что выложил CI.
Нужен пакет cryptography (pip install cryptography) и gh с входом в аккаунт владельца.
"""

import base64
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

REPO = os.environ.get('KLICK_RELEASES_REPO', 'vbu00/klick')
KEY = Path(os.environ.get('KLICK_UPDATE_KEY_FILE', Path.home() / '.ssh' / 'klick-update.key'))


def message(name, digest):
    return f'klick-update-v1\n{name}\n{digest}\n'.encode()


def load_key():
    if not KEY.exists():
        sys.exit(f'нет ключа {KEY}: сначала keygen')
    return serialization.load_pem_private_key(KEY.read_bytes(), password=None)


def public_b64(key):
    raw = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    return base64.b64encode(raw).decode()


def keygen():
    if KEY.exists():
        sys.exit(f'ключ уже есть: {KEY} — второй не создаю (старые сборки знают только этот)')
    key = Ed25519PrivateKey.generate()
    pem = key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption())
    KEY.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(KEY, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as f:
        f.write(pem)
    print(f'ключ: {KEY}')
    print(f'открытый ключ (UPDATE_KEY в crates/klick-service/src/update.rs): {public_b64(key)}')


def gh(*args):
    return subprocess.run(['gh', *args], check=True, capture_output=True, encoding='utf-8').stdout


def sign(tag):
    key = load_key()
    version = tag.removeprefix('v')
    # Установщики, которые служба ставит сама: Windows и macOS (версионный пакет, не klick-macos.pkg).
    names = [f'klick-{version}-x64-setup.exe', f'klick-{version}.pkg']
    release = json.loads(gh('api', f'repos/{REPO}/releases/tags/{tag}'))
    assets = {a['name']: a for a in release['assets']}
    found = [n for n in names if n in assets]
    if not found:
        sys.exit(f'в выпуске {tag} нет ни {names[0]}, ни {names[1]}')
    for name in names:
        if name not in assets:
            print(f'{name}: нет в выпуске — пропускаю (выложите и подпишите ещё раз)')
    with tempfile.TemporaryDirectory() as tmp:
        for name in found:
            expected = (assets[name].get('digest') or '').removeprefix('sha256:')
            gh('release', 'download', tag, '--repo', REPO, '--pattern', name, '--dir', tmp)
            digest = hashlib.sha256((Path(tmp) / name).read_bytes()).hexdigest()
            if expected and digest != expected:
                sys.exit(f'{name}: sha256 скачанного {digest} не совпал с записанным GitHub {expected} — не подписываю')
            sig = Path(tmp) / f'{name}.sig'
            sig.write_text(base64.b64encode(key.sign(message(name, digest))).decode() + '\n')
            gh('release', 'upload', tag, str(sig), '--repo', REPO, '--clobber')
            print(f'{name}: sha256 {digest}' + (' (совпал с GitHub)' if expected else ' (GitHub digest не дал)') + f' — подпись {name}.sig выложена')


def main():
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
    cmd = sys.argv[1:2]
    if cmd == ['keygen']:
        keygen()
    elif cmd == ['pub']:
        print(public_b64(load_key()))
    elif cmd == ['sign'] and len(sys.argv) == 3:
        sign(sys.argv[2])
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
