import os
import subprocess
import tempfile
from typing import Iterable, List, Optional

import spfs

from .. import api, storage, solve, exec, build
from ._build import TestError


class PackageSourceTester:
    def __init__(self, spec: api.Spec, script: str) -> None:
        self._prefix = "/spfs"
        self._spec = spec
        self._script = script
        self._repos: List[storage.Repository] = []
        self._options = api.OptionMap()
        self._solver: Optional[solve.Solver] = None

    def get_test_env_decision_tree(self) -> solve.DecisionTree:
        """Return the solver decision tree for the test environment.

        This is most useful for debugging test environments that failed to resolve,
        and test that failed with a SolverError.

        If the tester has not run, return an empty tree.
        """

        if self._solver is None:
            return solve.DecisionTree()
        return self._solver.decision_tree

    def with_option(self, name: str, value: str) -> "PackageSourceTester":

        self._options[name] = value
        return self

    def with_options(self, options: api.OptionMap) -> "PackageSourceTester":

        self._options.update(options)
        return self

    def with_repository(self, repo: storage.Repository) -> "PackageSourceTester":

        self._repos.append(repo)
        return self

    def with_repositories(
        self, repos: Iterable[storage.Repository]
    ) -> "PackageSourceTester":

        self._repos.extend(repos)
        return self

    def test(self) -> None:

        runtime = spfs.active_runtime()
        runtime.set_editable(True)
        spfs.remount_runtime(runtime)
        runtime.reset("**/*")
        runtime.reset_stack()
        runtime.set_editable(True)
        spfs.remount_runtime(runtime)

        self._solver = solve.Solver(self._options)
        for repo in self._repos:
            self._solver.add_repository(repo)
        self._solver.add_request(self._spec.pkg.with_build(api.SRC))
        solution = self._solver.solve()

        exec.configure_runtime(runtime, solution)
        spfs.remount_runtime(runtime)

        env = solution.to_environment() or {}
        env["PREFIX"] = self._prefix

        source_dir = build.source_package_path(
            self._spec.pkg.with_build(api.SRC), self._prefix
        )
        with tempfile.NamedTemporaryFile("w+") as script_file:
            script_file.write(self._script)
            script_file.flush()
            os.environ["SHELL"] = "sh"
            cmd = spfs.build_shell_initialized_command(
                "/bin/sh", "-ex", script_file.name
            )

            proc = subprocess.Popen(cmd, cwd=source_dir, env=env)
            proc.wait()
            if proc.returncode != 0:
                raise TestError(
                    f"Test script returned non-zero exit status: {proc.returncode}"
                )
