"""Package-owned checkpoint manifest and verified cache resolver.

One resolution path powers loading and read-only cache inspection so they cannot
drift. Downloads use an adjacent temporary file and atomic replacement.
Reads: packaged config/checkpoints.toml and SCNET_INFER_CACHE.
"""

from __future__ import annotations

import hashlib
import os
import tempfile
import tomllib
import urllib.parse
import urllib.request
from dataclasses import dataclass
from importlib.resources import files
from pathlib import Path
from typing import Any, Callable, Mapping

DownloadProgress = Callable[[int, int], None]


class CheckpointConfigError(ValueError):
    """Raised when checkpoint metadata is malformed."""


class ChecksumError(RuntimeError):
    """Raised when an artifact does not match its declared integrity metadata."""


@dataclass(frozen=True)
class CheckpointSpec:
    """Validated runtime view of one manifest entry."""

    model_id: str
    family: str
    url: str
    filename: str
    size: int
    sha256: str
    license: str
    provenance: str
    source_revision: str
    checked: str
    sample_rate: int
    chunk_size: int
    batch_size: int
    num_overlap: int
    normalize: bool
    use_amp: bool
    sources: tuple[str, ...]
    model: Mapping[str, Any]


def default_cache_dir() -> Path:
    """Return the configured cache root without creating it."""

    override = os.environ.get("SCNET_INFER_CACHE")
    return Path(override).expanduser() if override else Path.home() / ".cache" / "scnet-infer"


def load_manifest(path: str | Path | None = None) -> tuple[str, dict[str, CheckpointSpec]]:
    """Read and validate the package manifest or a generic TOML override."""

    try:
        if path is None:
            raw = tomllib.loads(files("scnet_infer").joinpath("config/checkpoints.toml").read_text())
        else:
            with Path(path).open("rb") as stream:
                raw = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise CheckpointConfigError(f"cannot read checkpoint manifest: {exc}") from exc
    if raw.get("schema_version") != 1 or not isinstance(raw.get("checkpoints"), dict):
        raise CheckpointConfigError("checkpoint manifest requires schema_version = 1 and [checkpoints]")
    default = raw.get("default")
    specs: dict[str, CheckpointSpec] = {}
    required = {
        "family", "url", "filename", "size", "sha256", "license", "provenance",
        "source_revision", "checked", "sample_rate", "chunk_size", "batch_size",
        "num_overlap", "normalize", "use_amp", "sources", "model",
    }
    for model_id, entry in raw["checkpoints"].items():
        missing = sorted(required - set(entry))
        if missing:
            raise CheckpointConfigError(f"checkpoint {model_id!r} is missing: {', '.join(missing)}")
        digest = entry["sha256"]
        if not isinstance(digest, str) or len(digest) != 64:
            raise CheckpointConfigError(f"checkpoint {model_id!r} has invalid sha256")
        try:
            specs[model_id] = CheckpointSpec(
                model_id=model_id,
                family=str(entry["family"]), url=str(entry["url"]),
                filename=str(entry["filename"]), size=int(entry["size"]),
                sha256=digest.lower(), license=str(entry["license"]),
                provenance=str(entry["provenance"]), source_revision=str(entry["source_revision"]),
                checked=str(entry["checked"]), sample_rate=int(entry["sample_rate"]),
                chunk_size=int(entry["chunk_size"]), batch_size=int(entry["batch_size"]),
                num_overlap=int(entry["num_overlap"]), normalize=bool(entry["normalize"]),
                use_amp=bool(entry["use_amp"]), sources=tuple(entry["sources"]),
                model=dict(entry["model"]),
            )
        except (TypeError, ValueError) as exc:
            raise CheckpointConfigError(f"checkpoint {model_id!r} has invalid field types") from exc
    if default not in specs:
        raise CheckpointConfigError("manifest default must name a configured checkpoint")
    return str(default), specs


def get_spec(model_id: str | None = None, manifest_path: str | Path | None = None) -> CheckpointSpec:
    """Select one validated checkpoint configuration."""

    default, specs = load_manifest(manifest_path)
    selected = model_id or default
    try:
        return specs[selected]
    except KeyError as exc:
        raise CheckpointConfigError(f"unknown model_id {selected!r}; choose from {sorted(specs)}") from exc


def sha256_file(path: Path) -> str:
    """Hash a file without loading it into memory."""

    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _expected_path(spec: CheckpointSpec, cache_dir: Path, checkpoint_path: str | Path | None, url: str) -> Path:
    if checkpoint_path is not None:
        return Path(checkpoint_path).expanduser()
    name = Path(urllib.parse.urlparse(url).path).name or spec.filename
    return cache_dir / spec.model_id / name


def cache_info(
    spec: CheckpointSpec,
    *,
    cache_dir: str | Path | None = None,
    checkpoint_path: str | Path | None = None,
    checkpoint_url: str | None = None,
) -> dict[str, object]:
    """Inspect the exact resolver target without creating or downloading anything."""

    root = Path(cache_dir).expanduser() if cache_dir is not None else default_cache_dir()
    path = _expected_path(spec, root, checkpoint_path, checkpoint_url or spec.url)
    return {
        "model_id": spec.model_id,
        "path": str(path),
        "exists": path.is_file(),
        "source": "manual" if checkpoint_path is not None else "cache",
        "license": spec.license,
        "sha256": spec.sha256,
    }


def resolve_checkpoint(
    spec: CheckpointSpec,
    *,
    cache_dir: str | Path | None = None,
    checkpoint_path: str | Path | None = None,
    checkpoint_url: str | None = None,
    checkpoint_sha256: str | None = None,
    progress: DownloadProgress | None = None,
) -> Path:
    """Resolve, download if needed, and verify one checkpoint."""

    root = Path(cache_dir).expanduser() if cache_dir is not None else default_cache_dir()
    url = checkpoint_url or spec.url
    expected = (checkpoint_sha256 or spec.sha256).lower()
    if checkpoint_url is not None and checkpoint_sha256 is None:
        raise CheckpointConfigError("checkpoint_sha256 is required with checkpoint_url")
    path = _expected_path(spec, root, checkpoint_path, url)
    if path.is_symlink() or path.parent.is_symlink():
        raise CheckpointConfigError("checkpoint paths must not be symbolic links")
    if not path.is_file():
        if checkpoint_path is not None:
            raise FileNotFoundError(f"checkpoint not found: {path}")
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.parent.is_symlink():
            raise CheckpointConfigError("checkpoint cache directories must not be symbolic links")
        handle, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".part", dir=path.parent)
        os.close(handle)
        temporary = Path(temporary_name)
        try:
            def report(blocks: int, block_size: int, total: int) -> None:
                if progress is not None:
                    progress(min(blocks * block_size, max(total, 0)), max(total, 0))

            urllib.request.urlretrieve(url, temporary, reporthook=report)
            if sha256_file(temporary) != expected:
                raise ChecksumError(f"downloaded checkpoint checksum mismatch for {url}")
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    if path.is_symlink() or path.stat().st_size != spec.size:
        raise ChecksumError(f"checkpoint size mismatch for {path}")
    actual = sha256_file(path)
    if actual != expected:
        raise ChecksumError(f"checkpoint checksum mismatch for {path}: expected {expected}, got {actual}")
    return path
