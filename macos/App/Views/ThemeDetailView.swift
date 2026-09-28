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
                    terminalSection
                    wallpaperSection
                }
            }
            .padding(24)
            .frame(maxWidth: 1100, alignment: .leading)
            .frame(maxWidth: .infinity)
            .animation(.default, value: model.banners[.theme(entry.slug)])
        }
        .overlay {
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
        .sheet(item: $applyTarget) { target in
            ApplySheet(theme: target.theme, wallpaperFile: target.wallpaperFile) { options in
                Task { await apply(target, options: options) }
            }
        }
        .confirmationDialog("Remove \(entry.name)?", isPresented: $confirmRemove) {
            Button("Remove Download", role: .destructive) {
                if let installed { model.remove(installed) }
            }
        } message: {
            Text(isActive
                ? "Its wallpapers are deleted from this Mac, including the one on your desktop. You can download the theme again at any time."
                : "Its wallpapers are deleted from this Mac. You can download the theme again at any time.")
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
                        StatusBadge(title: "Applied", symbol: "checkmark.circle.fill", tint: .green)
                    } else if installed != nil {
                        StatusBadge(title: "Downloaded", symbol: "arrow.down.circle.fill")
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
                        Button("Show in Finder") { NSWorkspace.shared.activateFileViewerSelecting([installed.directory]) }
                        Divider()
                        Button("Remove Download…", role: .destructive) { confirmRemove = true }
                    } label: {
                        Label("More", systemImage: "ellipsis.circle")
                    }
                    .menuIndicator(.hidden)
                    .fixedSize()
                    .accessibilityIdentifier("moreMenu")
                } else {
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
            }
            .controlSize(.large)
        }
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
            BannerView(banner: Banner(kind: .warning, title: "Can't apply this theme",
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

    // MARK: Terminal colors

    @ViewBuilder
    private var terminalSection: some View {
        if let palette {
            let exporter = model.selectedTerminal
            let themeName = installed?.name ?? entry.name
            let scheme = exporter.schemeName(forTheme: themeName)
            let isAdded = model.terminalRevision >= 0 && exporter.isAdded(slug: entry.slug, themeName: themeName)
            let colors = TerminalColors(palette)

            VStack(alignment: .leading, spacing: 10) {
                Text("Terminal Colors").font(.title2.weight(.semibold))
                HStack(spacing: 10) {
                    Picker("Terminal", selection: Binding(get: { exporter.id }, set: { model.selectTerminal($0) })) {
                        ForEach(model.terminals, id: \.id) { terminal in
                            Text(terminal.isInstalled ? terminal.displayName : "\(terminal.displayName) (not installed)")
                                .tag(terminal.id)
                        }
                    }
                    .fixedSize()
                    .accessibilityIdentifier("terminalPicker")

                    if isAdded {
                        Button(exporter.canRemove ? "Remove from \(exporter.displayName)" : "How to Remove…") {
                            showTerminalBanner(model.removeFromTerminal(entry))
                        }
                        .accessibilityIdentifier("removeFromTerminalButton")
                    } else {
                        Button("Add to \(exporter.displayName)") {
                            Task { showTerminalBanner(await model.addToTerminal(entry, palette: palette)) }
                        }
                        .disabled(!exporter.isInstalled)
                        .accessibilityIdentifier("addToTerminalButton")
                    }
                }
                HStack(spacing: 2) {
                    ForEach(Array(colors.ansi.enumerated()), id: \.offset) { index, color in
                        Rectangle()
                            .fill(Color(color))
                            .frame(width: 18, height: 12)
                            .help("\(index < 8 ? "" : "Bright ")\(TerminalColors.ansiNames[index % 8].lowercased()) \(color.hex.uppercased())")
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 3))
                .accessibilityHidden(true)
                Text(!exporter.isInstalled ? exporter.notInstalledHint
                     : isAdded ? "Available in \(exporter.displayName) as “\(scheme)”."
                     : exporter.addHint)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                ContextBanner(context: .terminal(entry.slug))
            }
        }
    }

    private func showTerminalBanner(_ banner: Banner) {
        withAnimation { model.banners[.terminal(entry.slug)] = banner }
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
                Text(installed == nil
                     ? "The selected wallpaper is used when you apply the theme."
                     : "The selected wallpaper is used when you apply the theme. Downloaded wallpapers work offline.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
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

struct ApplyTarget: Identifiable {
    let id = UUID()
    let theme: InstalledTheme
    let wallpaperFile: String?
}

struct WallpaperItem: Identifiable {
    var id: URL { url }
    let name: String
    let url: URL
}

struct WallpaperThumbnail: View {
    let item: WallpaperItem
    let isSelected: Bool
    let onPreview: () -> Void
    @State private var isHovered = false

    /// Just the image: the file name is the tooltip, the accessibility label (set by the caller)
    /// and shown in the large preview.
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

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
