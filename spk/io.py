from typing import List, Sequence, Set, Union
from colorama import Fore, Style

from . import api, solve


def format_ident(pkg: api.Ident) -> str:

    out = f"{Style.BRIGHT}{pkg.name}{Style.RESET_ALL}"
    if pkg.version.parts or pkg.build is not None:
        out += f"/{Fore.LIGHTBLUE_EX}{pkg.version}{Fore.RESET}"
    if pkg.build is not None:
        out += f"/{format_build(pkg.build)}"
    return out


def format_resolve(
    solver: Union[solve.Solver, solve.GraphSolver], verbosity: int = 1
) -> str:
    if isinstance(solver, solve.Solver):
        return format_decision_tree(solver.decision_tree, verbosity)
    else:
        graph = solver.get_last_solve_graph()
        return format_solve_graph(graph, verbosity)


def format_solve_graph(graph: solve.Graph, verbosity: int = 1) -> str:
    out = ""
    level = 0
    for node, decision in graph.walk():
        if verbosity > 1:
            for note in decision.iter_notes():
                out += f"{'.'*level} {format_note(note)}\n"

        for change in decision.iter_changes():
            if isinstance(change, solve.graph.SetPackage):
                fill = ">"
                prefix = " "
                level += 1
            elif isinstance(change, solve.graph.StepBack):
                fill = "<"
                prefix = "< "
                level -= 1
            else:
                fill = "."
                prefix = " "
            out += f"{fill*level}{prefix}{format_change(change)}\n"

    return out


def format_decision_tree(tree: solve.DecisionTree, verbosity: int = 1) -> str:

    out = ""
    for decision in tree.walk():
        out += ">" * decision.level()
        lines = format_decision(decision, verbosity).split("\n")
        out += " " + lines[0] + "\n"
        for line in lines[1:]:
            out += "." * decision.level()
            out += " " + line + "\n"
    return out[:-1]


def format_change(change: solve.graph.Change, verbosity: int = 1) -> str:

    out = ""
    if isinstance(change, solve.graph.RequestPackage):
        return f"{Fore.BLUE}REQUEST{Fore.RESET} {format_request(change.request.pkg.name, [change.request])}"
    elif isinstance(change, solve.graph.RequestVar):
        return f"{Fore.BLUE}REQUEST{Fore.RESET} {format_options(api.OptionMap({change.request.name(): change.request.value}))}"
    elif isinstance(change, solve.graph.SetPackage):
        return f"{Fore.GREEN}RESOLVE{Fore.RESET} {format_ident(change.spec.pkg)}"
    elif isinstance(change, solve.graph.SetOption):
        return f"{Fore.CYAN}SET{Fore.RESET} {format_options(api.OptionMap({change.name: change.value}))}"
    elif isinstance(change, solve.graph.UnresolvePackage):
        return f"{Fore.YELLOW}UNRESOLVE{Fore.RESET} {format_ident(change.pkg)}"
    elif isinstance(change, solve.graph.StepBack):
        return f"{Fore.RED}BLOCKED{Fore.RESET} {change.cause}"
    else:
        return f"{Fore.MAGENTA}OTHER{Fore.RESET} {change}"


def format_note(note: solve.graph.Note) -> str:

    if isinstance(note, solve.graph.SkipPackageNote):
        return f"{Fore.MAGENTA}TRY{Fore.RESET} {format_ident(note.pkg)} - {note.reason}"
    else:
        return f"{Fore.MAGENTA}NOTE{Fore.RESET} {note}"


def format_decision(decision: solve.Decision, verbosity: int = 1) -> str:

    end = "\n" if verbosity > 1 else " "
    out = ""

    error = decision.get_error()
    resolved = decision.get_resolved()
    requests = decision.get_requests()
    unresolved = decision.get_unresolved()
    if resolved:
        if verbosity > 1:
            for _, spec, _ in resolved.items():
                iterator = decision.get_iterator(spec.pkg.name)
                if iterator is not None:
                    versions = list(
                        f"{Fore.MAGENTA}TRY{Fore.RESET} {format_ident(v)} - {c}"
                        for v, c in iterator.get_history().items()
                    )
                    if versions:
                        out += end.join(reversed(versions)) + end
                out += f"{Fore.GREEN}RESOLVE{Fore.RESET} {format_ident(spec.pkg)}" + end
                if verbosity > 2:
                    opt = spec.resolve_all_options(decision.get_options())
                    if opt:
                        out += format_options(opt) + end
        else:
            values = list(format_ident(spec.pkg) for _, spec, _ in resolved.items())
            out += f"{Fore.GREEN}RESOLVE{Fore.RESET} {', '.join(values)}" + end
    if requests:
        values = list(format_request(n, pkgs) for n, pkgs in requests.items())
        out += f"{Fore.BLUE}REQUEST{Fore.RESET} {', '.join(values)}" + end
    if error is None and unresolved:
        if verbosity > 1:
            reasons = list(
                f"{Fore.YELLOW}UNRESOLVE{Fore.RESET} {v} - {c}"
                for v, c in unresolved.items()
            )
            if reasons:
                out += end.join(reversed(reasons)) + end
        else:
            out += f"{Fore.YELLOW}UNRESOLVE{Fore.RESET} {', '.join(unresolved)}" + end

    if error is not None:

        if not isinstance(error, solve.UnresolvedPackageError):
            out += f"{Fore.RED}BLOCKED{Fore.RESET} {error}"
        else:
            if verbosity > 1:
                versions = list(
                    f"{Fore.MAGENTA}TRY{Fore.RESET} {v} - {c}"
                    for v, c in (error.history or {}).items()
                )
                out += end.join(versions) + (end if versions else "")

            out += f"{Fore.RED}BLOCKED{Fore.RESET} {error.message}"

    return out.strip()


def format_request(name: str, requests: Sequence[api.Request]) -> str:

    out = f"{Style.BRIGHT}{name}{Style.RESET_ALL}/"
    versions = []
    for req in requests:
        assert isinstance(
            req, api.PkgRequest
        ), f"TODO: Unhandled request in formatter {type(req)}"
        ver = f"{Fore.LIGHTBLUE_EX}{str(req.pkg.version) or '*'}{Fore.RESET}"
        if req.pkg.build is not None:
            ver += f"/{format_build(req.pkg.build)}"
        versions.append(ver)
    out += ",".join(versions)
    return out


def format_options(options: api.OptionMap) -> str:

    formatted = []
    for name, value in options.items():
        formatted.append(
            f"{name}{Style.DIM}={Style.NORMAL}{Fore.CYAN}{value}{Fore.RESET}"
        )

    return f"{{{', '.join(formatted)}}}"


def format_build(build: api.Build) -> str:

    if build.is_emdeded():
        return f"{Fore.LIGHTMAGENTA_EX}{build}{Style.RESET_ALL}"
    elif build.is_source():
        return f"{Fore.LIGHTYELLOW_EX}{build}{Style.RESET_ALL}"
    else:
        return f"{Style.DIM}{build}{Style.RESET_ALL}"


def format_solution(solution: solve.Solution, verbosity: int = 0) -> str:

    out = "Installed Packages:\n"
    for _, spec, _ in solution.items():
        if verbosity:
            options = spec.resolve_all_options(api.OptionMap({}))
            out += f"  {format_ident(spec.pkg)} {format_options(options)}\n"
        else:
            out += f"  {format_ident(spec.pkg)}\n"
    return out
