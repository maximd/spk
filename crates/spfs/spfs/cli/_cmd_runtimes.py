import argparse

import spfs


def register(sub_parsers: argparse._SubParsersAction) -> None:

    runtimes_cmd = sub_parsers.add_parser("runtimes", help=_runtimes.__doc__)
    runtimes_cmd.set_defaults(func=_runtimes)


def _runtimes(args: argparse.Namespace) -> None:
    """List the active set of spfs runtimes."""

    config = spfs.get_config()
    runtime_storage = config.get_runtime_storage()
    runtimes = runtime_storage.list_runtimes()
    for runtime in runtimes:
        print(runtime.ref)