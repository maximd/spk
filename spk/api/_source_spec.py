from typing import Dict, List, Any, Optional
import abc
import os
import subprocess
from dataclasses import dataclass, field

from ._option_map import OptionMap
from ._ident import Ident


class SourceSpec(metaclass=abc.ABCMeta):
    def subdir(self) -> Optional[str]:

        return None

    @abc.abstractmethod
    def collect(self, dirname: str) -> None:
        """Collect the represented sources files into the given directory."""

        pass

    @staticmethod
    def from_dict(data: Dict[str, Any]) -> "SourceSpec":

        if "path" in data:
            return LocalSource.from_dict(data)
        else:
            raise ValueError("Cannot determine type of source specifier")


@dataclass
class LocalSource(SourceSpec):
    """Package source files in a local file path."""

    path: str = "."

    def collect(self, dirname: str) -> None:

        args = ["--recursive", "--archive"]
        if "SPM_DEBUG" in os.environ:
            args.append("--verbose")
        if os.path.exists(os.path.join(dirname, ".gitignore")):
            args += ["--filter", ":- .gitignore"]
        args += ["--cvs-exclude", self.path, dirname]
        cmd = ["rsync"] + args
        subprocess.check_call(cmd, cwd=dirname)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "path": self.path,
        }

    @staticmethod
    def from_dict(data: Dict[str, Any]) -> "LocalSource":

        return LocalSource(data["path"])