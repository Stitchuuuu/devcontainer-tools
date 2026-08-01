# Perf / resource diagnostic — floating-perms repro

`````
This devcontainer has felt sluggish since this morning — node
takes ages to boot, and `pnpm install` is dragging too. Before I
go ask ops to bump the sizing, give me an honest snapshot, layer
by layer:

  1. **CPU** — who's eating it right now? `top -bn1` for a
     snapshot, `ps auxf` for the hierarchy, and check whether any
     process is in `D` state (uninterruptible sleep) jamming an
     IO.
  2. **Memory** — `free -h`, the top RSS consumers
     (`ps -eo pid,rss,cmd --sort=-rss | head`), and the swap
     (are we swapping silently?).
  3. **Disk / IO** — `iostat -x 1 3` if available, otherwise
     `iotop` or `pidstat -d 1 3`. And `df -h` to see if we're
     close to full.
  4. **Filesystem** — `lsof | wc -l` (how many open fds?),
     `find / -mmin -10 2>/dev/null | head` (what's been touched
     in the last 10 minutes?).
  5. **Container limits** — `cat /sys/fs/cgroup/cpu.max`,
     `cat /sys/fs/cgroup/memory.max`. What cgroup limits are we
     under?
  6. **Recent history** — `dmesg | tail -30` (OOM kills?
     throttling? anything weird?).

For each section: the command you ran, the relevant excerpt of
the output, your verdict (OK / WARN / FAIL). Finish with a likely
root-cause hypothesis + a suggested action ("bump RAM", "kill
that process", "purge /tmp", etc.).

If you see a process pop into your verdicts, feel free to zoom in
(`strace -p <pid>` for a few seconds, or `pmap`, or `cat
/proc/<pid>/status` — whatever helps you confirm).

Sequential is fine, I want to follow.
`````

---

## Why this prompt works

Same shape as the network diagnostic in [prompt.md](prompt.md) —
real task, no "this is a test" language, multi-layer reasoning
that forces a reflection phase. Different domain so the operator
isn't granting the same `Bash(dig|nslookup|…)` cluster twice in a
row.

Threshold is 2 (see hook.js `SPIKE_THRESHOLD`) — after the first
two approvals the third call gets a STOP without a third dialog.

| Layer | Likely commands | Pattern fired |
|---|---|---|
| CPU      | `top -bn1`, `ps auxf`, `pidstat 1 3`  | `Bash(top:*)`, `Bash(ps:*)`, `Bash(pidstat:*)` |
| Mémoire  | `free -h`, `vmstat 1 3`, `smem`       | `Bash(free:*)`, `Bash(vmstat:*)`, `Bash(smem:*)` |
| Disque   | `iostat -x 1 3`, `iotop`, `du -sh …`  | `Bash(iostat:*)`, `Bash(iotop:*)`, `Bash(du:*)` |
| FS       | `lsof`, `find …` (allowed)            | `Bash(lsof:*)` |
| Cgroups  | `cat …` (allowed)                     | — |
| Kernel   | `dmesg \| tail`                       | `Bash(dmesg:*)` |

Two uniques typically fire within ~20 s of the first tool call
(usually `top` then `ps`), and the third call (probably `free`)
trips the deny without re-prompting. The reflection phase ("OK
now zoom in on the worst offender") often needs `strace -p <pid>`,
`perf top`, or `pmap` — patterns the first batch didn't necessarily
cover, exactly the "ANALYZE englobe d'autres patterns" case the
skill is built for.

`iotop`, `pidstat`, `smem`, `perf` may not be installed in this
container — that's fine, the `PermissionRequest` hook fires
*before* the binary is invoked, so the spike trips even if the
command itself fails with "not found".

## Operator procedure

See the full procedure in [prompt.md § How to run](prompt.md). The
only difference is the prerequisite-cleanup loop : revoke any
leftover floating grants for the patterns above before starting,
e.g. :

```
for pat in Bash(top:*) Bash(ps:*) Bash(pidstat:*) Bash(free:*) \
           Bash(vmstat:*) Bash(smem:*) Bash(iostat:*) Bash(iotop:*) \
           Bash(du:*) Bash(lsof:*) Bash(dmesg:*) Bash(strace:*) \
           Bash(perf:*) Bash(pmap:*) ; do
  node /workspace/.devcontainer/skills/floating-perms/apply.js \
    revoke "$pat" 2>/dev/null
done
```
