from typing import List
import os

import structlog
import spfs

from .. import api, storage
from ._env import data_path
from ._binary import BuildError

_LOGGER = structlog.get_logger("spk.build")


class CollectionError(BuildError):
    """Denotes a build error that happened during the collection of source files."""

    pass


def make_source_package(spec: api.Spec) -> api.Ident:
    """Create a local source package for the given spec."""

    repo = storage.local_repository()
    layer = collect_and_commit_sources(spec)
    repo.publish_package(spec.pkg.with_build(api.SRC), layer.digest())
    return spec.pkg.with_build(api.SRC)


def collect_and_commit_sources(spec: api.Spec) -> spfs.storage.Layer:
    """Collect sources for the given spec and commit them into an spfs layer."""

    pkg = spec.pkg.with_build(api.SRC)

    runtime = spfs.active_runtime()

    source_dir = data_path(pkg)
    collect_sources(spec, source_dir)

    diffs = spfs.diff()
    validate_source_changeset(diffs, source_dir)

    return spfs.commit_layer(runtime)


def collect_sources(spec: api.Spec, source_dir: str) -> None:
    """Collect the sources for a spec in the given directory."""
    os.makedirs(source_dir)

    for source in spec.sources:

        target_dir = source_dir
        subdir = source.subdir()
        if subdir:
            target_dir = os.path.join(source_dir, subdir.lstrip("/"))
            os.makedirs(target_dir, exist_ok=True)

        source.collect(target_dir)


def validate_source_changeset(diffs: List[spfs.tracking.Diff], source_dir: str) -> None:
    """Validate the set of diffs for a source package build.

    Raises:
      CollectionError: if any issues are identified in the changeset
    """

    if not diffs:
        raise CollectionError(
            "No source files collected, source package would be empty"
        )

    source_dir = source_dir.rstrip("/") + "/"
    if source_dir.startswith("/spfs"):
        source_dir = source_dir[len("/spfs") :]
    for diff in diffs:
        if diff.mode is spfs.tracking.DiffMode.unchanged:
            continue
        if diff.path.startswith(source_dir):
            # the change is within the source directory
            continue
        if source_dir.startswith(diff.path):
            # the path is to a parent directory of the source path
            continue
        raise CollectionError(
            f"Invalid source file path found: {diff.path} (not under {source_dir})"
        )