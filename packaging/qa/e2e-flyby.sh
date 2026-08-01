#!/bin/bash
# E2E test skutečného přeletu: EventKit → scheduler → viditelný overlay.
#
# Vytvoří UNIKÁTNÍ testovací událost v samostatném lokálním kalendáři
# PTACEK-QA (pokud nejde vytvořit, použije se první zapisovatelný kalendář,
# událost ale nese unikátní marker v názvu). Čeká na přelet v logu appky,
# vyfotí obrazovku a po sobě uklidí — i při selhání (trap).
#
# NIKDY nemanipuluje existujícími schůzkami: maže výhradně událost s přesně
# vygenerovaným unikátním názvem, po ověření identity.
#
# Použití: ./e2e-flyby.sh [minuty_do_startu]   (default 7)
set -uo pipefail

MINUTES="${1:-7}"
MARKER="PTACEK-E2E-$(date +%s)-$$"
LOG="$HOME/Library/Application Support/Ptacek/Ptacek.log"
OUTDIR="${TMPDIR:-/tmp}/ptacek-e2e"
mkdir -p "$OUTDIR"

cleanup() {
  /usr/bin/osascript <<EOF >/dev/null 2>&1
tell application "Calendar"
  repeat with c in calendars
    set evs to (every event of c whose summary is "$MARKER")
    repeat with ev in evs
      if summary of ev is "$MARKER" then delete ev
    end repeat
  end repeat
end tell
EOF
  echo "cleanup: událost $MARKER smazána (pokud existovala)"
}
trap cleanup EXIT

if ! pgrep -f "MacOS/Ptacek" >/dev/null; then
  echo "FAIL: Ptáček neběží"; exit 1
fi

BASE=$(wc -l < "$LOG" 2>/dev/null || echo 0)

# událost: start za $MINUTES minut, v QA kalendáři (vytvoří se, pokud chybí)
/usr/bin/osascript <<EOF
set startD to (current date) + ($MINUTES * minutes)
set endD to startD + (15 * minutes)
tell application "Calendar"
  set qa to missing value
  repeat with c in calendars
    if name of c is "PTACEK-QA" then set qa to c
  end repeat
  if qa is missing value then
    try
      set qa to make new calendar with properties {name:"PTACEK-QA"}
    on error
      set qa to item 1 of calendars
    end try
  end if
  tell qa
    make new event with properties {summary:"$MARKER", start date:startD, end date:endD}
  end tell
end tell
EOF
echo "událost $MARKER vytvořena (start +$MINUTES min)"

# čekej na přelet (poll ≤5 min + lead; notifikace to typicky zkrátí na vteřiny)
DEADLINE=$(( SECONDS + MINUTES*60 + 60 ))
while [ $SECONDS -lt $DEADLINE ]; do
  NEW=$(tail -n +$((BASE+1)) "$LOG" | grep -c "Overlay okno vytvořeno (mode=event")
  if [ "$NEW" -gt 0 ]; then
    for j in 1 2 3; do
      screencapture -x "$OUTDIR/e2e-$MARKER-$j.png"
      sleep 2
    done
    echo "PASS: přelet detekován, screenshoty v $OUTDIR"
    tail -n +$((BASE+1)) "$LOG" | grep -E "Scheduler|Overlay" | tail -6
    # privacy kontrola: název události nesmí být v logu
    if tail -n +$((BASE+1)) "$LOG" | grep -q "$MARKER"; then
      echo "FAIL: název testovací události prosákl do logu (privacy regrese)"
      exit 1
    fi
    echo "PASS: log neobsahuje název události"
    exit 0
  fi
  sleep 5
done
echo "FAIL: přelet nepřišel do deadline"
tail -n +$((BASE+1)) "$LOG" | tail -10
exit 1
