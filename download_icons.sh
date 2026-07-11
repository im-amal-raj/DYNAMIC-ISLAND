#!/bin/bash
mkdir -p assets/icons

declare -A icon_map
icon_map=(
    ["music"]="music"
    ["clipboard"]="clipboard"
    ["bell"]="bell"
    ["check-circle"]="circle-check"
    ["sliders"]="sliders-horizontal"
    ["search"]="search"
    ["wifi"]="wifi"
    ["bluetooth"]="bluetooth"
    ["volume-2"]="volume-2"
    ["sun"]="sun"
    ["calendar"]="calendar"
    ["grid"]="layout-grid"
    ["more-horizontal"]="ellipsis"
)

echo "Downloading Lucide icons..."
for dest_name in "${!icon_map[@]}"; do
    src_name="${icon_map[$dest_name]}"
    curl -sL "https://raw.githubusercontent.com/lucide-icons/lucide/main/icons/${src_name}.svg" -o "assets/icons/${dest_name}.svg"
done

echo "Done! Icons saved to assets/icons/"
