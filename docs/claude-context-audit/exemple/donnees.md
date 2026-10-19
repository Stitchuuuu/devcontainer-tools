# Données de l'audit du contexte — DevContainer Tools

Transcripts : `/home/node/.claude/projects/-workspace`  
Période : du 2026-09-16 au 2026-10-19

| DevContainer Tools | Min | Médiane | Moyenne | p90 | Max |
|---|--:|--:|--:|--:|--:|
| Contexte max, 190 sessions | 28 k | **238 k** | 264 k | 480 k | 947 k |
| &emsp;dont opus, 149 sessions | 28 k | **229 k** | 258 k | 470 k | 947 k |
| &emsp;dont sonnet, 18 sessions | 58 k | **189 k** | 225 k | 380 k | 446 k |
| &emsp;dont fable, 28 sessions | 54 k | **309 k** | 324 k | 534 k | 657 k |
| Contexte par appel, fil principal (16 740 appels) | 28 k | **216 k** | 244 k | 439 k | 947 k |
| Contexte par appel, sous-agents (6 630 appels) | 9 k | **70 k** | 74 k | 127 k | 216 k |

Seuils : 9 161 appels du fil principal dépassent 200 k (55 %), 2 200 dépassent 400 k, 58 dépassent 800 k, sur 16 740.  
Sessions compactées au moins une fois : 1 sur 190.
Médiane fil principal / sous-agents : 216 k / 70 k, soit 3 fois moins dans un sous-agent.

## Contexte par appel, par famille

| | appels | médiane | p90 | max |
|---|--:|--:|--:|--:|
| fil principal, opus | 13 849 | 214 k | 449 k | 947 k |
| fil principal, sonnet | 1 252 | 193 k | 306 k | 446 k |
| fil principal, fable | 1 638 | 256 k | 457 k | 657 k |
| fil principal, haiku | 1 | 41 k | 41 k | 41 k |
| sous-agents, opus | 3 458 | 69 k | 119 k | 192 k |
| sous-agents, sonnet | 2 386 | 84 k | 139 k | 216 k |
| sous-agents, fable | 32 | 82 k | 209 k | 211 k |
| sous-agents, haiku | 754 | 38 k | 67 k | 106 k |

## Sessions au plus gros contexte max (top 25)

| # | session | date | modèle | ctx max | ctx médian | appels | compacts | sujet |
|--:|---|---|---|--:|--:|--:|--:|---|
| 1 | `6dc6473c` | 2026-10-04 | opus | 947 k | 477 k | 386 | 0 | Ragnarokonline-v3 tree finish + webview bug |
| 2 | `24448860` | 2026-09-30 | opus | 749 k | 421 k | 409 | 0 | Remove initialize.sh, move to devcontainer.json |
| 3 | `ad936959` | 2026-09-25 | opus | 726 k | 393 k | 293 | 0 | Release-check échoué v1.5.0 |
| 4 | `7964983e` | 2026-10-04 | opus | 724 k | 461 k | 249 | 0 | Migrate ragnarokonline |
| 5 | `0f3e6b83` | 2026-09-23 | fable | 657 k | 441 k | 162 | 0 | Migrate trees to devcontainer-v3 |
| 6 | `2728dde8` | 2026-10-12 | opus | 654 k | 395 k | 379 | 0 | Serveur Discord sans arc 1 |
| 7 | `77231a2b` | 2026-10-11 | opus | 605 k | 335 k | 325 | 0 | Course 1.0 arc1-pilot reprise |
| 8 | `273aa026` | 2026-09-23 | opus | 594 k | 366 k | 266 | 0 | Release 1.4.1, 0.4.1, and symptems migration |
| 9 | `3f9c8020` | 2026-09-25 | opus | 583 k | 359 k | 271 | 0 | Daemon notify dans le paquet CLI, version 0.5.0 |
| 10 | `50372b51` | 2026-09-30 | opus | 566 k | 355 k | 185 | 0 | Symptems devcontainer v3 - Post publication 1.6.0 |
| 11 | `2254db79` | 2026-10-05 | opus | 558 k | 359 k | 272 | 0 | Devcontainer v3 upstream promotion |
| 12 | `766b6018` | 2026-09-17 | opus | 557 k | 366 k | 55 | 0 | Devc init wizard and scaffold templates |
| 13 | `e38472f0` | 2026-10-05 | opus | 554 k | 352 k | 209 | 0 | Séance 7.2f — français, dettes tranchées |
| 14 | `5b76b67e` | 2026-10-11 | opus | 552 k | 363 k | 183 | 0 | Cours-kevledev-tooling arc 1.0 pilot |
| 15 | `138c3196` | 2026-10-18 | opus | 543 k | 330 k | 145 | 0 | navidev-exos session 4 commit-and-solution |
| 16 | `120837bc` | 2026-09-30 | opus | 536 k | 325 k | 224 | 1 | INVENTAIRE.md modifications et image 1.6.0 |
| 17 | `933f7fc6` | 2026-10-05 | fable | 534 k | 376 k | 76 | 0 | Migrate dogfood devcontainer-v3 |
| 18 | `816a7e24` | 2026-10-10 | fable | 501 k | 388 k | 101 | 0 | Canva bridge tool and launcher |
| 19 | `204941d6` | 2026-10-10 | fable | 488 k | 374 k | 76 | 0 | Cours-kevledev-tooling foundation |
| 20 | `c8a0311a` | 2026-09-23 | opus | 480 k | 384 k | 77 | 0 | Migration tool — v2 tree compatibility and devc migrate |
| 21 | `6af4d6a2` | 2026-10-14 | opus | 470 k | 365 k | 139 | 0 | Course 1.6 — Design 1: visual hierarchy, typography, spacing |
| 22 | `46c05dd4` | 2026-10-14 | opus | 466 k | 330 k | 142 | 0 | Arc 1, course 1.8 — AI and LLM basics |
| 23 | `44ab3000` | 2026-10-17 | opus | 436 k | 242 k | 237 | 0 | Cours-corrections session 4 |
| 24 | `81aac675` | 2026-10-17 | fable | 434 k | 338 k | 81 | 0 | Arc 8 — Kev's tools and Claude Code daily use |
| 25 | `874d34ed` | 2026-10-11 | opus | 432 k | 319 k | 275 | 0 | Canva-bridge follow-ups |
