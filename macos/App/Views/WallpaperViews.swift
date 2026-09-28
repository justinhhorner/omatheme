import SwiftUI

/// A wallpaper on a theme page: a downloaded file, or its URL on GitHub.
struct WallpaperItem: Identifiable {
    var id: URL { url }
    let name: String
    let url: URL
}

/// Just the image, with a Preview button on hover: the file name is the tooltip, the
/// accessibility label (set by the caller) and shown in the large preview.
struct WallpaperThumbnail: View {
    let item: WallpaperItem
    let isSelected: Bool
    let onPreview: () -> Void
    @State private var isHovered = false

    var body: some View {
        ThumbnailImage(url: item.url, maxPixelSize: 400)
            .frame(width: 192, height: 108)
            .clipShape(RoundedRectangle(cornerRadius: 8))
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(isSelected ? Color.accentColor : .primary.opacity(0.12), lineWidth: isSelected ? 3 : 1))
            .help(item.name)
            .overlay(alignment: .topTrailing) {
                if isHovered {
                    Button(action: onPreview) {
                        Image(systemName: "arrow.up.left.and.arrow.down.right")
                            .font(.caption.weight(.semibold))
                            .padding(6)
                            .background(.regularMaterial, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .padding(6)
                    .help("Preview")
                    .accessibilityLabel("Preview \(item.name)")
                    .transition(.opacity)
                }
            }
            .onHover { hovering in
                withAnimation(.easeOut(duration: 0.12)) { isHovered = hovering }
            }
    }
}

/// A large preview of one wallpaper over the theme page, with previous/next (← →), "Use This
/// Wallpaper" to select it for Apply, and Esc or a click outside the controls to close.
struct WallpaperPreview: View {
    let items: [WallpaperItem]
    @Binding var index: Int
    let selectedIndex: Int
    let onSelect: (Int) -> Void
    let onClose: () -> Void

    private var item: WallpaperItem { items[index] }
    private var hasMany: Bool { items.count > 1 }

    var body: some View {
        ZStack {
            Rectangle()
                .fill(.black.opacity(0.82))
                .onTapGesture(perform: onClose)
                .accessibilityHidden(true)

            VStack(spacing: 16) {
                ZStack {
                    ProgressView()
                        .controlSize(.large)
                        .tint(.white)
                    ThumbnailImage(url: item.url, maxPixelSize: 2560, contentMode: .fit, showsBackground: false)
                        .id(item.id)
                        .shadow(color: .black.opacity(0.5), radius: 20, y: 8)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .allowsHitTesting(false) // a click anywhere but the controls reaches the backdrop and closes
                .accessibilityElement()
                .accessibilityLabel("Wallpaper \(item.name)")

                controls
            }
            .padding(28)
        }
        .environment(\.colorScheme, .dark)
        .accessibilityAddTraits(.isModal)
        .accessibilityIdentifier("wallpaperPreview")
    }

    private var controls: some View {
        HStack(spacing: 12) {
            if hasMany {
                Button { step(-1) } label: {
                    Image(systemName: "chevron.left")
                }
                .keyboardShortcut(.leftArrow, modifiers: [])
                .help("Previous wallpaper (←)")
                .accessibilityLabel("Previous wallpaper")
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(item.name)
                    .font(.headline)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if hasMany {
                    Text("\(index + 1) of \(items.count)")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
            }
            .frame(minWidth: 160, alignment: .leading)

            if hasMany {
                Button { step(1) } label: {
                    Image(systemName: "chevron.right")
                }
                .keyboardShortcut(.rightArrow, modifiers: [])
                .help("Next wallpaper (→)")
                .accessibilityLabel("Next wallpaper")
            }

            Spacer()

            if index == selectedIndex {
                Label("Selected", systemImage: "checkmark.circle.fill")
                    .foregroundStyle(.secondary)
            } else {
                Button("Use This Wallpaper") { onSelect(index) }
                    .help("Select this wallpaper for Apply")
            }

            Button(action: onClose) {
                Label("Close", systemImage: "xmark")
                    .labelStyle(.iconOnly)
            }
            .keyboardShortcut(.cancelAction)
            .help("Close preview (Esc)")
            .accessibilityIdentifier("closePreviewButton")
        }
        .controlSize(.large)
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
        .frame(maxWidth: 760)
    }

    private func step(_ delta: Int) {
        index = (index + delta + items.count) % items.count
    }
}
