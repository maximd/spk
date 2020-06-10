from typing import List, Any, Iterable, Union
import abc

import spfs

from .. import api


class VersionExistsError(FileExistsError):
    def __init__(self, pkg: Any) -> None:
        super(VersionExistsError, self).__init__(
            f"Package version already exists: {pkg}"
        )


class PackageNotFoundError(FileNotFoundError):
    def __init__(self, pkg: Any) -> None:
        super(PackageNotFoundError, self).__init__(f"Package not found: {pkg}")


class Repository(metaclass=abc.ABCMeta):
    @abc.abstractmethod
    def list_packages(self) -> Iterable[str]:
        """Return the set of known packages in this repo."""
        pass

    @abc.abstractmethod
    def list_package_versions(self, name: str) -> Iterable[str]:
        """Return the set of versions available for the named package."""
        pass

    @abc.abstractmethod
    def list_package_builds(self, pkg: Union[str, api.Ident]) -> Iterable[api.Ident]:
        """Return the set of builds for the given package name and version."""
        pass

    @abc.abstractmethod
    def read_spec(self, pkg: api.Ident) -> api.Spec:
        """Read a package spec file for the given package and version.

        Raises
            PackageNotFoundError: If the package or version does not exist
        """
        pass

    @abc.abstractmethod
    def get_package(self, pkg: api.Ident) -> spfs.encoding.Digest:
        """Identify the payload for the identified binary package and build options.

        The given build options should be resolved using the package spec
        before calling this function, unless the exact complete set of options
        can be known deterministically.
        """

        pass

    @abc.abstractmethod
    def publish_spec(self, spec: api.Spec) -> None:
        """Publish a package spec to this repository.

        The published spec represents all builds of a single version.
        The source package, or at least one binary package should be
        published as well in order to make the spec usable in environments.

        Raises:
            VersionExistsError: if the spec a this version is already present
        """
        pass

    @abc.abstractmethod
    def force_publish_spec(self, spec: api.Spec) -> None:
        """Publish a package spec to this repository.

        Same as 'publish_spec' except that it clobbers any existing
        spec at this version
        """
        pass

    @abc.abstractmethod
    def publish_package(self, pkg: api.Ident, digest: spfs.encoding.Digest) -> None:
        """Publish a binary package to this repository.

        The published digest is expected to identify an spfs layer which contains
        the propery constructed binary package files and metadata.
        """
        pass