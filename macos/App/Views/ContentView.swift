import OmarchyThemesKit
import OmarchyThemesStores
import SwiftUI

/// The main window: sidebar (Gallery, Downloaded) and a drill-in stack for each.
struct ContentView: View {
    @Environment(AppModel.self) private var model
    @Environment(Preferences.self) private var preferences
    @Environment(ThemeLibrary.self) private var library
    @State private var galleryPath: [CatalogEntry] = []
    @State private var downloadedPath: [CatalogEntry] = []

    var body: some View {
        @Bindable var model = model
        @Bindable var preferences = preferences

        NavigationSplitView {
            sidebar
        } detail: {
            switch model.sidebarSelection ?? .gallery {
            case .gallery:
                themeStack(path: $galleryPath) {
                    GalleryView()
                }
            case .downloaded:
                themeStack(path: $downloadedPath) {
                    DownloadedView { downloadedPath.append($0) }
                }
            }
        }
        .sheet(isPresented: $preferences.showWelcome, onDismiss: { preferences.dismissWelcome() }) {
            WelcomeView()
        }
        .overlay(alignment: .bottom) {
            if model.stores.isDryRun {
                DryRunBadge()
            }
        }
    }

    private var sidebar: some View {
        @Bindable var model = model

        return List(selection: $model.sidebarSelection) {
            Section("Themes") {
                Label("Gallery", systemImage: "square.grid.2x2")
                    .tag(SidebarItem.gallery)
                    .accessibilityIdentifier("sidebar.gallery")
                Label("Downloaded", systemImage: "arrow.down.circle")
                    .badge(library.installed.count)
                    .tag(SidebarItem.downloaded)
                    .accessibilityIdentifier("sidebar.downloaded")
            }
        }
        .navigationSplitViewColumnWidth(min: 170, ideal: 200, max: 280)
    }

    /// A drill-in stack whose pages are themes.
    private func themeStack(path: Binding<[CatalogEntry]>, @ViewBuilder root: () -> some View) -> some View {
        NavigationStack(path: path) {
            root()
                .navigationDestination(for: CatalogEntry.self) { ThemeDetailView(entry: $0) }
        }
    }
}

private struct DryRunBadge: View {
    var body: some View {
        Text("Dry run: applying a theme won't change your desktop")
            .font(.caption.weight(.medium))
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(.orange.opacity(0.9), in: Capsule())
            .foregroundStyle(.white)
            .padding(8)
            .allowsHitTesting(false)
    }
}
