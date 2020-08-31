from typing import List, Any
import argparse

import spk


def register(
    sub_parsers: argparse._SubParsersAction, **parser_args: Any
) -> argparse.ArgumentParser:

    version_cmd = sub_parsers.add_parser(
        "version", help=_version.__doc__, **parser_args
    )
    version_cmd.set_defaults(func=_version)
    return version_cmd


def _version(_: argparse.Namespace) -> None:
    """Print the spk version number and exit."""

    print(spk.__version__)