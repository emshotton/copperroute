"""Small helper for running cells with bounded concurrency.

Each cell is a blocking subprocess (plus an I/O-bound referee hook), so threads
are sufficient -- no need for multiprocessing.
"""
from __future__ import annotations

from collections.abc import Callable, Iterable
from concurrent.futures import ThreadPoolExecutor
from typing import TypeVar

T = TypeVar("T")
R = TypeVar("R")


def run_cells(cells: Iterable[T], fn: Callable[[T], R], jobs: int,
              on_done: Callable[[R], None] | None = None) -> list[R]:
    """Run `fn(cell)` for every cell in `cells`, using up to `jobs` worker threads.

    Futures are submitted up front so they run concurrently, but `on_done` is invoked
    by walking the futures list in submission order rather than completion order --
    so a run interrupted partway through still leaves a gap-free, order-preserving
    `meta.json`.
    """
    cells = list(cells)
    if jobs <= 1:
        results: list[R] = []
        for cell in cells:
            r = fn(cell)
            results.append(r)
            if on_done:
                on_done(r)
        return results
    results = []
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        futures = [pool.submit(fn, cell) for cell in cells]
        for fut in futures:
            r = fut.result()
            results.append(r)
            if on_done:
                on_done(r)
    return results
