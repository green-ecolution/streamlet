#!/usr/bin/env bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR/lib.sh"

TILE_ARCHIVE_PREFIXES=${TILE_ARCHIVE_PREFIXES:-$TILE_ARCHIVE_PREFIX}
ARCHIVE_RETENTION=${ARCHIVE_RETENTION:-3}

cleanup_prefix() {
	local prefix archives archive
	prefix=$1

	echo "🗑️  Starting cleanup of $prefix from S3..."

	readarray -t archives < <(mc ls "remote/$S3_BUCKET/$prefix" --json | tac | tail -n "+$((ARCHIVE_RETENTION+1))")

	echo "📊 ${#archives[@]} archives found for cleanup"

	for archive in "${archives[@]}"; do
		archive=$(echo "$archive" | yq -p=json '.key')
		echo "🗑️  Removing $archive from S3..."
		mc rm "remote/$S3_BUCKET/$archive" || echo "⚠️  Failed to remove $archive"
	done

	echo "✅ Cleanup of $prefix complete"
}

main() {
	local prefixes prefix

	echo "╔═══════════════════════════════════════════════════════════╗"
	echo "║        🧹 GreenEcolution Archive Cleanup 🧹               ║"
	echo "╚═══════════════════════════════════════════════════════════╝"
	echo ""

	readarray -d ',' -t prefixes < <(echo -n "$TILE_ARCHIVE_PREFIXES")

	if [[ -n $DEBUG ]]; then
		echo "🐛 Debug mode enabled - Configuration:"
		print_config
		echo "TILE_ARCHIVE_PREFIXES: $TILE_ARCHIVE_PREFIXES"
		echo "ARCHIVE_RETENTION: $ARCHIVE_RETENTION"
		echo "prefixes: ${prefixes[@]}"
		echo ""
	fi

	echo "⏳ Waiting for MinIO..."
	wait_for_minio
	echo ""

	echo "🔐 Logging into MinIO..."
	login_minio
	echo ""

	echo "📦 Processing ${#prefixes[@]} prefix(es)..."
	for prefix in "${prefixes[@]}"; do
		cleanup_prefix "$prefix"
		echo ""
	done

	echo "╔═══════════════════════════════════════════════════════════╗"
	echo "║              🎉 Cleanup Complete! 🎉                      ║"
	echo "╚═══════════════════════════════════════════════════════════╝"
}

main "$@"
