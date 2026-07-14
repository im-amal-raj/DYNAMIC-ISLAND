#!/bin/bash
mkdir -p assets/icons

declare -A icon_map
icon_map=(
    ["music"]="music-notes-simple"
    ["clipboard"]="clipboard-text"
    ["bell"]="bell-simple"
    ["check-circle"]="check-circle"
    ["sliders"]="sliders-horizontal"
    ["search"]="magnifying-glass"
    ["wifi"]="wifi-high"
    ["bluetooth"]="bluetooth"
    ["volume-2"]="speaker-high"
    ["sun"]="sun"
    ["calendar"]="calendar-blank"
    ["grid"]="squares-four"
    ["more-horizontal"]="dots-three"
    ["lock"]="lock-key"
    ["moon"]="moon"
    ["rotate-cw"]="arrows-clockwise"
    ["power"]="power"
    ["cpu"]="cpu"
    ["trash"]="trash"
    ["plus"]="plus"
)

echo "Downloading Phosphor icons (light and fill weights)..."
for dest_name in "${!icon_map[@]}"; do
    src_name="${icon_map[$dest_name]}"
    
    # Download Light weight
    curl -sL "https://raw.githubusercontent.com/phosphor-icons/core/main/assets/light/${src_name}-light.svg" -o "assets/icons/${dest_name}-light.svg"
    
    # Download Fill weight
    curl -sL "https://raw.githubusercontent.com/phosphor-icons/core/main/assets/fill/${src_name}-fill.svg" -o "assets/icons/${dest_name}-fill.svg"
    
    # For backward compatibility during migration, copy light to default name
    cp "assets/icons/${dest_name}-light.svg" "assets/icons/${dest_name}.svg"
done

echo "Done! Icons saved to assets/icons/"
