import OmarchyThemesKit
import SwiftUI

/// One theme. Downloaded themes render entirely from disk (no network); others are resolved
/// from GitHub on open. Downloads show progress and can be cancelled.
struct ThemeDetailView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    let entry: CatalogEntry

    @State private var details: ThemeDetails?
    @State private var isResolving = false
    @State private var resolveError: String?
    @State private var selectedWallpaper = 0
    /// The wallpaper shown in the large in-window preview, if it's open.
    @State private var previewIndex: Int?
    @State private var applyTarget: ApplyTarget?
    @State private var confirmRemove = false

    private var installed: InstalledTheme? { model.installedTheme(entry.slug) }
    private var download: DownloadState? { model.downloads[entry.slug] }
    private var palette: Palette? { installed?.palette ?? details?.palette }
    private var mode: AppearanceMode? { installed?.mode ?? details?.mode }
    private var isActive: Bool { installed != nil && model.activeSlug == entry.slug }
    private var isApplying: Bool { model.applyingSlug == entry.slug }

    private var wallpapers: [WallpaperItem] {
        if let installed {
            return installed.wallpapers.map { WallpaperItem(name: $0, url: installed.wallpaperURL($0)) }
        }
        return details?.wallpapers.map { WallpaperItem(name: $0.fileName, url: $0.downloadURL) } ?? []
    }

    private var paletteError: String? {
        if let installed {
            return installed.palette == nil ? "Couldn't read this theme's palette, so only the wallpaper can be applied." : nil
        }
        return details?.paletteError
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                header
                notices
                if let download {
                    DownloadProgressView(state: download) { model.cancelDownload(entry.slug) }
                }
                ThumbnailImage(url: installed?.screenshotURL ?? entry.screenshotURL, maxPixelSize: 1600, contentMode: .fit)
                    .aspectRatio(16 / 9, contentMode: .fit)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(.primary.opacity(0.1)))
                    .shadow(color: .black.opacity(0.12), radius: 8, y: 3)
                    .accessibilityLabel("\(entry.name) screenshot")

                if isResolving && installed == nil {
                    ProgressView("Reading theme from GitHub…")
                        .frame(maxWidth: .infinity)
                } else {
                    paletteSection
                    if let palette {
                        TerminalColorsSection(entry: entry, palette: palette)
                    }
                    wallpaperSection
                }
            }
            .padding(24)
            .frame(maxWidth: 1100, alignment: .leading)
            .frame(maxWidth: .infinity)
            .animation(.default, value: model.banners[.theme(entry.slug)])
        }
        .overlay { wallpaperPreview }
        .navigationTitle(entry.name)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                // A Button (not a Link) gets standard toolbar sizing; the title gives it a
                // sensible width next to the mark, plus a little horizontal breathing room.
                Button {
                    openURL(entry.repoURL)
                } label: {
                    Label("View on GitHub", image: "GitHubMark")
                        .labelStyle(.titleAndIcon)
                        .padding(.horizontal, 6)
                }
                .help("Open this theme's repository on GitHub")
                .accessibilityIdentifier("openOnGitHubButton")
            }
        }
        .task(id: entry.slug) { await load() }
        .onChange(of: wallpapers.map(\.name)) { _, names in
            // After Download Again with fewer wallpapers, fall back to the first rather than none.
            if !names.indices.contains(selectedWallpaper) { selectedWallpaper = 0 }
        }
        .sheet(item: $applyTarget) { target in
            ApplySheet(theme: target.theme, wallpaperFile: target.wallpaperFile) { options in
                Task { await apply(target, options: options) }
            }
        }
        .confirmationDialog("Remove \(entry.name)?", isPresented: $confirmRemove) {
            Button("Remove Download", role: .destructive) {
                if let installed { model.banners[.theme(entry.slug)] = model.remove(installed) }
            }
        } message: {
            RemoveDownloadMessage(isOnDesktop: isActive)
        }
    }

    // MARK: Header

    private var header: some View {
        HStack(alignment: .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 6) {
                Text(entry.name)
                    .font(.largeTitle.weight(.bold))
                    .textSelection(.enabled)
                HStack(spacing: 10) {
                    Link(destination: entry.repoURL) {
                        Label(entry.repoDisplay, systemImage: "chevron.left.forwardslash.chevron.right")
                    }
                    .font(.callout)
                    if let mode { ModeBadge(mode: mode) }
                    if isActive {
                        StatusBadge.applied
                    } else if installed != nil {
                        StatusBadge.downloaded
                    }
                }
            }
            Spacer()
            actions
        }
    }

    @ViewBuilder
    private var actions: some View {
        if download == nil {
            HStack(spacing: 8) {
                if let installed {
                    installedActions(installed)
                } else {
                    downloadActions
                }
            }
            .controlSize(.large)
        }
    }

    @ViewBuilder
    private func installedActions(_ installed: InstalledTheme) -> some View {
        if isApplying {
            ProgressView().controlSize(.small)
        }
        Button("Apply to Desktop…") {
            applyTarget = ApplyTarget(theme: installed, wallpaperFile: wallpapers[safe: selectedWallpaper]?.name)
        }
        .buttonStyle(.borderedProminent)
        .disabled(isApplying || model.applyingSlug != nil)
        .accessibilityIdentifier("applyButton")

        Menu {
            Button("Download Again") { Task { await model.download(entry) } }
            ShowInFinderButton(url: installed.directory)
            Divider()
            Button("Remove Download…", role: .destructive) { confirmRemove = true }
        } label: {
            Label("More", systemImage: "ellipsis.circle")
        }
        .menuIndicator(.hidden)
        .fixedSize()
        .accessibilityIdentifier("moreMenu")
    }

    @ViewBuilder
    private var downloadActions: some View {
        Button("Download") { Task { await model.download(entry) } }
            .disabled(details?.canApply != true)
            .accessibilityIdentifier("downloadButton")
        Button("Download and Apply…") {
            let index = selectedWallpaper
            Task {
                if let theme = await model.download(entry) {
                    applyTarget = ApplyTarget(theme: theme, wallpaperFile: theme.wallpapers[safe: index])
                }
            }
        }
        .buttonStyle(.borderedProminent)
        .disabled(details?.canApply != true)
        .accessibilityIdentifier("downloadAndApplyButton")
    }

    // MARK: Notices

    @ViewBuilder
    private var notices: some View {
        ContextBanner(context: .theme(entry.slug))

        if let resolveError, installed == nil {
            HStack(alignment: .firstTextBaseline) {
                BannerView(banner: Banner(kind: .error, title: "Couldn't read this theme", message: resolveError))
                Button("Try Again") { Task { await resolve(force: true) } }
            }
        }
        if installed == nil, let details, details.isStale {
            let reason = (details.staleReason as? GitHubError).map { " (\($0.localizedDescription))" } ?? ""
            BannerView(banner: Banner(kind: .warning, title: "Showing a cached copy", message: "GitHub couldn't be reached\(reason)."))
        }
        if installed == nil, let details, !details.canApply {
            BannerView(banner: Banner(
                kind: .warning, title: "Can't apply this theme",
                message: "It has no wallpapers and no readable color palette in a format this app understands."))
        }
    }

    // MARK: Palette

    @ViewBuilder
    private var paletteSection: some View {
        if let palette {
            VStack(alignment: .leading, spacing: 12) {
                Text("Colors").font(.title2.weight(.semibold))
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), alignment: .leading)], alignment: .leading, spacing: 12) {
                    ColorChip(name: "Background", color: palette.background)
                    ColorChip(name: "Foreground", color: palette.foreground)
                    ColorChip(name: "Accent", color: palette.accent)
                    if let cursor = palette.cursor { ColorChip(name: "Cursor", color: cursor) }
                    if let selection = palette.selection { ColorChip(name: "Selection", color: selection) }
                }
                if !palette.swatches.isEmpty {
                    Text("Terminal colors")
                        .font(.headline)
                        .padding(.top, 6)
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 6), count: 8), spacing: 6) {
                        ForEach(palette.swatches, id: \.name) { SwatchSquare(swatch: $0) }
                    }
                    .frame(maxWidth: 560)
                }
            }
        } else if let paletteError {
            Label(paletteError, systemImage: "paintpalette")
                .foregroundStyle(.secondary)
        }
    }

    // MARK: Wallpapers

    @ViewBuilder
    private var wallpaperSection: some View {
        let items = wallpapers
        if !items.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .firstTextBaseline) {
                    Text(items.count == 1 ? "Wallpaper" : "Wallpapers (\(items.count))")
                        .font(.title2.weight(.semibold))
                    Spacer()
                    Button {
                        preview(min(selectedWallpaper, items.count - 1))
                    } label: {
                        Label("Preview", systemImage: "arrow.up.left.and.arrow.down.right")
                    }
                    .help("Show the selected wallpaper larger")
                    .accessibilityIdentifier("previewWallpaperButton")
                }
                wallpaperPicker(items)
                Text(installed == nil
                     ? "The selected wallpaper is used when you apply the theme."
                     : "The selected wallpaper is used when you apply the theme. Downloaded wallpapers work offline.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func wallpaperPicker(_ items: [WallpaperItem]) -> some View {
        ScrollView(.horizontal) {
            LazyHStack(spacing: 12) {
                ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                    Button { selectedWallpaper = index } label: {
                        WallpaperThumbnail(item: item, isSelected: index == selectedWallpaper) {
                            preview(index)
                        }
                    }
                    .buttonStyle(.plain)
                    .simultaneousGesture(TapGesture(count: 2).onEnded { preview(index) })
                    .accessibilityLabel(item.name)
                    .accessibilityAddTraits(index == selectedWallpaper ? .isSelected : [])
                    .accessibilityAction(named: "Preview") { preview(index) }
                }
            }
            .padding(4)
        }
        .scrollIndicators(.visible)
    }

    @ViewBuilder
    private var wallpaperPreview: some View {
        if let previewIndex, wallpapers.indices.contains(previewIndex) {
            WallpaperPreview(
                items: wallpapers,
                index: Binding(get: { previewIndex }, set: { self.previewIndex = $0 }),
                selectedIndex: selectedWallpaper,
                onSelect: { selectedWallpaper = $0 },
                onClose: { withAnimation(.easeOut(duration: 0.15)) { self.previewIndex = nil } })
            .transition(.opacity)
        }
    }

    // MARK: Loading and applying

    private func load() async {
        if let installed {
            selectedWallpaper = model.preferredWallpaper(for: installed).flatMap(installed.wallpapers.firstIndex(of:)) ?? 0
            return // Everything needed is on disk; no network.
        }
        if let cached = model.cachedDetails(entry.slug) {
            details = cached
            return
        }
        await resolve(force: false)
    }

    private func resolve(force: Bool) async {
        isResolving = true
        resolveError = nil
        defer { isResolving = false }
        do {
            details = try await model.details(for: entry, force: force)
        } catch is CancellationError {
        } catch let error as URLError where error.code == .cancelled {
        } catch let error as URLError {
            resolveError = "Couldn't reach GitHub. Check your internet connection and try again. (\(error.localizedDescription))"
        } catch {
            resolveError = error.localizedDescription
        }
    }

    private func preview(_ index: Int) {
        withAnimation(.easeOut(duration: 0.15)) { previewIndex = index }
    }

    private func apply(_ target: ApplyTarget, options: ApplyOptions) async {
        if let file = target.wallpaperFile, let index = target.theme.wallpapers.firstIndex(of: file) {
            selectedWallpaper = index
        }
        let summary = await model.apply(target.theme, wallpaperFile: target.wallpaperFile, options: options)
        model.banners[.theme(entry.slug)] = Banner(summary)
    }
}

struct DownloadProgressView: View {
    let state: DownloadState
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                if let fraction = state.fraction {
                    ProgressView(value: fraction)
                } else {
                    ProgressView().progressViewStyle(.linear)
                }
                Text(state.status)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
            Button("Cancel", action: onCancel)
                .accessibilityIdentifier("cancelDownloadButton")
        }
        .padding(12)
        .background(.quaternary.opacity(0.5), in: RoundedRectangle(cornerRadius: 8))
        .accessibilityIdentifier("downloadProgress")
    }
}
