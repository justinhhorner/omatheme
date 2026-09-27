import OmarchyThemesKit
import SwiftUI

/// Downloaded themes, with one-click Apply using the saved defaults from Settings.
struct DownloadedView: View {
    @Environment(AppModel.self) private var model
    let onOpen: (CatalogEntry) -> Void

    @State private var applyTarget: ApplyTarget?
    @State private var removing: InstalledTheme?

    var body: some View {
        List {
            ContextBanner(context: .downloaded)
                .listRowSeparator(.hidden)
            ForEach(model.installed) { theme in
                DownloadedRow(
                    theme: theme,
                    isActive: model.activeSlug == theme.slug,
                    isApplying: model.applyingSlug == theme.slug,
                    canApply: model.applyingSlug == nil,
                    onOpen: { onOpen(model.entry(for: theme)) },
                    onApply: { Task { await applyWithDefaults(theme) } },
                    onApplyWithOptions: { applyTarget = ApplyTarget(theme: theme, wallpaperFile: model.preferredWallpaper(for: theme)) },
                    onRemove: { removing = theme })
            }
        }
        .listStyle(.inset)
        .animation(.default, value: model.installed)
        .overlay {
            if model.installed.isEmpty {
                ContentUnavailableView {
                    Label("No Downloaded Themes", systemImage: "arrow.down.circle")
                } description: {
                    Text("Themes you download appear here, and apply without an internet connection.")
                } actions: {
                    Button("Browse Themes") { model.sidebarSelection = .gallery }
                }
            }
        }
        .navigationTitle("Downloaded")
        .navigationSubtitle(subtitle)
        .sheet(item: $applyTarget) { target in
            ApplySheet(theme: target.theme, wallpaperFile: target.wallpaperFile) { options in
                Task {
                    let summary = await model.apply(target.theme, wallpaperFile: target.wallpaperFile, options: options)
                    model.banners[.downloaded] = Banner(summary)
                }
            }
        }
        .confirmationDialog("Remove \(removing?.name ?? "theme")?", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }), presenting: removing) { theme in
            Button("Remove Download", role: .destructive) {
                model.remove(theme)
                model.banners[.downloaded] = Banner(kind: .info, title: "Download removed", message: "\(theme.name) was removed from this Mac.")
            }
        } message: { theme in
            Text(model.activeSlug == theme.slug
                ? "Its wallpapers are deleted from this Mac, including the one on your desktop. You can download the theme again at any time."
                : "Its wallpapers are deleted from this Mac. You can download the theme again at any time.")
        }
    }

    private var subtitle: String {
        switch model.installed.count {
        case 0: ""
        case 1: "1 theme · works offline"
        case let n: "\(n) themes · work offline"
        }
    }

    private func applyWithDefaults(_ theme: InstalledTheme) async {
        let summary = await model.applyWithDefaults(theme)
        model.banners[.downloaded] = Banner(summary)
    }
}

struct DownloadedRow: View {
    let theme: InstalledTheme
    let isActive: Bool
    let isApplying: Bool
    let canApply: Bool
    let onOpen: () -> Void
    let onApply: () -> Void
    let onApplyWithOptions: () -> Void
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: 14) {
            Button(action: onOpen) {
                HStack(spacing: 14) {
                    ThumbnailImage(url: theme.screenshotURL ?? theme.wallpapers.first.map(theme.wallpaperURL), maxPixelSize: 400)
                        .frame(width: 144, height: 81)
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.primary.opacity(0.1)))
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 8) {
                            Text(theme.name).font(.headline)
                            if isActive {
                                StatusBadge(title: "Applied", symbol: "checkmark.circle.fill", tint: .green)
                            }
                        }
                        HStack(spacing: 8) {
                            ModeBadge(mode: theme.mode)
                            if let palette = theme.palette {
                                HStack(spacing: 2) {
                                    ForEach([palette.background, palette.foreground, palette.accent], id: \.self) { color in
                                        Circle().fill(Color(color))
                                            .overlay(Circle().strokeBorder(.primary.opacity(0.15)))
                                            .frame(width: 11, height: 11)
                                    }
                                }
                            }
                        }
                        Text(detailText)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 0)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Show details")
            .accessibilityLabel("\(theme.name) details")

            if isApplying {
                ProgressView().controlSize(.small)
            }
            Button("Apply", action: onApply)
                .disabled(!canApply || theme.wallpapers.isEmpty)
                .help("Set this theme's wallpaper using your one-click Apply settings")
                .accessibilityIdentifier("oneClickApply.\(theme.slug)")
            Menu {
                Button("Apply with Options…", action: onApplyWithOptions)
                Button("Show Details", action: onOpen)
                Button("Show in Finder") { NSWorkspace.shared.activateFileViewerSelecting([theme.directory]) }
                Divider()
                Button("Remove Download…", role: .destructive, action: onRemove)
            } label: {
                Image(systemName: "ellipsis.circle")
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .accessibilityLabel("More actions for \(theme.name)")
        }
        .padding(.vertical, 6)
        .contextMenu {
            Button("Apply", action: onApply).disabled(!canApply || theme.wallpapers.isEmpty)
            Button("Apply with Options…", action: onApplyWithOptions)
            Button("Show Details", action: onOpen)
            Divider()
            Button("Remove Download…", role: .destructive, action: onRemove)
        }
    }

    private var detailText: String {
        let count = theme.wallpapers.count == 1 ? "1 wallpaper" : "\(theme.wallpapers.count) wallpapers"
        return "\(count) · downloaded \(theme.downloadedAt.relativeDescription)"
    }
}
