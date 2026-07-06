#!/usr/bin/env bash

usage() {
	echo "📖 Usage: ${BASH_SOURCE[0]} [-hbm]"
	echo "   -h help"
	echo "   -b build local"
	echo "   -m no upload minio"
}

main() {
	local local_build=false
	local no_upload_minio=false

	while getopts "hbm" opt; do
		case "$opt" in
			h) usage; exit 0;;
			b) local_build=true;;
			m) no_upload_minio=true;;
			*) usage >&2; exit 1;;
		esac
	done

	shift $((OPTIND - 1))

	echo "🎯 Build mode: $(if $local_build; then echo 'local'; else echo 'docker'; fi)"
	echo "📤 Upload to MinIO: $(if $no_upload_minio; then echo 'disabled'; else echo 'enabled'; fi)"
	echo ""

	setup
	echo ""

	echo "📥 Downloading/copying tiles..."
	copy_or_download_tiles
	echo ""

	echo "🔧 Modifying tiles..."
	modify_tiles
	echo ""

	if "$local_build" ; then
		build_tiles_local
	else
		build_tiles
	fi
	echo ""

	echo "📦 Archiving tiles..."
	archive_tiles
	echo ""

	if ! "$no_upload_minio"; then
		echo "⏳ Waiting for S3..."
		wait_for_s3
		echo ""

		upload_archive
		echo ""
	fi

	if "$local_build" ; then
		cleanup_archive
	else
		echo "📁 Moving archive to local storage..."
		mv "$TILE_ARCHIVE" "$LOCAL_ARCHIVE_NAME"
		echo "✅ Archive moved"
	fi
}
