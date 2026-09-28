import OmarchyThemesKit
import OmarchyThemesStores
import SwiftUI

/// The theme catalog as a screenshot grid, with search, a "downloaded only" filter and refresh.
struct GalleryView: View {
    @Environment(CatalogStore.self) private var catalog
    @Environment(ThemeLibrary.self) private var library
    @Environment(DesktopStore.self) private var desktop
    @State private var searchText = ""
    @AppStorage("gallery.downloadedOnly") private var downloadedOnly = false

    private var installedSlugs: Set<String> { Set(library.installed.map(\.slug)) }

    private var query: String { searchText.trimmingCharacters(in: .whitespaces) }

    private var isSearching: Bool { !query.isEmpty }

    private var visible: [CatalogEntry] {
        let query = query
        let installed = installedSlugs
        return catalog.entries.filter { entry in
            let matchesQuery = query.isEmpty
                || entry.name.localizedCaseInsensitiveContains(query)
                || entry.repoDisplay.localizedCaseInsensitiveContains(query)
            return matchesQuery && (!downloadedOnly || installed.contains(entry.slug))
        }
    }

    private var subtitle: String {
        guard !catalog.entries.isEmpty else { return "" }
        let count = visible.count == catalog.entries.count
            ? "\(catalog.entries.count) themes"
            : "\(visible.count) of \(catalog.entries.count) themes"
        return catalog.fetchedAt.map { "\(count) · updated \($0.relativeDescription)" } ?? count
    }

    var body: some View {
        content
            .navigationTitle("Themes")
            .navigationSubtitle(subtitle)
            .searchable(text: $searchText, placement: .toolbar, prompt: "Search themes")
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    Toggle(isOn: $downloadedOnly) {
                        Label("Downloaded Only", systemImage: "arrow.down.circle")
                    }
                    .help("Show only downloaded themes")
                    .accessibilityIdentifier("downloadedOnlyToggle")

                    RefreshButton()
                }
            }
            .task { await catalog.loadIfNeeded() }
    }

    @ViewBuilder
    private var content: some View {
        if catalog.entries.isEmpty {
            // The current theme is local, so it shows even when the catalog can't load.
            withCurrentTheme {
                if let error = catalog.error {
                    ContentUnavailableView {
                        Label("Couldn't Load Themes", systemImage: "wifi.exclamationmark")
                    } description: {
                        Text(error)
                    } actions: {
                        Button("Try Again") { Task { await catalog.refresh() } }
                            .disabled(catalog.isRefreshing)
                    }
                } else {
                    ProgressView("Loading themes from omarchy.org…")
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        } else if visible.isEmpty {
            if isSearching {
                ContentUnavailableView.search(text: searchText)
            } else {
                ContentUnavailableView {
                    Label("No Downloaded Themes", systemImage: "arrow.down.circle")
                } description: {
                    Text("You haven't downloaded any themes yet.")
                } actions: {
                    Button("Show All Themes") { downloadedOnly = false }
                }
            }
        } else {
            grid
        }
    }

    @ViewBuilder
    private func withCurrentTheme(@ViewBuilder _ content: () -> some View) -> some View {
        if let theme = desktop.currentTheme, !isSearching {
            VStack(spacing: 0) {
                currentThemeSection(theme).padding(20)
                content()
            }
        } else {
            content()
        }
    }

    private func currentThemeSection(_ theme: InstalledTheme) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Current Theme")
                .font(.title2.weight(.semibold))
                .accessibilityAddTraits(.isHeader)
            CurrentThemeCard(theme: theme)
        }
        .padding(.bottom, 8)
    }

    private var grid: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                notices
                if let theme = desktop.currentTheme, !isSearching {
                    currentThemeSection(theme)
                }
                sections(visible)
            }
            .padding(20)
        }
        .accessibilityIdentifier("galleryGrid")
    }

    @ViewBuilder
    private var notices: some View {
        if let notice = catalog.notice {
            BannerView(banner: Banner(kind: .warning, title: "Showing a cached catalog", message: notice)) {
                catalog.notice = nil
            }
        }
        if let notice = catalog.defaultThemesNotice {
            BannerView(banner: Banner(kind: .warning, title: "Omarchy's themes are missing", message: notice)) {
                catalog.defaultThemesNotice = nil
            }
        }
    }

    /// "Included with Omarchy", then "Community"; an empty section is hidden.
    private func sections(_ items: [CatalogEntry]) -> some View {
        let defaults = items.filter(\.isDefaultTheme)
        let community = items.filter { !$0.isDefaultTheme }
        let columns = [GridItem(.adaptive(minimum: 250, maximum: 380), spacing: 20)]

        return LazyVGrid(columns: columns, alignment: .leading, spacing: 24) {
            if !defaults.isEmpty {
                Section {
                    cards(defaults)
                } header: {
                    GallerySectionHeader(title: "Included with Omarchy", count: defaults.count)
                }
            }
            if !community.isEmpty {
                Section {
                    cards(community)
                } header: {
                    GallerySectionHeader(title: "Community", detail: "From omarchy.org/themes", count: community.count)
                        .padding(.top, defaults.isEmpty ? 0 : 12)
                }
            }
        }
    }

    private func cards(_ entries: [CatalogEntry]) -> some View {
        let installed = installedSlugs
        return ForEach(entries) { entry in
            NavigationLink(value: entry) {
                ThemeCard(entry: entry, isDownloaded: installed.contains(entry.slug), isActive: desktop.activeSlug == entry.slug)
            }
            .buttonStyle(.plain)
            .contextMenu {
                Link("Open on GitHub", destination: entry.repoURL)
                Button("Copy Link") { Pasteboard.copy(entry.repoURL.absoluteString) }
            }
        }
    }
}

struct GallerySectionHeader: View {
    let title: String
    var detail: String? = nil
    let count: Int

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(title).font(.title2.weight(.semibold))
            Text("\(count)")
                .font(.callout.monospacedDigit())
                .foregroundStyle(.secondary)
            Spacer()
            if let detail {
                Text(detail)
                    .font(.callout)
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}

struct RefreshButton: View {
    @Environment(CatalogStore.self) private var catalog

    var body: some View {
        Button {
            Task { await catalog.refresh() }
        } label: {
            Label("Refresh", systemImage: "arrow.clockwise")
        }
        .help("Reload the theme list from omarchy.org (⌘R)")
        .disabled(catalog.isRefreshing)
        .overlay {
            if catalog.isRefreshing {
                ProgressView().controlSize(.small)
            }
        }
        .accessibilityIdentifier("refreshButton")
    }
}

struct ThemeCard: View {
    let entry: CatalogEntry
    let isDownloaded: Bool
    let isActive: Bool
    @State private var isHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ThumbnailImage(url: entry.screenshotURL, maxPixelSize: 760)
                .aspectRatio(16 / 9, contentMode: .fit)
                .clipShape(RoundedRectangle(cornerRadius: 10))
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(.primary.opacity(isHovered ? 0.25 : 0.1)))
                .overlay(alignment: .topTrailing) {
                    if isActive {
                        StatusBadge.applied.padding(8)
                    } else if isDownloaded {
                        StatusBadge.downloaded.padding(8)
                    }
                }
                .shadow(color: .black.opacity(isHovered ? 0.22 : 0.08), radius: isHovered ? 10 : 4, y: isHovered ? 5 : 2)
                .scaleEffect(isHovered ? 1.015 : 1)

            VStack(alignment: .leading, spacing: 1) {
                Text(entry.name)
                    .font(.headline)
                    .lineLimit(1)
                Text(entry.repoDisplay)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            .padding(.horizontal, 2)
        }
        .contentShape(Rectangle())
        .onHover { hovering in
            withAnimation(.easeOut(duration: 0.15)) { isHovered = hovering }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(entry.name + (isActive ? ", applied" : isDownloaded ? ", downloaded" : ""))
        .accessibilityIdentifier("themeCard.\(entry.slug)")
    }
}
