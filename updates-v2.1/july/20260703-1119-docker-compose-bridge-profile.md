# 20260703-1119 — `claude-bridge` opt-in via docker-compose profile

**Affects** : v2.1 devcontainers where `claude-bridge` starts
automatically alongside the main devcontainer.

**Symptom** : the `claude-bridge` sidecar always boots even when the
project doesn't use it. Wastes ~200 MB RAM and a Docker network slot
per project, and adds ~10 s to the devcontainer up flow.

**Cause** : `claude-bridge` had no `profiles` filter in
`docker-compose.yml`, so it counts as a default service.

**Resolution** : declare `claude-bridge` under `profiles: [bridge]`.
Compose will now skip it on plain `docker compose up`. Users who need
it start it explicitly :

`````bash
docker compose --profile bridge up -d claude-bridge

# or via the host helper wrapper
.devcontainer/host-helpers/claude-bridge up
`````

**Upstream commit** : `4156a7a` — `added bridge profile to stop claude-bridge to start by default`

## Apply

Run from your downstream project root (the one containing
`.devcontainer/`), after the [Targeted updates
bootstrap](../../UPGRADE-v2.md#targeted-updates-updatesname) populates
`.tmp/devcontainer-updates/` :

`````bash
git apply --check .tmp/devcontainer-updates/updates-v2.1/july/20260703-1119-docker-compose-bridge-profile.patch
git apply        .tmp/devcontainer-updates/updates-v2.1/july/20260703-1119-docker-compose-bridge-profile.patch

git add .devcontainer/docker-compose.yml
git commit -m "chore(docker-compose): gate claude-bridge behind bridge profile"
`````

No rebuild needed — recompose picks up the profile change on the next
`docker compose up`. If `claude-bridge` was already running, stop it :

`````bash
docker compose stop claude-bridge && docker compose rm -f claude-bridge
`````

## Verify

- [ ] `grep -A 2 "claude-bridge:" .devcontainer/docker-compose.yml | grep -c "profiles: \[bridge\]"`
      → 1 (profile declared).
- [ ] Fresh `docker compose up` from `.devcontainer/` : the
      `claude-bridge` container is **not** started.
- [ ] `docker compose --profile bridge up -d claude-bridge` : bridge
      starts on demand.
- [ ] Devcontainer boot time drops by ~10 s.

## Rollback

`````bash
git revert <commit-hash-from-the-apply-command>
`````
