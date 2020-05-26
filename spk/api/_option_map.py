from typing import Dict, Any
import hashlib
import base64
import platform

import distro
from sortedcontainers import SortedDict

# given option digests are namespaced by the package itself,
# there are slim likelyhoods of collision, so we roll the dice
# also must be a multiple of 8 to be decodable wich is generally
# a nice way to handle validation / and 16 is a lot
_DIGEST_SIZE = 8


class OptionMap(SortedDict):
    """A set of values for package build options."""

    def digest(self) -> str:

        hasher = hashlib.sha1()
        for name, value in self.items():
            hasher.update(name.encode())
            hasher.update(b"=")
            hasher.update(value.encode())
            hasher.update(bytes([0]))

        digest = hasher.digest()
        return base64.b32encode(digest)[:_DIGEST_SIZE].decode()

    @staticmethod
    def from_dict(data: Dict[str, Any]) -> "OptionMap":

        opts = OptionMap()
        for name, value in data.items():
            opts[name] = str(value)
        return opts


def host_options() -> OptionMap:
    """Detect and return the default options for the current host system"""

    opts = OptionMap(arch=platform.machine(), os=platform.system().lower())

    info = distro.info()
    distro_name = info["id"]
    opts["distro"] = distro_name
    opts[distro_name] = info["version"]

    return opts
