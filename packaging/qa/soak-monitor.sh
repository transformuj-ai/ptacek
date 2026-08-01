#!/bin/bash
# Soak monitoring Ptáčka: hlídá symptomy incidentu 1021 a zdraví procesu.
# Release gate: za celou dobu soaku NULA zásahů (too many connections,
# EKCADError, pád procesu, store generace > očekávaná mez).
#
# Použití: nohup ./soak-monitor.sh > /dev/null 2>&1 &
#   výstup: ~/ptacek-soak/soak-YYYYMMDD-HHMM.csv + alerts.log
set -u

OUTDIR="$HOME/ptacek-soak"
mkdir -p "$OUTDIR"
STAMP=$(date +%Y%m%d-%H%M)
CSV="$OUTDIR/soak-$STAMP.csv"
ALERTS="$OUTDIR/alerts.log"
APPLOG="$HOME/Library/Application Support/Ptacek/Ptacek.log"

echo "ts,pid,uptime_s,queue_line,store_gen,ekcad_hits,toomany_hits" > "$CSV"
echo "soak start $(date)" >> "$ALERTS"
BASELINE_GEN=$(grep -c "store vytvořen" "$APPLOG")
echo "baseline store generaci: $BASELINE_GEN" >> "$ALERTS"

while true; do
  TS=$(date "+%Y-%m-%d %H:%M:%S")
  PID=$(pgrep -f "MacOS/Ptacek" | head -1)
  if [ -z "$PID" ]; then
    echo "$TS ALERT: proces Ptacek nebězí (pád/ukončení)" >> "$ALERTS"
    echo "$TS,,,PROCESS-DOWN,,," >> "$CSV"
  else
    UPT=$(ps -o etimes= -p "$PID" | tr -d ' ')
    QLINE=$(grep "Scheduler poll" "$APPLOG" | tail -1 | sed 's/,/;/g')
    GEN=$(grep -c "store vytvořen" "$APPLOG")
    # unified log za posledních 10 minut — jen symptomy, žádné parsování jako API
    HITS=$(/usr/bin/log show --last 10m --predicate 'process == "Ptacek" AND subsystem == "com.apple.eventkit"' --style compact 2>/dev/null)
    EKCAD=$(echo "$HITS" | grep -c "EKCADError")
    TOOMANY=$(echo "$HITS" | grep -c "too many connections")
    echo "$TS,$PID,$UPT,\"$QLINE\",$GEN,$EKCAD,$TOOMANY" >> "$CSV"
    if [ "$TOOMANY" -gt 0 ] || [ "$EKCAD" -gt 0 ]; then
      echo "$TS ALERT: EventKit chyby v unified logu (EKCAD=$EKCAD toomany=$TOOMANY)" >> "$ALERTS"
    fi
    if [ "$GEN" -gt "$BASELINE_GEN" ]; then
      echo "$TS ALERT: store generace vzrostla ($BASELINE_GEN -> $GEN), recovery za soaku" >> "$ALERTS"
    fi
  fi
  sleep 300
done
