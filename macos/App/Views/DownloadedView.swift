import OmarchyThemesKit
import SwiftUI

/// Downloaded themes, with one-click Apply using the saved defaults from Settings.
struct DownloadedView: View {
    @Environment(AppModel.self) private var model
    let onOpen: (CatalogEntry) -> Void

    @State private var applyTarget: ApplyTarget?
    @State private var removing: InstalledTheme?

    private var isConfirmingRemoval: Binding<Bool> {
        Binding(get: { removing != nil }, set: { if !$0 { removing = nil } })
    }

    private var subtitle: String {
        switch model.installed.count {
        case 0: ""
        case 1: "1 theme · works offline"
        case let n: "\(n) themes · work offline"
        }
    }

    var body: some View {
        List {
            ContextBanner(context: .downloaded)
                .listRowSeparator(.hidden)
            ForEach(model.installed) { theme in
                row(theme)
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
        .confirmationDialog("Remove \(removing?.name ?? "theme")?", isPresented: isConfirmingRemoval, presenting: removing) { theme in
            Button("Remove Download", role: .destructive) {
                model.banners[.downloaded] = model.remove(theme)
            }
        } message: { theme in
            RemoveDownloadMessage(isOnDesktop: model.activeSlug == theme.slug)
        }
    }

    private func row(_ theme: InstalledTheme) -> some View {
        DownloadedRow(
            theme: theme,
            isActive: model.activeSlug == theme.slug,
            isApplying: model.applyingSlug == theme.slug,
            canApply: model.applyingSlug == nil,
            onOpen: { onOpen(model.entry(for: theme)) },
            onApply: { Task { await applyWithDefaults(theme) } },
            onApplyWithOptions: {
                applyTarget = ApplyTarget(theme: theme, wallpaperFile: model.preferredWallpaper(for: theme))
            },
            onRemove: { removing = theme })
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

    private var detailText: String {
        "\(theme.wallpaperCountDescription) · downloaded \(theme.downloadedAt.relativeDescription)"
    }

    private var isApplyDisabled: Bool { !canApply || theme.wallpapers.isEmpty }

    var body: some View {
        HStack(spacing: 14) {
            Button(action: onOpen) {
                summary
            }
            .buttonStyle(.plain)
            .help("Show details")
            .accessibilityLabel("\(theme.name) details")

            if isApplying {
                ProgressView().controlSize(.small)
            }
            Button("Apply", action: onApply)
                .disabled(isApplyDisabled)
                .help("Set this theme's wallpaper using your one-click Apply settings")
                .accessibilityIdentifier("oneClickApply.\(theme.slug)")
            Menu {
                Button("Apply with Options…", action: onApplyWithOptions)
                Button("Show Details", action: onOpen)
                ShowInFinderButton(url: theme.directory)
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
            Button("Apply", action: onApply).disabled(isApplyDisabled)
            Button("Apply with Options…", action: onApplyWithOptions)
            Button("Show Details", action: onOpen)
            Divider()
            Button("Remove Download…", role: .destructive, action: onRemove)
        }
    }

    /// Thumbnail, name, light/dark, key colors and details; opens the theme when clicked.
    private var summary: some View {
        HStack(spacing: 14) {
            ThumbnailImage(url: theme.thumbnailURL, maxPixelSize: 400)
                .frame(width: 144, height: 81)
                .clipShape(RoundedRectangle(cornerRadius: 6))
                .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(.primary.opacity(0.1)))
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Text(theme.name).font(.headline)
                    if isActive {
                        StatusBadge.applied
                    }
                }
                HStack(spacing: 8) {
                    ModeBadge(mode: theme.mode)
                    if let palette = theme.palette {
                        keyColors(palette)
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

    private func keyColors(_ palette: Palette) -> some View {
        HStack(spacing: 2) {
            ForEach([palette.background, palette.foreground, palette.accent], id: \.self) { color in
                Circle().fill(Color(color))
                    .overlay(Circle().strokeBorder(.primary.opacity(0.15)))
                    .frame(width: 11, height: 11)
            }
        }
    }
}
