#!/usr/bin/env bash
set -euo pipefail

if (($# < 3 || $# > 4)); then
  echo "Usage: $0 SBOM.cdx.json CVE_OR_GHSA PURL [OUTPUT.cdx.json]" >&2
  exit 2
fi

sbom=$1
vuln=$2
purl=$3
output=${4:-"vex/$vuln.cdx.json"}

case "$vuln" in
  CVE-*) source=NVD ;;
  GHSA-*) source=GITHUB ;;
  OSV-*) source=OSV ;;
  SNYK-*) source=SNYK ;;
  *) echo "Unsupported vulnerability ID source: $vuln" >&2; exit 2 ;;
esac

ref=$(jq -er --arg purl "$purl" \
  '[.components[]? | select(.purl == $purl) | .["bom-ref"]][0]' "$sbom")
mkdir -p "$(dirname "$output")"

jq -n --arg vuln "$vuln" --arg source "$source" --arg ref "$ref" '{
  "$schema": "https://cyclonedx.org/schema/bom-1.6.schema.json",
  bomFormat: "CycloneDX",
  specVersion: "1.6",
  version: 1,
  metadata: {timestamp: (now | todateiso8601)},
  vulnerabilities: [{
    id: $vuln,
    source: {name: $source},
    analysis: {state: "in_triage"},
    affects: [{ref: $ref}]
  }]
}' > "$output"

echo "Generated $output"
