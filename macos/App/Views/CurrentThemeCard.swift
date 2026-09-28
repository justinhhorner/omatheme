import OmarchyThemesKit
import OmarchyThemesStores
import SwiftUI

/// The theme on the desktop, above the gallery: what it is, a way to its page, and its wallpapers,
/// any of which can be set with one click.
struct CurrentThemeCard: View {
    @Environment(Banners.self) private var banners
    @Environment(CatalogStore.self) private var catalog
    @Environment(DesktopStore.self) private var desktop
    let theme: InstalledTheme

    private var details: String {
        let mode = theme.mode == .light ? "Light theme" : "Dark theme"
        return "\(mode) · \(theme.wallpaperCountDescription)"
    }

    /// Background, foreground and accent, then up to eight terminal colors.
    private var strip: [RgbColor] {
        guard let palette = theme.palette else { return [] }
        return [palette.background, palette.foreground, palette.accent] + palette.swatches.prefix(8).map(\.color)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 16) {
                ThumbnailImage(url: theme.thumbnailURL, maxPixelSize: 640)
                    .frame(width: 224, height: 126)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(.primary.opacity(0.1)))
                    .accessibilityHidden(true)

                summary
                Spacer(minLength: 0)
            }

            if !theme.wallpapers.isEmpty {
                wallpapers
            }

            ContextBanner(context: .currentTheme)
        }
        .padding(16)
        .background(.quaternary.opacity(0.35), in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(.primary.opacity(0.08)))
        .animation(.default, value: banners[.currentTheme])
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Current theme, \(theme.name)")
        .accessibilityIdentifier("currentThemeCard")
    }

    private var summary: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Text(theme.name)
                    .font(.title2.weight(.bold))
                StatusBadge(title: "On your desktop", symbol: "checkmark.circle.fill", tint: .green)
            }
            Text(details)
                .foregroundStyle(.secondary)
            if !strip.isEmpty {
                colorStrip
            }
            NavigationLink(value: catalog.entry(for: theme)) {
                Text("View Theme")
            }
            .buttonStyle(.bordered)
            .padding(.top, 2)
            .accessibilityIdentifier("viewCurrentThemeButton")
        }
    }

    private var colorStrip: some View {
        HStack(spacing: 0) {
            ForEach(Array(strip.enumerated()), id: \.offset) { _, color in
                Rectangle().fill(Color(color))
            }
        }
        .frame(width: 220, height: 10)
        .clipShape(Capsule())
        .overlay(Capsule().strokeBorder(.primary.opacity(0.12)))
        .accessibilityHidden(true)
    }

    private var wallpapers: some View {
        ScrollView(.horizontal) {
            LazyHStack(spacing: 10) {
                ForEach(theme.wallpapers, id: \.self) { file in
                    CurrentWallpaperTile(
                        url: theme.wallpaperURL(file),
                        name: file,
                        isCurrent: file == desktop.currentWallpaper,
                        isApplying: file == desktop.settingWallpaper,
                        isEnabled: desktop.applyingSlug == nil
                    ) {
                        Task { await desktop.setCurrentWallpaper(file) }
                    }
                }
            }
            .padding(4)
        }
        .scrollIndicators(.visible)
    }
}

/// One of the current theme's wallpapers. The one on the desktop has an accent ring and a check
/// mark; clicking another sets it.
struct CurrentWallpaperTile: View {
    let url: URL
    let name: String
    let isCurrent: Bool
    let isApplying: Bool
    let isEnabled: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            ThumbnailImage(url: url, maxPixelSize: 320)
                .frame(width: 144, height: 81)
                .clipShape(RoundedRectangle(cornerRadius: 7))
                .overlay(
                    RoundedRectangle(cornerRadius: 7)
                        .strokeBorder(isCurrent ? Color.accentColor : .primary.opacity(0.12), lineWidth: isCurrent ? 3 : 1))
                .overlay(alignment: .topTrailing) {
                    if isCurrent {
                        Image(systemName: "checkmark.circle.fill")
                            .symbolRenderingMode(.palette)
                            .foregroundStyle(.white, Color.accentColor)
                            .font(.title3)
                            .shadow(color: .black.opacity(0.3), radius: 2)
                            .padding(5)
                    }
                }
                .overlay {
                    if isApplying {
                        ZStack {
                            RoundedRectangle(cornerRadius: 7).fill(.black.opacity(0.35))
                            ProgressView().controlSize(.small).tint(.white)
                        }
                    }
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!isEnabled) // clicking the current one is a no-op (AppModel.setCurrentWallpaper)
        .help(isCurrent ? "Current wallpaper" : "Set as desktop wallpaper")
        .accessibilityLabel(isCurrent ? "\(name), current wallpaper" : "Set \(name) as the desktop wallpaper")
        .accessibilityAddTraits(isCurrent ? .isSelected : [])
    }
}
