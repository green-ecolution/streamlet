#!/usr/bin/env bash

LOCAL_ARCHIVE_NAME="${LOCAL_ARCHIVE_NAME:-valhalla.tar.gz}"
TILE_ARCHIVE_PREFIX=${TILE_ARCHIVE_PREFIX:-valhalla-tiles}
DATA_DIR=${DATA_DIR:-./valhalla}

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
source "$SCRIPT_DIR/lib.sh"
source "$SCRIPT_DIR/generate-lib.sh"

VALHALLA_CONFIG_FILE=${VALHALLA_CONFIG_FILE:-./valhalla.json}

setup() {
	echo "🚀 Starting setup..."
	setup_data_dir
	echo "📋 Copying configuration file..."
	_dir=$(dirname "$VALHALLA_CONFIG_FILE")
	if [[ "$_dir" != "." ]]; then
		cp "$VALHALLA_CONFIG_FILE" . || fatal "base config file not found"
		echo "✅ Configuration file copied"
	fi
	echo "✅ Setup complete"
}

build_tiles() {
	echo "🏗️  Building Valhalla tiles using Docker..."
	echo "⚙️  Configuration:"
	echo "   • use_default_speeds_config=True"
	echo "   • force_rebuild=True"
	echo "   • serve_tiles=False"
	echo "   • build_elevation=True"
	echo ""

	docker run -it --rm \
		-u "$(id -u):$(id -g)" \
		-v "$PWD:/custom_files" \
		-e use_default_speeds_config=True \
		-e force_rebuild=True \
		-e serve_tiles=False \
		-e build_elevation=True \
		ghcr.io/valhalla/valhalla-scripted:3.6.0 || fatal "building tiles"

	echo ""
	echo "✅ Valhalla tiles built successfully"
}

build_tiles_local() {
	echo "🏗️  Building Valhalla tiles locally..."
	echo ""

	/valhalla/scripts/docker-entrypoint.sh build_tiles || fatal "building tiles"

	echo ""
	echo "✅ Valhalla tiles built successfully"
}

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║         🗺️  GreenEcolution Valhalla Builder 🗺️            ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

if [[ -n $DEBUG ]]; then
	echo "🐛 Debug mode enabled - Configuration:"
	print_config
	echo "VALHALLA_CONFIG_FILE: $VALHALLA_CONFIG_FILE"
	echo ""
fi

main "$@"

echo ""
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║              🎉 Process Complete! 🎉                      ║"
echo "╚═══════════════════════════════════════════════════════════╝"
