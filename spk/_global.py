from typing import Union

import spkrs
from . import api, storage


def load_spec(pkg: Union[str, api.Ident]) -> api.Spec:
    """Load a package spec from the default repository."""

    if not isinstance(pkg, api.Ident):
        pkg = api.parse_ident(pkg)

    try:
        return storage.remote_repository().read_spec(pkg)
    except storage.PackageNotFoundError:
        return storage.local_repository().read_spec(pkg)


def save_spec(spec: api.Spec) -> None:
    """Save a package spec to the local repository."""

    repo = storage.local_repository()
    repo.force_publish_spec(spec)