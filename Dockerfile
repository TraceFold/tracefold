# 47 §1(c)'s compose needs something to run, and this is the smallest thing that is: **artefact (a)
# in a container, and nothing else**.
#
# 🔴 What this is **not**, and the line is 47 §6's rather than this file's taste:
#
#   > v0.1で必須なのは (a)単一静的バイナリ + (c)docker-compose.yaml … (b)コンテナイメージのcosign署名
#   > +SLSA provenance付与は…(d)Helm chart・T3（K8s常駐）・T4（air-gapped）は…v0.2+の提案
#
# So there is **no cosign signature and no SLSA provenance attestation here** (47 §1(b)), and this
# image is not the distributable 47 §1(b) describes — it is the wrapper 47 §1(c) needs in order to
# have a service to declare. req/88 §1 N-10 keeps K8s and air-gapped verification out of M6
# entirely.
#
# # Why `scratch`
#
# NFR-019 asks for 「単一静的バイナリとしてデプロイ可能」 and hand 7 measured that: `readelf -d` finds
# **0** `NEEDED` entries and there is no `INTERP` segment, so the binary needs no loader, no libc and
# no base image. A distroless or Wolfi base (47 §1(b)'s eventual shape) would add a filesystem this
# binary does not read. `scratch` makes the claim checkable: if the binary were not static, the
# container would not start, and the failure would be immediate rather than latent.
#
# The consequence, said rather than discovered later: there is **no shell, no CA bundle and no
# `/etc/passwd`** in here. `gx` needs none of the three (DR-4's default is a self-hosted tile log
# with no external anchor, so nothing dials out), and a future feature that needs TLS to a remote
# will need a base image and will have to say so.
#
# # The binary comes from the host build, not from a builder stage
#
# A multi-stage build would compile the workspace inside Docker, which is the right shape for a
# release pipeline and the wrong one for this hand: the artefact 47 §1(a) names is the one
# `tools/m6h7_dist.sh static` produced and measured (15,490,848 bytes, `statically linked`,
# `NEEDED=0`, `INTERP=0`), and an image built from a *different* compilation would be a container
# holding an unmeasured binary. `.dockerignore` keeps the context to that one file.
FROM scratch

# 🔴 The `x86_64-unknown-linux-gnu` + `+crt-static` arm, not musl, and the difference is measured
# rather than preferred: `psm 0.1.32` (cedar-policy → stacker → psm) compiles assembly through
# cc-rs, and cross-compiling it to musl needs `x86_64-linux-musl-gcc`, which this machine does not
# have and cannot install (no sudo). NFR-019's requirement is 「動的リンク依存なし」 and this arm meets
# it; 47 §1(a)'s named *means* is musl and that remains unmet. req/95 §6 carries both statements and
# raises the gap rather than letting the second stand in for the first.
COPY target/x86_64-unknown-linux-gnu/release/gx /gx

# `.gx/` lives on the volume the compose file declares. `WORKDIR` is the project root the CLI
# resolves `.gx/` against (req/56 §1).
WORKDIR /project

# 44 §1.1's verb. No `CMD` arguments: the compose file supplies them, so that an operator reading
# `docker-compose.yaml` sees the whole invocation in one place instead of half of it here.
ENTRYPOINT ["/gx"]
